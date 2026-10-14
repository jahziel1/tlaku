//! Autoguardado y recuperación.
//!
//! La interfaz manda cada tanto una copia del documento con cambios; aquí se guarda en
//! `<datos de la app>/recuperacion/`. Cada copia son dos archivos con el mismo id:
//!
//! - `<id>.tlaku`: el documento, igual que un proyecto.
//! - `<id>.meta.json`: nombre, ruta de origen y hora de la copia, para listar sin leer el documento.
//!
//! Siempre se escribe el documento primero y el meta después, y los dos de forma atómica: una copia
//! sin meta no existe para la lista (y se limpia sola), así un corte a media escritura nunca deja una
//! copia a medias. El número y el tamaño están acotados (`Limits`).
//!
//! **Copias vivas.** Con varias ventanas (`new_window`) todas leen la misma carpeta. Cada copia se
//! marca como viva de la ventana que la escribe (`LiveCopies`, en memoria: no sobrevive a un cierre
//! inesperado, que es justo lo que se quiere) y la lista de pendientes deja fuera las copias de
//! *otras* ventanas que siguen abiertas: al arrancar, o al abrir una ventana nueva, solo se ofrecen
//! las huérfanas (de una sesión anterior o de una ventana ya cerrada). Las copias de la propia
//! ventana que pregunta cuentan como huérfanas: pregunta al montarse, así que son de una carga
//! anterior de la página. Límite: el registro es de un solo proceso; dos instancias de la app a
//! la vez no se ven entre sí.

use crate::engine::write_atomic;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, WebviewWindow, WindowEvent};
use tlaku_doc::Document;
use tlaku_i18n::tr;

/// Carpeta de las copias dentro de la carpeta de datos de la app.
const DIR_NAME: &str = "recuperacion";
const DOC_EXT: &str = "tlaku";
const META_SUFFIX: &str = ".meta.json";

/// Cuántas copias se conservan, cuánto puede pesar una y cuánto pesan todas juntas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_copies: usize,
    pub max_copy_bytes: u64,
    pub max_total_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_copies: 10,
            max_copy_bytes: 128 * 1024 * 1024,
            max_total_bytes: 512 * 1024 * 1024,
        }
    }
}

/// Lo que se sabe de una copia sin abrir el documento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryInfo {
    pub id: String,
    /// Nombre para mostrar (el del proyecto, o "Sin título").
    pub name: String,
    /// Ruta del proyecto del que viene la copia, si ya tenía archivo.
    pub path: Option<String>,
    /// Momento de la copia, en milisegundos desde 1970.
    pub saved_at_ms: u64,
    /// Peso del documento guardado.
    pub size_bytes: u64,
}

/// Qué ventana es dueña de cada copia que sigue viva.
#[derive(Debug, Default)]
pub struct LiveCopies {
    /// id de copia -> etiqueta de la ventana que la escribe.
    owners: HashMap<String, String>,
    /// Ventanas que ya tienen enganchado el aviso de cierre.
    watched: HashSet<String>,
}

impl LiveCopies {
    /// Marca `id` como viva de `window`. Devuelve `true` si es la primera copia de esa ventana
    /// (hay que engancharle el aviso de cierre).
    pub fn mark(&mut self, window: &str, id: &str) -> bool {
        self.owners.insert(id.to_string(), window.to_string());
        self.watched.insert(window.to_string())
    }

    /// La copia ya no es de nadie (se borró).
    pub fn release_id(&mut self, id: &str) {
        self.owners.remove(id);
    }

    /// La ventana se cerró: sus copias que queden pasan a ser huérfanas.
    pub fn release_window(&mut self, window: &str) {
        self.owners.retain(|_, w| w != window);
        self.watched.remove(window);
    }

    /// Copias vivas de ventanas distintas de `asking`: no se ofrecen para recuperar.
    pub fn held_by_others(&self, asking: &str) -> HashSet<String> {
        self.owners
            .iter()
            .filter(|(_, w)| w.as_str() != asking)
            .map(|(id, _)| id.clone())
            .collect()
    }
}

static LIVE: Mutex<Option<LiveCopies>> = Mutex::new(None);

fn with_live<T>(f: impl FnOnce(&mut LiveCopies) -> T) -> T {
    let mut guard = LIVE.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(LiveCopies::default))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn to_ms(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// El id se usa como nombre de archivo: solo letras, dígitos, guion y guion bajo.
fn check_id(id: &str) -> Result<(), String> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(tr!("desktop.autosave.bad_id", id = id))
    }
}

fn doc_file(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.{DOC_EXT}"))
}

fn meta_file(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}{META_SUFFIX}"))
}

fn io_error(path: &Path, e: &std::io::Error) -> String {
    format!("{}: {e}", path.display())
}

/// Guarda una copia (reemplaza la anterior con el mismo id) y recorta las más viejas hasta cumplir
/// los límites. `json` es el documento ya serializado.
pub fn write_copy(
    dir: &Path,
    id: &str,
    name: &str,
    path: Option<&str>,
    json: &str,
    limits: Limits,
) -> Result<RecoveryInfo, String> {
    check_id(id)?;
    let size = json.len() as u64;
    if size > limits.max_copy_bytes {
        return Err(tr!(
            "desktop.autosave.too_big",
            size = size / (1024 * 1024),
            max = limits.max_copy_bytes / (1024 * 1024)
        ));
    }
    fs::create_dir_all(dir).map_err(|e| io_error(dir, &e))?;
    let info = RecoveryInfo {
        id: id.to_string(),
        name: name.to_string(),
        path: path.map(str::to_string),
        saved_at_ms: now_ms(),
        size_bytes: size,
    };
    write_atomic(&doc_file(dir, id), json)?;
    let meta =
        serde_json::to_string(&info).map_err(|e| tr!("desktop.autosave.meta_failed", err = e))?;
    write_atomic(&meta_file(dir, id), &meta)?;
    prune(dir, limits, Some(id));
    Ok(info)
}

/// Todas las copias que tienen meta legible, sin filtrar. De la más nueva a la más vieja.
fn all_copies(dir: &Path) -> Vec<RecoveryInfo> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<RecoveryInfo> = entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(META_SUFFIX))
        .filter_map(|e| fs::read_to_string(e.path()).ok())
        .filter_map(|text| serde_json::from_str::<RecoveryInfo>(&text).ok())
        .filter(|info| check_id(&info.id).is_ok() && doc_file(dir, &info.id).is_file())
        .collect();
    out.sort_by(|a, b| b.saved_at_ms.cmp(&a.saved_at_ms).then(a.id.cmp(&b.id)));
    out
}

/// `true` si el usuario ya guardó después de la copia: el proyecto de origen tiene fecha igual o
/// posterior. Una copia de un documento sin ruta, o cuyo proyecto ya no existe, no está superada.
fn superseded(info: &RecoveryInfo) -> bool {
    let Some(path) = &info.path else {
        return false;
    };
    fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|t| to_ms(t) >= info.saved_at_ms)
        .unwrap_or(false)
}

/// Copias pendientes de recuperar. Las superadas por un guardado posterior se borran.
pub fn list_pending(dir: &Path) -> Vec<RecoveryInfo> {
    let mut pending = Vec::new();
    for info in all_copies(dir) {
        if superseded(&info) {
            let _ = remove_copy(dir, &info.id);
        } else {
            pending.push(info);
        }
    }
    pending
}

/// Como [`list_pending`] pero sin las copias de `live` (las de otras ventanas abiertas).
pub fn list_orphans(dir: &Path, live: &HashSet<String>) -> Vec<RecoveryInfo> {
    let mut pending = list_pending(dir);
    pending.retain(|i| !live.contains(&i.id));
    pending
}

pub fn read_copy(dir: &Path, id: &str) -> Result<Document, String> {
    check_id(id)?;
    let file = doc_file(dir, id);
    let text = fs::read_to_string(&file).map_err(|e| io_error(&file, &e))?;
    Document::from_json(&text).map_err(|e| format!("{}: {e}", file.display()))
}

/// Borra la copia `id`. No es error que no exista.
pub fn remove_copy(dir: &Path, id: &str) -> Result<(), String> {
    check_id(id)?;
    // El meta primero: sin meta la copia deja de existir para la lista aunque falle el resto.
    for file in [meta_file(dir, id), doc_file(dir, id)] {
        match fs::remove_file(&file) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(io_error(&file, &e)),
        }
    }
    Ok(())
}

/// Quita copias de más (las más viejas primero) y archivos huérfanos o temporales. `keep` es la
/// copia que se acaba de escribir y nunca se borra.
fn prune(dir: &Path, limits: Limits, keep: Option<&str>) {
    let copies = all_copies(dir);
    let mut total: u64 = copies.iter().map(|c| c.size_bytes).sum();
    let mut count = copies.len();
    // `copies` va de la más nueva a la más vieja: se recorre al revés.
    for info in copies.iter().rev() {
        if count <= limits.max_copies && total <= limits.max_total_bytes {
            break;
        }
        if Some(info.id.as_str()) == keep {
            continue;
        }
        if remove_copy(dir, &info.id).is_ok() {
            count -= 1;
            total = total.saturating_sub(info.size_bytes);
        }
    }
    remove_orphans(dir);
}

/// Documentos sin meta y restos `.tmp` de una escritura interrumpida.
fn remove_orphans(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let orphan_doc = name
            .strip_suffix(&format!(".{DOC_EXT}"))
            .is_some_and(|id| !meta_file(dir, id).is_file());
        if name.ends_with(".tmp") || orphan_doc {
            let _ = fs::remove_file(e.path());
        }
    }
}

// ---------------------------------------------------------------------------
// Comandos de Tauri
// ---------------------------------------------------------------------------

fn recovery_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|d| d.join(DIR_NAME))
        .map_err(|e| tr!("desktop.autosave.data_dir", err = e))
}

/// Un solo acceso a la carpeta a la vez: la poda de una escritura no debe llevarse el documento
/// recién escrito de otra que aún no ha puesto su meta.
static LOCK: Mutex<()> = Mutex::new(());

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
pub async fn autosave_write(
    window: WebviewWindow,
    app: AppHandle,
    id: String,
    name: String,
    path: Option<String>,
    doc: Document,
) -> Result<RecoveryInfo, String> {
    let dir = recovery_dir(&app)?;
    let label = window.label().to_string();
    blocking(move || {
        let json = doc.to_json()?;
        // La copia se marca viva antes de escribirla: ninguna lista puede verla sin dueño.
        if with_live(|live| live.mark(&label, &id)) {
            let closing = label.clone();
            window.on_window_event(move |event| {
                if matches!(event, WindowEvent::Destroyed) {
                    with_live(|live| live.release_window(&closing));
                }
            });
        }
        write_copy(&dir, &id, &name, path.as_deref(), &json, Limits::default())
    })
    .await
}

#[tauri::command]
pub async fn autosave_list(
    window: WebviewWindow,
    app: AppHandle,
) -> Result<Vec<RecoveryInfo>, String> {
    let dir = recovery_dir(&app)?;
    let label = window.label().to_string();
    blocking(move || {
        let live = with_live(|live| live.held_by_others(&label));
        Ok(list_orphans(&dir, &live))
    })
    .await
}

#[tauri::command]
pub async fn autosave_read(app: AppHandle, id: String) -> Result<Document, String> {
    let dir = recovery_dir(&app)?;
    blocking(move || read_copy(&dir, &id)).await
}

#[tauri::command]
pub async fn autosave_discard(app: AppHandle, id: String) -> Result<(), String> {
    let dir = recovery_dir(&app)?;
    blocking(move || {
        with_live(|live| live.release_id(&id));
        remove_copy(&dir, &id)
    })
    .await
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
                std::env::temp_dir().join(format!("tlaku-autosave-{}-{n}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            Self(dir)
        }
        fn recovery(&self) -> PathBuf {
            self.0.join(DIR_NAME)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn json() -> String {
        Document::default().to_json().unwrap()
    }

    fn put(dir: &Path, id: &str, path: Option<&str>) -> RecoveryInfo {
        write_copy(dir, id, "Mesa", path, &json(), Limits::default()).unwrap()
    }

    #[test]
    fn guarda_lista_lee_y_borra() {
        let tmp = TempDir::new();
        let dir = tmp.recovery();
        assert!(list_pending(&dir).is_empty());
        let info = put(&dir, "a1", None);
        assert_eq!(info.name, "Mesa");
        assert!(info.size_bytes > 0);
        assert_eq!(list_pending(&dir), vec![info]);
        assert_eq!(read_copy(&dir, "a1").unwrap(), Document::default());
        remove_copy(&dir, "a1").unwrap();
        assert!(list_pending(&dir).is_empty());
        // Borrar lo que no existe no es un error.
        remove_copy(&dir, "a1").unwrap();
    }

    #[test]
    fn la_escritura_es_atomica_y_no_deja_temporales() {
        let tmp = TempDir::new();
        let dir = tmp.recovery();
        put(&dir, "a1", None);
        put(&dir, "a1", None); // reemplaza
        let names: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 2, "{names:?}");
        assert!(names.iter().all(|n| !n.ends_with(".tmp")));
        assert_eq!(list_pending(&dir).len(), 1);
    }

    #[test]
    fn rechaza_ids_peligrosos() {
        let tmp = TempDir::new();
        let dir = tmp.recovery();
        for bad in ["", "../x", "a/b", "a.b", &"x".repeat(65)] {
            assert!(
                write_copy(&dir, bad, "n", None, &json(), Limits::default()).is_err(),
                "{bad}"
            );
            assert!(read_copy(&dir, bad).is_err());
            assert!(remove_copy(&dir, bad).is_err());
        }
    }

    #[test]
    fn rechaza_un_documento_demasiado_grande() {
        let tmp = TempDir::new();
        let dir = tmp.recovery();
        let limits = Limits {
            max_copy_bytes: 10,
            ..Limits::default()
        };
        let err = write_copy(&dir, "a1", "n", None, &json(), limits).unwrap_err();
        assert!(err.contains("supera el máximo"), "{err}");
        assert!(list_pending(&dir).is_empty());
    }

    #[test]
    fn limita_el_numero_de_copias_y_conserva_las_mas_nuevas() {
        let tmp = TempDir::new();
        let dir = tmp.recovery();
        let limits = Limits {
            max_copies: 3,
            ..Limits::default()
        };
        for i in 0..6 {
            write_copy(&dir, &format!("c{i}"), "n", None, &json(), limits).unwrap();
            // Fechas distintas aunque el reloj sea de baja resolución.
            let meta = meta_file(&dir, &format!("c{i}"));
            let mut info: RecoveryInfo =
                serde_json::from_str(&fs::read_to_string(&meta).unwrap()).unwrap();
            info.saved_at_ms = 1_000 + i as u64;
            fs::write(&meta, serde_json::to_string(&info).unwrap()).unwrap();
        }
        let ids: Vec<String> = list_pending(&dir).into_iter().map(|c| c.id).collect();
        assert_eq!(ids.len(), 3, "{ids:?}");
        // La última escrita siempre se conserva; las viejas se van primero.
        assert!(ids.contains(&"c5".to_string()), "{ids:?}");
        assert!(!ids.contains(&"c0".to_string()), "{ids:?}");
    }

    #[test]
    fn limita_el_peso_total() {
        let tmp = TempDir::new();
        let dir = tmp.recovery();
        let one = json().len() as u64;
        let limits = Limits {
            max_total_bytes: one * 2,
            ..Limits::default()
        };
        for i in 0..4 {
            write_copy(&dir, &format!("p{i}"), "n", None, &json(), limits).unwrap();
        }
        let left = list_pending(&dir);
        assert!(left.len() <= 2, "{left:?}");
        assert!(left.iter().any(|c| c.id == "p3"));
    }

    #[test]
    fn una_copia_mas_vieja_que_el_guardado_del_usuario_se_descarta() {
        let tmp = TempDir::new();
        let dir = tmp.recovery();
        fs::create_dir_all(&tmp.0).unwrap();
        let project = tmp.0.join("mesa.tlaku");
        fs::write(&project, json()).unwrap();
        let path = project.to_string_lossy().into_owned();

        // Copia hecha ANTES del guardado: ya no sirve.
        let mut old = put(&dir, "vieja", Some(&path));
        old.saved_at_ms = 1;
        fs::write(
            meta_file(&dir, "vieja"),
            serde_json::to_string(&old).unwrap(),
        )
        .unwrap();
        // Copia hecha DESPUÉS del guardado: se ofrece.
        let mut newer = put(&dir, "nueva", Some(&path));
        newer.saved_at_ms = now_ms() + 60_000;
        fs::write(
            meta_file(&dir, "nueva"),
            serde_json::to_string(&newer).unwrap(),
        )
        .unwrap();
        // Sin ruta (documento nunca guardado): se ofrece siempre.
        put(&dir, "sinruta", None);

        let ids: Vec<String> = list_pending(&dir).into_iter().map(|c| c.id).collect();
        assert!(ids.contains(&"nueva".to_string()));
        assert!(ids.contains(&"sinruta".to_string()));
        assert!(!ids.contains(&"vieja".to_string()));
        // La superada se borró del disco, no solo de la lista.
        assert!(!doc_file(&dir, "vieja").exists());
    }

    #[test]
    fn si_el_proyecto_de_origen_ya_no_existe_la_copia_se_ofrece() {
        let tmp = TempDir::new();
        let dir = tmp.recovery();
        put(&dir, "huerfana", Some("/no/existe/mesa.tlaku"));
        assert_eq!(list_pending(&dir).len(), 1);
    }

    #[test]
    fn ignora_metas_rotos_y_limpia_huerfanos() {
        let tmp = TempDir::new();
        let dir = tmp.recovery();
        put(&dir, "ok", None);
        fs::write(meta_file(&dir, "roto"), "{ no es json").unwrap();
        fs::write(doc_file(&dir, "sinmeta"), json()).unwrap();
        fs::write(dir.join("resto.tlaku.tmp"), "x").unwrap();
        assert_eq!(list_pending(&dir).len(), 1);
        // La poda de la siguiente escritura limpia los huérfanos.
        put(&dir, "ok2", None);
        assert!(!doc_file(&dir, "sinmeta").exists());
        assert!(!dir.join("resto.tlaku.tmp").exists());
    }

    #[test]
    fn el_registro_sigue_a_cada_ventana() {
        let mut live = LiveCopies::default();
        // La primera copia de una ventana pide enganchar el aviso de cierre; las demás, no.
        assert!(live.mark("main", "a1"));
        assert!(!live.mark("main", "a2"));
        assert!(live.mark("doc-1", "b1"));
        assert_eq!(
            live.held_by_others("main"),
            HashSet::from(["b1".to_string()])
        );
        assert_eq!(
            live.held_by_others("doc-1"),
            HashSet::from(["a1".to_string(), "a2".to_string()])
        );
        // Una ventana desconocida (por ejemplo una recién abierta) ve todas como ajenas.
        assert_eq!(live.held_by_others("doc-2").len(), 3);
        live.release_id("a1");
        assert!(!live.held_by_others("doc-1").contains("a1"));
        // Al cerrarse, la ventana suelta sus copias y puede volver a engancharse.
        live.release_window("doc-1");
        assert_eq!(live.held_by_others("main").len(), 0);
        assert!(live.mark("doc-1", "b2"));
    }

    #[test]
    fn solo_se_ofrecen_las_copias_huerfanas() {
        let tmp = TempDir::new();
        let dir = tmp.recovery();
        let mut live = LiveCopies::default();
        // Dos ventanas abiertas con cambios, y una copia de una sesión anterior.
        put(&dir, "ventana-a", None);
        live.mark("main", "ventana-a");
        put(&dir, "ventana-b", None);
        live.mark("doc-1", "ventana-b");
        put(&dir, "vieja", None);

        let ids = |live: &LiveCopies, asking: &str| -> Vec<String> {
            let mut v: Vec<String> = list_orphans(&dir, &live.held_by_others(asking))
                .into_iter()
                .map(|c| c.id)
                .collect();
            v.sort();
            v
        };
        // Una ventana nueva no ve las copias de las otras dos, solo la huérfana.
        assert_eq!(ids(&live, "doc-2"), vec!["vieja"]);
        // La propia ventana que pregunta (recarga de la página) sí recupera la suya.
        assert_eq!(ids(&live, "main"), vec!["ventana-a", "vieja"]);

        // Al cerrarse una ventana su copia queda huérfana y se ofrece.
        live.release_window("doc-1");
        assert_eq!(ids(&live, "doc-2"), vec!["ventana-b", "vieja"]);

        // Tras un reinicio (registro vacío) todas son huérfanas.
        let fresh = LiveCopies::default();
        assert_eq!(list_orphans(&dir, &fresh.held_by_others("main")).len(), 3);
        // Y las copias vivas no se borran por consultar la lista.
        assert!(doc_file(&dir, "ventana-a").is_file());
    }
}
