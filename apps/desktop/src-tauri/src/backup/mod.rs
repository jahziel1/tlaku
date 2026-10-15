//! Respaldo de los ajustes a una carpeta que elige la persona (mejora 12, versión reducida).
//!
//! La interfaz arma el paquete `.tlakubundle` (ver `features/bundle`) y lo entrega aquí; este módulo
//! solo lo valida, comprueba que no lleve secretos, lo escribe de forma atómica en la carpeta elegida
//! con un nombre fechado y rota los viejos. La carpeta suele estar en iCloud Drive, Dropbox, OneDrive
//! o una memoria USB: puede no estar disponible (unidad desconectada), y eso no es un error grave:
//! se avisa y la app sigue.
//!
//! Qué NO hace: sincronizar en vivo ni fusionar solo. Detecta que hay un respaldo más nuevo hecho en
//! otra computadora y lo ofrece (`backup_check_newer`); aplicarlo siempre pasa por la vista previa.
//!
//! Estado propio en `backup.json` (carpeta de configuración): la carpeta elegida, cada cuánto
//! respaldar, cuántos conservar, la marca del último respaldo y un identificador de esta instalación.
//! No viaja en los paquetes (la ruta es de esta computadora).

use crate::engine::write_atomic;
use crate::prefs::{USER_BUNDLE_MAX_BYTES, check_user_bundle};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};
use tlaku_i18n::tr;

const CONFIG_FILE: &str = "backup.json";
const CONFIG_VERSION: u32 = 1;
/// Nombre de los respaldos que van a la carpeta elegida.
pub const FOLDER_PREFIX: &str = "tlaku-respaldo";
/// Respaldos del estado anterior que se guardan antes de importar (carpeta propia, local).
pub const SAFETY_PREFIX: &str = "tlaku-antes-de-importar";
const SAFETY_DIR: &str = "respaldos-locales";
const SAFETY_KEEP: usize = 5;
const EXT: &str = "tlakubundle";
pub const DEFAULT_KEEP: u32 = 10;
pub const MAX_KEEP: u32 = 100;
pub const MAX_EVERY_DAYS: u32 = 365;
/// Un `.tmp` propio más viejo que esto (ms) es de un corte a medias y se limpia al rotar.
const STALE_TMP_MS: u64 = 10 * 60 * 1000;

// ---------------------------------------------------------------------------
// Configuración
// ---------------------------------------------------------------------------

/// Lo que la persona decide en el diálogo de respaldo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BackupSettings {
    /// Carpeta de destino; `None` = todavía no se eligió (no hay respaldo automático).
    pub folder: Option<String>,
    /// Respaldar al cerrar la aplicación.
    pub on_close: bool,
    /// Respaldar al abrir si pasaron este número de días desde el último (0 = no).
    pub every_days: u32,
    /// Cuántos respaldos conservar en la carpeta (los más nuevos).
    pub keep: u32,
    /// Incluir el historial de trabajos (puede ser grande).
    pub include_history: bool,
}

impl Default for BackupSettings {
    fn default() -> Self {
        Self {
            folder: None,
            on_close: false,
            every_days: 0,
            keep: DEFAULT_KEEP,
            include_history: false,
        }
    }
}

impl BackupSettings {
    /// Valores dentro de rango y carpeta sin espacios de más.
    pub fn sanitized(mut self) -> Self {
        self.folder = self
            .folder
            .map(|f| f.trim().to_string())
            .filter(|f| !f.is_empty());
        self.keep = self.keep.clamp(1, MAX_KEEP);
        self.every_days = self.every_days.min(MAX_EVERY_DAYS);
        self
    }
}

/// Configuración guardada: lo elegido más el estado que lleva la app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BackupConfig {
    pub version: u32,
    #[serde(flatten)]
    pub settings: BackupSettings,
    /// Marca (ms desde 1970) del último respaldo hecho aquí a la carpeta.
    pub last_backup_ms: u64,
    /// Marca del respaldo más nuevo que ya se importó o que se decidió no importar.
    pub last_seen_ms: u64,
    /// Identificador de esta instalación (para no ofrecer como «de otra computadora» lo propio).
    pub install_id: String,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            settings: BackupSettings::default(),
            last_backup_ms: 0,
            last_seen_ms: 0,
            install_id: String::new(),
        }
    }
}

fn io_error(path: &Path, e: &std::io::Error) -> String {
    format!("{}: {e}", path.display())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn new_install_id() -> String {
    let mut buf = [0u8; 8];
    if getrandom::fill(&mut buf).is_err() {
        // Sin aleatoriedad del sistema: la hora basta, solo distingue instalaciones.
        buf = now_ms().to_le_bytes();
    }
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

static CONFIG_LOCK: Mutex<()> = Mutex::new(());

fn lock() -> std::sync::MutexGuard<'static, ()> {
    CONFIG_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Lee la configuración (la crea con su identificador si no existe). Un archivo dañado se aparta
/// como `backup.json.dañado` y se parte de cero; se pierde el estado, no los respaldos.
pub fn load_config(dir: &Path) -> Result<BackupConfig, String> {
    let file = dir.join(CONFIG_FILE);
    let mut cfg = match fs::read_to_string(&file) {
        Ok(text) => match serde_json::from_str::<BackupConfig>(&text) {
            Ok(c) if c.version <= CONFIG_VERSION => c,
            _ => {
                let _ = fs::rename(&file, dir.join(format!("{CONFIG_FILE}.dañado")));
                BackupConfig::default()
            }
        },
        Err(_) => BackupConfig::default(),
    };
    cfg.settings = cfg.settings.sanitized();
    if cfg.install_id.is_empty() {
        cfg.install_id = new_install_id();
        store_config(dir, &cfg)?;
    }
    Ok(cfg)
}

pub fn store_config(dir: &Path, cfg: &BackupConfig) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| io_error(dir, &e))?;
    let text = serde_json::to_string_pretty(cfg)
        .map_err(|e| tr!("desktop.prefs.serialize_failed", err = e))?;
    write_atomic(&dir.join(CONFIG_FILE), &text)
}

// ---------------------------------------------------------------------------
// Secretos
// ---------------------------------------------------------------------------

/// Nombres de campo (sin guiones ni mayúsculas) que nunca deben ir en un respaldo.
const SECRET_KEYS: &[&str] = &[
    "token",
    "remotetoken",
    "accesstoken",
    "authtoken",
    "bearer",
    "secret",
    "clientsecret",
    "password",
    "passwd",
    "passphrase",
    "apikey",
    "privatekey",
    "secretkey",
    "accesskey",
    "authorization",
    "cookie",
    "credentials",
    "credential",
];

fn is_secret_key(key: &str) -> bool {
    let k: String = key
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    SECRET_KEYS.contains(&k.as_str())
        || k.ends_with("token")
        || k.ends_with("password")
        || k.ends_with("secret")
}

/// `esquema://usuario@host` o `esquema://usuario:clave@host`: una URL con credenciales dentro.
fn url_with_credentials(s: &str) -> bool {
    let Some(i) = s.find("://") else {
        return false;
    };
    let authority = s[i + 3..].split(['/', '?', '#']).next().unwrap_or("");
    authority.contains('@')
}

/// Secciones de texto libre del usuario (diseños y notas): ahí no se buscan URLs con credenciales.
const FREE_TEXT_SECTIONS: &[&str] = &["templates", "history"];

fn scan(value: &Value, path: &str, free_text: bool) -> Result<(), String> {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let here = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                if is_secret_key(k) && !v.is_null() {
                    return Err(tr!("desktop.backup.secret_found", key = here));
                }
                let free =
                    free_text || (path.is_empty() && FREE_TEXT_SECTIONS.contains(&k.as_str()));
                scan(v, &here, free)?;
            }
            Ok(())
        }
        Value::Array(items) => items.iter().try_for_each(|v| scan(v, path, free_text)),
        Value::String(s) if !free_text && url_with_credentials(s) => {
            Err(tr!("desktop.backup.secret_found", key = path))
        }
        _ => Ok(()),
    }
}

/// Falla si el paquete lleva algo que parece un secreto (token, clave, URL con usuario y clave).
/// La interfaz no los incluye nunca; esto es la segunda barrera.
pub fn check_no_secrets(bundle: &Value) -> Result<(), String> {
    scan(bundle, "", false)
}

// ---------------------------------------------------------------------------
// Nombres fechados
// ---------------------------------------------------------------------------

/// Días desde 1970-01-01 -> (año, mes, día), calendario gregoriano proléptico.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = i64::from(if m > 2 { m - 3 } else { m + 9 });
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `tlaku-respaldo-2026-10-15_14-30-12` (hora UTC: ordena igual en cualquier computadora).
fn stamp(prefix: &str, ms: u64) -> String {
    let secs = ms / 1000;
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    let rem = secs % 86_400;
    format!(
        "{prefix}-{y:04}-{m:02}-{d:02}_{:02}-{:02}-{:02}",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Grupos de dígitos separados por `-`: `Some` solo si hay exactamente `n` y todos son cifras.
fn digits(s: &str, n: usize) -> Option<Vec<i64>> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != n {
        return None;
    }
    parts
        .iter()
        .map(|p| {
            if p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()) {
                None
            } else {
                p.parse().ok()
            }
        })
        .collect()
}

/// Marca de tiempo (ms) que lleva el nombre de un respaldo propio; `None` si no es de los nuestros.
pub fn parse_name(prefix: &str, file_name: &str) -> Option<u64> {
    let stem = file_name.strip_suffix(&format!(".{EXT}"))?;
    let rest = stem.strip_prefix(prefix)?.strip_prefix('-')?;
    // `2026-10-15_14-30-12` o, si hubo dos en el mismo segundo, `2026-10-15_14-30-12_2`.
    let mut parts = rest.split('_');
    let (date, time) = (parts.next()?, parts.next()?);
    if let Some(n) = parts.next()
        && (n.is_empty() || !n.chars().all(|c| c.is_ascii_digit()) || parts.next().is_some())
    {
        return None;
    }
    let d = digits(date, 3)?;
    let t = digits(time, 3)?;
    if !(1..=12).contains(&d[1]) || !(1..=31).contains(&d[2]) || t[0] > 23 || t[1] > 59 || t[2] > 59
    {
        return None;
    }
    let days = days_from_civil(d[0], d[1] as u32, d[2] as u32);
    let secs = days * 86_400 + t[0] * 3600 + t[1] * 60 + t[2];
    u64::try_from(secs).ok().map(|s| s * 1000)
}

// ---------------------------------------------------------------------------
// Escribir, listar, rotar
// ---------------------------------------------------------------------------

/// Un respaldo de la carpeta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupEntry {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    /// Marca (ms desde 1970) tomada del nombre del archivo.
    pub created_ms: u64,
}

/// Lo que devuelve un respaldo recién escrito.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupWrite {
    pub name: String,
    pub path: String,
    pub created_ms: u64,
    /// Respaldos viejos que la rotación borró.
    pub removed: Vec<String>,
}

/// Respaldos propios (`prefix`) de `dir`, el más nuevo primero. Ignora lo que no sea de los nuestros.
pub fn list_backups(dir: &Path, prefix: &str) -> Result<Vec<BackupEntry>, String> {
    let read = fs::read_dir(dir).map_err(|e| io_error(dir, &e))?;
    let mut out: Vec<BackupEntry> = read
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            let created_ms = parse_name(prefix, &name)?;
            let meta = e.metadata().ok()?;
            meta.is_file().then(|| BackupEntry {
                path: e.path().display().to_string(),
                size_bytes: meta.len(),
                name,
                created_ms,
            })
        })
        .collect();
    out.sort_by(|a, b| (b.created_ms, &b.name).cmp(&(a.created_ms, &a.name)));
    Ok(out)
}

fn modified_ms(path: &Path) -> Option<u64> {
    fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as u64)
}

/// Borra los respaldos propios que sobran (conserva los `keep` más nuevos) y los `.tmp` huérfanos
/// de un corte a medias. Nunca toca archivos que no lleven el nombre de nuestros respaldos.
fn rotate(dir: &Path, prefix: &str, keep: usize, now: u64) -> Vec<String> {
    let mut removed = Vec::new();
    if let Ok(entries) = list_backups(dir, prefix) {
        for e in entries.iter().skip(keep) {
            if fs::remove_file(&e.path).is_ok() {
                removed.push(e.name.clone());
            }
        }
    }
    if let Ok(read) = fs::read_dir(dir) {
        for e in read.filter_map(Result::ok) {
            let Some(name) = e.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let ours = name
                .strip_suffix(&format!(".{EXT}.tmp"))
                .is_some_and(|stem| parse_name(prefix, &format!("{stem}.{EXT}")).is_some());
            let old = modified_ms(&e.path()).is_some_and(|m| now.saturating_sub(m) > STALE_TMP_MS);
            if ours && old {
                let _ = fs::remove_file(e.path());
            }
        }
    }
    removed
}

/// Escribe `bundle` como un respaldo fechado en `dir` y rota. La carpeta tiene que existir (si no,
/// es que la unidad no está: no se crea una carpeta con ese nombre en el disco local). La escritura
/// es atómica; si falla no se borra nada.
pub fn write_backup(
    dir: &Path,
    prefix: &str,
    bundle: &Value,
    install_id: &str,
    keep: usize,
    now: u64,
) -> Result<BackupWrite, String> {
    if !dir.is_dir() {
        return Err(tr!(
            "desktop.backup.folder_unavailable",
            path = dir.display()
        ));
    }
    check_user_bundle(bundle)?;
    check_no_secrets(bundle)?;
    let mut bundle = bundle.clone();
    if let Some(obj) = bundle.as_object_mut() {
        obj.insert("install_id".into(), Value::String(install_id.to_string()));
    }
    let text = serde_json::to_string_pretty(&bundle)
        .map_err(|e| tr!("desktop.prefs.serialize_failed", err = e))?;
    if text.len() as u64 > USER_BUNDLE_MAX_BYTES {
        return Err(tr!(
            "desktop.backup.too_big",
            size = text.len(),
            max = USER_BUNDLE_MAX_BYTES
        ));
    }
    let base = stamp(prefix, now);
    let mut name = format!("{base}.{EXT}");
    let mut n = 2;
    while dir.join(&name).exists() {
        name = format!("{base}_{n}.{EXT}");
        n += 1;
    }
    let path = dir.join(&name);
    write_atomic(&path, &text)?;
    let removed = rotate(dir, prefix, keep.max(1), now);
    Ok(BackupWrite {
        name,
        path: path.display().to_string(),
        created_ms: now,
        removed,
    })
}

/// `install_id` que lleva un respaldo (los hechos por otras versiones no lo tienen).
fn install_id_of(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    if meta.len() > USER_BUNDLE_MAX_BYTES {
        return None;
    }
    let value: Value = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    value.get("install_id")?.as_str().map(str::to_string)
}

/// El respaldo más nuevo de `entries` si es posterior a lo último que esta instalación respaldó o
/// vio, y lo hizo otra instalación. `owner` dice de quién es un respaldo.
pub fn newer_backup(
    entries: &[BackupEntry],
    cfg: &BackupConfig,
    owner: impl Fn(&BackupEntry) -> Option<String>,
) -> Option<BackupEntry> {
    let newest = entries.first()?;
    let known = cfg.last_backup_ms.max(cfg.last_seen_ms);
    if newest.created_ms <= known {
        return None;
    }
    (owner(newest).as_deref() != Some(cfg.install_id.as_str())).then(|| newest.clone())
}

// ---------------------------------------------------------------------------
// Comandos de Tauri
// ---------------------------------------------------------------------------

fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(crate::edit::config_dir_missing)
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| tr!("desktop.common.task_interrupted", err = e))?
}

/// Lista de respaldos de la carpeta configurada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupList {
    pub configured: bool,
    /// La carpeta existe y se pudo leer (una unidad desconectada da `false`).
    pub available: bool,
    pub entries: Vec<BackupEntry>,
}

fn folder_of(cfg: &BackupConfig) -> Option<PathBuf> {
    cfg.settings.folder.as_deref().map(PathBuf::from)
}

pub fn list_configured(cfg: &BackupConfig) -> BackupList {
    let Some(dir) = folder_of(cfg) else {
        return BackupList {
            configured: false,
            available: false,
            entries: Vec::new(),
        };
    };
    match list_backups(&dir, FOLDER_PREFIX) {
        Ok(entries) => BackupList {
            configured: true,
            available: true,
            entries,
        },
        Err(_) => BackupList {
            configured: true,
            available: false,
            entries: Vec::new(),
        },
    }
}

#[tauri::command]
pub async fn backup_config_get(app: AppHandle) -> Result<BackupConfig, String> {
    let dir = config_dir(&app)?;
    blocking(move || {
        let _g = lock();
        load_config(&dir)
    })
    .await
}

#[tauri::command]
pub async fn backup_config_set(
    app: AppHandle,
    settings: BackupSettings,
) -> Result<BackupConfig, String> {
    let dir = config_dir(&app)?;
    blocking(move || {
        let _g = lock();
        let mut cfg = load_config(&dir)?;
        cfg.settings = settings.sanitized();
        store_config(&dir, &cfg)?;
        Ok(cfg)
    })
    .await
}

/// Escribe un respaldo en la carpeta configurada y anota la marca.
#[tauri::command]
pub async fn backup_run(app: AppHandle, bundle: Value) -> Result<BackupWrite, String> {
    let dir = config_dir(&app)?;
    blocking(move || {
        let _g = lock();
        let mut cfg = load_config(&dir)?;
        let Some(folder) = folder_of(&cfg) else {
            return Err(tr!("desktop.backup.no_folder"));
        };
        let written = write_backup(
            &folder,
            FOLDER_PREFIX,
            &bundle,
            &cfg.install_id,
            cfg.settings.keep as usize,
            now_ms(),
        )?;
        cfg.last_backup_ms = written.created_ms;
        store_config(&dir, &cfg)?;
        Ok(written)
    })
    .await
}

/// Respaldo del estado actual antes de importar: siempre local (en la carpeta de configuración),
/// con los últimos 5.
#[tauri::command]
pub async fn backup_safety(app: AppHandle, bundle: Value) -> Result<BackupWrite, String> {
    let dir = config_dir(&app)?;
    blocking(move || {
        let _g = lock();
        let cfg = load_config(&dir)?;
        let safety = dir.join(SAFETY_DIR);
        fs::create_dir_all(&safety).map_err(|e| io_error(&safety, &e))?;
        write_backup(
            &safety,
            SAFETY_PREFIX,
            &bundle,
            &cfg.install_id,
            SAFETY_KEEP,
            now_ms(),
        )
    })
    .await
}

#[tauri::command]
pub async fn backup_list(app: AppHandle) -> Result<BackupList, String> {
    let dir = config_dir(&app)?;
    blocking(move || {
        let _g = lock();
        Ok(list_configured(&load_config(&dir)?))
    })
    .await
}

/// El respaldo de la carpeta más nuevo que el estado de esta computadora y hecho en otra, o `None`.
#[tauri::command]
pub async fn backup_check_newer(app: AppHandle) -> Result<Option<BackupEntry>, String> {
    let dir = config_dir(&app)?;
    blocking(move || {
        let _g = lock();
        let cfg = load_config(&dir)?;
        let list = list_configured(&cfg);
        Ok(newer_backup(&list.entries, &cfg, |e| {
            install_id_of(Path::new(&e.path))
        }))
    })
    .await
}

/// Anota que el respaldo con esa marca ya se vio (se importó o se descartó): no se vuelve a ofrecer.
#[tauri::command(rename_all = "snake_case")]
pub async fn backup_mark_seen(app: AppHandle, created_ms: u64) -> Result<(), String> {
    let dir = config_dir(&app)?;
    blocking(move || {
        let _g = lock();
        let mut cfg = load_config(&dir)?;
        cfg.last_seen_ms = cfg.last_seen_ms.max(created_ms);
        store_config(&dir, &cfg)
    })
    .await
}

#[cfg(test)]
mod tests;
