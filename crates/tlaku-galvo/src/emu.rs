//! Tarjeta LMC simulada: recibe inmediatos y tandas de lista, decodifica, ejecuta en un reloj
//! virtual y registra el rastro del haz.
//!
//! Sirve para comprobar que **nuestro código es consistente con lo que creemos que dice el
//! protocolo** (docs/hardware/galvo.md, sección 8.4). No prueba que una tarjeta real interprete
//! igual los parámetros: el modelo de estado (ocupada `0x04`, lista `0x20`), la capacidad de la
//! lista y el efecto de cada comando son suposiciones nuestras sacadas de galvoplotter y
//! MeerK40t (MIT).
//!
//! Modelo:
//! - Las tandas se acumulan en una cola; no se ejecuta nada hasta `ExecuteList`.
//! - Cada consulta de estado (`GetVersion`, `GetListStatus`) avanza la ejecución un número fijo
//!   de comandos; así la sesión debe esperar a "lista" antes de cada tanda, como en el real.
//! - **Armado** = láser habilitado (`EnableLaser`) y bit del láser del puerto y, en fibra, MO
//!   abierto. Un `MarkTo` o `LaserOnPoint` emite solo si está armado y la potencia es mayor que 0;
//!   un salto nunca emite.
//! - `Faults` inyecta errores de escritura, lectura y desconexión para probar el apagado.

use crate::cmd::{Imm, ListOp};
use crate::params;
use crate::transport::{PRODUCT_ID, Transport, TransportError, TransportInfo, VENDOR_ID};
use crate::wire::{self, BATCH_BYTES, BATCH_CMDS, CMD_LEN, REPLY_LEN};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use tlaku_doc::LaserSource;

/// Bit de "ocupada" en la palabra 3 de las respuestas (galvoplotter).
pub const STATUS_BUSY: u16 = 0x04;
/// Bit de "lista" (puede recibir otra tanda).
pub const STATUS_READY: u16 = 0x20;

/// Errores a inyectar. Los índices cuentan desde 1 las llamadas a `write` y a `read`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Faults {
    /// Estas escrituras fallan con error de E/S y no llegan a la tarjeta.
    pub fail_writes: Vec<usize>,
    /// Estas lecturas se pierden (tiempo agotado).
    pub fail_reads: Vec<usize>,
    /// A partir de la escritura número N+1, el USB está desconectado: nada llega a la tarjeta,
    /// que conserva su último estado.
    pub disconnect_after_writes: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct EmuConfig {
    pub source: LaserSource,
    pub field_mm: f64,
    pub laser_bit: u8,
    pub light_bit: u8,
    /// Tandas que caben en la memoria de la lista antes de que deje de estar "lista".
    pub capacity_batches: usize,
    /// Comandos que se ejecutan en cada consulta de estado.
    pub cmds_per_poll: usize,
    /// Entradas (`ReadPort`).
    pub inputs: u16,
    pub vendor_id: u16,
    pub product_id: u16,
    pub faults: Faults,
}

impl Default for EmuConfig {
    fn default() -> Self {
        EmuConfig {
            source: LaserSource::FiberQ,
            field_mm: 110.0,
            laser_bit: 0,
            light_bit: 8,
            capacity_batches: 8,
            cmds_per_poll: 128,
            inputs: 0,
            vendor_id: VENDOR_ID,
            product_id: PRODUCT_ID,
            faults: Faults::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegKind {
    Jump,
    Mark,
    /// Disparo en un punto (`LaserOnPoint`).
    Dot,
}

/// Un tramo del recorrido de los espejos.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Seg {
    pub kind: SegKind,
    pub from: (u16, u16),
    pub to: (u16, u16),
    /// El haz salía durante el tramo.
    pub emitting: bool,
    /// El puntero rojo estaba encendido.
    pub red: bool,
    pub power: u16,
    pub period: u16,
    /// Duración, us virtuales.
    pub us: f64,
}

#[derive(Default, Clone)]
struct Regs {
    jump_speed: u16,
    mark_speed: u16,
    delay_on: bool,
    delay_off: bool,
    delay_poly: bool,
    period: u16,
    power: u16,
    pulse: u16,
}

/// Estado de la tarjeta simulada (compartido con el transporte).
pub struct EmuState {
    pub cfg: EmuConfig,
    pub pos: (u16, u16),
    /// `EnableLaser` / `DisableLaser`.
    pub enabled: bool,
    pub port: u16,
    pub mo_open: bool,
    queue: VecDeque<wire::Raw>,
    pub executing: bool,
    regs: Regs,
    pub trace: Vec<Seg>,
    /// Todos los inmediatos recibidos, en orden.
    pub immediates: Vec<wire::Raw>,
    pub batches_received: usize,
    /// Cosas que una tarjeta real no aceptaría o que rompen una regla de seguridad.
    pub violations: Vec<String>,
    pub writes: usize,
    pub reads: usize,
    pub clock_us: f64,
    /// Estado armado tras cada escritura.
    pub armed_history: Vec<bool>,
    disconnected: bool,
    reply: Option<[u8; REPLY_LEN]>,
}

pub type EmuHandle = Arc<Mutex<EmuState>>;

fn wire_words(all: [u16; 4]) -> [u8; REPLY_LEN] {
    let mut b = [0u8; REPLY_LEN];
    for (i, v) in all.iter().enumerate() {
        b[2 * i..2 * i + 2].copy_from_slice(&v.to_le_bytes());
    }
    b
}

/// Bloquea el estado aunque otro hilo haya entrado en pánico.
pub fn lock(h: &EmuHandle) -> MutexGuard<'_, EmuState> {
    h.lock().unwrap_or_else(|e| e.into_inner())
}

impl EmuState {
    fn new(cfg: EmuConfig) -> Self {
        EmuState {
            cfg,
            pos: (0x8000, 0x8000),
            enabled: false,
            port: 0,
            mo_open: false,
            queue: VecDeque::new(),
            executing: false,
            regs: Regs::default(),
            trace: Vec::new(),
            immediates: Vec::new(),
            batches_received: 0,
            violations: Vec::new(),
            writes: 0,
            reads: 0,
            clock_us: 0.0,
            armed_history: Vec::new(),
            disconnected: false,
            reply: None,
        }
    }

    fn laser_mask(&self) -> u16 {
        1 << self.cfg.laser_bit
    }

    fn light_mask(&self) -> u16 {
        1 << self.cfg.light_bit
    }

    /// El láser está habilitado, su bit del puerto puesto y (en fibra) el MO abierto: lo que se
    /// marque a continuación sale.
    pub fn armed(&self) -> bool {
        self.enabled
            && self.port & self.laser_mask() != 0
            && (!params::is_fiber(self.cfg.source) || self.mo_open)
    }

    /// Nada habilitado: ni láser, ni bit del puerto, ni MO, ni ejecución en curso.
    pub fn laser_safe(&self) -> bool {
        !self.enabled && !self.mo_open && self.port & self.laser_mask() == 0 && !self.executing
    }

    pub fn red_on(&self) -> bool {
        self.port & self.light_mask() != 0
    }

    pub fn busy(&self) -> bool {
        self.executing
    }

    pub fn ready(&self) -> bool {
        self.queue.len() + BATCH_CMDS <= self.cfg.capacity_batches * BATCH_CMDS
    }

    fn status_word(&self) -> u16 {
        (if self.busy() { STATUS_BUSY } else { 0 }) | (if self.ready() { STATUS_READY } else { 0 })
    }

    /// Tramos con el haz saliendo.
    pub fn emitted(&self) -> Vec<&Seg> {
        self.trace.iter().filter(|s| s.emitting).collect()
    }

    pub fn jumps(&self) -> Vec<&Seg> {
        self.trace
            .iter()
            .filter(|s| s.kind == SegKind::Jump)
            .collect()
    }

    pub fn elapsed_s(&self) -> f64 {
        self.clock_us / 1e6
    }

    /// Comandos inmediatos recibidos con ese código.
    pub fn count_immediates(&self, code: u16) -> usize {
        self.immediates.iter().filter(|r| r.code == code).count()
    }

    fn violation(&mut self, m: impl Into<String>) {
        self.violations.push(m.into());
    }

    fn reply_words(&self, w: [u16; 3]) -> [u8; REPLY_LEN] {
        wire_words([w[0], w[1], w[2], self.status_word()])
    }

    fn immediate(&mut self, raw: wire::Raw) {
        self.immediates.push(raw);
        let imm = Imm::decode(&raw);
        let mut words = [0u16; 3];
        match imm {
            Imm::Unknown(r) => {
                self.violation(format!("inmediato no permitido {:#06x}", r.code));
            }
            Imm::DisableLaser => self.enabled = false,
            Imm::EnableLaser => self.enabled = true,
            Imm::ExecuteList => self.executing = true,
            Imm::GetVersion | Imm::GetListStatus => {
                self.advance();
                words = [0, 0x0102, 0];
            }
            Imm::GetSerialNo => {
                // El número de serie ocupa las cuatro palabras de la respuesta.
                self.reply = Some(wire_words([0x1234, 0x5678, 0x9ABC, 0xDEF0]));
                return;
            }
            Imm::GetPositionXY => words = [self.pos.0, self.pos.1, 0],
            Imm::GotoXY { x, y, .. } => {
                if self.armed() {
                    self.violation("GotoXY con la fuente armada");
                }
                let from = self.pos;
                self.push_seg(SegKind::Jump, from, (x, y), false, 1000);
                self.pos = (x, y);
            }
            Imm::ResetList => {
                self.queue.clear();
                self.executing = false;
            }
            Imm::SetEndOfList(_) => {}
            Imm::StopExecute => self.executing = false,
            Imm::WritePort(v) => self.port = v,
            Imm::ReadPort => words = [0, self.cfg.inputs, 0],
            Imm::FiberSetMo(v) => self.mo_open = v == 1,
        }
        self.reply = Some(self.reply_words(words));
    }

    fn batch(&mut self, bytes: &[u8]) {
        self.batches_received += 1;
        if !self.ready() {
            self.violation("tanda recibida con la memoria de lista llena");
            return;
        }
        match wire::unpack(bytes) {
            Ok(cmds) => self.queue.extend(cmds),
            Err(e) => self.violation(format!("tanda inválida: {e}")),
        }
    }

    fn push_seg(
        &mut self,
        kind: SegKind,
        from: (u16, u16),
        to: (u16, u16),
        emitting: bool,
        speed: u16,
    ) {
        let dx = f64::from(to.0) - f64::from(from.0);
        let dy = f64::from(to.1) - f64::from(from.1);
        let us = if speed > 0 {
            dx.hypot(dy) / f64::from(speed) * 1000.0
        } else {
            0.0
        };
        self.clock_us += us;
        self.trace.push(Seg {
            kind,
            from,
            to,
            emitting,
            red: self.red_on(),
            power: self.regs.power,
            period: self.regs.period,
            us,
        });
    }

    fn fire_ready(&self) -> bool {
        let r = &self.regs;
        r.mark_speed > 0 && r.delay_on && r.delay_off && r.delay_poly && r.period > 0
    }

    /// Ejecuta hasta `cmds_per_poll` comandos de la lista.
    fn advance(&mut self) {
        if !self.executing {
            return;
        }
        for _ in 0..self.cfg.cmds_per_poll {
            let Some(raw) = self.queue.pop_front() else {
                break;
            };
            match ListOp::decode(&raw) {
                ListOp::Ready | ListOp::JumpDelay(_) => {}
                ListOp::EndOfList => {
                    self.executing = false;
                    self.queue.clear();
                    break;
                }
                ListOp::Unknown(r) => {
                    self.violation(format!("comando de lista no permitido {:#06x}", r.code));
                }
                ListOp::JumpSpeed(v) => self.regs.jump_speed = v,
                ListOp::MarkSpeed(v) => self.regs.mark_speed = v,
                ListOp::LaserOnDelay(_) => self.regs.delay_on = true,
                ListOp::LaserOffDelay(_) => self.regs.delay_off = true,
                ListOp::PolygonDelay(_) => self.regs.delay_poly = true,
                ListOp::QSwitchPeriod(p) | ListOp::MarkFreq(p) => self.regs.period = p,
                ListOp::MarkCurrent(v) | ListOp::PowerRatio(v) => self.regs.power = v,
                ListOp::Co2Fpk(_) => {}
                ListOp::PulseWidth(v) => self.regs.pulse = v,
                ListOp::WritePort(v) => self.port = v,
                ListOp::DelayTime { ticks } => self.clock_us += f64::from(ticks) * 10.0,
                ListOp::JumpTo { x, y, .. } => {
                    let from = self.pos;
                    let speed = self.regs.jump_speed;
                    if speed == 0 {
                        self.violation("salto sin velocidad de salto");
                    }
                    self.push_seg(SegKind::Jump, from, (x, y), false, speed);
                    self.pos = (x, y);
                }
                ListOp::MarkTo { x, y, .. } => {
                    let from = self.pos;
                    let armed = self.armed();
                    if armed && !self.fire_ready() {
                        self.violation("marca con la fuente armada y parámetros sin fijar");
                    }
                    let emitting = armed && self.fire_ready() && self.regs.power > 0;
                    let speed = self.regs.mark_speed;
                    self.push_seg(SegKind::Mark, from, (x, y), emitting, speed);
                    self.pos = (x, y);
                }
                ListOp::LaserOnPoint { ticks } => {
                    let armed = self.armed();
                    if armed && !self.fire_ready() {
                        self.violation("disparo con la fuente armada y parámetros sin fijar");
                    }
                    let emitting = armed && self.fire_ready() && self.regs.power > 0;
                    let at = self.pos;
                    self.push_seg(SegKind::Dot, at, at, emitting, 1);
                    let us = f64::from(ticks) * 10.0;
                    self.clock_us += us;
                    if let Some(last) = self.trace.last_mut() {
                        last.us = us;
                    }
                }
            }
        }
    }

    fn write(&mut self, data: &[u8]) -> Result<(), TransportError> {
        if self.disconnected {
            return Err(TransportError::Disconnected);
        }
        self.writes += 1;
        if let Some(n) = self.cfg.faults.disconnect_after_writes
            && self.writes > n
        {
            self.disconnected = true;
            return Err(TransportError::Disconnected);
        }
        if self.cfg.faults.fail_writes.contains(&self.writes) {
            return Err(TransportError::Io("fallo inyectado de escritura".into()));
        }
        self.reply = None;
        match data.len() {
            CMD_LEN => match wire::Raw::from_bytes(data) {
                Ok(raw) => self.immediate(raw),
                Err(e) => self.violation(e.to_string()),
            },
            BATCH_BYTES => self.batch(data),
            n => self.violation(format!("escritura de {n} bytes")),
        }
        let armed = self.armed();
        self.armed_history.push(armed);
        Ok(())
    }

    fn read(&mut self) -> Result<[u8; REPLY_LEN], TransportError> {
        if self.disconnected {
            return Err(TransportError::Disconnected);
        }
        self.reads += 1;
        if self.cfg.faults.fail_reads.contains(&self.reads) {
            self.reply = None;
            return Err(TransportError::Timeout);
        }
        self.reply.take().ok_or(TransportError::Timeout)
    }
}

/// Transporte hacia la tarjeta simulada.
pub struct EmuTransport {
    state: EmuHandle,
}

impl Transport for EmuTransport {
    fn write(&mut self, data: &[u8]) -> Result<(), TransportError> {
        lock(&self.state).write(data)
    }

    fn read(&mut self) -> Result<[u8; REPLY_LEN], TransportError> {
        lock(&self.state).read()
    }

    fn info(&self) -> TransportInfo {
        let s = lock(&self.state);
        TransportInfo {
            name: "tarjeta simulada".into(),
            vendor_id: s.cfg.vendor_id,
            product_id: s.cfg.product_id,
            emulated: true,
        }
    }
}

/// Crea una tarjeta simulada y devuelve su transporte y el estado compartido para inspeccionarla.
pub fn emulator(cfg: EmuConfig) -> (EmuTransport, EmuHandle) {
    let state = Arc::new(Mutex::new(EmuState::new(cfg)));
    (
        EmuTransport {
            state: Arc::clone(&state),
        },
        state,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::Delay;
    use crate::transport::exchange;

    fn send(t: &mut EmuTransport, imm: Imm) -> [u16; 4] {
        exchange(t, &imm).unwrap()
    }

    fn upload(t: &mut EmuTransport, ops: &[ListOp]) {
        let raws: Vec<_> = ops.iter().map(ListOp::to_raw).collect();
        for b in wire::pack_batches(&raws).unwrap() {
            t.write(&b).unwrap();
        }
    }

    fn mark_list() -> Vec<ListOp> {
        vec![
            ListOp::Ready,
            ListOp::WritePort(1),
            ListOp::JumpSpeed(2000),
            ListOp::QSwitchPeriod(667),
            ListOp::MarkCurrent(2048),
            ListOp::MarkSpeed(100),
            ListOp::LaserOnDelay(Delay::from_us(100)),
            ListOp::LaserOffDelay(Delay::from_us(100)),
            ListOp::PolygonDelay(10),
            ListOp::JumpTo {
                x: 0x8100,
                y: 0x8000,
                dist: 256,
            },
            ListOp::MarkTo {
                x: 0x8200,
                y: 0x8000,
                dist: 256,
            },
            ListOp::EndOfList,
        ]
    }

    #[test]
    fn replies_carry_status_in_word_three() {
        let (mut t, h) = emulator(EmuConfig::default());
        let r = send(&mut t, Imm::GetVersion);
        assert_eq!(r[3] & STATUS_READY, STATUS_READY);
        assert_eq!(r[3] & STATUS_BUSY, 0);
        assert_eq!(
            send(&mut t, Imm::GetSerialNo)[..3],
            [0x1234, 0x5678, 0x9ABC]
        );
        assert_eq!(lock(&h).immediates.len(), 2);
    }

    #[test]
    fn nothing_runs_until_execute_and_marks_emit_only_when_armed() {
        let (mut t, h) = emulator(EmuConfig::default());
        upload(&mut t, &mark_list());
        send(&mut t, Imm::GetVersion);
        assert!(lock(&h).trace.is_empty(), "sin ExecuteList no se ejecuta");

        // Sin armar: la marca recorre pero no emite.
        send(&mut t, Imm::ExecuteList);
        while lock(&h).busy() {
            send(&mut t, Imm::GetVersion);
        }
        {
            let s = lock(&h);
            assert_eq!(s.trace.len(), 2);
            assert!(s.emitted().is_empty());
            assert!(s.violations.is_empty());
        }

        // Armada: emite solo la marca.
        send(&mut t, Imm::ResetList);
        send(&mut t, Imm::EnableLaser);
        send(&mut t, Imm::FiberSetMo(1));
        upload(&mut t, &mark_list());
        send(&mut t, Imm::ExecuteList);
        while lock(&h).busy() {
            send(&mut t, Imm::GetVersion);
        }
        let s = lock(&h);
        assert!(s.armed());
        assert_eq!(s.emitted().len(), 1);
        assert_eq!(s.emitted()[0].kind, SegKind::Mark);
        assert!(s.jumps().iter().all(|j| !j.emitting));
    }

    #[test]
    fn busy_and_ready_follow_the_queue() {
        let cfg = EmuConfig {
            capacity_batches: 2,
            cmds_per_poll: 10,
            ..EmuConfig::default()
        };
        let (mut t, h) = emulator(cfg);
        let long: Vec<ListOp> = std::iter::once(ListOp::Ready)
            .chain(std::iter::repeat_n(ListOp::JumpSpeed(5), 600))
            .chain(std::iter::once(ListOp::EndOfList))
            .collect();
        let raws: Vec<_> = long.iter().map(ListOp::to_raw).collect();
        let batches = wire::pack_batches(&raws).unwrap();
        assert_eq!(batches.len(), 3);
        t.write(&batches[0]).unwrap();
        t.write(&batches[1]).unwrap();
        // Con dos tandas la memoria está llena: ya no está lista.
        assert_eq!(send(&mut t, Imm::GetVersion)[3] & STATUS_READY, 0);
        // Mandar otra tanda entonces es un error.
        t.write(&batches[2]).unwrap();
        assert!(!lock(&h).violations.is_empty());
        // Ejecutar libera espacio.
        lock(&h).violations.clear();
        send(&mut t, Imm::ExecuteList);
        let mut polls = 0;
        while send(&mut t, Imm::GetVersion)[3] & STATUS_READY == 0 {
            polls += 1;
            assert!(polls < 200);
        }
        assert!(lock(&h).busy());
    }

    #[test]
    fn unknown_commands_are_flagged() {
        let (mut t, h) = emulator(EmuConfig::default());
        t.write(&wire::Raw::bare(0x0015).to_bytes()).unwrap();
        t.write(&wire::Raw::bare(0x0042).to_bytes()).unwrap();
        t.write(&[0u8; 7]).unwrap();
        assert_eq!(lock(&h).violations.len(), 3);
    }

    #[test]
    fn injected_faults_hit_the_chosen_calls() {
        let cfg = EmuConfig {
            faults: Faults {
                fail_writes: vec![2],
                fail_reads: vec![3],
                disconnect_after_writes: Some(4),
            },
            ..EmuConfig::default()
        };
        let (mut t, h) = emulator(cfg);
        send(&mut t, Imm::GetVersion); // escritura 1, lectura 1
        assert!(matches!(
            t.write(&Imm::GetVersion.to_raw().to_bytes()),
            Err(TransportError::Io(_))
        )); // escritura 2
        send(&mut t, Imm::EnableLaser); // escritura 3, lectura 2
        t.write(&Imm::GetVersion.to_raw().to_bytes()).unwrap(); // escritura 4
        assert_eq!(t.read(), Err(TransportError::Timeout)); // lectura 3 perdida
        assert_eq!(
            t.write(&Imm::DisableLaser.to_raw().to_bytes()),
            Err(TransportError::Disconnected)
        ); // escritura 5
        // Tras desconectarse, la tarjeta conserva su último estado.
        assert!(lock(&h).enabled);
        assert_eq!(t.read(), Err(TransportError::Disconnected));
    }
}
