//! Comandos del área `materials` (fase 4): biblioteca de materiales y prueba de materiales.
//!
//! La biblioteca del usuario vive en `materials.json` dentro de la carpeta de configuración de la
//! app. Los archivos `.tlakumat` que se importan y exportan son el mismo JSON de `MaterialLibrary`.
//!
//! La biblioteca de fábrica (valores de partida con su fuente, ver `tlaku_doc::factory_library`)
//! viaja dentro del programa y es de solo lectura. Al arrancar se deja una copia en
//! `materials-factory.json` (la primera vez la instala, y cuando una versión nueva de la app trae
//! datos distintos la reemplaza). Es un archivo aparte: **nunca toca `materials.json`**, así que
//! actualizar la fábrica no pisa los ajustes del usuario.

use crate::engine::write_atomic;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use tlaku_pipeline::doc::{
    Document, MATERIAL_LIBRARY_VERSION, MaterialLibrary, MaterialTestParams, factory_library,
};

/// Archivo de la biblioteca dentro de la carpeta de configuración de la app.
const LIBRARY_FILE: &str = "materials.json";

/// Copia instalada de la biblioteca de fábrica (aparte de la del usuario).
const FACTORY_FILE: &str = "materials-factory.json";

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

pub fn factory_file(config_dir: &Path) -> PathBuf {
    config_dir.join(FACTORY_FILE)
}

/// Qué pasó con la copia instalada de la biblioteca de fábrica.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactorySync {
    /// No había copia (primer arranque): se instaló.
    Installed,
    /// La copia era de otra versión o estaba dañada: se reemplazó.
    Updated,
    /// Ya estaba al día; no se escribió nada.
    UpToDate,
    /// No se pudo escribir la copia (la biblioteca que se devuelve es la del programa de todos modos).
    Failed(String),
}

/// Deja al día la copia de la biblioteca de fábrica en `config_dir` y devuelve la que trae esta
/// versión de la app. Solo toca `materials-factory.json`.
pub fn sync_factory(config_dir: &Path) -> Result<(MaterialLibrary, FactorySync), String> {
    let bundled = factory_library()?;
    let file = factory_file(config_dir);
    let installed = fs::read_to_string(&file).ok();
    let current = installed
        .as_deref()
        .and_then(|text| serde_json::from_str::<MaterialLibrary>(text).ok())
        .is_some_and(|lib| lib == bundled);
    if current {
        return Ok((bundled, FactorySync::UpToDate));
    }
    let result = serde_json::to_string(&bundled)
        .map_err(|e| format!("No se pudo serializar la biblioteca de fábrica: {e}"))
        .and_then(|json| {
            fs::create_dir_all(config_dir).map_err(|e| format!("{}: {e}", config_dir.display()))?;
            write_atomic(&file, &json)
        });
    let status = match (result, installed.is_some()) {
        (Err(e), _) => FactorySync::Failed(e),
        (Ok(()), true) => FactorySync::Updated,
        (Ok(()), false) => FactorySync::Installed,
    };
    Ok((bundled, status))
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

/// Biblioteca de fábrica de esta versión (solo lectura): instala o actualiza su copia y la devuelve.
/// Si la copia no se puede escribir, igual se devuelve (el programa trae los datos).
#[tauri::command]
pub async fn load_factory_materials(app: AppHandle) -> Result<MaterialLibrary, String> {
    let dir = config_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || sync_factory(&dir).map(|(library, _)| library))
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
            role: None,
            machine_class: None,
            source: None,
            confidence: None,
            material_key: None,
            title_key: None,
            warnings: Vec::new(),
        }
    }

    fn sample() -> MaterialLibrary {
        MaterialLibrary {
            version: MATERIAL_LIBRARY_VERSION,
            presets: vec![preset("a", "MDF"), preset("b", "Acrílico")],
            revision: 0,
            restrictions: Vec::new(),
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

    // ---- biblioteca de fábrica ------------------------------------------------

    #[test]
    fn el_primer_arranque_instala_la_biblioteca_de_fabrica() {
        let tmp = TempDir::new();
        let (lib, status) = sync_factory(&tmp.0).unwrap();
        assert_eq!(status, FactorySync::Installed);
        assert!(lib.presets.len() > 50);
        // La copia instalada es la misma biblioteca, y no queda el temporal.
        let text = fs::read_to_string(factory_file(&tmp.0)).unwrap();
        assert_eq!(serde_json::from_str::<MaterialLibrary>(&text).unwrap(), lib);
        assert!(!tmp.0.join("materials-factory.json.tmp").exists());
        // Sin biblioteca del usuario: sigue vacía, no se creó nada a su nombre.
        assert!(!library_file(&tmp.0).exists());
        assert_eq!(load_library(&tmp.0).unwrap(), MaterialLibrary::default());
    }

    #[test]
    fn un_arranque_con_la_copia_al_dia_no_escribe() {
        let tmp = TempDir::new();
        sync_factory(&tmp.0).unwrap();
        let before = fs::metadata(factory_file(&tmp.0))
            .unwrap()
            .modified()
            .unwrap();
        let (_, status) = sync_factory(&tmp.0).unwrap();
        assert_eq!(status, FactorySync::UpToDate);
        assert_eq!(
            fs::metadata(factory_file(&tmp.0))
                .unwrap()
                .modified()
                .unwrap(),
            before
        );
    }

    #[test]
    fn una_version_nueva_actualiza_la_fabrica_sin_pisar_al_usuario() {
        let tmp = TempDir::new();
        // El usuario tiene sus ajustes (uno con el nombre de un material de fábrica).
        let mut mine = sample();
        mine.presets.push(preset("mi-mdf", "MDF"));
        store_library(&tmp.0, &mine).unwrap();
        let user_before = fs::read(library_file(&tmp.0)).unwrap();
        // Copia de otra versión de la app: revisión vieja y un ajuste de menos.
        let (bundled, _) = sync_factory(&tmp.0).unwrap();
        let mut old = bundled.clone();
        old.revision = bundled.revision.saturating_sub(1);
        old.presets.pop();
        fs::write(factory_file(&tmp.0), serde_json::to_string(&old).unwrap()).unwrap();

        let (lib, status) = sync_factory(&tmp.0).unwrap();
        assert_eq!(status, FactorySync::Updated);
        assert_eq!(lib, bundled);
        let text = fs::read_to_string(factory_file(&tmp.0)).unwrap();
        assert_eq!(
            serde_json::from_str::<MaterialLibrary>(&text).unwrap(),
            bundled
        );
        // La biblioteca del usuario quedó byte por byte igual.
        assert_eq!(fs::read(library_file(&tmp.0)).unwrap(), user_before);
        assert_eq!(load_library(&tmp.0).unwrap(), mine);
    }

    #[test]
    fn una_copia_danada_o_manipulada_se_reinstala() {
        let tmp = TempDir::new();
        sync_factory(&tmp.0).unwrap();
        fs::write(factory_file(&tmp.0), "{ no es json").unwrap();
        assert_eq!(sync_factory(&tmp.0).unwrap().1, FactorySync::Updated);
        // Mismo número de revisión pero valores cambiados a mano: se devuelve y se deja la de fábrica.
        let (bundled, _) = sync_factory(&tmp.0).unwrap();
        let mut edited = bundled.clone();
        edited.presets[0].settings.power = 1.0;
        fs::write(
            factory_file(&tmp.0),
            serde_json::to_string(&edited).unwrap(),
        )
        .unwrap();
        let (lib, status) = sync_factory(&tmp.0).unwrap();
        assert_eq!((status, lib), (FactorySync::Updated, bundled));
    }

    #[test]
    fn si_no_se_puede_escribir_la_fabrica_se_devuelve_igual() {
        let tmp = TempDir::new();
        // La "carpeta de configuración" es un archivo: no se puede crear nada dentro.
        fs::create_dir_all(&tmp.0).unwrap();
        let blocked = tmp.0.join("no-es-carpeta");
        fs::write(&blocked, "x").unwrap();
        let (lib, status) = sync_factory(&blocked).unwrap();
        assert!(matches!(status, FactorySync::Failed(_)));
        assert!(!lib.presets.is_empty());
    }

    #[test]
    fn la_biblioteca_del_usuario_no_lleva_nada_de_la_fabrica() {
        let tmp = TempDir::new();
        sync_factory(&tmp.0).unwrap();
        store_library(&tmp.0, &sample()).unwrap();
        let user = load_library(&tmp.0).unwrap();
        assert_eq!(user, sample());
        assert!(user.presets.iter().all(|p| !p.id.starts_with("factory.")));
    }
}
