//! Núcleo del control remoto, sin red ni Tauri: lo que la interfaz de la PC informa, las acciones
//! que pide el celular y las reglas de seguridad para iniciar.
//!
//! Quién sabe qué:
//!
//! - **El motor** (la máquina conectada) es la fuente de verdad de lo que hay que hacer *ya*: si
//!   está conectada, su estado (reposo, alarma, pausa) y las operaciones de pausar, reanudar y
//!   detener. Eso se consulta y se ejecuta directo, por [`Control`], sin pasar por la ventana: si la
//!   ventana de la PC se congela, detener sigue funcionando.
//! - **La interfaz** (la ventana) es dueña del documento y de las opciones del trabajo. Manda su
//!   estado con [`Snapshot`] (progreso, tiempo restante, capa, posición del cabezal y, sobre todo, si
//!   el trabajo ya se encuadró tras el último cambio). Todo eso sale de los eventos `machine://*`
//!   que la interfaz ya recibe.
//!
//! # Reglas
//!
//! - Pausar, reanudar y detener se aceptan siempre que haya máquina (el galvo no tiene pausa).
//! - **Iniciar** exige, todas a la vez: el checklist completo del celular, máquina conectada y en
//!   reposo, que la PC siga informando, que haya algo que enviar, que el trabajo se haya encuadrado
//!   en la PC desde el último cambio, y que la máquina no sea un galvo (haz invisible). Aun así el
//!   inicio lo ejecuta la interfaz con su ruta de siempre (preflight incluido), que vuelve a
//!   comprobar el encuadre: la lógica de inicio no se duplica aquí.

use super::auth::{Auth, ClientView};
use crate::machine::{galvo, not_connected};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tlaku_doc::{MachineState, StatusReport};
use tlaku_i18n::tr;
use tokio::sync::{oneshot, watch};

/// Si la interfaz no informa en este tiempo se la da por ausente y no se permite iniciar.
pub const SNAPSHOT_FRESH: Duration = Duration::from_secs(12);
/// Una imagen de cámara más vieja que esto ya no se sirve.
pub const CAMERA_FRESH: Duration = Duration::from_secs(20);
/// Tope de una imagen de cámara (la interfaz manda JPEG reducidos de ~50 KB).
pub const MAX_CAMERA_BYTES: usize = 600 * 1024;
/// Tope del recorrido dibujado en la página.
pub const MAX_ROUTE_BYTES: usize = 400 * 1024;
/// Cuánto espera el celular la respuesta de la interfaz a un inicio.
pub const START_TIMEOUT: Duration = Duration::from_secs(60);

// ---------------------------------------------------------------------------
// Lo que el núcleo necesita de la aplicación (en las pruebas es un doble)
// ---------------------------------------------------------------------------

/// Respaldo de la máquina conectada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MachineKind {
    Grbl,
    Ruida,
    Lhy,
    Galvo,
}

/// La máquina conectada y su último estado.
#[derive(Debug, Clone)]
pub struct MachineInfo {
    pub kind: MachineKind,
    pub is_virtual: bool,
    pub status: Option<StatusReport>,
}

/// Pedido de inicio que viaja a la interfaz.
#[derive(Debug, Clone, Serialize)]
pub struct StartRequest {
    pub id: u64,
    /// Quién lo pidió (`192.168.1.9 · iPhone`), para el aviso en la PC.
    pub client: String,
}

/// Lo que el núcleo necesita de la aplicación.
pub trait Control: Send + Sync + 'static {
    /// La máquina conectada; `None` si no hay.
    fn machine(&self) -> Option<MachineInfo>;
    /// Pausa el trabajo. Bloquea hasta que la máquina contesta.
    fn pause(&self) -> Result<(), String>;
    fn resume(&self) -> Result<(), String>;
    /// Detiene el trabajo (como el botón de parar de la PC).
    fn stop(&self) -> Result<(), String>;
    /// Pide a la interfaz iniciar el trabajo; contesta con [`Core::finish_start`].
    fn ask_start(&self, request: StartRequest);
    /// Avisa a la interfaz de una acción hecha desde el celular (`pause`, `resume`, `stop`).
    fn notify_action(&self, action: &str, client: &str);
    /// La lista de clientes cambió.
    fn clients_changed(&self, clients: &[ClientView]);
}

// ---------------------------------------------------------------------------
// Lo que informa la interfaz
// ---------------------------------------------------------------------------

/// Fase del trabajo, como la lleva la interfaz.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobPhase {
    #[default]
    Idle,
    Running,
    Paused,
    Finished,
    Cancelled,
    Error,
}

impl JobPhase {
    fn active(self) -> bool {
        matches!(self, Self::Running | Self::Paused)
    }
}

/// Capa en curso.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerInfo {
    /// Posición (1 = la primera) y total de capas del trabajo.
    pub index: u32,
    pub count: u32,
    /// Color de la capa (`#ff0000`).
    pub color: String,
    pub mode: String,
}

/// Estado que la ventana de la PC manda al servidor remoto.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Snapshot {
    pub doc_name: String,
    pub device_name: String,
    /// Tamaño de la cama, mm.
    pub bed: Option<[f64; 2]>,
    pub job_state: JobPhase,
    pub job_message: String,
    /// Avance, 0 a 1.
    pub progress: f64,
    pub elapsed_s: f64,
    pub remaining_s: Option<f64>,
    pub total_s: Option<f64>,
    pub layer: Option<LayerInfo>,
    /// Posición del cabezal en coordenadas del documento, mm.
    pub head: Option<[f64; 2]>,
    /// Hay algo que enviar.
    pub has_output: bool,
    /// El trabajo se encuadró en la PC después del último cambio del documento, de la máquina o de
    /// las opciones del trabajo.
    pub framed: bool,
}

fn clean_text(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect()
}

fn finite(v: f64) -> f64 {
    if v.is_finite() { v.max(0.0) } else { 0.0 }
}

fn finite_opt(v: Option<f64>) -> Option<f64> {
    v.filter(|x| x.is_finite()).map(|x| x.max(0.0))
}

fn finite_pair(v: Option<[f64; 2]>) -> Option<[f64; 2]> {
    v.filter(|p| p[0].is_finite() && p[1].is_finite())
}

impl Snapshot {
    /// Acota y limpia lo que llega de la ventana: el servidor no confía en el tamaño ni en el
    /// contenido de lo que va a repetir a otros dispositivos.
    pub fn sanitized(mut self) -> Self {
        self.doc_name = clean_text(&self.doc_name, 120);
        self.device_name = clean_text(&self.device_name, 120);
        self.job_message = clean_text(&self.job_message, 300);
        self.progress = finite(self.progress).min(1.0);
        self.elapsed_s = finite(self.elapsed_s);
        self.remaining_s = finite_opt(self.remaining_s);
        self.total_s = finite_opt(self.total_s);
        self.head = finite_pair(self.head);
        self.bed = finite_pair(self.bed).filter(|b| b[0] > 0.0 && b[1] > 0.0);
        if let Some(l) = &mut self.layer {
            l.color = clean_text(&l.color, 16);
            l.mode = clean_text(&l.mode, 16);
        }
        self
    }
}

/// Recorrido del trabajo para dibujarlo en la página.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RouteIn {
    /// Tamaño de la cama, mm.
    pub bed: [f64; 2],
    /// Trazado SVG (`M x y L x y ...`) en mm del documento, Y hacia arriba.
    pub d: String,
}

#[derive(Debug, Clone)]
struct Route {
    version: u64,
    bed: [f64; 2],
    d: String,
}

/// Resultado de un inicio, tal como lo informa la interfaz.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StartOutcome {
    pub ok: bool,
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub message: String,
}

// ---------------------------------------------------------------------------
// Errores de las acciones
// ---------------------------------------------------------------------------

/// Una acción rechazada: un código estable (que la página traduce) y el texto en español.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionError {
    pub code: &'static str,
    pub message: String,
}

impl ActionError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Por qué no se puede iniciar ahora.
pub fn start_reason_text(code: &str) -> String {
    match code {
        "not_connected" => not_connected(),
        "job_active" => tr!("desktop.remote.reason_job_active"),
        "not_idle" => tr!("desktop.remote.reason_not_idle"),
        "pc_offline" => tr!("desktop.remote.reason_pc_offline"),
        "nothing_to_send" => tr!("desktop.remote.reason_nothing_to_send"),
        "galvo" => tr!("desktop.remote.reason_galvo"),
        "not_framed" => tr!("desktop.remote.reason_not_framed"),
        "busy_request" => tr!("desktop.remote.reason_busy_request"),
        "checklist" => tr!("desktop.remote.reason_checklist"),
        "timeout" => tr!("desktop.remote.reason_timeout"),
        _ => tr!("desktop.remote.reason_default"),
    }
}

/// Las tres confirmaciones que pide el celular antes de iniciar.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Checklist {
    /// Estoy junto a la máquina.
    pub nearby: bool,
    /// El área está despejada.
    pub clear: bool,
    /// El aire o el extractor están encendidos.
    pub air: bool,
}

impl Checklist {
    pub fn complete(self) -> bool {
        self.nearby && self.clear && self.air
    }
}

// ---------------------------------------------------------------------------
// Núcleo
// ---------------------------------------------------------------------------

#[derive(Default)]
struct HubInner {
    snapshot: Option<(Snapshot, Instant)>,
    route: Option<Route>,
    camera: Option<(Vec<u8>, Instant)>,
    camera_version: u64,
    route_version: u64,
    finished_seq: u64,
    error_seq: u64,
    next_request: u64,
    pending_start: Option<(u64, oneshot::Sender<StartOutcome>)>,
    last_clients: String,
}

/// Estado compartido del servidor remoto.
pub struct Core {
    pub auth: Auth,
    control: std::sync::Arc<dyn Control>,
    inner: Mutex<HubInner>,
    changed: watch::Sender<u64>,
}

impl Core {
    pub fn new(control: std::sync::Arc<dyn Control>) -> Self {
        Self::with_auth(control, Auth::new())
    }

    pub fn with_auth(control: std::sync::Arc<dyn Control>, auth: Auth) -> Self {
        Self {
            auth,
            control,
            inner: Mutex::new(HubInner::default()),
            changed: watch::channel(0).0,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HubInner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Avisa a las conexiones en vivo de que hay algo nuevo que mandar.
    pub fn bump(&self) {
        self.changed.send_modify(|v| *v = v.wrapping_add(1));
    }

    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.changed.subscribe()
    }

    #[cfg(test)]
    pub fn control(&self) -> &dyn Control {
        self.control.as_ref()
    }

    // ---- lo que informa la interfaz ----

    /// Nuevo estado de la ventana de la PC.
    pub fn update(&self, snapshot: Snapshot, now: Instant) {
        let snapshot = snapshot.sanitized();
        {
            let mut g = self.lock();
            let before = g.snapshot.as_ref().map(|(s, _)| s.job_state);
            // Los avisos del final se cuentan aquí para que no dependan de que el celular
            // estuviera mirando justo en ese momento.
            if before != Some(snapshot.job_state) {
                match snapshot.job_state {
                    JobPhase::Finished if before.is_some_and(JobPhase::active) => {
                        g.finished_seq += 1
                    }
                    JobPhase::Error if before.is_some_and(JobPhase::active) => g.error_seq += 1,
                    _ => {}
                }
            }
            g.snapshot = Some((snapshot, now));
        }
        self.bump();
    }

    /// Recorrido del trabajo; `None` lo borra. Falla si es demasiado grande.
    pub fn set_route(&self, route: Option<RouteIn>) -> Result<(), String> {
        {
            let mut g = self.lock();
            match route {
                None => g.route = None,
                Some(r) => {
                    if r.d.len() > MAX_ROUTE_BYTES {
                        return Err(tr!("desktop.remote.route_too_big"));
                    }
                    if !r
                        .d
                        .chars()
                        .all(|c| c.is_ascii_digit() || " .-MLZ,e".contains(c))
                    {
                        return Err(tr!("desktop.remote.route_bad_chars"));
                    }
                    if !(r.bed[0].is_finite()
                        && r.bed[1].is_finite()
                        && r.bed[0] > 0.0
                        && r.bed[1] > 0.0)
                    {
                        return Err(tr!("desktop.remote.route_bad_bed"));
                    }
                    g.route_version += 1;
                    let version = g.route_version;
                    g.route = Some(Route {
                        version,
                        bed: r.bed,
                        d: r.d,
                    });
                }
            }
        }
        self.bump();
        Ok(())
    }

    /// El recorrido para `GET /api/route`.
    pub fn route_json(&self) -> Value {
        match &self.lock().route {
            Some(r) => json!({ "version": r.version, "bed": r.bed, "d": r.d }),
            None => json!({ "version": 0, "bed": null, "d": "" }),
        }
    }

    /// Imagen de la cámara de la PC (JPEG). Devuelve si algún celular la está esperando; si no, la
    /// interfaz deja de mandarla.
    pub fn set_camera(&self, jpeg: Vec<u8>, now: Instant) -> Result<bool, String> {
        if jpeg.len() > MAX_CAMERA_BYTES {
            return Err(tr!("desktop.remote.camera_too_big"));
        }
        if !jpeg.starts_with(&[0xff, 0xd8, 0xff]) {
            return Err(tr!("desktop.remote.camera_not_jpeg"));
        }
        let wanted = self.auth.connected_count(now) > 0;
        if wanted {
            let mut g = self.lock();
            g.camera_version += 1;
            g.camera = Some((jpeg, now));
            drop(g);
            self.bump();
        }
        Ok(wanted)
    }

    /// La cámara se apagó en la PC.
    pub fn clear_camera(&self) {
        let mut g = self.lock();
        if g.camera.take().is_some() {
            g.camera_version += 1;
            drop(g);
            self.bump();
        }
    }

    /// La imagen de la cámara, si es reciente.
    pub fn camera_jpeg(&self, now: Instant) -> Option<Vec<u8>> {
        let g = self.lock();
        let (bytes, at) = g.camera.as_ref()?;
        (now.saturating_duration_since(*at) < CAMERA_FRESH).then(|| bytes.clone())
    }

    // ---- reglas ----

    fn snapshot_now(&self, now: Instant) -> Option<Snapshot> {
        let g = self.lock();
        let (s, at) = g.snapshot.as_ref()?;
        (now.saturating_duration_since(*at) < SNAPSHOT_FRESH).then(|| s.clone())
    }

    /// Por qué **no** se puede iniciar ahora (código estable), o `None` si se puede. No incluye el
    /// checklist, que es una respuesta del usuario y no un estado.
    pub fn start_block(&self, now: Instant) -> Option<&'static str> {
        let Some(machine) = self.control.machine() else {
            return Some("not_connected");
        };
        let Some(snap) = self.snapshot_now(now) else {
            return Some("pc_offline");
        };
        if snap.job_state.active() {
            return Some("job_active");
        }
        if machine
            .status
            .as_ref()
            .is_none_or(|s| s.state != MachineState::Idle)
        {
            return Some("not_idle");
        }
        if !snap.has_output {
            return Some("nothing_to_send");
        }
        // Haz invisible, clase 4: se inicia mirando la máquina desde la PC y con su diálogo.
        if machine.kind == MachineKind::Galvo {
            return Some("galvo");
        }
        if !snap.framed {
            return Some("not_framed");
        }
        if self.lock().pending_start.is_some() {
            return Some("busy_request");
        }
        None
    }

    fn require_machine(&self) -> Result<MachineInfo, ActionError> {
        self.control
            .machine()
            .ok_or_else(|| ActionError::new("not_connected", not_connected()))
    }

    /// Pausa. No existe en el galvo.
    pub fn pause(&self, client: &str) -> Result<(), ActionError> {
        let m = self.require_machine()?;
        if m.kind == MachineKind::Galvo {
            return Err(ActionError::new("no_pause", galvo::no_pause()));
        }
        self.control
            .pause()
            .map_err(|e| ActionError::new("failed", e))?;
        self.control.notify_action("pause", client);
        Ok(())
    }

    pub fn resume(&self, client: &str) -> Result<(), ActionError> {
        let m = self.require_machine()?;
        if m.kind == MachineKind::Galvo {
            return Err(ActionError::new("no_pause", galvo::no_pause()));
        }
        self.control
            .resume()
            .map_err(|e| ActionError::new("failed", e))?;
        self.control.notify_action("resume", client);
        Ok(())
    }

    /// Detener: se atiende siempre que haya máquina, sin mirar el estado, el encuadre ni el checklist.
    pub fn stop(&self, client: &str) -> Result<(), ActionError> {
        self.require_machine()?;
        self.control
            .stop()
            .map_err(|e| ActionError::new("failed", e))?;
        self.control.notify_action("stop", client);
        Ok(())
    }

    /// Valida un inicio y lo registra; el pedido se manda a la interfaz con [`Core::ask_start`].
    /// Devuelve el número del pedido y por dónde llegará la respuesta.
    pub fn begin_start(
        &self,
        checklist: Checklist,
        now: Instant,
    ) -> Result<(u64, oneshot::Receiver<StartOutcome>), ActionError> {
        if !checklist.complete() {
            return Err(ActionError::new(
                "checklist",
                start_reason_text("checklist"),
            ));
        }
        if let Some(code) = self.start_block(now) {
            return Err(ActionError::new(code, start_reason_text(code)));
        }
        let (tx, rx) = oneshot::channel();
        let mut g = self.lock();
        if g.pending_start.is_some() {
            return Err(ActionError::new(
                "busy_request",
                start_reason_text("busy_request"),
            ));
        }
        g.next_request += 1;
        let id = g.next_request;
        g.pending_start = Some((id, tx));
        Ok((id, rx))
    }

    /// Manda el pedido de inicio a la interfaz.
    pub fn ask_start(&self, id: u64, client: &str) {
        self.control.ask_start(StartRequest {
            id,
            client: client.to_string(),
        });
    }

    /// La interfaz contestó al inicio `id`. `false` si ya nadie lo esperaba.
    pub fn finish_start(&self, id: u64, outcome: StartOutcome) -> bool {
        let mut g = self.lock();
        match g.pending_start.take() {
            Some((pending, tx)) if pending == id => tx.send(outcome).is_ok(),
            other => {
                g.pending_start = other;
                false
            }
        }
    }

    /// Descarta el pedido pendiente (se agotó la espera).
    pub fn abandon_start(&self, id: u64) {
        let mut g = self.lock();
        if g.pending_start.as_ref().is_some_and(|(p, _)| *p == id) {
            g.pending_start = None;
        }
    }

    // ---- clientes ----

    /// Avisa a la PC si la lista de clientes cambió desde el último aviso.
    pub fn refresh_clients(&self, now: Instant) {
        let clients = self.auth.clients(now);
        let sig = clients
            .iter()
            .map(|c| format!("{}:{}:{}", c.id, c.connected, c.ip))
            .collect::<Vec<_>>()
            .join(",");
        {
            let mut g = self.lock();
            if g.last_clients == sig {
                return;
            }
            g.last_clients = sig;
        }
        self.control.clients_changed(&clients);
    }

    // ---- lo que ve el celular ----

    /// Estado completo para el celular (`GET /api/state` y los eventos en vivo).
    pub fn view(&self, now: Instant) -> Value {
        let machine = self.control.machine();
        let snap = self.snapshot_now(now);
        let g = self.lock();
        let finished_seq = g.finished_seq;
        let error_seq = g.error_seq;
        let route_v = g.route.as_ref().map_or(0, |r| r.version);
        let camera_v = match &g.camera {
            Some((_, at)) if now.saturating_duration_since(*at) < CAMERA_FRESH => g.camera_version,
            _ => 0,
        };
        drop(g);

        let machine_json = match &machine {
            None => json!({ "connected": false }),
            Some(m) => json!({
                "connected": true,
                "kind": m.kind,
                "virtual": m.is_virtual,
                "state": m.status.as_ref().map(|s| format!("{:?}", s.state).to_ascii_lowercase()),
                "other_state": m.status.as_ref().and_then(|s| s.other_state.clone()),
            }),
        };
        let s = snap.clone().unwrap_or_default();
        let block = self.start_block(now);
        let can_pause = machine
            .as_ref()
            .is_some_and(|m| m.kind != MachineKind::Galvo);
        json!({
            "v": 1,
            "pc_online": snap.is_some(),
            "machine": machine_json,
            "device": s.device_name,
            "doc": s.doc_name,
            "job": {
                "phase": s.job_state,
                "message": s.job_message,
                "progress": s.progress,
                "elapsed_s": s.elapsed_s,
                "remaining_s": s.remaining_s,
                "total_s": s.total_s,
                "layer": s.layer,
                "finished_seq": finished_seq,
                "error_seq": error_seq,
            },
            "head": s.head,
            "bed": s.bed,
            "caps": {
                "pause": can_pause,
                "pause_reason": if machine.is_none() { Some("not_connected") } else if can_pause { None } else { Some("no_pause") },
                "stop": machine.is_some(),
            },
            "start": {
                "allowed": block.is_none(),
                "reason": block,
                "message": block.map(start_reason_text),
            },
            "route_v": route_v,
            "camera_v": camera_v,
        })
    }
}

#[cfg(test)]
pub mod testing {
    //! Doble de [`Control`] para las pruebas del núcleo y del servidor.
    use super::*;
    use std::sync::Arc;

    #[derive(Default)]
    pub struct Fake {
        pub machine: Mutex<Option<MachineInfo>>,
        pub calls: Mutex<Vec<String>>,
        pub fail_stop: Mutex<bool>,
    }

    impl Fake {
        pub fn with(kind: MachineKind, state: MachineState) -> Arc<Self> {
            let f = Arc::new(Self::default());
            f.set(Some((kind, state)));
            f
        }

        pub fn set(&self, m: Option<(MachineKind, MachineState)>) {
            *self.machine.lock().unwrap() = m.map(|(kind, state)| MachineInfo {
                kind,
                is_virtual: true,
                status: Some(StatusReport {
                    state,
                    other_state: None,
                    machine_pos: [0.0, 0.0],
                    work_pos: [1.0, 2.0],
                    feed: 0.0,
                    spindle: 0.0,
                    buffer: tlaku_doc::BufferStatus {
                        planner: 15,
                        rx: 127,
                    },
                    overrides: None,
                }),
            });
        }

        pub fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }

        fn log(&self, what: &str) {
            self.calls.lock().unwrap().push(what.to_string());
        }
    }

    impl Control for Fake {
        fn machine(&self) -> Option<MachineInfo> {
            self.machine.lock().unwrap().clone()
        }
        fn pause(&self) -> Result<(), String> {
            self.log("pause");
            Ok(())
        }
        fn resume(&self) -> Result<(), String> {
            self.log("resume");
            Ok(())
        }
        fn stop(&self) -> Result<(), String> {
            self.log("stop");
            if *self.fail_stop.lock().unwrap() {
                Err("falló".into())
            } else {
                Ok(())
            }
        }
        fn ask_start(&self, r: StartRequest) {
            self.log(&format!("ask_start {}", r.id));
        }
        fn notify_action(&self, action: &str, _client: &str) {
            self.log(&format!("notify {action}"));
        }
        fn clients_changed(&self, clients: &[ClientView]) {
            self.log(&format!("clients {}", clients.len()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::Fake;
    use super::*;
    use std::sync::Arc;

    fn ready() -> Snapshot {
        Snapshot {
            has_output: true,
            framed: true,
            job_state: JobPhase::Idle,
            ..Snapshot::default()
        }
    }

    fn core(fake: &Arc<Fake>) -> Core {
        Core::new(fake.clone())
    }

    fn all_checked() -> Checklist {
        Checklist {
            nearby: true,
            clear: true,
            air: true,
        }
    }

    #[test]
    fn start_needs_every_condition() {
        let fake = Fake::with(MachineKind::Grbl, MachineState::Idle);
        let c = core(&fake);
        let now = Instant::now();
        // Sin que la PC informe nada.
        assert_eq!(c.start_block(now), Some("pc_offline"));
        c.update(ready(), now);
        assert_eq!(c.start_block(now), None);
        // Sin encuadre.
        c.update(
            Snapshot {
                framed: false,
                ..ready()
            },
            now,
        );
        assert_eq!(c.start_block(now), Some("not_framed"));
        // Sin nada que enviar.
        c.update(
            Snapshot {
                has_output: false,
                ..ready()
            },
            now,
        );
        assert_eq!(c.start_block(now), Some("nothing_to_send"));
        // Trabajo ya en curso.
        c.update(
            Snapshot {
                job_state: JobPhase::Running,
                ..ready()
            },
            now,
        );
        assert_eq!(c.start_block(now), Some("job_active"));
        // Máquina fuera de reposo.
        c.update(ready(), now);
        for busy in [
            MachineState::Run,
            MachineState::Hold,
            MachineState::Alarm,
            MachineState::Door,
            MachineState::Other,
        ] {
            fake.set(Some((MachineKind::Grbl, busy)));
            assert_eq!(c.start_block(now), Some("not_idle"), "{busy:?}");
        }
        // Sin máquina.
        fake.set(None);
        assert_eq!(c.start_block(now), Some("not_connected"));
        // La PC dejó de informar.
        fake.set(Some((MachineKind::Grbl, MachineState::Idle)));
        assert_eq!(
            c.start_block(now + SNAPSHOT_FRESH + Duration::from_secs(1)),
            Some("pc_offline")
        );
    }

    #[test]
    fn the_galvo_is_never_started_from_the_phone() {
        let fake = Fake::with(MachineKind::Galvo, MachineState::Idle);
        let c = core(&fake);
        let now = Instant::now();
        c.update(ready(), now);
        assert_eq!(c.start_block(now), Some("galvo"));
        let e = c.begin_start(all_checked(), now).unwrap_err();
        assert_eq!(e.code, "galvo");
    }

    #[test]
    fn begin_start_requires_the_whole_checklist_first() {
        let fake = Fake::with(MachineKind::Grbl, MachineState::Idle);
        let c = core(&fake);
        let now = Instant::now();
        c.update(ready(), now);
        for partial in [
            Checklist::default(),
            Checklist {
                nearby: true,
                clear: true,
                air: false,
            },
            Checklist {
                nearby: false,
                clear: true,
                air: true,
            },
            Checklist {
                nearby: true,
                clear: false,
                air: true,
            },
        ] {
            assert_eq!(c.begin_start(partial, now).unwrap_err().code, "checklist");
        }
        // Nada llegó a la interfaz.
        assert!(fake.calls().is_empty());
        let (id, mut rx) = c.begin_start(all_checked(), now).unwrap();
        // Un segundo inicio simultáneo se rechaza.
        assert_eq!(
            c.begin_start(all_checked(), now).unwrap_err().code,
            "busy_request"
        );
        assert!(c.finish_start(
            id,
            StartOutcome {
                ok: true,
                code: String::new(),
                message: String::new()
            }
        ));
        assert!(rx.try_recv().unwrap().ok);
        // Ya no hay nada pendiente.
        assert!(!c.finish_start(
            id,
            StartOutcome {
                ok: true,
                code: String::new(),
                message: String::new()
            }
        ));
        assert!(c.begin_start(all_checked(), now).is_ok());
    }

    #[test]
    fn a_stale_answer_does_not_resolve_a_newer_request() {
        let fake = Fake::with(MachineKind::Grbl, MachineState::Idle);
        let c = core(&fake);
        let now = Instant::now();
        c.update(ready(), now);
        let (id, _rx) = c.begin_start(all_checked(), now).unwrap();
        let ok = StartOutcome {
            ok: true,
            code: String::new(),
            message: String::new(),
        };
        assert!(!c.finish_start(id + 1, ok.clone()));
        c.abandon_start(id);
        assert_eq!(
            c.start_block(now),
            None,
            "tras abandonar se puede volver a pedir"
        );
    }

    #[test]
    fn stop_works_whatever_the_state_and_only_needs_a_machine() {
        let fake = Fake::with(MachineKind::Grbl, MachineState::Alarm);
        let c = core(&fake);
        // Sin que la PC informe, sin encuadre, sin checklist: detiene igual.
        assert!(c.stop("192.168.1.9").is_ok());
        for kind in [MachineKind::Ruida, MachineKind::Lhy, MachineKind::Galvo] {
            fake.set(Some((kind, MachineState::Run)));
            assert!(c.stop("x").is_ok(), "{kind:?}");
        }
        assert_eq!(fake.calls().iter().filter(|c| *c == "stop").count(), 4);
        fake.set(None);
        assert_eq!(c.stop("x").unwrap_err().code, "not_connected");
        // Si la máquina falla al detener, el error llega tal cual.
        fake.set(Some((MachineKind::Grbl, MachineState::Run)));
        *fake.fail_stop.lock().unwrap() = true;
        let e = c.stop("x").unwrap_err();
        assert_eq!((e.code, e.message.as_str()), ("failed", "falló"));
    }

    #[test]
    fn pause_and_resume_exist_everywhere_but_the_galvo() {
        let fake = Fake::with(MachineKind::Grbl, MachineState::Run);
        let c = core(&fake);
        for kind in [MachineKind::Grbl, MachineKind::Ruida, MachineKind::Lhy] {
            fake.set(Some((kind, MachineState::Run)));
            assert!(c.pause("x").is_ok());
            assert!(c.resume("x").is_ok());
        }
        fake.set(Some((MachineKind::Galvo, MachineState::Run)));
        assert_eq!(c.pause("x").unwrap_err().code, "no_pause");
        assert_eq!(c.resume("x").unwrap_err().code, "no_pause");
        let v = c.view(Instant::now());
        assert_eq!(v["caps"]["pause"], false);
        assert_eq!(v["caps"]["pause_reason"], "no_pause");
        assert_eq!(v["caps"]["stop"], true);
    }

    #[test]
    fn the_end_of_a_job_is_counted_once_per_transition() {
        let fake = Fake::with(MachineKind::Grbl, MachineState::Run);
        let c = core(&fake);
        let now = Instant::now();
        let seqs = |c: &Core| {
            let v = c.view(now);
            (
                v["job"]["finished_seq"].as_u64().unwrap(),
                v["job"]["error_seq"].as_u64().unwrap(),
            )
        };
        // Un "terminado" sin haber visto el trabajo en curso no es un fin de trabajo.
        c.update(
            Snapshot {
                job_state: JobPhase::Finished,
                ..ready()
            },
            now,
        );
        assert_eq!(seqs(&c), (0, 0));
        c.update(
            Snapshot {
                job_state: JobPhase::Running,
                ..ready()
            },
            now,
        );
        c.update(
            Snapshot {
                job_state: JobPhase::Running,
                progress: 0.5,
                ..ready()
            },
            now,
        );
        c.update(
            Snapshot {
                job_state: JobPhase::Finished,
                ..ready()
            },
            now,
        );
        c.update(
            Snapshot {
                job_state: JobPhase::Finished,
                ..ready()
            },
            now,
        );
        assert_eq!(seqs(&c), (1, 0));
        c.update(
            Snapshot {
                job_state: JobPhase::Paused,
                ..ready()
            },
            now,
        );
        c.update(
            Snapshot {
                job_state: JobPhase::Error,
                ..ready()
            },
            now,
        );
        assert_eq!(seqs(&c), (1, 1));
        // Cancelar no dispara ningún aviso.
        c.update(
            Snapshot {
                job_state: JobPhase::Running,
                ..ready()
            },
            now,
        );
        c.update(
            Snapshot {
                job_state: JobPhase::Cancelled,
                ..ready()
            },
            now,
        );
        assert_eq!(seqs(&c), (1, 1));
    }

    #[test]
    fn snapshots_are_cleaned_before_being_repeated() {
        let s = Snapshot {
            doc_name: format!("a\u{7}b{}", "x".repeat(500)),
            progress: 7.0,
            elapsed_s: f64::NAN,
            remaining_s: Some(f64::INFINITY),
            head: Some([f64::NAN, 1.0]),
            bed: Some([0.0, 100.0]),
            ..Snapshot::default()
        }
        .sanitized();
        assert_eq!(s.doc_name.chars().count(), 120);
        assert!(!s.doc_name.contains('\u{7}'));
        assert_eq!(
            (s.progress, s.elapsed_s, s.remaining_s, s.head, s.bed),
            (1.0, 0.0, None, None, None)
        );
    }

    #[test]
    fn routes_and_camera_frames_are_validated() {
        let fake = Fake::with(MachineKind::Grbl, MachineState::Idle);
        let c = core(&fake);
        let now = Instant::now();
        assert!(
            c.set_route(Some(RouteIn {
                bed: [100.0, 100.0],
                d: "M0 0L10 10".into()
            }))
            .is_ok()
        );
        assert_eq!(c.route_json()["version"], 1);
        for bad in ["<script>", "M0 0 url(x)", "M1 1;"] {
            assert!(
                c.set_route(Some(RouteIn {
                    bed: [100.0, 100.0],
                    d: bad.into()
                }))
                .is_err(),
                "{bad}"
            );
        }
        assert!(
            c.set_route(Some(RouteIn {
                bed: [0.0, 100.0],
                d: "M0 0".into()
            }))
            .is_err()
        );
        assert!(
            c.set_route(Some(RouteIn {
                bed: [1.0, 1.0],
                d: "M".repeat(MAX_ROUTE_BYTES + 1)
            }))
            .is_err()
        );
        assert!(c.set_route(None).is_ok());
        assert_eq!(c.route_json()["version"], 0);

        // Nadie conectado: la imagen no se guarda y la interfaz sabe que no hace falta.
        let jpeg = vec![0xff, 0xd8, 0xff, 0xe0, 1, 2, 3];
        assert_eq!(c.set_camera(jpeg.clone(), now), Ok(false));
        assert!(c.camera_jpeg(now).is_none());
        assert!(c.set_camera(vec![1, 2, 3], now).is_err());
        assert!(c.set_camera(vec![0xff; MAX_CAMERA_BYTES + 1], now).is_err());
        // Con alguien conectado, sí, y caduca.
        c.auth
            .login("192.168.1.9".parse().unwrap(), "", &c.auth.token(), now)
            .unwrap();
        assert_eq!(c.set_camera(jpeg.clone(), now), Ok(true));
        assert_eq!(c.camera_jpeg(now).as_deref(), Some(&jpeg[..]));
        assert_eq!(c.view(now)["camera_v"], 1);
        let later = now + CAMERA_FRESH + Duration::from_secs(1);
        assert!(c.camera_jpeg(later).is_none());
        assert_eq!(c.view(later)["camera_v"], 0);
    }

    #[test]
    fn client_changes_are_reported_once() {
        let fake = Fake::with(MachineKind::Grbl, MachineState::Idle);
        let c = core(&fake);
        let now = Instant::now();
        // Una lista vacía no es un cambio.
        c.refresh_clients(now);
        assert!(fake.calls().is_empty());
        let (id, _) = c
            .auth
            .login("192.168.1.9".parse().unwrap(), "", &c.auth.token(), now)
            .unwrap();
        c.refresh_clients(now);
        c.refresh_clients(now);
        assert_eq!(fake.calls(), ["clients 1"]);
        c.auth.kick(&id);
        c.refresh_clients(now);
        assert_eq!(fake.calls(), ["clients 1", "clients 0"]);
    }
}
