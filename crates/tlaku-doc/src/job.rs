use crate::cut::Optimization;
use crate::document::DesignObject;
use crate::layer::LayerSettings;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum StartFrom {
    /// Coordenadas del documento tal cual.
    Absolute,
    /// El trabajo se coloca relativo a la posición actual de la cabeza.
    CurrentPosition,
    /// El trabajo se coloca relativo a `user_origin`.
    UserOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum LayerOrder {
    /// Grabados primero, líneas después y cortes al final.
    Auto,
    /// El orden de la lista de capas del documento.
    File,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct JobOptions {
    pub start_from: StartFrom,
    /// Punto del rectángulo envolvente que coincide con el origen, 0..8 fila por fila desde
    /// arriba a la izquierda (0 = arriba-izquierda, 4 = centro, 6 = abajo-izquierda).
    /// Solo cuenta con `current_position` y `user_origin`.
    pub job_origin: u32,
    pub user_origin: Option<[f64; 2]>,
    /// Ids de los objetos a enviar; `null` = todo lo que tenga salida.
    pub selection: Option<Vec<String>>,
    pub layer_order: LayerOrder,
    #[serde(default)]
    pub optimization: Optimization,
    /// Con `selection`: el origen del trabajo es la caja de la selección, no la de todo el
    /// diseño.
    #[serde(default)]
    pub use_selection_origin: bool,
}

impl Default for JobOptions {
    fn default() -> Self {
        Self {
            start_from: StartFrom::Absolute,
            job_origin: 6,
            user_origin: None,
            selection: None,
            layer_order: LayerOrder::Auto,
            optimization: Optimization::default(),
            use_selection_origin: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct PreviewLayer {
    #[ts(type = "string")]
    pub color: tlaku_core::Rgb,
    pub mode: String,
    pub seconds: f64,
    pub summary: String,
    /// Ahorro de recorrido en vacío respecto al orden original (solo capas line).
    pub travel_saved_pct: Option<f64>,
    pub explanations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct JobPreview {
    /// Una entrada por bloque, en el orden de ejecución.
    pub layers: Vec<PreviewLayer>,
    /// `[kind, x, y, power]` por movimiento; kind 0 = travel, 1 = cut (potencia 0 = overscan).
    pub moves: Vec<f32>,
    /// Segundos acumulados al terminar cada movimiento (uno por entrada de `moves`, con el
    /// mismo modelo de aceleración que `total_s`); el último vale `total_s`.
    pub move_times: Vec<f64>,
    pub total_s: f64,
    /// Esquinas mínima y máxima, con overscan.
    pub bounds: [[f64; 2]; 2],
    pub order_note: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct ImportResult {
    pub objects: Vec<DesignObject>,
    pub new_layers: Vec<LayerSettings>,
    pub warnings: Vec<String>,
}
