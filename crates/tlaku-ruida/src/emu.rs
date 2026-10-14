//! Controladora Ruida simulada, solo para pruebas (feature `emu` o `cargo test`).
//!
//! Habla el protocolo del puerto 50200 como lo describen las fuentes (ruida.md sección 12):
//! verifica el checksum (suma de los bytes del cable), responde un byte de ACK (`CC` swizzleado),
//! de NAK (`CF`) o de ERR (`CD`), des-swizzlea, acumula el flujo, lo decodifica y mantiene un
//! estado mínimo de máquina (en reposo, trabajo en marcha, pausa, posición) que contesta a las
//! lecturas `DA 00` con `DA 01 dirección valor` en un segundo datagrama, después del ACK.
//!
//! Dos formas de usarlo:
//! - en memoria, con [`Emulator::handle`] o [`Emulator::into_link`] (un `MemoryLink`);
//! - por UDP en `127.0.0.1` con puertos efímeros, con [`UdpEmulator`].
//!
//! También modela la gestión de documentos y ajustes (ola 6E, ver `docs/hardware/ruida.md`
//! sección 16): escritura de memoria `DA 01 dirección v v` (guarda el primer valor y registra
//! todas las escrituras en [`Emulator::writes`]), borrar un documento (`E8 00 n(2) m(2)`),
//! ejecutar uno (`E8 03 n(2)`) y nombres repetidos (se guardan **duplicados**: no hay sobrescritura
//! implícita, así que quien quiera reemplazar debe borrar antes). Los formatos de `E8 00` y
//! `E8 03` son los de MeerK40t; las demás fuentes los describen distinto.
//!
//! No es la máquina real: la semántica de `D8 00` dentro de un archivo guardado con `E8 02`
//! (que no arranca), la de "la red se ignora con un trabajo en marcha" y la de las escrituras
//! de memoria son hipótesis de las fuentes, sin verificar en hardware.

use crate::cmd::{self, Command, DecodeError, Limit, ProcessOp, RapidRef};
use crate::link::MemoryLink;
use crate::num;
use crate::packet::{self, PacketConfig, Response};
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;
use tlaku_core::Point;
use tlaku_core::toolpath::{Move, Toolpath, ToolpathBlock};

/// Direcciones de memoria que contesta el emulador (ruida.md sección 4.4).
pub mod mem {
    pub const STATUS: u16 = 0x200;
    pub const CARD_ID: u16 = 0x2FE;
    pub const POS_X: u16 = 0x221;
    pub const POS_Y: u16 = 0x231;
    pub const BED_X: u16 = 0x026;
    pub const BED_Y: u16 = 0x036;
    pub const DOC_COUNT: u16 = 0x205;
    pub const BOARD_VERSION: u16 = 0x2FF;
}

/// Versión de placa que informa el emulador.
pub const BOARD_VERSION: &str = "TLAKU-EMU";

/// Bits del estado de la máquina (`0x200`), de ruida-pa y MeerK40t (MIT, RDC6442S).
pub mod status_bits {
    /// Trabajo en curso.
    pub const JOB_RUNNING: u64 = 0x0000_0001;
    /// Pausa (ruida-pa) o "fin de parte" (MeerK40t): las fuentes discrepan en el nombre.
    pub const PAUSED: u64 = 0x0000_0002;
    /// Cabezal en movimiento.
    pub const MOVING: u64 = 0x0100_0000;
}

/// Fallos que el emulador puede provocar a propósito. Los índices cuentan los datagramas
/// recibidos desde 0.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EmuFaults {
    /// No contesta nada al datagrama n (se pierde el paquete: no se aplica).
    pub lose_packet: Option<usize>,
    /// Aplica el datagrama n pero pierde la respuesta (ACK perdido).
    pub lose_ack: Option<usize>,
    /// Contesta NAK al datagrama n sin aplicarlo (aunque el checksum sea bueno).
    pub nak_packet: Option<usize>,
    /// Contesta ERR al datagrama n sin aplicarlo.
    pub err_packet: Option<usize>,
    /// Deja de contestar a partir del datagrama n (red ignorada).
    pub silent_from: Option<usize>,
    /// Mientras hay un trabajo en marcha no contesta a nada (observación de [FOR3]).
    pub ignore_network_while_running: bool,
    /// Contesta ACK a las escrituras de memoria (`DA 01`) pero no las aplica (firmware que las
    /// ignora o protege): sirve para probar la verificación posterior.
    pub ignore_writes: bool,
    /// Aplica a medias las escrituras: guarda el valor sumándole 1 (una corrupción a propósito).
    pub corrupt_writes: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct EmuConfig {
    pub packets: PacketConfig,
    /// Respuesta de `0x2FE`; las tarjetas repiten el ID de 16 bits en ambas mitades.
    pub card_id: u32,
    /// Tamaño de la cama en µm.
    pub bed_um: [i64; 2],
    pub faults: EmuFaults,
}

impl Default for EmuConfig {
    fn default() -> Self {
        EmuConfig {
            packets: PacketConfig::default(),
            // RDC6442S (ruida-pa).
            card_id: 0x6510_6510,
            bed_um: [600_000, 400_000],
            faults: EmuFaults::default(),
        }
    }
}

/// Tramo recorrido por el cabezal (µm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub from: (i64, i64),
    pub to: (i64, i64),
    pub laser_on: bool,
    pub layer: u8,
    pub speed_um_s: u64,
    /// Potencia efectiva del corte (crudo de 14 bits): la última `C7` si la hay y, si no, la
    /// máxima del láser 1 vigente.
    pub power: u16,
}

/// Documento guardado con `E8 02` + `E7 01 nombre` (no se ejecuta).
#[derive(Debug, Clone, PartialEq)]
pub struct StoredDoc {
    pub name: String,
    pub stream: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct Emulator {
    cfg: EmuConfig,
    stream: Vec<u8>,
    /// Paquetes aceptados y rechazados.
    pub accepted: usize,
    pub rejected: usize,
    /// Datagramas recibidos (con o sin fallo).
    pub received: usize,
    /// Trabajo en marcha (llegó `D8 00` fuera de una transferencia de archivo).
    pub running: bool,
    pub paused: bool,
    /// Veces que llegó `D8 01` / `D8 02` / `D8 03`.
    pub stops: usize,
    pub pauses: usize,
    pub resumes: usize,
    /// Llegó el `D7` del trabajo.
    complete: bool,
    /// Posición del cabezal (µm, sistema de la máquina).
    pub pos_um: (i64, i64),
    /// Documento en transferencia (`E8 02`): nombre una vez que llega `E7 01`.
    transfer: Option<Option<String>>,
    transfer_start: usize,
    pub stored: Vec<StoredDoc>,
    /// Saltos `D9` recibidos con la máquina libre.
    pub jogs: Vec<Command>,
    /// Comandos que no se pudieron decodificar.
    pub decode_errors: usize,
    /// Escrituras de memoria recibidas (`DA 01`): dirección y los dos valores.
    pub writes: Vec<(u16, u64, u64)>,
    /// Números (desde 1) de los documentos borrados con `E8 00`, en orden.
    pub deleted: Vec<usize>,
    /// Nombres de los documentos que se pidió ejecutar con `E8 03`.
    pub ran: Vec<String>,
    /// Flujo del documento en ejecución (`E8 03`), para posicionar el cabezal al terminar.
    run_stream: Option<Vec<u8>>,
    /// Valores de memoria que contesta el emulador además de los de estado y posición (ajustes y
    /// contadores). Se pueden cambiar desde las pruebas.
    pub mem: std::collections::BTreeMap<u16, u64>,
}

impl Emulator {
    pub fn new(cfg: EmuConfig) -> Self {
        // Valores de ejemplo de una RDC6442S (los del emulador de MeerK40t donde existen):
        // velocidades en µm/s, aceleración en µm/s², tiempos en s y recorrido en m.
        let mem = setting_defaults()
            .into_iter()
            .chain([
                (0x005, 200_000),
                (0x00C, 20_000),
                (0x023, 400_000),
                (0x033, 400_000),
                (0x025, 3_000_000),
                (0x035, 3_000_000),
                (0x201, 7_200),
                (0x202, 3_600),
                (0x203, 42),
                (0x211, 1_800),
                (0x223, 120),
                (0x233, 95),
                (0x208, 600),
                (0x010, 0),
                // Foco habilitado y modo de aire 1.
                (0x18F, 0x0201),
            ])
            .collect();
        Emulator {
            cfg,
            mem,
            ..Default::default()
        }
    }

    pub fn config(&self) -> &EmuConfig {
        &self.cfg
    }

    /// Cambia los fallos a propósito en caliente (para probar qué pasa a mitad de un trabajo).
    pub fn set_faults(&mut self, faults: EmuFaults) {
        self.cfg.faults = faults;
    }

    fn wire(&self, r: Response) -> Vec<u8> {
        vec![r.wire(self.cfg.packets.magic).expect("respuesta con byte")]
    }

    /// Procesa un datagrama y devuelve los datagramas de respuesta, en orden (ACK primero y
    /// después, si hay lecturas, los datagramas `DA 01`).
    pub fn handle(&mut self, datagram: &[u8]) -> Vec<Vec<u8>> {
        let n = self.received;
        self.received += 1;
        let f = self.cfg.faults;
        if f.silent_from.is_some_and(|k| n >= k)
            || f.lose_packet == Some(n)
            || (f.ignore_network_while_running && self.running)
        {
            return Vec::new();
        }
        if f.nak_packet == Some(n) {
            self.rejected += 1;
            return vec![self.wire(Response::Nak)];
        }
        if f.err_packet == Some(n) {
            self.rejected += 1;
            return vec![self.wire(Response::Err)];
        }
        let plain = match packet::parse_packet(datagram, &self.cfg.packets) {
            Ok(p) => p,
            Err(_) => {
                self.rejected += 1;
                return vec![self.wire(Response::Nak)];
            }
        };
        self.accepted += 1;
        self.stream.extend(&plain);
        let mut out = vec![self.wire(Response::Ack)];
        let replies = self.apply(&plain);
        if f.lose_ack == Some(n) {
            return Vec::new();
        }
        out.extend(replies);
        out
    }

    /// Procesa un datagrama y devuelve el primer byte de respuesta (ACK o NAK). Atajo para
    /// pruebas del particionado.
    pub fn receive(&mut self, datagram: &[u8]) -> u8 {
        self.handle(datagram)
            .first()
            .and_then(|d| d.first())
            .copied()
            .unwrap_or(0)
    }

    /// Un enlace en memoria cuyas respuestas calcula este emulador (compartido para inspeccionarlo
    /// después).
    pub fn into_link(self) -> (MemoryLink, Arc<Mutex<Emulator>>) {
        let shared = Arc::new(Mutex::new(self));
        let for_link = shared.clone();
        let link = MemoryLink::with_responder(move |d| {
            for_link.lock().unwrap_or_else(|e| e.into_inner()).handle(d)
        });
        (link, shared)
    }

    fn mem_value(&self, addr: u16) -> u64 {
        match addr {
            mem::CARD_ID => u64::from(self.cfg.card_id),
            mem::STATUS => {
                let mut s = 0;
                if self.running {
                    s |= status_bits::JOB_RUNNING;
                }
                if self.paused {
                    s |= status_bits::PAUSED;
                }
                s
            }
            mem::POS_X => wrap35(self.pos_um.0),
            mem::POS_Y => wrap35(self.pos_um.1),
            mem::BED_X => wrap35(self.cfg.bed_um[0]),
            mem::BED_Y => wrap35(self.cfg.bed_um[1]),
            mem::DOC_COUNT => self.stored.len() as u64,
            _ => self.mem.get(&addr).copied().unwrap_or(0),
        }
    }

    /// Aplica una escritura de memoria (salvo que `ignore_writes` la descarte).
    fn write_mem(&mut self, addr: u16, value: u64) {
        let f = self.cfg.faults;
        if f.ignore_writes {
            return;
        }
        let v = if f.corrupt_writes { value + 1 } else { value } & ((1u64 << 35) - 1);
        match addr {
            mem::BED_X => self.cfg.bed_um[0] = v as i64,
            mem::BED_Y => self.cfg.bed_um[1] = v as i64,
            _ => {
                self.mem.insert(addr, v);
            }
        }
    }

    fn apply(&mut self, plain: &[u8]) -> Vec<Vec<u8>> {
        let cmds = match cmd::decode_stream(plain) {
            Ok(c) => c,
            Err(_) => {
                self.decode_errors += 1;
                return Vec::new();
            }
        };
        let mut replies = Vec::new();
        for c in cmds {
            match c {
                // Escritura de memoria `DA 01 dirección v v` (ajustes; el pie de un `.rd` de
                // RDWorks usa la misma forma). Se guarda el primer valor.
                Command::MemWrite { addr, values } if values.len() == 2 => {
                    self.writes.push((addr, values[0], values[1]));
                    self.write_mem(addr, values[0]);
                }
                // La versión de la placa es una cadena (MeerK40t y ruida-pa): `DA 01 05 7F texto\0`.
                Command::MemRead {
                    addr: mem::BOARD_VERSION,
                } => {
                    let mut reply = vec![0xDA, 0x01, 0x05, 0x7F];
                    reply.extend(BOARD_VERSION.bytes());
                    reply.push(0);
                    replies.push(self.cfg.packets.magic.swizzle_all(&reply));
                }
                Command::MemRead { addr } => {
                    let v = Command::MemWrite {
                        addr,
                        values: vec![self.mem_value(addr)],
                    };
                    if let Ok(b) = v.to_bytes() {
                        replies.push(self.cfg.packets.magic.swizzle_all(&b));
                    }
                }
                // `E8 02`: lo que sigue es un documento que se guarda, no un trabajo.
                Command::Unknown { op: 0xE8, data } if data == [0x02] => {
                    self.transfer = Some(None);
                }
                // `E8 01 n`: nombre del documento n (1 en adelante). Contesta `E8 01 n nombre\0`
                // (formato del emulador de MeerK40t; sin verificar en una máquina real).
                Command::Unknown { op: 0xE8, data } if data.len() == 3 && data[0] == 0x01 => {
                    let n = (usize::from(data[1]) << 7) | usize::from(data[2]);
                    if let Some(doc) = n.checked_sub(1).and_then(|i| self.stored.get(i)) {
                        let mut reply = vec![0xE8, 0x01, data[1], data[2]];
                        reply.extend(doc.name.bytes());
                        reply.push(0);
                        replies.push(self.cfg.packets.magic.swizzle_all(&reply));
                    }
                }
                // `E8 00 n(2) m(2)`: borra el documento n (desde 1); 0 los borra todos (formato
                // del emulador de MeerK40t; Tlaku nunca envía el 0).
                Command::Unknown { op: 0xE8, data } if data.len() == 5 && data[0] == 0x00 => {
                    let n = (usize::from(data[1]) << 7) | usize::from(data[2]);
                    if n == 0 {
                        self.deleted.extend(1..=self.stored.len());
                        self.stored.clear();
                    } else if n <= self.stored.len() {
                        self.stored.remove(n - 1);
                        self.deleted.push(n);
                    }
                }
                // `E8 03 n(2)`: ejecuta el documento n (MeerK40t vuelca su contenido al
                // cabezal). Solo con la máquina libre.
                Command::Unknown { op: 0xE8, data } if data.len() == 3 && data[0] == 0x03 => {
                    let n = (usize::from(data[1]) << 7) | usize::from(data[2]);
                    if let Some(doc) = n.checked_sub(1).and_then(|i| self.stored.get(i))
                        && !self.running
                    {
                        self.ran.push(doc.name.clone());
                        self.run_stream = Some(doc.stream.clone());
                        self.running = true;
                        self.paused = false;
                        self.complete = false;
                    }
                }
                Command::FileName(name) if self.transfer.is_some() => {
                    // El documento guardado empieza después del nombre.
                    self.transfer = Some(Some(name));
                    self.transfer_start = self.stream.len();
                }
                Command::Process(ProcessOp::Start) => {
                    if self.transfer.is_none() {
                        self.running = true;
                        self.paused = false;
                        self.complete = false;
                    }
                }
                Command::Process(ProcessOp::Stop) => {
                    self.run_stream = None;
                    self.running = false;
                    self.paused = false;
                    self.stops += 1;
                }
                Command::Process(ProcessOp::Pause) => {
                    self.pauses += 1;
                    if self.running {
                        self.paused = true;
                    }
                }
                Command::Process(ProcessOp::Resume) => {
                    self.resumes += 1;
                    self.paused = false;
                }
                Command::Process(ProcessOp::HomeXY) if !self.running => self.pos_um = (0, 0),
                Command::RapidXY { rel, x, y } if !self.running => {
                    self.jog(rel, Some(x), Some(y));
                    self.jogs.push(c);
                }
                Command::RapidX { rel, v } if !self.running => {
                    self.jog(rel, Some(v), None);
                    self.jogs.push(c);
                }
                Command::RapidY { rel, v } if !self.running => {
                    self.jog(rel, None, Some(v));
                    self.jogs.push(c);
                }
                Command::EndOfFile => {
                    if let Some(name) = self.transfer.take() {
                        self.stored.push(StoredDoc {
                            name: name.unwrap_or_default(),
                            stream: self.stream[self.transfer_start..].to_vec(),
                        });
                    } else {
                        self.complete = true;
                    }
                }
                _ => {}
            }
        }
        replies
    }

    fn jog(&mut self, rel: RapidRef, x: Option<i64>, y: Option<i64>) {
        let (px, py) = self.pos_um;
        let nx = x.map_or(px, |v| if rel == RapidRef::Machine { v } else { px + v });
        let ny = y.map_or(py, |v| if rel == RapidRef::Machine { v } else { py + v });
        self.pos_um = (nx, ny);
    }

    /// Da por terminado el trabajo en marcha: el cabezal queda en el último punto y la máquina
    /// libre.
    pub fn finish_job(&mut self) {
        let segs = match self.run_stream.take() {
            Some(doc) => cmd::decode_stream(&doc).map(|c| replay(&c)),
            None => self.replay(),
        };
        if let Ok(segs) = segs
            && let Some(last) = segs.last()
        {
            self.pos_um = last.to;
        }
        self.running = false;
        self.paused = false;
    }

    /// Flujo en claro recibido hasta ahora (todo lo aceptado, incluidas consultas y controles).
    pub fn stream(&self) -> &[u8] {
        &self.stream
    }

    /// `true` si el flujo recibido termina en `D7` (fin de archivo).
    pub fn job_complete(&self) -> bool {
        self.stream.last() == Some(&0xD7)
    }

    /// `true` si llegó el `D7` de un trabajo en marcha (no de un documento guardado).
    pub fn run_complete(&self) -> bool {
        self.complete
    }

    pub fn commands(&self) -> Result<Vec<Command>, DecodeError> {
        cmd::decode_stream(&self.stream)
    }

    /// Direcciones de memoria consultadas con `DA 00`.
    pub fn read_requests(&self) -> Result<Vec<u16>, DecodeError> {
        Ok(self
            .commands()?
            .into_iter()
            .filter_map(|c| match c {
                Command::MemRead { addr } => Some(addr),
                _ => None,
            })
            .collect())
    }

    /// Recorre los comandos como lo haría el cabezal: posición (sumando relativos), capa,
    /// velocidad y potencia vigentes.
    pub fn replay(&self) -> Result<Vec<Segment>, DecodeError> {
        Ok(replay(&self.commands()?))
    }

    /// Reconstruye un `Toolpath` (un bloque por capa, en mm) para compararlo con el original.
    /// La velocidad sale de `C9 02` y la potencia de `C7` o, si no hay, de `C6 02` (láser 1).
    pub fn to_toolpath(&self) -> Result<Toolpath, DecodeError> {
        Ok(to_toolpath(&replay(&self.commands()?)))
    }
}

/// Valores de ejemplo de los ajustes de la lista cerrada (`crate::settings`) que no tienen ya uno.
/// Son inventados para la prueba (pasos de 6350, recorridos de 600 x 400 mm...), no los de una
/// máquina real.
fn setting_defaults() -> Vec<(u16, u64)> {
    crate::settings::WRITABLE
        .iter()
        .map(|d| {
            let v: i64 = match d.key {
                "g0_speed" | "x_max_speed" | "y_max_speed" => 400_000,
                "home_speed" => 20_000,
                "x_step" | "y_step" | "z_step" | "u_step" => 6_350,
                "x_start_speed" | "y_start_speed" => 8_000,
                "x_max_accel" | "y_max_accel" => 3_000_000,
                "x_travel" => 600_000,
                "y_travel" => 400_000,
                "x_button_speed" | "y_button_speed" => 10_000,
                "x_button_accel" | "y_button_accel" => 500_000,
                "x_estop_accel" | "y_estop_accel" => 5_000_000,
                "x_home_offset" | "y_home_offset" => 0,
                "rotary_pulse" => 5_000,
                "rotary_diameter" => 50_000,
                _ => 0,
            };
            (d.addr, wrap35(v))
        })
        .collect()
}

/// Valor de 35 bits en complemento a dos para devolver una posición con signo.
fn wrap35(v: i64) -> u64 {
    (v as u64) & ((1u64 << 35) - 1)
}

/// Emulador escuchando por UDP en `127.0.0.1` con un puerto efímero. Se detiene al soltarlo.
pub struct UdpEmulator {
    addr: SocketAddr,
    emu: Arc<Mutex<Emulator>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl UdpEmulator {
    pub fn spawn(cfg: EmuConfig) -> io::Result<Self> {
        Self::spawn_shared(Arc::new(Mutex::new(Emulator::new(cfg))))
    }

    /// Como [`UdpEmulator::spawn`] sobre un emulador ya creado y compartido.
    pub fn spawn_shared(emu: Arc<Mutex<Emulator>>) -> io::Result<Self> {
        let socket = UdpSocket::bind("127.0.0.1:0")?;
        socket.set_read_timeout(Some(Duration::from_millis(20)))?;
        let addr = socket.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let (e, s) = (emu.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while !s.load(Ordering::SeqCst) {
                let Ok((n, from)) = socket.recv_from(&mut buf) else {
                    continue;
                };
                let replies = e
                    .lock()
                    .unwrap_or_else(|x| x.into_inner())
                    .handle(&buf[..n]);
                for r in replies {
                    let _ = socket.send_to(&r, from);
                }
            }
        });
        Ok(UdpEmulator {
            addr,
            emu,
            stop,
            thread: Some(thread),
        })
    }

    /// Dirección y puerto en los que escucha (el puerto "50200" del emulador).
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Acceso al estado del emulador.
    pub fn emulator(&self) -> MutexGuard<'_, Emulator> {
        self.emu.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn shared(&self) -> Arc<Mutex<Emulator>> {
        self.emu.clone()
    }
}

impl Drop for UdpEmulator {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Ejecuta los comandos sobre un cabezal virtual que parte de (0, 0).
pub fn replay(cmds: &[Command]) -> Vec<Segment> {
    let mut out = Vec::new();
    let (mut x, mut y) = (0i64, 0i64);
    let (mut layer, mut speed, mut power) = (0u8, 0u64, 0u16);
    // Potencia inmediata (`C7`) vigente en la capa; manda sobre la máxima de la capa.
    let mut imm: Option<u16> = None;
    for c in cmds {
        let (nx, ny, on) = match *c {
            Command::MoveAbs { x, y } => (x, y, false),
            Command::CutAbs { x, y } => (x, y, true),
            Command::MoveRel { dx, dy } => (x + i64::from(dx), y + i64::from(dy), false),
            Command::CutRel { dx, dy } => (x + i64::from(dx), y + i64::from(dy), true),
            Command::MoveRelX(d) => (x + i64::from(d), y, false),
            Command::CutRelX(d) => (x + i64::from(d), y, true),
            Command::MoveRelY(d) => (x, y + i64::from(d), false),
            Command::CutRelY(d) => (x, y + i64::from(d), true),
            Command::LayerSelect(l) => {
                layer = l;
                imm = None;
                continue;
            }
            Command::Speed(v) => {
                speed = v;
                continue;
            }
            Command::ImmPower(raw) => {
                imm = Some(raw);
                continue;
            }
            Command::LaserPower {
                laser: 1,
                limit: Limit::Max,
                raw,
            } => {
                power = raw;
                continue;
            }
            _ => continue,
        };
        out.push(Segment {
            from: (x, y),
            to: (nx, ny),
            laser_on: on,
            layer,
            speed_um_s: speed,
            power: imm.unwrap_or(power),
        });
        (x, y) = (nx, ny);
    }
    out
}

fn to_toolpath(segments: &[Segment]) -> Toolpath {
    let mut blocks: Vec<(u8, ToolpathBlock)> = Vec::new();
    for s in segments {
        if blocks.last().map(|b| b.0) != Some(s.layer) {
            blocks.push((
                s.layer,
                ToolpathBlock {
                    speed_mm_min: num::speed_to_mm_min(s.speed_um_s),
                    ..Default::default()
                },
            ));
        }
        let to = Point::new(s.to.0 as f64 / 1000.0, s.to.1 as f64 / 1000.0);
        let mv = if s.laser_on {
            Move::Cut {
                to,
                power_pct: num::power_to_pct(s.power),
            }
        } else {
            Move::Travel { to }
        };
        blocks.last_mut().unwrap().1.moves.push(mv);
    }
    Toolpath {
        blocks: blocks.into_iter().map(|b| b.1).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::{build_packet, split_into_packets};
    use crate::rd::{self, HomeCorner, LayerOptions, RuidaJobOptions};
    use crate::swizzle::Magic;

    fn demo_toolpath() -> Toolpath {
        let mut blocks = Vec::new();
        for (i, (speed, power)) in [(6000.0, 40.0), (1800.0, 80.0)].into_iter().enumerate() {
            let o = 20.0 * i as f64;
            let c = |x: f64, y: f64| Move::Cut {
                to: Point::new(x + o, y + o),
                power_pct: power,
            };
            let mut moves = vec![Move::Travel {
                to: Point::new(10.0 + o, 10.0 + o),
            }];
            // Muchos segmentos para forzar varios paquetes.
            for k in 0..200 {
                let a = 1.0 + (k % 7) as f64 * 0.37;
                moves.push(c(10.0 + a, 10.0 + (k as f64) * 0.05));
                moves.push(c(30.0 - a, 10.0 + (k as f64) * 0.05 + 0.02));
            }
            blocks.push(ToolpathBlock {
                speed_mm_min: speed,
                air_assist: i == 0,
                moves,
                ..Default::default()
            });
        }
        Toolpath { blocks }
    }

    fn opts() -> RuidaJobOptions {
        RuidaJobOptions {
            layers: vec![LayerOptions::default(), LayerOptions::default()],
            home: HomeCorner::BottomLeft,
            ..Default::default()
        }
    }

    fn ack(emu: &Emulator) -> u8 {
        Response::Ack.wire(emu.cfg.packets.magic).unwrap()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 0.0006
    }

    #[test]
    fn full_job_through_packets_is_reconstructed() {
        let tp = demo_toolpath();
        let plain = rd::build_rd(&tp, &opts()).unwrap();
        let packets_cfg = PacketConfig {
            max_data_bytes: 300,
            magic: Magic(0x88),
        };
        let packets = split_into_packets(&plain, &packets_cfg).unwrap();
        assert!(packets.len() > 3, "debe haber varios paquetes");
        let mut emu = Emulator::new(EmuConfig {
            packets: packets_cfg,
            ..Default::default()
        });
        for p in &packets {
            assert_eq!(emu.receive(p), ack(&emu));
        }
        assert_eq!(emu.stream(), &plain[..]);
        assert!(emu.job_complete() && emu.run_complete() && emu.running);
        assert_eq!(emu.rejected, 0);
        // Reconstrucción geométrica: mismas posiciones en µm y misma cantidad de tramos.
        let back = emu.to_toolpath().unwrap();
        assert_eq!(back.blocks.len(), 2);
        for (orig, got) in tp.blocks.iter().zip(&back.blocks) {
            assert!((orig.speed_mm_min - got.speed_mm_min).abs() < 0.01);
            assert_eq!(orig.moves.len(), got.moves.len());
            for (a, b) in orig.moves.iter().zip(&got.moves) {
                assert!(close(a.to().x, b.to().x) && close(a.to().y, b.to().y));
                assert_eq!(matches!(a, Move::Cut { .. }), matches!(b, Move::Cut { .. }));
            }
        }
        let (Move::Cut { power_pct: p0, .. }, Move::Cut { power_pct: p1, .. }) =
            (back.blocks[0].moves[1], back.blocks[1].moves[1])
        else {
            panic!("se esperaban cortes");
        };
        assert!((p0 - 40.0).abs() < 0.01 && (p1 - 80.0).abs() < 0.01);
        emu.finish_job();
        assert!(!emu.running);
    }

    #[test]
    fn raster_job_with_overscan_is_reconstructed_with_per_run_power() {
        // Tres filas de 20 mm: overscan 2 mm a potencia 0, un degradado de potencia y overscan.
        let mut moves = Vec::new();
        for r in 0..3 {
            let y = 10.0 + r as f64 * 0.1;
            let (x0, x1) = if r % 2 == 0 { (8.0, 32.0) } else { (32.0, 8.0) };
            let dir = if x1 > x0 { 1.0 } else { -1.0 };
            moves.push(Move::Travel {
                to: Point::new(x0, y),
            });
            moves.push(Move::Cut {
                to: Point::new(x0 + 2.0 * dir, y),
                power_pct: 0.0,
            });
            for k in 0..20 {
                moves.push(Move::Cut {
                    to: Point::new(x0 + (3.0 + k as f64) * dir, y),
                    power_pct: 5.0 + 4.0 * k as f64,
                });
            }
            moves.push(Move::Cut {
                to: Point::new(x1, y),
                power_pct: 0.0,
            });
        }
        let tp = Toolpath {
            blocks: vec![ToolpathBlock {
                speed_mm_min: 12000.0,
                moves,
                ..Default::default()
            }],
        };
        let plain = rd::build_rd(&tp, &opts()).unwrap();
        let mut emu = Emulator::new(EmuConfig::default());
        for p in split_into_packets(&plain, &PacketConfig::default()).unwrap() {
            assert_eq!(emu.receive(&p), ack(&emu));
        }
        let back = emu.to_toolpath().unwrap();
        assert_eq!(back.blocks.len(), 1);
        let (a, b) = (&tp.blocks[0].moves, &back.blocks[0].moves);
        assert_eq!(a.len(), b.len());
        for (m, n) in a.iter().zip(b) {
            assert!(close(m.to().x, n.to().x) && close(m.to().y, n.to().y));
            match (m, n) {
                (Move::Cut { power_pct: p, .. }, Move::Cut { power_pct: q, .. }) => {
                    assert!((p - q).abs() < 0.01, "{p} vs {q}");
                }
                // El overscan a potencia 0 vuelve como salto con el láser apagado.
                (Move::Cut { power_pct: p, .. }, Move::Travel { .. }) => assert_eq!(*p, 0.0),
                (Move::Travel { .. }, Move::Travel { .. }) => {}
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn corrupted_packet_gets_nak_and_is_not_stored() {
        let plain = rd::build_rd(&demo_toolpath(), &opts()).unwrap();
        let cfg = PacketConfig {
            max_data_bytes: 300,
            ..Default::default()
        };
        let packets = split_into_packets(&plain, &cfg).unwrap();
        let mut emu = Emulator::new(EmuConfig {
            packets: cfg,
            ..Default::default()
        });
        assert_eq!(emu.receive(&packets[0]), 0xC6);
        let mut bad = packets[1].clone();
        bad[5] ^= 0x10;
        // NAK = CF swizzleado con 0x88 = 0x48.
        assert_eq!(emu.receive(&bad), 0x48);
        assert_eq!(emu.rejected, 1);
        // Reintento del mismo paquete: aceptado.
        assert_eq!(emu.receive(&packets[1]), 0xC6);
        // Datagrama demasiado grande: NAK.
        assert_eq!(emu.receive(&vec![0u8; 2000]), 0x48);
        assert_eq!(emu.accepted, 2);
        assert!(!emu.job_complete());
    }

    #[test]
    fn reads_are_answered_after_the_ack_in_a_second_datagram() {
        let mut emu = Emulator::new(EmuConfig::default());
        let m = emu.config().packets.magic;
        let q = build_packet(&[0xDA, 0x00, 0x05, 0x7E], &emu.config().packets);
        let out = emu.handle(&q);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0], [0xC6]);
        let reply = m.unswizzle_all(&out[1]);
        let cmds = cmd::decode_stream(&reply).unwrap();
        assert_eq!(
            cmds,
            [Command::MemWrite {
                addr: 0x2FE,
                values: vec![0x6510_6510]
            }]
        );
        assert_eq!(emu.read_requests().unwrap(), [0x2FE]);
    }

    #[test]
    fn faults_lose_packets_and_acks() {
        let mut emu = Emulator::new(EmuConfig {
            faults: EmuFaults {
                lose_packet: Some(0),
                lose_ack: Some(1),
                err_packet: Some(2),
                ..Default::default()
            },
            ..Default::default()
        });
        let p = build_packet(&[0xD8, 0x02], &emu.config().packets);
        assert!(emu.handle(&p).is_empty(), "paquete perdido");
        assert_eq!(emu.pauses, 0);
        assert!(emu.handle(&p).is_empty(), "ACK perdido");
        assert_eq!(emu.pauses, 1, "pero el paquete sí se aplicó");
        assert_eq!(emu.handle(&p), [vec![0x46]], "ERR");
        assert_eq!(emu.pauses, 1);
    }

    #[test]
    fn stored_documents_do_not_start_and_jogs_move_the_head() {
        let mut emu = Emulator::new(EmuConfig::default());
        let p = |bytes: &[u8]| build_packet(bytes, &emu_cfg());
        fn emu_cfg() -> PacketConfig {
            PacketConfig::default()
        }
        emu.handle(&p(&[0xE8, 0x02]));
        emu.handle(&p(&[0xE7, 0x01, b'A', b'B', 0]));
        emu.handle(&p(&[0xD8, 0x00]));
        assert!(!emu.running);
        emu.handle(&p(&[0xD7]));
        assert_eq!(emu.stored.len(), 1);
        assert_eq!(emu.stored[0].name, "AB");
        let jog = Command::RapidXY {
            rel: RapidRef::Machine,
            x: 5_000,
            y: 6_000,
        };
        emu.handle(&p(&jog.to_bytes().unwrap()));
        assert_eq!(emu.pos_um, (5_000, 6_000));
        // Con un trabajo en marcha los saltos se ignoran.
        emu.handle(&p(&[0xD8, 0x00]));
        assert!(emu.running);
        emu.handle(&p(&jog.to_bytes().unwrap()));
        assert_eq!(emu.jogs.len(), 1);
    }

    #[test]
    fn udp_emulator_answers_on_an_ephemeral_port_and_stops_on_drop() {
        let server = UdpEmulator::spawn(EmuConfig::default()).unwrap();
        assert_ne!(server.addr().port(), 0);
        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let q = build_packet(&[0xDA, 0x00, 0x02, 0x00], &PacketConfig::default());
        client.send_to(&q, server.addr()).unwrap();
        let mut buf = [0u8; 64];
        let (n, _) = client.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..n], [0xC6]);
        let (n, _) = client.recv_from(&mut buf).unwrap();
        assert_eq!(n, 9, "DA 01 + dirección(2) + valor(5)");
        let addr = server.addr();
        drop(server);
        // El hilo terminó: nadie contesta ya.
        client
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        client.send_to(&q, addr).unwrap();
        assert!(client.recv_from(&mut buf).is_err());
    }
}
