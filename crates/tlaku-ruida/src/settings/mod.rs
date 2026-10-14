//! Ajustes de la controladora: lista cerrada de lo escribible, instantáneas, diferencias, archivo
//! de respaldo `.tlakuruida` (JSON) y calibración de ejes.
//!
//! **EXPERIMENTAL: sin probar con una controladora real.** Escribir la memoria de una
//! controladora puede dejarla inutilizable. Por eso este módulo solo deja pasar lo que está en
//! [`defs::WRITABLE`], dentro de su rango, y la sesión exige además respaldo previo, confirmación
//! explícita y verificación posterior (`session/settings.rs`).

mod calibrate;
mod defs;
mod file;
mod plan;

pub use calibrate::{Calibration, calibrate_step};
pub use defs::{
    MAX_RAW, SettingDef, SettingGroup, SettingKind, WRITABLE, check_write, find, find_key,
};
pub use file::{
    FILE_VERSION, FORMAT, MAX_FILE_BYTES, SettingsFile, backup_file_name, format_utc, parse_file,
};
pub use plan::{PlannedChange, SettingChange, SettingValue, SettingsPlan, SettingsSnapshot, plan};

use std::fmt;
use tlaku_i18n::tr;

#[derive(Debug, Clone, PartialEq)]
pub enum SettingsError {
    /// La dirección no está en la lista cerrada de ajustes escribibles.
    NotWritable { addr: u16 },
    OutOfRange {
        addr: u16,
        key: &'static str,
        value: i64,
        min: i64,
        max: i64,
    },
    /// La misma dirección dos veces en una petición.
    Duplicate { addr: u16 },
    /// No se pudo leer el valor actual: sin él no hay respaldo ni comparación.
    NoCurrentValue { addr: u16, key: &'static str },
    /// Un paso de eje no puede cambiar más del doble ni menos de la mitad de una vez.
    StepJump {
        key: &'static str,
        current: i64,
        value: i64,
    },
    /// El archivo no es un respaldo de ajustes de Tlaku o no se puede leer.
    BadFile(String),
    /// El respaldo es de otra tarjeta que la conectada.
    CardMismatch { file: u32, machine: u32 },
    /// Calibración: medidas inválidas.
    BadMeasure(&'static str),
    /// Calibración: la corrección pedida es demasiado grande para ser una medida creíble.
    ImplausibleRatio { ratio: f64 },
    /// Calibración: el cambio es menor que la resolución del ajuste (1 unidad).
    BelowResolution { current: i64, ratio: f64 },
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingsError::NotWritable { addr } => f.write_str(&tr!(
                "ruida.settings.not_writable",
                addr = format!("0x{addr:03X}")
            )),
            SettingsError::OutOfRange {
                addr,
                key,
                value,
                min,
                max,
            } => f.write_str(&tr!(
                "ruida.settings.out_of_range",
                addr = format!("0x{addr:03X}"),
                key = key,
                value = value,
                min = min,
                max = max
            )),
            SettingsError::Duplicate { addr } => f.write_str(&tr!(
                "ruida.settings.duplicate",
                addr = format!("0x{addr:03X}")
            )),
            SettingsError::NoCurrentValue { addr, key } => f.write_str(&tr!(
                "ruida.settings.no_current",
                addr = format!("0x{addr:03X}"),
                key = key
            )),
            SettingsError::StepJump {
                key,
                current,
                value,
            } => f.write_str(&tr!(
                "ruida.settings.step_jump",
                key = key,
                current = current,
                value = value
            )),
            SettingsError::BadFile(m) => f.write_str(&tr!("ruida.settings.bad_file", why = m)),
            SettingsError::CardMismatch { file, machine } => f.write_str(&tr!(
                "ruida.settings.card_mismatch",
                file = format!("{file:08X}"),
                machine = format!("{machine:08X}")
            )),
            SettingsError::BadMeasure(which) => f.write_str(&match *which {
                "commanded" => tr!("ruida.settings.bad_commanded"),
                "current" => tr!("ruida.settings.bad_current"),
                _ => tr!("ruida.settings.bad_measured"),
            }),
            SettingsError::ImplausibleRatio { ratio } => f.write_str(&tr!(
                "ruida.settings.implausible_ratio",
                pct = format!("{:.1}", (ratio - 1.0) * 100.0)
            )),
            SettingsError::BelowResolution { current, ratio } => f.write_str(&tr!(
                "ruida.settings.below_resolution",
                current = current,
                pct = format!("{:.3}", (ratio - 1.0) * 100.0)
            )),
        }
    }
}

impl std::error::Error for SettingsError {}

#[cfg(test)]
mod tests;
