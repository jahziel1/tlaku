//! Envío de trabajos a GRBL 1.1 con conteo de caracteres, comandos en tiempo
//! real y parser del reporte de estado.
//!
//! El conteo de caracteres lleva los bytes que cada línea ocupa en el búfer de
//! recepción de la controladora y solo envía otra mientras quepa; cada `ok` o
//! `error:N` libera la línea más antigua. Así el búfer se mantiene lleno sin
//! desbordarse (a diferencia del modo "una línea por vez", que se entrecorta).

use std::collections::VecDeque;
use std::fmt;
use std::io;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::serial::{LineReader, Transport};

/// Espera máxima de cada lectura; acota la latencia de pausa, cancelación y sondeo.
const TICK: Duration = Duration::from_millis(5);

/// Comandos en tiempo real de GRBL 1.1: un solo byte, sin `\n` y fuera del conteo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Realtime {
    StatusQuery,
    FeedHold,
    CycleStart,
    SoftReset,
    SafetyDoor,
    JogCancel,
    FeedReset,
    FeedPlus10,
    FeedMinus10,
    FeedPlus1,
    FeedMinus1,
    RapidFull,
    RapidHalf,
    RapidQuarter,
    SpindleReset,
    SpindlePlus10,
    SpindleMinus10,
    SpindlePlus1,
    SpindleMinus1,
    SpindleStop,
    ToggleFlood,
    ToggleMist,
}

impl Realtime {
    pub const fn byte(self) -> u8 {
        match self {
            Self::StatusQuery => b'?',
            Self::FeedHold => b'!',
            Self::CycleStart => b'~',
            Self::SoftReset => 0x18,
            Self::SafetyDoor => 0x84,
            Self::JogCancel => 0x85,
            Self::FeedReset => 0x90,
            Self::FeedPlus10 => 0x91,
            Self::FeedMinus10 => 0x92,
            Self::FeedPlus1 => 0x93,
            Self::FeedMinus1 => 0x94,
            Self::RapidFull => 0x95,
            Self::RapidHalf => 0x96,
            Self::RapidQuarter => 0x97,
            Self::SpindleReset => 0x99,
            Self::SpindlePlus10 => 0x9A,
            Self::SpindleMinus10 => 0x9B,
            Self::SpindlePlus1 => 0x9C,
            Self::SpindleMinus1 => 0x9D,
            Self::SpindleStop => 0x9E,
            Self::ToggleFlood => 0xA0,
            Self::ToggleMist => 0xA1,
        }
    }
}

// ---------------------------------------------------------------------------
// Reporte de estado
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineState {
    Idle,
    Run,
    /// Subcódigo: 0 = detención completa, 1 = deteniéndose.
    Hold(Option<u8>),
    Jog,
    Alarm,
    /// Subcódigo: 0 = puerta cerrada y lista, 1 = puerta abierta, 2 = reanudando, 3 = ya cerrada.
    Door(Option<u8>),
    Check,
    Home,
    Sleep,
}

/// Coordenadas de la máquina. `z` vale 0 si el reporte solo trae dos ejes;
/// los ejes adicionales (A, B, ...) van en `extra`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub extra: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferState {
    /// Bloques libres en el planificador de movimiento.
    pub planner_blocks: u32,
    /// Bytes libres en el búfer de recepción serie.
    pub rx_bytes: u32,
}

/// Porcentajes de override activos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overrides {
    pub feed: u32,
    pub rapid: u32,
    pub spindle: u32,
}

/// Reporte de estado (`<Idle|MPos:...|...>`). Los campos ausentes quedan en `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusReport {
    pub state: MachineState,
    pub mpos: Option<Position>,
    pub wpos: Option<Position>,
    pub wco: Option<Position>,
    pub buffer: Option<BufferState>,
    pub line_number: Option<u32>,
    /// Velocidad de avance actual (de `FS:` o `F:`).
    pub feed: Option<f64>,
    /// Velocidad/potencia del husillo (solo en `FS:`).
    pub spindle: Option<f64>,
    pub overrides: Option<Overrides>,
    /// Pines activos (`Pn:`), p. ej. `XYZPDHRS`.
    pub pins: Option<String>,
    /// Accesorios activos (`A:`), p. ej. `SFM`.
    pub accessories: Option<String>,
}

fn parse_position(s: &str) -> Option<Position> {
    let v: Vec<f64> = s
        .split(',')
        .map(|n| n.trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .ok()?;
    if v.len() < 2 {
        return None;
    }
    Some(Position {
        x: v[0],
        y: v[1],
        z: v.get(2).copied().unwrap_or(0.0),
        extra: v.iter().skip(3).copied().collect(),
    })
}

fn parse_state(s: &str) -> Option<MachineState> {
    let (name, sub) = match s.split_once(':') {
        Some((n, c)) => (n, Some(c.trim().parse::<u8>().ok()?)),
        None => (s, None),
    };
    Some(match name {
        "Idle" => MachineState::Idle,
        "Run" => MachineState::Run,
        "Hold" => MachineState::Hold(sub),
        "Jog" => MachineState::Jog,
        "Alarm" => MachineState::Alarm,
        "Door" => MachineState::Door(sub),
        "Check" => MachineState::Check,
        "Home" => MachineState::Home,
        "Sleep" => MachineState::Sleep,
        _ => return None,
    })
}

impl StatusReport {
    /// Interpreta una línea `<...>`. Devuelve `None` si no es un reporte o no
    /// trae un estado válido; los campos desconocidos o mal formados se ignoran.
    pub fn parse(line: &str) -> Option<Self> {
        let inner = line.trim().strip_prefix('<')?.strip_suffix('>')?;
        let mut fields = inner.split('|');
        let mut report = StatusReport {
            state: parse_state(fields.next()?.trim())?,
            mpos: None,
            wpos: None,
            wco: None,
            buffer: None,
            line_number: None,
            feed: None,
            spindle: None,
            overrides: None,
            pins: None,
            accessories: None,
        };
        for field in fields {
            let Some((key, value)) = field.split_once(':') else {
                continue;
            };
            let nums = || -> Option<Vec<f64>> {
                value
                    .split(',')
                    .map(|n| n.trim().parse::<f64>().ok())
                    .collect()
            };
            match key {
                "MPos" => report.mpos = parse_position(value),
                "WPos" => report.wpos = parse_position(value),
                "WCO" => report.wco = parse_position(value),
                "Bf" => {
                    if let Some(&[a, b]) = nums().as_deref() {
                        report.buffer = Some(BufferState {
                            planner_blocks: a as u32,
                            rx_bytes: b as u32,
                        });
                    }
                }
                "Ln" => report.line_number = value.trim().parse().ok(),
                "FS" => {
                    if let Some(&[f, s]) = nums().as_deref() {
                        report.feed = Some(f);
                        report.spindle = Some(s);
                    }
                }
                "F" => report.feed = value.trim().parse().ok(),
                "Ov" => {
                    if let Some(&[f, r, s]) = nums().as_deref() {
                        report.overrides = Some(Overrides {
                            feed: f as u32,
                            rapid: r as u32,
                            spindle: s as u32,
                        });
                    }
                }
                "Pn" => report.pins = Some(value.trim().to_string()),
                "A" => report.accessories = Some(value.trim().to_string()),
                _ => {}
            }
        }
        Some(report)
    }
}

// ---------------------------------------------------------------------------
// Mensajes de error y alarma
// ---------------------------------------------------------------------------

/// Descripción breve de un `error:N` de GRBL 1.1.
pub fn error_message(code: u32) -> &'static str {
    match code {
        1 => "falta la letra del comando G-code",
        2 => "formato de número incorrecto",
        3 => "comando '$' no válido",
        4 => "valor negativo no permitido",
        5 => "ajuste desactivado",
        6 => "pulso de paso demasiado corto",
        7 => "fallo al leer la memoria no volátil",
        8 => "la máquina no está en reposo",
        9 => "G-code bloqueado por alarma o jog",
        10 => "límite blando excedido",
        11 => "línea demasiado larga para el búfer de la controladora",
        12 => "velocidad de pasos máxima excedida",
        13 => "puerta abierta",
        14 => "longitud de línea excedida",
        15 => "recorrido del jog excedido",
        16 => "comando de jog no válido",
        17 => "el modo láser requiere salida PWM",
        20 => "comando G-code no soportado o no válido",
        21 => "varios comandos del mismo grupo modal en una línea",
        22 => "velocidad de avance sin definir",
        23 => "el comando requiere un valor entero",
        24 => "dos comandos usan palabras de eje a la vez",
        25 => "palabra G-code repetida",
        26 => "el comando requiere palabras de eje",
        27 => "número de línea no válido",
        28 => "falta un valor requerido",
        29 => "G59.x no soportado",
        30 => "G53 solo es válido con G0 y G1",
        31 => "hay palabras de eje que ningún comando usa",
        32 => "G2/G3 requieren al menos un eje del plano",
        33 => "destino del movimiento no válido",
        34 => "radio de arco no válido",
        35 => "G2/G3 requieren al menos un desplazamiento del plano",
        36 => "hay palabras con valor sin usar",
        37 => "compensación de herramienta G43.1 no asignada al eje configurado",
        38 => "número de herramienta mayor al máximo",
        _ => "error desconocido",
    }
}

/// Descripción breve de un `ALARM:N` de GRBL 1.1.
pub fn alarm_message(code: u32) -> &'static str {
    match code {
        1 => "límite físico activado",
        2 => "el movimiento excede el recorrido de la máquina",
        3 => "reinicio durante un movimiento: se perdió la posición",
        4 => "fallo de sonda: estado inicial inesperado",
        5 => "fallo de sonda: sin contacto",
        6 => "fallo de homing: reinicio durante el ciclo",
        7 => "fallo de homing: puerta abierta durante el ciclo",
        8 => "fallo de homing: no se pudo liberar el final de carrera",
        9 => "fallo de homing: no se encontró el final de carrera",
        _ => "alarma desconocida",
    }
}

// ---------------------------------------------------------------------------
// Preparación de líneas
// ---------------------------------------------------------------------------

/// Quita comentarios (`;...` y `(...)`) y espacios. Devuelve cadena vacía si no
/// queda nada. Falla con el carácter ofensor si queda uno que GRBL interpretaría
/// como comando en tiempo real (`?`, `!`, `~`, no ASCII) o de control.
pub fn prepare_line(raw: &str) -> Result<String, char> {
    let mut out = String::with_capacity(raw.len());
    let mut in_paren = false;
    for c in raw.chars() {
        match c {
            ';' if !in_paren => break,
            '(' => in_paren = true,
            ')' => in_paren = false,
            _ if in_paren || c.is_whitespace() => {}
            '?' | '!' | '~' => return Err(c),
            _ if !c.is_ascii() || c.is_control() => return Err(c),
            _ => out.push(c),
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamError {
    /// La línea (con su `\n`) no cabe en el búfer de recepción.
    LineTooLong {
        source_line: usize,
        len: usize,
        max: usize,
    },
    InvalidChar {
        source_line: usize,
        ch: char,
    },
}

impl fmt::Display for StreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LineTooLong {
                source_line,
                len,
                max,
            } => write!(
                f,
                "la línea {source_line} ocupa {len} bytes y el búfer de la controladora es de {max}"
            ),
            Self::InvalidChar { source_line, ch } => write!(
                f,
                "la línea {source_line} contiene {ch:?}, que GRBL interpretaría como comando en tiempo real"
            ),
        }
    }
}

impl std::error::Error for StreamError {}

struct Prepared {
    /// Texto sin el `\n` final.
    text: String,
    /// Número de línea (desde 1) en la entrada original.
    source_line: usize,
}

// ---------------------------------------------------------------------------
// Configuración, eventos y control
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorPolicy {
    /// Un `error:N` termina el trabajo.
    Stop,
    /// Se registra el error y el trabajo sigue con la línea siguiente.
    Continue,
}

#[derive(Debug, Clone)]
pub struct StreamerConfig {
    /// Bytes que se pueden tener pendientes en la controladora. GRBL declara 128,
    /// pero su búfer circular deja libre un byte, así que se usa 127 por defecto;
    /// grblHAL admite bastante más.
    pub rx_buffer: usize,
    pub error_policy: ErrorPolicy,
    /// Cada cuánto se consulta `?`; `None` desactiva el sondeo (y con él la
    /// espera de reposo y el aviso por silencio).
    pub status_interval: Option<Duration>,
    /// Sin recibir ningún byte durante este tiempo (con sondeo activo) se da
    /// la conexión por perdida.
    pub silence_timeout: Duration,
    /// Terminar solo cuando la máquina informe `Idle` tras el último `ok`
    /// (los `ok` llegan al interpretar la línea, no al ejecutarla).
    pub wait_idle: bool,
    /// Pausa entre feed hold y soft reset al cancelar (y después del reset).
    pub cancel_delay: Duration,
}

impl Default for StreamerConfig {
    fn default() -> Self {
        Self {
            rx_buffer: 127,
            error_policy: ErrorPolicy::Stop,
            status_interval: Some(Duration::from_millis(200)),
            silence_timeout: Duration::from_secs(5),
            wait_idle: true,
            cancel_delay: Duration::from_millis(50),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Una línea fue respondida. `sent` incluye las aún sin confirmar y
    /// `buffer_used` los bytes que se cree ocupados en la controladora.
    Progress {
        acked: usize,
        sent: usize,
        total: usize,
        buffer_used: usize,
    },
    Status(Box<StatusReport>),
    /// `error:N`. `source_line` y `text` son `None` si no hay línea pendiente a la que atribuirlo.
    Error {
        code: u32,
        source_line: Option<usize>,
        text: Option<String>,
    },
    Alarm(u32),
    /// Mensaje `[...]` u otra línea no reconocida.
    Message(String),
    /// La controladora se reinició (saludo `Grbl ...`).
    Reset(String),
    Finished(Outcome),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Todas las líneas se respondieron; `errors` cuenta los `error:N` tolerados.
    Completed {
        errors: usize,
    },
    Cancelled,
    /// `error:N` con la política `Stop`.
    LineError {
        code: u32,
        source_line: usize,
    },
    Alarm(u32),
    /// La controladora se reinició a mitad del trabajo.
    Reset(String),
    /// Fallo de E/S o silencio prolongado.
    Disconnected(String),
}

#[derive(Debug)]
enum Command {
    Pause,
    Resume,
    Cancel,
    Realtime(Realtime),
}

/// Mando remoto de un trabajo en curso; se puede clonar y usar desde otro hilo.
/// Los envíos a un `Streamer` ya destruido se ignoran.
#[derive(Clone)]
pub struct Control {
    tx: Sender<Command>,
}

impl Control {
    /// Feed hold y dejar de enviar líneas nuevas.
    pub fn pause(&self) {
        let _ = self.tx.send(Command::Pause);
    }

    /// Reanuda (`~`) y vuelve a enviar. No hace nada si no estaba en pausa.
    pub fn resume(&self) {
        let _ = self.tx.send(Command::Resume);
    }

    /// Feed hold, soft reset y fin del trabajo.
    pub fn cancel(&self) {
        let _ = self.tx.send(Command::Cancel);
    }

    /// Byte en tiempo real (overrides, jog cancel...). `SoftReset` también
    /// termina el trabajo como cancelado.
    pub fn realtime(&self, cmd: Realtime) {
        let _ = self.tx.send(Command::Realtime(cmd));
    }
}

/// Trabajo lanzado en su propio hilo con [`Streamer::spawn`].
pub struct Job<T: Transport> {
    pub control: Control,
    pub events: Receiver<Event>,
    handle: JoinHandle<(Streamer<T>, Outcome)>,
}

impl<T: Transport> Job<T> {
    /// Espera el fin del trabajo y devuelve el streamer (con su transporte) y el resultado.
    pub fn join(self) -> thread::Result<(Streamer<T>, Outcome)> {
        self.handle.join()
    }
}

// ---------------------------------------------------------------------------
// Streamer
// ---------------------------------------------------------------------------

pub struct Streamer<T: Transport> {
    transport: T,
    config: StreamerConfig,
    cmd_tx: Sender<Command>,
    cmd_rx: Receiver<Command>,
    reader: LineReader,
}

struct RunState {
    next: usize,
    acked: usize,
    errors: usize,
    used: usize,
    /// (índice en el trabajo, bytes) de las líneas enviadas sin responder.
    pending: VecDeque<(usize, usize)>,
    paused: bool,
    idle_seen: bool,
    last_poll: Option<Instant>,
    last_rx: Instant,
}

impl<T: Transport> Streamer<T> {
    pub fn new(transport: T, config: StreamerConfig) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        Self {
            transport,
            config,
            cmd_tx,
            cmd_rx,
            reader: LineReader::default(),
        }
    }

    pub fn control(&self) -> Control {
        Control {
            tx: self.cmd_tx.clone(),
        }
    }

    pub fn into_transport(self) -> T {
        self.transport
    }

    /// Envía un comando en tiempo real fuera de un trabajo.
    pub fn send_realtime(&mut self, cmd: Realtime) -> io::Result<()> {
        self.transport.write_all(&[cmd.byte()])
    }

    fn prepare<S: AsRef<str>>(&self, lines: &[S]) -> Result<Vec<Prepared>, StreamError> {
        let max = self.config.rx_buffer;
        let mut job = Vec::new();
        for (i, raw) in lines.iter().enumerate() {
            let source_line = i + 1;
            let text = prepare_line(raw.as_ref())
                .map_err(|ch| StreamError::InvalidChar { source_line, ch })?;
            if text.is_empty() {
                continue;
            }
            let len = text.len() + 1;
            if len > max {
                return Err(StreamError::LineTooLong {
                    source_line,
                    len,
                    max,
                });
            }
            job.push(Prepared { text, source_line });
        }
        Ok(job)
    }

    /// Ejecuta el trabajo en el hilo actual hasta que termine, informando cada
    /// evento a `on_event`. Las líneas se validan antes de enviar nada.
    pub fn run<S: AsRef<str>>(
        &mut self,
        lines: &[S],
        mut on_event: impl FnMut(Event),
    ) -> Result<Outcome, StreamError> {
        let job = self.prepare(lines)?;
        Ok(self.execute(&job, &mut on_event))
    }

    /// Como [`run`](Self::run), pero en un hilo propio; los eventos llegan por
    /// `Job::events` y el mando es `Job::control`.
    pub fn spawn<S: AsRef<str>>(self, lines: &[S]) -> Result<Job<T>, StreamError>
    where
        T: 'static,
    {
        let job = self.prepare(lines)?;
        let control = self.control();
        let (tx, events) = mpsc::channel();
        let handle = thread::spawn(move || {
            let mut streamer = self;
            let outcome = streamer.execute(&job, &mut |e| {
                let _ = tx.send(e);
            });
            (streamer, outcome)
        });
        Ok(Job {
            control,
            events,
            handle,
        })
    }

    fn execute(&mut self, job: &[Prepared], emit: &mut dyn FnMut(Event)) -> Outcome {
        while self.cmd_rx.try_recv().is_ok() {}
        self.reader.clear();
        let mut st = RunState {
            next: 0,
            acked: 0,
            errors: 0,
            used: 0,
            pending: VecDeque::new(),
            paused: false,
            idle_seen: false,
            last_poll: None,
            last_rx: Instant::now(),
        };
        let outcome = if job.is_empty() {
            Outcome::Completed { errors: 0 }
        } else {
            loop {
                match self.tick(job, &mut st, emit) {
                    Ok(Some(outcome)) => break outcome,
                    Ok(None) => {}
                    Err(e) => break Outcome::Disconnected(e.to_string()),
                }
            }
        };
        emit(Event::Finished(outcome.clone()));
        outcome
    }

    /// Una vuelta del ciclo: mandos, envío, sondeo, lectura y comprobaciones de fin.
    fn tick(
        &mut self,
        job: &[Prepared],
        st: &mut RunState,
        emit: &mut dyn FnMut(Event),
    ) -> io::Result<Option<Outcome>> {
        while let Ok(cmd) = self.cmd_rx.try_recv() {
            if let Some(outcome) = self.handle_command(cmd, st)? {
                return Ok(Some(outcome));
            }
        }

        if !st.paused && st.next < job.len() {
            let mut batch = Vec::new();
            while let Some(line) = job.get(st.next) {
                let len = line.text.len() + 1;
                if st.used + len > self.config.rx_buffer {
                    break;
                }
                batch.extend_from_slice(line.text.as_bytes());
                batch.push(b'\n');
                st.pending.push_back((st.next, len));
                st.used += len;
                st.next += 1;
            }
            if !batch.is_empty() {
                self.transport.write_all(&batch)?;
            }
        }

        let polling = self.config.status_interval;
        if let Some(interval) = polling
            && st.last_poll.is_none_or(|t| t.elapsed() >= interval)
        {
            self.transport.write_all(&[Realtime::StatusQuery.byte()])?;
            st.last_poll = Some(Instant::now());
        }

        let mut buf = [0u8; 256];
        let n = self.transport.read_timeout(&mut buf, TICK)?;
        if n > 0 {
            st.last_rx = Instant::now();
            self.reader.feed(&buf[..n]);
        }
        while let Some(line) = self.reader.next_line() {
            if let Some(outcome) = self.handle_line(&line, job, st, emit) {
                return Ok(Some(outcome));
            }
        }

        if st.acked == job.len() && (st.idle_seen || !(self.config.wait_idle && polling.is_some()))
        {
            return Ok(Some(Outcome::Completed { errors: st.errors }));
        }
        if polling.is_some() && st.last_rx.elapsed() > self.config.silence_timeout {
            return Ok(Some(Outcome::Disconnected(
                "la controladora dejó de responder".into(),
            )));
        }
        Ok(None)
    }

    fn handle_command(&mut self, cmd: Command, st: &mut RunState) -> io::Result<Option<Outcome>> {
        match cmd {
            Command::Pause if !st.paused => {
                self.transport.write_all(&[Realtime::FeedHold.byte()])?;
                st.paused = true;
            }
            Command::Resume if st.paused => {
                self.transport.write_all(&[Realtime::CycleStart.byte()])?;
                st.paused = false;
            }
            Command::Pause | Command::Resume => {}
            Command::Cancel => {
                self.transport.write_all(&[Realtime::FeedHold.byte()])?;
                thread::sleep(self.config.cancel_delay);
                self.reset_device()?;
                return Ok(Some(Outcome::Cancelled));
            }
            Command::Realtime(Realtime::SoftReset) => {
                self.reset_device()?;
                return Ok(Some(Outcome::Cancelled));
            }
            Command::Realtime(r) => self.transport.write_all(&[r.byte()])?,
        }
        Ok(None)
    }

    fn reset_device(&mut self) -> io::Result<()> {
        self.transport.write_all(&[Realtime::SoftReset.byte()])?;
        thread::sleep(self.config.cancel_delay);
        // El saludo posterior al reinicio no debe confundirse con respuestas futuras.
        self.transport.clear_input()
    }

    fn handle_line(
        &mut self,
        line: &str,
        job: &[Prepared],
        st: &mut RunState,
        emit: &mut dyn FnMut(Event),
    ) -> Option<Outcome> {
        let progress = |st: &RunState| Event::Progress {
            acked: st.acked,
            sent: st.next,
            total: job.len(),
            buffer_used: st.used,
        };
        if line == "ok" {
            if let Some((_, len)) = st.pending.pop_front() {
                st.used -= len;
                st.acked += 1;
                emit(progress(st));
            }
        } else if let Some(code) = line.strip_prefix("error:") {
            let Ok(code) = code.trim().parse::<u32>() else {
                emit(Event::Message(line.to_string()));
                return None;
            };
            let Some((idx, len)) = st.pending.pop_front() else {
                emit(Event::Error {
                    code,
                    source_line: None,
                    text: None,
                });
                return None;
            };
            st.used -= len;
            st.acked += 1;
            st.errors += 1;
            let source_line = job[idx].source_line;
            emit(Event::Error {
                code,
                source_line: Some(source_line),
                text: Some(job[idx].text.clone()),
            });
            emit(progress(st));
            if self.config.error_policy == ErrorPolicy::Stop {
                return Some(Outcome::LineError { code, source_line });
            }
        } else if let Some(code) = line.strip_prefix("ALARM:") {
            if let Ok(code) = code.trim().parse::<u32>() {
                emit(Event::Alarm(code));
                return Some(Outcome::Alarm(code));
            }
            emit(Event::Message(line.to_string()));
        } else if line.starts_with('<') {
            match StatusReport::parse(line) {
                Some(report) => {
                    if st.acked == job.len() && report.state == MachineState::Idle {
                        st.idle_seen = true;
                    }
                    emit(Event::Status(Box::new(report)));
                }
                None => emit(Event::Message(line.to_string())),
            }
        } else if line.len() >= 4 && line[..4].eq_ignore_ascii_case("grbl") {
            emit(Event::Reset(line.to_string()));
            return Some(Outcome::Reset(line.to_string()));
        } else {
            emit(Event::Message(line.to_string()));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serial::memory_pair;

    #[test]
    fn prepare_strips_comments_and_spaces() {
        assert_eq!(
            prepare_line("G1 X10.5 Y-2 F1000").unwrap(),
            "G1X10.5Y-2F1000"
        );
        assert_eq!(prepare_line("G0 X1 ; mover").unwrap(), "G0X1");
        assert_eq!(prepare_line("G0 (rápido) X1 (otro) Y2").unwrap(), "G0X1Y2");
        assert_eq!(prepare_line("(solo comentario)").unwrap(), "");
        assert_eq!(prepare_line("   \t ").unwrap(), "");
        assert_eq!(prepare_line("; todo ! ? ~").unwrap(), "");
        assert_eq!(prepare_line("G1 X1 (sin cerrar ! ?").unwrap(), "G1X1");
        assert_eq!(prepare_line("$J=G91 X10 F500").unwrap(), "$J=G91X10F500");
    }

    #[test]
    fn prepare_rejects_realtime_lookalikes() {
        assert_eq!(prepare_line("G1 X1 !"), Err('!'));
        assert_eq!(prepare_line("G1 ~"), Err('~'));
        assert_eq!(prepare_line("?"), Err('?'));
        assert_eq!(prepare_line("G1 Xé"), Err('é'));
        assert_eq!(prepare_line("G1\u{18}"), Err('\u{18}'));
    }

    fn streamer(rx_buffer: usize) -> Streamer<crate::serial::MemoryTransport> {
        let (host, _dev) = memory_pair();
        let config = StreamerConfig {
            rx_buffer,
            ..Default::default()
        };
        Streamer::new(host, config)
    }

    #[test]
    fn line_must_fit_with_newline() {
        let s = streamer(8);
        assert!(s.prepare(&["G1X1Y22"]).is_ok()); // 7 + '\n' = 8
        assert_eq!(
            s.prepare(&["G0", "G1X1Y223"]).err(),
            Some(StreamError::LineTooLong {
                source_line: 2,
                len: 9,
                max: 8
            })
        );
    }

    #[test]
    fn empty_lines_are_skipped_but_keep_source_numbers() {
        let s = streamer(128);
        let job = s.prepare(&["G0 X0", "", "; nada", "G1 X1"]).unwrap();
        let got: Vec<_> = job
            .iter()
            .map(|p| (p.source_line, p.text.as_str()))
            .collect();
        assert_eq!(got, [(1, "G0X0"), (4, "G1X1")]);
    }

    #[test]
    fn empty_job_completes_without_io() {
        let mut s = streamer(128);
        let mut events = Vec::new();
        let out = s.run::<&str>(&[], |e| events.push(e)).unwrap();
        assert_eq!(out, Outcome::Completed { errors: 0 });
        assert_eq!(events, [Event::Finished(Outcome::Completed { errors: 0 })]);
    }

    #[test]
    fn realtime_bytes_match_grbl_11() {
        assert_eq!(Realtime::SoftReset.byte(), 0x18);
        assert_eq!(Realtime::JogCancel.byte(), 0x85);
        assert_eq!(Realtime::FeedPlus10.byte(), 0x91);
        assert_eq!(Realtime::SpindleStop.byte(), 0x9E);
    }

    #[test]
    fn status_edge_cases() {
        assert_eq!(StatusReport::parse("ok"), None);
        assert_eq!(StatusReport::parse("<>"), None);
        assert_eq!(StatusReport::parse("<Bogus|MPos:0,0,0>"), None);
        assert_eq!(StatusReport::parse("<Idle|MPos:0,0,0"), None);
        let r = StatusReport::parse("<Run|MPos:abc|Zz:1|Bf:1|FS:5,6>").unwrap();
        assert_eq!((r.mpos, r.buffer), (None, None));
        assert_eq!((r.feed, r.spindle), (Some(5.0), Some(6.0)));
        let r = StatusReport::parse(" <Idle|MPos:1.5,2.5>\r").unwrap();
        assert_eq!(r.mpos.unwrap().z, 0.0);
        let r = StatusReport::parse("<Idle|MPos:1,2,3,4,5>").unwrap();
        assert_eq!(r.mpos.unwrap().extra, [4.0, 5.0]);
        assert_eq!(
            StatusReport::parse("<Hold|MPos:0,0,0>").unwrap().state,
            MachineState::Hold(None)
        );
    }

    #[test]
    fn error_tables_cover_known_codes() {
        assert_eq!(error_message(20), "comando G-code no soportado o no válido");
        assert_eq!(error_message(999), "error desconocido");
        assert_eq!(alarm_message(1), "límite físico activado");
        assert_eq!(alarm_message(99), "alarma desconocida");
    }
}
