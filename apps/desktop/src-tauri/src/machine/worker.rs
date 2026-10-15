//! El trabajador de la conexión GRBL: dueño del puerto serie. El bucle (`run`) y los tipos comunes
//! viven aquí; cada fase del `impl Worker` está en un submódulo por tema.

mod command;
mod idle;
mod jog;
pub(in crate::machine) mod limits;
mod probe;
mod start;
mod status;
mod stream;

use super::*;
use tlaku_i18n::tr;

pub(super) fn log(sink: &dyn Sink, dir: LogDir, text: &str) {
    sink.emit(Emit::Log(LogEvent {
        dir,
        text: text.to_string(),
    }));
}

/// `error:N`, `ALARM:N` o `ERROR:NN` de Creality con su explicación; el resto, tal cual.
pub(super) fn describe_line(fw: Firmware, line: &str) -> String {
    if let Some(code) = line
        .strip_prefix("error:")
        .and_then(|c| c.trim().parse().ok())
    {
        format!("{line}: {}", error_message_for(fw, code))
    } else if let Some(code) = line
        .strip_prefix("ALARM:")
        .and_then(|c| c.trim().parse().ok())
    {
        format!("{line}: {}", alarm_message_for(fw, code))
    } else if let Some(code) = parse_creality_error(line) {
        // Normalizado (`ERROR:06.` -> `ERROR:06`) para que la interfaz lo reconozca.
        format!("ERROR:{code:02}: {}", creality_error_message(code))
    } else {
        line.to_string()
    }
}

/// `ALARM:1`/`ALARM:2` y `[MSG:Reset to continue]`: GRBL queda bloqueado y puede dejar de
/// contestar `?` hasta un soft reset.
pub(super) fn is_critical_alarm(line: &str) -> bool {
    matches!(
        line.strip_prefix("ALARM:")
            .and_then(|c| c.trim().parse::<u32>().ok()),
        Some(1 | 2)
    ) || line.contains("Reset to continue")
}

/// Cada cuánto se pide `M114` a una controladora sin `?` (Marlin, Smoothieware) en reposo.
pub(super) const REPRAP_POLL: Duration = Duration::from_millis(500);

/// Cada cuánto se manda `?` durante un bloqueo crítico (que probablemente no lo contesta).
pub(super) const CRITICAL_POLL: Duration = Duration::from_secs(2);

/// Por qué se tolera silencio (ver la regla del silencio en la cabecera del módulo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Quiet {
    /// Hay un `$H` sin responder: plazo `home_timeout`.
    Homing,
    /// Bloqueo tras `ALARM:1`/`ALARM:2`: sin plazo hasta un `ok` o el saludo.
    CriticalAlarm,
}

// ---------------------------------------------------------------------------
// Trabajador
// ---------------------------------------------------------------------------

/// Fallo de una operación: `Lost` termina la conexión; `Msg` solo se informa al que llamó.
#[derive(Debug)]
pub(super) enum Fail {
    Msg(String),
    Lost(String),
}

impl From<io::Error> for Fail {
    fn from(e: io::Error) -> Self {
        Self::Lost(tr!("desktop.worker.connection_lost_io", err = e))
    }
}

/// Un trabajo (o encuadre) en marcha.
pub(super) struct Run {
    job: Job<Box<dyn Transport>>,
    kind: RunKind,
    last_progress: Option<Instant>,
    cancelling: bool,
    /// El punto de control del trabajo (solo trabajos, no encuadres).
    checkpoint: Option<CheckpointRun>,
}

/// El escritor del punto de control y su carpeta.
pub(super) struct CheckpointRun {
    writer: resume::CheckpointWriter,
    store: Arc<resume::store::CheckpointStore>,
}

pub(super) enum Phase {
    /// El trabajador tiene el puerto.
    Idle(Box<dyn Transport>),
    /// El puerto está en manos del `Streamer`.
    Streaming(Run),
    /// Desconexión pedida.
    Done,
}

pub(super) struct Worker {
    rx: Receiver<Request>,
    pending: VecDeque<Request>,
    sink: Arc<dyn Sink>,
    shared: Arc<Shared>,
    timing: Timing,
    port: String,
    pub(super) reader: LineReader,
    last_poll: Option<Instant>,
    last_rx: Instant,
    /// Desplazamiento de coordenadas de trabajo (`WCO`), para convertir MPos <-> WPos.
    wco: [f64; 2],
    /// Cuántos reportes de estado se han leído (la sonda espera uno fresco antes de mover).
    statuses: u64,
    last: StatusReport,
    /// Familia del firmware, para leer los códigos extendidos.
    pub(super) fw: Firmware,
    /// Familia del protocolo: GRBL (`?`, tiempo real) o RepRap (Marlin, Smoothieware: una línea
    /// por `ok`, sin `?`; la posición se consulta con `M114`).
    pub(super) link_family: LinkFamily,
    /// El Marlin conectado informó `EMERGENCY_PARSER` (pausa con `M0` y `M108`).
    pub(super) emergency_parser: bool,
    /// Silencio tolerado ahora mismo (ver la cabecera del módulo).
    quiet: Option<Quiet>,
    /// Guardas de la cama y del reinicio de la placa (ver `limits.rs`).
    lim: limits::Limits,
}

pub(super) fn empty_status() -> StatusReport {
    StatusReport {
        state: MachineState::Idle,
        other_state: None,
        machine_pos: [0.0, 0.0],
        work_pos: [0.0, 0.0],
        feed: 0.0,
        spindle: 0.0,
        // GRBL con la cola vacía: 15 bloques libres y el búfer serie (127 bytes) sin usar.
        buffer: BufferStatus {
            planner: 15,
            rx: 127,
        },
        overrides: None,
        position_unconfirmed: None,
    }
}

pub(super) fn convert_state(s: streamer::MachineState) -> MachineState {
    use streamer::MachineState as G;
    match s {
        G::Idle => MachineState::Idle,
        G::Run => MachineState::Run,
        G::Hold(_) => MachineState::Hold,
        G::Jog => MachineState::Jog,
        G::Alarm => MachineState::Alarm,
        G::Door(_) => MachineState::Door,
        G::Check => MachineState::Check,
        G::Home => MachineState::Home,
        G::Sleep => MachineState::Sleep,
        // Estado que Tlaku no conoce (p. ej. `Tool` de grblHAL): ocupada, no en reposo. El
        // nombre original viaja aparte (`other_state`) para que la interfaz lo muestre.
        G::Other => MachineState::Other,
    }
}

impl Worker {
    pub(super) fn new(
        rx: Receiver<Request>,
        sink: Arc<dyn Sink>,
        shared: Arc<Shared>,
        timing: Timing,
        port: String,
    ) -> Self {
        Self {
            rx,
            pending: VecDeque::new(),
            sink,
            shared,
            timing,
            port,
            reader: LineReader::grbl(),
            last_poll: None,
            last_rx: Instant::now(),
            wco: [0.0, 0.0],
            statuses: 0,
            last: empty_status(),
            fw: Firmware::Unknown,
            link_family: LinkFamily::Grbl,
            emergency_parser: false,
            quiet: None,
            lim: limits::Limits::default(),
        }
    }

    pub(super) fn run(mut self, transport: Box<dyn Transport>) {
        let mut phase = Phase::Idle(transport);
        let lost = loop {
            let next = match phase {
                Phase::Idle(t) => self.idle_step(t),
                Phase::Streaming(run) => self.stream_step(run),
                Phase::Done => break None,
            };
            match next {
                Ok(p) => phase = p,
                Err(Fail::Lost(msg) | Fail::Msg(msg)) => break Some(msg),
            }
        };
        self.shared.dead.store(true, Ordering::SeqCst);
        self.sink.emit(Emit::Connection(ConnectionEvent {
            connected: false,
            port: Some(self.port.clone()),
            error: lost,
        }));
    }

    fn log(&self, dir: LogDir, text: &str) {
        log(self.sink.as_ref(), dir, text);
    }

    fn job_event(&self, state: JobState, message: &str) {
        self.job_event_at(state, message, false);
    }

    /// Como [`job_event`](Self::job_event), indicando si la posición puede haberse perdido.
    fn job_event_at(&self, state: JobState, message: &str, position_lost: bool) {
        self.sink.emit(Emit::Job(JobEvent {
            state,
            message: message.to_string(),
            position_lost,
        }));
    }

    fn next_request(&mut self) -> Result<Option<Request>, Fail> {
        if let Some(r) = self.pending.pop_front() {
            return Ok(Some(r));
        }
        match self.rx.try_recv() {
            Ok(r) => Ok(Some(r)),
            Err(TryRecvError::Empty) => Ok(None),
            // Se soltó el mando sin pedir la desconexión: se cierra sin error.
            Err(TryRecvError::Disconnected) => Ok(Some(Request::Disconnect)),
        }
    }
}

pub(super) enum StartFail {
    /// No se transmitió nada; el puerto se devuelve.
    Refused(Box<dyn Transport>, String),
    Lost(String),
}

pub(super) fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or(s).to_string()
}

/// Número para G-code: hasta 3 decimales, sin ceros sobrantes ni `-0`.
pub(super) fn num(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s.into()
    }
}

pub(super) fn jog_error(reply: &str) -> String {
    if reply.starts_with("error:9") || reply.starts_with("ALARM") {
        tr!("desktop.worker.jog_rejected_alarm", reply = reply)
    } else {
        tr!("desktop.worker.jog_rejected", reply = reply)
    }
}

/// Valida el G-code contra el búfer de la controladora antes de tocar la máquina.
pub(super) fn validate_lines(lines: &[String], link: &LinkProfile) -> Result<(), String> {
    validate_gcode_for(&lines.join("\n"), link).map(drop)
}
