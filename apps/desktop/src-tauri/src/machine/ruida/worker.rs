//! El hilo dueño de la sesión Ruida: bucle (`run`) y textos de error. El estado, las solicitudes
//! y la subida viven en los submódulos.

mod requests;
mod status;
mod upload;

use super::*;
use tlaku_i18n::tr;

// ---------------------------------------------------------------------------
// Hilo
// ---------------------------------------------------------------------------

/// Un trabajo ejecutándose en la máquina (modo `Run`).
pub(super) struct ActiveJob {
    pub(super) since: Instant,
    /// Ya se vio la máquina en marcha.
    pub(super) seen_running: bool,
    pub(super) paused: bool,
}

/// Recorrido de un encuadre: esquinas pendientes (mm del sistema de la máquina).
pub(super) struct FrameRun {
    pub(super) corners: VecDeque<(f64, f64)>,
}

pub(super) struct Worker<L: PacketLink> {
    pub(super) session: RuidaSession<L>,
    pub(super) rx: Receiver<Request>,
    pub(super) sink: Arc<dyn Sink>,
    pub(super) shared: Arc<Shared>,
    pub(super) timing: RuidaTiming,
    pub(super) port: String,
    /// Cama que informó la máquina al conectar.
    pub(super) bed_mm: Option<[f64; 2]>,
    pub(super) job: Option<ActiveJob>,
    pub(super) frame: Option<FrameRun>,
    /// Sondeos seguidos sin respuesta.
    pub(super) failures: u32,
    pub(super) silent_logged: bool,
    pub(super) sim: Option<VirtualSim>,
}

impl<L: PacketLink> Worker<L> {
    pub(super) fn run(mut self) {
        let mut next_poll = Instant::now() + self.timing.status_interval;
        let lost = loop {
            let wait = next_poll.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(wait) {
                Ok(Request::Disconnect) | Err(RecvTimeoutError::Disconnected) => {
                    self.shutdown();
                    break None;
                }
                Ok(req) => self.handle(req),
                Err(RecvTimeoutError::Timeout) => {
                    if let Err(msg) = self.poll() {
                        break Some(msg);
                    }
                    let every = if self.frame.is_some() {
                        self.timing.frame_interval
                    } else {
                        self.timing.status_interval
                    };
                    next_poll = Instant::now() + every;
                }
            }
        };
        self.shared.dead.store(true, Ordering::SeqCst);
        self.sink.emit(Emit::Connection(ConnectionEvent {
            connected: false,
            port: Some(self.port.clone()),
            error: lost,
        }));
    }

    pub(super) fn log(&self, dir: LogDir, text: &str) {
        log(self.sink.as_ref(), dir, text);
    }

    pub(super) fn job_event(&self, state: JobState, message: &str) {
        self.sink.emit(Emit::Job(JobEvent {
            state,
            message: message.to_string(),
            position_lost: false,
        }));
    }

    /// Al desconectar, un trabajo en marcha se detiene (igual que con GRBL).
    pub(super) fn shutdown(&mut self) {
        if self.job.take().is_some() {
            self.log(LogDir::Out, &tr!("desktop.ruida.log_disconnect_stop"));
            match self.session.stop() {
                Ok(()) => self.job_event(
                    JobState::Cancelled,
                    &tr!("desktop.ruida.job_stopped_disconnect"),
                ),
                Err(e) => self.log(
                    LogDir::In,
                    &tr!("desktop.ruida.log_stop_unconfirmed", err = e),
                ),
            }
        }
    }
}

/// Texto de un fallo de subida antes del primer paquete: el de la causa con su pista.
pub(super) fn op_error_text(cause: &SessionError) -> String {
    // `UploadError` añade al texto de la causa el recuento de paquetes y el aviso sobre el
    // documento incompleto, que aquí no aplican: solo se usa la causa.
    match cause {
        SessionError::Timeout { .. } => tr!("desktop.ruida.upload_silent", err = cause),
        SessionError::Busy(_) => tr!("desktop.ruida.upload_busy", err = cause),
        _ => cause.to_string(),
    }
}

pub(super) fn idle_report() -> StatusReport {
    StatusReport {
        state: MachineState::Idle,
        other_state: None,
        machine_pos: [0.0, 0.0],
        work_pos: [0.0, 0.0],
        feed: 0.0,
        spindle: 0.0,
        // Ruida no informa búferes: ceros (la interfaz los oculta con Ruida).
        buffer: BufferStatus { planner: 0, rx: 0 },
        overrides: None,
    }
}

/// Información de la sesión a los tipos del contrato con la interfaz.
pub(super) fn to_info(i: ControllerInfo) -> RuidaInfo {
    let secs = |v: Option<u64>| v.map(|v| v as f64);
    RuidaInfo {
        model: i.probe.model.map(str::to_string),
        card_id: format!("0x{:08X}", i.probe.card_id),
        magic: i.probe.magic.0,
        bed: i.probe.bed_mm,
        rapid_mm_s: i.rapid_mm_s,
        home_mm_s: i.home_mm_s,
        axis_max_mm_s: i.axis_max_mm_s,
        axis_accel_mm_s2: i.axis_accel_mm_s2,
        total_on_s: secs(i.total_on_s),
        total_work_s: secs(i.total_work_s),
        total_laser_s: secs(i.total_laser_s),
        total_jobs: i.total_jobs.map(|v| u32::try_from(v).unwrap_or(u32::MAX)),
        travel_m: [secs(i.travel_m[0]), secs(i.travel_m[1])],
        documents: i.documents,
    }
}
