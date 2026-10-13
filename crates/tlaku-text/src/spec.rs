//! Descripción del texto a convertir; refleja `TextObject` del contrato salvo `outline`.

use serde::{Deserialize, Serialize};

/// Alineación horizontal de cada línea.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// Colocación sobre una trayectoria (que se entrega aparte, como `BezPath`).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct OnPathSpec {
    /// Distancia a lo largo de la trayectoria donde empieza el texto (mm).
    pub offset_mm: f64,
    /// Recorre la trayectoria al revés: el texto empieza por su final y queda en el otro lado.
    pub flip: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextSpec {
    /// Admite varias líneas separadas por `\n`.
    pub text: String,
    pub font_family: String,
    pub bold: bool,
    pub italic: bool,
    /// Tamaño del em en mm (como en tipografía), no el de la mayúscula.
    pub size_mm: f64,
    /// Espacio extra entre caracteres (mm); negativo los acerca.
    pub letter_spacing_mm: f64,
    /// Distancia entre líneas base, en múltiplos de `size_mm`.
    pub line_spacing: f64,
    pub align: Align,
    pub on_path: Option<OnPathSpec>,
}

impl Default for TextSpec {
    fn default() -> Self {
        Self {
            text: String::new(),
            font_family: String::new(),
            bold: false,
            italic: false,
            size_mm: 10.0,
            letter_spacing_mm: 0.0,
            line_spacing: 1.2,
            align: Align::Left,
            on_path: None,
        }
    }
}
