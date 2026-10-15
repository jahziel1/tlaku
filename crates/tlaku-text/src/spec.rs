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

/// Alineación vertical del bloque de texto respecto del origen (o del trayecto).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VAlign {
    /// La línea base de la primera línea queda en el origen (comportamiento de siempre).
    #[default]
    Baseline,
    /// La parte alta de la mayúscula de la primera línea queda en el origen.
    Top,
    /// El origen queda a media altura entre la mayúscula de la primera línea y la línea base
    /// de la última.
    Middle,
    /// La línea base de la última línea queda en el origen.
    Bottom,
}

/// Función OpenType (`liga`, `ss01`, `smcp`, `kern`...) con su valor: 0 la apaga, 1 la
/// enciende (algunas, como `salt`, aceptan el número de la alternativa).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontFeature {
    /// Etiqueta de cuatro caracteres ASCII.
    pub tag: String,
    pub value: u32,
}

/// Valor de un eje de una fuente variable (`wght`, `wdth`, `slnt`, `opsz` o personalizado).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontAxis {
    /// Etiqueta de cuatro caracteres ASCII.
    pub tag: String,
    pub value: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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
    /// Texto curvado: ángulo (grados) que abarca la línea más larga sobre un arco. Positivo
    /// arquea el texto hacia arriba (centro del arco debajo), negativo hacia abajo, 0 lo
    /// deja recto. No se combina con `on_path` (que manda). Máximo ±360.
    pub bend_deg: f64,
    /// Une en un solo contorno las letras que se tocan o se solapan (fuentes cursivas).
    /// No aplica a las fuentes de una línea.
    pub welded: bool,
    /// Funciones OpenType que se fuerzan (encendidas o apagadas) sobre las de la fuente. Las
    /// que no aparecen siguen el valor por omisión de HarfBuzz. Se aplican a todo el texto.
    pub features: Vec<FontFeature>,
    /// Posición en los ejes de una fuente variable; los ejes no citados quedan en su valor
    /// por omisión y los valores fuera de rango se acotan al eje.
    pub variations: Vec<FontAxis>,
    /// Pasa el texto a mayúsculas antes de darle forma.
    pub uppercase: bool,
    /// Alineación vertical del bloque.
    pub valign: VAlign,
    /// Espacio extra (mm) que se suma a cada espacio entre palabras; negativo lo acerca.
    pub word_spacing_mm: f64,
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
            bend_deg: 0.0,
            welded: false,
            features: Vec::new(),
            variations: Vec::new(),
            uppercase: false,
            valign: VAlign::Baseline,
            word_spacing_mm: 0.0,
        }
    }
}
