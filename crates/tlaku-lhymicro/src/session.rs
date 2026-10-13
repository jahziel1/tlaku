//! Sesión con una placa M2 Nano: sondeo, envío de un trabajo con progreso, pausa, reanudación,
//! detención, saltos y home.
//!
//! # Protocolo (fuentes: MeerK40t `lihuiyu/controller.py`, MIT; ver [`crate::status`])
//!
//! - Antes de cada paquete se consulta el estado hasta que la placa lo acepta (206, o 236 si ya
//!   terminó); se escribe el paquete (34 bytes con CRC) y se consulta hasta que la placa lo
//!   confirma. 207 es CRC malo: se reenvía el mismo paquete. 238 es "ocupada": se espera.
//! - Un `-` en el flujo obliga a esperar el estado 236 ("terminó") antes de seguir.
//! - La pausa es `PN` en tiempo real (la misma orden pausa y reanuda; inferido) y la detención es
//!   `I` seguido de `IUS1P` (cancelar y apagar el láser).
//! - La placa **no informa de su posición**: la sesión la lleva por conteo desde el home. Sin
//!   home (o tras detener o fallar un trabajo) la posición es desconocida y no se envía ningún
//!   movimiento hasta volver a hacer home o confirmar con [`LhySession::assume_at_home`].
//!
//! # Seguridad
//!
//! - **Ningún paquete sin confirmación de estado listo**: se espera a 206/236 antes de escribir.
//! - Ante un error de USB, un CRC repetido, un tiempo agotado, la falta de energía (239) o un
//!   estado inutilizable, el trabajo se aborta y se intenta apagar el láser y detener la placa
//!   (`I`, `IUS1P`). El resultado ([`StopOutcome`]) dice si la placa lo confirmó; si no, hay que
//!   usar el paro de emergencia o abrir la tapa.
//! - Los límites de la cama y de velocidad se comprueban al codificar ([`crate::encode`]) y en
//!   los saltos.
//!
//! # Uso desde otro hilo
//!
//! [`LhySession::send_job`] bloquea mientras dura el trabajo (la placa recibe los datos a
//! medida que los ejecuta). Para pausar, reanudar o detener hay que usar el [`JobControl`] que
//! da [`LhySession::control`], que se puede clonar y mover a otro hilo:
//!
//! ```ignore
//! let control = session.control();
//! std::thread::spawn(move || session.send_job(&program, &mut |p| println!("{}/{}", p.packet, p.packets)));
//! control.request_pause();
//! control.request_resume();
//! control.request_stop();
//! ```

use crate::encode::{
    EncodeConfig, EncodeError, HomeCorner, Program, RAPID_ESTIMATE_MM_S, Warning, encode,
    mil_to_mm, mm_to_mil, rapid_text,
};
use crate::packet::{
    Frame, FrameError, frame_stream, laser_off_frame, pause_toggle_frame, stop_frames,
};
use crate::speed::Board;
use crate::status::{Status, StatusReport, looks_like_moshiboard};
use crate::transport::{self, Transport, TransportError, TransportInfo};
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::{Duration, Instant};
use tlaku_core::Point;
use tlaku_core::toolpath::Toolpath;

#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub board: Board,
    /// Ancho y alto de la cama (mm).
    pub bed_mm: [f64; 2],
    pub home: HomeCorner,
    pub autolock: bool,
    /// Velocidad máxima de corte (mm/s).
    pub max_speed_mm_s: f64,
    /// Forzar el modo sin rampa de aceleración (ver [`EncodeConfig::constant_speed`]).
    pub constant_speed: bool,
    pub fix_speeds: bool,
    /// Reenvíos de un paquete rechazado por CRC antes de rendirse.
    pub crc_retries: u32,
    /// Espera máxima a que la placa esté lista o confirme un paquete. Un paquete puede encerrar
    /// un movimiento largo y lento, así que es generosa.
    pub busy_timeout: Duration,
    /// Espera máxima de las órdenes en tiempo real (pausa, reanudar, detener).
    pub realtime_timeout: Duration,
    /// Sondeos seguidos sin un estado utilizable (0, truncado o desconocido) antes de abortar.
    pub anomaly_polls: u32,
    /// Pausa máxima entre sondeos cuando la placa tarda (tras los primeros diez).
    pub poll_backoff_max: Duration,
    /// Esperar el estado 236 en cada `-` del flujo (como MeerK40t). Si se desactiva, solo
    /// manda la espera por "lista" de cada paquete.
    pub strict_finish: bool,
    /// Espera máxima del home.
    pub home_timeout: Duration,
    /// Margen (s) sobre el tiempo estimado de un salto antes de darlo por terminado.
    pub motion_margin_s: f64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        let e = EncodeConfig::default();
        SessionConfig {
            board: e.board,
            bed_mm: e.bed_mm,
            home: e.home,
            autolock: e.autolock,
            max_speed_mm_s: e.max_speed_mm_s,
            constant_speed: false,
            fix_speeds: false,
            crc_retries: 3,
            busy_timeout: Duration::from_secs(600),
            realtime_timeout: Duration::from_secs(5),
            anomaly_polls: 200,
            poll_backoff_max: Duration::from_millis(50),
            strict_finish: true,
            home_timeout: Duration::from_secs(60),
            motion_margin_s: 3.0,
        }
    }
}

/// Estado del trabajo, visible desde otros hilos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    Idle,
    Running,
    Paused,
}

#[derive(Debug, Default)]
struct ControlInner {
    stop: AtomicBool,
    pause: AtomicBool,
    state: AtomicU8,
}

/// Mando de un trabajo en marcha: se puede clonar y usar desde otro hilo.
#[derive(Debug, Clone, Default)]
pub struct JobControl {
    inner: Arc<ControlInner>,
}

impl JobControl {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pide detener el trabajo: se cancela, se apaga el láser y la posición queda desconocida.
    pub fn request_stop(&self) {
        self.inner.stop.store(true, Ordering::SeqCst);
    }

    pub fn request_pause(&self) {
        self.inner.pause.store(true, Ordering::SeqCst);
    }

    pub fn request_resume(&self) {
        self.inner.pause.store(false, Ordering::SeqCst);
    }

    pub fn stop_requested(&self) -> bool {
        self.inner.stop.load(Ordering::SeqCst)
    }

    pub fn pause_requested(&self) -> bool {
        self.inner.pause.load(Ordering::SeqCst)
    }

    pub fn state(&self) -> JobState {
        match self.inner.state.load(Ordering::SeqCst) {
            1 => JobState::Running,
            2 => JobState::Paused,
            _ => JobState::Idle,
        }
    }

    fn set_state(&self, s: JobState) {
        self.inner.state.store(
            match s {
                JobState::Idle => 0,
                JobState::Running => 1,
                JobState::Paused => 2,
            },
            Ordering::SeqCst,
        );
    }

    fn reset(&self) {
        self.inner.stop.store(false, Ordering::SeqCst);
        self.inner.pause.store(false, Ordering::SeqCst);
    }
}

#[derive(Debug)]
pub enum SessionError {
    Transport(TransportError),
    Timeout {
        what: &'static str,
    },
    /// Tras los reenvíos, la placa sigue rechazando el paquete por CRC.
    CrcRepeated {
        packet: usize,
    },
    /// La placa informa 239: sin energía suficiente.
    LowPower,
    /// El estado delata una Moshiboard, que no es una M2 Nano.
    NotM2Nano {
        code: u8,
    },
    /// Estados inutilizables seguidos (0, truncados o desconocidos).
    NoStatus(Status),
    /// La placa no está en reposo.
    Busy(Status),
    /// No se sabe dónde está el cabezal: hay que hacer home.
    UnknownPosition,
    /// El programa se codificó para otro punto de partida o placa.
    StartMismatch {
        expected: (i64, i64),
        found: (i64, i64),
    },
    OutOfBed {
        x_mm: f64,
        y_mm: f64,
    },
    /// Ya hay un trabajo en marcha.
    JobActive,
    Encode(EncodeError),
    Frame(FrameError),
    BadProgram(&'static str),
    /// El usuario detuvo el trabajo.
    Cancelled,
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionError::Transport(e) => write!(f, "{e}"),
            SessionError::Timeout { what } => write!(f, "sin respuesta de la placa ({what})"),
            SessionError::CrcRepeated { packet } => write!(
                f,
                "la placa rechazó el paquete {} por CRC varias veces seguidas (¿cable USB \
                 defectuoso o interferencias?)",
                packet + 1
            ),
            SessionError::LowPower => write!(
                f,
                "la placa informa energía insuficiente (239): revisa el paro de emergencia, la \
                 tapa y que la fuente del láser esté encendida"
            ),
            SessionError::NotM2Nano { code } => write!(
                f,
                "esta placa responde con el código {code}, que no existe en la M2 Nano: parece \
                 una Moshiboard (mismo USB 1A86:5512). Tlaku no la soporta todavía"
            ),
            SessionError::NoStatus(s) => write!(
                f,
                "la placa no devuelve un estado utilizable ({})",
                s.describe()
            ),
            SessionError::Busy(s) => write!(f, "la placa no está en reposo ({})", s.describe()),
            SessionError::UnknownPosition => write!(
                f,
                "no se sabe dónde está el cabezal: haz home primero (la M2 Nano no informa su \
                 posición)"
            ),
            SessionError::StartMismatch { expected, found } => write!(
                f,
                "el trabajo se codificó desde {expected:?} mil y el cabezal está en {found:?}"
            ),
            SessionError::OutOfBed { x_mm, y_mm } => {
                write!(f, "({x_mm:.2}, {y_mm:.2}) mm queda fuera de la cama")
            }
            SessionError::JobActive => write!(f, "ya hay un trabajo en marcha"),
            SessionError::Encode(e) => write!(f, "{e}"),
            SessionError::Frame(e) => write!(f, "{e}"),
            SessionError::BadProgram(m) => write!(f, "{m}"),
            SessionError::Cancelled => write!(f, "cancelado por el usuario"),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<TransportError> for SessionError {
    fn from(e: TransportError) -> Self {
        SessionError::Transport(e)
    }
}

impl From<EncodeError> for SessionError {
    fn from(e: EncodeError) -> Self {
        SessionError::Encode(e)
    }
}

impl From<FrameError> for SessionError {
    fn from(e: FrameError) -> Self {
        SessionError::Frame(e)
    }
}

/// Lo que se sabe de la placa tras el sondeo.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardInfo {
    pub status: StatusReport,
    pub transport: TransportInfo,
    /// Acepta paquetes ahora mismo.
    pub ready: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub packet: usize,
    pub packets: usize,
    pub bytes_sent: usize,
    pub bytes_total: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JobReport {
    pub packets: usize,
    pub bytes: usize,
    /// Paquetes reenviados por CRC.
    pub resends: usize,
    /// Se vio el estado 236 al final (el trabajo terminó con confirmación de la placa).
    pub finish_confirmed: bool,
    pub elapsed: Duration,
    pub warnings: Vec<Warning>,
    /// Posición final del cabezal (mm, ejes de Tlaku).
    pub end_mm: Point,
}

/// Qué pasó con el intento de detener la placa tras un fallo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopOutcome {
    /// La placa confirmó `I` y `IUS1P`.
    Acked,
    /// No hubo confirmación: la placa puede seguir moviéndose o con el láser encendido; usa el
    /// paro de emergencia o abre la tapa.
    Unconfirmed,
}

/// El trabajo falló o se detuvo. La posición del cabezal queda desconocida.
#[derive(Debug)]
pub struct JobError {
    pub cause: SessionError,
    pub packets_sent: usize,
    pub packets_total: usize,
    pub stop: StopOutcome,
}

impl JobError {
    /// El láser pudo quedar encendido (no se confirmó el apagado).
    pub fn laser_may_be_on(&self) -> bool {
        self.stop == StopOutcome::Unconfirmed
    }
}

impl fmt::Display for JobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (paquetes confirmados: {} de {})",
            self.cause, self.packets_sent, self.packets_total
        )?;
        match self.stop {
            StopOutcome::Acked => write!(f, ". Se detuvo la placa y se apagó el láser"),
            StopOutcome::Unconfirmed => write!(
                f,
                ". Se intentó detener la placa y apagar el láser pero NO hubo confirmación: \
                 puede seguir en marcha. Usa el paro de emergencia o abre la tapa"
            ),
        }
    }
}

impl std::error::Error for JobError {}

/// Duración a partir de segundos, sin entrar en pánico con valores no finitos o negativos.
fn secs(x: f64, fallback: Duration) -> Duration {
    Duration::try_from_secs_f64(x).unwrap_or(fallback)
}

enum Confirm {
    Accepted,
    CrcRejected,
}

enum Step {
    Done,
    Continue,
}

pub struct LhySession<T: Transport> {
    transport: T,
    cfg: SessionConfig,
    control: JobControl,
    /// Posición del cabezal en mils (ejes de la placa) o `None` si no se sabe.
    pos: Option<(i64, i64)>,
    /// La pausa ya se mandó a la placa.
    pause_applied: bool,
    /// Último paquete confirmado, por si la placa avisa tarde de un CRC malo.
    last_confirmed: Option<Frame>,
    resends: usize,
}

impl<T: Transport> LhySession<T> {
    pub fn new(transport: T, cfg: SessionConfig) -> Self {
        LhySession {
            transport,
            cfg,
            control: JobControl::new(),
            pos: None,
            pause_applied: false,
            last_confirmed: None,
            resends: 0,
        }
    }

    pub fn config(&self) -> &SessionConfig {
        &self.cfg
    }

    /// Mando para pausar, reanudar o detener un trabajo desde otro hilo.
    pub fn control(&self) -> JobControl {
        self.control.clone()
    }

    pub fn into_transport(self) -> T {
        self.transport
    }

    /// Posición del cabezal (mils, ejes de la placa), si se sabe.
    pub fn position_mil(&self) -> Option<(i64, i64)> {
        self.pos
    }

    /// Posición del cabezal en el sistema de Tlaku (mm), si se sabe.
    pub fn position_mm(&self) -> Option<Point> {
        self.pos.map(|(x, y)| {
            self.cfg
                .home
                .from_machine_mm((mil_to_mm(x), mil_to_mm(y)), self.cfg.bed_mm)
        })
    }

    /// El usuario asegura que el cabezal está en el home (sin moverlo).
    pub fn assume_at_home(&mut self) {
        self.pos = Some((0, 0));
    }

    /// Configuración del codificador para el punto de partida actual.
    pub fn encode_config(&self) -> Result<EncodeConfig, SessionError> {
        let start = self.pos.ok_or(SessionError::UnknownPosition)?;
        Ok(EncodeConfig {
            board: self.cfg.board,
            bed_mm: self.cfg.bed_mm,
            home: self.cfg.home,
            start_mil: start,
            autolock: self.cfg.autolock,
            max_speed_mm_s: self.cfg.max_speed_mm_s,
            constant_speed: self.cfg.constant_speed,
            fix_speeds: self.cfg.fix_speeds,
            ..EncodeConfig::default()
        })
    }

    /// Codifica un trabajo desde la posición actual del cabezal.
    pub fn encode(&self, toolpath: &Toolpath) -> Result<Program, SessionError> {
        Ok(encode(toolpath, &self.encode_config()?)?)
    }

    // ------------------------------------------------------------------------------------
    // Estado

    fn backoff(&self, n: u32) {
        if n > 10 && !self.cfg.poll_backoff_max.is_zero() {
            let ms = u64::from(n - 10).min(1000);
            std::thread::sleep(Duration::from_millis(ms).min(self.cfg.poll_backoff_max));
        }
    }

    /// Una lectura de estado con reintentos cortos ante un tiempo agotado del USB.
    fn read_status(&mut self) -> Result<StatusReport, SessionError> {
        let mut last = TransportError::Timeout;
        for _ in 0..3 {
            match transport::read_status(&mut self.transport) {
                Ok(r) => return Ok(r),
                Err(TransportError::Timeout) => last = TransportError::Timeout,
                Err(e) => return Err(e.into()),
            }
        }
        Err(last.into())
    }

    /// Lee el estado de la placa (solo lectura: no mueve nada).
    pub fn status(&mut self) -> Result<StatusReport, SessionError> {
        let r = self.read_status()?;
        if looks_like_moshiboard(r.code()) {
            return Err(SessionError::NotM2Nano { code: r.code() });
        }
        Ok(r)
    }

    /// Sondeo de solo lectura: una consulta de estado. Detecta una Moshiboard.
    pub fn probe(&mut self) -> Result<BoardInfo, SessionError> {
        let status = self.status()?;
        let mut notes = Vec::new();
        match status.status {
            Status::LowPower => notes.push(
                "energía insuficiente (239): paro de emergencia pulsado, tapa abierta o fuente \
                 del láser apagada"
                    .to_string(),
            ),
            Status::CrcError => notes.push("la placa conserva un error de CRC previo".to_string()),
            Status::NoReply => notes.push("la placa no devolvió estado".to_string()),
            Status::Unknown(c) => notes.push(format!("código de estado desconocido ({c})")),
            _ => {}
        }
        Ok(BoardInfo {
            ready: status.status.accepts_packets(),
            status,
            transport: self.transport.info(),
            notes,
        })
    }

    /// Sondea el estado hasta que `pred` diga `Done`. Atiende pausa y detención si hay `ctl`.
    fn poll_until(
        &mut self,
        ctl: Option<&JobControl>,
        timeout: Duration,
        what: &'static str,
        mut pred: impl FnMut(&StatusReport) -> Step,
    ) -> Result<StatusReport, SessionError> {
        let mut start = Instant::now();
        let mut n = 0u32;
        let mut anomalies = 0u32;
        loop {
            if let Some(c) = ctl
                && self.service(c)?
            {
                // El tiempo en pausa no cuenta para el tiempo máximo de espera.
                start = Instant::now();
            }
            let r = self.read_status()?;
            if looks_like_moshiboard(r.code()) {
                return Err(SessionError::NotM2Nano { code: r.code() });
            }
            match r.status {
                Status::LowPower => return Err(SessionError::LowPower),
                Status::NoReply | Status::Unknown(_) => {
                    anomalies += 1;
                    if anomalies > self.cfg.anomaly_polls {
                        return Err(SessionError::NoStatus(r.status));
                    }
                }
                _ => anomalies = 0,
            }
            if let Step::Done = pred(&r) {
                return Ok(r);
            }
            if start.elapsed() > timeout {
                return Err(SessionError::Timeout { what });
            }
            n += 1;
            self.backoff(n);
        }
    }

    // ------------------------------------------------------------------------------------
    // Pausa, reanudación y detención

    /// Orden en tiempo real: se escribe sin esperar a que haya sitio (con la placa pausada y el
    /// búfer lleno nunca lo habría) y vale con que no haya error de CRC.
    fn send_realtime(&mut self, frame: &Frame) -> Result<(), SessionError> {
        let started = Instant::now();
        for _ in 0..=self.cfg.crc_retries {
            transport::write_frame(&mut self.transport, frame)?;
            let r = self.read_status()?;
            match r.status {
                Status::CrcError => {
                    self.resends += 1;
                    if started.elapsed() > self.cfg.realtime_timeout {
                        break;
                    }
                }
                Status::LowPower => return Err(SessionError::LowPower),
                _ => return Ok(()),
            }
        }
        Err(SessionError::CrcRepeated { packet: 0 })
    }

    /// Atiende las peticiones de pausa y detención. Si hay pausa, espera aquí. Devuelve `true`
    /// si estuvo en pausa (para no contar ese tiempo en las esperas).
    fn service(&mut self, ctl: &JobControl) -> Result<bool, SessionError> {
        if ctl.stop_requested() {
            return Err(SessionError::Cancelled);
        }
        let mut paused = false;
        if ctl.pause_requested() && !self.pause_applied {
            self.send_realtime(&pause_toggle_frame())?;
            self.pause_applied = true;
            ctl.set_state(JobState::Paused);
        }
        let mut idle_polls = 0u32;
        while self.pause_applied {
            paused = true;
            if ctl.stop_requested() {
                return Err(SessionError::Cancelled);
            }
            if !ctl.pause_requested() {
                self.send_realtime(&pause_toggle_frame())?;
                self.pause_applied = false;
                ctl.set_state(JobState::Running);
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
            idle_polls += 1;
            if idle_polls.is_multiple_of(25) {
                // Vigila la conexión y la energía mientras está en pausa.
                let r = self.read_status()?;
                if r.status == Status::LowPower {
                    return Err(SessionError::LowPower);
                }
            }
        }
        Ok(paused)
    }

    /// Detiene la placa y apaga el láser: `I` y `IUS1P`. No exige ninguna precondición.
    pub fn stop(&mut self) -> StopOutcome {
        let mut ok = true;
        for f in stop_frames() {
            if self.send_realtime(&f).is_err() {
                ok = false;
            }
        }
        self.pause_applied = false;
        self.last_confirmed = None;
        self.pos = None;
        self.control.set_state(JobState::Idle);
        if ok {
            StopOutcome::Acked
        } else {
            StopOutcome::Unconfirmed
        }
    }

    /// Intenta apagar el láser sin cancelar nada más (`IUS1P`).
    pub fn laser_off(&mut self) -> Result<(), SessionError> {
        self.send_realtime(&laser_off_frame())
    }

    // ------------------------------------------------------------------------------------
    // Envío de paquetes

    fn wait_ready(&mut self, ctl: &JobControl) -> Result<StatusReport, SessionError> {
        let t = self.cfg.busy_timeout;
        self.poll_until(Some(ctl), t, "placa lista", |r| {
            if r.status.accepts_packets() || r.status == Status::CrcError {
                Step::Done
            } else {
                Step::Continue
            }
        })
    }

    fn confirm(&mut self, ctl: &JobControl) -> Result<Confirm, SessionError> {
        let t = self.cfg.busy_timeout;
        let r = self.poll_until(Some(ctl), t, "confirmación del paquete", |r| {
            if r.status.accepts_packets() || r.status == Status::CrcError {
                Step::Done
            } else {
                Step::Continue
            }
        })?;
        Ok(if r.status == Status::CrcError {
            Confirm::CrcRejected
        } else {
            Confirm::Accepted
        })
    }

    /// Un paquete: espera a que la placa esté lista, lo escribe y espera la confirmación. Un CRC
    /// malo se resuelve reenviando; si la placa avisa tarde de un CRC malo del paquete anterior,
    /// se reenvía ese antes de seguir.
    fn send_frame(
        &mut self,
        frame: &Frame,
        index: usize,
        ctl: &JobControl,
    ) -> Result<(), SessionError> {
        let mut fails = 0u32;
        let mut current_failed = false;
        loop {
            let r = self.wait_ready(ctl)?;
            if r.status == Status::CrcError && !std::mem::take(&mut current_failed) {
                // Rechazo tardío del paquete anterior: se reenvía antes de seguir.
                fails += 1;
                if fails > self.cfg.crc_retries {
                    return Err(SessionError::CrcRepeated {
                        packet: index.saturating_sub(1),
                    });
                }
                if let Some(prev) = self.last_confirmed.clone() {
                    self.resends += 1;
                    transport::write_frame(&mut self.transport, &prev)?;
                    if let Confirm::CrcRejected = self.confirm(ctl)? {
                        continue;
                    }
                }
            }
            transport::write_frame(&mut self.transport, frame)?;
            match self.confirm(ctl)? {
                Confirm::Accepted => {
                    self.last_confirmed = Some(frame.clone());
                    return Ok(());
                }
                Confirm::CrcRejected => {
                    fails += 1;
                    self.resends += 1;
                    if fails > self.cfg.crc_retries {
                        return Err(SessionError::CrcRepeated { packet: index });
                    }
                    current_failed = true;
                }
            }
        }
    }

    /// Comprueba que la placa no haya rechazado tarde el último paquete confirmado; si lo hizo,
    /// lo reenvía. Se usa antes de dar un trabajo por terminado o de esperar su fin.
    fn resolve_late_crc(&mut self, ctl: &JobControl, index: usize) -> Result<(), SessionError> {
        let mut fails = 0u32;
        loop {
            let r = self.read_status()?;
            if r.status != Status::CrcError {
                return Ok(());
            }
            let Some(prev) = self.last_confirmed.clone() else {
                return Ok(());
            };
            fails += 1;
            if fails > self.cfg.crc_retries {
                return Err(SessionError::CrcRepeated { packet: index });
            }
            self.resends += 1;
            transport::write_frame(&mut self.transport, &prev)?;
            self.confirm(ctl)?;
        }
    }

    /// Espera a que la placa vacíe su búfer (estado 236). Con `strict` agotar el tiempo es un
    /// error; si no, se da por terminado y se devuelve `false`.
    fn wait_empty(
        &mut self,
        ctl: &JobControl,
        timeout: Duration,
        strict: bool,
    ) -> Result<bool, SessionError> {
        let r = self.poll_until(Some(ctl), timeout, "fin del movimiento", |r| {
            if r.status.is_empty() {
                Step::Done
            } else {
                Step::Continue
            }
        });
        match r {
            Ok(_) => Ok(true),
            Err(SessionError::Timeout { .. }) if !strict => Ok(false),
            Err(e) => Err(e),
        }
    }

    // ------------------------------------------------------------------------------------
    // Trabajo

    /// Envía un trabajo y espera a que termine. `progress` se llama tras cada paquete. Detener,
    /// pausar y reanudar se hacen con [`LhySession::control`].
    ///
    /// Exige que se conozca la posición del cabezal y que coincida con la que se usó al
    /// codificar. Si algo falla, el trabajo se aborta, se intenta apagar el láser y la posición
    /// queda desconocida.
    pub fn send_job(
        &mut self,
        program: &Program,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<JobReport, Box<JobError>> {
        let early = |cause: SessionError| {
            Box::new(JobError {
                cause,
                packets_sent: 0,
                packets_total: 0,
                stop: StopOutcome::Acked,
            })
        };
        if self.control.state() != JobState::Idle {
            return Err(early(SessionError::JobActive));
        }
        if program.board != self.cfg.board {
            return Err(early(SessionError::BadProgram(
                "el trabajo se codificó para otra placa",
            )));
        }
        let Some(pos) = self.pos else {
            return Err(early(SessionError::UnknownPosition));
        };
        if pos != program.start_mil {
            return Err(early(SessionError::StartMismatch {
                expected: program.start_mil,
                found: pos,
            }));
        }
        let frames = program.frames().map_err(|e| early(e.into()))?;
        // La placa debe estar en reposo y lista antes de empezar.
        let st = self.status().map_err(early)?;
        match st.status {
            Status::LowPower => return Err(early(SessionError::LowPower)),
            s @ (Status::NoReply | Status::Unknown(_)) => {
                return Err(early(SessionError::NoStatus(s)));
            }
            s if !s.accepts_packets() && s != Status::CrcError => {
                return Err(early(SessionError::Busy(s)));
            }
            _ => {}
        }

        let total = frames.len();
        let bytes_total = total * crate::crc::WIRE_LEN;
        let ctl = self.control.clone();
        ctl.reset();
        ctl.set_state(JobState::Running);
        self.pause_applied = false;
        self.last_confirmed = None;
        self.resends = 0;
        let started = Instant::now();
        let mut sent = 0usize;
        let mut finish_confirmed = false;

        let run = (|| -> Result<(), SessionError> {
            for (i, frame) in frames.iter().enumerate() {
                self.service(&ctl)?;
                self.send_frame(frame, i, &ctl)?;
                sent = i + 1;
                if frame.wait_finish || sent == total {
                    self.resolve_late_crc(&ctl, i)?;
                }
                progress(Progress {
                    packet: sent,
                    packets: total,
                    bytes_sent: sent * crate::crc::WIRE_LEN,
                    bytes_total,
                });
                if frame.wait_finish && (self.cfg.strict_finish || i + 1 == total) {
                    let strict = self.cfg.strict_finish;
                    // Sin espera estricta, el final del trabajo se espera lo que estima el
                    // codificador (con margen), no los diez minutos del tiempo máximo.
                    let limit = if strict {
                        self.cfg.busy_timeout
                    } else {
                        secs(
                            program.stats.est_seconds * 1.5 + self.cfg.motion_margin_s,
                            self.cfg.busy_timeout,
                        )
                        .min(self.cfg.busy_timeout)
                    };
                    let ok = self.wait_empty(&ctl, limit, strict)?;
                    if i + 1 == total {
                        finish_confirmed = ok;
                    }
                }
            }
            if total == 0 {
                finish_confirmed = true;
            }
            Ok(())
        })();

        match run {
            Ok(()) => {
                // Una pausa pedida justo al final se cancela: ya no hay nada que pausar.
                if self.pause_applied {
                    let _ = self.send_realtime(&pause_toggle_frame());
                    self.pause_applied = false;
                }
                ctl.set_state(JobState::Idle);
                self.pos = Some(program.end_mil);
                Ok(JobReport {
                    packets: total,
                    bytes: bytes_total,
                    resends: self.resends,
                    finish_confirmed,
                    elapsed: started.elapsed(),
                    warnings: program.warnings.clone(),
                    end_mm: program.end_mm(),
                })
            }
            Err(cause) => {
                let stop = self.stop();
                Err(Box::new(JobError {
                    cause,
                    packets_sent: sent,
                    packets_total: total,
                    stop,
                }))
            }
        }
    }

    // ------------------------------------------------------------------------------------
    // Saltos y home

    fn require_idle(&mut self) -> Result<(), SessionError> {
        if self.control.state() != JobState::Idle {
            return Err(SessionError::JobActive);
        }
        let st = self.status()?;
        match st.status {
            Status::LowPower => Err(SessionError::LowPower),
            s @ (Status::NoReply | Status::Unknown(_)) => Err(SessionError::NoStatus(s)),
            s if s.accepts_packets() || s == Status::CrcError => Ok(()),
            s => Err(SessionError::Busy(s)),
        }
    }

    /// Manda un flujo corto de saltos (sin `-`) y confirma cada paquete.
    fn send_short(&mut self, text: &str, ctl: &JobControl) -> Result<(), SessionError> {
        self.last_confirmed = None;
        let frames = frame_stream(text.as_bytes())?;
        for (i, f) in frames.iter().enumerate() {
            self.send_frame(f, i, ctl)?;
        }
        Ok(())
    }

    /// Lleva el cabezal al home (finales de carrera) y fija la posición en (0, 0).
    pub fn home(&mut self) -> Result<(), SessionError> {
        self.require_idle()?;
        let ctl = JobControl::new();
        self.send_short("IPP\n", &ctl)?;
        // El cabezal puede estar moviéndose: hasta que la placa vacíe su búfer o venza el tiempo.
        self.wait_empty(&ctl, self.cfg.home_timeout, false)?;
        self.pos = Some((0, 0));
        Ok(())
    }

    /// Salto con el láser apagado a una posición en mils de la placa.
    fn rapid_to_mil(&mut self, target: (i64, i64)) -> Result<(), SessionError> {
        let from = self.pos.ok_or(SessionError::UnknownPosition)?;
        let (dx, dy) = (target.0 - from.0, target.1 - from.1);
        if dx == 0 && dy == 0 {
            return Ok(());
        }
        let ctl = JobControl::new();
        let text = rapid_text(dx, dy, self.cfg.autolock);
        if let Err(e) = self.send_short(&text, &ctl) {
            // No se sabe si el salto llegó a ejecutarse.
            self.pos = None;
            return Err(e);
        }
        let dist_mm = mil_to_mm(dx.abs().max(dy.abs()));
        let limit = secs(
            dist_mm / RAPID_ESTIMATE_MM_S * 1.5 + self.cfg.motion_margin_s,
            self.cfg.busy_timeout,
        );
        match self.wait_empty(&ctl, limit, false) {
            Ok(_) => {
                self.pos = Some(target);
                Ok(())
            }
            Err(e) => {
                self.pos = None;
                Err(e)
            }
        }
    }

    fn machine_mil(&self, p: Point) -> Result<(i64, i64), SessionError> {
        let m = self.cfg.home.to_machine_mm(p, self.cfg.bed_mm);
        let eps = crate::speed::MM_PER_MIL;
        if !(m.0.is_finite() && m.1.is_finite())
            || m.0 < -eps
            || m.1 < -eps
            || m.0 > self.cfg.bed_mm[0] + eps
            || m.1 > self.cfg.bed_mm[1] + eps
        {
            return Err(SessionError::OutOfBed {
                x_mm: p.x,
                y_mm: p.y,
            });
        }
        Ok((mm_to_mil(m.0).max(0), mm_to_mil(m.1).max(0)))
    }

    /// Lleva el cabezal a un punto (mm, sistema de Tlaku) con el láser apagado.
    pub fn jog_to(&mut self, target: Point) -> Result<(), SessionError> {
        self.require_idle()?;
        let t = self.machine_mil(target)?;
        self.rapid_to_mil(t)
    }

    /// Desplaza el cabezal `dx`, `dy` mm (ejes de Tlaku) con el láser apagado.
    pub fn jog_by(&mut self, dx: f64, dy: f64) -> Result<(), SessionError> {
        self.require_idle()?;
        let here = self.position_mm().ok_or(SessionError::UnknownPosition)?;
        let t = self.machine_mil(Point::new(here.x + dx, here.y + dy))?;
        self.rapid_to_mil(t)
    }

    /// Recorre las cuatro esquinas del rectángulo (mm) con el láser apagado.
    pub fn frame(&mut self, min: Point, max: Point) -> Result<(), SessionError> {
        self.require_idle()?;
        let corners = [
            Point::new(min.x, min.y),
            Point::new(max.x, min.y),
            Point::new(max.x, max.y),
            Point::new(min.x, max.y),
            Point::new(min.x, min.y),
        ];
        // Se valida todo el recorrido antes de mover nada.
        let targets: Vec<(i64, i64)> = corners
            .iter()
            .map(|&c| self.machine_mil(c))
            .collect::<Result<_, _>>()?;
        for t in targets {
            self.rapid_to_mil(t)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emu::{EmuBoard, EmuConfig, EmuFaults, EmuTransport, emulator};
    use crate::encode::Warning;
    use std::sync::Mutex;
    use tlaku_core::toolpath::{Move, ToolpathBlock};

    type Shared = Arc<Mutex<EmuBoard>>;

    fn fast_cfg() -> SessionConfig {
        SessionConfig {
            busy_timeout: Duration::from_secs(5),
            realtime_timeout: Duration::from_secs(1),
            poll_backoff_max: Duration::ZERO,
            home_timeout: Duration::from_secs(5),
            ..SessionConfig::default()
        }
    }

    fn session(emu: EmuConfig) -> (LhySession<EmuTransport>, Shared) {
        let (t, b) = emulator(emu);
        (LhySession::new(t, fast_cfg()), b)
    }

    fn square(x0: f64, y0: f64, side: f64, speed_mm_min: f64) -> Toolpath {
        let c = |x: f64, y: f64| Move::Cut {
            to: Point::new(x, y),
            power_pct: 50.0,
        };
        Toolpath {
            blocks: vec![ToolpathBlock {
                speed_mm_min,
                moves: vec![
                    Move::Travel {
                        to: Point::new(x0, y0),
                    },
                    c(x0 + side, y0),
                    c(x0 + side, y0 + side),
                    c(x0, y0 + side),
                    c(x0, y0),
                ],
                ..Default::default()
            }],
        }
    }

    fn no_progress() -> impl FnMut(Progress) {
        |_| {}
    }

    fn board(b: &Shared) -> std::sync::MutexGuard<'_, EmuBoard> {
        b.lock().unwrap()
    }

    #[test]
    fn probe_reads_only_the_status() {
        let (mut s, b) = session(EmuConfig::default());
        let info = s.probe().unwrap();
        assert!(info.ready && info.notes.is_empty());
        assert_eq!(info.status.status, Status::Ok);
        assert_eq!(info.transport.chip_version, Some(0x30));
        assert_eq!(board(&b).accepted, 0, "el sondeo no escribe paquetes");
    }

    #[test]
    fn a_moshiboard_is_detected_and_rejected() {
        let (mut s, _b) = session(EmuConfig {
            moshiboard: true,
            ..Default::default()
        });
        match s.probe() {
            Err(SessionError::NotM2Nano { code: 205 }) => {}
            other => panic!("{other:?}"),
        }
        let e = s.probe().unwrap_err();
        assert!(e.to_string().contains("Moshiboard"));
    }

    #[test]
    fn jobs_need_a_known_position() {
        let (mut s, _b) = session(EmuConfig::default());
        assert!(matches!(
            s.encode(&square(10.0, 150.0, 20.0, 1200.0)),
            Err(SessionError::UnknownPosition)
        ));
        assert!(matches!(
            s.jog_by(1.0, 1.0),
            Err(SessionError::UnknownPosition)
        ));
    }

    #[test]
    fn full_job_in_the_emulator_burns_the_square_and_never_overruns() {
        let (mut s, b) = session(EmuConfig::default());
        s.probe().unwrap();
        s.assume_at_home();
        let program = s.encode(&square(10.0, 150.0, 20.0, 1200.0)).unwrap();
        let mut seen = Vec::new();
        let report = s.send_job(&program, &mut |p| seen.push(p)).unwrap();
        assert_eq!(report.packets, program.frames().unwrap().len());
        assert!(report.finish_confirmed);
        assert_eq!(report.resends, 0);
        assert_eq!(seen.len(), report.packets);
        assert!(seen.windows(2).all(|w| w[0].bytes_sent < w[1].bytes_sent));
        assert_eq!(seen.last().unwrap().packet, report.packets);
        assert!(!report.warnings.is_empty(), "potencia ignorada");
        assert!(
            report
                .warnings
                .iter()
                .any(|w| matches!(w, Warning::PowerIgnored { .. }))
        );

        let mut e = board(&b);
        e.run_to_idle();
        assert_eq!(e.overruns, 0, "nunca se escribe con el búfer lleno");
        assert!(
            e.busy_replies > 0,
            "la prueba debe haber pasado por «ocupada»"
        );
        assert_eq!(e.rejected, 0);
        assert!(!e.laser_on());
        // El láser quemó el cuadrado de 20 mm y nada más.
        assert!(
            (e.burn_length_mm() - 80.0).abs() < 0.3,
            "{}",
            e.burn_length_mm()
        );
        let (lo, hi) = e.burn_bbox_mm().unwrap();
        // En ejes de la placa (home arriba-izquierda): x 10..30, y = 200-150-20 .. 200-150.
        assert!(
            (lo.0 - 10.0).abs() < 0.1 && (hi.0 - 30.0).abs() < 0.1,
            "{lo:?} {hi:?}"
        );
        assert!(
            (lo.1 - 30.0).abs() < 0.1 && (hi.1 - 50.0).abs() < 0.1,
            "{lo:?} {hi:?}"
        );
        drop(e);
        // El cabezal terminó donde dice la sesión.
        let end = s.position_mil().unwrap();
        let head = board(&b).head_mil();
        assert!((head.0 - end.0 as f64).abs() < 1.0 && (head.1 - end.1 as f64).abs() < 1.0);
        assert_eq!(s.control().state(), JobState::Idle);
    }

    #[test]
    fn the_emulator_trace_never_burns_during_travels() {
        // Dos cuadrados separados: el salto entre ellos y los viajes cortos no queman.
        let mut tp = square(10.0, 150.0, 10.0, 1200.0);
        let second = square(60.0, 100.0, 10.0, 1200.0);
        tp.blocks[0]
            .moves
            .extend(second.blocks[0].moves.iter().copied());
        // Un viaje corto (3 mm) también.
        tp.blocks[0].moves.push(Move::Travel {
            to: Point::new(63.0, 100.0),
        });
        tp.blocks[0].moves.push(Move::Cut {
            to: Point::new(68.0, 100.0),
            power_pct: 50.0,
        });
        let (mut s, b) = session(EmuConfig::default());
        s.assume_at_home();
        let program = s.encode(&tp).unwrap();
        s.send_job(&program, &mut no_progress()).unwrap();
        let mut e = board(&b);
        e.run_to_idle();
        let burn_toolpath = 40.0 + 40.0 + 5.0;
        assert!(
            (e.burn_length_mm() - burn_toolpath).abs() < 0.5,
            "{}",
            e.burn_length_mm()
        );
        // Cada tramo quemado está sobre algún segmento cortado del Toolpath (ejes de la placa).
        let segs: Vec<((f64, f64), (f64, f64))> = {
            let mut v = Vec::new();
            let mut cur = (0.0, 0.0);
            for m in &tp.blocks[0].moves {
                let to = m.to();
                let mm = (to.x, 200.0 - to.y);
                if let Move::Cut { power_pct, .. } = m
                    && *power_pct > 0.0
                {
                    v.push((cur, mm));
                }
                cur = mm;
            }
            v
        };
        for (a, bb) in e.burn_pieces_mm() {
            for p in [a, bb] {
                let d = segs
                    .iter()
                    .map(|&(s0, s1)| dist_point_segment(p, s0, s1))
                    .fold(f64::INFINITY, f64::min);
                assert!(d < 0.06, "quemó a {d} mm del trazado en {p:?}");
            }
        }
    }

    fn dist_point_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
        let (abx, aby) = (b.0 - a.0, b.1 - a.1);
        let l2 = abx * abx + aby * aby;
        let t = if l2 == 0.0 {
            0.0
        } else {
            (((p.0 - a.0) * abx + (p.1 - a.1) * aby) / l2).clamp(0.0, 1.0)
        };
        ((p.0 - (a.0 + t * abx)).powi(2) + (p.1 - (a.1 + t * aby)).powi(2)).sqrt()
    }

    #[test]
    fn a_crc_error_is_resent_and_the_job_completes() {
        let (mut s, b) = session(EmuConfig {
            faults: EmuFaults {
                corrupt_packet: Some(1),
                ..Default::default()
            },
            ..Default::default()
        });
        s.assume_at_home();
        let program = s.encode(&square(10.0, 150.0, 20.0, 1200.0)).unwrap();
        let r = s.send_job(&program, &mut no_progress()).unwrap();
        assert_eq!(r.resends, 1);
        let mut e = board(&b);
        e.run_to_idle();
        assert_eq!(e.rejected, 1);
        assert!(
            (e.burn_length_mm() - 80.0).abs() < 0.3,
            "no se perdió ni duplicó nada"
        );
    }

    #[test]
    fn a_late_crc_report_resends_the_previous_packet_in_order() {
        let (mut s, b) = session(EmuConfig {
            faults: EmuFaults {
                late_crc_packet: Some(2),
                ..Default::default()
            },
            ..Default::default()
        });
        s.assume_at_home();
        let program = s.encode(&square(10.0, 150.0, 20.0, 1200.0)).unwrap();
        let frames = program.frames().unwrap();
        s.send_job(&program, &mut no_progress()).unwrap();
        let mut e = board(&b);
        e.run_to_idle();
        // Las cargas aceptadas son exactamente las del trabajo, en orden y sin huecos.
        let accepted: Vec<_> = e.packets.clone();
        let want: Vec<_> = frames.iter().map(|f| f.payload).collect();
        assert_eq!(accepted, want);
        assert_eq!(e.rejected, 1);
    }

    #[test]
    fn repeated_crc_errors_abort_the_job_and_stop_the_board() {
        let (mut s, b) = session(EmuConfig {
            faults: EmuFaults {
                corrupt_from: Some(2),
                ..Default::default()
            },
            ..Default::default()
        });
        s.assume_at_home();
        let program = s.encode(&square(10.0, 150.0, 20.0, 1200.0)).unwrap();
        let err = s.send_job(&program, &mut no_progress()).unwrap_err();
        assert!(
            matches!(err.cause, SessionError::CrcRepeated { .. }),
            "{err}"
        );
        assert_eq!(err.packets_sent, 2);
        // Con CRC malo el emulador rechaza todo, incluida la parada: no hay confirmación.
        assert_eq!(err.stop, StopOutcome::Unconfirmed);
        assert!(err.laser_may_be_on());
        assert!(err.to_string().contains("paro de emergencia"));
        assert_eq!(s.position_mil(), None);
        assert_eq!(board(&b).overruns, 0);
    }

    #[test]
    fn a_usb_disconnect_mid_job_aborts_cleanly() {
        let (mut s, b) = session(EmuConfig {
            faults: EmuFaults {
                disconnect_after_packets: Some(2),
                ..Default::default()
            },
            ..Default::default()
        });
        s.assume_at_home();
        let program = s.encode(&square(10.0, 150.0, 20.0, 1200.0)).unwrap();
        let err = s.send_job(&program, &mut no_progress()).unwrap_err();
        assert!(
            matches!(
                err.cause,
                SessionError::Transport(TransportError::Disconnected)
            ),
            "{err}"
        );
        assert_eq!(err.stop, StopOutcome::Unconfirmed);
        assert_eq!(s.position_mil(), None);
        assert_eq!(s.control().state(), JobState::Idle);
        assert_eq!(board(&b).accepted, 2);
    }

    #[test]
    fn a_board_stuck_busy_times_out_instead_of_writing() {
        let (t, b) = emulator(EmuConfig {
            faults: EmuFaults {
                busy_from_poll: Some(3),
                ..Default::default()
            },
            ..Default::default()
        });
        let mut s = LhySession::new(
            t,
            SessionConfig {
                busy_timeout: Duration::from_millis(100),
                ..fast_cfg()
            },
        );
        s.assume_at_home();
        let program = s.encode(&square(10.0, 150.0, 20.0, 1200.0)).unwrap();
        let err = s.send_job(&program, &mut no_progress()).unwrap_err();
        assert!(
            matches!(err.cause, SessionError::Timeout { .. })
                || matches!(err.cause, SessionError::Busy(_)),
            "{err}"
        );
        assert_eq!(board(&b).overruns, 0);
    }

    #[test]
    fn low_power_aborts_the_job() {
        let (mut s, _b) = session(EmuConfig {
            faults: EmuFaults {
                low_power_after_packets: Some(1),
                ..Default::default()
            },
            ..Default::default()
        });
        s.assume_at_home();
        let program = s.encode(&square(10.0, 150.0, 20.0, 1200.0)).unwrap();
        let err = s.send_job(&program, &mut no_progress()).unwrap_err();
        assert!(matches!(err.cause, SessionError::LowPower), "{err}");
        assert!(err.to_string().contains("239"));
    }

    #[test]
    fn a_status_without_data_aborts_after_many_polls() {
        let (t, _b) = emulator(EmuConfig {
            faults: EmuFaults {
                empty_from_poll: Some(3),
                ..Default::default()
            },
            ..Default::default()
        });
        let mut s = LhySession::new(
            t,
            SessionConfig {
                anomaly_polls: 20,
                ..fast_cfg()
            },
        );
        s.assume_at_home();
        let program = s.encode(&square(10.0, 150.0, 20.0, 1200.0)).unwrap();
        let err = s.send_job(&program, &mut no_progress()).unwrap_err();
        assert!(
            matches!(err.cause, SessionError::NoStatus(Status::NoReply)),
            "{err}"
        );
    }

    #[test]
    fn stop_mid_job_cancels_turns_the_laser_off_and_forgets_the_position() {
        let (t, b) = emulator(EmuConfig::default());
        let mut s = LhySession::new(t, fast_cfg());
        s.assume_at_home();
        // Trabajo largo: lados de 150 mm a 20 mm/s.
        let program = s.encode(&square(10.0, 20.0, 150.0, 1200.0)).unwrap();
        let control = s.control();
        let b2 = Arc::clone(&b);
        let mut laser_was_on = false;
        let err = s
            .send_job(&program, &mut |p| {
                if p.packet == 4 {
                    laser_was_on = b2.lock().unwrap().laser_on();
                    control.request_stop();
                }
            })
            .unwrap_err();
        assert!(
            laser_was_on,
            "la prueba debe detener con el láser disparando"
        );
        assert!(matches!(err.cause, SessionError::Cancelled), "{err}");
        assert_eq!(err.stop, StopOutcome::Acked);
        assert!(!err.laser_may_be_on());
        assert_eq!(err.packets_sent, 4);
        let mut e = board(&b);
        assert!(!e.laser_on(), "el láser quedó apagado");
        e.run_to_idle();
        assert!(e.burn_length_mm() < 4.0 * 150.0, "no terminó el cuadrado");
        assert!(!e.laser_on() && !e.busy());
        assert!(e.aborts >= 1);
        drop(e);
        assert_eq!(s.position_mil(), None);
        assert_eq!(s.control().state(), JobState::Idle);
        // Tras detener hay que hacer home antes de otro trabajo.
        assert!(matches!(
            s.send_job(&program, &mut no_progress()).unwrap_err().cause,
            SessionError::UnknownPosition
        ));
        s.home().unwrap();
        assert_eq!(s.position_mil(), Some((0, 0)));
    }

    #[test]
    fn pause_and_resume_from_another_thread() {
        let (t, b) = emulator(EmuConfig::default());
        let mut s = LhySession::new(t, fast_cfg());
        s.assume_at_home();
        let program = s.encode(&square(10.0, 20.0, 30.0, 3000.0)).unwrap();
        let control = s.control();
        let (c2, b2) = (control.clone(), Arc::clone(&b));
        let helper = std::thread::spawn(move || {
            // Espera a que la sesión haya mandado la pausa.
            let t0 = Instant::now();
            while c2.state() != JobState::Paused {
                assert!(t0.elapsed() < Duration::from_secs(5), "la pausa no llegó");
                std::thread::yield_now();
            }
            {
                let e = b2.lock().unwrap();
                assert!(e.is_paused());
                assert!(!e.laser_on(), "en pausa el láser está apagado");
            }
            std::thread::sleep(Duration::from_millis(50));
            c2.request_resume();
        });
        let r = s
            .send_job(&program, &mut |p| {
                if p.packet == 3 {
                    control.request_pause();
                }
            })
            .unwrap();
        helper.join().unwrap();
        assert!(r.finish_confirmed);
        let mut e = board(&b);
        e.run_to_idle();
        assert_eq!(e.pause_toggles, 2);
        assert!(!e.is_paused());
        assert!(
            (e.burn_length_mm() - 120.0).abs() < 0.5,
            "{}",
            e.burn_length_mm()
        );
        drop(e);
        assert_eq!(s.control().state(), JobState::Idle);
    }

    #[test]
    fn stop_while_paused_works() {
        let (t, b) = emulator(EmuConfig::default());
        let mut s = LhySession::new(t, fast_cfg());
        s.assume_at_home();
        let program = s.encode(&square(10.0, 20.0, 100.0, 1200.0)).unwrap();
        let control = s.control();
        let c2 = control.clone();
        let helper = std::thread::spawn(move || {
            let t0 = Instant::now();
            while c2.state() != JobState::Paused {
                assert!(t0.elapsed() < Duration::from_secs(5));
                std::thread::yield_now();
            }
            c2.request_stop();
        });
        let err = s
            .send_job(&program, &mut |p| {
                if p.packet == 3 {
                    control.request_pause();
                }
            })
            .unwrap_err();
        helper.join().unwrap();
        assert!(matches!(err.cause, SessionError::Cancelled));
        assert_eq!(err.stop, StopOutcome::Acked);
        let e = board(&b);
        assert!(!e.is_paused() && !e.laser_on());
    }

    #[test]
    fn home_jog_and_frame_move_the_head_with_the_laser_off() {
        let (mut s, b) = session(EmuConfig::default());
        s.home().unwrap();
        assert_eq!(s.position_mil(), Some((0, 0)));
        s.jog_to(Point::new(100.0, 100.0)).unwrap();
        {
            let mut e = board(&b);
            e.run_to_idle();
            // Home arriba a la izquierda: (100, 100) de Tlaku es x=100, y=200-100=100 de la placa.
            let h = e.head_mm();
            assert!(
                (h.0 - 100.0).abs() < 0.05 && (h.1 - 100.0).abs() < 0.05,
                "{h:?}"
            );
        }
        let p = s.position_mm().unwrap();
        assert!((p.x - 100.0).abs() < 0.05 && (p.y - 100.0).abs() < 0.05);
        s.jog_by(10.0, -20.0).unwrap();
        let p = s.position_mm().unwrap();
        assert!((p.x - 110.0).abs() < 0.05 && (p.y - 80.0).abs() < 0.05);
        s.frame(Point::new(20.0, 40.0), Point::new(60.0, 90.0))
            .unwrap();
        let mut e = board(&b);
        e.run_to_idle();
        assert_eq!(
            e.burn_length_mm(),
            0.0,
            "ningún movimiento de estos enciende el láser"
        );
        assert_eq!(e.overruns, 0);
        let h = e.head_mm();
        assert!(
            (h.0 - 20.0).abs() < 0.05 && (h.1 - 160.0).abs() < 0.05,
            "{h:?}"
        );
        drop(e);
        // Home de nuevo.
        s.home().unwrap();
        board(&b).run_to_idle();
        assert_eq!(board(&b).head_mil(), (0.0, 0.0));
    }

    #[test]
    fn jogs_outside_the_bed_are_rejected_before_moving() {
        let (mut s, b) = session(EmuConfig::default());
        s.assume_at_home();
        let before = board(&b).accepted;
        assert!(matches!(
            s.jog_to(Point::new(301.0, 10.0)),
            Err(SessionError::OutOfBed { .. })
        ));
        assert!(matches!(
            s.jog_by(-5.0, 0.0).and(s.jog_to(Point::new(10.0, 201.0))),
            Err(SessionError::OutOfBed { .. })
        ));
        // El marco se valida completo antes de moverse.
        assert!(matches!(
            s.frame(Point::new(10.0, 10.0), Point::new(400.0, 50.0)),
            Err(SessionError::OutOfBed { .. })
        ));
        assert_eq!(board(&b).accepted, before);
        assert_eq!(s.position_mil(), Some((0, 0)));
    }

    #[test]
    fn a_program_for_another_start_or_board_is_refused() {
        let (mut s, _b) = session(EmuConfig::default());
        s.assume_at_home();
        let program = s.encode(&square(10.0, 150.0, 20.0, 1200.0)).unwrap();
        s.jog_to(Point::new(50.0, 50.0)).unwrap();
        let e = s.send_job(&program, &mut no_progress()).unwrap_err();
        assert!(matches!(e.cause, SessionError::StartMismatch { .. }), "{e}");
        let mut other = program.clone();
        other.board = Board::B2;
        assert!(matches!(
            s.send_job(&other, &mut no_progress()).unwrap_err().cause,
            SessionError::BadProgram(_)
        ));
    }

    #[test]
    fn two_jobs_in_a_row_chain_through_the_known_position() {
        let (mut s, b) = session(EmuConfig::default());
        s.assume_at_home();
        let p1 = s.encode(&square(10.0, 150.0, 20.0, 1200.0)).unwrap();
        s.send_job(&p1, &mut no_progress()).unwrap();
        let p2 = s.encode(&square(50.0, 100.0, 20.0, 1500.0)).unwrap();
        assert_eq!(p2.start_mil, p1.end_mil);
        s.send_job(&p2, &mut no_progress()).unwrap();
        let mut e = board(&b);
        e.run_to_idle();
        assert!((e.burn_length_mm() - 160.0).abs() < 0.5);
        assert_eq!(e.overruns, 0);
    }

    #[test]
    fn disabling_strict_finish_still_completes() {
        let (t, b) = emulator(EmuConfig::default());
        let mut s = LhySession::new(
            t,
            SessionConfig {
                strict_finish: false,
                ..fast_cfg()
            },
        );
        s.assume_at_home();
        let program = s.encode(&square(10.0, 150.0, 20.0, 1200.0)).unwrap();
        s.send_job(&program, &mut no_progress()).unwrap();
        board(&b).run_to_idle();
        assert!((board(&b).burn_length_mm() - 80.0).abs() < 0.3);
    }

    #[test]
    fn encode_errors_surface_before_any_write() {
        let (mut s, b) = session(EmuConfig::default());
        s.assume_at_home();
        let e = s.encode(&square(290.0, 150.0, 40.0, 1200.0)).unwrap_err();
        assert!(
            matches!(e, SessionError::Encode(EncodeError::OutOfBed { .. })),
            "{e}"
        );
        assert_eq!(board(&b).accepted, 0);
    }
}
