//! Comandos del área `materials` (fase 4): biblioteca de materiales y prueba de materiales.

use tauri::AppHandle;
use tlaku_pipeline::doc::{Document, MaterialLibrary, MaterialTestParams};

/// Biblioteca del usuario (`materials.json` en la carpeta de configuración). Sin archivo, vacía.
#[tauri::command]
pub async fn load_material_library(app: AppHandle) -> Result<MaterialLibrary, String> {
    // TODO(fase4)
    let _ = app;
    Ok(MaterialLibrary::default())
}

#[tauri::command]
pub async fn save_material_library(app: AppHandle, library: MaterialLibrary) -> Result<(), String> {
    // TODO(fase4)
    let _ = (app, library);
    Err("aún no implementado".into())
}

/// Lee un archivo de biblioteca (`.tlakumat`) elegido por el usuario.
#[tauri::command]
pub async fn import_material_library(path: String) -> Result<MaterialLibrary, String> {
    // TODO(fase4)
    let _ = path;
    Err("aún no implementado".into())
}

#[tauri::command]
pub async fn export_material_library(path: String, library: MaterialLibrary) -> Result<(), String> {
    // TODO(fase4)
    let _ = (path, library);
    Err("aún no implementado".into())
}

#[tauri::command]
pub async fn material_test(params: MaterialTestParams) -> Result<Document, String> {
    tauri::async_runtime::spawn_blocking(move || tlaku_pipeline::material_test(&params))
        .await
        .map_err(|e| format!("La tarea se interrumpió: {e}"))?
}
