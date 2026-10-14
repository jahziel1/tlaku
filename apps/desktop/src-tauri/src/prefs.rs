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
use tlaku_i18n::tr;

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
        .map_err(|e| tr!("desktop.common.config_dir", err = e))
}

fn io_error(path: &Path, e: &std::io::Error) -> String {
    format!("{}: {e}", path.display())
}

/// Comprueba que `value` es un sobre de preferencias que esta versión puede leer.
pub fn check_envelope(value: &Value) -> Result<(), String> {
    let Some(obj) = value.as_object() else {
        return Err(tr!("desktop.prefs.not_object"));
    };
    match obj.get("version").map(Value::as_u64) {
        None => Err(tr!("desktop.prefs.no_version")),
        Some(Some(PREFS_VERSION)) => Ok(()),
        Some(_) => Err(tr!(
            "desktop.prefs.unsupported_version",
            version = obj["version"],
            current = PREFS_VERSION
        )),
    }
}

fn check_bundle(value: &Value) -> Result<(), String> {
    check_envelope(value)?;
    if value.get("format").and_then(Value::as_str) == Some(BUNDLE_FORMAT) {
        Ok(())
    } else {
        Err(tr!("desktop.prefs.not_bundle"))
    }
}

fn read_json(path: &Path) -> Result<Value, String> {
    let meta = fs::metadata(path).map_err(|e| io_error(path, &e))?;
    if meta.len() > MAX_BYTES {
        return Err(tr!("desktop.prefs.file_too_big", path = path.display()));
    }
    let text = fs::read_to_string(path).map_err(|e| io_error(path, &e))?;
    serde_json::from_str(&text)
        .map_err(|e| tr!("desktop.prefs.invalid_json", path = path.display(), err = e))
}

fn to_text(value: &Value) -> Result<String, String> {
    serde_json::to_string_pretty(value).map_err(|e| tr!("desktop.prefs.serialize_failed", err = e))
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
            Err(tr!(
                "desktop.prefs.corrupt_moved",
                path = aside.display(),
                err = e
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
        .map_err(|e| tr!("desktop.common.task_interrupted", err = e))?
}

#[tauri::command]
pub async fn save_preferences(app: AppHandle, data: Value) -> Result<(), String> {
    let dir = config_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || store(&dir, &data))
        .await
        .map_err(|e| tr!("desktop.common.task_interrupted", err = e))?
}

#[tauri::command]
pub async fn export_preferences(path: String, bundle: Value) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || export_bundle(Path::new(&path), &bundle))
        .await
        .map_err(|e| tr!("desktop.common.task_interrupted", err = e))?
}

#[tauri::command]
pub async fn import_preferences(path: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || import_bundle(Path::new(&path)))
        .await
        .map_err(|e| tr!("desktop.common.task_interrupted", err = e))?
}

// ---------------------------------------------------------------------------
// Paquetes de usuario `.tlakubundle`
// ---------------------------------------------------------------------------

/// Marca de un paquete de usuario completo (máquinas, materiales, preferencias).
pub const USER_BUNDLE_FORMAT: &str = "tlaku-bundle";
/// Versión del paquete que esta compilación entiende.
pub const USER_BUNDLE_VERSION: u64 = 1;
/// Extensión obligatoria al escribir un paquete.
const USER_BUNDLE_EXT: &str = "tlakubundle";
/// Un paquete lleva perfiles y materiales; más de esto no es un paquete razonable.
const USER_BUNDLE_MAX_BYTES: u64 = 32 * 1024 * 1024;

/// Comprueba el sobre de un paquete de usuario: objeto con `format`, `version` y, si existe,
/// `devices` como arreglo. El contenido de cada sección lo valida la interfaz.
pub fn check_user_bundle(value: &Value) -> Result<(), String> {
    let Some(obj) = value.as_object() else {
        return Err(tr!("desktop.prefs.bundle_not_object"));
    };
    if obj.get("format").and_then(Value::as_str) != Some(USER_BUNDLE_FORMAT) {
        return Err(tr!("desktop.prefs.bundle_wrong_format"));
    }
    match obj.get("version").map(Value::as_u64) {
        None => return Err(tr!("desktop.prefs.bundle_no_version")),
        Some(Some(USER_BUNDLE_VERSION)) => {}
        Some(_) => {
            return Err(tr!(
                "desktop.prefs.bundle_unsupported_version",
                version = obj["version"],
                current = USER_BUNDLE_VERSION
            ));
        }
    }
    if obj.get("devices").is_some_and(|d| !d.is_array()) {
        return Err(tr!("desktop.prefs.bundle_devices_not_list"));
    }
    Ok(())
}

pub fn export_user_bundle_file(path: &Path, bundle: &Value) -> Result<(), String> {
    let has_ext = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(USER_BUNDLE_EXT));
    if !has_ext {
        return Err(tr!(
            "desktop.prefs.bundle_needs_ext",
            path = path.display(),
            ext = USER_BUNDLE_EXT
        ));
    }
    check_user_bundle(bundle)?;
    write_atomic(path, &to_text(bundle)?)
}

pub fn import_user_bundle_file(path: &Path) -> Result<Value, String> {
    let meta = fs::metadata(path).map_err(|e| io_error(path, &e))?;
    if meta.len() > USER_BUNDLE_MAX_BYTES {
        return Err(tr!("desktop.prefs.bundle_too_big", path = path.display()));
    }
    let text = fs::read_to_string(path).map_err(|e| io_error(path, &e))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|e| tr!("desktop.prefs.invalid_json", path = path.display(), err = e))?;
    check_user_bundle(&value)?;
    Ok(value)
}

#[tauri::command]
pub async fn export_user_bundle(path: String, bundle: Value) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || export_user_bundle_file(Path::new(&path), &bundle))
        .await
        .map_err(|e| tr!("desktop.common.task_interrupted", err = e))?
}

#[tauri::command]
pub async fn import_user_bundle(path: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || import_user_bundle_file(Path::new(&path)))
        .await
        .map_err(|e| tr!("desktop.common.task_interrupted", err = e))?
}

/// Para cada ruta, si el archivo existe. Sirve para quitar de «recientes» lo que ya no está.
#[tauri::command]
pub async fn paths_exist(paths: Vec<String>) -> Result<Vec<bool>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        paths.iter().map(|p| Path::new(p).is_file()).collect()
    })
    .await
    .map_err(|e| tr!("desktop.common.task_interrupted", err = e))
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

    fn user_bundle() -> Value {
        json!({
            "format": USER_BUNDLE_FORMAT,
            "version": 1,
            "created": "2026-01-01T00:00:00.000Z",
            "app": "0.1.0",
            "devices": [{"id": "a", "name": "A"}],
            "active_device": "a",
            "materials": null,
            "prefs": null,
        })
    }

    #[test]
    fn paquete_de_usuario_ida_y_vuelta() {
        let tmp = TempDir::new();
        let file = tmp.0.join("todo.tlakubundle");
        export_user_bundle_file(&file, &user_bundle()).unwrap();
        assert_eq!(import_user_bundle_file(&file).unwrap(), user_bundle());
        assert!(!tmp.0.join("todo.tlakubundle.tmp").exists());
    }

    #[test]
    fn paquete_de_usuario_exige_extension_al_escribir() {
        let tmp = TempDir::new();
        let file = tmp.0.join("todo.json");
        let err = export_user_bundle_file(&file, &user_bundle()).unwrap_err();
        assert!(err.contains(".tlakubundle"), "{err}");
        assert!(!file.exists());
    }

    #[test]
    fn paquete_de_usuario_rechaza_formato_ajeno_y_devices_mal_formados() {
        let tmp = TempDir::new();
        let file = tmp.0.join("x.tlakubundle");
        let mut ajeno = user_bundle();
        ajeno["format"] = json!(BUNDLE_FORMAT);
        assert!(export_user_bundle_file(&file, &ajeno).is_err());
        fs::write(&file, serde_json::to_string(&ajeno).unwrap()).unwrap();
        assert!(
            import_user_bundle_file(&file)
                .unwrap_err()
                .contains("no es un paquete")
        );
        let mut mal = user_bundle();
        mal["devices"] = json!({"a": 1});
        assert!(
            export_user_bundle_file(&file, &mal)
                .unwrap_err()
                .contains("devices")
        );
        assert!(check_user_bundle(&json!([1])).is_err());
    }

    #[test]
    fn paquete_de_usuario_rechaza_version_futura() {
        let tmp = TempDir::new();
        let file = tmp.0.join("futuro.tlakubundle");
        let mut futuro = user_bundle();
        futuro["version"] = json!(2);
        assert!(export_user_bundle_file(&file, &futuro).is_err());
        fs::write(&file, serde_json::to_string(&futuro).unwrap()).unwrap();
        let err = import_user_bundle_file(&file).unwrap_err();
        assert!(err.contains("no soportada"), "{err}");
        let mut sin = user_bundle();
        sin.as_object_mut().unwrap().remove("version");
        assert!(check_user_bundle(&sin).unwrap_err().contains("version"));
    }

    #[test]
    fn paquete_de_usuario_rechaza_archivos_enormes_y_rotos() {
        let tmp = TempDir::new();
        let file = tmp.0.join("grande.tlakubundle");
        fs::write(&file, vec![b' '; (USER_BUNDLE_MAX_BYTES + 1) as usize]).unwrap();
        assert!(
            import_user_bundle_file(&file)
                .unwrap_err()
                .contains("demasiado grande")
        );
        fs::write(&file, "{ roto").unwrap();
        assert!(
            import_user_bundle_file(&file)
                .unwrap_err()
                .contains("JSON inválido")
        );
        assert!(import_user_bundle_file(&tmp.0.join("falta.tlakubundle")).is_err());
    }
}
