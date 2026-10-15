//! Historial de trabajos (mejora 4 del plan de usuarios): un archivo JSONL (`job-history.jsonl`, un
//! trabajo por línea) en la carpeta de datos de la app. Sustituye al registro de `localStorage`
//! (200 entradas, sin ajustes): sobrevive a limpiar el WebView, lo comparten las ventanas y no sale
//! del equipo.
//!
//! Rust no conoce el significado de cada campo (lo define y sanea la interfaz, `features/help/jobLog.ts`);
//! exige solo lo que necesita para guardar y rotar: un objeto con `id` (texto) y `at` (ms desde 1970,
//! número), de a lo más [`MAX_ENTRY_BYTES`]. Toda escritura es atómica (`write_atomic`: archivo
//! temporal y renombrado), así que un corte a mitad no deja líneas a medias. Un archivo con líneas
//! dañadas se lee igual (se saltan) y, antes de reescribirlo sin ellas, se aparta una copia como
//! `job-history.jsonl.dañado`.
//!
//! La rotación borra siempre las entradas más viejas (por `at`) al pasar el tope que pide la
//! interfaz (preferencia `jobHistoryMax`), acotado a [`MIN_MAX`]..=[`HARD_MAX`].

use crate::engine::write_atomic;
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tlaku_i18n::tr;

/// Nombre del archivo, dentro de la carpeta de datos de la app.
pub const FILE: &str = "job-history.jsonl";
/// Tope por omisión de trabajos guardados.
pub const DEFAULT_MAX: usize = 1000;
/// Menos que esto no se guarda (el historial dejaría de servir y la calibración de tiempos con él).
pub const MIN_MAX: usize = 20;
/// Más que esto tampoco: el archivo se reescribe entero en cada cambio.
pub const HARD_MAX: usize = 20_000;
/// Una entrada (con su miniatura) no pasa de esto; una más grande es un error de la interfaz.
pub const MAX_ENTRY_BYTES: usize = 48 * 1024;
/// Tamaño máximo de un CSV exportado.
pub const MAX_EXPORT_BYTES: usize = 32 * 1024 * 1024;

/// Serializa las operaciones: varias ventanas pueden anotar a la vez.
static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> std::sync::MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn clamp_max(max: Option<usize>) -> usize {
    max.unwrap_or(DEFAULT_MAX).clamp(MIN_MAX, HARD_MAX)
}

fn io_error(path: &Path, e: &std::io::Error) -> String {
    format!("{}: {e}", path.display())
}

fn id_of(v: &Value) -> Option<&str> {
    v.get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

fn at_of(v: &Value) -> f64 {
    v.get("at").and_then(Value::as_f64).unwrap_or(0.0)
}

/// Comprueba que `v` se puede guardar: objeto con `id` y `at`, y de tamaño razonable.
pub fn check_entry(v: &Value) -> Result<(), String> {
    if !v.is_object() || id_of(v).is_none() || !v.get("at").is_some_and(Value::is_number) {
        return Err(tr!("desktop.job_history.bad_entry"));
    }
    let size = v.to_string().len();
    if size > MAX_ENTRY_BYTES {
        return Err(tr!(
            "desktop.job_history.entry_too_big",
            size = size,
            max = MAX_ENTRY_BYTES
        ));
    }
    Ok(())
}

/// Entradas válidas del archivo (de la más vieja a la más nueva) y cuántas líneas se saltaron.
fn read_all(path: &Path) -> Result<(Vec<Value>, usize), String> {
    let text = match fs::read(path) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((Vec::new(), 0)),
        Err(e) => return Err(io_error(path, &e)),
    };
    let mut out = Vec::new();
    let mut skipped = 0;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        match serde_json::from_str::<Value>(line) {
            Ok(v) if check_entry(&v).is_ok() => out.push(v),
            _ => skipped += 1,
        }
    }
    sort_by_time(&mut out);
    Ok((out, skipped))
}

/// Orden estable de la más vieja a la más nueva.
fn sort_by_time(entries: &mut [Value]) {
    entries.sort_by(|a, b| at_of(a).total_cmp(&at_of(b)));
}

/// Deja las `max` más nuevas.
fn rotate(entries: &mut Vec<Value>, max: usize) {
    if entries.len() > max {
        entries.drain(..entries.len() - max);
    }
}

fn write_all(path: &Path, entries: &[Value], had_skipped: bool) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| io_error(dir, &e))?;
    }
    if had_skipped {
        // Antes de tirar líneas ilegibles se guarda el original (una vez).
        let aside = path.with_extension("jsonl.dañado");
        if !aside.exists() {
            let _ = fs::copy(path, &aside);
        }
    }
    let mut text = String::new();
    for e in entries {
        text.push_str(&e.to_string());
        text.push('\n');
    }
    write_atomic(path, &text)
}

/// Lee el historial (de la más vieja a la más nueva). Si el archivo pasa de `max`, lo rota.
pub fn load(dir: &Path, max: Option<usize>) -> Result<Vec<Value>, String> {
    let _g = lock();
    let path = dir.join(FILE);
    let (mut entries, skipped) = read_all(&path)?;
    let max = clamp_max(max);
    if entries.len() > max {
        rotate(&mut entries, max);
        write_all(&path, &entries, skipped > 0)?;
    }
    Ok(entries)
}

/// Agrega una entrada (o reemplaza la que tiene el mismo `id`) y rota.
pub fn append(dir: &Path, entry: Value, max: Option<usize>) -> Result<(), String> {
    check_entry(&entry)?;
    let _g = lock();
    let path = dir.join(FILE);
    let (mut entries, skipped) = read_all(&path)?;
    let id = id_of(&entry).unwrap_or_default().to_owned();
    match entries.iter().position(|e| id_of(e) == Some(id.as_str())) {
        Some(i) => entries[i] = entry,
        None => entries.push(entry),
    }
    sort_by_time(&mut entries);
    rotate(&mut entries, clamp_max(max));
    write_all(&path, &entries, skipped > 0)
}

/// Mezcla `patch` (campos de primer nivel) en la entrada `id`; un `null` borra el campo. Devuelve si existía.
pub fn update(dir: &Path, id: &str, patch: &Map<String, Value>) -> Result<bool, String> {
    let _g = lock();
    let path = dir.join(FILE);
    let (mut entries, skipped) = read_all(&path)?;
    let Some(entry) = entries.iter_mut().find(|e| id_of(e) == Some(id)) else {
        return Ok(false);
    };
    let Some(obj) = entry.as_object_mut() else {
        return Ok(false);
    };
    for (k, v) in patch {
        // `id` y `at` identifican la entrada: no se cambian.
        if k == "id" || k == "at" {
            continue;
        }
        if v.is_null() {
            obj.remove(k);
        } else {
            obj.insert(k.clone(), v.clone());
        }
    }
    check_entry(entry)?;
    write_all(&path, &entries, skipped > 0)?;
    Ok(true)
}

/// Quita las entradas con esos `ids`. Devuelve cuántas quitó.
pub fn remove(dir: &Path, ids: &[String]) -> Result<usize, String> {
    let _g = lock();
    let path = dir.join(FILE);
    let (mut entries, skipped) = read_all(&path)?;
    let before = entries.len();
    entries.retain(|e| id_of(e).is_none_or(|id| !ids.iter().any(|x| x == id)));
    let removed = before - entries.len();
    if removed > 0 {
        write_all(&path, &entries, skipped > 0)?;
    }
    Ok(removed)
}

/// Borra todo el historial.
pub fn clear(dir: &Path) -> Result<(), String> {
    let _g = lock();
    let path = dir.join(FILE);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_error(&path, &e)),
    }
}

/// Migración: agrega las entradas válidas cuyo `id` aún no está (el registro viejo de `localStorage`).
/// Devuelve cuántas entraron.
pub fn import(dir: &Path, incoming: Vec<Value>, max: Option<usize>) -> Result<usize, String> {
    let _g = lock();
    let path = dir.join(FILE);
    let (mut entries, skipped) = read_all(&path)?;
    let mut added = 0;
    for entry in incoming {
        if check_entry(&entry).is_err() {
            continue;
        }
        let id = id_of(&entry).unwrap_or_default();
        if entries.iter().any(|e| id_of(e) == Some(id)) {
            continue;
        }
        entries.push(entry);
        added += 1;
    }
    if added > 0 {
        sort_by_time(&mut entries);
        rotate(&mut entries, clamp_max(max));
        write_all(&path, &entries, skipped > 0)?;
    }
    Ok(added)
}

/// Escribe el CSV exportado. Exige la extensión `.csv` y un tamaño razonable.
pub fn export_csv(path: &Path, text: &str) -> Result<(), String> {
    let is_csv = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("csv"));
    if !is_csv {
        return Err(tr!("desktop.job_history.csv_ext", path = path.display()));
    }
    if text.len() > MAX_EXPORT_BYTES {
        return Err(tr!(
            "desktop.job_history.csv_too_big",
            size = text.len(),
            max = MAX_EXPORT_BYTES
        ));
    }
    write_atomic(path, text)
}

// ---------------------------------------------------------------------------
// Comandos de Tauri
// ---------------------------------------------------------------------------

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| tr!("desktop.job_history.data_dir", err = e))
}

/// Historial guardado, de la más vieja a la más nueva. `max`: tope (por omisión 1000); si el archivo
/// lo pasa, se rota.
#[tauri::command]
pub async fn job_history_load(app: AppHandle, max: Option<u32>) -> Result<Vec<Value>, String> {
    let dir = data_dir(&app)?;
    crate::commands::blocking(move || load(&dir, max.map(|m| m as usize))).await
}

/// Agrega (o reemplaza, por `id`) una entrada y rota al tope `max`.
#[tauri::command]
pub async fn job_history_append(
    app: AppHandle,
    entry: Value,
    max: Option<u32>,
) -> Result<(), String> {
    let dir = data_dir(&app)?;
    crate::commands::blocking(move || append(&dir, entry, max.map(|m| m as usize))).await
}

/// Cambia campos de una entrada (notas, marca de interrupción). `null` borra el campo.
#[tauri::command]
pub async fn job_history_update(
    app: AppHandle,
    id: String,
    patch: Map<String, Value>,
) -> Result<bool, String> {
    let dir = data_dir(&app)?;
    crate::commands::blocking(move || update(&dir, &id, &patch)).await
}

/// Quita entradas por `id`.
#[tauri::command]
pub async fn job_history_remove(app: AppHandle, ids: Vec<String>) -> Result<u32, String> {
    let dir = data_dir(&app)?;
    crate::commands::blocking(move || remove(&dir, &ids).map(|n| n as u32)).await
}

/// Borra todo el historial.
#[tauri::command]
pub async fn job_history_clear(app: AppHandle) -> Result<(), String> {
    let dir = data_dir(&app)?;
    crate::commands::blocking(move || clear(&dir)).await
}

/// Migra entradas de otro almacén (el `localStorage` anterior). Devuelve cuántas entraron.
#[tauri::command]
pub async fn job_history_import(
    app: AppHandle,
    entries: Vec<Value>,
    max: Option<u32>,
) -> Result<u32, String> {
    let dir = data_dir(&app)?;
    crate::commands::blocking(move || {
        import(&dir, entries, max.map(|m| m as usize)).map(|n| n as u32)
    })
    .await
}

/// Escribe el CSV del historial en `path` (extensión `.csv`).
#[tauri::command]
pub async fn job_history_export(path: String, text: String) -> Result<(), String> {
    crate::commands::blocking(move || export_csv(Path::new(&path), &text)).await
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
            let dir =
                std::env::temp_dir().join(format!("tlaku-job-history-{}-{n}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn file(&self) -> PathBuf {
            self.0.join(FILE)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn job(id: &str, at: u64) -> Value {
        json!({ "id": id, "at": at, "machine": "Láser A", "result": "finished" })
    }

    fn ids(entries: &[Value]) -> Vec<String> {
        entries
            .iter()
            .map(|e| e["id"].as_str().unwrap().to_owned())
            .collect()
    }

    #[test]
    fn sin_archivo_el_historial_esta_vacio() {
        let dir = TempDir::new();
        assert!(load(&dir.0, None).unwrap().is_empty());
        assert!(!dir.file().exists());
    }

    #[test]
    fn ida_y_vuelta_ordenada_por_fecha() {
        let dir = TempDir::new();
        append(&dir.0, job("b", 200), None).unwrap();
        append(&dir.0, job("a", 100), None).unwrap();
        append(&dir.0, job("c", 300), None).unwrap();
        assert_eq!(ids(&load(&dir.0, None).unwrap()), ["a", "b", "c"]);
        // Es JSONL: una línea por trabajo.
        let text = fs::read_to_string(dir.file()).unwrap();
        assert_eq!(text.lines().count(), 3);
        for line in text.lines() {
            serde_json::from_str::<Value>(line).unwrap();
        }
    }

    #[test]
    fn el_mismo_id_reemplaza_y_no_duplica() {
        let dir = TempDir::new();
        append(&dir.0, json!({"id":"x","at":1,"result":"running"}), None).unwrap();
        append(&dir.0, json!({"id":"x","at":1,"result":"finished"}), None).unwrap();
        let all = load(&dir.0, None).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0]["result"], "finished");
    }

    #[test]
    fn la_rotacion_borra_las_mas_viejas() {
        let dir = TempDir::new();
        for i in 0..30u64 {
            append(&dir.0, job(&format!("j{i:02}"), 1000 + i), Some(25)).unwrap();
        }
        let all = load(&dir.0, Some(25)).unwrap();
        assert_eq!(all.len(), 25);
        assert_eq!(all[0]["id"], "j05");
        assert_eq!(all[24]["id"], "j29");
    }

    #[test]
    fn el_tope_tiene_piso_y_techo() {
        assert_eq!(clamp_max(Some(1)), MIN_MAX);
        assert_eq!(clamp_max(Some(10_000_000)), HARD_MAX);
        assert_eq!(clamp_max(None), DEFAULT_MAX);
        // Bajar el tope rota al leer.
        let dir = TempDir::new();
        let entries: Vec<Value> = (0..40u64).map(|i| job(&format!("j{i:02}"), i)).collect();
        import(&dir.0, entries, Some(1000)).unwrap();
        assert_eq!(load(&dir.0, Some(30)).unwrap().len(), 30);
        assert_eq!(fs::read_to_string(dir.file()).unwrap().lines().count(), 30);
    }

    #[test]
    fn rechaza_entradas_invalidas_y_demasiado_grandes() {
        let dir = TempDir::new();
        for bad in [
            json!("texto"),
            json!({"at": 1}),
            json!({"id": "", "at": 1}),
            json!({"id": "x"}),
            json!({"id": "x", "at": "ayer"}),
        ] {
            assert!(append(&dir.0, bad.clone(), None).is_err(), "{bad}");
        }
        let big = json!({"id": "x", "at": 1, "thumb": "a".repeat(MAX_ENTRY_BYTES)});
        assert!(
            append(&dir.0, big, None)
                .unwrap_err()
                .contains("demasiado grande")
        );
        assert!(!dir.file().exists());
    }

    #[test]
    fn update_mezcla_campos_y_null_los_borra() {
        let dir = TempDir::new();
        append(&dir.0, job("a", 5), None).unwrap();
        let patch = json!({"notes": "sobró", "result": null, "id": "otro", "at": 99});
        assert!(update(&dir.0, "a", patch.as_object().unwrap()).unwrap());
        let all = load(&dir.0, None).unwrap();
        assert_eq!(all[0]["notes"], "sobró");
        assert!(all[0].get("result").is_none());
        // `id` y `at` no se tocan.
        assert_eq!(all[0]["id"], "a");
        assert_eq!(all[0]["at"], 5);
        assert!(!update(&dir.0, "no-existe", patch.as_object().unwrap()).unwrap());
    }

    #[test]
    fn quitar_y_borrar_todo() {
        let dir = TempDir::new();
        for (i, id) in ["a", "b", "c"].iter().enumerate() {
            append(&dir.0, job(id, i as u64), None).unwrap();
        }
        assert_eq!(remove(&dir.0, &["a".into(), "zzz".into()]).unwrap(), 1);
        assert_eq!(ids(&load(&dir.0, None).unwrap()), ["b", "c"]);
        clear(&dir.0).unwrap();
        clear(&dir.0).unwrap();
        assert!(load(&dir.0, None).unwrap().is_empty());
    }

    #[test]
    fn migracion_agrega_lo_nuevo_sin_duplicar_y_salta_lo_invalido() {
        let dir = TempDir::new();
        append(&dir.0, job("b", 200), None).unwrap();
        let old = vec![
            job("a", 100),
            job("b", 200),
            json!({"sin": "id"}),
            job("c", 300),
        ];
        assert_eq!(import(&dir.0, old.clone(), None).unwrap(), 2);
        assert_eq!(ids(&load(&dir.0, None).unwrap()), ["a", "b", "c"]);
        // Repetirla no cambia nada (la interfaz la borra del origen, pero por si acaso).
        assert_eq!(import(&dir.0, old, None).unwrap(), 0);
    }

    #[test]
    fn lineas_danadas_se_saltan_y_se_guarda_el_original() {
        let dir = TempDir::new();
        let original = format!(
            "{}\n{{esto no es json\n\n{}\n[1,2]\n",
            job("a", 1),
            job("b", 2)
        );
        fs::write(dir.file(), &original).unwrap();
        assert_eq!(ids(&load(&dir.0, None).unwrap()), ["a", "b"]);
        // Al reescribir sin lo dañado, queda copia del original.
        append(&dir.0, job("c", 3), None).unwrap();
        assert_eq!(ids(&load(&dir.0, None).unwrap()), ["a", "b", "c"]);
        assert_eq!(
            fs::read_to_string(dir.0.join("job-history.jsonl.dañado")).unwrap(),
            original
        );
    }

    #[test]
    fn la_escritura_no_deja_temporales() {
        let dir = TempDir::new();
        append(&dir.0, job("a", 1), None).unwrap();
        append(&dir.0, job("b", 2), None).unwrap();
        let names: Vec<_> = fs::read_dir(&dir.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec![FILE.to_string()]);
    }

    #[test]
    fn exportar_csv_exige_extension_y_tamano() {
        let dir = TempDir::new();
        let ok = dir.0.join("historial.csv");
        export_csv(&ok, "a;b\n1;2\n").unwrap();
        assert_eq!(fs::read_to_string(&ok).unwrap(), "a;b\n1;2\n");
        let bad = dir.0.join("historial.exe");
        assert!(export_csv(&bad, "x").unwrap_err().contains(".csv"));
        assert!(!bad.exists());
        let huge = "x".repeat(MAX_EXPORT_BYTES + 1);
        assert!(
            export_csv(&ok, &huge)
                .unwrap_err()
                .contains("demasiado grande")
        );
    }
}
