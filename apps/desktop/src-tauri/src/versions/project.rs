//! Historial de un proyecto: dónde vive, cómo se captura, lista, lee y borra.
//!
//! Todo recibe la carpeta raíz del almacén de la app (`root`), la configuración y la hora, así que
//! se prueba sin Tauri. Los comandos de `commands.rs` solo resuelven esas carpetas.
//!
//! **Identidad del proyecto.** El `.tlaku` no tiene un id propio: se identifica por su ruta absoluta
//! (`std::path::absolute`, sin resolver enlaces). Mover o renombrar el archivo empieza un historial
//! nuevo; el anterior sigue en la lista «Otros proyectos con historial» y se puede abrir desde ahí.
//!
//! **Dos lugares.** Se escribe donde diga `Location` (por omisión la carpeta de datos de la app), pero
//! al listar, leer y borrar se miran los dos: cambiar la preferencia no deja el historial viejo
//! escondido.

use super::config::{Location, VersionsConfig};
use super::store::{Kind, NewVersion, Store, VersionInfo, content_hash};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use tlaku_doc::Document;
use tlaku_export::{ImageHandling, SvgOptions, export_svg};
use tlaku_i18n::tr;

/// Carpeta oculta junto al archivo cuando `Location::Beside`.
pub const BESIDE_DIR: &str = ".tlaku-versions";
const PROJECT_FILE: &str = "project.json";
/// Una miniatura mayor que esto no se guarda (el diseño es demasiado denso para verse en chico).
const MAX_THUMB_BYTES: usize = 250 * 1024;

/// A qué proyecto pertenece una carpeta de versiones (para la lista de «otros proyectos»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub path: String,
    pub name: String,
}

/// Lo que devuelve «listar»: las versiones y el contexto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Listing {
    /// De la más nueva a la más vieja.
    pub versions: Vec<VersionInfo>,
    /// Lo que ocupa el historial de este proyecto.
    pub total_bytes: u64,
    /// Carpeta donde se escribirían las próximas versiones.
    pub dir: String,
}

/// Un proyecto con historial en la carpeta de datos de la app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectSummary {
    pub path: String,
    pub name: String,
    pub versions: u32,
    pub total_bytes: u64,
    pub latest_ms: u64,
    /// `false` si el archivo del proyecto ya no existe (borrado o movido).
    pub exists: bool,
}

fn abs(path: &str) -> Result<PathBuf, String> {
    std::path::absolute(path).map_err(|e| format!("{path}: {e}"))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Nombre de carpeta seguro: letras, dígitos, `-`, `_` y `.`; lo demás pasa a `_`.
fn safe_name(name: &str) -> String {
    let s: String = name
        .chars()
        .take(40)
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() { "proyecto".into() } else { s }
}

/// Carpeta del historial de `path` en la carpeta de datos de la app.
pub fn app_dir(root: &Path, path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let hash = content_hash(path.to_string_lossy().as_bytes());
    root.join(format!("{}-{hash}", safe_name(&stem)))
}

/// Carpeta del historial de `path` junto al archivo.
pub fn beside_dir(path: &Path) -> PathBuf {
    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    parent.join(BESIDE_DIR).join(safe_name(&file_name(path)))
}

/// Las carpetas de `path`: primero donde se escribe, luego la otra (solo si existe).
fn stores(root: &Path, cfg: &VersionsConfig, path: &Path) -> Vec<Store> {
    let (primary, other) = match cfg.location {
        Location::App => (app_dir(root, path), beside_dir(path)),
        Location::Beside => (beside_dir(path), app_dir(root, path)),
    };
    let mut out = vec![Store::new(primary)];
    if other.is_dir() {
        out.push(Store::new(other));
    }
    out
}

fn primary(root: &Path, cfg: &VersionsConfig, path: &Path) -> Store {
    stores(root, cfg, path).swap_remove(0)
}

fn find_store(root: &Path, cfg: &VersionsConfig, path: &Path, id: &str) -> Result<Store, String> {
    stores(root, cfg, path)
        .into_iter()
        .find(|s| s.has(id))
        .ok_or_else(|| tr!("desktop.versions.not_found", id = id))
}

/// El texto JSON que iría al `.tlaku` de `path` (rutas de datos relativas, como al guardar).
pub fn serialize_for(path: &Path, doc: &Document) -> Result<String, String> {
    let dir = path.parent();
    match dir {
        Some(dir)
            if !doc.variable_text.sources.is_empty() || doc.variable_text.csv_path.is_some() =>
        {
            let mut doc = doc.clone();
            doc.relativize_data_paths(dir);
            doc.to_json()
        }
        _ => doc.to_json(),
    }
}

/// Miniatura SVG del diseño (sin imágenes), o `None` si está vacío o es demasiado denso.
pub fn thumbnail(doc: &Document) -> Option<String> {
    let mut opts = SvgOptions {
        images: ImageHandling::Skip,
        stroke_mm: 0.1,
        margin_mm: 0.0,
        keep_position: false,
        fill_layers: false,
    };
    let first = export_svg(doc, None, &opts).ok()?;
    let side = first.size_mm[0].max(first.size_mm[1]);
    if !side.is_finite() || side <= 0.0 {
        return None;
    }
    // El trazo se escala al tamaño del diseño para que se vea igual en una pieza de 20 mm y en una de 2 m.
    opts.stroke_mm = (side / 120.0).max(0.1);
    opts.margin_mm = side / 40.0;
    let svg = export_svg(doc, None, &opts).ok()?;
    let text = String::from_utf8(svg.bytes).ok()?;
    (text.len() <= MAX_THUMB_BYTES).then_some(text)
}

/// Resumen y miniatura de un texto de proyecto. Un texto que no se entiende se guarda igual (es
/// lo que había en el archivo), solo sin resumen.
fn describe(text: &str) -> (u32, u32, Option<String>) {
    match Document::from_json(text) {
        Ok(doc) => (
            u32::try_from(doc.objects.len()).unwrap_or(u32::MAX),
            u32::try_from(doc.layers.len()).unwrap_or(u32::MAX),
            thumbnail(&doc),
        ),
        Err(_) => (0, 0, None),
    }
}

fn remember_project(store: &Store, path: &Path) {
    let meta = ProjectMeta {
        path: path.to_string_lossy().into_owned(),
        name: file_name(path),
    };
    if let Ok(bytes) = serde_json::to_vec(&meta) {
        let _ = super::store::write_atomic_bytes(&store.dir.join(PROJECT_FILE), &bytes);
    }
}

fn add(
    root: &Path,
    cfg: &VersionsConfig,
    path: &Path,
    new: NewVersion<'_>,
    now_ms: u64,
) -> Result<Option<VersionInfo>, String> {
    let store = primary(root, cfg, path);
    let added = store.add(new)?;
    if added.is_some() {
        remember_project(&store, path);
    }
    // La poda es de cortesía: si falla, la versión ya está guardada.
    store.prune(now_ms, cfg.policy());
    Ok(added)
}

/// Antes de un guardado: conserva el contenido que tiene ahora el archivo `path` como versión
/// `Saved` (con la fecha del archivo). Sin archivo, o con el historial apagado, no hace nada.
pub fn capture_file(
    root: &Path,
    cfg: &VersionsConfig,
    path: &str,
    now_ms: u64,
) -> Result<Option<VersionInfo>, String> {
    if !cfg.enabled {
        return Ok(None);
    }
    let path = abs(path)?;
    let meta = match fs::metadata(&path) {
        Ok(m) if m.is_file() => m,
        _ => return Ok(None),
    };
    if meta.len() > super::store::MAX_VERSION_BYTES {
        return Err(tr!(
            "desktop.versions.too_big",
            size = meta.len() / (1024 * 1024),
            max = super::store::MAX_VERSION_BYTES / (1024 * 1024)
        ));
    }
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let created = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(now_ms, |d| d.as_millis() as u64);
    let (objects, layers, thumb) = describe(&text);
    add(
        root,
        cfg,
        &path,
        NewVersion {
            kind: Kind::Saved,
            label: None,
            created_ms: created,
            text: &text,
            objects,
            layers,
            thumb,
        },
        now_ms,
    )
}

/// Guarda el documento tal como está en memoria (`Auto`, `Manual` o `Restore`). Las automáticas se
/// ignoran con el historial apagado; las manuales y las de restauración, no.
pub fn snapshot(
    root: &Path,
    cfg: &VersionsConfig,
    path: &str,
    doc: &Document,
    kind: Kind,
    label: Option<String>,
    now_ms: u64,
) -> Result<Option<VersionInfo>, String> {
    if kind == Kind::Saved {
        return Err(tr!("desktop.versions.bad_kind"));
    }
    if kind == Kind::Auto && (!cfg.enabled || cfg.auto_minutes == 0) {
        return Ok(None);
    }
    let path = abs(path)?;
    let text = serialize_for(&path, doc)?;
    let label = label
        .map(|l| l.trim().chars().take(120).collect::<String>())
        .filter(|l| !l.is_empty());
    if kind == Kind::Manual && label.is_none() {
        return Err(tr!("desktop.versions.need_label"));
    }
    add(
        root,
        cfg,
        &path,
        NewVersion {
            kind,
            label,
            created_ms: now_ms,
            text: &text,
            objects: u32::try_from(doc.objects.len()).unwrap_or(u32::MAX),
            layers: u32::try_from(doc.layers.len()).unwrap_or(u32::MAX),
            thumb: thumbnail(doc),
        },
        now_ms,
    )
}

/// Las versiones de `path` en los dos lugares, de la más nueva a la más vieja.
pub fn list(root: &Path, cfg: &VersionsConfig, path: &str) -> Result<Listing, String> {
    let path = abs(path)?;
    let stores = stores(root, cfg, &path);
    let mut versions: Vec<VersionInfo> = Vec::new();
    for s in &stores {
        for v in s.list() {
            if !versions.iter().any(|x| x.id == v.id) {
                versions.push(v);
            }
        }
    }
    versions.sort_by(|a, b| b.created_ms.cmp(&a.created_ms).then(b.id.cmp(&a.id)));
    Ok(Listing {
        total_bytes: versions.iter().map(|v| v.stored_bytes).sum(),
        dir: stores[0].dir.to_string_lossy().into_owned(),
        versions,
    })
}

/// El documento de la versión `id`, con las rutas de datos absolutas respecto a la carpeta de `path`.
pub fn read_doc(
    root: &Path,
    cfg: &VersionsConfig,
    path: &str,
    id: &str,
) -> Result<Document, String> {
    let path = abs(path)?;
    let text = find_store(root, cfg, &path, id)?.read_text(id)?;
    let mut doc = Document::from_json(&text).map_err(|e| format!("{id}: {e}"))?;
    if let Some(dir) = path.parent() {
        doc.absolutize_data_paths(dir);
    }
    Ok(doc)
}

pub fn read_thumb(
    root: &Path,
    cfg: &VersionsConfig,
    path: &str,
    id: &str,
) -> Result<Option<String>, String> {
    let path = abs(path)?;
    find_store(root, cfg, &path, id)?.read_thumb(id)
}

/// Borra las versiones `ids`. Devuelve cuántas.
pub fn delete(
    root: &Path,
    cfg: &VersionsConfig,
    path: &str,
    ids: &[String],
) -> Result<u32, String> {
    let path = abs(path)?;
    let mut n = 0;
    for s in stores(root, cfg, &path) {
        n += s.delete(ids)?;
    }
    Ok(n)
}

/// Borra todo el historial de `path`. Devuelve cuántas versiones había.
pub fn clear(root: &Path, cfg: &VersionsConfig, path: &str) -> Result<u32, String> {
    let path = abs(path)?;
    let mut n = 0;
    for s in stores(root, cfg, &path) {
        n += s.clear()?;
        let _ = fs::remove_file(s.dir.join(PROJECT_FILE));
        // Las carpetas vacías se quitan; si no lo están (algo ajeno), se dejan.
        let _ = fs::remove_dir(&s.dir);
    }
    if let Some(parent) = beside_dir(&path).parent() {
        let _ = fs::remove_dir(parent);
    }
    Ok(n)
}

/// Proyectos con historial en la carpeta de datos de la app, el más reciente primero.
pub fn projects(root: &Path) -> Vec<ProjectSummary> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let dir = e.path();
        let Some(meta) = fs::read_to_string(dir.join(PROJECT_FILE))
            .ok()
            .and_then(|t| serde_json::from_str::<ProjectMeta>(&t).ok())
        else {
            continue;
        };
        let versions = Store::new(dir).list();
        let Some(latest) = versions.first() else {
            continue;
        };
        out.push(ProjectSummary {
            exists: Path::new(&meta.path).is_file(),
            path: meta.path,
            name: meta.name,
            versions: u32::try_from(versions.len()).unwrap_or(u32::MAX),
            total_bytes: versions.iter().map(|v| v.stored_bytes).sum(),
            latest_ms: latest.created_ms,
        });
    }
    out.sort_by(|a, b| b.latest_ms.cmp(&a.latest_ms).then(a.path.cmp(&b.path)));
    out
}
