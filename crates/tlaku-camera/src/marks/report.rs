//! Resultado de la detección y opciones.

use crate::homography::Homography;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Resultado
// ---------------------------------------------------------------------------

/// Una marca de la disposición, con lo que se encontró de ella.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkResult {
    pub index: usize,
    /// Posición teórica en la cama (mm).
    pub bed: [f64; 2],
    /// Centro detectado en la imagen (px); `None` si no se encontró.
    pub image: Option<[f64; 2]>,
    /// Calidad de la detección, de 0 a 1 (simetría de la cruz).
    pub confidence: f64,
    /// Participa en el ajuste final (no fue descartada por incoherente).
    pub used: bool,
    /// Distancia en la cama (mm) entre la marca proyectada con el ajuste y su posición.
    pub residual_mm: Option<f64>,
    /// Error de validación cruzada (mm): se ajusta sin esta marca y se mide aquí.
    pub loo_mm: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarksReport {
    pub marks: Vec<MarkResult>,
    /// Todos los candidatos con forma de cruz que vio el detector (px), para dibujarlos.
    pub candidates: Vec<[f64; 2]>,
    /// Homografía imagen a cama, por filas.
    pub to_bed: [[f64; 3]; 3],
    pub expected: usize,
    pub found: usize,
    pub used: usize,
    /// Error del ajuste (rms, mm) sobre las marcas usadas.
    pub rms_mm: f64,
    pub max_residual_mm: f64,
    /// Validación cruzada (dejar una fuera): rms y máximo en mm. `None` con menos de 5 marcas
    /// usadas: el ajuste es exacto y no hay con qué medir.
    pub loo_rms_mm: Option<f64>,
    pub loo_max_mm: Option<f64>,
    /// Escala de la imagen en el centro de la disposición (px por mm).
    pub scale_px_per_mm: f64,
    pub used_prior: bool,
    pub warnings: Vec<String>,
}

/// Opciones de la detección.
#[derive(Debug, Clone)]
pub struct DetectOptions {
    /// Alineación provisional imagen a cama (p. ej. la manual de 4 puntos), en las mismas
    /// coordenadas de píxel que se entregan. Con ella se busca cerca de lo previsto.
    pub prior_to_bed: Option<Homography>,
    /// Desplazamiento entre el origen de las coordenadas de entrada/salida y el centro del
    /// píxel: 0.0 si `(i, j)` es el centro del píxel, 0.5 si es su esquina.
    pub pixel_offset: f64,
    /// Umbral de RANSAC, en mm de cama.
    pub ransac_threshold_mm: f64,
}

impl Default for DetectOptions {
    fn default() -> Self {
        Self {
            prior_to_bed: None,
            pixel_offset: 0.0,
            ransac_threshold_mm: 1.5,
        }
    }
}
