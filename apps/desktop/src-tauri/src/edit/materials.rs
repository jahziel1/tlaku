//! Comandos del área `materials` (fase 4): biblioteca de materiales y prueba de materiales.
//!
//! La biblioteca del usuario vive en `materials.json` dentro de la carpeta de configuración de la
//! app. Los archivos `.tlakumat` que se importan y exportan son el mismo JSON de `MaterialLibrary`.

use crate::engine::write_atomic;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use tlaku_pipeline::doc::{
    Document, MATERIAL_LIBRARY_VERSION, MaterialLibrary, MaterialTestParams,
};

/// Archivo de la biblioteca dentro de la carpeta de configuración de la app.
const LIBRARY_FILE: &str = "materials.json";

fn blocking_error(e: tauri::Error) -> String {
    format!("La tarea se interrumpió: {e}")
}

fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|e| format!("No se encontró la carpeta de configuración: {e}"))
}

pub fn library_file(config_dir: &Path) -> PathBuf {
    config_dir.join(LIBRARY_FILE)
}

/// Lee una biblioteca desde texto JSON, validando primero la versión (para dar un mensaje claro
/// con archivos de otra versión de la app) y después los datos.
pub fn parse_library(text: &str) -> Result<MaterialLibrary, String> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|e| format!("Biblioteca de materiales inválida: {e}"))?;
    match value.get("version").map(|v| v.as_u64()) {
        None => {
            return Err("Biblioteca de materiales inválida: falta el campo \"version\".".into());
        }
        Some(Some(v)) if v == u64::from(MATERIAL_LIBRARY_VERSION) => {}
        Some(_) => {
            return Err(format!(
                "Versión de biblioteca no soportada: {} (esta aplicación entiende la \
                 {MATERIAL_LIBRARY_VERSION}); actualiza la aplicación para abrirla.",
                value["version"]
            ));
        }
    }
    let library: MaterialLibrary = serde_json::from_value(value)
        .map_err(|e| format!("Biblioteca de materiales inválida: {e}"))?;
    check_library(&library)?;
    Ok(library)
}

/// Comprueba lo que el formato no garantiza: ids no vacíos y únicos, material con nombre.
pub fn check_library(library: &MaterialLibrary) -> Result<(), String> {
    if library.version != MATERIAL_LIBRARY_VERSION {
        return Err(format!(
            "Versión de biblioteca no soportada: {}.",
            library.version
        ));
    }
    let mut seen = HashSet::new();
    for (i, p) in library.presets.iter().enumerate() {
        if p.id.trim().is_empty() {
            return Err(format!("El ajuste {} no tiene identificador.", i + 1));
        }
        if !seen.insert(p.id.as_str()) {
            return Err(format!("El identificador \"{}\" está repetido.", p.id));
        }
        if p.material.trim().is_empty() {
            return Err(format!("El ajuste \"{}\" no tiene material.", p.id));
        }
        if let Some(t) = p.thickness
            && (!t.is_finite() || t < 0.0)
        {
            return Err(format!("El grosor de \"{}\" no es válido.", p.material));
        }
    }
    Ok(())
}

/// Lee la biblioteca del usuario. Sin archivo, devuelve una vacía. Si el archivo no es JSON o
/// no tiene la forma esperada lo aparta como `materials.json.dañado` (no se pierde) y devuelve
/// una vacía; si es de una versión que esta app no conoce, falla sin tocarlo.
pub fn load_library(config_dir: &Path) -> Result<MaterialLibrary, String> {
    let file = library_file(config_dir);
    let text = match fs::read_to_string(&file) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(MaterialLibrary::default());
        }
        Err(e) => return Err(format!("{}: {e}", file.display())),
    };
    match parse_library(&text) {
        Ok(library) => Ok(library),
        Err(e) if e.starts_with("Versión de biblioteca no soportada") => Err(e),
        Err(_) => {
            let aside = file.with_file_name(format!("{LIBRARY_FILE}.dañado"));
            let _ = fs::rename(&file, &aside);
            Ok(MaterialLibrary::default())
        }
    }
}

fn to_json(library: &MaterialLibrary) -> Result<String, String> {
    serde_json::to_string_pretty(library)
        .map_err(|e| format!("No se pudo serializar la biblioteca de materiales: {e}"))
}

/// Valida y guarda la biblioteca del usuario con escritura atómica.
pub fn store_library(config_dir: &Path, library: &MaterialLibrary) -> Result<(), String> {
    check_library(library)?;
    fs::create_dir_all(config_dir).map_err(|e| format!("{}: {e}", config_dir.display()))?;
    write_atomic(&library_file(config_dir), &to_json(library)?)
}

/// Lee un archivo `.tlakumat`.
pub fn read_library_file(path: &Path) -> Result<MaterialLibrary, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse_library(&text)
}

/// Escribe un archivo `.tlakumat`.
pub fn write_library_file(path: &Path, library: &MaterialLibrary) -> Result<(), String> {
    check_library(library)?;
    write_atomic(path, &to_json(library)?)
}

/// Biblioteca del usuario (`materials.json` en la carpeta de configuración). Sin archivo, vacía.
#[tauri::command]
pub async fn load_material_library(app: AppHandle) -> Result<MaterialLibrary, String> {
    let dir = config_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || load_library(&dir))
        .await
        .map_err(blocking_error)?
}

#[tauri::command]
pub async fn save_material_library(app: AppHandle, library: MaterialLibrary) -> Result<(), String> {
    let dir = config_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || store_library(&dir, &library))
        .await
        .map_err(blocking_error)?
}

/// Lee un archivo de biblioteca (`.tlakumat`) elegido por el usuario.
#[tauri::command]
pub async fn import_material_library(path: String) -> Result<MaterialLibrary, String> {
    tauri::async_runtime::spawn_blocking(move || read_library_file(Path::new(&path)))
        .await
        .map_err(blocking_error)?
}

#[tauri::command]
pub async fn export_material_library(path: String, library: MaterialLibrary) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || write_library_file(Path::new(&path), &library))
        .await
        .map_err(blocking_error)?
}

#[tauri::command]
pub async fn material_test(params: MaterialTestParams) -> Result<Document, String> {
    tauri::async_runtime::spawn_blocking(move || tlaku_pipeline::material_test(&params))
        .await
        .map_err(|e| format!("La tarea se interrumpió: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tlaku_pipeline::doc::{LayerSettings, MaterialPreset};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir =
                std::env::temp_dir().join(format!("tlaku-materials-{}-{n}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn preset(id: &str, material: &str) -> MaterialPreset {
        MaterialPreset {
            id: id.into(),
            material: material.into(),
            thickness: Some(3.0),
            title: "Corte".into(),
            notes: "dos pasadas".into(),
            device_id: None,
            settings: LayerSettings::default(),
        }
    }

    fn sample() -> MaterialLibrary {
        MaterialLibrary {
            version: MATERIAL_LIBRARY_VERSION,
            presets: vec![preset("a", "MDF"), preset("b", "Acrílico")],
        }
    }

    #[test]
    fn la_biblioteca_sobrevive_al_reinicio() {
        let tmp = TempDir::new();
        assert_eq!(load_library(&tmp.0).unwrap(), MaterialLibrary::default());
        store_library(&tmp.0, &sample()).unwrap();
        assert_eq!(load_library(&tmp.0).unwrap(), sample());
        // La escritura atómica no deja el temporal.
        assert!(!tmp.0.join("materials.json.tmp").exists());
    }

    #[test]
    fn un_archivo_danado_se_aparta_sin_perder_nada() {
        let tmp = TempDir::new();
        fs::create_dir_all(&tmp.0).unwrap();
        fs::write(library_file(&tmp.0), "{ esto no es json").unwrap();
        assert_eq!(load_library(&tmp.0).unwrap(), MaterialLibrary::default());
        assert!(tmp.0.join("materials.json.dañado").exists());
        assert!(!library_file(&tmp.0).exists());
    }

    #[test]
    fn una_version_desconocida_falla_sin_tocar_el_archivo() {
        let tmp = TempDir::new();
        fs::create_dir_all(&tmp.0).unwrap();
        fs::write(library_file(&tmp.0), r#"{"version": 9, "presets": []}"#).unwrap();
        let err = load_library(&tmp.0).unwrap_err();
        assert!(err.contains("no soportada"), "{err}");
        assert!(library_file(&tmp.0).exists());
        assert!(!tmp.0.join("materials.json.dañado").exists());
    }

    #[test]
    fn rechaza_ids_repetidos_y_datos_incompletos() {
        let tmp = TempDir::new();
        let mut lib = sample();
        lib.presets[1].id = "a".into();
        assert!(
            store_library(&tmp.0, &lib)
                .unwrap_err()
                .contains("repetido")
        );
        let mut lib = sample();
        lib.presets[0].material = "  ".into();
        assert!(
            store_library(&tmp.0, &lib)
                .unwrap_err()
                .contains("material")
        );
        let mut lib = sample();
        lib.presets[0].thickness = Some(-1.0);
        assert!(store_library(&tmp.0, &lib).unwrap_err().contains("grosor"));
        let mut lib = sample();
        lib.presets[0].id.clear();
        assert!(store_library(&tmp.0, &lib).is_err());
        // Nada se escribió.
        assert!(!library_file(&tmp.0).exists());
    }

    #[test]
    fn exporta_e_importa_un_tlakumat() {
        let tmp = TempDir::new();
        fs::create_dir_all(&tmp.0).unwrap();
        let file = tmp.0.join("mis-ajustes.tlakumat");
        write_library_file(&file, &sample()).unwrap();
        assert_eq!(read_library_file(&file).unwrap(), sample());
    }

    #[test]
    fn la_importacion_valida_la_version_y_la_forma() {
        let tmp = TempDir::new();
        fs::create_dir_all(&tmp.0).unwrap();
        let file = tmp.0.join("x.tlakumat");
        fs::write(&file, r#"{"version": 2, "presets": []}"#).unwrap();
        assert!(
            read_library_file(&file)
                .unwrap_err()
                .contains("no soportada")
        );
        fs::write(&file, r#"{"presets": []}"#).unwrap();
        assert!(read_library_file(&file).unwrap_err().contains("version"));
        fs::write(&file, "no es json").unwrap();
        assert!(read_library_file(&file).unwrap_err().contains("inválida"));
        fs::write(&file, r#"{"version": 1, "presets": [{"id": "x"}]}"#).unwrap();
        assert!(read_library_file(&file).unwrap_err().contains("inválida"));
        assert!(read_library_file(&tmp.0.join("no-existe.tlakumat")).is_err());
    }

    #[test]
    fn la_importacion_rechaza_ids_repetidos() {
        let tmp = TempDir::new();
        fs::create_dir_all(&tmp.0).unwrap();
        let mut lib = sample();
        lib.presets[1].id = "a".into();
        let file = tmp.0.join("dup.tlakumat");
        fs::write(&file, serde_json::to_string(&lib).unwrap()).unwrap();
        assert!(read_library_file(&file).unwrap_err().contains("repetido"));
    }
}
