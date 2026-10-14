//! Plantillas de proyecto: documentos (diseño, capas y ajustes) guardados en la carpeta de
//! configuración de la app para empezar proyectos nuevos desde ellos.
//!
//! Cada plantilla es un archivo `<nombre>.tlakutemplate` dentro de `<configuración>/plantillas/`
//! con el mismo JSON que un proyecto. El nombre del archivo es el nombre que ve el usuario, así
//! que se valida: sin separadores de ruta ni caracteres que algún sistema de archivos rechace.
//! Se escribe de forma atómica; una plantilla ilegible se omite de la lista en lugar de romperla.

use crate::engine::write_atomic;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use tauri::{AppHandle, Manager};
use tlaku_doc::Document;
use tlaku_i18n::tr;

const DIR_NAME: &str = "plantillas";
const EXT: &str = "tlakutemplate";
/// Una plantilla pesa lo que un proyecto, pero un archivo enorme no es una plantilla.
const MAX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_NAME_CHARS: usize = 80;

/// Lo que se muestra de una plantilla sin abrirla.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateInfo {
    pub name: String,
    /// Última modificación, en milisegundos desde 1970.
    pub saved_at_ms: u64,
    pub size_bytes: u64,
}

fn io_error(path: &Path, e: &std::io::Error) -> String {
    format!("{}: {e}", path.display())
}

/// El nombre se usa como nombre de archivo: no vacío, corto, sin separadores ni caracteres
/// reservados de Windows, sin punto inicial ni espacios (o puntos) al final.
pub fn check_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    let bad_char = |c: char| c.is_control() || "/\\:*?\"<>|".contains(c);
    if name.is_empty() {
        Err(tr!("desktop.templates.name_empty"))
    } else if name.chars().count() > MAX_NAME_CHARS {
        Err(tr!("desktop.templates.name_too_long", max = MAX_NAME_CHARS))
    } else if name.chars().any(bad_char) || name.starts_with('.') || name.ends_with('.') {
        Err(tr!("desktop.templates.name_invalid", name = name))
    } else {
        Ok(name)
    }
}

fn file_of(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.{EXT}"))
}

fn info_of(path: &Path) -> Option<TemplateInfo> {
    if path.extension()?.to_str()? != EXT {
        return None;
    }
    let name = path.file_stem()?.to_str()?.to_string();
    check_name(&name).ok()?;
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let saved_at_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64);
    Some(TemplateInfo {
        name,
        saved_at_ms,
        size_bytes: meta.len(),
    })
}

/// Plantillas guardadas, por nombre (sin distinguir mayúsculas).
pub fn list(dir: &Path) -> Vec<TemplateInfo> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<TemplateInfo> = entries
        .flatten()
        .filter_map(|e| info_of(&e.path()))
        .collect();
    out.sort_by_key(|t| (t.name.to_lowercase(), t.name.clone()));
    out
}

/// Guarda (o reemplaza) la plantilla `name`.
pub fn save(dir: &Path, name: &str, doc: &Document) -> Result<TemplateInfo, String> {
    let name = check_name(name)?;
    let json = doc.to_json()?;
    if json.len() as u64 > MAX_BYTES {
        return Err(tr!("desktop.templates.doc_too_big"));
    }
    fs::create_dir_all(dir).map_err(|e| io_error(dir, &e))?;
    let file = file_of(dir, name);
    write_atomic(&file, &json)?;
    info_of(&file).ok_or_else(|| tr!("desktop.templates.read_back_failed", path = file.display()))
}

pub fn load(dir: &Path, name: &str) -> Result<Document, String> {
    let name = check_name(name)?;
    let file = file_of(dir, name);
    let meta = fs::metadata(&file).map_err(|_| tr!("desktop.templates.not_found", name = name))?;
    if meta.len() > MAX_BYTES {
        return Err(tr!("desktop.templates.too_big", name = name));
    }
    let text = fs::read_to_string(&file).map_err(|e| io_error(&file, &e))?;
    Document::from_json(&text)
        .map_err(|e| tr!("desktop.templates.load_failed", name = name, err = e))
}

/// Borra la plantilla. No es error que no exista.
pub fn remove(dir: &Path, name: &str) -> Result<(), String> {
    let name = check_name(name)?;
    let file = file_of(dir, name);
    match fs::remove_file(&file) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_error(&file, &e)),
    }
}

// ---------------------------------------------------------------------------
// Comandos de Tauri
// ---------------------------------------------------------------------------

fn templates_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|d| d.join(DIR_NAME))
        .map_err(|e| tr!("desktop.common.config_dir", err = e))
}

async fn blocking<T: Send + 'static>(
    job: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(job)
        .await
        .map_err(|e| tr!("desktop.common.task_interrupted", err = e))?
}

#[tauri::command]
pub async fn list_templates(app: AppHandle) -> Result<Vec<TemplateInfo>, String> {
    let dir = templates_dir(&app)?;
    blocking(move || Ok(list(&dir))).await
}

#[tauri::command]
pub async fn save_template(
    app: AppHandle,
    name: String,
    doc: Document,
) -> Result<TemplateInfo, String> {
    let dir = templates_dir(&app)?;
    blocking(move || save(&dir, &name, &doc)).await
}

#[tauri::command]
pub async fn load_template(app: AppHandle, name: String) -> Result<Document, String> {
    let dir = templates_dir(&app)?;
    blocking(move || load(&dir, &name)).await
}

#[tauri::command]
pub async fn delete_template(app: AppHandle, name: String) -> Result<(), String> {
    let dir = templates_dir(&app)?;
    blocking(move || remove(&dir, &name)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir =
                std::env::temp_dir().join(format!("tlaku-plantillas-{}-{n}", std::process::id()));
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

    fn doc_with_notes(notes: &str) -> Document {
        Document {
            notes: notes.into(),
            ..Document::default()
        }
    }

    #[test]
    fn guarda_lista_carga_y_borra() {
        let tmp = TempDir::new();
        let dir = tmp.0.join("config/plantillas");
        assert!(list(&dir).is_empty());
        let info = save(&dir, "  Llavero 3 mm ", &doc_with_notes("hola")).unwrap();
        assert_eq!(info.name, "Llavero 3 mm");
        save(&dir, "acrílico", &doc_with_notes("otra")).unwrap();
        let names: Vec<_> = list(&dir).into_iter().map(|t| t.name).collect();
        assert_eq!(names, ["acrílico", "Llavero 3 mm"]);
        assert_eq!(load(&dir, "Llavero 3 mm").unwrap().notes, "hola");
        // Guardar con el mismo nombre reemplaza.
        save(&dir, "Llavero 3 mm", &doc_with_notes("nueva")).unwrap();
        assert_eq!(load(&dir, "Llavero 3 mm").unwrap().notes, "nueva");
        assert_eq!(list(&dir).len(), 2);
        remove(&dir, "Llavero 3 mm").unwrap();
        remove(&dir, "Llavero 3 mm").unwrap();
        assert_eq!(list(&dir).len(), 1);
        assert!(
            load(&dir, "Llavero 3 mm")
                .unwrap_err()
                .contains("No existe")
        );
        assert!(!dir.join("acrílico.tlakutemplate.tmp").exists());
    }

    #[test]
    fn rechaza_nombres_peligrosos() {
        for bad in [
            "", "   ", "../x", "a/b", "a\\b", "con:dos", ".oculta", "fin.", "a\nb",
        ] {
            assert!(check_name(bad).is_err(), "{bad:?}");
        }
        assert!(check_name(&"x".repeat(MAX_NAME_CHARS + 1)).is_err());
        assert_eq!(check_name("  Cortes de 3 mm ").unwrap(), "Cortes de 3 mm");
        let tmp = TempDir::new();
        assert!(save(&tmp.0, "../fuera", &Document::default()).is_err());
        assert!(!tmp.0.join("../fuera.tlakutemplate").exists());
    }

    #[test]
    fn la_lista_omite_lo_que_no_es_plantilla() {
        let tmp = TempDir::new();
        save(&tmp.0, "buena", &Document::default()).unwrap();
        fs::write(tmp.0.join("nota.txt"), "x").unwrap();
        fs::write(tmp.0.join(".oculta.tlakutemplate"), "x").unwrap();
        fs::create_dir(tmp.0.join("carpeta.tlakutemplate")).unwrap();
        let names: Vec<_> = list(&tmp.0).into_iter().map(|t| t.name).collect();
        assert_eq!(names, ["buena"]);
    }

    #[test]
    fn una_plantilla_dañada_da_error_claro() {
        let tmp = TempDir::new();
        fs::write(file_of(&tmp.0, "rota"), "{ no es json").unwrap();
        let e = load(&tmp.0, "rota").unwrap_err();
        assert!(e.contains("Plantilla «rota»"), "{e}");
    }
}
