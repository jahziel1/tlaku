//! Automatización por UDP (M54): un servidor local opcional para que otros programas o scripts
//! manejen Tlaku con comandos de texto. Protocolo documentado en `docs/app-api.md`.
//!
//! # Seguridad
//!
//! - Está **apagado por omisión**; se activa en Preferencias.
//! - Solo escucha en `127.0.0.1` (nunca en la red) y en un puerto sin privilegios.
//! - `START` exige que la máquina esté conectada y en reposo, y además la confirmación de la
//!   interfaz (el diálogo de inicio de siempre) salvo que el usuario haya activado
//!   "permitir iniciar desde automatización", que también está apagada por omisión.
//! - `LOAD` solo acepta rutas absolutas de archivos que existen; la interfaz los abre con su
//!   importador de siempre. No hay forma de leer ni escribir archivos desde aquí.
//! - `STOP` siempre se atiende (detener nunca es peligroso).
//!
//! La interfaz es dueña del documento y de las opciones del trabajo: por eso `LOAD` y `START` no
//! se ejecutan aquí sino que se piden a la ventana con los eventos `automation://load` y
//! `automation://start`.

use crate::commands::AppState;
use crate::machine::NOT_CONNECTED;
use serde::{Deserialize, Serialize};
use std::io;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tlaku_doc::{MachineState, StatusReport};

/// Primer puerto aceptado: por debajo de 1024 hacen falta privilegios.
pub const MIN_PORT: u16 = 1024;
/// Tamaño máximo de un comando; más largo se rechaza.
const MAX_DATAGRAM: usize = 4096;

// ---------------------------------------------------------------------------
// Protocolo
// ---------------------------------------------------------------------------

/// Lo que el servidor necesita de la aplicación (en las pruebas es un doble).
pub trait Backend: Send + Sync + 'static {
    /// Último estado de la máquina; `None` si no hay conexión.
    fn machine(&self) -> Option<StatusReport>;
    /// Detiene el trabajo en curso (como el botón de parar).
    fn stop(&self) -> Result<(), String>;
    /// Pide a la interfaz abrir `path`.
    fn load(&self, path: &str);
    /// Pide a la interfaz iniciar el trabajo. Con `confirmed == false` la interfaz muestra su
    /// diálogo de inicio y el usuario decide.
    fn start(&self, confirmed: bool);
}

fn state_name(s: MachineState) -> String {
    format!("{s:?}").to_ascii_lowercase()
}

/// Ejecuta una línea de comando y devuelve la respuesta (una sola línea, sin salto final).
pub fn execute(line: &str, backend: &dyn Backend, allow_start: bool) -> String {
    let line = line.trim();
    let (word, rest) = match line.split_once(char::is_whitespace) {
        Some((w, r)) => (w, r.trim()),
        None => (line, ""),
    };
    match word.to_ascii_uppercase().as_str() {
        "PING" => "PONG".into(),
        "STATUS" => match backend.machine() {
            None => "STATUS connected=0".into(),
            Some(s) => format!(
                "STATUS connected=1 state={} x={:.3} y={:.3}",
                state_name(s.state),
                s.work_pos[0],
                s.work_pos[1]
            ),
        },
        "STOP" => match backend.machine() {
            None => err("not_connected", NOT_CONNECTED),
            Some(_) => match backend.stop() {
                Ok(()) => "OK".into(),
                Err(e) => err("failed", &e),
            },
        },
        "LOAD" => load(rest, backend),
        "START" => start(backend, allow_start),
        "" => err("empty", "Comando vacío."),
        _ => err(
            "unknown_command",
            "Comando desconocido. Disponibles: PING, LOAD <ruta>, START, STATUS, STOP.",
        ),
    }
}

fn err(code: &str, text: &str) -> String {
    format!("ERR {code}: {text}")
}

fn load(path: &str, backend: &dyn Backend) -> String {
    if path.is_empty() {
        return err("bad_argument", "Falta la ruta: LOAD <ruta absoluta>.");
    }
    if path.chars().any(char::is_control) {
        return err("bad_argument", "La ruta tiene caracteres de control.");
    }
    let p = Path::new(path);
    if !p.is_absolute() {
        return err("bad_argument", "La ruta debe ser absoluta.");
    }
    if !p.is_file() {
        return err("not_found", "No existe ese archivo.");
    }
    backend.load(path);
    "OK loading".into()
}

fn start(backend: &dyn Backend, allow_start: bool) -> String {
    let Some(status) = backend.machine() else {
        return err("not_connected", NOT_CONNECTED);
    };
    if status.state != MachineState::Idle {
        return err(
            "busy",
            &format!(
                "La máquina no está en reposo (estado: {}).",
                state_name(status.state)
            ),
        );
    }
    if allow_start {
        backend.start(true);
        "OK starting".into()
    } else {
        backend.start(false);
        "PENDING confirm_in_app: el usuario debe confirmar el inicio en Tlaku.".into()
    }
}

// ---------------------------------------------------------------------------
// Servidor UDP
// ---------------------------------------------------------------------------

/// Servidor UDP en marcha. Soltarlo lo detiene y libera el puerto.
pub struct Server {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    /// Escucha en `127.0.0.1:port` (0 = un puerto libre, para las pruebas).
    pub fn start(
        port: u16,
        backend: Arc<dyn Backend>,
        allow_start: Arc<AtomicBool>,
    ) -> io::Result<Self> {
        let socket = UdpSocket::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))?;
        socket.set_read_timeout(Some(Duration::from_millis(100)))?;
        let addr = socket.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = thread::Builder::new()
            .name("tlaku-automation".into())
            .spawn(move || serve(&socket, backend.as_ref(), &allow_start, &flag))?;
        Ok(Self {
            addr,
            stop,
            thread: Some(thread),
        })
    }

    #[cfg(test)]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn port(&self) -> u16 {
        self.addr.port()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn serve(socket: &UdpSocket, backend: &dyn Backend, allow_start: &AtomicBool, stop: &AtomicBool) {
    // Un byte más que el máximo para distinguir "cabe justo" de "se pasó".
    let mut buf = vec![0u8; MAX_DATAGRAM + 1];
    while !stop.load(Ordering::SeqCst) {
        let (n, from) = match socket.recv_from(&mut buf) {
            Ok(r) => r,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            // En Windows un ICMP de "puerto cerrado" de un cliente que se fue llega como error.
            Err(_) => continue,
        };
        let reply = if n > MAX_DATAGRAM {
            err("too_long", "El comando es demasiado largo.")
        } else {
            match std::str::from_utf8(&buf[..n]) {
                Err(_) => err("bad_text", "El comando no es texto UTF-8."),
                Ok(text) => {
                    let text = text.trim_end_matches(['\r', '\n']);
                    if text.contains(['\r', '\n']) {
                        err("bad_text", "Un comando por mensaje, en una sola línea.")
                    } else {
                        execute(text, backend, allow_start.load(Ordering::SeqCst))
                    }
                }
            }
        };
        let _ = socket.send_to(format!("{reply}\n").as_bytes(), from);
    }
}

// ---------------------------------------------------------------------------
// Aplicación (Tauri)
// ---------------------------------------------------------------------------

/// Eventos hacia la ventana.
pub const EVENT_LOAD: &str = "automation://load";
pub const EVENT_START: &str = "automation://start";

#[derive(Debug, Clone, Serialize)]
struct LoadEvent {
    path: String,
}

#[derive(Debug, Clone, Serialize)]
struct StartEvent {
    /// `true`: el usuario permitió iniciar desde automatización; no hay que preguntar.
    confirmed: bool,
}

struct AppBackend(AppHandle);

impl Backend for AppBackend {
    fn machine(&self) -> Option<StatusReport> {
        self.0
            .state::<AppState>()
            .machine()
            .ok()
            .and_then(|m| m.status_report())
    }

    fn stop(&self) -> Result<(), String> {
        self.0.state::<AppState>().machine()?.cancel()
    }

    fn load(&self, path: &str) {
        let _ = self.0.emit(
            EVENT_LOAD,
            LoadEvent {
                path: path.to_string(),
            },
        );
    }

    fn start(&self, confirmed: bool) {
        let _ = self.0.emit(EVENT_START, StartEvent { confirmed });
    }
}

/// Lo que la interfaz configura desde Preferencias.
#[derive(Debug, Clone, Deserialize)]
pub struct AutomationConfig {
    pub enabled: bool,
    pub port: u16,
    /// Permitir `START` sin confirmación en la interfaz.
    pub allow_start: bool,
}

/// Estado del servidor, que la interfaz muestra en Preferencias.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AutomationStatus {
    pub running: bool,
    pub port: u16,
    /// Por qué no pudo arrancar (puerto ocupado, etc.).
    pub error: Option<String>,
}

/// Estado administrado por Tauri: el servidor (si está encendido).
#[derive(Default)]
pub struct AutomationState {
    server: Mutex<Option<Server>>,
    allow_start: Arc<AtomicBool>,
}

impl AutomationState {
    /// Aplica la configuración: enciende, apaga o cambia de puerto el servidor.
    pub fn configure(
        &self,
        config: &AutomationConfig,
        backend: Arc<dyn Backend>,
    ) -> Result<AutomationStatus, String> {
        self.allow_start.store(config.allow_start, Ordering::SeqCst);
        let mut slot = self.server.lock().map_err(|_| "estado interno dañado")?;
        if !config.enabled {
            *slot = None;
            return Ok(AutomationStatus {
                running: false,
                port: config.port,
                error: None,
            });
        }
        if config.port < MIN_PORT {
            *slot = None;
            return Ok(AutomationStatus {
                running: false,
                port: config.port,
                error: Some(format!(
                    "El puerto debe estar entre {MIN_PORT} y 65535 (los menores necesitan \
                     privilegios)."
                )),
            });
        }
        if let Some(s) = slot.as_ref()
            && s.port() == config.port
        {
            return Ok(AutomationStatus {
                running: true,
                port: config.port,
                error: None,
            });
        }
        // Se suelta el servidor anterior antes de abrir el puerto nuevo.
        *slot = None;
        match Server::start(config.port, backend, self.allow_start.clone()) {
            Ok(server) => {
                let port = server.port();
                *slot = Some(server);
                Ok(AutomationStatus {
                    running: true,
                    port,
                    error: None,
                })
            }
            Err(e) => Ok(AutomationStatus {
                running: false,
                port: config.port,
                error: Some(if e.kind() == io::ErrorKind::AddrInUse {
                    format!(
                        "El puerto {} ya lo usa otro programa; elige otro.",
                        config.port
                    )
                } else {
                    format!("No se pudo abrir el puerto {}: {e}", config.port)
                }),
            }),
        }
    }
}

/// Enciende, apaga o reconfigura la automatización por UDP.
#[tauri::command]
pub fn automation_configure(
    app: AppHandle,
    state: State<'_, AutomationState>,
    config: AutomationConfig,
) -> Result<AutomationStatus, String> {
    state.configure(&config, Arc::new(AppBackend(app)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    #[derive(Default)]
    struct Fake {
        status: StdMutex<Option<StatusReport>>,
        calls: StdMutex<Vec<String>>,
    }

    fn report(state: MachineState) -> StatusReport {
        StatusReport {
            state,
            other_state: None,
            machine_pos: [0.0, 0.0],
            work_pos: [12.5, 3.0],
            feed: 0.0,
            spindle: 0.0,
            buffer: tlaku_doc::BufferStatus {
                planner: 15,
                rx: 127,
            },
            overrides: None,
        }
    }

    impl Fake {
        fn connected(state: MachineState) -> Arc<Self> {
            let f = Arc::new(Self::default());
            *f.status.lock().unwrap() = Some(report(state));
            f
        }
        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl Backend for Fake {
        fn machine(&self) -> Option<StatusReport> {
            self.status.lock().unwrap().clone()
        }
        fn stop(&self) -> Result<(), String> {
            self.calls.lock().unwrap().push("stop".into());
            Ok(())
        }
        fn load(&self, path: &str) {
            self.calls.lock().unwrap().push(format!("load {path}"));
        }
        fn start(&self, confirmed: bool) {
            self.calls
                .lock()
                .unwrap()
                .push(format!("start {confirmed}"));
        }
    }

    #[test]
    fn ping_is_case_insensitive_and_trims() {
        let f = Fake::default();
        assert_eq!(execute("PING", &f, false), "PONG");
        assert_eq!(execute("  ping \r\n", &f, false), "PONG");
    }

    #[test]
    fn status_without_and_with_machine() {
        let f = Fake::default();
        assert_eq!(execute("STATUS", &f, false), "STATUS connected=0");
        let f = Fake::connected(MachineState::Run);
        assert_eq!(
            execute("status", &*f, false),
            "STATUS connected=1 state=run x=12.500 y=3.000"
        );
    }

    #[test]
    fn start_needs_a_connected_idle_machine() {
        let f = Fake::default();
        assert!(execute("START", &f, true).starts_with("ERR not_connected"));
        assert!(f.calls().is_empty());
        for busy in [MachineState::Run, MachineState::Hold, MachineState::Alarm] {
            let f = Fake::connected(busy);
            assert!(execute("START", &*f, true).starts_with("ERR busy"));
            assert!(f.calls().is_empty(), "{busy:?}");
        }
    }

    #[test]
    fn start_asks_for_confirmation_unless_allowed() {
        let f = Fake::connected(MachineState::Idle);
        assert!(execute("START", &*f, false).starts_with("PENDING confirm_in_app"));
        assert_eq!(f.calls(), ["start false"]);
        let f = Fake::connected(MachineState::Idle);
        assert_eq!(execute("START", &*f, true), "OK starting");
        assert_eq!(f.calls(), ["start true"]);
    }

    #[test]
    fn stop_requires_a_connection_but_not_a_preference() {
        let f = Fake::default();
        assert!(execute("STOP", &f, false).starts_with("ERR not_connected"));
        let f = Fake::connected(MachineState::Run);
        assert_eq!(execute("STOP", &*f, false), "OK");
        assert_eq!(f.calls(), ["stop"]);
    }

    #[test]
    fn load_validates_the_path() {
        let f = Fake::default();
        assert!(execute("LOAD", &f, false).starts_with("ERR bad_argument"));
        assert!(execute("LOAD relativa/diseno.svg", &f, false).starts_with("ERR bad_argument"));
        let missing = std::env::temp_dir().join("tlaku-no-existe-8f3a.svg");
        assert!(
            execute(&format!("LOAD {}", missing.display()), &f, false).starts_with("ERR not_found")
        );
        let dir = std::env::temp_dir().join("tlaku-auto-load-test");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("mi diseño.svg");
        std::fs::write(&file, "<svg/>").unwrap();
        // La ruta lleva un espacio: se toma el resto de la línea entero.
        assert_eq!(
            execute(&format!("load {}", file.display()), &f, false),
            "OK loading"
        );
        assert_eq!(f.calls(), [format!("load {}", file.display())]);
        // Un directorio no es un archivo.
        assert!(
            execute(&format!("LOAD {}", dir.display()), &f, false).starts_with("ERR not_found")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_and_empty_commands_are_errors() {
        let f = Fake::default();
        assert!(execute("FORMAT", &f, true).starts_with("ERR unknown_command"));
        assert!(execute("", &f, true).starts_with("ERR empty"));
        assert!(f.calls().is_empty());
    }

    fn ask(socket: &UdpSocket, port: u16, text: &[u8]) -> String {
        socket.send_to(text, (Ipv4Addr::LOCALHOST, port)).unwrap();
        let mut buf = [0u8; 512];
        let (n, _) = socket.recv_from(&mut buf).expect("sin respuesta");
        String::from_utf8_lossy(&buf[..n]).into_owned()
    }

    fn client() -> UdpSocket {
        let s = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        s
    }

    #[test]
    fn udp_server_answers_and_rejects_bad_datagrams() {
        let f = Fake::connected(MachineState::Idle);
        let allow = Arc::new(AtomicBool::new(false));
        let server = Server::start(0, f.clone(), allow.clone()).unwrap();
        let c = client();
        assert_eq!(ask(&c, server.port(), b"PING"), "PONG\n");
        assert_eq!(ask(&c, server.port(), b"PING\r\n"), "PONG\n");
        assert!(ask(&c, server.port(), b"PING\nSTART").starts_with("ERR bad_text"));
        assert!(ask(&c, server.port(), &[0xff, 0xfe]).starts_with("ERR bad_text"));
        assert!(ask(&c, server.port(), &vec![b'A'; MAX_DATAGRAM + 10]).starts_with("ERR too_long"));
        // Sin la preferencia, START solo pide confirmación; con ella, inicia.
        assert!(ask(&c, server.port(), b"START").starts_with("PENDING"));
        allow.store(true, Ordering::SeqCst);
        assert_eq!(ask(&c, server.port(), b"START"), "OK starting\n");
        assert_eq!(f.calls(), ["start false", "start true"]);
    }

    #[test]
    fn server_listens_only_on_loopback_and_frees_the_port_on_drop() {
        let f = Arc::new(Fake::default());
        let server = Server::start(0, f.clone(), Arc::default()).unwrap();
        assert!(server.addr().ip().is_loopback());
        let port = server.port();
        // El puerto está tomado mientras el servidor vive...
        assert!(UdpSocket::bind((Ipv4Addr::LOCALHOST, port)).is_err());
        drop(server);
        // ...y se libera al soltarlo.
        UdpSocket::bind((Ipv4Addr::LOCALHOST, port)).unwrap();
    }

    #[test]
    fn configure_starts_stops_moves_and_reports_errors() {
        let f = Arc::new(Fake::default());
        let state = AutomationState::default();
        let cfg = |enabled, port, allow_start| AutomationConfig {
            enabled,
            port,
            allow_start,
        };
        // Apagada por omisión: no escucha.
        let s = state.configure(&cfg(false, 0, false), f.clone()).unwrap();
        assert!(!s.running);
        // Puertos con privilegios: error claro, sin arrancar.
        let s = state.configure(&cfg(true, 80, false), f.clone()).unwrap();
        assert!(!s.running && s.error.unwrap().contains("1024"));
        // Puerto ocupado por otro programa.
        let busy = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let busy_port = busy.local_addr().unwrap().port();
        let s = state
            .configure(&cfg(true, busy_port, false), f.clone())
            .unwrap();
        assert!(!s.running && s.error.unwrap().contains("ya lo usa otro programa"));
        drop(busy);
        // Se enciende y responde; cambiar la preferencia de START no reinicia el servidor.
        let s = state
            .configure(&cfg(true, busy_port, false), f.clone())
            .unwrap();
        assert!(s.running && s.port == busy_port);
        let c = client();
        assert_eq!(ask(&c, busy_port, b"PING"), "PONG\n");
        let s = state
            .configure(&cfg(true, busy_port, true), f.clone())
            .unwrap();
        assert!(s.running);
        assert!(state.allow_start.load(Ordering::SeqCst));
        // Apagarla libera el puerto.
        let s = state
            .configure(&cfg(false, busy_port, true), f.clone())
            .unwrap();
        assert!(!s.running);
        UdpSocket::bind((Ipv4Addr::LOCALHOST, busy_port)).unwrap();
    }
}
