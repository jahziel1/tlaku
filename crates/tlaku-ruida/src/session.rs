//! Sesión con una controladora Ruida por paquetes (UDP 50200): sondeo, envío de un trabajo,
//! estado, control del proceso, saltos y encuadre.
//!
//! Protocolo (ruida.md sección 12, fuentes ruida-pa y MeerK40t, ambas MIT y probadas en una
//! RDC6442S):
//! - cada mensaje es un datagrama `checksum(2) + datos swizzleados` y la máquina responde con
//!   **un byte de ACK** por paquete; las lecturas (`DA 00`) añaden un segundo datagrama
//!   `DA 01 dirección valor(5)` sin checksum, también swizzleado;
//! - la máquina "habla solo cuando se le habla": no hay mensajes espontáneos;
//! - sondeo: `DA 00 05 7E` (ID de tarjeta). Solo lectura. Es lo que usa ruida-pa de ping;
//! - pausar `D8 02`, reanudar `D8 03`, detener `D8 01`, home `D8 2A`, saltos `D9` con las
//!   opciones `00`/`02` (láser apagado). Las cuatro de control las mandan MeerK40t y ruida-pa
//!   por el mismo canal incluso con un trabajo en marcha.
//!
//! **Seguridad.** Un trabajo en modo [`SendMode::Run`] empieza con `D8 00` y la máquina puede
//! arrancar antes de recibir el final; ninguna fuente documenta un "abortar" distinto de
//! `D8 01`. Si el envío falla a medias se intenta ese `D8 01` y se avisa al usuario de que
//! puede que no llegue (la máquina ignora la red con un trabajo en marcha, [FOR3]). El modo
//! [`SendMode::Store`] guarda el trabajo como documento (`E8 02` + `E7 01 nombre`, igual que la
//! captura de Nova 35) y no arranca nada hasta que el usuario lo inicia en el panel: es el
//! predeterminado de la línea de comandos.

use crate::cmd::{self, Command, ProcessOp, RapidRef};
use crate::link::PacketLink;
use crate::num;
use crate::packet::{self, PacketConfig, PacketError, Response};
use crate::rd::HomeCorner;
use crate::safety::{self, SafetyError, SafetyLimits};
use crate::swizzle::{Magic, probe_candidates};
use std::fmt;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tlaku_core::Point;

/// Direcciones de memoria de solo lectura que usa la sesión (ruida.md sección 4.4).
pub mod addr {
    pub const CARD_ID: u16 = 0x2FE;
    pub const STATUS: u16 = 0x200;
    pub const POS_X: u16 = 0x221;
    pub const POS_Y: u16 = 0x231;
    pub const BED_X: u16 = 0x026;
    pub const BED_Y: u16 = 0x036;
}

/// Bits de `0x200`: ruida-pa y MeerK40t coinciden en el movimiento (`0x01000000`) y el trabajo
/// (`0x1`); el bit `0x2` es "pausa" para ruida-pa y "fin de parte" para MeerK40t.
const BIT_RUNNING: u64 = 0x0000_0001;
const BIT_PAUSED: u64 = 0x0000_0002;
const BIT_MOVING: u64 = 0x0100_0000;

/// Modelos conocidos por su ID de tarjeta (ruida-pa).
pub fn model_name(card_id: u32) -> Option<&'static str> {
    match card_id {
        0x6510_6510 => Some("RDC6442S"),
        0x9010_9010 => Some("RDC8445S"),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Magic con el que empezar. Si la máquina contesta con otro, el sondeo lo detecta.
    pub magic: Magic,
    /// Probar los demás magics conocidos si la máquina no contesta al primero.
    pub probe_all_magics: bool,
    pub max_data_bytes: usize,
    /// Espera del ACK de un paquete. Las fuentes dan 0,25 a 1 s (MeerK40t, ruida-pa) y 4 s
    /// (EduTech, FabLab, versiones antiguas).
    pub ack_timeout: Duration,
    /// Espera de la respuesta de una lectura (tras el ACK).
    pub reply_timeout: Duration,
    /// Espera larga: movimientos que bloquean la comunicación (home). MeerK40t usa 40 s y
    /// ruida-pa 15 s.
    pub long_timeout: Duration,
    /// Reenvíos de un paquete tras un NAK (checksum rechazado: no se aplicó).
    pub nak_retries: u32,
    /// Reenvíos de un paquete de datos tras un timeout. 0 por defecto: sin números de
    /// secuencia no se distingue un paquete perdido de un ACK perdido y reenviar duplicaría
    /// comandos en el trabajo.
    pub timeout_retries: u32,
    /// Reintentos de una lectura (idempotente).
    pub query_retries: u32,
    /// Exigir la máquina en reposo antes de enviar, saltar o encuadrar.
    pub require_idle: bool,
    pub limits: SafetyLimits,
    pub home: HomeCorner,
    /// Velocidad de los saltos y del encuadre, mm/s.
    pub jog_speed_mm_s: f64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        SessionConfig {
            magic: Magic::default(),
            probe_all_magics: true,
            max_data_bytes: packet::DEFAULT_MAX_DATA_BYTES,
            ack_timeout: Duration::from_secs(1),
            reply_timeout: Duration::from_secs(1),
            long_timeout: Duration::from_secs(15),
            nak_retries: 2,
            timeout_retries: 0,
            query_retries: 2,
            require_idle: true,
            limits: SafetyLimits::default(),
            home: HomeCorner::default(),
            jog_speed_mm_s: 50.0,
        }
    }
}

#[derive(Debug)]
pub enum SessionError {
    Io(io::Error),
    /// No llegó la respuesta a tiempo.
    Timeout {
        what: &'static str,
    },
    /// La máquina rechazó el paquete (NAK repetido o ERR).
    Rejected {
        response: Response,
        packet: usize,
    },
    /// Respuesta que no corresponde a lo esperado (p. ej. un magic equivocado).
    Protocol(String),
    Safety(SafetyError),
    Packet(PacketError),
    /// La máquina no está en reposo.
    Busy(MachineStatus),
    /// El usuario canceló.
    Cancelled,
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionError::Io(e) => write!(f, "error de red: {e}"),
            SessionError::Timeout { what } => write!(f, "sin respuesta de la máquina ({what})"),
            SessionError::Rejected { response, packet } => write!(
                f,
                "la máquina rechazó el paquete {} ({})",
                packet + 1,
                match response {
                    Response::Nak => "checksum rechazado",
                    Response::Err => "error de mensaje",
                    _ => "respuesta inesperada",
                }
            ),
            SessionError::Protocol(m) => write!(f, "{m}"),
            SessionError::Safety(e) => write!(f, "{e}"),
            SessionError::Packet(e) => write!(f, "{e}"),
            SessionError::Busy(s) => {
                write!(f, "la máquina no está en reposo ({})", s.describe_state())
            }
            SessionError::Cancelled => write!(f, "cancelado por el usuario"),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<io::Error> for SessionError {
    fn from(e: io::Error) -> Self {
        SessionError::Io(e)
    }
}

impl From<SafetyError> for SessionError {
    fn from(e: SafetyError) -> Self {
        SessionError::Safety(e)
    }
}

impl From<PacketError> for SessionError {
    fn from(e: PacketError) -> Self {
        SessionError::Packet(e)
    }
}

/// Resultado del sondeo.
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeInfo {
    pub magic: Magic,
    /// El magic detectado no es el configurado.
    pub magic_changed: bool,
    /// ID de tarjeta de 32 bits (`0x65106510` en una RDC6442S).
    pub card_id: u32,
    pub model: Option<&'static str>,
    /// Tamaño de la cama que informa la máquina (mm), si contestó.
    pub bed_mm: Option<[f64; 2]>,
}

/// Estado leído de la máquina.
#[derive(Debug, Clone, PartialEq)]
pub struct MachineStatus {
    pub raw: u64,
    pub running: bool,
    /// Bit `0x2`: pausa o fin de parte según la fuente.
    pub paused: bool,
    pub moving: bool,
    /// Posición en el sistema de la máquina (µm desde el home).
    pub pos_um: (i64, i64),
    /// Posición en el sistema de Tlaku (mm, abajo-izquierda, Y arriba).
    pub pos_mm: Point,
}

impl MachineStatus {
    pub fn is_idle(&self) -> bool {
        !self.running && !self.moving
    }

    pub fn describe_state(&self) -> &'static str {
        match (self.running, self.paused, self.moving) {
            (true, true, _) => "trabajo en pausa",
            (true, false, _) => "trabajo en marcha",
            (false, _, true) => "moviéndose",
            _ => "en reposo",
        }
    }
}

/// Cómo enviar el trabajo.
#[derive(Debug, Clone, PartialEq)]
pub enum SendMode {
    /// Flujo directo: el trabajo empieza a ejecutarse al llegar su `D8 00` (como LightBurn).
    Run,
    /// Se guarda como documento con ese nombre y no arranca; se inicia desde el panel.
    Store { name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub packet: usize,
    pub packets: usize,
    pub bytes_sent: usize,
    pub bytes_total: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UploadReport {
    pub packets: usize,
    pub bytes: usize,
    /// NAK que se resolvieron reenviando.
    pub resends: usize,
    /// El trabajo ya está ejecutándose en la máquina (modo `Run`).
    pub running: bool,
}

/// Qué pasó con el intento de dejar la máquina en estado seguro tras un fallo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopOutcome {
    /// No hacía falta (modo `Store`: nada se ejecuta).
    NotNeeded,
    /// La máquina confirmó el `D8 01`.
    Acked,
    /// No hubo confirmación: puede seguir en marcha; usar el panel o el paro de emergencia.
    Unconfirmed,
}

/// El envío falló. `machine_may_be_running` es verdadero si pudo llegar el `D8 00` y no hay
/// confirmación de que la máquina esté parada.
#[derive(Debug)]
pub struct UploadError {
    pub cause: SessionError,
    pub packets_sent: usize,
    pub packets_total: usize,
    pub stop: StopOutcome,
    pub machine_may_be_running: bool,
}

impl fmt::Display for UploadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (paquetes confirmados: {} de {})",
            self.cause, self.packets_sent, self.packets_total
        )?;
        match self.stop {
            StopOutcome::NotNeeded => write!(
                f,
                ". El documento guardado queda incompleto: bórralo desde el panel de la máquina"
            ),
            StopOutcome::Acked => write!(f, ". Se mandó Detener (D8 01) y la máquina lo confirmó"),
            StopOutcome::Unconfirmed => write!(
                f,
                ". Se mandó Detener (D8 01) pero la máquina no lo confirmó: puede estar \
                 ejecutando el trabajo incompleto; pulsa Stop en su panel o usa el paro de emergencia"
            ),
        }
    }
}

impl std::error::Error for UploadError {}

pub struct RuidaSession<L: PacketLink> {
    link: L,
    cfg: SessionConfig,
    /// Cama informada por la máquina (µm), si ya se leyó.
    bed_um: Option<[i64; 2]>,
}

impl<L: PacketLink> RuidaSession<L> {
    pub fn new(link: L, cfg: SessionConfig) -> Self {
        RuidaSession {
            link,
            cfg,
            bed_um: None,
        }
    }

    pub fn config(&self) -> &SessionConfig {
        &self.cfg
    }

    pub fn magic(&self) -> Magic {
        self.cfg.magic
    }

    fn pcfg(&self) -> PacketConfig {
        PacketConfig {
            magic: self.cfg.magic,
            max_data_bytes: self.cfg.max_data_bytes,
        }
    }

    fn datagram(&self, cmds: &[Command]) -> Result<Vec<u8>, SessionError> {
        let plain = cmd::encode_stream(cmds)
            .map_err(|e| SessionError::Protocol(format!("comando inválido: {e}")))?;
        Ok(packet::build_packet(&plain, &self.pcfg()))
    }

    fn bed_mm(&self) -> [f64; 2] {
        match self.bed_um {
            Some([x, y]) => [x as f64 / 1000.0, y as f64 / 1000.0],
            None => self.cfg.limits.bed_mm,
        }
    }

    // ------------------------------------------------------------------------------------
    // Envío con ACK

    /// Espera el byte de respuesta de un paquete. Los datagramas más largos (respuestas de
    /// lectura) se guardan en `stash`.
    fn wait_response(
        &mut self,
        timeout: Duration,
        stash: &mut Vec<Vec<u8>>,
    ) -> Result<Option<Response>, SessionError> {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.link.recv(left)? {
                None => return Ok(None),
                Some(d) if d.len() == 1 => {
                    return Ok(Some(packet::interpret_response(d[0], self.cfg.magic)));
                }
                Some(d) if d.is_empty() => {}
                Some(d) => stash.push(d),
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
        }
    }

    /// Envía un datagrama y espera su ACK. `allow_err_retry`: reenviar una vez ante `ERR`
    /// (FabLab: solo el primer paquete de un trabajo).
    fn send_acked(
        &mut self,
        datagram: &[u8],
        index: usize,
        timeout: Duration,
        allow_err_retry: bool,
        stash: &mut Vec<Vec<u8>>,
        resends: &mut usize,
    ) -> Result<(), SessionError> {
        let (mut naks, mut errs, mut timeouts) = (0u32, 0u32, 0u32);
        loop {
            self.link.send(datagram)?;
            match self.wait_response(timeout, stash)? {
                Some(Response::Ack) => return Ok(()),
                Some(Response::Nak) => {
                    naks += 1;
                    if naks > self.cfg.nak_retries {
                        return Err(SessionError::Rejected {
                            response: Response::Nak,
                            packet: index,
                        });
                    }
                    *resends += 1;
                }
                Some(Response::Err) => {
                    errs += 1;
                    if !allow_err_retry || errs > 1 {
                        return Err(SessionError::Rejected {
                            response: Response::Err,
                            packet: index,
                        });
                    }
                    *resends += 1;
                }
                // Keep-alive: no es la respuesta a este paquete; se vuelve a esperar sin reenviar.
                Some(Response::Enq) => {
                    if let Some(Response::Ack) = self.wait_response(timeout, stash)? {
                        return Ok(());
                    }
                    return Err(SessionError::Timeout { what: "ACK" });
                }
                Some(Response::Unknown(b)) => {
                    return Err(SessionError::Protocol(format!(
                        "respuesta inesperada 0x{b:02X} al paquete {}; ¿el magic (0x{:02X}) es \
                         el de esta máquina?",
                        index + 1,
                        self.cfg.magic.0
                    )));
                }
                None => {
                    timeouts += 1;
                    if timeouts > self.cfg.timeout_retries {
                        return Err(SessionError::Timeout { what: "ACK" });
                    }
                    *resends += 1;
                }
            }
        }
    }

    /// Envía un mensaje corto (un solo paquete) y espera el ACK.
    fn command(&mut self, cmds: &[Command], timeout: Duration) -> Result<(), SessionError> {
        let d = self.datagram(cmds)?;
        self.link.drain()?;
        let (mut stash, mut resends) = (Vec::new(), 0);
        self.send_acked(&d, 0, timeout, false, &mut stash, &mut resends)
    }

    // ------------------------------------------------------------------------------------
    // Lecturas

    /// Lee una dirección de memoria (`DA 00`, solo lectura) y devuelve el valor de 35 bits.
    pub fn read_mem(&mut self, address: u16) -> Result<u64, SessionError> {
        let d = self.datagram(&[Command::MemRead { addr: address }])?;
        let mut last = SessionError::Timeout { what: "lectura" };
        for _ in 0..=self.cfg.query_retries {
            self.link.drain()?;
            self.link.send(&d)?;
            let deadline = Instant::now() + self.cfg.ack_timeout + self.cfg.reply_timeout;
            let mut nak = false;
            while !nak {
                let left = deadline.saturating_duration_since(Instant::now());
                let Some(dg) = self.link.recv(left)? else {
                    break;
                };
                if dg.len() == 1 {
                    match packet::interpret_response(dg[0], self.cfg.magic) {
                        Response::Nak | Response::Err => {
                            last = SessionError::Rejected {
                                response: packet::interpret_response(dg[0], self.cfg.magic),
                                packet: 0,
                            };
                            nak = true;
                        }
                        Response::Unknown(b) => {
                            return Err(SessionError::Protocol(format!(
                                "respuesta inesperada 0x{b:02X}; ¿el magic (0x{:02X}) es el de \
                                 esta máquina?",
                                self.cfg.magic.0
                            )));
                        }
                        Response::Ack | Response::Enq => {}
                    }
                    continue;
                }
                if let Some(v) = self.find_reply(&dg, address) {
                    return Ok(v);
                }
            }
            if !nak {
                last = SessionError::Timeout {
                    what: "respuesta de lectura",
                };
            }
        }
        Err(last)
    }

    /// Busca la respuesta `DA 01 dirección valor` de `address` en un datagrama de respuesta.
    fn find_reply(&self, datagram: &[u8], address: u16) -> Option<u64> {
        let plain = self.cfg.magic.unswizzle_all(datagram);
        let cmds = cmd::decode_stream(&plain).ok()?;
        cmds.into_iter().find_map(|c| match c {
            Command::MemWrite { addr, values } if addr == address && values.len() == 1 => {
                Some(values[0])
            }
            _ => None,
        })
    }

    /// Estado, movimiento y posición (cuatro lecturas de solo lectura).
    pub fn status(&mut self) -> Result<MachineStatus, SessionError> {
        let raw = self.read_mem(addr::STATUS)?;
        let x = signed35(self.read_mem(addr::POS_X)?);
        let y = signed35(self.read_mem(addr::POS_Y)?);
        let bed = self.bed_mm();
        let pos_mm = self
            .cfg
            .home
            .from_machine(x as f64 / 1000.0, y as f64 / 1000.0, bed);
        Ok(MachineStatus {
            raw,
            running: raw & BIT_RUNNING != 0,
            paused: raw & BIT_PAUSED != 0,
            moving: raw & BIT_MOVING != 0,
            pos_um: (x, y),
            pos_mm,
        })
    }

    /// Tamaño de la cama que informa la máquina (µm).
    pub fn read_bed_um(&mut self) -> Result<[i64; 2], SessionError> {
        let bx = signed35(self.read_mem(addr::BED_X)?);
        let by = signed35(self.read_mem(addr::BED_Y)?);
        self.bed_um = Some([bx, by]);
        Ok([bx, by])
    }

    // ------------------------------------------------------------------------------------
    // Sondeo

    /// Sondeo de solo lectura: `DA 00 05 7E`. Detecta el magic (el byte de ACK de la máquina va
    /// swizzleado con el suyo, así que un solo intercambio lo revela) y lee el ID de tarjeta y
    /// la cama. No envía nada que escriba ni mueva.
    ///
    /// El checksum no se sondea: la suma sobre los bytes del cable está confirmada por capturas
    /// reales (ruida.md sección 12).
    pub fn probe(&mut self) -> Result<ProbeInfo, SessionError> {
        let configured = self.cfg.magic;
        let mut candidates = vec![configured];
        if self.cfg.probe_all_magics {
            candidates.extend(probe_candidates().into_iter().filter(|m| *m != configured));
        }
        let mut last = SessionError::Timeout { what: "sondeo" };
        for (i, m) in candidates.into_iter().enumerate() {
            self.cfg.magic = m;
            // El primer magic con todos los reintentos; los demás con un solo intento corto
            // (una máquina que ignora el primero no suele contestar a los otros).
            let (attempts, wait) = if i == 0 {
                (
                    self.cfg.query_retries + 1,
                    self.cfg.ack_timeout + self.cfg.reply_timeout,
                )
            } else {
                (1, self.cfg.ack_timeout)
            };
            match self.probe_current(attempts, wait) {
                Ok(mut info) => {
                    info.magic_changed = info.magic != configured;
                    return Ok(info);
                }
                Err(e @ SessionError::Io(_)) => {
                    self.cfg.magic = configured;
                    return Err(e);
                }
                Err(e) => {
                    // Solo el primer intento informa de errores distintos de un timeout.
                    if i == 0 || matches!(e, SessionError::Timeout { .. }) {
                        last = e;
                    }
                }
            }
        }
        self.cfg.magic = configured;
        Err(last)
    }

    /// Un intercambio de sondeo con el magic actual; si el ACK delata otro magic, repite una
    /// vez con él.
    fn probe_current(&mut self, attempts: u32, wait: Duration) -> Result<ProbeInfo, SessionError> {
        let mut redirected = false;
        loop {
            let d = self.datagram(&[Command::MemRead {
                addr: addr::CARD_ID,
            }])?;
            let mut last = SessionError::Timeout { what: "sondeo" };
            let mut wire_ack: Option<u8> = None;
            for _ in 0..attempts {
                self.link.drain()?;
                self.link.send(&d)?;
                let deadline = Instant::now() + wait;
                loop {
                    let left = deadline.saturating_duration_since(Instant::now());
                    let Some(dg) = self.link.recv(left)? else {
                        break;
                    };
                    if dg.len() == 1 {
                        wire_ack = Some(dg[0]);
                        continue;
                    }
                    if let Some(v) = self.find_reply(&dg, addr::CARD_ID) {
                        let card_id = v as u32;
                        let model = model_name(card_id);
                        let bed_mm = self
                            .read_bed_um()
                            .ok()
                            .map(|[x, y]| [x as f64 / 1000.0, y as f64 / 1000.0]);
                        return Ok(ProbeInfo {
                            magic: self.cfg.magic,
                            magic_changed: false,
                            card_id,
                            model,
                            bed_mm,
                        });
                    }
                    last = SessionError::Protocol(
                        "la respuesta de la tarjeta no se entiende con este magic".into(),
                    );
                }
                // El ACK de otro magic delata a la máquina: no hace falta insistir con éste.
                if wire_ack.is_some_and(|b| Magic::from_wire_ack(b) != self.cfg.magic) {
                    break;
                }
            }
            // ¿Contestó con el ACK de otro magic?
            if let Some(b) = wire_ack {
                let theirs = Magic::from_wire_ack(b);
                if theirs != self.cfg.magic && !redirected {
                    redirected = true;
                    self.cfg.magic = theirs;
                    continue;
                }
                if packet::interpret_response(b, self.cfg.magic) == Response::Ack {
                    return Err(SessionError::Protocol(
                        "la máquina confirma el paquete pero no contesta a la lectura del ID de \
                         tarjeta"
                            .into(),
                    ));
                }
            }
            return Err(last);
        }
    }

    // ------------------------------------------------------------------------------------
    // Envío de un trabajo

    /// Envía un flujo `.rd` en claro (sin swizzle, como lo genera `rd::build_rd`).
    ///
    /// Comprueba la seguridad del flujo, que la máquina esté en reposo y que termine en `D7`;
    /// lo parte en paquetes en límite de comando y espera el ACK de cada uno. `cancel` se
    /// consulta entre paquetes. Ver la nota de seguridad del módulo sobre fallos a medias.
    pub fn send_job(
        &mut self,
        plain: &[u8],
        mode: &SendMode,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<UploadReport, Box<UploadError>> {
        let early = |cause: SessionError| {
            Box::new(UploadError {
                cause,
                packets_sent: 0,
                packets_total: 0,
                stop: StopOutcome::NotNeeded,
                machine_may_be_running: false,
            })
        };
        let cmds = cmd::decode_stream(plain).map_err(|e| {
            early(SessionError::Protocol(format!(
                "el trabajo no es válido: {e}"
            )))
        })?;
        if cmds.last() != Some(&Command::EndOfFile) {
            return Err(early(SessionError::Protocol(
                "el trabajo no termina en D7 (fin de archivo)".into(),
            )));
        }
        safety::check_commands(&cmds, &self.cfg.limits, false).map_err(|e| early(e.into()))?;
        if let SendMode::Store { name } = mode
            && (name.is_empty()
                || name.len() > 40
                || name.bytes().any(|b| !(0x20..0x7F).contains(&b)))
        {
            return Err(early(SessionError::Protocol(
                "el nombre del documento debe tener de 1 a 40 caracteres ASCII imprimibles".into(),
            )));
        }
        if self.cfg.require_idle {
            let st = self.status().map_err(early)?;
            if !st.is_idle() {
                return Err(early(SessionError::Busy(st)));
            }
        }

        let pcfg = self.pcfg();
        let mut datagrams = Vec::new();
        if let SendMode::Store { name } = mode {
            // Igual que la captura de Nova 35: `E8 02` y `E7 01 nombre` en paquetes propios.
            datagrams.push(packet::build_packet(&[0xE8, 0x02], &pcfg));
            let mut n = Vec::new();
            Command::FileName(name.clone())
                .encode(&mut n)
                .map_err(|e| early(SessionError::Protocol(e.to_string())))?;
            datagrams.push(packet::build_packet(&n, &pcfg));
        }
        let body = packet::split_into_packets(plain, &pcfg).map_err(|e| early(e.into()))?;
        datagrams.extend(body);
        let total = datagrams.len();
        let bytes_total: usize = datagrams.iter().map(Vec::len).sum();

        let mut resends = 0usize;
        let mut bytes_sent = 0usize;
        let mut stash = Vec::new();
        self.link.drain().map_err(|e| early(e.into()))?;
        for (i, d) in datagrams.iter().enumerate() {
            let result = if cancel.load(Ordering::SeqCst) {
                Err(SessionError::Cancelled)
            } else {
                // FabLab: ante ERR se reintenta solo el primer paquete.
                self.send_acked(d, i, self.cfg.ack_timeout, i == 0, &mut stash, &mut resends)
            };
            if let Err(cause) = result {
                return Err(self.fail_upload(cause, i, total, mode));
            }
            bytes_sent += d.len();
            progress(Progress {
                packet: i + 1,
                packets: total,
                bytes_sent,
                bytes_total,
            });
        }
        Ok(UploadReport {
            packets: total,
            bytes: bytes_total,
            resends,
            running: matches!(mode, SendMode::Run),
        })
    }

    /// Deja la máquina lo más segura posible tras un fallo a medias.
    fn fail_upload(
        &mut self,
        cause: SessionError,
        sent: usize,
        total: usize,
        mode: &SendMode,
    ) -> Box<UploadError> {
        let (stop, may_run) = match mode {
            // Nada se ejecuta: el documento incompleto no arranca solo.
            SendMode::Store { .. } => (StopOutcome::NotNeeded, false),
            SendMode::Run => {
                // Hasta el primer intento de enviar puede haber llegado el D8 00.
                let acked = self.try_stop();
                (
                    if acked {
                        StopOutcome::Acked
                    } else {
                        StopOutcome::Unconfirmed
                    },
                    !acked,
                )
            }
        };
        Box::new(UploadError {
            cause,
            packets_sent: sent,
            packets_total: total,
            stop,
            machine_may_be_running: may_run,
        })
    }

    /// `D8 01` con reintentos cortos; `true` si la máquina lo confirmó.
    fn try_stop(&mut self) -> bool {
        for _ in 0..3 {
            if self
                .command(&[Command::Process(ProcessOp::Stop)], self.cfg.ack_timeout)
                .is_ok()
            {
                return true;
            }
        }
        false
    }

    // ------------------------------------------------------------------------------------
    // Control del proceso

    /// Pausa el trabajo (`D8 02`).
    pub fn pause(&mut self) -> Result<(), SessionError> {
        self.command(&[Command::Process(ProcessOp::Pause)], self.cfg.ack_timeout)
    }

    /// Reanuda el trabajo (`D8 03`).
    pub fn resume(&mut self) -> Result<(), SessionError> {
        self.command(&[Command::Process(ProcessOp::Resume)], self.cfg.ack_timeout)
    }

    /// Detiene el trabajo (`D8 01`).
    pub fn stop(&mut self) -> Result<(), SessionError> {
        self.command(&[Command::Process(ProcessOp::Stop)], self.cfg.ack_timeout)
    }

    /// Lleva el cabezal al home (`D8 2A`) y espera a que termine.
    pub fn home(&mut self) -> Result<(), SessionError> {
        self.ensure_idle()?;
        self.command(
            &[Command::Process(ProcessOp::HomeXY)],
            self.cfg.long_timeout,
        )?;
        self.wait_idle(self.cfg.long_timeout)
    }

    fn ensure_idle(&mut self) -> Result<(), SessionError> {
        if self.cfg.require_idle {
            let st = self.status()?;
            if !st.is_idle() {
                return Err(SessionError::Busy(st));
            }
        }
        Ok(())
    }

    /// Espera hasta que la máquina esté en reposo (consulta el estado cada 100 ms).
    pub fn wait_idle(&mut self, timeout: Duration) -> Result<(), SessionError> {
        let deadline = Instant::now() + timeout;
        loop {
            if self.status()?.is_idle() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(SessionError::Timeout {
                    what: "fin del movimiento",
                });
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    // ------------------------------------------------------------------------------------
    // Saltos y encuadre (siempre con el láser apagado: opciones `D9` 00 y 02)

    /// Lleva el cabezal a un punto (mm, sistema de Tlaku) con el láser apagado.
    pub fn jog_to(&mut self, target: Point) -> Result<(), SessionError> {
        self.ensure_idle()?;
        self.jog_unchecked(target)
    }

    fn jog_unchecked(&mut self, target: Point) -> Result<(), SessionError> {
        let bed = self.bed_mm();
        let (mx, my) = self.cfg.home.to_machine(target, bed);
        let (x, y) = (
            num::mm_to_um(mx).map_err(num_err)?,
            num::mm_to_um(my).map_err(num_err)?,
        );
        let speed = num::speed_from_mm_min(self.cfg.jog_speed_mm_s * 60.0).map_err(num_err)?;
        let cmds = [
            Command::Speed(speed),
            Command::RapidXY {
                rel: RapidRef::Machine,
                x,
                y,
            },
        ];
        let mut limits = self.cfg.limits;
        limits.bed_mm = bed;
        limits.max_speed_mm_min = limits.max_speed_mm_min.max(self.cfg.jog_speed_mm_s * 60.0);
        safety::check_commands(&cmds, &limits, false)?;
        self.command(&cmds, self.cfg.ack_timeout)
    }

    /// Desplaza el cabezal `dx`, `dy` mm (ejes de Tlaku) desde donde está.
    pub fn jog_by(&mut self, dx: f64, dy: f64) -> Result<(), SessionError> {
        self.ensure_idle()?;
        let st = self.status()?;
        let target = Point::new(st.pos_mm.x + dx, st.pos_mm.y + dy);
        self.jog_unchecked(target)
    }

    /// Recorre las cuatro esquinas del rectángulo (mm) con el láser apagado, esperando a que el
    /// cabezal llegue a cada una. No hay un comando de encuadre documentado (`A5 53 00` del
    /// panel remoto no se conoce): se hace con saltos.
    pub fn frame(&mut self, min: Point, max: Point) -> Result<(), SessionError> {
        self.ensure_idle()?;
        let corners = [
            Point::new(min.x, min.y),
            Point::new(max.x, min.y),
            Point::new(max.x, max.y),
            Point::new(min.x, max.y),
            Point::new(min.x, min.y),
        ];
        for c in corners {
            self.jog_unchecked(c)?;
            self.wait_idle(self.cfg.long_timeout)?;
        }
        Ok(())
    }
}

fn num_err(e: num::NumError) -> SessionError {
    SessionError::Protocol(e.to_string())
}

/// Valor de 35 bits con signo.
fn signed35(v: u64) -> i64 {
    if v & (1 << 34) != 0 {
        v as i64 - (1 << 35)
    } else {
        v as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emu::{EmuConfig, EmuFaults, Emulator, UdpEmulator};
    use crate::link::{MemoryLink, UdpLink};
    use crate::rd::{self, RuidaJobOptions};
    use tlaku_core::toolpath::{Move, Toolpath, ToolpathBlock};

    fn fast_cfg() -> SessionConfig {
        SessionConfig {
            ack_timeout: Duration::from_millis(300),
            reply_timeout: Duration::from_millis(300),
            long_timeout: Duration::from_secs(2),
            home: HomeCorner::BottomLeft,
            ..Default::default()
        }
    }

    fn square_job() -> Vec<u8> {
        let c = |x: f64, y: f64| Move::Cut {
            to: Point::new(x, y),
            power_pct: 20.0,
        };
        let tp = Toolpath {
            blocks: vec![ToolpathBlock {
                speed_mm_min: 6000.0,
                moves: vec![
                    Move::Travel {
                        to: Point::new(10.0, 10.0),
                    },
                    c(30.0, 10.0),
                    c(30.0, 30.0),
                    c(10.0, 30.0),
                    c(10.0, 10.0),
                ],
                ..Default::default()
            }],
        };
        let o = RuidaJobOptions {
            home: HomeCorner::BottomLeft,
            ..Default::default()
        };
        rd::build_rd(&tp, &o).unwrap()
    }

    fn mem_session(
        emu_cfg: EmuConfig,
        cfg: SessionConfig,
    ) -> (
        RuidaSession<MemoryLink>,
        MemoryLink,
        std::sync::Arc<std::sync::Mutex<Emulator>>,
    ) {
        let (link, shared) = Emulator::new(emu_cfg).into_link();
        (RuidaSession::new(link.clone(), cfg), link, shared)
    }

    fn no_progress() -> impl FnMut(Progress) {
        |_| {}
    }

    #[test]
    fn probe_reads_card_id_and_bed_and_only_sends_reads() {
        let (mut s, link, emu) = mem_session(EmuConfig::default(), fast_cfg());
        let info = s.probe().unwrap();
        assert_eq!(info.card_id, 0x6510_6510);
        assert_eq!(info.model, Some("RDC6442S"));
        assert_eq!(info.bed_mm, Some([600.0, 400.0]));
        assert!(!info.magic_changed);
        // Solo lecturas: ningún D8, D9, DA 01...
        let e = emu.lock().unwrap();
        let reads = e.read_requests().unwrap();
        assert_eq!(reads, [0x2FE, 0x026, 0x036]);
        assert!(
            e.commands()
                .unwrap()
                .iter()
                .all(|c| matches!(c, Command::MemRead { .. }))
        );
        assert_eq!(link.sent().len(), 3);
    }

    #[test]
    fn probe_detects_a_different_magic_from_the_ack_byte() {
        for m in [0x11u8, 0x38, 0x77, 0xA1] {
            let cfg = EmuConfig {
                packets: PacketConfig {
                    magic: Magic(m),
                    ..Default::default()
                },
                ..Default::default()
            };
            let (mut s, _l, _e) = mem_session(cfg, fast_cfg());
            let info = s.probe().unwrap();
            assert_eq!(info.magic, Magic(m));
            assert!(info.magic_changed, "{m:#x}");
            assert_eq!(s.magic(), Magic(m));
            assert_eq!(info.card_id, 0x6510_6510);
        }
    }

    #[test]
    fn probe_times_out_when_nobody_answers() {
        let link = MemoryLink::new();
        let mut s = RuidaSession::new(
            link.clone(),
            SessionConfig {
                probe_all_magics: false,
                ..fast_cfg()
            },
        );
        assert!(matches!(s.probe(), Err(SessionError::Timeout { .. })));
        // 1 intento + 2 reintentos, siempre la misma consulta.
        assert_eq!(link.sent().len(), 3);
    }

    #[test]
    fn probe_without_answer_tries_the_other_magics_once_each() {
        let link = MemoryLink::new();
        let mut s = RuidaSession::new(link.clone(), fast_cfg());
        assert!(matches!(s.probe(), Err(SessionError::Timeout { .. })));
        // 3 intentos con el magic configurado y uno con cada uno de los demás conocidos.
        assert_eq!(link.sent().len(), 3 + probe_candidates().len() - 1);
        assert_eq!(
            s.magic(),
            Magic::default(),
            "el magic configurado no cambia"
        );
    }

    #[test]
    fn full_job_in_memory_with_progress_and_state() {
        let cfg = EmuConfig {
            packets: PacketConfig {
                max_data_bytes: 300,
                ..Default::default()
            },
            ..Default::default()
        };
        let (mut s, _l, emu) = mem_session(
            cfg,
            SessionConfig {
                max_data_bytes: 300,
                ..fast_cfg()
            },
        );
        s.probe().unwrap();
        let job = square_job();
        let mut seen = Vec::new();
        let report = s
            .send_job(&job, &SendMode::Run, &AtomicBool::new(false), &mut |p| {
                seen.push(p)
            })
            .unwrap();
        assert!(report.running && report.packets > 1 && report.resends == 0);
        assert_eq!(seen.len(), report.packets);
        assert_eq!(seen.last().unwrap().packet, report.packets);
        assert!(seen.windows(2).all(|w| w[0].bytes_sent < w[1].bytes_sent));
        {
            let e = emu.lock().unwrap();
            assert!(e.running && e.run_complete());
            assert_eq!(e.rejected, 0);
        }
        // Estado: en marcha, y no se puede enviar otro trabajo.
        let st = s.status().unwrap();
        assert!(st.running && !st.is_idle());
        assert!(matches!(
            s.send_job(&job, &SendMode::Run, &AtomicBool::new(false), &mut no_progress()),
            Err(e) if matches!(e.cause, SessionError::Busy(_))
        ));
        // Pausa, reanudar, detener.
        s.pause().unwrap();
        assert!(s.status().unwrap().paused);
        s.resume().unwrap();
        assert!(!s.status().unwrap().paused);
        s.stop().unwrap();
        let st = s.status().unwrap();
        assert!(st.is_idle() && !st.paused);
        let e = emu.lock().unwrap();
        assert_eq!((e.pauses, e.resumes, e.stops), (1, 1, 1));
    }

    #[test]
    fn store_mode_saves_a_document_and_does_not_start() {
        let (mut s, link, emu) = mem_session(EmuConfig::default(), fast_cfg());
        let job = square_job();
        let r = s
            .send_job(
                &job,
                &SendMode::Store {
                    name: "CUADRO".into(),
                },
                &AtomicBool::new(false),
                &mut no_progress(),
            )
            .unwrap();
        assert!(!r.running);
        let e = emu.lock().unwrap();
        assert!(!e.running, "un documento guardado no arranca");
        assert_eq!(e.stored.len(), 1);
        assert_eq!(e.stored[0].name, "CUADRO");
        assert_eq!(e.stored[0].stream, job);
        // Los dos primeros paquetes son E8 02 y E7 01 nombre.
        let sent = link.sent();
        let p = PacketConfig::default();
        let reads_before = 3; // estado: 0x200, 0x221, 0x231
        assert_eq!(
            packet::parse_packet(&sent[reads_before], &p).unwrap(),
            [0xE8, 0x02]
        );
        assert_eq!(
            cmd::decode_stream(&packet::parse_packet(&sent[reads_before + 1], &p).unwrap())
                .unwrap(),
            [Command::FileName("CUADRO".into())]
        );
    }

    #[test]
    fn nak_is_resent_and_a_lost_packet_aborts_with_a_safe_stop() {
        // NAK en el 2.º paquete de datos: se reenvía el mismo y el trabajo llega entero.
        let cfg = EmuConfig {
            faults: EmuFaults {
                nak_packet: Some(2),
                ..Default::default()
            },
            ..Default::default()
        };
        let sc = SessionConfig {
            max_data_bytes: 150,
            require_idle: false,
            ..fast_cfg()
        };
        let (mut s, _l, emu) = mem_session(cfg, sc.clone());
        let job = square_job();
        let r = s
            .send_job(
                &job,
                &SendMode::Run,
                &AtomicBool::new(false),
                &mut no_progress(),
            )
            .unwrap();
        assert_eq!(r.resends, 1);
        {
            let e = emu.lock().unwrap();
            assert_eq!(e.rejected, 1);
            assert!(e.running && e.run_complete());
        }
        // Un paquete perdido (sin respuesta): sin reintento por timeout, aborta y manda D8 01.
        let cfg = EmuConfig {
            faults: EmuFaults {
                lose_packet: Some(2),
                ..Default::default()
            },
            ..Default::default()
        };
        let (mut s, _l, emu) = mem_session(cfg, sc);
        let e = s
            .send_job(
                &job,
                &SendMode::Run,
                &AtomicBool::new(false),
                &mut no_progress(),
            )
            .unwrap_err();
        assert!(matches!(e.cause, SessionError::Timeout { .. }));
        assert_eq!(e.packets_sent, 2);
        assert_eq!(e.stop, StopOutcome::Acked);
        assert!(!e.machine_may_be_running);
        let st = emu.lock().unwrap();
        assert_eq!(st.stops, 1);
        assert!(!st.running, "tras el fallo la máquina queda detenida");
        assert!(!st.run_complete(), "el trabajo no llegó entero");
    }

    #[test]
    fn if_the_machine_ignores_the_network_the_user_is_warned() {
        // Tras el primer paquete (con D8 00) la máquina deja de contestar: el D8 01 no se confirma.
        let cfg = EmuConfig {
            faults: EmuFaults {
                silent_from: Some(2),
                ..Default::default()
            },
            ..Default::default()
        };
        let (mut s, _l, _e) = mem_session(
            cfg,
            SessionConfig {
                max_data_bytes: 200,
                require_idle: false,
                ..fast_cfg()
            },
        );
        let e = s
            .send_job(
                &square_job(),
                &SendMode::Run,
                &AtomicBool::new(false),
                &mut no_progress(),
            )
            .unwrap_err();
        assert_eq!(e.stop, StopOutcome::Unconfirmed);
        assert!(e.machine_may_be_running);
        let text = e.to_string();
        assert!(
            text.contains("pulsa Stop") && text.contains("paro de emergencia"),
            "{text}"
        );
    }

    #[test]
    fn store_mode_failure_does_not_send_stop_and_says_so() {
        let cfg = EmuConfig {
            faults: EmuFaults {
                lose_packet: Some(3),
                ..Default::default()
            },
            ..Default::default()
        };
        let (mut s, link, emu) = mem_session(
            cfg,
            SessionConfig {
                require_idle: false,
                max_data_bytes: 200,
                ..fast_cfg()
            },
        );
        let e = s
            .send_job(
                &square_job(),
                &SendMode::Store { name: "X".into() },
                &AtomicBool::new(false),
                &mut no_progress(),
            )
            .unwrap_err();
        assert_eq!(e.stop, StopOutcome::NotNeeded);
        assert!(!e.machine_may_be_running);
        assert!(e.to_string().contains("bórralo desde el panel"));
        assert_eq!(emu.lock().unwrap().stops, 0);
        assert_eq!(link.sent().len(), 4);
    }

    #[test]
    fn cancel_between_packets_stops_the_machine() {
        let (mut s, _l, emu) = mem_session(
            EmuConfig::default(),
            SessionConfig {
                max_data_bytes: 150,
                require_idle: false,
                ..fast_cfg()
            },
        );
        let cancel = AtomicBool::new(false);
        let mut n = 0;
        let e = s
            .send_job(&square_job(), &SendMode::Run, &cancel, &mut |_| {
                n += 1;
                if n == 2 {
                    cancel.store(true, Ordering::SeqCst);
                }
            })
            .unwrap_err();
        assert!(matches!(e.cause, SessionError::Cancelled));
        assert_eq!(e.stop, StopOutcome::Acked);
        assert_eq!(emu.lock().unwrap().stops, 1);
    }

    #[test]
    fn unsafe_or_invalid_jobs_are_refused_before_sending_anything() {
        let (mut s, link, _e) = mem_session(EmuConfig::default(), fast_cfg());
        // Sin D7.
        let mut job = square_job();
        job.pop();
        let e = s
            .send_job(
                &job,
                &SendMode::Run,
                &AtomicBool::new(false),
                &mut no_progress(),
            )
            .unwrap_err();
        assert!(matches!(e.cause, SessionError::Protocol(_)));
        // Fuera de la cama del perfil.
        let mut cfg = fast_cfg();
        cfg.limits.bed_mm = [20.0, 20.0];
        let (mut s2, link2, _e2) = mem_session(EmuConfig::default(), cfg);
        let e = s2
            .send_job(
                &square_job(),
                &SendMode::Run,
                &AtomicBool::new(false),
                &mut no_progress(),
            )
            .unwrap_err();
        assert!(matches!(e.cause, SessionError::Safety(_)));
        // Con escritura de memoria.
        let mut cmds = cmd::decode_stream(&square_job()).unwrap();
        let n = cmds.len();
        cmds.insert(
            n - 1,
            Command::MemWrite {
                addr: 0x320,
                values: vec![1, 1],
            },
        );
        let bytes = cmd::encode_stream(&cmds).unwrap();
        let e = s
            .send_job(
                &bytes,
                &SendMode::Run,
                &AtomicBool::new(false),
                &mut no_progress(),
            )
            .unwrap_err();
        assert!(matches!(
            e.cause,
            SessionError::Safety(SafetyError::ForbiddenCommand { .. })
        ));
        assert_eq!(link.sent().len(), 0);
        assert_eq!(link2.sent().len(), 0);
        // Nombre de documento inválido.
        let e = s
            .send_job(
                &square_job(),
                &SendMode::Store { name: "ñ".into() },
                &AtomicBool::new(false),
                &mut no_progress(),
            )
            .unwrap_err();
        assert!(matches!(e.cause, SessionError::Protocol(_)));
    }

    #[test]
    fn jog_and_frame_are_laser_off_and_checked() {
        let (mut s, _l, emu) = mem_session(EmuConfig::default(), fast_cfg());
        s.probe().unwrap();
        s.jog_to(Point::new(100.0, 50.0)).unwrap();
        assert_eq!(emu.lock().unwrap().pos_um, (100_000, 50_000));
        assert_eq!(s.status().unwrap().pos_mm, Point::new(100.0, 50.0));
        s.jog_by(5.0, -10.0).unwrap();
        assert_eq!(emu.lock().unwrap().pos_um, (105_000, 40_000));
        // Fuera de la cama: se rechaza sin enviar el salto.
        let before = emu.lock().unwrap().jogs.len();
        assert!(matches!(
            s.jog_to(Point::new(700.0, 10.0)),
            Err(SessionError::Safety(_))
        ));
        assert_eq!(emu.lock().unwrap().jogs.len(), before);
        s.frame(Point::new(10.0, 10.0), Point::new(60.0, 40.0))
            .unwrap();
        let e = emu.lock().unwrap();
        assert_eq!(e.pos_um, (10_000, 10_000));
        // Todos los saltos son D9 con las opciones 00 (los de láser encendido no existen).
        for j in &e.jogs {
            let b = j.to_bytes().unwrap();
            assert_eq!(b[0], 0xD9);
            assert_eq!(b[2] & 1, 0, "el bit del haz de luz nunca se activa");
        }
    }

    #[test]
    fn home_corner_maps_the_reported_position() {
        let cfg = SessionConfig {
            home: HomeCorner::TopRight,
            ..fast_cfg()
        };
        let (mut s, _l, emu) = mem_session(EmuConfig::default(), cfg);
        s.probe().unwrap();
        // Home arriba a la derecha: la máquina en (0,0) es la esquina (600, 400) de Tlaku.
        emu.lock().unwrap().pos_um = (0, 0);
        assert_eq!(s.status().unwrap().pos_mm, Point::new(600.0, 400.0));
        s.jog_to(Point::new(500.0, 300.0)).unwrap();
        assert_eq!(emu.lock().unwrap().pos_um, (100_000, 100_000));
        s.home().unwrap();
        assert_eq!(emu.lock().unwrap().pos_um, (0, 0));
    }

    #[test]
    fn a_wrong_magic_is_reported_clearly_not_misread() {
        let emu_cfg = EmuConfig {
            packets: PacketConfig {
                magic: Magic(0x11),
                ..Default::default()
            },
            ..Default::default()
        };
        let (mut s, _l, _e) = mem_session(
            emu_cfg,
            SessionConfig {
                require_idle: false,
                ..fast_cfg()
            },
        );
        // Sin sondear: el magic configurado (0x88) no es el de la máquina.
        let e = s
            .send_job(
                &square_job(),
                &SendMode::Run,
                &AtomicBool::new(false),
                &mut no_progress(),
            )
            .unwrap_err();
        assert!(
            matches!(e.cause, SessionError::Protocol(ref m) if m.contains("magic")),
            "{e}"
        );
    }

    /// Si el magic del sondeo no es el de la máquina, ésta interpreta los 4 bytes de la consulta
    /// como basura. Esta prueba comprueba que, para todo magic de la máquina (0..=255) y todo
    /// magic de los que sondeamos, esa basura nunca forma un comando completo que mueva, corte,
    /// inicie o controle (un comando incompleto se descarta con el siguiente byte de opcode).
    #[test]
    fn probing_with_the_wrong_magic_never_forms_a_dangerous_command() {
        let q = cmd::encode_stream(&[Command::MemRead {
            addr: addr::CARD_ID,
        }])
        .unwrap();
        let mut checked = 0;
        for sender in probe_candidates() {
            let wire = sender.swizzle_all(&q);
            for machine in 0..=255u8 {
                if Magic(machine) == sender {
                    continue;
                }
                let garbage = Magic(machine).unswizzle_all(&wire);
                let start = garbage
                    .iter()
                    .position(|&b| b >= 0x80)
                    .unwrap_or(garbage.len());
                for r in cmd::command_ranges(&garbage[start..]).unwrap() {
                    let Ok((c, _)) = cmd::decode_one(&garbage[start..][r.clone()]) else {
                        continue; // incompleto
                    };
                    let dangerous = matches!(
                        c,
                        Command::MoveAbs { .. }
                            | Command::CutAbs { .. }
                            | Command::MoveRel { .. }
                            | Command::CutRel { .. }
                            | Command::MoveRelX(_)
                            | Command::MoveRelY(_)
                            | Command::CutRelX(_)
                            | Command::CutRelY(_)
                            | Command::RapidX { .. }
                            | Command::RapidY { .. }
                            | Command::RapidXY { .. }
                            | Command::Process(_)
                    ) || matches!(&c, Command::Unknown { op: 0xA5, data }
                        if data.len() == 2 && (data[0] == 0x50 || data[0] == 0x51));
                    assert!(
                        !dangerous,
                        "sondeo {sender:?} contra máquina {machine:#x}: {garbage:02x?} -> {c}"
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked > 100, "la prueba debe examinar comandos completos");
    }

    /// Sesión completa contra el emulador UDP en localhost con puertos efímeros.
    #[test]
    fn full_session_over_udp_probe_send_status_pause_resume_stop() {
        let emu = UdpEmulator::spawn(EmuConfig::default()).unwrap();
        let link = UdpLink::to_controller_ports(emu.addr().ip(), emu.addr().port(), 0).unwrap();
        let mut s = RuidaSession::new(
            link,
            SessionConfig {
                max_data_bytes: 400,
                ..fast_cfg()
            },
        );
        let info = s.probe().unwrap();
        assert_eq!(info.model, Some("RDC6442S"));
        assert_eq!(info.bed_mm, Some([600.0, 400.0]));
        assert!(s.status().unwrap().is_idle());

        let job = square_job();
        let mut last = None;
        let r = s
            .send_job(&job, &SendMode::Run, &AtomicBool::new(false), &mut |p| {
                last = Some(p)
            })
            .unwrap();
        assert!(r.packets >= 2);
        assert_eq!(last.unwrap().packet, r.packets);
        {
            let e = emu.emulator();
            assert!(e.running && e.run_complete());
            assert_eq!(e.rejected, 0);
            // Lo que llegó es el trabajo íntegro (más las lecturas de antes).
            let tail = &e.stream()[e.stream().len() - job.len()..];
            assert_eq!(tail, &job[..]);
            let back = e.to_toolpath().unwrap();
            assert_eq!(back.blocks.len(), 1);
            assert_eq!(back.blocks[0].moves.len(), 5);
        }
        let st = s.status().unwrap();
        assert!(st.running && !st.paused);
        s.pause().unwrap();
        assert!(s.status().unwrap().paused);
        s.resume().unwrap();
        assert!(!s.status().unwrap().paused);
        s.stop().unwrap();
        assert!(s.status().unwrap().is_idle());
        let e = emu.emulator();
        assert_eq!((e.pauses, e.resumes, e.stops), (1, 1, 1));
    }

    #[test]
    fn udp_session_finds_the_magic_and_survives_a_lost_ack_in_a_read() {
        let emu = UdpEmulator::spawn(EmuConfig {
            packets: PacketConfig {
                magic: Magic(0x33),
                ..Default::default()
            },
            // La lectura 0 (primer sondeo) se pierde del todo.
            faults: EmuFaults {
                lose_packet: Some(0),
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        let link = UdpLink::to_controller_ports(emu.addr().ip(), emu.addr().port(), 0).unwrap();
        let mut s = RuidaSession::new(link, fast_cfg());
        let info = s.probe().unwrap();
        assert_eq!(info.magic, Magic(0x33));
        assert!(info.magic_changed);
        assert!(s.status().unwrap().is_idle());
    }
}
