//! Comandos de Tauri (`invoke`): adaptan argumentos y delegan en `engine` (motor y archivos)
//! y en `machine` (conexión). Los pesados corren con `spawn_blocking`, fuera del hilo principal.

use crate::engine::{self, Head};
use crate::machine::{Connection, Emit, Handle, NOT_CONNECTED, Sink, Timing};
use laser_doc::{
    ConnectResult, DeviceProfile, Document, ExportResult, ImportResult, JobOptions, JobPreview,
    RealtimeCommand,
};
use laser_grbl::serial::Grbl;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

/// Estado administrado por Tauri: la conexión con la máquina (si hay una).
#[derive(Default)]
pub struct AppState {
    conn: Mutex<Option<Connection>>,
    /// Hay un `connect` en curso (abrir el puerto tarda ~2 s por el reinicio de la placa).
    connecting: AtomicBool,
}

impl AppState {
    /// Mando de la conexión viva. Si la conexión se perdió, la suelta y avisa.
    fn handle(&self) -> Result<Handle, String> {
        let mut g = self.conn.lock().map_err(|_| "estado interno dañado")?;
        match g.as_ref().map(Connection::handle) {
            Some(h) if !h.is_closed() => Ok(h),
            Some(_) => {
                *g = None;
                Err(NOT_CONNECTED.into())
            }
            None => Err(NOT_CONNECTED.into()),
        }
    }

    /// Posición de trabajo de la cabeza, si hay máquina conectada con estado conocido.
    fn head(&self) -> Head {
        self.handle().ok().and_then(|h| h.work_pos())
    }

    /// Al cerrar la app: cancela un trabajo en curso y suelta el puerto.
    pub fn shutdown(&self) {
        if let Ok(mut g) = self.conn.lock()
            && let Some(c) = g.take()
        {
            c.close();
        }
    }
}

/// Emite los eventos de la conexión a la ventana (`machine://*`).
struct TauriSink(AppHandle);

impl Sink for TauriSink {
    fn emit(&self, event: Emit) {
        let name = event.name();
        let _ = match event {
            Emit::Status(p) => self.0.emit(name, p),
            Emit::Progress(p) => self.0.emit(name, p),
            Emit::Log(p) => self.0.emit(name, p),
            Emit::Job(p) => self.0.emit(name, p),
            Emit::Connection(p) => self.0.emit(name, p),
        };
    }
}

/// Ejecuta `f` en el pool de hilos bloqueantes.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("La tarea se interrumpió: {e}"))?
}

fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|e| format!("No se encontró la carpeta de configuración: {e}"))
}

/// Permite que la interfaz muestre estas imágenes con `convertFileSrc` (protocolo de assets).
fn allow_images(app: &AppHandle, paths: impl IntoIterator<Item = PathBuf>) {
    let scope = app.asset_protocol_scope();
    for p in paths {
        let _ = scope.allow_file(p);
    }
}

// ---------------------------------------------------------------------------
// Aplicación
// ---------------------------------------------------------------------------

/// Comando de prueba: confirma que la interfaz habla con Rust.
#[tauri::command]
pub fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// ---------------------------------------------------------------------------
// Archivos y proyecto
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn import_file(app: AppHandle, path: String) -> Result<ImportResult, String> {
    let result = blocking(move || engine::import(&path)).await?;
    let images = result.objects.iter().filter_map(|o| match &o.kind {
        laser_doc::ObjectKind::Image(i) => Some(PathBuf::from(&i.source_path)),
        laser_doc::ObjectKind::Vector(_) => None,
    });
    allow_images(&app, images);
    Ok(result)
}

#[tauri::command]
pub async fn save_project(path: String, doc: Document) -> Result<(), String> {
    blocking(move || engine::save_project(&path, &doc)).await
}

#[tauri::command]
pub async fn load_project(app: AppHandle, path: String) -> Result<Document, String> {
    let doc = blocking(move || engine::load_project(&path)).await?;
    allow_images(&app, engine::image_paths(&doc));
    Ok(doc)
}

// ---------------------------------------------------------------------------
// Trabajo
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn compute_job(
    state: State<'_, AppState>,
    doc: Document,
    device: DeviceProfile,
    options: JobOptions,
) -> Result<JobPreview, String> {
    let head = state.head();
    blocking(move || engine::compute_job(&doc, &device, &options, head)).await
}

#[tauri::command]
pub async fn export_gcode(
    state: State<'_, AppState>,
    doc: Document,
    device: DeviceProfile,
    options: JobOptions,
    path: String,
) -> Result<ExportResult, String> {
    let head = state.head();
    blocking(move || engine::export_gcode(&doc, &device, &options, &path, head)).await
}

// ---------------------------------------------------------------------------
// Dispositivos
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_devices(app: AppHandle) -> Result<Vec<DeviceProfile>, String> {
    let dir = config_dir(&app)?;
    blocking(move || engine::list_devices(&dir)).await
}

#[tauri::command]
pub async fn save_device(app: AppHandle, device: DeviceProfile) -> Result<(), String> {
    let dir = config_dir(&app)?;
    blocking(move || engine::save_device(&dir, device)).await
}

#[tauri::command]
pub async fn delete_device(app: AppHandle, id: String) -> Result<(), String> {
    let dir = config_dir(&app)?;
    blocking(move || engine::delete_device(&dir, &id)).await
}

// ---------------------------------------------------------------------------
// Máquina
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_ports() -> Result<Vec<String>, String> {
    blocking(|| {
        laser_grbl::serial::list_ports()
            .map_err(|e| format!("No se pudieron listar los puertos serie: {e}"))
    })
    .await
}

#[tauri::command]
pub async fn connect(
    app: AppHandle,
    state: State<'_, AppState>,
    port: String,
    baud: u32,
) -> Result<ConnectResult, String> {
    if baud == 0 {
        return Err("Los baudios deben ser mayores que 0.".into());
    }
    if state.handle().is_ok() {
        return Err(
            "Ya hay una máquina conectada. Desconéctala antes de abrir otro puerto.".into(),
        );
    }
    if state.connecting.swap(true, Ordering::SeqCst) {
        return Err("Ya se está conectando con una máquina; espera un momento.".into());
    }
    let sink: Arc<dyn Sink> = Arc::new(TauriSink(app));
    let result = blocking(move || {
        // Abrir el puerto reinicia muchas placas: `Grbl::open` espera el saludo y lo descarta.
        let transport = Grbl::open(&port, baud)
            .map_err(|e| {
                format!(
                    "No se pudo abrir {port}: {e}. Revisa el cable, que la máquina esté \
                     encendida y que ningún otro programa use el puerto."
                )
            })?
            .into_transport();
        Connection::start(transport, &port, sink, Timing::default())
    })
    .await;
    state.connecting.store(false, Ordering::SeqCst);
    let (conn, info) = result?;
    *state.conn.lock().map_err(|_| "estado interno dañado")? = Some(conn);
    Ok(info)
}

#[tauri::command]
pub async fn disconnect(state: State<'_, AppState>) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|_| "estado interno dañado")?
        .take();
    if let Some(c) = conn {
        blocking(move || {
            c.close();
            Ok(())
        })
        .await?;
    }
    Ok(())
}

/// Ejecuta `f` con el mando de la conexión, fuera del hilo principal.
async fn with_handle<T: Send + 'static>(
    state: &State<'_, AppState>,
    f: impl FnOnce(Handle) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let h = state.handle()?;
    blocking(move || f(h)).await
}

#[tauri::command]
pub async fn machine_command(
    state: State<'_, AppState>,
    line: String,
) -> Result<Vec<String>, String> {
    with_handle(&state, move |h| h.command(&line)).await
}

#[tauri::command]
pub async fn machine_realtime(
    state: State<'_, AppState>,
    cmd: RealtimeCommand,
) -> Result<(), String> {
    with_handle(&state, move |h| h.realtime(cmd)).await
}

#[tauri::command]
pub async fn jog(state: State<'_, AppState>, dx: f64, dy: f64, speed: f64) -> Result<(), String> {
    with_handle(&state, move |h| h.jog(dx, dy, speed)).await
}

#[tauri::command]
pub async fn start_job(
    state: State<'_, AppState>,
    doc: Document,
    device: DeviceProfile,
    options: JobOptions,
) -> Result<(), String> {
    let h = state.handle()?;
    blocking(move || {
        let lines = engine::job_lines(&doc, &device, &options, h.work_pos())?;
        h.start_job(lines, device.rx_buffer as usize)
    })
    .await
}

#[tauri::command]
pub async fn frame_job(
    state: State<'_, AppState>,
    doc: Document,
    device: DeviceProfile,
    options: JobOptions,
    speed: f64,
) -> Result<(), String> {
    let h = state.handle()?;
    blocking(move || {
        let lines = engine::frame_lines(&doc, &device, &options, speed, h.work_pos())?;
        h.frame(lines, device.rx_buffer as usize)
    })
    .await
}

#[tauri::command]
pub async fn pause_job(state: State<'_, AppState>) -> Result<(), String> {
    with_handle(&state, |h| h.pause()).await
}

#[tauri::command]
pub async fn resume_job(state: State<'_, AppState>) -> Result<(), String> {
    with_handle(&state, |h| h.resume()).await
}

#[tauri::command]
pub async fn cancel_job(state: State<'_, AppState>) -> Result<(), String> {
    with_handle(&state, |h| h.cancel()).await
}
