//! Opciones, informe y puntuaciones de la calibración.

use super::{LensCalibration, LensModel};
use serde::{Deserialize, Serialize};

/// Qué modelo de lente ajustar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LensModelChoice {
    /// Ajusta los dos y elige por error de reproyección y validación cruzada.
    #[default]
    Auto,
    BrownConrady,
    Fisheye,
}

/// Opciones del ajuste.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalibrationOptions {
    /// Modelo de lente: uno fijo o la elección automática.
    pub model: LensModelChoice,
    /// Ajusta la distorsión tangencial (p1, p2).
    pub fit_tangential: bool,
    /// Ajusta k3. Solo conviene con distorsión fuerte y buena cobertura del campo;
    /// si no, extrapola mal fuera de donde se vio el tablero.
    pub fit_k3: bool,
    pub max_iterations: usize,
}

impl Default for CalibrationOptions {
    fn default() -> Self {
        Self {
            model: LensModelChoice::Auto,
            fit_tangential: true,
            fit_k3: false,
            max_iterations: 150,
        }
    }
}

/// Resultado con diagnóstico.
#[derive(Debug, Clone, Serialize)]
pub struct CalibrationReport {
    pub calibration: LensCalibration,
    /// Avisos para el usuario (vistas descartadas, pocas vistas, error alto...).
    pub warnings: Vec<String>,
    /// Índices (de la lista de entrada) de las vistas usadas.
    pub used_views: Vec<usize>,
    /// Error rms en píxeles de cada vista usada (mismo orden que `used_views`).
    pub view_rms_px: Vec<f64>,
    /// Puntaje de cada modelo probado (uno solo si se fijó el modelo).
    pub model_scores: Vec<ModelScore>,
}

/// Cómo le fue a un modelo de lente con los mismos datos.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelScore {
    pub model: LensModel,
    /// Error de reproyección rms con todas las vistas usadas (px).
    pub rms_px: f64,
    /// Error rms (px) en las vistas reservadas: se ajusta sin ellas y se mide en ellas con la
    /// pose recalculada. `None` si hay muy pocas vistas.
    pub cv_rms_px: Option<f64>,
    /// Es el modelo de la calibración devuelta.
    pub chosen: bool,
}
