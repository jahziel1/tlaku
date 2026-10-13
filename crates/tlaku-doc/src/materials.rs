//! Biblioteca de materiales y prueba de materiales.

use crate::layer::{LayerMode, LayerSettings};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Versión del archivo de biblioteca (`.tlakumat`, JSON).
pub const MATERIAL_LIBRARY_VERSION: u32 = 1;

/// Ajustes probados para un material y una operación.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct MaterialPreset {
    /// Único en la biblioteca.
    pub id: String,
    /// Por ejemplo "MDF", "Contrachapado de abedul", "Acrílico negro".
    pub material: String,
    /// Grosor en mm; `null` = no aplica (grabados).
    pub thickness: Option<f64>,
    /// Operación, por ejemplo "Corte", "Grabado ligero".
    pub title: String,
    pub notes: String,
    /// Máquina para la que se probó; `null` = cualquiera.
    pub device_id: Option<String>,
    /// Al aplicarse a una capa se copian todos los campos menos `color`, `name`, `output`,
    /// `visible` y `order`.
    pub settings: LayerSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct MaterialLibrary {
    #[ts(type = "1")]
    pub version: u32,
    pub presets: Vec<MaterialPreset>,
}

impl Default for MaterialLibrary {
    fn default() -> Self {
        Self {
            version: MATERIAL_LIBRARY_VERSION,
            presets: Vec::new(),
        }
    }
}

/// Parámetro que varía a lo largo de filas o columnas en la prueba de materiales.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TestParam {
    /// mm/min.
    Speed,
    /// %.
    Power,
    Passes,
    /// mm (solo fill e image).
    Interval,
}

/// Eje de la cuadrícula de prueba: qué parámetro varía y entre qué valores (inclusive).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct TestAxis {
    pub param: TestParam,
    pub count: u32,
    pub from: f64,
    pub to: f64,
}

/// Cuadrícula de prueba: cada celda es una capa con sus propios ajustes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct MaterialTestParams {
    /// Line (cuadros) o fill (cuadros rellenos).
    pub mode: LayerMode,
    /// Varía de una fila a otra (de arriba hacia abajo).
    pub rows: TestAxis,
    /// Varía de una columna a otra (de izquierda a derecha).
    pub cols: TestAxis,
    /// Ancho y alto de cada celda en mm.
    pub cell: [f64; 2],
    /// Separación entre celdas en mm.
    pub gap: f64,
    /// Ajustes base de todas las celdas (lo que no varía).
    pub base: LayerSettings,
    /// Rotular valores de filas y columnas con trazo sencillo.
    pub labels: bool,
    /// Velocidad (mm/min) y potencia (%) de los rótulos.
    pub label_speed: f64,
    pub label_power: f64,
    /// Esquina inferior izquierda de la cuadrícula (con rótulos), en mm del documento.
    pub origin: [f64; 2],
}
