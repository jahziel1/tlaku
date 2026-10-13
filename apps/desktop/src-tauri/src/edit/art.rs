//! Biblioteca de arte (D47): los SVG propios del usuario viven en `<configuración>/art/` y
//! cualquier SVG (de la biblioteca incluida o del usuario) se importa desde su texto.

use super::files::{ImportDialogOptions, import_with_options};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use tauri::{AppHandle, Manager};
use tlaku_doc::ImportResult;

/// Tamaño máximo de un SVG de la biblioteca.
const MAX_SVG_BYTES: u64 = 2 * 1024 * 1024;

/// Un SVG de la biblioteca del usuario.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UserArt {
    /// Nombre del archivo sin la extensión; es su identificador.
    pub name: String,
    pub svg: String,
}

/// Lo que dejó agregar archivos: los que entraron y el motivo de los que no.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AddArtResult {
    pub added: Vec<UserArt>,
    pub errors: Vec<String>,
}

fn art_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|d| d.join("art"))
        .map_err(|e| format!("No se encontró la carpeta de configuración: {e}"))
}

fn is_svg(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
}

fn read_art(path: &Path) -> Result<UserArt, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.len() > MAX_SVG_BYTES {
        return Err(format!(
            "{}: el SVG pesa demasiado ({} KB; el máximo es {} KB).",
            path.display(),
            meta.len() / 1024,
            MAX_SVG_BYTES / 1024
        ));
    }
    let svg = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(UserArt { name, svg })
}

/// Los SVG de `dir`, por orden alfabético. Una carpeta que no existe es una biblioteca vacía; un
/// archivo ilegible se salta.
pub fn list_art(dir: &Path) -> Result<Vec<UserArt>, String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(Vec::new());
    };
    let mut out: Vec<UserArt> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_svg(p))
        .filter_map(|p| read_art(&p).ok())
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

/// Un nombre libre en `dir`: `base`, `base-2`, `base-3`...
fn unique_name(dir: &Path, base: &str) -> String {
    let taken = |n: &str| dir.join(format!("{n}.svg")).exists();
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|k| format!("{base}-{k}"))
        .find(|n| !taken(n))
        .expect("siempre hay un número libre")
}

/// Copia los SVG de `paths` a `dir`. Solo entran los que se pueden leer como dibujo.
pub fn add_art(dir: &Path, paths: &[String]) -> Result<AddArtResult, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut result = AddArtResult {
        added: Vec::new(),
        errors: Vec::new(),
    };
    for p in paths {
        let src = Path::new(p);
        let shown = src
            .file_name()
            .map_or_else(|| p.clone(), |n| n.to_string_lossy().into_owned());
        if !is_svg(src) {
            result
                .errors
                .push(format!("{shown}: no es un archivo SVG."));
            continue;
        }
        let art = match read_art(src) {
            Ok(a) => a,
            Err(e) => {
                result.errors.push(e);
                continue;
            }
        };
        // Debe leerse como dibujo: se prueba con el mismo importador que lo insertará.
        if let Err(e) = import_svg_text(&art.svg, None) {
            result.errors.push(format!("{shown}: {e}"));
            continue;
        }
        let name = unique_name(dir, &art.name);
        let dst = dir.join(format!("{name}.svg"));
        if let Err(e) = std::fs::write(&dst, &art.svg) {
            result.errors.push(format!("{}: {e}", dst.display()));
            continue;
        }
        result.added.push(UserArt { name, svg: art.svg });
    }
    Ok(result)
}

/// Borra un SVG de la biblioteca del usuario. `name` es solo el nombre, nunca una ruta.
pub fn remove_art(dir: &Path, name: &str) -> Result<(), String> {
    if name.is_empty() || name.contains(['/', '\\']) || name.contains("..") {
        return Err(format!("Nombre de dibujo inválido: «{name}»."));
    }
    let path = dir.join(format!("{name}.svg"));
    std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Importa el texto de un SVG con el importador de siempre (curvas Bézier, milímetros). Con
/// `target_layer` (`#rrggbb`) todo cae en esa capa.
pub fn import_svg_text(svg: &str, target_layer: Option<&str>) -> Result<ImportResult, String> {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("tlaku-arte-{}-{n}.svg", std::process::id()));
    std::fs::write(&path, svg).map_err(|e| format!("{}: {e}", path.display()))?;
    let options = ImportDialogOptions {
        target_layer: target_layer.map(str::to_string),
        ..ImportDialogOptions::default()
    };
    let result = import_with_options(&path.to_string_lossy(), &options);
    let _ = std::fs::remove_file(&path);
    let result = result?;
    if result.objects.is_empty() {
        return Err("El SVG no tiene trayectos que insertar.".into());
    }
    Ok(result)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn art_import_svg(
    svg: String,
    target_layer: Option<String>,
) -> Result<ImportResult, String> {
    tauri::async_runtime::spawn_blocking(move || import_svg_text(&svg, target_layer.as_deref()))
        .await
        .map_err(|e| format!("La tarea se interrumpió: {e}"))?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn art_list_user(app: AppHandle) -> Result<Vec<UserArt>, String> {
    let dir = art_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || list_art(&dir))
        .await
        .map_err(|e| format!("La tarea se interrumpió: {e}"))?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn art_add_user(app: AppHandle, paths: Vec<String>) -> Result<AddArtResult, String> {
    let dir = art_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || add_art(&dir, &paths))
        .await
        .map_err(|e| format!("La tarea se interrumpió: {e}"))?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn art_remove_user(app: AppHandle, name: String) -> Result<(), String> {
    let dir = art_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || remove_art(&dir, &name))
        .await
        .map_err(|e| format!("La tarea se interrumpió: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20mm" height="20mm" viewBox="0 0 20 20"><rect x="1" y="1" width="18" height="18" fill="none" stroke="#000000" stroke-width="0.2"/></svg>"##;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tlaku-arte-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn importa_el_texto_de_un_svg_en_milimetros() {
        let r = import_svg_text(SQUARE, None).unwrap();
        assert!(!r.objects.is_empty());
        let b = r.objects[0].bounds().unwrap();
        assert!((b.max.x - b.min.x - 18.0).abs() < 0.5, "{b:?}");
        let r = import_svg_text(SQUARE, Some("#00ff00")).unwrap();
        assert!(r.objects.iter().all(|o| o.layer.to_string() == "#00ff00"));
        assert!(import_svg_text("<svg", None).is_err());
        let vacio = r#"<svg xmlns="http://www.w3.org/2000/svg" width="5mm" height="5mm"/>"#;
        assert!(import_svg_text(vacio, None).is_err());
    }

    #[test]
    fn agrega_lista_y_quita_dibujos_del_usuario() {
        let work = scratch("biblioteca");
        let lib = work.join("art");
        // Sin carpeta: biblioteca vacía.
        assert!(list_art(&lib).unwrap().is_empty());
        let src = work.join("cuadro.svg");
        std::fs::write(&src, SQUARE).unwrap();
        let txt = work.join("notas.txt");
        std::fs::write(&txt, "hola").unwrap();
        let roto = work.join("roto.svg");
        std::fs::write(&roto, "esto no es un svg").unwrap();
        let paths = [src, txt, roto].map(|p| p.to_string_lossy().into_owned());
        let r = add_art(&lib, &paths).unwrap();
        assert_eq!(r.added.len(), 1);
        assert_eq!(r.errors.len(), 2, "{:?}", r.errors);
        assert_eq!(r.added[0].name, "cuadro");
        // El mismo archivo otra vez no pisa al primero.
        let r = add_art(&lib, &paths[..1]).unwrap();
        assert_eq!(r.added[0].name, "cuadro-2");
        let names: Vec<_> = list_art(&lib)
            .unwrap()
            .into_iter()
            .map(|a| a.name)
            .collect();
        assert_eq!(names, ["cuadro", "cuadro-2"]);
        assert!(remove_art(&lib, "../fuera").is_err());
        remove_art(&lib, "cuadro").unwrap();
        assert_eq!(list_art(&lib).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(work);
    }

    #[test]
    fn toda_la_biblioteca_incluida_se_importa_con_su_tamano_en_mm() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/features/art/svg");
        let mut n = 0;
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if !is_svg(&path) {
                continue;
            }
            let art = read_art(&path).unwrap();
            let r = import_svg_text(&art.svg, None)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let b = r
                .objects
                .iter()
                .filter_map(|o| o.bounds())
                .reduce(tlaku_doc::union)
                .unwrap_or_else(|| panic!("{}: sin geometría", path.display()));
            let (w, h) = (b.max.x - b.min.x, b.max.y - b.min.y);
            assert!(
                w.max(h) > 30.0 && w.max(h) < 120.0,
                "{}: tamaño raro {w} x {h} mm",
                path.display()
            );
            n += 1;
        }
        assert!(
            n >= 30,
            "la biblioteca incluida debe tener unos 30-50 dibujos: {n}"
        );
    }
}
