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

/// Alineación vertical del bloque de texto respecto del origen del objeto (o del trayecto).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TextVAlign {
    /// La línea base de la primera línea en el origen (comportamiento de siempre).
    Baseline,
    /// La mayúscula de la primera línea cuelga del origen.
    Top,
    /// El origen a media altura del bloque.
    Middle,
    /// La línea base de la última línea en el origen.
    Bottom,
}

/// Función OpenType forzada en un texto (`liga`, `ss01`, `smcp`, `kern`...). Valor 0 la apaga y
/// 1 la enciende (algunas, como `salt`, aceptan el número de la alternativa).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct TextFeature {
    /// Etiqueta OpenType de cuatro caracteres ASCII.
    pub tag: String,
    pub value: u32,
}

/// Posición de un texto en un eje de una fuente variable (`wght`, `wdth`, `slnt`, `opsz` o uno
/// propio de la fuente).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct TextAxis {
    /// Etiqueta del eje, cuatro caracteres ASCII.
    pub tag: String,
    pub value: f32,
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
    /// Funciones OpenType forzadas (ligaduras, alternativas, versalitas, numerales, kerning...).
    /// Ausente o vacío: las que trae la fuente por omisión.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub features: Option<Vec<TextFeature>>,
    /// Posición en los ejes de una fuente variable; los ejes no citados quedan en su valor
    /// por omisión.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub variations: Option<Vec<TextAxis>>,
    /// Mayúsculas forzadas: el texto se escribe tal cual pero se da forma en mayúsculas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub uppercase: Option<bool>,
    /// Alineación vertical; ausente equivale a `baseline`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub valign: Option<TextVAlign>,
    /// Espacio extra (mm) entre palabras; ausente equivale a 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub word_spacing_mm: Option<f64>,
    /// Contorno ya generado; vacío si todavía no se generó.
    pub outline: Vec<BezPath>,
}
