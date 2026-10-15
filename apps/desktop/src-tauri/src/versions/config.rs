//! Configuración del historial de versiones (`versions.json` en la carpeta de configuración).
//!
//! Vive en Rust, no en las preferencias de la interfaz, porque el guardado y la poda la necesitan sin
//! esperar a la ventana. Los valores se acotan al leer y al escribir: un archivo editado a mano no
//! puede dejar la política en algo absurdo.

use super::retention::Policy;
use super::store::write_atomic_bytes;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use tlaku_i18n::tr;

const FILE: &str = "versions.json";

/// Dónde se guardan las versiones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Location {
    /// En la carpeta de datos de la app, una subcarpeta por proyecto (por omisión: no ensucia la
    /// carpeta del usuario ni viaja con ella a una nube o a un correo).
    App,
    /// Junto al archivo, en la carpeta oculta `.tlaku-versions` (viaja con el proyecto).
    Beside,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VersionsConfig {
    /// Apagado: no se guarda la versión anterior en cada guardado ni instantáneas automáticas
    /// (las manuales con nombre siguen funcionando: las pide el usuario).
    pub enabled: bool,
    pub location: Location,
    /// Cada cuántos minutos una instantánea automática si hay cambios; 0 = sin instantáneas.
    pub auto_minutes: u32,
    pub recent_hours: u32,
    pub daily_days: u32,
    pub weekly_weeks: u32,
    /// Tope de lo que ocupa el historial de UN proyecto, en MB.
    pub max_mb: u32,
    /// Tope de versiones de un proyecto.
    pub max_versions: u32,
}

impl Default for VersionsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            location: Location::App,
            auto_minutes: 5,
            recent_hours: 24,
            daily_days: 30,
            weekly_weeks: 26,
            max_mb: 200,
            max_versions: 500,
        }
    }
}

impl VersionsConfig {
    /// La misma configuración con cada valor dentro de su rango.
    pub fn clamped(mut self) -> Self {
        self.auto_minutes = self.auto_minutes.min(240);
        self.recent_hours = self.recent_hours.clamp(1, 24 * 30);
        self.daily_days = self.daily_days.min(365);
        self.weekly_weeks = self.weekly_weeks.min(520);
        self.max_mb = self.max_mb.clamp(1, 10_240);
        self.max_versions = self.max_versions.clamp(10, 5_000);
        self
    }

    pub fn policy(&self) -> Policy {
        Policy {
            recent_hours: self.recent_hours,
            daily_days: self.daily_days,
            weekly_weeks: self.weekly_weeks,
            max_bytes: u64::from(self.max_mb) * 1024 * 1024,
            max_versions: self.max_versions as usize,
        }
    }
}

/// Lee la configuración; sin archivo, o si está dañado, la de fábrica (el archivo dañado no se toca).
pub fn load(dir: &Path) -> VersionsConfig {
    fs::read_to_string(dir.join(FILE))
        .ok()
        .and_then(|text| serde_json::from_str::<VersionsConfig>(&text).ok())
        .unwrap_or_default()
        .clamped()
}

pub fn save(dir: &Path, config: &VersionsConfig) -> Result<VersionsConfig, String> {
    let config = config.clone().clamped();
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let text = serde_json::to_vec_pretty(&config)
        .map_err(|e| tr!("desktop.versions.meta_failed", err = e))?;
    write_atomic_bytes(&dir.join(FILE), &text)?;
    Ok(config)
}
