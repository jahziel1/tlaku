//! Núcleo del emulador: una controladora completa como máquina de estados que avanza con
//! tiempo simulado. No tiene hilos ni relojes propios: se le dan bytes con [`Machine::receive`],
//! se le hace avanzar con [`Machine::advance_to`] y se leen sus respuestas con
//! [`Machine::take_output`]. [`crate::VirtualMachine`] la envuelve como `Transport`.
//!
//! Qué modela (ver la documentación del crate para el detalle y los límites):
//!
//! - Búfer RX circular (`rx_size - 1` bytes utilizables) que se llena de verdad si el
//!   intérprete está bloqueado; los bytes que no caben se pierden y se cuentan.
//! - Planificador de `planner_blocks` bloques: una línea de movimiento recibe `ok` al entrar;
//!   con el planificador lleno el intérprete se bloquea (y el RX se acumula).
//! - Estados y comandos en tiempo real de GRBL 1.1 (y extras de grblHAL).
//! - Cinemática trapezoidal simple, láser M3/M4/M5 con `$32`, rastro del láser.

use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

use crate::fault::{Fault, Scheduled, Trigger};
use crate::model::{Action, Block, Effect, Gc, MotionMode, Parsed, StateKind};
use crate::profile::{Family, LineEnding, Profile, Protocol, Setting, format_value};
use crate::trace::{LaserSegment, LaserTrace, MoveKind, SpindleMode};

/// Paso de simulación.
const STEP: Duration = Duration::from_millis(1);
/// Velocidad mínima (mm/s) para que un bloque siempre termine.
const MIN_SPEED: f64 = 0.5;
/// Velocidad mínima de paso por una esquina (mm/s).
const MIN_JUNCTION: f64 = 2.0;
/// Máximo de bytes crudos de una línea antes de descartar el resto.
const MAX_RAW_LINE: usize = 2048;

/// Opciones de arranque de la máquina emulada.
#[derive(Debug, Clone)]
pub struct Options {
    /// Imprime el banner al crearla (equivale a abrir el puerto y que la placa se reinicie).
    pub banner_on_connect: bool,
    /// Posición de máquina inicial.
    pub start_position: [f64; 3],
    /// Arranca ya referenciada y sin el bloqueo de alarma que exige `$22=1`.
    pub homed: bool,
    /// Paso de simulación.
    pub step: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            banner_on_connect: true,
            start_position: [0.0; 3],
            homed: false,
            step: STEP,
        }
    }
}

/// Lo que recibió la controladora, para inspección de las pruebas.
#[derive(Debug, Clone, PartialEq)]
pub enum Received {
    /// Línea completa tal como llegó (sin terminador).
    Line(String),
    /// Byte en tiempo real.
    Realtime(u8),
}

/// Entrada del registro de recepción.
#[derive(Debug, Clone, PartialEq)]
pub struct LogEntry {
    pub at: Duration,
    pub what: Received,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reply {
    Normal,
    Error(u32),
    Drop,
}

#[derive(Debug, Clone, Copy)]
struct LineCtx {
    bytes: usize,
    reply: Reply,
}

struct Pending {
    actions: VecDeque<Action>,
    dwell_until: Option<Duration>,
    awaiting_resume: bool,
    homing: bool,
}

struct Exec {
    block: Block,
    done: f64,
}

struct Homing {
    start: Duration,
    end: Duration,
    from: [f64; 3],
}

#[derive(Default, Clone, Copy)]
struct ReportState {
    wco_counter: u32,
    ov_counter: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ResetKind {
    /// Reset pedido por el anfitrión: ALARM:3 si había movimiento.
    FromHost,
    /// Reinicio de la placa en pleno movimiento.
    Motion,
    /// Reinicio en reposo.
    Quiet,
}

/// Controladora emulada.
pub struct Machine {
    pub(crate) profile: Profile,
    opts: Options,
    now: Duration,
    // Enlace.
    out: VecDeque<u8>,
    held: Vec<u8>,
    silence: Option<(Duration, bool)>,
    connected: bool,
    line_ending: LineEnding,
    rx: VecDeque<u8>,
    line_buf: Vec<u8>,
    line_bytes: usize,
    in_flight: usize,
    max_in_flight: usize,
    overflow_bytes: usize,
    log: Vec<LogEntry>,
    pub(crate) lines_seen: usize,
    line_ctx: LineCtx,
    next_reply: Option<Reply>,
    last_error: Option<u32>,
    // Parámetros persistentes.
    settings: BTreeMap<u32, Setting>,
    user_info: String,
    startup: [String; 2],
    pub(crate) wcs: [[f64; 3]; 6],
    pub(crate) g28: [f64; 3],
    pub(crate) g30: [f64; 3],
    pub(crate) g92: [f64; 3],
    // Estado modal.
    pub(crate) gc: Gc,
    // Sistema.
    alarm: Option<u32>,
    hold: Option<bool>,
    door: Option<u8>,
    door_open: bool,
    door_resume_at: Option<Duration>,
    check: bool,
    sleeping: bool,
    homing: Option<Homing>,
    halted: bool,
    tool_pending: bool,
    limit_lock: bool,
    position_lost: bool,
    homed: bool,
    jog_cancel: bool,
    block_done: bool,
    resets: usize,
    // Movimiento.
    pos: [f64; 3],
    v: f64,
    planner: VecDeque<Block>,
    current: Option<Exec>,
    block_seq: u64,
    pending: Option<Pending>,
    exec_spindle: (SpindleMode, f64),
    spindle_stop_ovr: bool,
    ovr_feed: u32,
    ovr_rapid: u32,
    ovr_spindle: u32,
    // Rastro.
    trace: LaserTrace,
    open_seg: Option<(usize, u64, bool)>,
    // Reporte.
    rep: ReportState,
    // Fallas.
    faults: Vec<Scheduled>,
    nan: Option<bool>,
    // Protocolo RepRap.
    /// Velocidad (0-255) de cada ventilador, por índice `P`.
    fans: BTreeMap<u32, f64>,
    /// Modo inline de Marlin (`M3 S0 I`): la potencia viaja con cada movimiento.
    inline: bool,
    /// Último número de línea aceptado (líneas numeradas de Marlin).
    last_line: i64,
    /// Altera un byte de la próxima línea (falla `CorruptLine`).
    corrupt_next: bool,
    /// Bytes de la línea en curso, para el analizador de emergencia.
    ep_buf: Vec<u8>,
}

fn fmt3(v: f64) -> String {
    format_value(v, 3)
}

fn fmt_trim(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Quita comentarios y espacios y pasa a mayúsculas.
fn clean_line(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_paren = false;
    for c in raw.chars() {
        match c {
            ';' if !in_paren => break,
            '(' => in_paren = true,
            ')' => in_paren = false,
            _ if in_paren || c.is_whitespace() || c.is_control() => {}
            _ => out.push(c.to_ascii_uppercase()),
        }
    }
    out
}

impl Machine {
    pub fn new(profile: Profile, opts: Options) -> Self {
        let settings: BTreeMap<u32, Setting> =
            profile.settings.iter().map(|s| (s.id, s.clone())).collect();
        let pos = opts.start_position;
        let line_ending = profile.line_ending;
        let mut m = Self {
            profile,
            now: Duration::ZERO,
            out: VecDeque::new(),
            held: Vec::new(),
            silence: None,
            connected: true,
            line_ending,
            rx: VecDeque::new(),
            line_buf: Vec::new(),
            line_bytes: 0,
            in_flight: 0,
            max_in_flight: 0,
            overflow_bytes: 0,
            log: Vec::new(),
            lines_seen: 0,
            line_ctx: LineCtx {
                bytes: 0,
                reply: Reply::Normal,
            },
            next_reply: None,
            last_error: None,
            settings,
            user_info: String::new(),
            startup: [String::new(), String::new()],
            wcs: [[0.0; 3]; 6],
            g28: [0.0; 3],
            g30: [0.0; 3],
            g92: [0.0; 3],
            gc: Gc::new(pos),
            alarm: None,
            hold: None,
            door: None,
            door_open: false,
            door_resume_at: None,
            check: false,
            sleeping: false,
            homing: None,
            halted: false,
            tool_pending: false,
            limit_lock: false,
            position_lost: false,
            homed: opts.homed,
            jog_cancel: false,
            block_done: false,
            resets: 0,
            pos,
            v: 0.0,
            planner: VecDeque::new(),
            current: None,
            block_seq: 0,
            pending: None,
            exec_spindle: (SpindleMode::Off, 0.0),
            spindle_stop_ovr: false,
            ovr_feed: 100,
            ovr_rapid: 100,
            ovr_spindle: 100,
            trace: LaserTrace::default(),
            open_seg: None,
            rep: ReportState::default(),
            faults: Vec::new(),
            nan: None,
            fans: BTreeMap::new(),
            inline: false,
            last_line: 0,
            corrupt_next: false,
            ep_buf: Vec::new(),
            opts,
        };
        if m.homing_lock() && !m.opts.homed {
            m.alarm = Some(0);
        }
        if m.opts.banner_on_connect {
            let banner = m.profile.banner_connect.clone();
            for l in banner {
                m.emit_line(&l);
            }
            if m.alarm.is_some() && m.profile.unlock_message {
                m.emit_line("[MSG:'$H'|'$X' to unlock]");
            }
        }
        m
    }

    // -----------------------------------------------------------------------
    // Acceso para pruebas
    // -----------------------------------------------------------------------

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// Tiempo simulado actual de la controladora.
    pub fn now(&self) -> Duration {
        self.now
    }

    /// Posición de máquina.
    pub fn mpos(&self) -> [f64; 3] {
        self.pos
    }

    /// Posición de trabajo (MPos - WCO).
    pub fn wpos(&self) -> [f64; 3] {
        let w = self.wco();
        [self.pos[0] - w[0], self.pos[1] - w[1], self.pos[2] - w[2]]
    }

    /// Desplazamiento de coordenadas de trabajo activo.
    pub fn wco(&self) -> [f64; 3] {
        self.wco_for(self.gc.wcs)
    }

    pub(crate) fn wco_for(&self, wcs: usize) -> [f64; 3] {
        let o = self.wcs[wcs.min(5)];
        [o[0] + self.g92[0], o[1] + self.g92[1], o[2] + self.g92[2]]
    }

    pub fn state(&self) -> StateKind {
        if let Some(c) = self.alarm {
            return StateKind::Alarm(c);
        }
        if self.sleeping {
            return StateKind::Sleep;
        }
        if self.homing.is_some() {
            return StateKind::Home;
        }
        if self.check {
            return StateKind::Check;
        }
        if let Some(d) = self.door {
            return StateKind::Door(d);
        }
        if self.tool_pending {
            return StateKind::Tool;
        }
        if let Some(done) = self.hold {
            return StateKind::Hold(u8::from(!done));
        }
        let jog = self.current.as_ref().map(|e| e.block.kind) == Some(MoveKind::Jog)
            || self.planner.front().map(|b| b.kind) == Some(MoveKind::Jog);
        if jog {
            return StateKind::Jog;
        }
        if self.current.is_some() || !self.planner.is_empty() {
            return StateKind::Run;
        }
        StateKind::Idle
    }

    pub fn trace(&self) -> &LaserTrace {
        &self.trace
    }

    pub fn clear_trace(&mut self) {
        self.trace.clear();
        self.open_seg = None;
    }

    /// Potencia efectiva del láser ahora (0 si está apagado).
    pub fn laser_power(&self) -> f64 {
        self.current_power()
    }

    fn current_power(&self) -> f64 {
        self.laser_sample().map_or(0.0, |s| s.0)
    }

    pub fn laser_on(&self) -> bool {
        self.current_power() > 0.0
    }

    pub fn log(&self) -> &[LogEntry] {
        &self.log
    }

    /// Bytes que no cupieron en el búfer RX y se perdieron.
    pub fn rx_overflow_bytes(&self) -> usize {
        self.overflow_bytes
    }

    /// Mayor cantidad de bytes de líneas enviadas y sin respuesta que se llegó a tener.
    /// Con conteo de caracteres correcto nunca pasa de `rx_size - 1`.
    pub fn max_in_flight(&self) -> usize {
        self.max_in_flight
    }

    /// Bytes de líneas enviadas aún sin respuesta.
    pub fn in_flight(&self) -> usize {
        self.in_flight
    }

    /// Líneas no vacías recibidas hasta ahora.
    pub fn lines_received(&self) -> usize {
        self.lines_seen
    }

    pub fn planner_used(&self) -> usize {
        self.planner.len() + usize::from(self.current.is_some())
    }

    pub fn alarm(&self) -> Option<u32> {
        self.alarm.filter(|&c| c > 0)
    }

    /// `true` si la posición se declaró perdida (ALARM:3 o ALARM:6) y no se ha vuelto a
    /// referenciar.
    pub fn position_lost(&self) -> bool {
        self.position_lost
    }

    pub fn resets(&self) -> usize {
        self.resets
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn setting(&self, id: u32) -> f64 {
        self.settings.get(&id).map_or(0.0, |s| s.value)
    }

    pub fn set_setting(&mut self, id: u32, value: f64) {
        match self.settings.get_mut(&id) {
            Some(s) => s.value = value,
            None => {
                self.settings.insert(id, Setting::new(id, value, 0));
            }
        }
    }

    /// Cierra o abre la puerta de seguridad (`true` = cerrada).
    pub fn set_door_closed(&mut self, closed: bool) {
        if closed {
            self.door_open = false;
            if self.door == Some(1) {
                self.door = Some(0);
            }
        } else if self.door.is_none() || self.door == Some(0) {
            self.door_open = true;
            self.safety_door();
        }
    }

    /// Bytes pendientes de leer por el anfitrión.
    pub fn output_len(&self) -> usize {
        self.out.len()
    }

    /// Copia en `buf` lo que haya para el anfitrión y devuelve cuántos bytes.
    pub fn take_output(&mut self, buf: &mut [u8]) -> usize {
        let n = buf.len().min(self.out.len());
        for (slot, b) in buf.iter_mut().zip(self.out.drain(..n)) {
            *slot = b;
        }
        n
    }

    /// Descarta lo no leído (equivale a `clear_input` del anfitrión).
    pub fn clear_output(&mut self) {
        self.out.clear();
    }

    /// Todo lo pendiente como texto (para pruebas sin transporte).
    pub fn drain_output_string(&mut self) -> String {
        let v: Vec<u8> = self.out.drain(..).collect();
        String::from_utf8_lossy(&v).into_owned()
    }

    /// Reporte de estado tal como lo daría `?` ahora, sin tocar los contadores de WCO/Ov.
    pub fn status_line(&self) -> String {
        let mut tmp = self.rep;
        self.status_with(&mut tmp, false, true)
    }

    // -----------------------------------------------------------------------
    // Enlace
    // -----------------------------------------------------------------------

    fn emit_raw(&mut self, b: &[u8]) {
        if !self.connected {
            return;
        }
        if let Some((until, drop)) = self.silence
            && self.now < until
        {
            if !drop {
                self.held.extend_from_slice(b);
            }
            return;
        }
        self.out.extend(b.iter().copied());
    }

    fn emit_line(&mut self, s: &str) {
        let mut v = s.as_bytes().to_vec();
        v.extend_from_slice(self.line_ending.bytes());
        self.emit_raw(&v);
    }

    fn rx_capacity(&self) -> usize {
        self.profile.rx_size.saturating_sub(1)
    }

    fn is_realtime(&self, b: u8) -> bool {
        match self.profile.protocol {
            Protocol::Grbl => matches!(b, b'?' | b'!' | b'~' | 0x18) || b >= 0x80,
            // Smoothie atiende `?` al llegar; Marlin no tiene tiempo real.
            Protocol::PingPong => self.profile.family == Family::Smoothie && b == b'?',
        }
    }

    /// Entrega a la controladora los bytes que envía el anfitrión. Los de tiempo real se
    /// atienden al llegar; el resto pasa por el búfer RX y se pierde (contado en
    /// [`rx_overflow_bytes`](Self::rx_overflow_bytes)) si no cabe.
    pub fn receive(&mut self, data: &[u8]) {
        if !self.connected {
            return;
        }
        for &b in data {
            if self.is_realtime(b) {
                self.pump();
                self.realtime(b);
                continue;
            }
            if self.profile.emergency_parser {
                self.emergency_scan(b);
            }
            if self.rx.len() >= self.rx_capacity() {
                // El intérprete va tan rápido como llegan los bytes salvo que esté bloqueado.
                self.pump();
            }
            if self.rx.len() >= self.rx_capacity() {
                self.overflow_bytes += 1;
                continue;
            }
            self.rx.push_back(b);
            self.in_flight += 1;
            self.max_in_flight = self.max_in_flight.max(self.in_flight);
            if b == b'\n' || b == b'\r' {
                self.pump();
            }
        }
        self.pump();
    }

    // -----------------------------------------------------------------------
    // Avance del tiempo
    // -----------------------------------------------------------------------

    fn homing_lock(&self) -> bool {
        self.setting(22) >= 1.0
    }

    fn quiescent(&self) -> bool {
        self.current.is_none()
            && self.planner.is_empty()
            && self.pending.is_none()
            && self.homing.is_none()
            && self.door_resume_at.is_none()
            && self.hold != Some(false)
            && self.door != Some(2)
            && !self.jog_cancel
            && self.v == 0.0
            && self.laser_sample().is_none()
    }

    fn next_event(&self) -> Option<Duration> {
        let mut t: Option<Duration> = None;
        let mut take = |x: Duration| t = Some(t.map_or(x, |c| c.min(x)));
        for f in &self.faults {
            if let Trigger::At(at) = f.trigger {
                take(at);
            }
        }
        if let Some((until, _)) = self.silence {
            take(until);
        }
        t
    }

    /// Hace avanzar la simulación hasta el instante `target` (nunca retrocede).
    pub fn advance_to(&mut self, target: Duration) {
        while self.now < target {
            if self.quiescent() {
                let horizon = self
                    .next_event()
                    .filter(|&e| e > self.now)
                    .map_or(target, |e| e.min(target));
                self.now = horizon;
                self.fire_due_faults();
                self.flush_silence();
                continue;
            }
            let step = (target - self.now).min(self.opts.step.max(Duration::from_micros(100)));
            self.now += step;
            self.tick(step.as_secs_f64());
        }
    }

    fn tick(&mut self, dt: f64) {
        self.fire_due_faults();
        self.flush_silence();
        self.tick_door_hold();
        self.tick_homing();
        let prev = self.pos;
        self.step_motion(dt);
        self.update_laser(prev, dt);
        if self.block_done {
            self.block_done = false;
            self.current = None;
            if self.planner.is_empty() {
                self.v = 0.0;
            }
        }
        self.pump();
    }

    fn flush_silence(&mut self) {
        if let Some((until, _)) = self.silence
            && self.now >= until
        {
            self.silence = None;
            let held = std::mem::take(&mut self.held);
            self.out.extend(held);
        }
    }

    fn tick_door_hold(&mut self) {
        if self.hold == Some(false) && self.v <= 1e-9 {
            self.hold = Some(true);
        }
        if self.door == Some(2) && self.v <= 1e-9 {
            self.door = Some(if self.door_open { 1 } else { 0 });
        }
        if let Some(t) = self.door_resume_at
            && self.now >= t
        {
            self.door_resume_at = None;
            self.door = None;
        }
    }

    fn tick_homing(&mut self) {
        let Some(h) = &self.homing else { return };
        let total = h.end.saturating_sub(h.start).as_secs_f64().max(1e-9);
        let f = (self.now.saturating_sub(h.start).as_secs_f64() / total).clamp(0.0, 1.0);
        let from = h.from;
        let done = self.now >= h.end;
        for (p, f0) in self.pos.iter_mut().zip(from) {
            *p = f0 * (1.0 - f);
        }
        self.v = self.setting(25).max(1.0) / 60.0;
        if done {
            self.homing = None;
            self.pos = [0.0; 3];
            self.v = 0.0;
            self.gc.position = self.pos;
            self.alarm = None;
            self.homed = true;
            self.position_lost = false;
            self.limit_lock = false;
        }
    }

    // -----------------------------------------------------------------------
    // Movimiento
    // -----------------------------------------------------------------------

    fn accel(&self) -> f64 {
        self.setting(120).min(self.setting(121)).max(10.0)
    }

    fn max_rate_mm_s(&self) -> f64 {
        let r = self.setting(110).min(self.setting(111));
        (if r > 0.0 { r } else { 6000.0 }) / 60.0
    }

    fn cruise_speed(&self, b: &Block) -> f64 {
        let max = self.max_rate_mm_s();
        let v = match b.kind {
            // Marlin: G0 y G1 comparten `F`.
            MoveKind::Rapid if b.feed > 0.0 => {
                (b.feed / 60.0).min(max) * f64::from(self.ovr_rapid) / 100.0
            }
            MoveKind::Rapid => max * f64::from(self.ovr_rapid) / 100.0,
            MoveKind::Jog => (b.feed / 60.0).min(max),
            _ => ((b.feed / 60.0) * f64::from(self.ovr_feed) / 100.0).min(max),
        };
        v.max(0.1)
    }

    fn junction_speed(&self, cur: &Block, next: &Block) -> f64 {
        let c = self.cruise_speed(cur).min(self.cruise_speed(next));
        let (a, b) = (cur.dir(), next.dir());
        let cos = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        if cos >= 0.999 {
            return c;
        }
        (c * cos.max(0.0).powi(8)).max(MIN_JUNCTION).min(c)
    }

    fn stopping(&self) -> bool {
        self.hold.is_some() || self.door.is_some() || self.jog_cancel || self.tool_pending
    }

    fn step_motion(&mut self, dt: f64) {
        if self.homing.is_some() {
            return;
        }
        if self.alarm.is_some() || self.check || self.sleeping {
            self.v = 0.0;
            return;
        }
        if self.current.is_none() {
            if self.stopping() {
                self.v = 0.0;
                if self.jog_cancel {
                    self.finish_jog_cancel();
                }
                return;
            }
            match self.planner.pop_front() {
                Some(b) => {
                    // Con el láser en un ventilador, lo manda `M106`/`M107`, no el bloque.
                    if self.profile.laser_fan.is_none() {
                        self.exec_spindle = (b.spindle, b.s);
                    }
                    self.block_seq += 1;
                    self.current = Some(Exec {
                        block: b,
                        done: 0.0,
                    });
                }
                None => {
                    self.v = 0.0;
                    return;
                }
            }
        }
        let stopping = self.stopping();
        let accel = self.accel();
        let (block, done) = {
            let e = self.current.as_ref().expect("bloque en curso");
            (e.block.clone(), e.done)
        };
        let cruise = self.cruise_speed(&block);
        let remaining = (block.len - done).max(0.0);
        let end_speed = if stopping {
            0.0
        } else {
            self.planner
                .front()
                .map_or(0.0, |n| self.junction_speed(&block, n))
        };
        let limit = (end_speed * end_speed + 2.0 * accel * remaining).sqrt();
        let target = if stopping { 0.0 } else { cruise.min(limit) };
        let dv = accel * dt;
        if self.v < target {
            self.v = (self.v + dv).min(target);
        } else {
            self.v = (self.v - dv).max(target);
        }
        let eff = if stopping {
            self.v
        } else {
            self.v.max(MIN_SPEED)
        };
        let new_done = done + eff * dt;
        if new_done >= block.len - 1e-9 {
            // El bloque se retira tras contabilizar el láser de este último paso.
            self.pos = block.to;
            self.block_done = true;
        } else {
            let f = new_done / block.len;
            for i in 0..3 {
                self.pos[i] = block.from[i] + (block.to[i] - block.from[i]) * f;
            }
            if let Some(e) = self.current.as_mut() {
                e.done = new_done;
            }
        }
        if self.jog_cancel && self.v <= 1e-9 {
            self.finish_jog_cancel();
        }
    }

    fn finish_jog_cancel(&mut self) {
        self.current = None;
        self.planner.clear();
        self.v = 0.0;
        self.jog_cancel = false;
        self.gc.position = self.pos;
        // Un jog bloqueado esperando sitio queda cancelado también.
        if self.pending.is_some() {
            self.pending = None;
            self.reply(Ok(()));
        }
    }

    pub(crate) fn laser_mode(&self) -> bool {
        match self.profile.family {
            // Con el láser en el ventilador, G0 no lo apaga: se comporta como `$32=0`.
            Family::Marlin if self.profile.laser_fan.is_some() => false,
            Family::Marlin | Family::Smoothie => true,
            _ => (self.setting(32) - 1.0).abs() < f64::EPSILON,
        }
    }

    /// ¿Puede quemar con la cabeza parada y el husillo en `M3`? Marlin en modo inline y
    /// Smoothie solo disparan durante un movimiento `G1`.
    fn fires_while_stationary(&self) -> bool {
        match self.profile.family {
            Family::Marlin => !self.inline,
            Family::Smoothie => false,
            _ => true,
        }
    }

    /// Potencia efectiva, tipo de movimiento y modo si el láser está encendido.
    fn laser_sample(&self) -> Option<(f64, MoveKind, SpindleMode, f64)> {
        if self.check
            || self.alarm.is_some()
            || self.sleeping
            || self.homing.is_some()
            || self.door.is_some()
            || self.halted
        {
            return None;
        }
        let (mode, s) = self.exec_spindle;
        if mode == SpindleMode::Off || s <= 0.0 || self.spindle_stop_ovr {
            return None;
        }
        let smax = self.setting(30);
        let mut eff = s * f64::from(self.ovr_spindle) / 100.0;
        if smax > 0.0 {
            eff = eff.min(smax);
        }
        if !self.laser_mode() {
            let kind = self
                .current
                .as_ref()
                .map_or(MoveKind::Stationary, |e| e.block.kind);
            return Some((eff, kind, mode, s));
        }
        match &self.current {
            Some(e) => match e.block.kind {
                MoveKind::Rapid | MoveKind::Jog | MoveKind::Stationary => None,
                MoveKind::Feed => match mode {
                    SpindleMode::Cw if self.hold.is_some() && self.profile.laser_off_in_hold => {
                        None
                    }
                    SpindleMode::Cw => Some((eff, MoveKind::Feed, mode, s)),
                    _ => {
                        if self.v <= 1e-9 {
                            return None;
                        }
                        let nominal = self.cruise_speed(&e.block);
                        let ratio = (self.v / nominal).clamp(0.0, 1.0);
                        Some((eff * ratio, MoveKind::Feed, mode, s))
                    }
                },
            },
            None => match mode {
                SpindleMode::Cw
                    if !(self.hold.is_some() && self.profile.laser_off_in_hold)
                        && self.fires_while_stationary() =>
                {
                    Some((eff, MoveKind::Stationary, mode, s))
                }
                _ => None,
            },
        }
    }

    fn update_laser(&mut self, prev: [f64; 3], dt: f64) {
        let sample = self.laser_sample();
        let Some((power, kind, mode, s)) = sample else {
            self.open_seg = None;
            return;
        };
        let in_block = self.current.is_some();
        let key = (self.block_seq, in_block);
        let extend = matches!(self.open_seg, Some((_, seq, ib)) if (seq, ib) == key);
        if extend && let Some((idx, _, _)) = self.open_seg {
            let (now, pos) = (self.now, self.pos);
            if let Some(seg) = self.trace.last_mut(idx) {
                seg.end = pos;
                seg.t1 = now;
                seg.energy += power * dt;
                seg.s_peak = seg.s_peak.max(power);
            }
            return;
        }
        let line = self.current.as_ref().map_or(0, |e| e.block.line);
        let idx = self.trace.push(LaserSegment {
            start: prev,
            end: self.pos,
            t0: self.now.saturating_sub(Duration::from_secs_f64(dt)),
            t1: self.now,
            kind,
            mode,
            s_programmed: s,
            s_peak: power,
            energy: power * dt,
            line,
        });
        self.open_seg = Some((idx, key.0, key.1));
    }

    fn quiet(&self) -> bool {
        self.planner.is_empty() && self.current.is_none() && self.v <= 1e-9
    }

    pub(crate) fn soft_limits_on(&self) -> bool {
        self.setting(20) >= 1.0
    }

    pub(crate) fn outside_travel(&self, p: [f64; 3]) -> bool {
        if !self.soft_limits_on() {
            return false;
        }
        (0..3).any(|i| {
            let t = self.setting(130 + i as u32);
            if t <= 0.0 {
                return false;
            }
            let positive = (self.setting(23) as u32 >> i) & 1 == 1;
            let (lo, hi) = if positive { (0.0, t) } else { (-t, 0.0) };
            p[i] < lo - 1e-6 || p[i] > hi + 1e-6
        })
    }

    // -----------------------------------------------------------------------
    // Bombeo de líneas
    // -----------------------------------------------------------------------

    /// Avanza lo que no depende del tiempo: acciones pendientes y líneas del RX.
    fn pump(&mut self) {
        loop {
            let a = self.run_pending();
            let b = self.process_rx();
            if !a && !b {
                break;
            }
        }
    }

    fn process_rx(&mut self) -> bool {
        let mut progressed = false;
        while self.pending.is_none() {
            let Some(b) = self.rx.pop_front() else { break };
            progressed = true;
            self.line_bytes += 1;
            if b == b'\n' || b == b'\r' {
                let raw = String::from_utf8_lossy(&self.line_buf).trim().to_string();
                let overflow = self.line_buf.len() >= MAX_RAW_LINE;
                self.line_buf.clear();
                let bytes = std::mem::take(&mut self.line_bytes);
                self.exec_line(raw, bytes, overflow);
            } else if self.line_buf.len() < MAX_RAW_LINE {
                self.line_buf.push(b);
            }
        }
        progressed
    }

    fn reply(&mut self, result: Result<(), u32>) {
        let ctx = self.line_ctx;
        self.in_flight = self.in_flight.saturating_sub(ctx.bytes);
        self.line_ctx.bytes = 0;
        match (ctx.reply, result) {
            (Reply::Drop, _) => {}
            (Reply::Error(c), _) => self.emit_line(&format!("error:{c}")),
            (Reply::Normal, Ok(())) => self.emit_line("ok"),
            (Reply::Normal, Err(c)) => self.emit_line(&format!("error:{c}")),
        }
    }

    fn exec_line(&mut self, raw: String, bytes: usize, overflow: bool) {
        self.line_ctx = LineCtx {
            bytes,
            reply: Reply::Normal,
        };
        if self.halted && self.profile.family == Family::Smoothie && clean_line(&raw) == "M999" {
            // Smoothie sale del paro con M999; Marlin tras `M112` solo con un reinicio.
            self.log.push(LogEntry {
                at: self.now,
                what: Received::Line(raw),
            });
            self.halted = false;
            self.alarm = None;
            self.reply(Ok(()));
            return;
        }
        if self.halted {
            // Detenida por una falla de Creality: no contesta líneas hasta el reset.
            self.log.push(LogEntry {
                at: self.now,
                what: Received::Line(raw),
            });
            return;
        }
        let text = clean_line(&raw);
        self.log.push(LogEntry {
            at: self.now,
            what: Received::Line(raw.clone()),
        });
        if text.is_empty() {
            self.last_error = None;
            self.reply(Ok(()));
            return;
        }
        self.lines_seen += 1;
        self.fire_line_faults();
        if let Some(r) = self.next_reply.take() {
            self.line_ctx.reply = r;
            if let Reply::Error(_) = r {
                self.reply(Ok(()));
                return;
            }
        }
        if self.halted {
            return;
        }
        if overflow || self.profile.max_line_len.is_some_and(|m| text.len() > m) {
            self.reply(Err(11));
            return;
        }
        if self.profile.protocol == Protocol::PingPong {
            self.exec_pingpong(&raw, &text);
        } else if text.starts_with('$') {
            self.last_error = None;
            self.exec_dollar(&raw, &text);
        } else if let Some(c) = self.last_error.filter(|_| self.profile.persistent_error) {
            self.reply(Err(c));
        } else {
            self.exec_gcode(&text);
        }
    }

    fn exec_gcode(&mut self, text: &str) {
        if self.alarm.is_some() || self.sleeping || self.state() == StateKind::Jog {
            self.reply(Err(9));
            return;
        }
        match self.parse_gcode(text) {
            Err(c) => {
                if self.profile.persistent_error {
                    self.last_error = Some(c);
                }
                self.reply(Err(c));
            }
            Ok(parsed) => {
                let before = self.wco();
                self.gc = parsed.gc;
                for e in parsed.effects {
                    match e {
                        Effect::SetWcs(i, v) => self.wcs[i] = v,
                        Effect::SetG92(v) => self.g92 = v,
                        Effect::StoreG28(v) => self.g28 = v,
                        Effect::StoreG30(v) => self.g30 = v,
                    }
                }
                if self.wco() != before {
                    // El desplazamiento cambió: el próximo reporte lo incluye.
                    self.rep.wco_counter = 0;
                }
                if self.check {
                    self.reply(Ok(()));
                    return;
                }
                self.pending = Some(Pending {
                    actions: parsed.actions.into(),
                    dwell_until: None,
                    awaiting_resume: false,
                    homing: false,
                });
            }
        }
    }

    fn run_pending(&mut self) -> bool {
        let Some(mut p) = self.pending.take() else {
            return false;
        };
        let mut progressed = false;
        loop {
            if p.homing {
                if self.homing.is_some() {
                    break;
                }
                self.reply(Ok(()));
                return true;
            }
            let Some(front) = p.actions.front() else {
                self.reply(Ok(()));
                return true;
            };
            match front {
                Action::Sync => {
                    if !self.quiet() {
                        break;
                    }
                }
                Action::ApplySpindle(m, s) => {
                    let (m, s) = (*m, *s);
                    self.exec_spindle = (m, s);
                }
                Action::Move(_) => {
                    if self.planner_used() >= self.profile.planner_blocks.max(1) {
                        break;
                    }
                    if let Some(Action::Move(b)) = p.actions.pop_front() {
                        self.planner.push_back(*b);
                    }
                    progressed = true;
                    continue;
                }
                Action::Dwell(secs) => {
                    let secs = *secs;
                    let until = *p
                        .dwell_until
                        .get_or_insert(self.now + Duration::from_secs_f64(secs.max(0.0)));
                    if self.now < until {
                        break;
                    }
                    p.dwell_until = None;
                }
                Action::ProgramEnd => self.emit_line("[MSG:Pgm End]"),
                Action::Pause => {
                    if !p.awaiting_resume {
                        p.awaiting_resume = true;
                        if self.hold.is_none() {
                            self.hold = Some(true);
                        }
                    }
                    break;
                }
                Action::ToolChange => {
                    if !p.awaiting_resume {
                        p.awaiting_resume = true;
                        self.tool_pending = true;
                    }
                    break;
                }
                Action::SoftLimitAlarm => {
                    self.reply(Ok(()));
                    self.raise_alarm(2);
                    return true;
                }
            }
            p.actions.pop_front();
            progressed = true;
        }
        self.pending = Some(p);
        progressed
    }

    // -----------------------------------------------------------------------
    // Comandos `$`
    // -----------------------------------------------------------------------

    fn reply_ok_with(&mut self, lines: &[String]) {
        for l in lines {
            self.emit_line(l);
        }
        self.reply(Ok(()));
    }

    fn exec_dollar(&mut self, raw: &str, text: &str) {
        let body = &text[1..];
        if body.is_empty() {
            let help = "[HLP:$$ $# $G $I $N $x=val $Nx=line $J=line $C $X $H ~ ! ? ctrl-x]";
            self.reply_ok_with(&[help.to_string()]);
            return;
        }
        if body.starts_with('J') {
            self.exec_jog(text);
            return;
        }
        if !matches!(
            self.state(),
            StateKind::Idle | StateKind::Alarm(_) | StateKind::Check
        ) {
            self.reply(Err(8));
            return;
        }
        match body {
            "$" => {
                let lines: Vec<String> = self
                    .settings
                    .values()
                    .map(|s| format!("${}={}", s.id, s.format()))
                    .collect();
                self.reply_ok_with(&lines);
            }
            "#" => {
                let lines = self.parameter_lines();
                self.reply_ok_with(&lines);
            }
            "G" => {
                let l = format!("[GC:{}]", self.parser_state());
                self.reply_ok_with(&[l]);
            }
            "I" => {
                let mut lines = self.profile.info.clone();
                if !self.user_info.is_empty() && !lines.is_empty() {
                    let first = lines[0].trim_end_matches(":]").to_string();
                    lines[0] = format!("{first}:{}]", self.user_info);
                }
                self.reply_ok_with(&lines);
            }
            "N" => {
                let lines = vec![
                    format!("$N0={}", self.startup[0]),
                    format!("$N1={}", self.startup[1]),
                ];
                self.reply_ok_with(&lines);
            }
            "C" => self.toggle_check(),
            "X" => {
                if self.alarm.is_some() && !self.halted {
                    self.alarm = None;
                    self.emit_line("[MSG:Caution: Unlocked]");
                }
                self.reply(Ok(()));
            }
            "H" => self.start_homing(),
            "SLP" => {
                self.stop_motion();
                self.sleeping = true;
                self.emit_line("[MSG:Sleeping]");
                self.reply(Ok(()));
            }
            _ if body.starts_with("RST=") => self.restore(&body[4..]),
            _ if body.starts_with("I=") => {
                // Cadena de usuario: se conserva tal como llegó.
                let v = raw.split_once('=').map_or("", |x| x.1).trim();
                if v.len() >= 80 {
                    self.reply(Err(14));
                } else {
                    self.user_info = v.to_string();
                    self.reply(Ok(()));
                }
            }
            _ if body.starts_with('N') && body.contains('=') => {
                let (k, v) = body.split_once('=').unwrap_or_default();
                match k {
                    "N0" | "N1" => {
                        let i = usize::from(k == "N1");
                        // Se prueba la línea antes de guardarla, como hace GRBL.
                        match self.parse_gcode(v) {
                            Ok(_) => {
                                self.startup[i] = v.to_string();
                                self.reply(Ok(()));
                            }
                            Err(c) => self.reply(Err(c)),
                        }
                    }
                    _ => self.reply(Err(3)),
                }
            }
            _ => match body.split_once('=') {
                Some((k, v)) if k.bytes().all(|b| b.is_ascii_digit()) && !k.is_empty() => {
                    let r = k
                        .parse::<u32>()
                        .map_err(|_| 3u32)
                        .and_then(|id| self.write_setting(id, v));
                    self.reply(r);
                }
                _ => self.reply(Err(3)),
            },
        }
    }

    fn write_setting(&mut self, id: u32, text: &str) -> Result<(), u32> {
        if !self.settings.contains_key(&id) {
            return Err(3);
        }
        let v: f64 = text.parse().map_err(|_| 2u32)?;
        if !v.is_finite() {
            return Err(2);
        }
        if v < 0.0 {
            return Err(4);
        }
        match id {
            0 if v < 3.0 => return Err(6),
            20 if v >= 1.0 && self.setting(22) < 1.0 => return Err(10),
            _ => {}
        }
        self.set_setting(id, v);
        Ok(())
    }

    fn restore(&mut self, what: &str) {
        match what {
            "$" | "#" | "*" => {
                if what != "#" {
                    self.settings = self
                        .profile
                        .settings
                        .iter()
                        .map(|s| (s.id, s.clone()))
                        .collect();
                }
                if what != "$" {
                    self.wcs = [[0.0; 3]; 6];
                    self.g28 = [0.0; 3];
                    self.g30 = [0.0; 3];
                }
                if what == "*" {
                    self.user_info.clear();
                    self.startup = [String::new(), String::new()];
                }
                self.emit_line("[MSG:Restoring defaults]");
                self.reply(Ok(()));
                self.do_reset(ResetKind::Quiet);
            }
            _ => self.reply(Err(3)),
        }
    }

    fn exec_jog(&mut self, text: &str) {
        if self.alarm.is_some() {
            self.reply(Err(9));
            return;
        }
        if !matches!(self.state(), StateKind::Idle | StateKind::Jog) {
            self.reply(Err(8));
            return;
        }
        let Some(rest) = text.strip_prefix("$J=") else {
            self.reply(Err(16));
            return;
        };
        match self.parse_jog(rest) {
            Err(c) => self.reply(Err(c)),
            Ok(b) => {
                self.gc.position = b.to;
                self.pending = Some(Pending {
                    actions: VecDeque::from([Action::Move(Box::new(b))]),
                    dwell_until: None,
                    awaiting_resume: false,
                    homing: false,
                });
            }
        }
    }

    fn toggle_check(&mut self) {
        if self.check {
            self.check = false;
            self.emit_line("[MSG:Disabled]");
            self.reply(Ok(()));
            self.do_reset(ResetKind::Quiet);
        } else {
            self.check = true;
            self.emit_line("[MSG:Enabled]");
            self.reply(Ok(()));
        }
    }

    fn start_homing(&mut self) {
        if self.setting(22) < 1.0 {
            self.reply(Err(5));
            return;
        }
        let end = self.now + self.profile.homing_duration;
        self.alarm = None;
        self.homing = Some(Homing {
            start: self.now,
            end,
            from: self.pos,
        });
        self.pending = Some(Pending {
            actions: VecDeque::new(),
            dwell_until: None,
            awaiting_resume: false,
            homing: true,
        });
    }

    fn parameter_lines(&self) -> Vec<String> {
        let f3 = |v: [f64; 3]| format!("{},{},{}", fmt3(v[0]), fmt3(v[1]), fmt3(v[2]));
        let mut lines = Vec::new();
        for (i, o) in self.wcs.iter().enumerate() {
            if i == 0 && self.nan.is_some() {
                lines.push("[G54:-nan,1.579,1.619]".to_string());
            } else {
                lines.push(format!("[G{}:{}]", 54 + i, f3(*o)));
            }
        }
        lines.push(format!("[G28:{}]", f3(self.g28)));
        lines.push(format!("[G30:{}]", f3(self.g30)));
        lines.push(format!("[G92:{}]", f3(self.g92)));
        lines.push("[TLO:0.000]".to_string());
        lines.push("[PRB:0.000,0.000,0.000:0]".to_string());
        lines
    }

    fn parser_state(&self) -> String {
        let g = &self.gc;
        let motion = match g.motion {
            MotionMode::Rapid => "G0",
            MotionMode::Linear => "G1",
            MotionMode::ArcCw => "G2",
            MotionMode::ArcCcw => "G3",
            MotionMode::None => "G80",
        };
        let spindle = match g.spindle {
            SpindleMode::Off => "M5",
            SpindleMode::Cw => "M3",
            SpindleMode::Ccw => "M4",
        };
        let coolant = match (g.mist, g.flood) {
            (false, false) => "M9".to_string(),
            (true, false) => "M7".to_string(),
            (false, true) => "M8".to_string(),
            (true, true) => "M7 M8".to_string(),
        };
        let feed = if g.feed == 0.0 {
            "0.0".to_string()
        } else {
            fmt_trim(g.feed)
        };
        format!(
            "{motion} G{} G{} G{} G{} G{} {spindle} {coolant} T{} F{feed} S{}",
            54 + g.wcs,
            g.plane,
            if g.inches { 20 } else { 21 },
            if g.absolute { 90 } else { 91 },
            if g.inverse_time { 93 } else { 94 },
            g.tool,
            fmt_trim(g.s),
        )
    }

    // -----------------------------------------------------------------------
    // Protocolo RepRap (Marlin, Smoothie)
    // -----------------------------------------------------------------------

    /// Velocidad (0-255) del ventilador `idx` según el último `M106`/`M107`.
    pub fn fan_speed(&self, idx: u32) -> f64 {
        self.fans.get(&idx).copied().unwrap_or(0.0)
    }

    /// Paro total (`M112`) en curso: no contesta líneas hasta reiniciar (Smoothie: o `M999`).
    pub fn is_halted(&self) -> bool {
        self.halted
    }

    /// Modo inline de Marlin activo (`M3 S0 I`).
    pub fn is_inline(&self) -> bool {
        self.inline
    }

    /// Último número de línea aceptado (líneas numeradas de Marlin).
    pub fn last_line_number(&self) -> i64 {
        self.last_line
    }

    /// Orden sin su numeración (`N5 G1 X1*12` -> `G1X1`), en mayúsculas y sin espacios.
    fn strip_numbering(cleaned: &str) -> &str {
        let body = cleaned.split('*').next().unwrap_or("");
        match body.strip_prefix('N') {
            Some(rest) => rest.trim_start_matches(|c: char| c.is_ascii_digit()),
            None => body,
        }
    }

    /// Analizador de emergencia de Marlin: reconoce `M410`, `M112` y `M108` en cuanto llega
    /// la línea completa, sin esperar a que el intérprete llegue a ella.
    fn emergency_scan(&mut self, b: u8) {
        if b == b'\n' || b == b'\r' {
            let line = String::from_utf8_lossy(&self.ep_buf).into_owned();
            self.ep_buf.clear();
            let cleaned = clean_line(&line);
            match Self::strip_numbering(&cleaned) {
                "M410" => self.quick_stop(),
                "M112" => self.kill(),
                "M108" => self.cycle_start(),
                _ => {}
            }
        } else if self.ep_buf.len() < MAX_RAW_LINE {
            self.ep_buf.push(b);
        }
    }

    /// `M410`: detiene los motores sin deceleración y descarta lo encolado (incluida la línea
    /// que esperaba sitio, que no recibe `ok`). **No apaga el láser**: el anfitrión manda `M5`.
    fn quick_stop(&mut self) {
        self.planner.clear();
        self.current = None;
        self.block_done = false;
        self.v = 0.0;
        self.hold = None;
        if self.pending.take().is_some() {
            self.in_flight = self.in_flight.saturating_sub(self.line_ctx.bytes);
            self.line_ctx.bytes = 0;
        }
        self.gc.position = self.pos;
        self.position_lost = true;
    }

    /// `M112`: paro total. Apaga el láser y deja de contestar hasta reiniciar (en Smoothie,
    /// hasta `M999`).
    fn kill(&mut self) {
        self.stop_motion();
        self.halted = true;
        self.alarm = Some(0);
        self.position_lost = true;
        self.line_ctx.bytes = 0;
        let msg = if self.profile.family == Family::Smoothie {
            // Inferido: Smoothie avisa con `!!` y pide reinicio o M999.
            "!! Kill button pressed - reset or M999 to continue"
        } else {
            "Error:Printer halted. kill() called!"
        };
        self.emit_line(msg);
    }

    /// Respuesta de Smoothie a `?`: `<Idle,MPos:x,y,z,WPos:x,y,z>` sin `ok` (formato por
    /// confirmar, ver [`Profile::smoothie`](crate::profile::Profile::smoothie)).
    fn report_smoothie(&mut self) {
        let name = match self.state() {
            StateKind::Alarm(_) => "Alarm",
            StateKind::Home => "Home",
            StateKind::Run | StateKind::Jog => "Run",
            StateKind::Hold(_) | StateKind::Door(_) => "Hold",
            _ => "Idle",
        };
        let (m, w) = (self.mpos(), self.wpos());
        let line = format!(
            "<{name},MPos:{:.4},{:.4},{:.4},WPos:{:.4},{:.4},{:.4}>",
            m[0], m[1], m[2], w[0], w[1], w[2]
        );
        self.emit_line(&line);
    }

    /// Líneas numeradas de Marlin. Devuelve la línea sin su numeración si es válida o no
    /// estaba numerada; si falla contesta con `Error:` + `Resend:` + `ok` y devuelve `None`.
    fn check_numbering(&mut self, raw: &str) -> Option<String> {
        let mut line = raw.trim().to_string();
        if std::mem::take(&mut self.corrupt_next) {
            // Ruido en el cable: se altera el último byte antes del `*` (o el último).
            let at = line.rfind('*').unwrap_or(line.len()).saturating_sub(1);
            if at < line.len() && line.is_char_boundary(at) {
                let c = if line.as_bytes()[at] == b'#' {
                    "$"
                } else {
                    "#"
                };
                line.replace_range(at..=at, c);
            }
        }
        let (body, sum) = match line.rsplit_once('*') {
            Some((b, c)) => (b, Some(c)),
            None => (line.as_str(), None),
        };
        let numbered = body
            .strip_prefix('N')
            .is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit()));
        if !numbered && sum.is_none() {
            return Some(line);
        }
        let last = self.last_line;
        let fail = |m: &mut Self, why: &str| {
            m.emit_line(&format!("Error:{why}, Last Line: {last}"));
            m.emit_line(&format!("Resend: {}", last + 1));
            m.reply(Ok(()));
        };
        let Some(sum) = sum else {
            fail(self, "No Checksum with line number");
            return None;
        };
        if !numbered {
            fail(self, "No Line Number with checksum");
            return None;
        }
        let expect = body.bytes().fold(0u8, |a, b| a ^ b);
        if sum.trim().parse::<u8>().ok() != Some(expect) {
            fail(self, "checksum mismatch");
            return None;
        }
        let digits: String = body[1..].chars().take_while(char::is_ascii_digit).collect();
        let n: i64 = digits.parse().unwrap_or(-1);
        let rest = body[1 + digits.len()..].trim();
        // `M110 N<n>` fija el contador y se acepta con cualquier número.
        if !rest.to_ascii_uppercase().starts_with("M110") && n != last + 1 {
            fail(self, "Line Number is not Last Line Number+1");
            return None;
        }
        self.last_line = n;
        Some(rest.to_string())
    }

    fn exec_pingpong(&mut self, raw: &str, text: &str) {
        let mut text = text.to_string();
        let mut raw = raw.to_string();
        if self.profile.family == Family::Marlin {
            match self.check_numbering(&raw) {
                Some(body) => {
                    text = clean_line(&body);
                    raw = body;
                }
                None => return,
            }
        }
        let raw = raw.as_str();
        let mut t = text;
        // `M3 S0 I` de Marlin: el modificador `I` no lleva valor.
        if t.ends_with('I') && (t.starts_with("M3") || t.starts_with("M4")) {
            t.pop();
            self.inline = true;
        }
        if t.starts_with("M5") {
            self.inline = false;
        }
        match t.as_str() {
            "M115" => {
                let info = self.profile.info.clone();
                self.reply_ok_with(&info);
            }
            "M114" => {
                let l = format!(
                    "X:{:.2} Y:{:.2} Z:{:.2} E:0.00 Count X: {} Y:{} Z:{}",
                    self.pos[0],
                    self.pos[1],
                    self.pos[2],
                    (self.pos[0] * 80.0).round() as i64,
                    (self.pos[1] * 80.0).round() as i64,
                    (self.pos[2] * 80.0).round() as i64,
                );
                self.reply_ok_with(&[l]);
            }
            "M400" => {
                self.pending = Some(Pending {
                    actions: VecDeque::from([Action::Sync]),
                    dwell_until: None,
                    awaiting_resume: false,
                    homing: false,
                });
            }
            // `ok` de lo que no hay que ejecutar (M110 fija el contador de líneas).
            _ if t.starts_with("M110") => {
                if let Some(n) = t.split_once('N').and_then(|(_, n)| n.parse::<i64>().ok()) {
                    self.last_line = n;
                }
                self.reply(Ok(()));
            }
            _ if t.starts_with("M105") || t.starts_with("M155") || t.starts_with("M999") => {
                self.reply(Ok(()));
            }
            "M410" => {
                // En la cola: llega aquí cuando el intérprete llega a ella.
                self.quick_stop();
                self.reply(Ok(()));
            }
            "M112" => self.kill(),
            // Sin LCD y sin `EMERGENCY_PARSER`, `M0` se ignora; con él, pausa hasta `M108`.
            "M108" => self.reply(Ok(())),
            _ if (t == "M0" || t.starts_with("M0P") || t.starts_with("M0S"))
                && !self.profile.emergency_parser =>
            {
                self.reply(Ok(()));
            }
            _ if t.starts_with("M106") || t.starts_with("M107") => self.exec_fan(&t),
            _ => match self.parse_gcode(&t) {
                Ok(mut parsed) => {
                    self.rapids_for_reprap(&mut parsed);
                    self.gc = parsed.gc;
                    for e in parsed.effects {
                        match e {
                            Effect::SetWcs(i, v) => self.wcs[i] = v,
                            Effect::SetG92(v) => self.g92 = v,
                            Effect::StoreG28(v) => self.g28 = v,
                            Effect::StoreG30(v) => self.g30 = v,
                        }
                    }
                    self.pending = Some(Pending {
                        actions: parsed.actions.into(),
                        dwell_until: None,
                        awaiting_resume: false,
                        homing: false,
                    });
                }
                Err(_) => {
                    self.emit_line(&format!("echo:Unknown command: \"{raw}\""));
                    self.reply(Ok(()));
                }
            },
        }
    }

    /// `M106 P<n> S<0-255>` y `M107 P<n>`. Con `laser_fan` el ventilador manda sobre el láser
    /// (con sincronización del planificador, como `LASER_SYNCHRONOUS_M106_M107`).
    fn exec_fan(&mut self, t: &str) {
        let Ok(words) = crate::parse::parse_words(&t[4..]) else {
            self.emit_line(&format!("echo:Unknown command: \"{t}\""));
            self.reply(Ok(()));
            return;
        };
        let get = |c: u8| words.iter().find(|w| w.0 == c).map(|w| w.1);
        let idx = get(b'P').unwrap_or(0.0).max(0.0) as u32;
        let speed = if t.starts_with("M107") {
            0.0
        } else {
            get(b'S').unwrap_or(255.0).clamp(0.0, 255.0)
        };
        self.fans.insert(idx, speed);
        if self.profile.laser_fan == Some(idx) {
            let mode = if speed > 0.0 {
                SpindleMode::Cw
            } else {
                SpindleMode::Off
            };
            self.pending = Some(Pending {
                actions: VecDeque::from([Action::Sync, Action::ApplySpindle(mode, speed)]),
                dwell_until: None,
                awaiting_resume: false,
                homing: false,
            });
        } else {
            self.reply(Ok(()));
        }
    }

    /// Particularidades de Marlin en los `G0`: comparten `F` con `G1` y, en modo estándar
    /// (`M3 S<n>` en línea aparte), apagan el láser hasta el siguiente `M3`.
    fn rapids_for_reprap(&self, parsed: &mut Parsed) {
        if self.profile.family != Family::Marlin {
            return;
        }
        let feed = parsed.gc.feed;
        let standard = self.profile.laser_fan.is_none() && !self.inline;
        let mut any = false;
        for a in &mut parsed.actions {
            if let Action::Move(b) = a
                && b.kind == MoveKind::Rapid
            {
                any = true;
                b.feed = feed;
                if standard {
                    b.spindle = SpindleMode::Off;
                }
            }
        }
        if any && standard {
            parsed.gc.spindle = SpindleMode::Off;
        }
    }

    // -----------------------------------------------------------------------
    // Tiempo real
    // -----------------------------------------------------------------------

    fn realtime(&mut self, b: u8) {
        self.log.push(LogEntry {
            at: self.now,
            what: Received::Realtime(b),
        });
        if self.profile.protocol == Protocol::PingPong {
            if b == b'?' {
                self.report_smoothie();
            }
            return;
        }
        let ext = self.profile.extended_realtime;
        let b = match b {
            0x80 if ext => b'?',
            0x81 if ext => b'~',
            0x82 if ext => b'!',
            x => x,
        };
        match b {
            b'?' => self.report_status(false),
            0x87 if ext => self.report_status(true),
            0x83 if ext => {
                let l = format!("[GC:{}]", self.parser_state());
                self.emit_line(&l);
            }
            b'!' => self.feed_hold(),
            b'~' => self.cycle_start(),
            0x18 => self.do_reset(ResetKind::FromHost),
            0x84 => self.safety_door(),
            0x85 => self.jog_cancel_cmd(),
            0x90 => self.ovr_feed = 100,
            0x91 => self.ovr_feed = (self.ovr_feed + 10).min(200),
            0x92 => self.ovr_feed = self.ovr_feed.saturating_sub(10).max(10),
            0x93 => self.ovr_feed = (self.ovr_feed + 1).min(200),
            0x94 => self.ovr_feed = self.ovr_feed.saturating_sub(1).max(10),
            0x95 => self.ovr_rapid = 100,
            0x96 => self.ovr_rapid = 50,
            0x97 => self.ovr_rapid = 25,
            0x99 => self.ovr_spindle = 100,
            0x9A => self.ovr_spindle = (self.ovr_spindle + 10).min(200),
            0x9B => self.ovr_spindle = self.ovr_spindle.saturating_sub(10).max(10),
            0x9C => self.ovr_spindle = (self.ovr_spindle + 1).min(200),
            0x9D => self.ovr_spindle = self.ovr_spindle.saturating_sub(1).max(10),
            0x9E => {
                if self.hold == Some(true) {
                    self.spindle_stop_ovr = !self.spindle_stop_ovr;
                }
            }
            0xA0 => {
                if matches!(
                    self.state(),
                    StateKind::Idle | StateKind::Run | StateKind::Hold(_)
                ) {
                    self.gc.flood = !self.gc.flood;
                }
            }
            0xA1 => {
                if self.profile.m7_enabled
                    && matches!(
                        self.state(),
                        StateKind::Idle | StateKind::Run | StateKind::Hold(_)
                    )
                {
                    self.gc.mist = !self.gc.mist;
                }
            }
            _ => {}
        }
        if (0x90..=0xA1).contains(&b) {
            self.rep.ov_counter = 0;
        }
    }

    fn feed_hold(&mut self) {
        match self.state() {
            StateKind::Idle | StateKind::Run => {
                if self.hold.is_none() {
                    self.hold = Some(self.v <= 1e-9);
                }
            }
            StateKind::Jog => self.jog_cancel_cmd(),
            _ => {}
        }
    }

    fn cycle_start(&mut self) {
        if self.door == Some(0) {
            self.door = Some(3);
            self.door_resume_at = Some(self.now + Duration::from_millis(100));
            return;
        }
        if self.door.is_some() {
            return;
        }
        if self.hold == Some(true) {
            self.hold = None;
            self.spindle_stop_ovr = false;
            self.resolve_awaiting();
        } else if self.tool_pending {
            self.tool_pending = false;
            self.resolve_awaiting();
        }
    }

    fn resolve_awaiting(&mut self) {
        if let Some(p) = self.pending.as_mut()
            && p.awaiting_resume
        {
            p.awaiting_resume = false;
            p.actions.pop_front();
        }
        self.tool_pending = false;
    }

    fn safety_door(&mut self) {
        if matches!(
            self.state(),
            StateKind::Idle
                | StateKind::Run
                | StateKind::Hold(_)
                | StateKind::Home
                | StateKind::Jog
        ) {
            self.door_open = true;
            self.door = Some(2);
            self.emit_line("[MSG:Check Door]");
        }
    }

    fn jog_cancel_cmd(&mut self) {
        if self.state() == StateKind::Jog {
            self.jog_cancel = true;
            self.planner.clear();
        }
    }

    fn report_status(&mut self, full: bool) {
        if self.limit_lock {
            return;
        }
        if self.homing.is_some() && !self.profile.status_during_home {
            return;
        }
        let mut counters = self.rep;
        let line = self.status_with(&mut counters, full, false);
        self.rep = counters;
        self.emit_line(&line);
    }

    fn status_with(&self, counters: &mut ReportState, full: bool, peek: bool) -> String {
        let st = self.state();
        let mask = self.setting(10) as u32;
        let hal = self.profile.hal_status_mask;
        let show_bf = mask & 2 != 0;
        let show_ln = hal && mask & 4 != 0;
        let show_fs = !hal || mask & 8 != 0;
        let show_pn = !hal || mask & 16 != 0;
        let show_wco = !hal || mask & 32 != 0;
        let show_ov = if hal {
            mask & 64 != 0
        } else {
            self.profile.overrides_in_status
        };
        let mpos = mask & 1 != 0;
        let inches = self.setting(13) >= 1.0;
        let (div, dec) = if inches { (25.4, 4) } else { (1.0, 3) };
        let wco = self.wco();
        let p = if mpos {
            self.pos
        } else {
            [
                self.pos[0] - wco[0],
                self.pos[1] - wco[1],
                self.pos[2] - wco[2],
            ]
        };
        let coords = |v: [f64; 3]| {
            format!(
                "{},{},{}",
                format_value(v[0] / div, dec),
                format_value(v[1] / div, dec),
                format_value(v[2] / div, dec)
            )
        };
        let mut s = String::from("<");
        s.push_str(st.name());
        match st {
            StateKind::Hold(n) | StateKind::Door(n) => s.push_str(&format!(":{n}")),
            StateKind::Alarm(c) if self.profile.alarm_substate && c > 0 => {
                s.push_str(&format!(":{c}"));
            }
            _ => {}
        }
        s.push_str(&format!(
            "|{}:{}",
            if mpos { "MPos" } else { "WPos" },
            coords(p)
        ));
        if show_bf {
            let planner_free = self
                .profile
                .planner_blocks
                .saturating_sub(self.planner_used());
            let rx_free = self.rx_capacity().saturating_sub(self.rx.len());
            s.push_str(&format!("|Bf:{planner_free},{rx_free}"));
        }
        if show_ln && let Some(n) = self.current.as_ref().and_then(|e| e.block.n) {
            s.push_str(&format!("|Ln:{n}"));
        }
        if show_fs {
            let feed = (self.v * 60.0).round() as i64;
            if self.profile.fs_three_values {
                s.push_str(&format!(
                    "|FS:{feed},{},{}",
                    self.exec_spindle.1.round() as i64,
                    self.current_power().round() as i64
                ));
            } else {
                s.push_str(&format!(
                    "|FS:{feed},{}",
                    self.current_power().round() as i64
                ));
            }
        }
        if show_pn {
            let mut pins = String::new();
            if self.profile.has_limit_switches && self.homed {
                if self.pos[0].abs() < 0.5 {
                    pins.push('X');
                }
                if self.pos[1].abs() < 0.5 {
                    pins.push('Y');
                }
            }
            if self.door_open {
                pins.push('D');
            }
            if !pins.is_empty() {
                s.push_str(&format!("|Pn:{pins}"));
            }
        }
        let busy = st.is_moving();
        if show_wco && (full || (!peek && counters.wco_counter == 0)) {
            let w = if self.nan == Some(true) {
                "-nan,0.000,0.000".to_string()
            } else {
                coords(wco)
            };
            s.push_str(&format!("|WCO:{w}"));
            if !peek {
                counters.wco_counter = if busy { 10 } else { 30 };
            }
        } else if !peek {
            counters.wco_counter = counters.wco_counter.saturating_sub(1);
        }
        if show_ov && (full || (!peek && counters.ov_counter == 0)) {
            s.push_str(&format!(
                "|Ov:{},{},{}",
                self.ovr_feed, self.ovr_rapid, self.ovr_spindle
            ));
            let mut acc = String::new();
            match self.exec_spindle.0 {
                SpindleMode::Cw if !self.spindle_stop_ovr => acc.push('S'),
                SpindleMode::Ccw if !self.spindle_stop_ovr => acc.push('C'),
                _ => {}
            }
            if self.gc.flood {
                acc.push('F');
            }
            if self.gc.mist {
                acc.push('M');
            }
            if !acc.is_empty() {
                s.push_str(&format!("|A:{acc}"));
            }
            if !peek {
                counters.ov_counter = if busy { 20 } else { 10 };
            }
        } else if !peek {
            counters.ov_counter = counters.ov_counter.saturating_sub(1);
        }
        if self.profile.extra_status_fields {
            s.push_str(&format!(
                "|WCS:G{}|H:{}",
                54 + self.gc.wcs,
                u8::from(self.homed)
            ));
        }
        s.push('>');
        s
    }

    // -----------------------------------------------------------------------
    // Alarmas y reinicios
    // -----------------------------------------------------------------------

    fn stop_motion(&mut self) {
        self.planner.clear();
        self.current = None;
        self.v = 0.0;
        self.hold = None;
        self.jog_cancel = false;
        self.homing = None;
        self.pending = None;
        self.exec_spindle = (SpindleMode::Off, self.exec_spindle.1);
        self.gc.position = self.pos;
    }

    fn raise_alarm(&mut self, code: u32) {
        self.stop_motion();
        self.alarm = Some(code);
        self.line_ctx.bytes = 0;
        self.emit_line(&format!("ALARM:{code}"));
        if matches!(code, 1 | 2) {
            self.emit_line("[MSG:Reset to continue]");
        }
        if code == 1 {
            self.limit_lock = true;
        }
    }

    fn do_reset(&mut self, kind: ResetKind) {
        let st = self.state();
        let homing = self.homing.is_some();
        let moving = match kind {
            ResetKind::FromHost => st.is_moving(),
            ResetKind::Motion => true,
            ResetKind::Quiet => false,
        };
        let was_sleeping = self.sleeping;
        self.rx.clear();
        self.line_buf.clear();
        self.line_bytes = 0;
        self.in_flight = 0;
        self.line_ctx.bytes = 0;
        self.pending = None;
        self.planner.clear();
        self.current = None;
        self.v = 0.0;
        self.hold = None;
        self.jog_cancel = false;
        self.homing = None;
        self.tool_pending = false;
        self.halted = false;
        self.limit_lock = false;
        self.last_error = None;
        self.next_reply = None;
        self.sleeping = false;
        self.check = false;
        self.exec_spindle = (SpindleMode::Off, 0.0);
        self.spindle_stop_ovr = false;
        self.ovr_feed = 100;
        self.ovr_rapid = 100;
        self.ovr_spindle = 100;
        self.g92 = [0.0; 3];
        self.gc = Gc::new(self.pos);
        self.door = self.door_open.then_some(1);
        self.door_resume_at = None;
        self.rep = ReportState::default();
        self.open_seg = None;
        self.resets += 1;
        self.fans.clear();
        self.inline = false;
        self.last_line = 0;
        self.corrupt_next = false;
        self.ep_buf.clear();
        let mut alarm = None;
        if homing && kind == ResetKind::FromHost {
            alarm = Some(6);
        } else if moving {
            alarm = Some(3);
        }
        if let Some(c) = alarm {
            self.position_lost = true;
            self.homed = false;
            self.emit_line(&format!("ALARM:{c}"));
        }
        let banner = self.profile.banner_reset.clone();
        for l in banner {
            self.emit_line(&l);
        }
        self.alarm = alarm.or_else(|| (self.homing_lock() || was_sleeping).then_some(0));
        if self.alarm.is_some() {
            if self.profile.unlock_message {
                self.emit_line("[MSG:'$H'|'$X' to unlock]");
            }
        } else {
            self.run_startup();
        }
    }

    fn run_startup(&mut self) {
        for i in 0..2 {
            let line = self.startup[i].clone();
            if line.is_empty() {
                continue;
            }
            match self.parse_gcode(&line) {
                Ok(parsed) => {
                    self.gc = parsed.gc;
                    for a in parsed.actions {
                        match a {
                            Action::Move(b) => self.planner.push_back(*b),
                            Action::ApplySpindle(m, s) => self.exec_spindle = (m, s),
                            _ => {}
                        }
                    }
                    self.emit_line(&format!(">{line}:ok"));
                }
                Err(c) => self.emit_line(&format!(">{line}:error:{c}")),
            }
        }
    }

    // -----------------------------------------------------------------------
    // Fallas
    // -----------------------------------------------------------------------

    /// Programa una falla. `Trigger::Now` se aplica en el acto.
    pub fn inject(&mut self, trigger: Trigger, fault: Fault) {
        if trigger == Trigger::Now {
            self.apply_fault(fault);
        } else {
            self.faults.push(Scheduled { trigger, fault });
        }
    }

    fn fire_due_faults(&mut self) {
        let now = self.now;
        let mut due = Vec::new();
        self.faults.retain(|f| match f.trigger {
            Trigger::At(t) if t <= now => {
                due.push(f.fault.clone());
                false
            }
            _ => true,
        });
        for f in due {
            self.apply_fault(f);
        }
    }

    fn fire_line_faults(&mut self) {
        let n = self.lines_seen;
        let mut due = Vec::new();
        self.faults.retain(|f| match f.trigger {
            Trigger::Line(l) if l == n => {
                due.push(f.fault.clone());
                false
            }
            _ => true,
        });
        for f in due {
            self.apply_fault(f);
        }
    }

    fn apply_fault(&mut self, fault: Fault) {
        match fault {
            Fault::Noise(bytes) => self.emit_raw(&bytes),
            Fault::ErrorReply(c) => self.next_reply = Some(Reply::Error(c)),
            Fault::DropReply => self.next_reply = Some(Reply::Drop),
            Fault::CrealityError { code, halt } => {
                self.emit_line(&format!("ERROR:{code:02}"));
                if halt {
                    self.stop_motion();
                    self.halted = true;
                    self.alarm = Some(0);
                    self.pending = None;
                    self.line_ctx.bytes = 0;
                }
            }
            Fault::Alarm(c) => self.raise_alarm(c),
            Fault::Alarm3WithBanner => self.do_reset(ResetKind::Motion),
            Fault::Reboot => self.do_reset(ResetKind::Quiet),
            Fault::Silence {
                duration,
                drop_output,
            } => self.silence = Some((self.now + duration, drop_output)),
            Fault::Disconnect => self.connected = false,
            Fault::NanOffsets { in_status } => self.nan = Some(in_status),
            Fault::LineEnding(le) => self.line_ending = le,
            Fault::CorruptLine => self.corrupt_next = true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_line_strips_and_uppercases() {
        assert_eq!(clean_line("g1 x10.5 (hola) y2 ; fin"), "G1X10.5Y2");
        assert_eq!(clean_line("$i=Texto"), "$I=TEXTO");
        assert_eq!(clean_line("   "), "");
    }

    #[test]
    fn fmt_helpers() {
        assert_eq!(fmt_trim(1000.0), "1000");
        assert_eq!(fmt_trim(0.5), "0.5");
        assert_eq!(fmt3(-0.0), "0.000");
    }
}
