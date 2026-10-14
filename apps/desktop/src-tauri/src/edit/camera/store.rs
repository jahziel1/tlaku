//! Archivo `camera.json`, comandos de guardar/cargar la configuración y el tablero imprimible.

use super::*;

pub fn setup_file(config_dir: &Path) -> PathBuf {
    config_dir.join(SETUP_FILE)
}

/// Lee la configuración. Sin archivo, devuelve una vacía. Si el archivo está dañado lo aparta
/// como `camera.json.dañado` (no se pierde) y devuelve una vacía.
pub fn load_setup(config_dir: &Path) -> Result<CameraSetup, String> {
    let file = setup_file(config_dir);
    let text = match fs::read_to_string(&file) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(CameraSetup::default()),
        Err(e) => return Err(format!("{}: {e}", file.display())),
    };
    match serde_json::from_str::<CameraSetup>(&text)
        .map_err(|e| e.to_string())
        .and_then(normalize_setup)
    {
        Ok(setup) => Ok(setup),
        Err(_) => {
            let aside = file.with_file_name(format!("{SETUP_FILE}.dañado"));
            let _ = fs::rename(&file, &aside);
            Ok(CameraSetup::default())
        }
    }
}

/// Valida y guarda; devuelve lo guardado (con la homografía recalculada).
pub fn store_setup(config_dir: &Path, setup: CameraSetup) -> Result<CameraSetup, String> {
    let setup = normalize_setup(setup)?;
    fs::create_dir_all(config_dir).map_err(|e| format!("{}: {e}", config_dir.display()))?;
    let text = serde_json::to_string_pretty(&setup)
        .map_err(|e| format!("No se pudo serializar la configuración de cámara: {e}"))?;
    write_atomic(&setup_file(config_dir), &text)?;
    Ok(setup)
}

pub(super) fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|e| format!("No se encontró la carpeta de configuración: {e}"))
}

#[tauri::command]
pub async fn load_camera_setup(app: AppHandle) -> Result<CameraSetup, String> {
    let dir = config_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || load_setup(&dir))
        .await
        .map_err(blocking_error)?
}

#[tauri::command]
pub async fn save_camera_setup(app: AppHandle, setup: CameraSetup) -> Result<CameraSetup, String> {
    let dir = config_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || store_setup(&dir, setup))
        .await
        .map_err(blocking_error)?
}

/// Guarda el tablero de calibración (SVG o PDF) que eligió el usuario en el diálogo de guardar.
pub fn write_board_file(path: &str, content_b64: &str) -> Result<(), String> {
    let p = Path::new(path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    if !matches!(ext.as_deref(), Some("svg" | "pdf")) {
        return Err("El tablero solo se guarda como .svg o .pdf.".into());
    }
    let bytes = STANDARD
        .decode(content_b64)
        .map_err(|e| format!("El contenido no es base64 válido: {e}"))?;
    fs::write(p, bytes).map_err(|e| format!("{}: {e}", p.display()))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn save_board_file(path: String, content_base64: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || write_board_file(&path, &content_base64))
        .await
        .map_err(blocking_error)?
}
