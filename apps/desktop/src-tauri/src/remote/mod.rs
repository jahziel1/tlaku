//! Control remoto desde el celular por la red local (ola 5.5).
//!
//! Un servidor HTTP opcional deja ver y controlar el trabajo desde el navegador del celular, sin
//! instalar nada: la PC muestra un código QR con la dirección y una clave aleatoria, el celular lo
//! escanea y abre una página ligera con el estado, el avance, el recorrido, la cámara y los botones.
//!
//! # Seguridad
//!
//! - **Apagado de fábrica.** Se activa en Preferencias, sección Control remoto.
//! - Solo la **red local**: escucha en una sola dirección privada (la que eliges o la recomendada),
//!   nunca en `0.0.0.0`, y rechaza cualquier petición cuyo origen no sea una dirección local.
//! - Cada petición exige la clave (cookie de sesión tras abrir el QR, o encabezado). Los intentos
//!   fallidos se limitan y la clave se puede regenerar, lo que expulsa a todos los clientes. La PC
//!   lista quién está conectado y permite desconectarlo. Detalle en `auth` y `server`.
//! - **Pausar, reanudar y detener** se atienden siempre (el galvo no tiene pausa). **Iniciar** exige
//!   el checklist del celular y que el trabajo se haya encuadrado en la PC desde el último cambio
//!   (ver `hub`); el inicio real lo ejecuta la interfaz con su ruta de siempre.
//! - No hay HTTPS: en una red local no se puede obtener un certificado de confianza. Por eso la
//!   clave viaja en el fragmento de la URL (que no sale del celular) y la sesión es una cookie
//!   aparte. Quien pueda espiar la red local (Wi-Fi sin cifrar) podría ver el tráfico: usa solo
//!   redes de confianza.
//!
//! # Piezas
//!
//! - [`net`]: direcciones locales y qué pares se aceptan.
//! - [`auth`]: clave, sesiones, límite de intentos, clientes.
//! - [`hub`]: estado compartido y reglas (qué se puede hacer y cuándo).
//! - [`server`]: HTTP con `axum` (la página va embebida con `include_str!`; no depende de Vite).
//!
//! La ventana de la PC informa su estado con `remote_update` (progreso, tiempo restante, capa,
//! cabezal, encuadre) a partir de los eventos `machine://*` que ya recibe; la cámara llega como un
//! JPEG reducido cada pocos segundos, solo si hay celulares conectados. Protocolo en
//! `docs/app-api.md`.

pub mod auth;
pub mod hub;
pub mod net;
pub mod server;

#[cfg(test)]
mod tests;

use crate::commands::AppState;
use auth::ClientView;
use base64::Engine;
use hub::{Control, Core, MachineInfo, MachineKind, RouteIn, Snapshot, StartOutcome, StartRequest};
use net::LocalAddr;
use serde::{Deserialize, Serialize};
use server::Server;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, State};
use tlaku_codes::{CodeKind, CodeSpec, QrEcc};
use tlaku_doc::{BezPath, PathSeg};
use tlaku_i18n::tr;
use tlaku_text::FontLibrary;

/// Primer puerto aceptado: por debajo de 1024 hacen falta privilegios.
pub const MIN_PORT: u16 = 1024;

/// Eventos hacia la ventana.
pub const EVENT_CLIENTS: &str = "remote://clients";
pub const EVENT_START: &str = "remote://start";
pub const EVENT_ACTION: &str = "remote://action";

// ---------------------------------------------------------------------------
// Configuración y estado que ve la interfaz
// ---------------------------------------------------------------------------

/// Lo que la interfaz configura desde Preferencias.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RemoteConfig {
    pub enabled: bool,
    pub port: u16,
    /// IP donde escuchar; `None` o una que ya no existe = la recomendada.
    #[serde(default)]
    pub ip: Option<String>,
}

/// Estado del servidor, que la interfaz muestra en Preferencias.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RemoteStatus {
    pub running: bool,
    pub port: u16,
    pub ip: Option<String>,
    /// Dirección sin la clave (`http://192.168.1.20:47802/`).
    pub url: Option<String>,
    /// Dirección con la clave, la del QR (`http://192.168.1.20:47802/#clave`).
    pub full_url: Option<String>,
    /// El QR de `full_url` como imagen SVG.
    pub qr_svg: Option<String>,
    /// Por qué no pudo arrancar (puerto ocupado, sin red, etc.).
    pub error: Option<String>,
    /// Direcciones de este equipo donde se puede escuchar.
    pub addrs: Vec<LocalAddr>,
    pub clients: Vec<ClientView>,
}

/// Evento `remote://clients`.
#[derive(Debug, Clone, Serialize)]
struct ClientsEvent<'a> {
    clients: &'a [ClientView],
}

/// Evento `remote://action`: algo se hizo desde el celular.
#[derive(Debug, Clone, Serialize)]
struct ActionEvent<'a> {
    action: &'a str,
    client: &'a str,
}

// ---------------------------------------------------------------------------
// Código QR
// ---------------------------------------------------------------------------

/// Trazado SVG (`M..L..Z`) de las formas del QR.
fn paths_to_d(paths: &[BezPath]) -> String {
    use std::fmt::Write;
    let mut d = String::new();
    for p in paths {
        for seg in &p.0 {
            let _ = match seg {
                PathSeg::MoveTo { to } => write!(d, "M{:.3} {:.3}", to.x, to.y),
                PathSeg::LineTo { to } => write!(d, "L{:.3} {:.3}", to.x, to.y),
                PathSeg::QuadTo { ctrl, to } => {
                    write!(d, "Q{:.3} {:.3} {:.3} {:.3}", ctrl.x, ctrl.y, to.x, to.y)
                }
                PathSeg::CubicTo { ctrl1, ctrl2, to } => write!(
                    d,
                    "C{:.3} {:.3} {:.3} {:.3} {:.3} {:.3}",
                    ctrl1.x, ctrl1.y, ctrl2.x, ctrl2.y, to.x, to.y
                ),
                PathSeg::Close => write!(d, "Z"),
            };
        }
    }
    d
}

/// El QR de `text` como SVG autónomo: módulos negros sobre fondo blanco con la zona en blanco que
/// pide el estándar (4 módulos). `tlaku-codes` entrega las formas con Y hacia arriba, así que se
/// voltean para dibujarlas (un QR volteado no se lee).
pub fn qr_svg(text: &str) -> Result<String, String> {
    let spec = CodeSpec {
        kind: CodeKind::Qr,
        data: text.to_string(),
        ecc: QrEcc::M,
        size_mm: 100.0,
        margin_mm: 0.0,
        ..CodeSpec::default()
    };
    let out = tlaku_codes::generate(&FontLibrary::new(), &spec)?;
    let module = out.width_mm / out.modules.max(1) as f64;
    let quiet = 4.0 * module;
    let (w, h) = (out.width_mm + 2.0 * quiet, out.height_mm + 2.0 * quiet);
    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w:.3} {h:.3}\" shape-rendering=\"crispEdges\">\
         <rect width=\"{w:.3}\" height=\"{h:.3}\" fill=\"#fff\"/>\
         <path transform=\"translate({quiet:.3} {top:.3}) scale(1 -1)\" fill=\"#000\" fill-rule=\"evenodd\" d=\"{d}\"/></svg>",
        top = quiet + out.height_mm,
        d = paths_to_d(&out.paths),
    ))
}

// ---------------------------------------------------------------------------
// Estado de la aplicación
// ---------------------------------------------------------------------------

struct Running {
    core: Arc<Core>,
    /// Se mantiene vivo mientras corre; soltarlo apaga el servidor.
    _server: Server,
    ip: Ipv4Addr,
    port: u16,
    requested_ip: Option<String>,
    token_url: String,
}

#[derive(Default)]
struct Inner {
    running: Option<Running>,
    error: Option<String>,
    config_port: u16,
}

/// Estado administrado por Tauri: el servidor remoto (si está encendido).
#[derive(Default)]
pub struct RemoteState {
    inner: Mutex<Inner>,
}

fn open_error(e: &io::Error, ip: Ipv4Addr, port: u16) -> String {
    match e.kind() {
        io::ErrorKind::AddrInUse => tr!("desktop.common.port_in_use", port = port),
        io::ErrorKind::AddrNotAvailable => tr!("desktop.remote.addr_unavailable", ip = ip),
        _ => tr!("desktop.remote.open_failed", port = port, ip = ip, err = e),
    }
}

impl RemoteState {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// El núcleo del servidor en marcha, si lo hay.
    pub fn core(&self) -> Option<Arc<Core>> {
        self.lock().running.as_ref().map(|r| r.core.clone())
    }

    /// Estado actual para la interfaz.
    pub fn status(&self, addrs: Vec<LocalAddr>) -> RemoteStatus {
        let g = self.lock();
        match &g.running {
            Some(r) => RemoteStatus {
                running: true,
                port: r.port,
                ip: Some(r.ip.to_string()),
                url: Some(format!("http://{}:{}/", r.ip, r.port)),
                full_url: Some(r.token_url.clone()),
                qr_svg: qr_svg(&r.token_url).ok(),
                error: None,
                addrs,
                clients: r.core.auth.clients(Instant::now()),
            },
            None => RemoteStatus {
                running: false,
                port: g.config_port,
                ip: None,
                url: None,
                full_url: None,
                qr_svg: None,
                error: g.error.clone(),
                addrs,
                clients: Vec::new(),
            },
        }
    }

    /// Aplica la configuración: enciende, apaga o mueve el servidor. `addrs` son las direcciones
    /// locales del equipo (en las pruebas, las que se quiera).
    pub fn configure(
        &self,
        config: &RemoteConfig,
        control: Arc<dyn Control>,
        addrs: Vec<LocalAddr>,
    ) -> RemoteStatus {
        {
            let mut g = self.lock();
            g.config_port = config.port;
            if !config.enabled {
                g.running = None;
                g.error = None;
                drop(g);
                return self.status(addrs);
            }
            if config.port < MIN_PORT {
                g.running = None;
                g.error = Some(tr!("desktop.automation.port_range", min = MIN_PORT));
                drop(g);
                return self.status(addrs);
            }
            let same = g
                .running
                .as_ref()
                .is_some_and(|r| r.port == config.port && r.requested_ip == config.ip);
            if same {
                drop(g);
                return self.status(addrs);
            }
            // Se suelta el servidor anterior antes de abrir el puerto nuevo.
            g.running = None;
            g.error = None;
        }
        let result = net::pick_addr(config.ip.as_deref(), &addrs).and_then(|ip| {
            let core = Arc::new(Core::new(control));
            let server = Server::start(core.clone(), SocketAddr::from((ip, config.port)))
                .map_err(|e| open_error(&e, ip, config.port))?;
            let port = server.addr().port();
            let token_url = format!("http://{ip}:{port}/#{}", core.auth.token());
            Ok(Running {
                core,
                _server: server,
                ip,
                port,
                requested_ip: config.ip.clone(),
                token_url,
            })
        });
        let mut g = self.lock();
        match result {
            Ok(r) => g.running = Some(r),
            Err(e) => g.error = Some(e),
        }
        drop(g);
        self.status(addrs)
    }

    /// Clave nueva: expulsa a todos los clientes y devuelve el estado con el QR nuevo.
    pub fn regenerate(&self, addrs: Vec<LocalAddr>) -> RemoteStatus {
        {
            let mut g = self.lock();
            if let Some(r) = g.running.as_mut() {
                let token = r.core.auth.regenerate();
                r.token_url = format!("http://{}:{}/#{token}", r.ip, r.port);
                r.core.bump();
                r.core.refresh_clients(Instant::now());
            }
        }
        self.status(addrs)
    }

    /// Desconecta a un cliente. `true` si existía.
    pub fn kick(&self, id: &str) -> bool {
        let Some(core) = self.core() else {
            return false;
        };
        let kicked = core.auth.kick(id);
        if kicked {
            core.bump();
            core.refresh_clients(Instant::now());
        }
        kicked
    }
}

// ---------------------------------------------------------------------------
// Conexión con la aplicación (Tauri)
// ---------------------------------------------------------------------------

struct AppControl(AppHandle);

impl Control for AppControl {
    fn machine(&self) -> Option<MachineInfo> {
        let m = self.0.state::<AppState>().machine().ok()?;
        let (kind, is_virtual) = match &m {
            crate::machine::active::Machine::Grbl(h) => (MachineKind::Grbl, h.is_virtual()),
            // El dato de "virtual" solo es informativo; los demás respaldos no lo exponen en su mando.
            crate::machine::active::Machine::Ruida(_) => (MachineKind::Ruida, false),
            crate::machine::active::Machine::Lhy(_) => (MachineKind::Lhy, false),
            crate::machine::active::Machine::Galvo(_) => (MachineKind::Galvo, false),
        };
        Some(MachineInfo {
            kind,
            is_virtual,
            status: m.status_report(),
        })
    }

    fn pause(&self) -> Result<(), String> {
        self.0.state::<AppState>().machine()?.pause()
    }

    fn resume(&self) -> Result<(), String> {
        self.0.state::<AppState>().machine()?.resume()
    }

    fn stop(&self) -> Result<(), String> {
        self.0.state::<AppState>().machine()?.cancel()
    }

    fn ask_start(&self, request: StartRequest) {
        let _ = self.0.emit(EVENT_START, request);
    }

    fn notify_action(&self, action: &str, client: &str) {
        let _ = self.0.emit(EVENT_ACTION, ActionEvent { action, client });
    }

    fn clients_changed(&self, clients: &[ClientView]) {
        let _ = self.0.emit(EVENT_CLIENTS, ClientsEvent { clients });
    }
}

/// Enciende, apaga o reconfigura el control remoto.
#[tauri::command(rename_all = "snake_case")]
pub fn remote_configure(
    app: AppHandle,
    state: State<'_, RemoteState>,
    config: RemoteConfig,
) -> RemoteStatus {
    state.configure(&config, Arc::new(AppControl(app)), net::local_addrs())
}

/// Estado actual (QR, clientes, direcciones disponibles).
#[tauri::command(rename_all = "snake_case")]
pub fn remote_status(state: State<'_, RemoteState>) -> RemoteStatus {
    state.status(net::local_addrs())
}

/// Genera una clave nueva: la anterior deja de valer y los clientes conectados quedan expulsados.
#[tauri::command(rename_all = "snake_case")]
pub fn remote_regenerate(state: State<'_, RemoteState>) -> RemoteStatus {
    state.regenerate(net::local_addrs())
}

/// Desconecta a un cliente por su id.
#[tauri::command(rename_all = "snake_case")]
pub fn remote_kick(state: State<'_, RemoteState>, id: String) -> RemoteStatus {
    state.kick(&id);
    state.status(net::local_addrs())
}

/// La ventana informa su estado (progreso, tiempo, capa, cabezal, encuadre).
#[tauri::command(rename_all = "snake_case")]
pub fn remote_update(state: State<'_, RemoteState>, snapshot: Snapshot) {
    if let Some(core) = state.core() {
        core.update(snapshot, Instant::now());
    }
}

/// La ventana manda el recorrido del trabajo (o `null` si ya no hay).
#[tauri::command(rename_all = "snake_case")]
pub fn remote_set_route(
    state: State<'_, RemoteState>,
    route: Option<RouteIn>,
) -> Result<(), String> {
    match state.core() {
        Some(core) => core.set_route(route),
        None => Ok(()),
    }
}

/// La ventana manda un cuadro de la cámara (JPEG en base64); vacío = la cámara se apagó. Devuelve si
/// hay algún celular esperándolo (si no, la ventana deja de mandar).
#[tauri::command(rename_all = "snake_case")]
pub fn remote_camera_frame(
    state: State<'_, RemoteState>,
    jpeg_b64: String,
) -> Result<bool, String> {
    let Some(core) = state.core() else {
        return Ok(false);
    };
    if jpeg_b64.is_empty() {
        core.clear_camera();
        return Ok(false);
    }
    if jpeg_b64.len() > hub::MAX_CAMERA_BYTES * 4 / 3 + 8 {
        return Err(tr!("desktop.remote.camera_too_big"));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(jpeg_b64.as_bytes())
        .map_err(|_| tr!("desktop.remote.camera_bad_base64"))?;
    core.set_camera(bytes, Instant::now())
}

/// Respuesta de la ventana a un pedido de inicio (`remote://start`).
#[derive(Debug, Clone, Deserialize)]
pub struct StartResult {
    pub id: u64,
    pub ok: bool,
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub message: String,
}

#[tauri::command(rename_all = "snake_case")]
pub fn remote_start_result(state: State<'_, RemoteState>, result: StartResult) {
    if let Some(core) = state.core() {
        core.finish_start(
            result.id,
            StartOutcome {
                ok: result.ok,
                code: result.code,
                message: result.message,
            },
        );
    }
}
