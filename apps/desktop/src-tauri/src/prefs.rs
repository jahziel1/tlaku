//! Preferencias del usuario y paquetes `.tlakuprefs`.
//!
//! Las preferencias viven en `preferences.json` dentro de la carpeta de configuración de la app
//! (`app_config_dir`), no en el almacenamiento de la ventana web: la carpeta de configuración
//! sobrevive a reinstalar la aplicación y es un archivo que el usuario puede copiar. Rust solo
//! valida el sobre (`version`, y `format` en los paquetes) y escribe de forma atómica; el
//! significado de cada preferencia lo conoce y lo sanea la interfaz.
//!
//! Un paquete `.tlakuprefs` es el mismo JSON con `"format": "tlaku-prefs"`.

use crate::engine::write_atomic;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const FILE: &str = "preferences.json";
/// Versión del sobre que esta compilación entiende.
pub const PREFS_VERSION: u64 = 1;
/// Marca de un paquete exportado.
pub const BUNDLE_FORMAT: &str = "tlaku-prefs";
/// Las preferencias son pequeñas; un archivo enorme es un error o algo ajeno.
const MAX_BYTES: u64 = 1024 * 1024;

fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|e| format!("No se encontró la carpeta de configuración: {e}"))
}

fn io_error(path: &Path, e: &std::io::Error) -> String {
    format!("{}: {e}", path.display())
}

/// Comprueba que `value` es un sobre de preferencias que esta versión puede leer.
pub fn check_envelope(value: &Value) -> Result<(), String> {
    let Some(obj) = value.as_object() else {
        return Err("Las preferencias no son un objeto JSON.".into());
    };
    match obj.get("version").map(Value::as_u64) {
        None => Err("Preferencias inválidas: falta el campo \"version\".".into()),
        Some(Some(PREFS_VERSION)) => Ok(()),
        Some(_) => Err(format!(
            "Versión de preferencias no soportada: {} (esta aplicación entiende la \
             {PREFS_VERSION}); actualiza la aplicación para abrirlas.",
            obj["version"]
        )),
    }
}

fn check_bundle(value: &Value) -> Result<(), String> {
    check_envelope(value)?;
    if value.get("format").and_then(Value::as_str) == Some(BUNDLE_FORMAT) {
        Ok(())
    } else {
        Err("El archivo no es un paquete de preferencias de Tlaku (.tlakuprefs).".into())
    }
}

fn read_json(path: &Path) -> Result<Value, String> {
    let meta = fs::metadata(path).map_err(|e| io_error(path, &e))?;
    if meta.len() > MAX_BYTES {
        return Err(format!(
            "{}: el archivo es demasiado grande para ser de preferencias.",
            path.display()
        ));
    }
    let text = fs::read_to_string(path).map_err(|e| io_error(path, &e))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: JSON inválido: {e}", path.display()))
}

fn to_text(value: &Value) -> Result<String, String> {
    serde_json::to_string_pretty(value).map_err(|e| format!("No se pudo serializar: {e}"))
}

/// Lee las preferencias guardadas; `None` si todavía no hay. Un archivo dañado se aparta como
/// `preferences.json.dañado` (sin perder nada) y se parte de cero; una versión desconocida es un
/// error y no se toca el archivo.
pub fn load(dir: &Path) -> Result<Option<Value>, String> {
    let file = dir.join(FILE);
    if !file.exists() {
        return Ok(None);
    }
    match read_json(&file) {
        Ok(value) => {
            check_envelope(&value)?;
            Ok(Some(value))
        }
        Err(e) => {
            let aside = dir.join(format!("{FILE}.dañado"));
            let _ = fs::rename(&file, &aside);
            Err(format!(
                "Las preferencias estaban dañadas y se apartaron en {} ({e}).",
                aside.display()
            ))
        }
    }
}

pub fn store(dir: &Path, value: &Value) -> Result<(), String> {
    check_envelope(value)?;
    fs::create_dir_all(dir).map_err(|e| io_error(dir, &e))?;
    write_atomic(&dir.join(FILE), &to_text(value)?)
}

pub fn export_bundle(path: &Path, bundle: &Value) -> Result<(), String> {
    check_bundle(bundle)?;
    write_atomic(path, &to_text(bundle)?)
}

pub fn import_bundle(path: &Path) -> Result<Value, String> {
    let value = read_json(path)?;
    check_bundle(&value)?;
    Ok(value)
}

// ---------------------------------------------------------------------------
// Comandos de Tauri
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn load_preferences(app: AppHandle) -> Result<Option<Value>, String> {
    let dir = config_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || load(&dir))
        .await
        .map_err(|e| format!("La tarea se interrumpió: {e}"))?
}

#[tauri::command]
pub async fn save_preferences(app: AppHandle, data: Value) -> Result<(), String> {
    let dir = config_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || store(&dir, &data))
        .await
        .map_err(|e| format!("La tarea se interrumpió: {e}"))?
}

#[tauri::command]
pub async fn export_preferences(path: String, bundle: Value) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || export_bundle(Path::new(&path), &bundle))
        .await
        .map_err(|e| format!("La tarea se interrumpió: {e}"))?
}

#[tauri::command]
pub async fn import_preferences(path: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || import_bundle(Path::new(&path)))
        .await
        .map_err(|e| format!("La tarea se interrumpió: {e}"))?
}

/// Para cada ruta, si el archivo existe. Sirve para quitar de «recientes» lo que ya no está.
#[tauri::command]
pub async fn paths_exist(paths: Vec<String>) -> Result<Vec<bool>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        paths.iter().map(|p| Path::new(p).is_file()).collect()
    })
    .await
    .map_err(|e| format!("La tarea se interrumpió: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("tlaku-prefs-{}-{n}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn sample() -> Value {
        json!({"version": 1, "prefs": {"snapPx": 12}, "shortcuts": {"edit.undo": ["Mod+U"]}})
    }

    #[test]
    fn sin_archivo_no_hay_preferencias() {
        let tmp = TempDir::new();
        assert_eq!(load(&tmp.0).unwrap(), None);
    }

    #[test]
    fn guarda_y_recarga_en_una_carpeta_nueva() {
        let tmp = TempDir::new();
        let dir = tmp.0.join("config/tlaku");
        store(&dir, &sample()).unwrap();
        assert_eq!(load(&dir).unwrap(), Some(sample()));
        assert!(!dir.join("preferences.json.tmp").exists());
    }

    #[test]
    fn un_archivo_danado_se_aparta() {
        let tmp = TempDir::new();
        fs::write(tmp.0.join(FILE), "{ esto no es json").unwrap();
        let err = load(&tmp.0).unwrap_err();
        assert!(err.contains("dañadas"), "{err}");
        assert!(tmp.0.join("preferences.json.dañado").exists());
        assert_eq!(load(&tmp.0).unwrap(), None);
    }

    #[test]
    fn una_version_desconocida_falla_sin_tocar_el_archivo() {
        let tmp = TempDir::new();
        let text = r#"{"version": 9}"#;
        fs::write(tmp.0.join(FILE), text).unwrap();
        let err = load(&tmp.0).unwrap_err();
        assert!(err.contains("no soportada"), "{err}");
        assert!(tmp.0.join(FILE).exists());
        assert!(!tmp.0.join("preferences.json.dañado").exists());
    }

    #[test]
    fn rechaza_sobres_invalidos() {
        let tmp = TempDir::new();
        assert!(store(&tmp.0, &json!([1, 2])).is_err());
        assert!(store(&tmp.0, &json!({"prefs": {}})).is_err());
        assert!(store(&tmp.0, &json!({"version": 2})).is_err());
    }

    #[test]
    fn exporta_e_importa_un_paquete() {
        let tmp = TempDir::new();
        let file = tmp.0.join("mis.tlakuprefs");
        let mut bundle = sample();
        bundle["format"] = json!(BUNDLE_FORMAT);
        export_bundle(&file, &bundle).unwrap();
        assert_eq!(import_bundle(&file).unwrap(), bundle);
    }

    #[test]
    fn no_exporta_ni_importa_algo_que_no_es_un_paquete() {
        let tmp = TempDir::new();
        let file = tmp.0.join("x.tlakuprefs");
        assert!(export_bundle(&file, &sample()).is_err()); // sin "format"
        assert!(!file.exists());
        fs::write(&file, serde_json::to_string(&sample()).unwrap()).unwrap();
        assert!(
            import_bundle(&file)
                .unwrap_err()
                .contains("no es un paquete")
        );
        fs::write(&file, "basura").unwrap();
        assert!(import_bundle(&file).unwrap_err().contains("JSON inválido"));
        assert!(import_bundle(&tmp.0.join("falta.tlakuprefs")).is_err());
    }

    #[test]
    fn rechaza_archivos_enormes() {
        let tmp = TempDir::new();
        let file = tmp.0.join("grande.tlakuprefs");
        fs::write(&file, vec![b' '; (MAX_BYTES + 1) as usize]).unwrap();
        assert!(
            import_bundle(&file)
                .unwrap_err()
                .contains("demasiado grande")
        );
    }
}
