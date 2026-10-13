use serde::{Deserialize, Serialize};
use tlaku_core::bezpath::BezPath;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/// Texto que sigue la forma de otro objeto del documento.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct TextOnPath {
    /// Id del objeto cuya primera trayectoria guía el texto.
    pub object_id: String,
    /// Distancia (mm) desde el inicio de la trayectoria.
    pub offset_mm: f64,
    /// Escribe por el otro lado de la trayectoria.
    pub flip: bool,
}

/// Objeto de texto editable. El motor de texto genera `outline` (curvas en coordenadas
/// locales, mm); el pipeline corta/graba ese contorno y no vuelve a interpretar la fuente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct TextObject {
    /// Admite varias líneas.
    pub text: String,
    pub font_family: String,
    pub bold: bool,
    pub italic: bool,
    /// Tamaño del em (como en tipografía), no de la mayúscula.
    pub size_mm: f64,
    pub letter_spacing_mm: f64,
    /// Interlineado en múltiplos del tamaño.
    pub line_spacing: f64,
    pub align: TextAlign,
    pub on_path: Option<TextOnPath>,
    /// Texto curvado (Bend): grados que abarca la línea más larga sobre un arco. Positivo
    /// arquea hacia arriba, negativo hacia abajo; ausente o 0 deja el texto recto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub bend_deg: Option<f64>,
    /// Texto soldado (Welded): une en un solo contorno las letras que se tocan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub welded: Option<bool>,
    /// Contorno ya generado; vacío si todavía no se generó.
    pub outline: Vec<BezPath>,
}
