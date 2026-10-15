//! Comandos de Tauri del historial de versiones. Todos son asíncronos y trabajan en un hilo aparte
//! (`spawn_blocking`): comprimir y escribir no bloquea la interfaz. Un solo acceso al almacén a la
//! vez (`LOCK`): la poda de una escritura no puede llevarse la versión que otra está escribiendo.

use super::config::{self, VersionsConfig};
use super::project::{self, Listing, ProjectSummary};
use super::store::{Kind, VersionInfo};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};
use tlaku_doc::Document;
use tlaku_i18n::tr;

/// Carpeta de las versiones dentro de la carpeta de datos de la app.
const DIR_NAME: &str = "versiones";

static LOCK: Mutex<()> = Mutex::new(());

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|d| d.join(DIR_NAME))
        .map_err(|e| tr!("desktop.autosave.data_dir", err = e))
}

fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|e| tr!("desktop.common.config_dir", err = e))
}

/// Carpeta raíz y configuración vigente (se lee en cada llamada: es un archivo chico).
fn context(app: &AppHandle) -> Result<(PathBuf, VersionsConfig), String> {
    Ok((root(app)?, config::load(&config_dir(app)?)))
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        f()
    })
    .await
    .map_err(|e| tr!("desktop.common.task_interrupted", err = e))?
}

#[tauri::command]
pub async fn versions_list(app: AppHandle, path: String) -> Result<Listing, String> {
    let (root, cfg) = context(&app)?;
    blocking(move || project::list(&root, &cfg, &path)).await
}

#[tauri::command]
pub async fn versions_thumb(
    app: AppHandle,
    path: String,
    id: String,
) -> Result<Option<String>, String> {
    let (root, cfg) = context(&app)?;
    blocking(move || project::read_thumb(&root, &cfg, &path, &id)).await
}

#[tauri::command]
pub async fn versions_read(app: AppHandle, path: String, id: String) -> Result<Document, String> {
    let (root, cfg) = context(&app)?;
    blocking(move || project::read_doc(&root, &cfg, &path, &id)).await
}

/// Antes de sobrescribir `path`: conserva lo que tiene ahora el archivo.
#[tauri::command]
pub async fn versions_capture(app: AppHandle, path: String) -> Result<Option<VersionInfo>, String> {
    let (root, cfg) = context(&app)?;
    blocking(move || project::capture_file(&root, &cfg, &path, now_ms())).await
}

/// Guarda el documento en memoria como versión. `kind`: `auto`, `manual` (con `label`) o `restore`.
#[tauri::command]
pub async fn versions_snapshot(
    app: AppHandle,
    path: String,
    doc: Document,
    kind: Kind,
    label: Option<String>,
) -> Result<Option<VersionInfo>, String> {
    let (root, cfg) = context(&app)?;
    blocking(move || project::snapshot(&root, &cfg, &path, &doc, kind, label, now_ms())).await
}

#[tauri::command]
pub async fn versions_delete(
    app: AppHandle,
    path: String,
    ids: Vec<String>,
) -> Result<u32, String> {
    let (root, cfg) = context(&app)?;
    blocking(move || project::delete(&root, &cfg, &path, &ids)).await
}

#[tauri::command]
pub async fn versions_clear(app: AppHandle, path: String) -> Result<u32, String> {
    let (root, cfg) = context(&app)?;
    blocking(move || project::clear(&root, &cfg, &path)).await
}

#[tauri::command]
pub async fn versions_projects(app: AppHandle) -> Result<Vec<ProjectSummary>, String> {
    let root = root(&app)?;
    blocking(move || Ok(project::projects(&root))).await
}

#[tauri::command]
pub async fn versions_config_get(app: AppHandle) -> Result<VersionsConfig, String> {
    let dir = config_dir(&app)?;
    blocking(move || Ok(config::load(&dir))).await
}

/// Guarda la configuración (acotada a rangos válidos) y devuelve la que quedó.
#[tauri::command]
pub async fn versions_config_set(
    app: AppHandle,
    config: VersionsConfig,
) -> Result<VersionsConfig, String> {
    let dir = config_dir(&app)?;
    blocking(move || config::save(&dir, &config)).await
}
