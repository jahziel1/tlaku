//! Sesión con la tarjeta: identificar, encuadrar con luz roja, marcar con progreso, detener.
//!
//! Misma forma que `RuidaSession` y `LhySession`: `new`, `probe`, `status`, `send_job` con
//! progreso, `frame`, `jog_to`, `stop`, y un [`JobControl`] clonable para detener desde otro hilo
//! (Ctrl+C, botón de la app). **No hay pausa**: en reportes de usuarios (docs/hardware/galvo.md,
//! sección 5) pausar deja el espejo quieto con el haz encendido, así que solo se puede detener.
//!
//! # Garantías de seguridad (verificadas con la tarjeta simulada, no con hardware)
//!
//! - Toda orden sale por [`GalvoSession::send`], que pasa por `safety::check_immediate`: lo que
//!   no está en la lista blanca (EEPROM, configuración, tabla de corrección) nunca se envía, y
//!   `EnableLaser`, `Fiber_SetMo(1)` y el bit del láser solo se permiten dentro de un marcado real.
//! - Toda lista pasa por `safety::validate_list` antes de tocar la tarjeta.
//! - Cualquier salida de `send_job`, `frame` o `jog_to` (éxito, error, cancelación) termina con
//!   la **secuencia de apagado** completa (`safety::shutdown_sequence`), con reintentos. Si no se
//!   confirma, el error lo dice (`laser_may_be_on`).
//! - `Drop` repite el apagado si la sesión quedó sucia. Si el USB se cae, nada de esto llega a la
//!   tarjeta: lo que haga la tarjeta sin anfitrión **no se puede garantizar por software** y hay
//!   que medirlo (docs/hardware/galvo.md, sección 9).
//! - Cada marcado empieza con la fuente desarmada y la arma justo antes de `ExecuteList`.
//! - No se ejecuta la secuencia de inicio de EZCAD (modos, standby, tabla de corrección...): eso
//!   es configuración de la tarjeta y Tlaku no la escribe. Si la tarjeta la necesita, hay que
//!   dejarla inicializada con EZCAD antes de encenderla de nuevo; la corrección de campo se hace
//!   en el anfitrión y supone la tabla de la tarjeta en identidad.

use crate::calib::FieldMap;
use crate::cmd::Imm;
use crate::compile::{
    self, CompileError, CompileOptions, CompiledList, FieldOrigin, LightOptions, LightShape,
    ListStats,
};
use crate::params;
use crate::safety::{self, ImmContext, ListKind, SafetyError, SafetyLimits};
use crate::transport::{self, Support, Transport, TransportError, TransportInfo};
use crate::wire;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::{Duration, Instant};
use tlaku_core::Point;
use tlaku_core::toolpath::Toolpath;
use tlaku_doc::GalvoDevice;

/// Condición sobre las entradas que debe cumplirse antes de cada trabajo (enclavamiento).
/// `(entradas & mask) == value`. Los bits de EMSTOP y alarmas no están documentados: el usuario
/// los fija a partir de lo que mida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputCheck {
    pub mask: u16,
    pub value: u16,
}

#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub device: GalvoDevice,
    pub limits: SafetyLimits,
    pub origin: FieldOrigin,
    /// Espera entre consultas de estado (0 con la tarjeta simulada).
    pub poll_interval: Duration,
    /// Máximo de espera a que la tarjeta pueda recibir otra tanda.
    pub ready_timeout: Duration,
    /// Tandas enviadas antes de `ExecuteList` (galvoplotter: 3).
    pub execute_after_batches: usize,
    /// Espera tras abrir el MO de la fibra, ms.
    pub mo_delay_ms: f64,
    pub inputs_required: Option<InputCheck>,
    /// Reintentos de cada paso del apagado.
    pub shutdown_retries: u32,
    /// Reintentos de lectura de estado ante un tiempo agotado.
    pub status_retries: u32,
}

impl SessionConfig {
    pub fn new(device: GalvoDevice) -> Result<Self, SafetyError> {
        let limits = SafetyLimits::from_device(&device)?;
        Ok(SessionConfig {
            device,
            limits,
            origin: FieldOrigin::Center,
            poll_interval: Duration::from_millis(5),
            ready_timeout: Duration::from_secs(30),
            execute_after_batches: 3,
            mo_delay_ms: 8.0,
            inputs_required: None,
            shutdown_retries: 3,
            status_retries: 3,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    Idle,
    Running,
}

#[derive(Debug, Default)]
struct ControlInner {
    stop: AtomicBool,
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

    /// Pide detener: se cancela, se apaga todo y los espejos quedan donde estén.
    pub fn request_stop(&self) {
        self.inner.stop.store(true, Ordering::SeqCst);
    }

    pub fn stop_requested(&self) -> bool {
        self.inner.stop.load(Ordering::SeqCst)
    }

    /// Borra una petición de detener anterior para poder lanzar otro trabajo.
    pub fn clear(&self) {
        self.inner.stop.store(false, Ordering::SeqCst);
    }

    pub fn state(&self) -> JobState {
        if self.inner.state.load(Ordering::SeqCst) == 1 {
            JobState::Running
        } else {
            JobState::Idle
        }
    }

    fn set_state(&self, s: JobState) {
        self.inner
            .state
            .store(u8::from(s == JobState::Running), Ordering::SeqCst);
    }
}

/// Estado de la tarjeta según la palabra 3 de `GetVersion` (galvoplotter; sin verificar).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardStatus {
    pub raw: u16,
    pub busy: bool,
    pub ready: bool,
    pub axis: bool,
}

impl BoardStatus {
    pub fn from_word(raw: u16) -> Self {
        BoardStatus {
            raw,
            busy: raw & 0x04 != 0,
            ready: raw & 0x20 != 0,
            axis: raw & 0x40 != 0,
        }
    }
}

/// Lo que se sabe de la tarjeta tras el sondeo (solo lectura).
#[derive(Debug, Clone, PartialEq)]
pub struct BoardInfo {
    pub transport: TransportInfo,
    pub board_name: String,
    pub serial: [u16; 4],
    pub version: [u16; 4],
    pub status: BoardStatus,
    pub inputs: u16,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub batch: usize,
    pub batches: usize,
    pub bytes_sent: usize,
    pub bytes_total: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightProgress {
    /// Vueltas completas del encuadre hasta ahora.
    pub loops: u32,
}

/// Qué pasó con el intento de dejar la tarjeta en estado seguro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopOutcome {
    /// No se tocó la tarjeta: no hacía falta apagar nada.
    NotNeeded,
    /// Todos los pasos del apagado recibieron respuesta.
    Acked,
    /// Algún paso no se confirmó: la fuente puede seguir habilitada. Abre la tapa o usa el paro
    /// de emergencia.
    Unconfirmed,
}

#[derive(Debug)]
pub enum SessionError {
    Transport(TransportError),
    Timeout {
        what: &'static str,
    },
    /// VID:PID que no es una tarjeta compatible.
    UnknownBoard {
        vendor_id: u16,
        product_id: u16,
        why: String,
    },
    /// La tarjeta está ejecutando algo.
    Busy,
    Cancelled,
    Compile(CompileError),
    Safety(SafetyError),
    BadReply(&'static str),
    /// Las entradas no cumplen el enclavamiento pedido.
    InterlockOpen {
        inputs: u16,
        mask: u16,
        value: u16,
    },
    /// El trabajo terminó pero el apagado final no se confirmó.
    ShutdownUnconfirmed,
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionError::Transport(e) => write!(f, "{e}"),
            SessionError::Timeout { what } => write!(f, "tiempo de espera agotado: {what}"),
            SessionError::UnknownBoard {
                vendor_id,
                product_id,
                why,
            } => write!(
                f,
                "tarjeta {vendor_id:04X}:{product_id:04X} no compatible: {why}"
            ),
            SessionError::Busy => write!(f, "la tarjeta está ocupada con otro trabajo"),
            SessionError::Cancelled => write!(f, "cancelado"),
            SessionError::Compile(e) => write!(f, "{e}"),
            SessionError::Safety(e) => write!(f, "{e}"),
            SessionError::BadReply(w) => write!(f, "respuesta inesperada de la tarjeta: {w}"),
            SessionError::InterlockOpen {
                inputs,
                mask,
                value,
            } => write!(
                f,
                "el enclavamiento no está cerrado: entradas {inputs:#06x}, se exigía \
                 (entradas & {mask:#06x}) == {value:#06x}"
            ),
            SessionError::ShutdownUnconfirmed => write!(
                f,
                "el trabajo terminó pero no se pudo confirmar el apagado de la fuente"
            ),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<TransportError> for SessionError {
    fn from(e: TransportError) -> Self {
        SessionError::Transport(e)
    }
}

impl From<CompileError> for SessionError {
    fn from(e: CompileError) -> Self {
        SessionError::Compile(e)
    }
}

impl From<SafetyError> for SessionError {
    fn from(e: SafetyError) -> Self {
        SessionError::Safety(e)
    }
}

/// El trabajo falló, se detuvo o terminó sin poder confirmar el apagado.
#[derive(Debug)]
pub struct JobError {
    pub cause: SessionError,
    pub batches_sent: usize,
    pub batches_total: usize,
    pub stop: StopOutcome,
}

impl JobError {
    /// La fuente pudo quedar habilitada (no se confirmó el apagado).
    pub fn laser_may_be_on(&self) -> bool {
        self.stop == StopOutcome::Unconfirmed
    }
}

impl fmt::Display for JobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (tandas enviadas: {} de {})",
            self.cause, self.batches_sent, self.batches_total
        )?;
        match self.stop {
            StopOutcome::NotNeeded => write!(f, ". No se tocó la tarjeta"),
            StopOutcome::Acked => write!(f, ". Se apagó la fuente y se vació la lista"),
            StopOutcome::Unconfirmed => write!(
                f,
                ". Se intentó apagar la fuente pero NO hubo confirmación: puede seguir \
                 habilitada. Abre la tapa o usa el paro de emergencia"
            ),
        }
    }
}

impl std::error::Error for JobError {}

#[derive(Debug, Clone, PartialEq)]
pub struct MarkReport {
    pub batches: usize,
    pub commands: usize,
    pub bytes: usize,
    /// `false` en una prueba en rojo: la fuente nunca se armó.
    pub armed: bool,
    pub elapsed: Duration,
    pub stats: ListStats,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LightReport {
    pub loops: u32,
    /// Terminó porque se pidió detener (y no por el máximo de vueltas).
    pub stopped: bool,
}

/// Qué contador de tandas llevan los errores.
#[derive(Default)]
struct Counters {
    sent: usize,
    total: usize,
}

pub struct GalvoSession<T: Transport> {
    transport: Option<T>,
    cfg: SessionConfig,
    control: JobControl,
    /// Se mandó algo que exige apagar al terminar.
    dirty: bool,
    /// Se cree que la fuente está armada (se marca antes de enviar, se borra al confirmar).
    armed: bool,
    arming_allowed: bool,
    counters: Counters,
}

impl<T: Transport> GalvoSession<T> {
    /// Crea la sesión. Rechaza el transporte si su VID:PID no es una tarjeta compatible; no
    /// envía nada.
    pub fn new(transport: T, cfg: SessionConfig) -> Result<Self, SessionError> {
        cfg.limits.validate()?;
        let info = transport.info();
        match transport::identify(info.vendor_id, info.product_id) {
            Some(k) if k.support == Support::Supported => {}
            Some(k) => {
                return Err(SessionError::UnknownBoard {
                    vendor_id: info.vendor_id,
                    product_id: info.product_id,
                    why: match k.support {
                        Support::Rejected(w) => w.to_string(),
                        Support::Supported => String::new(),
                    },
                });
            }
            None => {
                return Err(SessionError::UnknownBoard {
                    vendor_id: info.vendor_id,
                    product_id: info.product_id,
                    why: "VID:PID desconocido".into(),
                });
            }
        }
        Ok(GalvoSession {
            transport: Some(transport),
            cfg,
            control: JobControl::new(),
            dirty: false,
            armed: false,
            arming_allowed: false,
            counters: Counters::default(),
        })
    }

    pub fn config(&self) -> &SessionConfig {
        &self.cfg
    }

    /// Mando para detener un trabajo desde otro hilo.
    pub fn control(&self) -> JobControl {
        self.control.clone()
    }

    /// Apaga (si hace falta) y devuelve el transporte.
    pub fn into_transport(mut self) -> T {
        if self.dirty || self.armed {
            let _ = self.safe_off();
        }
        self.transport
            .take()
            .expect("el transporte solo se retira aquí")
    }

    // ------------------------------------------------------------------------------------
    // Envío

    fn t(&mut self) -> Result<&mut T, SessionError> {
        self.transport
            .as_mut()
            .ok_or(SessionError::Transport(TransportError::Disconnected))
    }

    /// Único camino de los inmediatos: lista blanca, estado de armado y respuesta.
    fn send(&mut self, imm: Imm) -> Result<[u16; 4], SessionError> {
        let ctx = ImmContext {
            arming_allowed: self.arming_allowed,
            armed: self.armed,
        };
        safety::check_immediate(&imm, &self.cfg.limits, ctx)?;
        if !matches!(
            imm,
            Imm::GetVersion
                | Imm::GetSerialNo
                | Imm::GetListStatus
                | Imm::GetPositionXY
                | Imm::ReadPort
        ) {
            self.dirty = true;
        }
        if matches!(imm, Imm::EnableLaser | Imm::FiberSetMo(1))
            || matches!(imm, Imm::WritePort(v) if v == self.cfg.limits.laser_mask())
        {
            // Se anota antes de enviar: si falla a medias, se asume armada.
            self.armed = true;
        }
        Ok(transport::exchange(self.t()?, &imm)?)
    }

    fn write_batch(&mut self, bytes: &[u8]) -> Result<(), SessionError> {
        debug_assert_eq!(bytes.len(), wire::BATCH_BYTES);
        self.dirty = true;
        Ok(self.t()?.write(bytes)?)
    }

    /// Consulta el estado (reintenta ante tiempos agotados: es una lectura).
    pub fn status(&mut self) -> Result<BoardStatus, SessionError> {
        let mut tries = 0;
        loop {
            match self.send(Imm::GetVersion) {
                Ok(w) => return Ok(BoardStatus::from_word(w[3])),
                Err(SessionError::Transport(TransportError::Timeout))
                    if tries < self.cfg.status_retries =>
                {
                    tries += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Sondeo de solo lectura: serie, versión, estado y entradas. No mueve nada ni enciende nada.
    pub fn probe(&mut self) -> Result<BoardInfo, SessionError> {
        let serial = self.send(Imm::GetSerialNo)?;
        let version = self.send(Imm::GetVersion)?;
        let inputs = self.read_inputs()?;
        let transport = self.t()?.info();
        let mut notes = vec![
            "EXPERIMENTAL: el protocolo no está verificado con hardware propio.".to_string(),
            "Tlaku no inicializa la tarjeta ni escribe su tabla de corrección: la corrección de \
             campo se hace en el anfitrión y supone la tabla de la tarjeta en identidad."
                .to_string(),
            "Los bits de EMSTOP y alarmas de la fuente no están documentados: el software no \
             puede comprobar el enclavamiento salvo con --require-input."
                .to_string(),
        ];
        if transport.emulated {
            notes.push("Tarjeta SIMULADA: no hay hardware detrás.".into());
        }
        Ok(BoardInfo {
            board_name: transport::identify(transport.vendor_id, transport.product_id)
                .map_or_else(|| "desconocida".into(), |k| k.name.to_string()),
            transport,
            serial,
            version,
            status: BoardStatus::from_word(version[3]),
            inputs,
            notes,
        })
    }

    /// Lee las entradas (palabra 1 de `ReadPort`). Solo lectura.
    pub fn read_inputs(&mut self) -> Result<u16, SessionError> {
        Ok(self.send(Imm::ReadPort)?[1])
    }

    fn check_inputs(&mut self) -> Result<(), SessionError> {
        if let Some(c) = self.cfg.inputs_required {
            let inputs = self.read_inputs()?;
            if inputs & c.mask != c.value {
                return Err(SessionError::InterlockOpen {
                    inputs,
                    mask: c.mask,
                    value: c.value,
                });
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------------------------------
    // Apagado

    /// Secuencia de apagado completa, con reintentos. Se intenta **cada paso** aunque falle uno.
    pub fn safe_off(&mut self) -> StopOutcome {
        if self.transport.is_none() {
            return StopOutcome::Unconfirmed;
        }
        self.arming_allowed = false;
        let mut all_ok = true;
        for imm in safety::shutdown_sequence(self.cfg.device.source) {
            let mut ok = false;
            for _ in 0..=self.cfg.shutdown_retries {
                let ctx = ImmContext {
                    arming_allowed: false,
                    armed: self.armed,
                };
                if safety::check_immediate(&imm, &self.cfg.limits, ctx).is_err() {
                    break;
                }
                let Some(t) = self.transport.as_mut() else {
                    break;
                };
                match transport::exchange(t, &imm) {
                    Ok(_) => {
                        ok = true;
                        break;
                    }
                    // Sin USB no hay nada más que intentar con este paso.
                    Err(TransportError::Disconnected) => break,
                    Err(_) => {}
                }
            }
            all_ok &= ok;
        }
        self.control.set_state(JobState::Idle);
        if all_ok {
            self.armed = false;
            self.dirty = false;
            StopOutcome::Acked
        } else {
            StopOutcome::Unconfirmed
        }
    }

    /// Detiene todo y apaga la fuente. No exige ninguna precondición.
    pub fn stop(&mut self) -> StopOutcome {
        self.safe_off()
    }

    // ------------------------------------------------------------------------------------
    // Esperas

    fn wait_status(
        &mut self,
        ctl: &JobControl,
        timeout: Duration,
        what: &'static str,
        done: impl Fn(&BoardStatus) -> bool,
    ) -> Result<BoardStatus, SessionError> {
        let start = Instant::now();
        loop {
            if ctl.stop_requested() {
                return Err(SessionError::Cancelled);
            }
            let st = self.status()?;
            if done(&st) {
                return Ok(st);
            }
            if start.elapsed() > timeout {
                return Err(SessionError::Timeout { what });
            }
            if !self.cfg.poll_interval.is_zero() {
                std::thread::sleep(self.cfg.poll_interval);
            }
        }
    }

    fn wait_ready(&mut self, ctl: &JobControl) -> Result<(), SessionError> {
        let t = self.cfg.ready_timeout;
        self.wait_status(ctl, t, "la tarjeta lista para otra tanda", |s| s.ready)
            .map(|_| ())
    }

    fn wait_idle(&mut self, ctl: &JobControl, est_s: f64) -> Result<(), SessionError> {
        let t = Duration::try_from_secs_f64(est_s * 3.0 + 30.0).unwrap_or(Duration::from_secs(600));
        self.wait_status(ctl, t, "el fin de la ejecución", |s| !s.busy)
            .map(|_| ())
    }

    // ------------------------------------------------------------------------------------
    // Trabajos

    /// Compila un `Toolpath` con el perfil de la sesión. `dry_run`: lista para una prueba en
    /// rojo (láser deshabilitado, puntero rojo encendido).
    pub fn compile(&self, tp: &Toolpath, dry_run: bool) -> Result<CompiledList, SessionError> {
        let opts = CompileOptions {
            origin: self.cfg.origin,
            kind: if dry_run {
                ListKind::DryRun
            } else {
                ListKind::Mark
            },
            mo_delay_ms: self.cfg.mo_delay_ms,
            max_commands: self.cfg.limits.max_commands,
            ..CompileOptions::default()
        };
        Ok(compile::compile(tp, &self.cfg.device, &opts)?)
    }

    fn early(&self, cause: SessionError) -> Box<JobError> {
        Box::new(JobError {
            cause,
            batches_sent: 0,
            batches_total: 0,
            stop: StopOutcome::NotNeeded,
        })
    }

    fn finish(
        &mut self,
        result: Result<(), SessionError>,
        ok_stop: bool,
    ) -> Result<(), Box<JobError>> {
        let outcome = self.safe_off();
        let sent = self.counters.sent;
        let total = self.counters.total;
        let fail = |cause| {
            Err(Box::new(JobError {
                cause,
                batches_sent: sent,
                batches_total: total,
                stop: outcome,
            }))
        };
        match result {
            Err(SessionError::Cancelled) if ok_stop => {
                if outcome == StopOutcome::Unconfirmed {
                    fail(SessionError::ShutdownUnconfirmed)
                } else {
                    Ok(())
                }
            }
            Err(e) => fail(e),
            Ok(()) if outcome == StopOutcome::Unconfirmed => {
                fail(SessionError::ShutdownUnconfirmed)
            }
            Ok(()) => Ok(()),
        }
    }

    /// Envía una lista compilada y espera a que termine. `progress` se llama tras cada tanda.
    /// `ctl.request_stop()` cancela en la siguiente espera. Termina siempre con el apagado.
    pub fn send_job(
        &mut self,
        job: &CompiledList,
        ctl: &JobControl,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<MarkReport, Box<JobError>> {
        if job.kind == ListKind::Light {
            return Err(self.early(
                SafetyError::ImmediateRefused {
                    name: "send_job",
                    why: "un encuadre se manda con frame o light",
                }
                .into(),
            ));
        }
        if let Err(e) = safety::validate_list(&job.commands, job.kind, &self.cfg.limits) {
            return Err(self.early(e.into()));
        }
        self.counters = Counters::default();
        ctl.set_state(JobState::Running);
        let start = Instant::now();
        let result = self.run_mark(job, ctl, progress);
        ctl.set_state(JobState::Idle);
        let (batches, bytes) = (self.counters.total, self.counters.total * wire::BATCH_BYTES);
        self.finish(result, false)?;
        Ok(MarkReport {
            batches,
            commands: job.commands.len(),
            bytes,
            armed: job.kind == ListKind::Mark,
            elapsed: start.elapsed(),
            stats: job.stats.clone(),
            warnings: job.warnings.clone(),
        })
    }

    /// Compila y envía un `Toolpath`.
    pub fn mark(
        &mut self,
        tp: &Toolpath,
        dry_run: bool,
        ctl: &JobControl,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<MarkReport, Box<JobError>> {
        let job = self.compile(tp, dry_run).map_err(|e| self.early(e))?;
        self.send_job(&job, ctl, progress)
    }

    fn preflight(&mut self) -> Result<(), SessionError> {
        if self.status()?.busy {
            return Err(SessionError::Busy);
        }
        self.check_inputs()
    }

    fn upload_and_run(
        &mut self,
        batches: &[Vec<u8>],
        arming: bool,
        ctl: &JobControl,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<(), SessionError> {
        let total_bytes = batches.len() * wire::BATCH_BYTES;
        let mut executing = false;
        for (i, b) in batches.iter().enumerate() {
            self.wait_ready(ctl)?;
            self.write_batch(b)?;
            self.counters.sent = i + 1;
            progress(Progress {
                batch: i + 1,
                batches: batches.len(),
                bytes_sent: (i + 1) * wire::BATCH_BYTES,
                bytes_total: total_bytes,
            });
            self.send(Imm::SetEndOfList(0))?;
            if !executing && i + 1 >= self.cfg.execute_after_batches {
                self.start(arming)?;
                executing = true;
            }
        }
        if !executing {
            self.start(arming)?;
        }
        Ok(())
    }

    /// Arma la fuente (solo en un marcado real) y ejecuta la lista.
    fn start(&mut self, arming: bool) -> Result<(), SessionError> {
        if arming {
            self.arming_allowed = true;
            self.send(Imm::EnableLaser)?;
            self.send(Imm::WritePort(self.cfg.limits.laser_mask()))?;
            if params::is_fiber(self.cfg.device.source) {
                self.send(Imm::FiberSetMo(1))?;
            }
        }
        self.send(Imm::ExecuteList)?;
        Ok(())
    }

    fn run_mark(
        &mut self,
        job: &CompiledList,
        ctl: &JobControl,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<(), SessionError> {
        self.preflight()?;
        // Arranque siempre desarmado y con la lista vacía.
        self.send(Imm::DisableLaser)?;
        self.send(Imm::ResetList)?;
        self.send(Imm::WritePort(0))?;
        let batches = job.batches();
        self.counters.total = batches.len();
        self.upload_and_run(&batches, job.kind == ListKind::Mark, ctl, progress)?;
        self.wait_idle(ctl, job.stats.est_seconds)
    }

    /// Dibuja `shape` con el puntero rojo en bucle hasta que `ctl` pida detener o se alcance
    /// `max_loops`. Nunca habilita el láser. `progress` se llama tras cada vuelta.
    pub fn light(
        &mut self,
        shape: &LightShape,
        ctl: &JobControl,
        max_loops: Option<u32>,
        progress: &mut dyn FnMut(LightProgress),
    ) -> Result<LightReport, Box<JobError>> {
        let list = compile::compile_light(
            shape,
            &self.cfg.device,
            &LightOptions {
                origin: self.cfg.origin,
            },
        )
        .map_err(|e| self.early(e.into()))?;
        if let Err(e) = safety::validate_list(&list.commands, ListKind::Light, &self.cfg.limits) {
            return Err(self.early(e.into()));
        }
        self.counters = Counters::default();
        ctl.set_state(JobState::Running);
        let mut report = LightReport {
            loops: 0,
            stopped: false,
        };
        let result = self.run_light(&list, ctl, max_loops, &mut report, progress);
        ctl.set_state(JobState::Idle);
        // Detener es la forma normal de terminar un encuadre en bucle.
        if matches!(result, Err(SessionError::Cancelled)) {
            report.stopped = true;
        }
        self.finish(result, true)?;
        Ok(report)
    }

    /// Encuadre de un rectángulo (mm del sistema de la sesión).
    pub fn frame(
        &mut self,
        min: Point,
        max: Point,
        ctl: &JobControl,
        max_loops: Option<u32>,
        progress: &mut dyn FnMut(LightProgress),
    ) -> Result<LightReport, Box<JobError>> {
        self.light(&LightShape::Rect { min, max }, ctl, max_loops, progress)
    }

    fn run_light(
        &mut self,
        list: &CompiledList,
        ctl: &JobControl,
        max_loops: Option<u32>,
        report: &mut LightReport,
        progress: &mut dyn FnMut(LightProgress),
    ) -> Result<(), SessionError> {
        self.preflight()?;
        self.send(Imm::DisableLaser)?;
        if params::is_fiber(self.cfg.device.source) {
            self.send(Imm::FiberSetMo(0))?;
        }
        self.send(Imm::WritePort(0))?;
        let batches = list.batches();
        self.counters.total = batches.len();
        loop {
            self.send(Imm::ResetList)?;
            let mut noop = |_p: Progress| {};
            self.upload_and_run(&batches, false, ctl, &mut noop)?;
            self.wait_idle(ctl, list.stats.est_seconds)?;
            report.loops += 1;
            progress(LightProgress {
                loops: report.loops,
            });
            if ctl.stop_requested() {
                report.stopped = true;
                return Ok(());
            }
            if max_loops.is_some_and(|m| report.loops >= m) {
                return Ok(());
            }
        }
    }

    /// Enciende o apaga el puntero rojo en su sitio (con el láser deshabilitado).
    pub fn set_red_light(&mut self, on: bool) -> Result<(), Box<JobError>> {
        let result = (|| {
            self.send(Imm::DisableLaser)?;
            if params::is_fiber(self.cfg.device.source) {
                self.send(Imm::FiberSetMo(0))?;
            }
            self.send(Imm::WritePort(if on {
                self.cfg.limits.light_mask()
            } else {
                0
            }))?;
            Ok(())
        })();
        if result.is_err() || !on {
            self.finish(result, false)
        } else {
            // El puntero queda encendido a propósito; `stop` o `Drop` lo apagan.
            Ok(())
        }
    }

    /// Mueve los espejos a `target` (mm) con la fuente desarmada (`GotoXY`).
    pub fn jog_to(&mut self, target: Point) -> Result<(), Box<JobError>> {
        let map = FieldMap::new(self.cfg.device.field_mm, &self.cfg.device.correction)
            .map_err(|e| self.early(SessionError::Compile(e.into())))?;
        let p = self
            .cfg
            .origin
            .to_centered(target, self.cfg.device.field_mm);
        let (x, y) = map
            .to_units(p)
            .map_err(|e| self.early(SessionError::Compile(e.into())))?;
        let result = (|| {
            self.preflight()?;
            self.send(Imm::GotoXY { x, y, dist: 0 })?;
            Ok(())
        })();
        if result.is_err() {
            return self.finish(result, false);
        }
        Ok(())
    }
}

impl<T: Transport> Drop for GalvoSession<T> {
    fn drop(&mut self) {
        if self.dirty || self.armed {
            let _ = self.safe_off();
        }
    }
}
