//! Conexión con la máquina GRBL.
//!
//! Un hilo dedicado ("trabajador") es el único dueño del puerto serie. Los comandos de la
//! interfaz llegan por un canal ([`Handle`]) y el trabajador:
//!
//! - en reposo, sondea `?` cada ~250 ms, emite `machine://status` y registra lo que llegue;
//! - al empezar un trabajo, comprueba el estado (misma lógica que `tlaku send`) y le
//!   entrega el puerto a un `Streamer` (conteo de caracteres) en su propio hilo; mientras tanto
//!   traduce sus eventos a progreso, estado y registro, y atiende pausa, reanudación y cancelación;
//! - al terminar el trabajo recupera el puerto y vuelve a sondear;
//! - si el puerto falla o la controladora calla, emite `machine://connection` con el error y termina.
//!
//! Los reportes de estado (`?`) no se registran en `machine://log`: solo llegan como estado.
//! Nada aquí depende de Tauri: los eventos salen por [`Sink`], de modo que se prueba con
//! `memory_pair` sin hardware.

use std::collections::VecDeque;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tlaku_doc::{
    BufferStatus, ConnectResult, ConnectionEvent, JobEvent, JobState, LogDir, LogEvent,
    MachineState, ProgressEvent, RealtimeCommand, StatusReport,
};
use tlaku_grbl::preflight::{Advice, PreflightConfig, outcome_result, preflight_with};
use tlaku_grbl::serial::{Grbl, LineReader, Transport};
use tlaku_grbl::streamer::{
    self, Event, Job, Outcome, Realtime, Streamer, StreamerConfig, alarm_message, error_message,
    prepare_line,
};

/// Mensaje cuando no hay conexión (o se perdió).
pub const NOT_CONNECTED: &str = "No hay máquina conectada. Conéctala desde la pestaña Máquina.";

// ---------------------------------------------------------------------------
// Salida de eventos
// ---------------------------------------------------------------------------

/// Evento hacia la interfaz (`machine://*`).
#[derive(Debug, Clone, PartialEq)]
pub enum Emit {
    Status(StatusReport),
    Progress(ProgressEvent),
    Log(LogEvent),
    Job(JobEvent),
    Connection(ConnectionEvent),
}

impl Emit {
    /// Nombre del evento de Tauri.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Status(_) => "machine://status",
            Self::Progress(_) => "machine://progress",
            Self::Log(_) => "machine://log",
            Self::Job(_) => "machine://job",
            Self::Connection(_) => "machine://connection",
        }
    }
}

/// Destino de los eventos. En la app emite a la ventana; en las pruebas los guarda.
pub trait Sink: Send + Sync + 'static {
    fn emit(&self, event: Emit);
}

// ---------------------------------------------------------------------------
// Tiempos
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Timing {
    /// Cada cuánto se consulta `?` en reposo.
    pub status_interval: Duration,
    /// Sin recibir nada durante este tiempo (con el sondeo activo) la conexión se da por perdida.
    pub silence_timeout: Duration,
    /// Espera de la respuesta a un comando normal.
    pub command_timeout: Duration,
    /// Espera de la respuesta a `$H` (GRBL contesta `ok` al terminar el homing).
    pub home_timeout: Duration,
    /// Espera de la respuesta a `$I` al conectar.
    pub identify_timeout: Duration,
    /// Lectura máxima por vuelta del ciclo: acota la latencia de los mandos.
    pub tick: Duration,
    /// Mínimo entre dos eventos de progreso (el último siempre sale).
    pub progress_interval: Duration,
    pub preflight: PreflightConfig,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            status_interval: Duration::from_millis(250),
            silence_timeout: Duration::from_secs(5),
            command_timeout: Duration::from_secs(5),
            home_timeout: Duration::from_secs(60),
            identify_timeout: Duration::from_secs(3),
            tick: Duration::from_millis(20),
            progress_interval: Duration::from_millis(100),
            preflight: PreflightConfig::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Estado compartido y solicitudes
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Shared {
    /// Falso cuando el trabajador terminó (desconexión pedida o perdida).
    dead: AtomicBool,
    /// Último reporte de estado.
    status: Mutex<Option<StatusReport>>,
}

type Reply = Sender<Result<Vec<String>, String>>;

/// Qué se transmite: el trabajo o su encuadre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunKind {
    Job,
    Frame,
}

enum Request {
    Command {
        line: String,
        reply: Reply,
    },
    Realtime {
        cmd: RealtimeCommand,
        reply: Reply,
    },
    Jog {
        dx: f64,
        dy: f64,
        speed: f64,
        reply: Reply,
    },
    Start {
        lines: Vec<String>,
        rx_buffer: usize,
        kind: RunKind,
        reply: Reply,
    },
    Pause(Reply),
    Resume(Reply),
    Cancel(Reply),
    Disconnect,
}

/// Mando de la conexión; se clona y se usa desde cualquier hilo. Las llamadas bloquean hasta que
/// el trabajador responde: desde Tauri se invocan dentro de `spawn_blocking`.
#[derive(Clone)]
pub struct Handle {
    tx: Sender<Request>,
    shared: Arc<Shared>,
}

impl Handle {
    fn call(&self, build: impl FnOnce(Reply) -> Request) -> Result<Vec<String>, String> {
        if self.is_closed() {
            return Err(NOT_CONNECTED.into());
        }
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(build(tx))
            .map_err(|_| NOT_CONNECTED.to_string())?;
        rx.recv()
            .map_err(|_| "Se perdió la conexión con la máquina.".to_string())?
    }

    /// `true` si la conexión terminó (a petición o por un fallo).
    pub fn is_closed(&self) -> bool {
        self.shared.dead.load(Ordering::SeqCst)
    }

    /// Posición de trabajo [x, y] del último reporte de estado.
    pub fn work_pos(&self) -> Option<[f64; 2]> {
        self.shared.status.lock().ok()?.as_ref().map(|s| s.work_pos)
    }

    /// Envía una línea y devuelve la respuesta, que termina en `ok`, `error:N` o `ALARM:N`.
    pub fn command(&self, line: &str) -> Result<Vec<String>, String> {
        self.call(|reply| Request::Command {
            line: line.into(),
            reply,
        })
    }

    pub fn realtime(&self, cmd: RealtimeCommand) -> Result<(), String> {
        self.call(|reply| Request::Realtime { cmd, reply })
            .map(drop)
    }

    /// Movimiento relativo en mm a `speed` mm/min.
    pub fn jog(&self, dx: f64, dy: f64, speed: f64) -> Result<(), String> {
        self.call(|reply| Request::Jog {
            dx,
            dy,
            speed,
            reply,
        })
        .map(drop)
    }

    /// Comprueba el estado de la máquina y, solo si está en reposo, transmite el G-code.
    pub fn start_job(&self, lines: Vec<String>, rx_buffer: usize) -> Result<(), String> {
        self.start(lines, rx_buffer, RunKind::Job)
    }

    /// Como [`start_job`](Self::start_job) para el encuadre (sin eventos de progreso).
    pub fn frame(&self, lines: Vec<String>, rx_buffer: usize) -> Result<(), String> {
        self.start(lines, rx_buffer, RunKind::Frame)
    }

    fn start(&self, lines: Vec<String>, rx_buffer: usize, kind: RunKind) -> Result<(), String> {
        self.call(|reply| Request::Start {
            lines,
            rx_buffer,
            kind,
            reply,
        })
        .map(drop)
    }

    pub fn pause(&self) -> Result<(), String> {
        self.call(Request::Pause).map(drop)
    }

    pub fn resume(&self) -> Result<(), String> {
        self.call(Request::Resume).map(drop)
    }

    /// Cancela el trabajo (feed hold y soft reset). Sin trabajo, cancela un jog (0x85).
    pub fn cancel(&self) -> Result<(), String> {
        self.call(Request::Cancel).map(drop)
    }
}

/// Conexión abierta: el mando y el hilo del trabajador.
pub struct Connection {
    handle: Handle,
    thread: Option<JoinHandle<()>>,
}

impl Connection {
    /// Identifica la controladora con `$I` y arranca el trabajador. `transport` es un puerto
    /// ya abierto (con la espera de reinicio de la placa cumplida).
    pub fn start(
        transport: Box<dyn Transport>,
        port: &str,
        sink: Arc<dyn Sink>,
        timing: Timing,
    ) -> Result<(Self, ConnectResult), String> {
        let mut grbl = Grbl::with_transport(transport);
        let reply = grbl.command("$I", timing.identify_timeout);
        log(sink.as_ref(), LogDir::Out, "$I");
        let lines = reply.map_err(|e| {
            format!(
                "La máquina no respondió a la identificación ($I): {e}. Revisa que sea el puerto \
                 correcto, que esté encendida, que use GRBL 1.1 y los baudios del perfil."
            )
        })?;
        for l in &lines {
            log(sink.as_ref(), LogDir::In, l);
        }
        log(sink.as_ref(), LogDir::In, "ok");
        let firmware = firmware_name(&lines);

        let shared = Arc::new(Shared::default());
        let (tx, rx) = mpsc::channel();
        let worker = Worker::new(rx, sink.clone(), shared.clone(), timing, port.to_string());
        let transport = grbl.into_transport();
        sink.emit(Emit::Connection(ConnectionEvent {
            connected: true,
            port: Some(port.to_string()),
            error: None,
        }));
        let thread = thread::Builder::new()
            .name("tlaku-machine".into())
            .spawn(move || worker.run(transport))
            .map_err(|e| format!("No se pudo iniciar el hilo de la conexión: {e}"))?;
        Ok((
            Self {
                handle: Handle { tx, shared },
                thread: Some(thread),
            },
            ConnectResult { firmware },
        ))
    }

    pub fn handle(&self) -> Handle {
        self.handle.clone()
    }

    /// Cierra la conexión (cancelando antes un trabajo en curso) y espera al hilo.
    pub fn close(mut self) {
        let _ = self.handle.tx.send(Request::Disconnect);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // Si no se llamó a `close`, al menos se le pide al trabajador que termine.
        let _ = self.handle.tx.send(Request::Disconnect);
    }
}

/// `Grbl 1.1h` a partir de la línea `[VER:1.1h.20190825:]` de `$I`.
fn firmware_name(lines: &[String]) -> String {
    let Some(ver) = lines.iter().find_map(|l| l.trim().strip_prefix("[VER:")) else {
        return "GRBL".into();
    };
    let ver = ver.trim_end_matches(']').split(':').next().unwrap_or("");
    let ver = match ver.rsplit_once('.') {
        Some((head, build)) if build.len() == 8 && build.bytes().all(|b| b.is_ascii_digit()) => {
            head
        }
        _ => ver,
    };
    if ver.is_empty() {
        "GRBL".into()
    } else {
        format!("Grbl {ver}")
    }
}

fn log(sink: &dyn Sink, dir: LogDir, text: &str) {
    sink.emit(Emit::Log(LogEvent {
        dir,
        text: text.to_string(),
    }));
}

/// `error:N` o `ALARM:N` con su explicación; el resto, tal cual.
fn describe_line(line: &str) -> String {
    if let Some(code) = line
        .strip_prefix("error:")
        .and_then(|c| c.trim().parse().ok())
    {
        format!("{line}: {}", error_message(code))
    } else if let Some(code) = line
        .strip_prefix("ALARM:")
        .and_then(|c| c.trim().parse().ok())
    {
        format!("{line}: {}", alarm_message(code))
    } else {
        line.to_string()
    }
}

// ---------------------------------------------------------------------------
// Trabajador
// ---------------------------------------------------------------------------

/// Fallo de una operación: `Lost` termina la conexión; `Msg` solo se informa al que llamó.
#[derive(Debug)]
enum Fail {
    Msg(String),
    Lost(String),
}

impl From<io::Error> for Fail {
    fn from(e: io::Error) -> Self {
        Self::Lost(format!("se perdió la conexión con la máquina: {e}"))
    }
}

/// Un trabajo (o encuadre) en marcha.
struct Run {
    job: Job<Box<dyn Transport>>,
    kind: RunKind,
    last_progress: Option<Instant>,
    cancelling: bool,
}

enum Phase {
    /// El trabajador tiene el puerto.
    Idle(Box<dyn Transport>),
    /// El puerto está en manos del `Streamer`.
    Streaming(Run),
    /// Desconexión pedida.
    Done,
}

struct Worker {
    rx: Receiver<Request>,
    pending: VecDeque<Request>,
    sink: Arc<dyn Sink>,
    shared: Arc<Shared>,
    timing: Timing,
    port: String,
    reader: LineReader,
    last_poll: Option<Instant>,
    last_rx: Instant,
    /// Desplazamiento de coordenadas de trabajo (`WCO`), para convertir MPos <-> WPos.
    wco: [f64; 2],
    last: StatusReport,
}

fn empty_status() -> StatusReport {
    StatusReport {
        state: MachineState::Idle,
        machine_pos: [0.0, 0.0],
        work_pos: [0.0, 0.0],
        feed: 0.0,
        spindle: 0.0,
        // GRBL con la cola vacía: 15 bloques libres y el búfer serie (127 bytes) sin usar.
        buffer: BufferStatus {
            planner: 15,
            rx: 127,
        },
    }
}

fn convert_state(s: streamer::MachineState) -> MachineState {
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
    }
}

impl Worker {
    fn new(
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
            reader: LineReader::default(),
            last_poll: None,
            last_rx: Instant::now(),
            wco: [0.0, 0.0],
            last: empty_status(),
        }
    }

    fn run(mut self, transport: Box<dyn Transport>) {
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
        self.sink.emit(Emit::Job(JobEvent {
            state,
            message: message.to_string(),
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

    // ---- estado ------------------------------------------------------------

    fn on_status(&mut self, r: &streamer::StatusReport) {
        if let Some(w) = &r.wco {
            self.wco = [w.x, w.y];
        }
        let mut s = self.last.clone();
        s.state = convert_state(r.state);
        match (&r.mpos, &r.wpos) {
            (Some(m), _) => {
                s.machine_pos = [m.x, m.y];
                s.work_pos = match &r.wpos {
                    Some(w) => [w.x, w.y],
                    None => [m.x - self.wco[0], m.y - self.wco[1]],
                };
            }
            (None, Some(w)) => {
                s.work_pos = [w.x, w.y];
                s.machine_pos = [w.x + self.wco[0], w.y + self.wco[1]];
            }
            (None, None) => {}
        }
        s.feed = r.feed.unwrap_or(0.0);
        s.spindle = r.spindle.unwrap_or(0.0);
        if let Some(b) = r.buffer {
            s.buffer = BufferStatus {
                planner: b.planner_blocks,
                rx: b.rx_bytes,
            };
        }
        self.last = s.clone();
        if let Ok(mut g) = self.shared.status.lock() {
            *g = Some(s.clone());
        }
        self.sink.emit(Emit::Status(s));
    }

    /// Una vuelta de lectura: sondea `?` si toca, lee hasta `wait` y devuelve las líneas que no
    /// son reportes de estado (esos se emiten y no se registran).
    fn pump(&mut self, t: &mut dyn Transport, wait: Duration) -> Result<Vec<String>, Fail> {
        if self
            .last_poll
            .is_none_or(|p| p.elapsed() >= self.timing.status_interval)
        {
            t.write_all(&[Realtime::StatusQuery.byte()])?;
            self.last_poll = Some(Instant::now());
        }
        let mut buf = [0u8; 256];
        let n = t.read_timeout(&mut buf, wait)?;
        if n > 0 {
            self.last_rx = Instant::now();
            self.reader.feed(&buf[..n]);
        } else if self.last_rx.elapsed() > self.timing.silence_timeout {
            return Err(Fail::Lost(
                "la controladora dejó de responder (¿se desconectó el cable USB o se apagó la \
                 máquina?)"
                    .into(),
            ));
        }
        let mut others = Vec::new();
        while let Some(line) = self.reader.next_line() {
            match streamer::StatusReport::parse(&line) {
                Some(r) => self.on_status(&r),
                None => others.push(line),
            }
        }
        Ok(others)
    }

    // ---- reposo ------------------------------------------------------------

    fn idle_step(&mut self, mut t: Box<dyn Transport>) -> Result<Phase, Fail> {
        while let Some(req) = self.next_request()? {
            match req {
                Request::Disconnect => return Ok(Phase::Done),
                Request::Start {
                    lines,
                    rx_buffer,
                    kind,
                    reply,
                } => match self.try_start(t, lines, rx_buffer, kind) {
                    Ok(run) => {
                        let _ = reply.send(Ok(Vec::new()));
                        return Ok(Phase::Streaming(run));
                    }
                    Err(StartFail::Refused(back, msg)) => {
                        let _ = reply.send(Err(msg));
                        t = back;
                    }
                    Err(StartFail::Lost(msg)) => {
                        let _ = reply.send(Err(msg.clone()));
                        return Err(Fail::Lost(msg));
                    }
                },
                other => self.simple_request(&mut *t, other)?,
            }
        }
        for line in self.pump(&mut *t, self.timing.tick)? {
            self.log(LogDir::In, &describe_line(&line));
        }
        Ok(Phase::Idle(t))
    }

    /// Solicitudes que no cambian de fase. Responde siempre; solo devuelve `Err` si se perdió
    /// la conexión.
    fn simple_request(&mut self, t: &mut dyn Transport, req: Request) -> Result<(), Fail> {
        let (reply, result) = match req {
            Request::Command { line, reply } => (reply, self.command(t, &line)),
            Request::Realtime { cmd, reply } => (reply, self.realtime(t, cmd).map(|()| vec![])),
            Request::Jog {
                dx,
                dy,
                speed,
                reply,
            } => (reply, self.jog(t, dx, dy, speed).map(|()| vec![])),
            // Sin trabajo, pausar o reanudar no hace nada; cancelar apaga un jog en curso.
            Request::Pause(reply) | Request::Resume(reply) => (reply, Ok(vec![])),
            Request::Cancel(reply) => (
                reply,
                self.realtime(t, RealtimeCommand::JogCancel)
                    .map(|()| vec![]),
            ),
            Request::Start { .. } | Request::Disconnect => unreachable!("se atienden antes"),
        };
        match result {
            Ok(v) => {
                let _ = reply.send(Ok(v));
                Ok(())
            }
            Err(Fail::Msg(m)) => {
                let _ = reply.send(Err(m));
                Ok(())
            }
            Err(Fail::Lost(m)) => {
                let _ = reply.send(Err(m.clone()));
                Err(Fail::Lost(m))
            }
        }
    }

    fn realtime(&mut self, t: &mut dyn Transport, cmd: RealtimeCommand) -> Result<(), Fail> {
        let (rt, text) = match cmd {
            RealtimeCommand::Status => {
                // El reporte llega por el flujo normal de estado.
                t.write_all(&[Realtime::StatusQuery.byte()])?;
                self.last_poll = Some(Instant::now());
                return Ok(());
            }
            RealtimeCommand::Hold => (Realtime::FeedHold, "! (pausa, feed hold)"),
            RealtimeCommand::Resume => (Realtime::CycleStart, "~ (reanudar, cycle start)"),
            RealtimeCommand::Reset => (Realtime::SoftReset, "Ctrl+X (reinicio, soft reset)"),
            RealtimeCommand::JogCancel => (Realtime::JogCancel, "0x85 (cancelar movimiento)"),
        };
        self.log(LogDir::Out, text);
        t.write_all(&[rt.byte()])?;
        Ok(())
    }

    /// Envía una línea y espera `ok`, `error:N` o `ALARM:N`. Devuelve todas las líneas de
    /// respuesta, la última incluida (los errores con su explicación).
    fn command(&mut self, t: &mut dyn Transport, raw: &str) -> Result<Vec<String>, Fail> {
        let line = prepare_line(raw).map_err(|ch| {
            Fail::Msg(format!(
                "'{raw}' contiene {ch:?}, que GRBL interpretaría como comando en tiempo real. \
                 Para pausar, reanudar o reiniciar usa los botones de la pestaña Máquina."
            ))
        })?;
        if line.is_empty() {
            return Err(Fail::Msg("La línea está vacía.".into()));
        }
        let max = streamer::StreamerConfig::default().rx_buffer;
        if line.len() + 1 > max {
            return Err(Fail::Msg(format!(
                "'{line}' no cabe en el búfer de la controladora ({max} bytes)."
            )));
        }
        let timeout = if line.eq_ignore_ascii_case("$H") {
            self.timing.home_timeout
        } else {
            self.timing.command_timeout
        };
        self.exchange(t, &line, timeout)
    }

    fn exchange(
        &mut self,
        t: &mut dyn Transport,
        line: &str,
        timeout: Duration,
    ) -> Result<Vec<String>, Fail> {
        self.log(LogDir::Out, line);
        self.reader.clear();
        t.write_all(format!("{line}\n").as_bytes())?;
        let deadline = Instant::now() + timeout;
        let mut out = Vec::new();
        loop {
            let lines = self.pump(t, self.timing.tick)?;
            self.urgent_requests(t)?;
            for l in lines {
                let shown = describe_line(&l);
                self.log(LogDir::In, &shown);
                if l == "ok" {
                    out.push(l);
                    return Ok(out);
                }
                if l.starts_with("error") || l.starts_with("ALARM") {
                    out.push(shown);
                    return Ok(out);
                }
                out.push(l);
            }
            if Instant::now() > deadline {
                return Err(Fail::Msg(format!(
                    "La máquina no respondió a '{line}' en {:.0} s.",
                    timeout.as_secs_f64()
                )));
            }
        }
    }

    /// Durante una espera larga (p. ej. `$H`) atiende al instante pausa, reinicio y
    /// cancelación de jog; lo demás espera su turno.
    fn urgent_requests(&mut self, t: &mut dyn Transport) -> Result<(), Fail> {
        while let Ok(req) = self.rx.try_recv() {
            match req {
                Request::Realtime {
                    cmd:
                        cmd @ (RealtimeCommand::Hold
                        | RealtimeCommand::Reset
                        | RealtimeCommand::JogCancel
                        | RealtimeCommand::Status),
                    reply,
                } => {
                    let r = self.realtime(t, cmd);
                    let _ = reply.send(r.as_ref().map(|()| vec![]).map_err(|e| match e {
                        Fail::Msg(m) | Fail::Lost(m) => m.clone(),
                    }));
                    r?;
                }
                other => self.pending.push_back(other),
            }
        }
        Ok(())
    }

    fn jog(&mut self, t: &mut dyn Transport, dx: f64, dy: f64, speed: f64) -> Result<(), Fail> {
        if !(dx.is_finite() && dy.is_finite()) {
            return Err(Fail::Msg("El desplazamiento no es un número.".into()));
        }
        if !(speed.is_finite() && speed > 0.0) {
            return Err(Fail::Msg(
                "La velocidad del movimiento debe ser mayor que 0.".into(),
            ));
        }
        let line = format!("$J=G91 G21 X{} Y{} F{}", num(dx), num(dy), num(speed));
        let reply = self.exchange(t, &line, self.timing.command_timeout)?;
        match reply.last().map(String::as_str) {
            Some("ok") => Ok(()),
            Some(e) => Err(Fail::Msg(jog_error(e))),
            None => Err(Fail::Msg("La máquina no respondió al movimiento.".into())),
        }
    }

    // ---- inicio de trabajo -------------------------------------------------

    fn try_start(
        &mut self,
        mut t: Box<dyn Transport>,
        lines: Vec<String>,
        rx_buffer: usize,
        kind: RunKind,
    ) -> Result<Run, StartFail> {
        // Mismo orden que `tlaku send`: validar, comprobar el estado y solo entonces enviar.
        if let Err(e) = validate_lines(&lines, rx_buffer) {
            return Err(StartFail::Refused(t, e));
        }
        self.reader.clear();
        let mut shown = Vec::new();
        if let Err(msg) =
            preflight_with(&mut t, &self.timing.preflight, Advice::Console, &mut shown)
        {
            self.log(LogDir::In, &first_line(&msg));
            // Un fallo de E/S en la consulta no se distingue aquí de una máquina muda: el
            // sondeo normal detectará la pérdida si es real.
            return Err(StartFail::Refused(t, msg));
        }
        self.last_poll = None;
        let total = lines.len();
        let config = StreamerConfig {
            rx_buffer,
            ..StreamerConfig::default()
        };
        match Streamer::new(t, config).spawn(&lines) {
            Ok(job) => {
                match kind {
                    RunKind::Job => {
                        self.log(LogDir::Out, &format!("; trabajo: {total} líneas de G-code"));
                        self.job_event(JobState::Started, &format!("Enviando {total} líneas."));
                        self.sink.emit(Emit::Progress(ProgressEvent {
                            acked: 0,
                            total: total.try_into().unwrap_or(u32::MAX),
                        }));
                    }
                    RunKind::Frame => self.log(
                        LogDir::Out,
                        "; encuadre: recorrido del rectángulo con el láser apagado",
                    ),
                }
                Ok(Run {
                    job,
                    kind,
                    last_progress: None,
                    cancelling: false,
                })
            }
            // Las líneas ya se validaron: no debería pasar, pero el puerto se habría perdido.
            Err(e) => Err(StartFail::Lost(e.to_string())),
        }
    }

    // ---- transmisión -------------------------------------------------------

    fn stream_step(&mut self, mut run: Run) -> Result<Phase, Fail> {
        while let Some(req) = self.next_request()? {
            match req {
                Request::Disconnect => {
                    run.job.control.cancel();
                    self.wait_finished(run);
                    return Ok(Phase::Done);
                }
                other => self.stream_request(&mut run, other),
            }
        }
        match run.job.events.recv_timeout(self.timing.tick) {
            Ok(Event::Finished(_)) | Err(RecvTimeoutError::Disconnected) => self.finish(run),
            Ok(ev) => {
                self.stream_event(&mut run, ev);
                Ok(Phase::Streaming(run))
            }
            Err(RecvTimeoutError::Timeout) => Ok(Phase::Streaming(run)),
        }
    }

    fn stream_request(&mut self, run: &mut Run, req: Request) {
        let busy = "Hay un trabajo en curso. Pausa o cancela antes de enviar otra cosa a la \
                    máquina.";
        let ok = |reply: Reply| {
            let _ = reply.send(Ok(Vec::new()));
        };
        match req {
            Request::Pause(reply) => {
                self.pause(run);
                ok(reply);
            }
            Request::Resume(reply) => {
                self.resume(run);
                ok(reply);
            }
            Request::Cancel(reply) => {
                self.cancel(run);
                ok(reply);
            }
            Request::Realtime { cmd, reply } => {
                match cmd {
                    RealtimeCommand::Status => {}
                    RealtimeCommand::Hold => self.pause(run),
                    RealtimeCommand::Resume => self.resume(run),
                    RealtimeCommand::Reset => {
                        run.cancelling = true;
                        run.job.control.realtime(Realtime::SoftReset);
                    }
                    RealtimeCommand::JogCancel => run.job.control.realtime(Realtime::JogCancel),
                }
                ok(reply);
            }
            Request::Command { reply, .. }
            | Request::Jog { reply, .. }
            | Request::Start { reply, .. } => {
                let _ = reply.send(Err(busy.into()));
            }
            Request::Disconnect => unreachable!("se atiende antes"),
        }
    }

    fn pause(&self, run: &Run) {
        run.job.control.pause();
        if run.kind == RunKind::Job {
            self.job_event(JobState::Paused, "En pausa.");
        }
    }

    fn resume(&self, run: &Run) {
        run.job.control.resume();
        if run.kind == RunKind::Job {
            self.job_event(JobState::Started, "Reanudado.");
        }
    }

    fn cancel(&self, run: &mut Run) {
        run.cancelling = true;
        run.job.control.cancel();
    }

    fn stream_event(&mut self, run: &mut Run, ev: Event) {
        match ev {
            Event::Progress { acked, total, .. } => {
                if run.kind != RunKind::Job {
                    return;
                }
                let due = run
                    .last_progress
                    .is_none_or(|t| t.elapsed() >= self.timing.progress_interval);
                if due || acked == total {
                    run.last_progress = Some(Instant::now());
                    self.sink.emit(Emit::Progress(ProgressEvent {
                        acked: acked.try_into().unwrap_or(u32::MAX),
                        total: total.try_into().unwrap_or(u32::MAX),
                    }));
                }
            }
            Event::Status(r) => self.on_status(&r),
            Event::Error {
                code,
                source_line,
                text,
            } => {
                let at = match (source_line, text) {
                    (Some(l), Some(t)) => format!(" en la línea {l} ({t})"),
                    _ => String::new(),
                };
                self.log(
                    LogDir::In,
                    &format!("error:{code}{at}: {}", error_message(code)),
                );
            }
            Event::Alarm(code) => {
                self.log(
                    LogDir::In,
                    &format!("ALARM:{code}: {}", alarm_message(code)),
                );
            }
            Event::Message(m) | Event::Reset(m) => self.log(LogDir::In, &m),
            Event::Finished(_) => {}
        }
    }

    /// Espera el fin del envío tras pedir la cancelación.
    fn wait_finished(&mut self, run: Run) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            match run.job.events.recv_timeout(Duration::from_millis(50)) {
                Ok(Event::Finished(_)) | Err(RecvTimeoutError::Disconnected) => break,
                Ok(_) | Err(RecvTimeoutError::Timeout) => {}
            }
        }
        if run.kind == RunKind::Job {
            self.job_event(JobState::Cancelled, "Trabajo cancelado al desconectar.");
        }
        let _ = run.job.join();
    }

    /// El `Streamer` terminó: recupera el puerto y comunica el resultado.
    fn finish(&mut self, run: Run) -> Result<Phase, Fail> {
        let kind = run.kind;
        let (streamer, outcome) = run
            .job
            .join()
            .map_err(|_| Fail::Lost("el hilo de envío terminó de forma inesperada".into()))?;
        let t = streamer.into_transport();
        self.reader.clear();
        self.last_poll = None;
        self.last_rx = Instant::now();
        match (&outcome, kind) {
            (Outcome::Completed { errors: 0 }, RunKind::Job) => {
                self.job_event(JobState::Finished, "Trabajo terminado.");
            }
            (Outcome::Completed { errors: 0 }, RunKind::Frame) => {
                self.log(LogDir::In, "; encuadre terminado");
            }
            (Outcome::Cancelled, RunKind::Job) => self.job_event(
                JobState::Cancelled,
                "Trabajo cancelado: se envió pausa y reinicio, el láser quedó apagado.",
            ),
            (Outcome::Cancelled, RunKind::Frame) => {
                self.log(LogDir::In, "; encuadre cancelado");
            }
            _ => {
                let msg = outcome_result(&outcome)
                    .err()
                    .unwrap_or_else(|| "el envío terminó con errores".into());
                self.log(LogDir::In, &first_line(&msg));
                self.job_event(JobState::Error, &msg);
            }
        }
        if let Outcome::Disconnected(m) = outcome {
            return Err(Fail::Lost(format!(
                "se perdió la conexión con la máquina: {m}"
            )));
        }
        Ok(Phase::Idle(t))
    }
}

enum StartFail {
    /// No se transmitió nada; el puerto se devuelve.
    Refused(Box<dyn Transport>, String),
    Lost(String),
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or(s).to_string()
}

/// Número para G-code: hasta 3 decimales, sin ceros sobrantes ni `-0`.
fn num(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s.into()
    }
}

fn jog_error(reply: &str) -> String {
    let hint = if reply.starts_with("error:9") || reply.starts_with("ALARM") {
        " Si la máquina está en alarma, desbloquéala con $X o haz homing ($H) desde la consola."
    } else {
        ""
    };
    format!("La máquina rechazó el movimiento ({reply}).{hint}")
}

/// Valida el G-code contra el búfer de la controladora antes de tocar la máquina.
fn validate_lines(lines: &[String], rx_buffer: usize) -> Result<(), String> {
    tlaku_grbl::preflight::validate_gcode(&lines.join("\n"), rx_buffer).map(drop)
}

#[cfg(test)]
mod tests;
