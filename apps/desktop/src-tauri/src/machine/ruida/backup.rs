//! Respaldos automáticos de los ajustes de la controladora Ruida (EXPERIMENTAL).
//!
//! Antes de escribir ajustes, el motor lee toda la lista y la guarda aquí, en un archivo con
//! fecha (`ruida-AAAAMMDD-HHMMSS.tlakuruida`, UTC) dentro de `ruida-backups/` de la carpeta de
//! configuración de la app. Si guardarlo falla, no se escribe nada en la máquina.

use crate::engine::write_atomic;
use std::fs;
use std::path::{Path, PathBuf};
use tlaku_doc::RuidaBackup;
use tlaku_i18n::tr;
use tlaku_ruida::settings::{self, SettingsFile, SettingsSnapshot};

/// Extensión del archivo de respaldo.
pub const EXTENSION: &str = "tlakuruida";
/// Carpeta, dentro de la de configuración.
pub const DIR_NAME: &str = "ruida-backups";

/// Carpeta de los respaldos automáticos.
pub fn backups_dir(config_dir: &Path) -> PathBuf {
    config_dir.join(DIR_NAME)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Guarda la instantánea y devuelve la ruta. Nunca pisa un respaldo existente: si ya hay uno de
/// ese segundo, agrega `-2`, `-3`...
pub fn save_snapshot(
    dir: &Path,
    snap: &SettingsSnapshot,
    unix_secs: u64,
) -> Result<String, String> {
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let name = settings::backup_file_name(unix_secs);
    let stem = name.trim_end_matches(&format!(".{EXTENSION}")).to_string();
    let mut path = dir.join(&name);
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{stem}-{n}.{EXTENSION}"));
        n += 1;
    }
    let file = SettingsFile::from_snapshot(snap, &settings::format_utc(unix_secs));
    write_atomic(&path, &file.to_json())?;
    Ok(path.display().to_string())
}

/// El guardado que usa el motor antes de escribir: reloj actual.
pub fn save_now(dir: &Path, snap: &SettingsSnapshot) -> Result<String, String> {
    save_snapshot(dir, snap, now_secs())
}

/// Respaldos guardados, del más reciente al más antiguo. Los archivos ajenos o ilegibles se
/// omiten.
pub fn list(dir: &Path) -> Vec<RuidaBackup> {
    let Ok(rd) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<RuidaBackup> = rd
        .filter_map(Result::ok)
        .filter_map(|e| {
            let path = e.path();
            let name = path.file_name()?.to_str()?.to_string();
            if !name.ends_with(&format!(".{EXTENSION}")) || !path.is_file() {
                return None;
            }
            let text = fs::read_to_string(&path).ok()?;
            let file = settings::parse_file(&text).ok()?;
            Some(RuidaBackup {
                name,
                path: path.display().to_string(),
                created: file.created,
                card_id: file.card_id,
            })
        })
        .collect();
    // Los nombres llevan la fecha: orden descendente = más reciente primero.
    out.sort_by(|a, b| b.name.cmp(&a.name));
    out
}

/// Texto de un respaldo por nombre de archivo. El nombre no puede llevar rutas.
pub fn read(dir: &Path, name: &str) -> Result<String, String> {
    let ok = !name.is_empty()
        && name.ends_with(&format!(".{EXTENSION}"))
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
        && !name.contains("..");
    if !ok {
        return Err(tr!("desktop.ruida.backup_bad_name", name = name));
    }
    let path = dir.join(name);
    fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
}
