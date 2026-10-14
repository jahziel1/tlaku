//! Archivo de respaldo `.tlakuruida` (JSON propio de Tlaku) y fechas para los respaldos
//! automáticos. El formato es nuestro: no tiene relación con el `.lbset` de LightBurn.

use super::SettingsError;
use super::defs;
use super::plan::{SettingChange, SettingsSnapshot};
use serde::{Deserialize, Serialize};

/// Marca que identifica el archivo.
pub const FORMAT: &str = "tlaku-ruida-settings";
pub const FILE_VERSION: u32 = 1;
/// Un respaldo real pesa unos 2 KiB; más de 64 KiB no es uno.
pub const MAX_FILE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub key: String,
    /// Dirección en hexadecimal (`0x023`): se comprueba contra la clave al leer.
    pub addr: String,
    pub value: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsFile {
    pub format: String,
    pub version: u32,
    /// Fecha UTC ISO 8601 (informativa).
    pub created: String,
    /// ID de tarjeta de la controladora de la que sale (`0x65106510`).
    pub card_id: String,
    pub model: Option<String>,
    pub settings: Vec<FileEntry>,
}

impl SettingsFile {
    /// Respaldo de una instantánea completa o parcial (solo entra lo que contestó la máquina).
    pub fn from_snapshot(snap: &SettingsSnapshot, created: &str) -> Self {
        SettingsFile {
            format: FORMAT.to_string(),
            version: FILE_VERSION,
            created: created.to_string(),
            card_id: format!("0x{:08X}", snap.card_id),
            model: snap.model.clone(),
            settings: snap
                .values
                .iter()
                .filter_map(|v| {
                    Some(FileEntry {
                        key: v.key.clone(),
                        addr: format!("0x{:03X}", v.addr),
                        value: v.value?,
                    })
                })
                .collect(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("el respaldo siempre se serializa") + "\n"
    }

    /// ID de tarjeta como número.
    pub fn card(&self) -> Result<u32, SettingsError> {
        let t = self.card_id.trim();
        let t = t
            .strip_prefix("0x")
            .or_else(|| t.strip_prefix("0X"))
            .unwrap_or(t);
        u32::from_str_radix(t, 16)
            .map_err(|_| SettingsError::BadFile(format!("card_id: {}", self.card_id)))
    }

    /// Los valores del archivo como cambios pedidos.
    pub fn changes(&self) -> Vec<SettingChange> {
        self.settings
            .iter()
            .filter_map(|e| {
                Some(SettingChange {
                    addr: defs::find_key(&e.key)?.addr,
                    value: e.value,
                })
            })
            .collect()
    }
}

/// Lee y valida un respaldo: tamaño, formato, versión y que cada entrada sea de la lista cerrada
/// (clave y dirección coherentes, valor dentro de rango). Nada de lo dudoso se ignora.
pub fn parse_file(text: &str) -> Result<SettingsFile, SettingsError> {
    if text.len() > MAX_FILE_BYTES {
        return Err(SettingsError::BadFile(format!(
            "size {} > {MAX_FILE_BYTES}",
            text.len()
        )));
    }
    let file: SettingsFile =
        serde_json::from_str(text).map_err(|e| SettingsError::BadFile(e.to_string()))?;
    if file.format != FORMAT {
        return Err(SettingsError::BadFile(format!("format = {}", file.format)));
    }
    if file.version != FILE_VERSION {
        return Err(SettingsError::BadFile(format!(
            "version = {}",
            file.version
        )));
    }
    file.card()?;
    let mut seen: Vec<&str> = Vec::new();
    for e in &file.settings {
        let def = defs::find_key(&e.key)
            .ok_or_else(|| SettingsError::BadFile(format!("unknown key: {}", e.key)))?;
        let want = format!("0x{:03X}", def.addr);
        if !e.addr.eq_ignore_ascii_case(&want) {
            return Err(SettingsError::BadFile(format!(
                "{}: addr {} (expected {want})",
                e.key, e.addr
            )));
        }
        if seen.contains(&e.key.as_str()) {
            return Err(SettingsError::BadFile(format!("repeated key: {}", e.key)));
        }
        seen.push(&e.key);
        def.encode(e.value)?;
    }
    Ok(file)
}

/// Fecha UTC (`AAAA-MM-DDTHH:MM:SSZ`) de unos segundos Unix.
pub fn format_utc(unix_secs: u64) -> String {
    let (y, m, d, hh, mm, ss) = civil(unix_secs);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Nombre del respaldo automático: `ruida-AAAAMMDD-HHMMSS.tlakuruida` (UTC).
pub fn backup_file_name(unix_secs: u64) -> String {
    let (y, m, d, hh, mm, ss) = civil(unix_secs);
    format!("ruida-{y:04}{m:02}{d:02}-{hh:02}{mm:02}{ss:02}.tlakuruida")
}

/// Fecha y hora civil (UTC) de unos segundos Unix (algoritmo de días civiles de H. Hinnant).
fn civil(unix_secs: u64) -> (i64, u32, u32, u32, u32, u32) {
    let days = (unix_secs / 86_400) as i64;
    let rem = unix_secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (
        y,
        m,
        d,
        (rem / 3_600) as u32,
        (rem % 3_600 / 60) as u32,
        (rem % 60) as u32,
    )
}
