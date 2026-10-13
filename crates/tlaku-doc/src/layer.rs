use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use tlaku_core::Rgb;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum LayerMode {
    /// Sigue los contornos.
    Line,
    /// Relleno por barrido de las trayectorias cerradas.
    Fill,
    /// Grabado de imágenes.
    Image,
}

impl LayerMode {
    pub fn name(self) -> &'static str {
        match self {
            LayerMode::Line => "line",
            LayerMode::Fill => "fill",
            LayerMode::Image => "image",
        }
    }
}

/// Overscan de rellenos e imágenes: `"auto"` (según velocidad y aceleración) o mm.
#[derive(Debug, Clone, Copy, PartialEq, TS)]
#[ts(export, type = "\"auto\" | number")]
pub enum Overscan {
    Auto,
    Mm(f64),
}

impl Serialize for Overscan {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Overscan::Auto => serializer.serialize_str("auto"),
            Overscan::Mm(v) => serializer.serialize_f64(*v),
        }
    }
}

impl<'de> Deserialize<'de> for Overscan {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = Overscan;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("\"auto\" o un número en mm")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Overscan, E> {
                if v.eq_ignore_ascii_case("auto") {
                    Ok(Overscan::Auto)
                } else {
                    Err(E::custom(format!(
                        "overscan inválido '{v}': usa \"auto\" o un número en mm"
                    )))
                }
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Overscan, E> {
                Ok(Overscan::Mm(v))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Overscan, E> {
                Ok(Overscan::Mm(v as f64))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Overscan, E> {
                Ok(Overscan::Mm(v as f64))
            }
        }
        deserializer.deserialize_any(V)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ImageMode {
    Threshold,
    Floyd,
    Jarvis,
    Stucki,
    Atkinson,
    Bayer4,
    Grayscale,
    /// Puntos de tamaño variable en una rejilla girada (`halftone_cell`, `halftone_angle`).
    Halftone,
    /// Líneas paralelas de grosor variable (como en un periódico).
    Newsprint,
    /// Solo los bordes de la imagen, como un dibujo a lápiz.
    Sketch,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct ImageSettings {
    pub mode: ImageMode,
    /// Umbral 0-1; solo se usa con `mode = threshold`.
    pub threshold: f64,
    /// -1 a 1.
    pub brightness: f64,
    /// -1 a 1.
    pub contrast: f64,
    /// 1 = sin cambio.
    pub gamma: f64,
    pub invert: bool,
    /// Ancho del punto del haz en mm; acorta cada tramo encendido.
    pub dot_width: f64,
    /// Tamaño de la celda en mm (halftone y newsprint).
    #[serde(default = "default_halftone_cell")]
    pub halftone_cell: f64,
    /// Ángulo de la rejilla en grados (halftone y newsprint).
    #[serde(default = "default_halftone_angle")]
    pub halftone_angle: f64,
}

fn default_halftone_cell() -> f64 {
    0.5
}

fn default_halftone_angle() -> f64 {
    45.0
}

impl Default for ImageSettings {
    fn default() -> Self {
        Self {
            mode: ImageMode::Stucki,
            threshold: 0.5,
            brightness: 0.0,
            contrast: 0.0,
            gamma: 1.0,
            invert: false,
            dot_width: 0.0,
            halftone_cell: default_halftone_cell(),
            halftone_angle: default_halftone_angle(),
        }
    }
}

/// Cómo se rellena una forma cerrada en modo fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum FillStyle {
    /// Líneas paralelas de barrido (`interval`, `angle`).
    #[default]
    Scan,
    /// Contornos concéntricos hacia dentro, separados `interval`: sin aceleraciones de
    /// barrido, mejor para formas delgadas o letras.
    Offset,
}

/// Puentes sin cortar que sostienen la pieza en la placa (solo capas line).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct TabSettings {
    pub enabled: bool,
    /// Largo de cada puente, en mm.
    pub width: f64,
    /// Puentes por forma cerrada si `spacing` es 0; si no, el mínimo por forma.
    pub count: u32,
    /// Distancia en mm entre puentes a lo largo del contorno; 0 = usar `count`.
    pub spacing: f64,
    /// No poner puentes en las formas que están dentro de otra (huecos).
    pub skip_inner: bool,
}

impl Default for TabSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            width: 0.5,
            count: 4,
            spacing: 0.0,
            skip_inner: true,
        }
    }
}

/// Ajustes de una capa (un color). Cada modo usa solo parte de los campos:
/// line: `speed`, `power`, `passes`, `air`, `kerf`;
/// fill: además `interval`, `angle`, `crosshatch`, `bidirectional`, `overscan`, `fill_style`;
/// image: `speed`, `power` (máxima), `min_power`, `passes`, `air`, `interval`, `angle`
/// (ángulo de barrido), `bidirectional`, `overscan` e `image`.
/// line usa además `tabs`; todos los modos usan `z_offset` y `z_step` si la máquina tiene Z.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct LayerSettings {
    #[ts(type = "string")]
    pub color: Rgb,
    pub name: String,
    pub mode: LayerMode,
    /// Se envía a la máquina.
    pub output: bool,
    /// Se dibuja en el lienzo (no afecta a la salida).
    pub visible: bool,
    /// mm/min.
    pub speed: f64,
    /// 0-100 %; en imágenes, la potencia máxima.
    pub power: f64,
    pub min_power: f64,
    pub passes: u32,
    pub air: bool,
    /// Separación entre líneas de barrido, en mm.
    pub interval: f64,
    /// Grados, antihorario.
    pub angle: f64,
    pub crosshatch: bool,
    pub bidirectional: bool,
    pub overscan: Overscan,
    /// Solo line; positivo = la pieza sale del tamaño dibujado.
    pub kerf: f64,
    pub image: ImageSettings,
    /// Posición forzada en el orden de ejecución (desde 1); `null` = automático.
    pub order: Option<u32>,
    /// Solo fill.
    #[serde(default)]
    pub fill_style: FillStyle,
    /// Solo line.
    #[serde(default)]
    pub tabs: TabSettings,
    /// mm que baja la mesa (o sube el cabezal) respecto al foco al empezar la capa; positivo =
    /// enfoca más adentro del material. Solo con `DeviceProfile.z_axis`.
    #[serde(default)]
    pub z_offset: f64,
    /// mm adicionales por cada pasada después de la primera (cortes profundos).
    #[serde(default)]
    pub z_step: f64,
}

impl LayerSettings {
    /// Capa nueva de un color con los valores por defecto del motor (los mismos del CLI).
    pub fn for_color(color: Rgb) -> Self {
        Self {
            color,
            name: color.to_string(),
            mode: LayerMode::Line,
            output: true,
            visible: true,
            speed: 1000.0,
            power: 20.0,
            min_power: 0.0,
            passes: 1,
            air: false,
            interval: 0.1,
            angle: 0.0,
            crosshatch: false,
            bidirectional: true,
            overscan: Overscan::Auto,
            kerf: 0.0,
            image: ImageSettings::default(),
            order: None,
            fill_style: FillStyle::Scan,
            tabs: TabSettings::default(),
            z_offset: 0.0,
            z_step: 0.0,
        }
    }

    /// Color con el que `import_file` agrupa las imágenes.
    pub const IMAGE_COLOR: Rgb = Rgb(128, 128, 128);

    /// Capa de imágenes: modo image y la velocidad de grabado habitual (3000 mm/min).
    pub fn for_images() -> Self {
        Self {
            name: "Imagen".into(),
            mode: LayerMode::Image,
            speed: 3000.0,
            ..Self::for_color(Self::IMAGE_COLOR)
        }
    }
}

impl Default for LayerSettings {
    fn default() -> Self {
        Self::for_color(Rgb(0, 0, 0))
    }
}
