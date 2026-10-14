//! Conexión con la máquina GRBL. (La de Ruida vive en [`ruida`] y el despacho entre respaldos, en
//! [`active`].)
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
//!
//! Máquinas virtuales ([`virtual_port`]). Un puerto `virtual:<perfil>` abre un
//! `tlaku_emu::VirtualMachine` con reloj de pared en lugar del puerto serie; el resto (estado,
//! envío, pausa, jog, consola) corre exactamente igual. [`Handle::simulate_fault`] provoca fallas
//! (llama, alarma 3, desconexión) y solo existe con una máquina virtual.
//!
//! Regla del silencio ("conexión perdida"). Con el sondeo activo, sin recibir ni un byte durante
//! `silence_timeout` (5 s) la conexión se da por perdida, porque en funcionamiento normal GRBL
//! contesta `?` en 5 a 20 ms. Pero hay dos casos en que el silencio es legítimo, y en ellos
//! no se declara la pérdida:
//!
//! - **Homing (`$H`)**: `?` puede no contestarse y el `ok` llega al terminar el ciclo (un
//!   minuto o más). Mientras haya un `$H` sin responder, el plazo es `home_timeout` (120 s).
//!   En el trabajo (`Streamer`) rige la misma regla si una de sus líneas es un `$H`.
//! - **Bloqueo crítico** tras `ALARM:1`/`ALARM:2` o `[MSG:Reset to continue]`: GRBL deja de
//!   atender `?` hasta un soft reset. No hay plazo: solo se sale al recibir un `ok` o el
//!   saludo (`Grbl ...`) tras el reset, y entretanto `?` se manda cada 2 s en vez de cada
//!   250 ms. Un cable arrancado sigue detectándose, porque la lectura o la escritura
//!   fallan con error de E/S (eso no depende del silencio).
//!
//! Fuera de esos dos casos el plazo no cambia.
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
    MachineState, OverrideKind, Overrides, ProgressEvent, RealtimeCommand, SimulatedFault,
    StatusReport,
};
use tlaku_grbl::link::{LinkFamily, LinkProfile};
use tlaku_grbl::preflight::{
    Advice, JobExpect, PreflightConfig, ReprapInfo, outcome_result_for, preflight_link,
    validate_gcode_for,
};
use tlaku_grbl::serial::{Grbl, LineReader, Transport};
use tlaku_grbl::streamer::{
    self, Event, Firmware, Job, Outcome, POSITION_LOST_NOTE, Realtime, Streamer, StreamerConfig,
    alarm_message_for, creality_error_message, error_message_for, is_greeting, is_homing_line,
    override_plan, parse_creality_error, prepare_line_with,
};

pub mod active;
pub mod extras;
pub mod frame;
pub mod lhymicro;
pub mod ruida;
pub mod virtual_port;

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
    /// Espera de la respuesta a `$H` (GRBL contesta `ok` al terminar el homing) y plazo de
    /// silencio mientras dura (`?` puede no contestarse durante el homing).
    pub home_timeout: Duration,
    /// Tras un feed hold de cancelación, cuánto se espera a que la máquina termine de frenar
    /// (`Hold:0`) antes de resetear de todos modos.
    pub hold_timeout: Duration,
    /// Cuánto se espera el saludo tras un soft reset de cancelación.
    pub reset_wait: Duration,
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
            home_timeout: Duration::from_secs(120),
            hold_timeout: Duration::from_secs(2),
            reset_wait: Duration::from_millis(500),
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
    /// Movimiento absoluto en coordenadas de trabajo, con el láser apagado.
    JogTo {
        x: f64,
        y: f64,
        speed: f64,
        reply: Reply,
    },
    /// Ajusta un override en tiempo real (velocidad o potencia) al porcentaje pedido.
    Override {
        kind: OverrideKind,
        percent: u32,
        reply: Reply,
    },
    Start {
        lines: Vec<String>,
        link: Box<LinkProfile>,
        s_max: Option<f64>,
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
    /// Mando del emulador si la conexión es una máquina virtual.
    emu: Option<tlaku_emu::Handle>,
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

    /// `true` si la conexión es una máquina virtual (nada se graba de verdad).
    pub fn is_virtual(&self) -> bool {
        self.emu.is_some()
    }

    /// Provoca una falla en la máquina virtual. Con una máquina real no hace nada y avisa.
    pub fn simulate_fault(&self, fault: SimulatedFault) -> Result<(), String> {
        if self.is_closed() {
            return Err(NOT_CONNECTED.into());
        }
        match &self.emu {
            Some(emu) if self.is_virtual() => {
                virtual_port::inject(emu, fault);
                Ok(())
            }
            _ => Err(
                "Simular fallas solo es posible con una máquina virtual; esta es una máquina \
                 real."
                    .into(),
            ),
        }
    }

    /// Posición de trabajo [x, y] del último reporte de estado.
    pub fn work_pos(&self) -> Option<[f64; 2]> {
        self.shared.status.lock().ok()?.as_ref().map(|s| s.work_pos)
    }

    /// Último reporte de estado completo (para la automatización: `STATUS`).
    pub fn status_report(&self) -> Option<StatusReport> {
        self.shared.status.lock().ok()?.clone()
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

    /// Movimiento absoluto a (`x`, `y`) en coordenadas de trabajo (las de `work_pos`), en mm, a
    /// `speed` mm/min. Es un movimiento de posicionamiento: el láser no se enciende.
    pub fn jog_to(&self, x: f64, y: f64, speed: f64) -> Result<(), String> {
        self.call(|reply| Request::JogTo { x, y, speed, reply })
            .map(drop)
    }

    /// Lleva el override de `kind` (velocidad o potencia) a `percent` por ciento (10 a 200).
    /// Solo GRBL: Marlin y Smoothieware no tienen bytes en tiempo real. Vale con o sin trabajo.
    pub fn set_override(&self, kind: OverrideKind, percent: u32) -> Result<(), String> {
        self.call(|reply| Request::Override {
            kind,
            percent,
            reply,
        })
        .map(drop)
    }

    /// Comprueba el estado de la máquina y, solo si está en reposo y en modo láser (`$32=1`),
    /// transmite el G-code. No compara `$30` con la S máx del perfil; para eso usa
    /// [`start_job_for`](Self::start_job_for).
    #[cfg(test)]
    pub fn start_job(&self, lines: Vec<String>, rx_buffer: usize) -> Result<(), String> {
        self.start_job_for(lines, rx_buffer, None)
    }

    /// Como [`start_job`](Self::start_job), con la S máx del perfil: si `$30` de la controladora
    /// es distinto se avisa en el registro (la potencia saldría escalada).
    #[cfg(test)]
    pub fn start_job_for(
        &self,
        lines: Vec<String>,
        rx_buffer: usize,
        s_max: Option<f64>,
    ) -> Result<(), String> {
        self.start_job_link(lines, LinkProfile::grbl_with_buffer(rx_buffer), s_max)
    }

    /// Como [`start_job_for`](Self::start_job_for) con el enlace del perfil de la máquina
    /// ([`LinkProfile::for_device`]): GRBL con conteo de caracteres, o Marlin y Smoothieware con
    /// una línea por `ok`. Debe coincidir con la familia de la controladora conectada.
    pub fn start_job_link(
        &self,
        lines: Vec<String>,
        link: LinkProfile,
        s_max: Option<f64>,
    ) -> Result<(), String> {
        self.start(lines, link, s_max, RunKind::Job)
    }

    /// Como [`start_job`](Self::start_job) para el encuadre (sin eventos de progreso). El
    /// encuadre va con el láser apagado (M5, S0), así que no exige el modo láser.
    #[cfg(test)]
    pub fn frame(&self, lines: Vec<String>, rx_buffer: usize) -> Result<(), String> {
        self.frame_link(lines, LinkProfile::grbl_with_buffer(rx_buffer))
    }

    /// Como [`frame`](Self::frame) con el enlace del perfil de la máquina.
    pub fn frame_link(&self, lines: Vec<String>, link: LinkProfile) -> Result<(), String> {
        self.start(lines, link, None, RunKind::Frame)
    }

    fn start(
        &self,
        lines: Vec<String>,
        link: LinkProfile,
        s_max: Option<f64>,
        kind: RunKind,
    ) -> Result<(), String> {
        self.call(|reply| Request::Start {
            lines,
            link: Box::new(link),
            s_max,
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
        Self::start_with(transport, port, sink, timing, None)
    }

    /// Abre la máquina virtual de un puerto `virtual:...` (ver [`virtual_port`]).
    pub fn start_virtual(
        port: &str,
        sink: Arc<dyn Sink>,
        timing: Timing,
    ) -> Result<(Self, ConnectResult), String> {
        let (transport, emu) = virtual_port::open(port)?;
        Self::start_with(transport, port, sink, timing, Some(emu))
    }

    /// Como [`start`](Self::start), con el mando del emulador si `transport` es uno.
    pub fn start_with(
        transport: Box<dyn Transport>,
        port: &str,
        sink: Arc<dyn Sink>,
        timing: Timing,
        emu: Option<tlaku_emu::Handle>,
    ) -> Result<(Self, ConnectResult), String> {
        let mut grbl = Grbl::with_transport(transport);
        let reply = grbl.command("$I", timing.identify_timeout);
        log(sink.as_ref(), LogDir::Out, "$I");
        // Marlin y Smoothieware no conocen `$I` (`echo:Unknown command` o `error`): se prueba
        // `M115`. Si tampoco contesta, vale el mensaje de siempre.
        let answered_like_grbl = matches!(&reply, Ok(l) if !looks_like_reprap(l));
        let reprap = if answered_like_grbl {
            None
        } else {
            if let Ok(l) = &reply {
                for l in l {
                    log(sink.as_ref(), LogDir::In, l);
                }
            }
            probe_reprap(&mut grbl, sink.as_ref(), timing.identify_timeout)
        };
        let (firmware, family, link_family, emergency_parser) = match reprap {
            Some(r) => (r.name, Firmware::Unknown, r.family, r.emergency_parser),
            None => {
                let lines = reply.map_err(|e| {
                    format!(
                        "La máquina no respondió a la identificación ($I): {e}. Revisa que sea el \
                         puerto correcto, que esté encendida, que use GRBL 1.1 y los baudios del \
                         perfil."
                    )
                })?;
                for l in &lines {
                    log(sink.as_ref(), LogDir::In, l);
                }
                log(sink.as_ref(), LogDir::In, "ok");
                let (name, fw) = identify(&lines);
                (name, fw, LinkFamily::Grbl, false)
            }
        };
        if emu.is_some() {
            log(
                sink.as_ref(),
                LogDir::In,
                "; Máquina virtual (emulador): no hay láser ni hardware reales.",
            );
        }

        let shared = Arc::new(Shared::default());
        let (tx, rx) = mpsc::channel();
        let mut worker = Worker::new(rx, sink.clone(), shared.clone(), timing, port.to_string());
        worker.fw = family;
        worker.link_family = link_family;
        worker.emergency_parser = emergency_parser;
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
                handle: Handle { tx, shared, emu },
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

/// Nombre legible del firmware (`Grbl 1.1h`, `FluidNC v3.9.7`, `grblHAL 1.1f`) y su familia, a
/// partir de las líneas de `$I`. La familia decide cómo se leen los códigos `error:N` y
/// `ALARM:N` extendidos. Se toleran líneas ajenas (`[software versions: ...]`,
/// `inquire_info...` de Creality) y la respuesta vacía.
fn identify(lines: &[String]) -> (String, Firmware) {
    let fw = Firmware::detect(lines);
    let generic = match fw {
        Firmware::GrblHal => "grblHAL",
        Firmware::FluidNc => "FluidNC",
        Firmware::Grbl | Firmware::Unknown => "GRBL",
    };
    let Some(ver) = lines.iter().find_map(|l| l.trim().strip_prefix("[VER:")) else {
        return (generic.into(), fw);
    };
    let ver = ver
        .trim_end_matches(']')
        .split(':')
        .next()
        .unwrap_or("")
        .trim();
    let ver = match ver.rsplit_once('.') {
        Some((head, build)) if build.len() == 8 && build.bytes().all(|b| b.is_ascii_digit()) => {
            head
        }
        _ => ver,
    };
    let name = match fw {
        // `3.9 FluidNC v3.9.7` -> `FluidNC v3.9.7`.
        Firmware::FluidNc => {
            let rest = ver
                .to_ascii_lowercase()
                .find("fluidnc")
                .map_or(ver, |i| &ver[i + "fluidnc".len()..])
                .trim();
            if rest.is_empty() {
                generic.to_string()
            } else {
                format!("{generic} {rest}")
            }
        }
        _ if ver.is_empty() => generic.to_string(),
        Firmware::GrblHal => format!("grblHAL {ver}"),
        Firmware::Grbl | Firmware::Unknown => format!("Grbl {ver}"),
    };
    (name, fw)
}

/// `Grbl 1.1h` a partir de la línea `[VER:1.1h.20190825:]` de `$I`.
#[cfg(test)]
fn firmware_name(lines: &[String]) -> String {
    identify(lines).0
}

/// ¿La respuesta a `$I` es la de un firmware RepRap (Marlin, Smoothieware) que no lo conoce?
fn looks_like_reprap(lines: &[String]) -> bool {
    !lines.iter().any(|l| l.trim_start().starts_with("[VER:"))
        && lines.iter().any(|l| l.starts_with("echo:"))
}

/// Lo que se averigua de Marlin o Smoothieware con `M115`.
struct Reprap {
    name: String,
    family: LinkFamily,
    /// Informa `Cap:EMERGENCY_PARSER:1`: `M0` y `M108` actúan al instante.
    emergency_parser: bool,
}

/// Pregunta `M115` y reconoce Marlin o Smoothieware por su `FIRMWARE_NAME`.
fn probe_reprap(grbl: &mut Grbl, sink: &dyn Sink, timeout: Duration) -> Option<Reprap> {
    log(sink, LogDir::Out, "M115");
    let lines = grbl.command("M115", timeout).ok()?;
    for l in &lines {
        log(sink, LogDir::In, l);
    }
    log(sink, LogDir::In, "ok");
    let info = ReprapInfo::parse(&lines);
    let name = info.firmware_name.clone()?;
    let low = name.to_ascii_lowercase();
    let family = if low.contains("smoothie") {
        LinkFamily::Smoothie
    } else if low.contains("marlin") {
        LinkFamily::Marlin
    } else {
        return None;
    };
    Some(Reprap {
        name,
        family,
        emergency_parser: info.caps.get("EMERGENCY_PARSER").copied().unwrap_or(false),
    })
}

/// `X:12.00 Y:3.50 Z:0.00 E:0.00 Count ...` (respuesta de `M114`) -> [x, y].
fn parse_m114(line: &str) -> Option<[f64; 2]> {
    let field = |key: &str| -> Option<f64> {
        let i = line.find(key)? + key.len();
        let rest = &line[i..];
        let end = rest
            .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+')))
            .unwrap_or(rest.len());
        rest[..end].parse().ok()
    };
    // `Count X:` también contiene "X:": solo vale la primera aparición, que va antes.
    Some([field("X:")?, field("Y:")?])
}

/// ¿Es la confirmación de una línea? GRBL: `ok`. Marlin puede añadir texto (`ok T:20`).
fn is_ack(family: LinkFamily, line: &str) -> bool {
    line == "ok" || (family != LinkFamily::Grbl && line.starts_with("ok "))
}

fn log(sink: &dyn Sink, dir: LogDir, text: &str) {
    sink.emit(Emit::Log(LogEvent {
        dir,
        text: text.to_string(),
    }));
}

/// `error:N`, `ALARM:N` o `ERROR:NN` de Creality con su explicación; el resto, tal cual.
fn describe_line(fw: Firmware, line: &str) -> String {
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
fn is_critical_alarm(line: &str) -> bool {
    matches!(
        line.strip_prefix("ALARM:")
            .and_then(|c| c.trim().parse::<u32>().ok()),
        Some(1 | 2)
    ) || line.contains("Reset to continue")
}

/// Cada cuánto se pide `M114` a una controladora sin `?` (Marlin, Smoothieware) en reposo.
const REPRAP_POLL: Duration = Duration::from_millis(500);

/// Cada cuánto se manda `?` durante un bloqueo crítico (que probablemente no lo contesta).
const CRITICAL_POLL: Duration = Duration::from_secs(2);

/// Por qué se tolera silencio (ver la regla del silencio en la cabecera del módulo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Quiet {
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
    /// Familia del firmware, para leer los códigos extendidos.
    fw: Firmware,
    /// Familia del protocolo: GRBL (`?`, tiempo real) o RepRap (Marlin, Smoothieware: una línea
    /// por `ok`, sin `?`; la posición se consulta con `M114`).
    link_family: LinkFamily,
    /// El Marlin conectado informó `EMERGENCY_PARSER` (pausa con `M0` y `M108`).
    emergency_parser: bool,
    /// Silencio tolerado ahora mismo (ver la cabecera del módulo).
    quiet: Option<Quiet>,
}

fn empty_status() -> StatusReport {
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
        // Estado que Tlaku no conoce (p. ej. `Tool` de grblHAL): ocupada, no en reposo. El
        // nombre original viaja aparte (`other_state`) para que la interfaz lo muestre.
        G::Other => MachineState::Other,
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
            fw: Firmware::Unknown,
            link_family: LinkFamily::Grbl,
            emergency_parser: false,
            quiet: None,
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

    // ---- estado ------------------------------------------------------------

    fn on_status(&mut self, r: &streamer::StatusReport) {
        if let Some(w) = &r.wco {
            self.wco = [w.x, w.y];
        }
        let mut s = self.last.clone();
        s.state = convert_state(r.state);
        s.other_state = r
            .other_state
            .clone()
            .filter(|_| s.state == MachineState::Other);
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
        // GRBL manda `Ov:` solo de vez en cuando: se conserva el último valor visto.
        if let Some(o) = r.overrides {
            s.overrides = Some(Overrides {
                feed: o.feed,
                rapid: o.rapid,
                spindle: o.spindle,
            });
        }
        self.last = s.clone();
        if let Ok(mut g) = self.shared.status.lock() {
            *g = Some(s.clone());
        }
        self.sink.emit(Emit::Status(s));
    }

    /// ¿Se superó el silencio tolerado? Ver la regla en la cabecera del módulo.
    fn silence_exceeded(&self) -> bool {
        // Sin `?` no hay sondeo que contestar: la pérdida se detecta con `M114` y con los
        // errores de E/S.
        if self.link_family != LinkFamily::Grbl {
            return false;
        }
        let elapsed = self.last_rx.elapsed();
        match self.quiet {
            None => elapsed > self.timing.silence_timeout,
            Some(Quiet::Homing) => {
                elapsed > self.timing.silence_timeout.max(self.timing.home_timeout)
            }
            Some(Quiet::CriticalAlarm) => false,
        }
    }

    /// Actualiza el silencio tolerado y la familia del firmware con una línea recibida.
    fn note_line(&mut self, line: &str) {
        if is_critical_alarm(line) {
            self.quiet = Some(Quiet::CriticalAlarm);
        } else if self.quiet == Some(Quiet::CriticalAlarm) && (line == "ok" || is_greeting(line)) {
            self.quiet = None;
        }
        if let Some(fw) = Firmware::from_banner(line) {
            self.fw = fw;
        }
    }

    /// Una vuelta de lectura: sondea `?` si toca, lee hasta `wait` y devuelve las líneas que no
    /// son reportes de estado (esos se emiten y no se registran).
    fn pump(&mut self, t: &mut dyn Transport, wait: Duration) -> Result<Vec<String>, Fail> {
        let interval = if self.quiet == Some(Quiet::CriticalAlarm) {
            self.timing.status_interval.max(CRITICAL_POLL)
        } else {
            self.timing.status_interval
        };
        if self.link_family == LinkFamily::Grbl
            && self.last_poll.is_none_or(|p| p.elapsed() >= interval)
        {
            t.write_all(&[Realtime::StatusQuery.byte()])?;
            self.last_poll = Some(Instant::now());
        }
        let mut buf = [0u8; 256];
        let n = t.read_timeout(&mut buf, wait)?;
        if n > 0 {
            self.last_rx = Instant::now();
            self.reader.feed(&buf[..n]);
        } else if self.silence_exceeded() {
            return Err(Fail::Lost(
                "la controladora dejó de responder (¿se desconectó el cable USB o la red, o se \
                 apagó la máquina?)"
                    .into(),
            ));
        }
        let mut others = Vec::new();
        while let Some(line) = self.reader.next_line() {
            match streamer::StatusReport::parse(&line) {
                Some(r) => self.on_status(&r),
                None => {
                    self.note_line(&line);
                    others.push(line);
                }
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
                    link,
                    s_max,
                    kind,
                    reply,
                } => match self.try_start(t, lines, *link, s_max, kind) {
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
        if self.link_family != LinkFamily::Grbl
            && self
                .last_poll
                .is_none_or(|p| p.elapsed() >= REPRAP_POLL.max(self.timing.status_interval))
        {
            self.poll_reprap(&mut *t)?;
        }
        for line in self.pump(&mut *t, self.timing.tick)? {
            self.log(LogDir::In, &describe_line(self.fw, &line));
        }
        Ok(Phase::Idle(t))
    }

    /// Marlin y Smoothieware no tienen `?`: la posición se pide con `M114` (sin registrar la
    /// consulta) y se informa como estado en reposo. Si no contesta en el plazo largo
    /// (`home_timeout`), la conexión se da por perdida.
    fn poll_reprap(&mut self, t: &mut dyn Transport) -> Result<(), Fail> {
        self.reader.clear();
        t.write_all(b"M114\n")?;
        self.last_poll = Some(Instant::now());
        // Marlin contesta `M114` en su turno, detrás de los movimientos que aún tenga en cola
        // (p. ej. tras cancelar un trabajo largo): el plazo es el largo, como en el homing.
        let deadline = Instant::now() + self.timing.silence_timeout.max(self.timing.home_timeout);
        let mut pos = None;
        loop {
            for l in self.pump(t, self.timing.tick)? {
                if is_ack(self.link_family, &l) {
                    if let Some(p) = pos {
                        self.report_position(p);
                    }
                    return Ok(());
                }
                match parse_m114(&l) {
                    Some(p) => pos = Some(p),
                    None => self.log(LogDir::In, &l),
                }
            }
            if Instant::now() > deadline {
                return Err(Fail::Lost(
                    "la controladora dejó de responder (¿se desconectó el cable USB o la red, o se \
                     apagó la máquina?)"
                        .into(),
                ));
            }
        }
    }

    /// Estado de una controladora sin `?`: la posición de `M114` y el estado que sabe el
    /// anfitrión (en marcha, en pausa o en reposo).
    fn report_position(&mut self, p: [f64; 2]) {
        self.last.machine_pos = p;
        self.last.work_pos = p;
        self.publish_last();
    }

    /// Cambia el estado informado (solo para controladoras sin `?`) y lo emite.
    fn set_reported_state(&mut self, state: MachineState) {
        if self.link_family != LinkFamily::Grbl {
            self.last.state = state;
            self.publish_last();
        }
    }

    fn publish_last(&mut self) {
        let s = self.last.clone();
        if let Ok(mut g) = self.shared.status.lock() {
            *g = Some(s.clone());
        }
        self.sink.emit(Emit::Status(s));
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
            Request::JogTo { x, y, speed, reply } => {
                (reply, self.jog_to(t, x, y, speed).map(|()| vec![]))
            }
            Request::Override {
                kind,
                percent,
                reply,
            } => (reply, self.override_idle(t, kind, percent).map(|()| vec![])),
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
        if self.link_family != LinkFamily::Grbl {
            // Marlin y Smoothieware no tienen bytes en tiempo real: pausar y cancelar son cosas
            // del anfitrión durante un trabajo (ver `stream_request`). Fuera de él no hay nada
            // que detener.
            return match cmd {
                RealtimeCommand::Status | RealtimeCommand::JogCancel => Ok(()),
                RealtimeCommand::Hold | RealtimeCommand::Resume | RealtimeCommand::Reset => {
                    Err(Fail::Msg(
                        "Esta controladora no tiene comandos en tiempo real: pausa y cancela \
                         desde el trabajo en curso."
                            .into(),
                    ))
                }
            };
        }
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
        // Un reset en pleno movimiento (o homing) pierde la posición: GRBL lanzará ALARM:3.
        if rt == Realtime::SoftReset
            && matches!(
                self.last.state,
                MachineState::Run | MachineState::Jog | MachineState::Home
            )
        {
            self.log(LogDir::In, &format!("; {POSITION_LOST_NOTE}"));
        }
        Ok(())
    }

    /// Envía una línea y espera `ok`, `error:N` o `ALARM:N`. Devuelve todas las líneas de
    /// respuesta, la última incluida (los errores con su explicación).
    fn command(&mut self, t: &mut dyn Transport, raw: &str) -> Result<Vec<String>, Fail> {
        // GRBL ignora los espacios y se quitan para ahorrar búfer; Marlin y Smoothie los
        // necesitan para separar parámetros (por ejemplo `M3 S0 I`).
        let grbl = self.link_family == LinkFamily::Grbl;
        let line = prepare_line_with(raw, grbl, grbl).map_err(|ch| {
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
        // Durante el homing `?` puede no contestarse: se tolera el silencio hasta `home_timeout`.
        if is_homing_line(line) && self.quiet.is_none() {
            self.quiet = Some(Quiet::Homing);
        }
        let r = self.exchange_inner(t, line, timeout);
        if self.quiet == Some(Quiet::Homing) {
            self.quiet = None;
        }
        r
    }

    fn exchange_inner(
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
                let shown = describe_line(self.fw, &l);
                self.log(LogDir::In, &shown);
                if is_ack(self.link_family, &l) {
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
        if self.link_family != LinkFamily::Grbl {
            return self.jog_reprap(t, dx, dy, speed);
        }
        let line = format!("$J=G91 G21 X{} Y{} F{}", num(dx), num(dy), num(speed));
        let reply = self.exchange(t, &line, self.timing.command_timeout)?;
        match reply.last().map(String::as_str) {
            Some("ok") => Ok(()),
            Some(e) => Err(Fail::Msg(jog_error(e))),
            None => Err(Fail::Msg("La máquina no respondió al movimiento.".into())),
        }
    }

    /// Movimiento absoluto: `$J=G90` en GRBL; en Marlin y Smoothieware `G90` y un `G1`.
    fn jog_to(&mut self, t: &mut dyn Transport, x: f64, y: f64, speed: f64) -> Result<(), Fail> {
        if !(x.is_finite() && y.is_finite()) {
            return Err(Fail::Msg("La posición de destino no es un número.".into()));
        }
        if !(speed.is_finite() && speed > 0.0) {
            return Err(Fail::Msg(
                "La velocidad del movimiento debe ser mayor que 0.".into(),
            ));
        }
        // `M5` primero: el láser va apagado aunque una orden de la consola lo hubiera dejado
        // encendido (el jog no lo apaga).
        let lines = if self.link_family == LinkFamily::Grbl {
            vec![
                "M5".to_string(),
                format!("$J=G90 G21 X{} Y{} F{}", num(x), num(y), num(speed)),
            ]
        } else {
            vec![
                "M5".to_string(),
                "G90".to_string(),
                format!("G1 X{} Y{} F{}", num(x), num(y), num(speed)),
            ]
        };
        let mut failure = None;
        for line in &lines {
            let reply = self.exchange(t, line, self.timing.command_timeout)?;
            if failure.is_none()
                && let Some(last) = reply.last()
                && !is_ack(self.link_family, last)
            {
                failure = Some(last.clone());
            }
        }
        match failure {
            None => Ok(()),
            Some(e) => Err(Fail::Msg(jog_error(&e))),
        }
    }

    /// Plan de bytes para llevar el override de `kind` a `percent` desde lo último que informó
    /// la controladora. Falla fuera de GRBL, que no tiene overrides.
    fn override_bytes(&self, kind: OverrideKind, percent: u32) -> Result<Vec<Realtime>, Fail> {
        if self.link_family != LinkFamily::Grbl {
            return Err(Fail::Msg(
                "Los ajustes en vivo de velocidad y potencia solo existen en controladoras GRBL:                  Marlin y Smoothieware no los tienen."
                    .into(),
            ));
        }
        let current = self.last.overrides.map(|o| match kind {
            OverrideKind::Feed => o.feed,
            OverrideKind::Spindle => o.spindle,
        });
        Ok(override_plan(kind, current, percent))
    }

    /// Registra el override y recuerda el valor pedido hasta el próximo `Ov:` de la controladora.
    fn note_override(&mut self, kind: OverrideKind, percent: u32) {
        let percent = percent.clamp(streamer::OVERRIDE_MIN, streamer::OVERRIDE_MAX);
        let (name, what) = match kind {
            OverrideKind::Feed => ("velocidad", "feed"),
            OverrideKind::Spindle => ("potencia", "spindle"),
        };
        self.log(
            LogDir::Out,
            &format!("; override de {name} ({what}) al {percent} %"),
        );
        let mut o = self.last.overrides.unwrap_or(Overrides {
            feed: 100,
            rapid: 100,
            spindle: 100,
        });
        match kind {
            OverrideKind::Feed => o.feed = percent,
            OverrideKind::Spindle => o.spindle = percent,
        }
        self.last.overrides = Some(o);
        self.publish_last();
    }

    /// Override fuera de un trabajo: el trabajador escribe los bytes en el puerto.
    fn override_idle(
        &mut self,
        t: &mut dyn Transport,
        kind: OverrideKind,
        percent: u32,
    ) -> Result<(), Fail> {
        let plan = self.override_bytes(kind, percent)?;
        for r in &plan {
            t.write_all(&[r.byte()])?;
        }
        self.note_override(kind, percent);
        Ok(())
    }

    /// Movimiento relativo sin `$J=` (solo GRBL): `G91`, el movimiento y vuelta a `G90`.
    fn jog_reprap(
        &mut self,
        t: &mut dyn Transport,
        dx: f64,
        dy: f64,
        speed: f64,
    ) -> Result<(), Fail> {
        let lines = [
            "G91".to_string(),
            format!("G1 X{} Y{} F{}", num(dx), num(dy), num(speed)),
            "G90".to_string(),
        ];
        let mut failure = None;
        for line in &lines {
            let reply = self.exchange(t, line, self.timing.command_timeout)?;
            if failure.is_none()
                && let Some(last) = reply.last()
                && !is_ack(self.link_family, last)
            {
                failure = Some(last.clone());
            }
        }
        match failure {
            None => Ok(()),
            Some(e) => Err(Fail::Msg(jog_error(&e))),
        }
    }

    // ---- inicio de trabajo -------------------------------------------------

    fn try_start(
        &mut self,
        mut t: Box<dyn Transport>,
        lines: Vec<String>,
        link: LinkProfile,
        s_max: Option<f64>,
        kind: RunKind,
    ) -> Result<Run, StartFail> {
        // El perfil de la máquina debe corresponder a la controladora conectada.
        if link.family != self.link_family {
            let msg = format!(
                "el perfil de la máquina es de {} pero la controladora conectada es {}. Elige el \
                 perfil que corresponde a su firmware. No se transmitió nada.",
                link.family_name(),
                match self.link_family {
                    LinkFamily::Grbl => "GRBL",
                    LinkFamily::Marlin => "Marlin",
                    LinkFamily::Smoothie => "Smoothieware",
                }
            );
            return Err(StartFail::Refused(t, capitalize(&msg)));
        }
        // Con el `EMERGENCY_PARSER` informado, la pausa de Marlin también manda `M0` y `M108`.
        let link = if link.family == LinkFamily::Marlin {
            link.with_emergency_parser(self.emergency_parser)
        } else {
            link
        };
        // Mismo orden que `tlaku send`: validar, comprobar el estado y solo entonces enviar.
        if let Err(e) = validate_lines(&lines, &link) {
            return Err(StartFail::Refused(t, e));
        }
        self.reader.clear();
        let mut shown = Vec::new();
        // El encuadre va con el láser apagado (M5, S0): no necesita el modo láser.
        let expect = JobExpect {
            require_laser_mode: kind == RunKind::Job,
            s_max,
            check_parameters: true,
        };
        let checked = preflight_link(
            &mut t,
            &self.timing.preflight,
            Advice::Console,
            &expect,
            &link,
            &mut shown,
        );
        // Los avisos ("Aviso: $30...") no bloquean, pero se dejan en el registro.
        for w in String::from_utf8_lossy(&shown)
            .lines()
            .filter(|l| l.starts_with("Aviso"))
        {
            self.log(LogDir::In, w);
        }
        if let Err(msg) = checked {
            self.log(LogDir::In, &first_line(&msg));
            // Un fallo de E/S en la consulta no se distingue aquí de una máquina muda: el
            // sondeo normal detectará la pérdida si es real.
            return Err(StartFail::Refused(t, msg));
        }
        self.last_poll = None;
        let total = lines.len();
        let config = StreamerConfig {
            silence_timeout: self.timing.silence_timeout,
            home_timeout: self.timing.home_timeout,
            hold_timeout: self.timing.hold_timeout,
            reset_wait: self.timing.reset_wait,
            ..StreamerConfig::for_link(&link)
        };
        match Streamer::with_link(t, config, link).spawn(&lines) {
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
                self.set_reported_state(MachineState::Run);
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
                    // Marlin y Smoothieware ignoran los bytes en tiempo real: se cancela el trabajo.
                    RealtimeCommand::Reset if self.link_family != LinkFamily::Grbl => {
                        self.cancel(run);
                    }
                    RealtimeCommand::Reset => {
                        run.cancelling = true;
                        run.job.control.realtime(Realtime::SoftReset);
                    }
                    RealtimeCommand::JogCancel if self.link_family != LinkFamily::Grbl => {}
                    RealtimeCommand::JogCancel => run.job.control.realtime(Realtime::JogCancel),
                }
                ok(reply);
            }
            // Los overrides son justo para esto: se mandan por el mando del trabajo en curso.
            Request::Override {
                kind,
                percent,
                reply,
            } => {
                let r = self.override_bytes(kind, percent).map(|plan| {
                    for r in plan {
                        run.job.control.realtime(r);
                    }
                    self.note_override(kind, percent);
                });
                let _ = reply.send(match r {
                    Ok(()) => Ok(Vec::new()),
                    Err(Fail::Msg(m) | Fail::Lost(m)) => Err(m),
                });
            }
            Request::Command { reply, .. }
            | Request::Jog { reply, .. }
            | Request::JogTo { reply, .. }
            | Request::Start { reply, .. } => {
                let _ = reply.send(Err(busy.into()));
            }
            Request::Disconnect => unreachable!("se atiende antes"),
        }
    }

    fn pause(&mut self, run: &Run) {
        run.job.control.pause();
        self.set_reported_state(MachineState::Hold);
        if run.kind == RunKind::Job {
            self.job_event(JobState::Paused, "En pausa.");
        }
    }

    fn resume(&mut self, run: &Run) {
        run.job.control.resume();
        self.set_reported_state(MachineState::Run);
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
                    &format!("error:{code}{at}: {}", error_message_for(self.fw, code)),
                );
            }
            Event::Alarm(code) => {
                self.log(
                    LogDir::In,
                    &format!("ALARM:{code}: {}", alarm_message_for(self.fw, code)),
                );
            }
            Event::Reset(m) => {
                self.note_line(&m);
                self.log(LogDir::In, &m);
            }
            Event::Message(m) => {
                self.note_line(&m);
                self.log(LogDir::In, &m);
            }
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
        // Si la máquina se detuvo (cancelación, error, falla de Creality), cómo quedó.
        // `ALARM:3` es justo eso: reinicio con la máquina en movimiento.
        let position_lost = streamer.last_stop().is_some_and(|s| s.position_maybe_lost)
            || matches!(outcome, Outcome::Alarm(3));
        let t = streamer.into_transport();
        self.reader.clear();
        self.last_poll = None;
        self.last_rx = Instant::now();
        // Un ALARM:1/2 que terminó el trabajo deja la placa bloqueada y callada hasta un
        // reset; tras cualquier otro final ya no hay bloqueo que esperar.
        self.quiet = matches!(outcome, Outcome::Alarm(1 | 2)).then_some(Quiet::CriticalAlarm);
        self.set_reported_state(MachineState::Idle);
        match (&outcome, kind) {
            (Outcome::Completed { errors: 0 }, RunKind::Job) => {
                self.job_event(JobState::Finished, "Trabajo terminado.");
            }
            (Outcome::Completed { errors: 0 }, RunKind::Frame) => {
                self.log(LogDir::In, "; encuadre terminado");
            }
            (Outcome::Cancelled, RunKind::Job) => {
                let mut msg = String::from(
                    "Trabajo cancelado: se envió pausa, se esperó a que frenara y se reinició; \
                     el láser quedó apagado.",
                );
                if position_lost {
                    msg.push(' ');
                    msg.push_str(POSITION_LOST_NOTE);
                }
                self.job_event_at(JobState::Cancelled, &msg, position_lost);
            }
            (Outcome::Cancelled, RunKind::Frame) => {
                self.log(LogDir::In, "; encuadre cancelado");
                if position_lost {
                    self.log(LogDir::In, &format!("; {POSITION_LOST_NOTE}"));
                }
            }
            _ => {
                let mut msg = outcome_result_for(self.fw, &outcome)
                    .err()
                    .unwrap_or_else(|| "el envío terminó con errores".into());
                if position_lost {
                    msg.push(' ');
                    msg.push_str(POSITION_LOST_NOTE);
                }
                self.log(LogDir::In, &first_line(&msg));
                self.job_event_at(JobState::Error, &msg, position_lost);
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
fn validate_lines(lines: &[String], link: &LinkProfile) -> Result<(), String> {
    validate_gcode_for(&lines.join("\n"), link).map(drop)
}

/// Primera letra en mayúscula.
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
