//! Funciones OpenType y ejes de fuentes variables: qué trae cada fuente y cómo se pasan a
//! HarfBuzz.
//!
//! - [`font_capabilities`] lee las tablas GSUB/GPOS y `fvar` de la cara elegida y devuelve solo
//!   lo que la fuente realmente declara (para que la interfaz no ofrezca lo que no existe), con
//!   una vista previa de cada función encendida y apagada.
//! - [`selected_features`] y [`selected_variations`] convierten lo guardado en el objeto de
//!   texto a los tipos de `rustybuzz`.

use crate::layout::outline;
use crate::library::FontLibrary;
use crate::spec::{FontAxis, FontFeature, TextSpec};
use rustybuzz::{Feature, Variation};
use serde::Serialize;
use std::collections::BTreeSet;
use tlaku_core::bezpath::BezPath;
use tlaku_i18n::tr;
use ttf_parser::Tag;

/// Tamaño (mm) de las muestras de vista previa.
const PREVIEW_SIZE_MM: f64 = 10.0;
/// Funciones que HarfBuzz enciende sin que nadie lo pida.
const DEFAULT_ON: [&str; 4] = ["liga", "clig", "calt", "kern"];
/// Funciones de uso directo (el resto de GSUB/GPOS —`ccmp`, `locl`, marcas...— las aplica el
/// motor solo y no se ofrecen). Además de `ssNN` y `cvNN`.
const USER_FEATURES: [&str; 20] = [
    "liga", "clig", "dlig", "calt", "kern", "salt", "swsh", "cswh", "smcp", "c2sc", "pcap", "c2pc",
    "lnum", "onum", "tnum", "pnum", "frac", "zero", "case", "sups",
];
const USER_FEATURES_MORE: [&str; 3] = ["subs", "ordn", "afrc"];
/// Muestras de varias letras que suelen cambiar con ligaduras, kerning o fracciones.
const MULTI_SAMPLES: [&str; 26] = [
    "fi", "fl", "ff", "ffi", "ffl", "fj", "ft", "Th", "st", "ct", "tt", "ab", "AB", "AV", "AT",
    "To", "Wa", "Ye", "LT", "Va", "1/2", "3/4", "1/4", "ae", "oe", "AE",
];
/// Caracteres que se prueban uno a uno para armar la muestra de una función.
const SINGLE_SAMPLES: &str = "aAbBcCdDeEfFgGhHiIjJkKlLmMnNoOpPqQrRsStTuUvVwWxXyYzZ0123456789&@?";
/// Muestra de reserva cuando la función no cambia nada en las anteriores.
const FALLBACK_SAMPLE: &str = "Aa Rg 123";
/// Máximo de fragmentos en una muestra.
const MAX_SAMPLE_PARTS: usize = 5;

/// Etiqueta de cuatro caracteres ASCII imprimibles.
pub(crate) fn parse_tag(s: &str) -> Option<Tag> {
    let b = s.as_bytes();
    (b.len() == 4 && b.iter().all(|c| (0x20..=0x7e).contains(c)))
        .then(|| Tag::from_bytes(&[b[0], b[1], b[2], b[3]]))
}

/// Las funciones del objeto en el tipo de `rustybuzz` (las de etiqueta inválida se descartan;
/// `outline` ya las rechazó antes).
pub(crate) fn selected_features(features: &[FontFeature]) -> Vec<Feature> {
    features
        .iter()
        .filter_map(|f| Some(Feature::new(parse_tag(&f.tag)?, f.value, ..)))
        .collect()
}

pub(crate) fn selected_variations(axes: &[FontAxis]) -> Vec<Variation> {
    axes.iter()
        .filter_map(|a| {
            Some(Variation {
                tag: parse_tag(&a.tag)?,
                value: a.value,
            })
        })
        .collect()
}

/// Etiquetas de todas las funciones que declaran GSUB y GPOS.
pub(crate) fn layout_tags(face: &ttf_parser::Face) -> BTreeSet<[u8; 4]> {
    let mut out = BTreeSet::new();
    for table in [face.tables().gsub, face.tables().gpos]
        .into_iter()
        .flatten()
    {
        for i in 0..table.features.len() {
            if let Some(f) = table.features.get(i) {
                out.insert(f.tag.to_bytes());
            }
        }
    }
    out
}

fn is_user_feature(tag: &str) -> bool {
    if USER_FEATURES.contains(&tag) || USER_FEATURES_MORE.contains(&tag) {
        return true;
    }
    let numbered = |prefix: &str, max: u32| {
        tag.strip_prefix(prefix)
            .filter(|n| n.len() == 2 && n.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|n| n.parse::<u32>().ok())
            .is_some_and(|n| (1..=max).contains(&n))
    };
    numbered("ss", 20) || numbered("cv", 99)
}

/// Una función OpenType que la fuente declara.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FeatureInfo {
    pub tag: String,
    /// Encendida sin que nadie lo pida (`liga`, `kern`...).
    pub default_on: bool,
    /// Texto de la vista previa: letras que la función cambia en esta fuente.
    pub sample: String,
    /// ¿La muestra sale distinta con la función encendida y apagada? Si no, la función existe
    /// pero ninguna letra de las probadas la usa.
    pub changes: bool,
    /// Muestra con la función encendida (curvas en mm, tamaño 10 mm, origen en la línea base).
    pub on: Vec<BezPath>,
    /// Muestra con la función apagada.
    pub off: Vec<BezPath>,
}

/// Eje de una fuente variable.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AxisInfo {
    pub tag: String,
    /// Nombre que da la fuente al eje, si lo trae.
    pub name: Option<String>,
    pub min: f32,
    pub default: f32,
    pub max: f32,
}

/// Instancia con nombre de una fuente variable (por ejemplo «Bold Condensed»).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InstanceInfo {
    pub name: String,
    pub coords: Vec<FontAxis>,
}

/// Lo que ofrece una cara: funciones y ejes. Todo vacío para las fuentes de una línea.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct FontCapabilities {
    pub features: Vec<FeatureInfo>,
    pub axes: Vec<AxisInfo>,
    pub instances: Vec<InstanceInfo>,
}

/// Función hallada en la fuente, antes de generar su vista previa.
struct Probe {
    tag: String,
    default_on: bool,
    sample: String,
    changes: bool,
}

type Signature = Vec<(u32, i32, i32, i32)>;

fn signature(face: &rustybuzz::Face, text: &str, features: &[Feature]) -> Signature {
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    let shaped = rustybuzz::shape(face, features, buffer);
    shaped
        .glyph_infos()
        .iter()
        .zip(shaped.glyph_positions())
        .map(|(i, p)| (i.glyph_id, p.x_advance, p.x_offset, p.y_offset))
        .collect()
}

/// Busca letras que cambian al encender/apagar `tag`; devuelve la muestra y si hubo cambio.
fn find_sample(face: &rustybuzz::Face, tag: Tag) -> (String, bool) {
    let on = [Feature::new(tag, 1, ..)];
    let off = [Feature::new(tag, 0, ..)];
    let changes = |text: &str| signature(face, text, &on) != signature(face, text, &off);
    let mut parts: Vec<String> = Vec::new();
    for s in MULTI_SAMPLES {
        if parts.len() >= 3 {
            break;
        }
        if changes(s) {
            parts.push(s.to_string());
        }
    }
    for c in SINGLE_SAMPLES.chars() {
        if parts.len() >= MAX_SAMPLE_PARTS {
            break;
        }
        let s = c.to_string();
        if changes(&s) && !parts.iter().any(|p| p.contains(c)) {
            parts.push(s);
        }
    }
    if parts.is_empty() {
        (FALLBACK_SAMPLE.to_string(), false)
    } else {
        (parts.join(" "), true)
    }
}

fn name_string(face: &ttf_parser::Face, id: u16) -> Option<String> {
    let names = face.names();
    let english = names
        .into_iter()
        .filter(|n| n.name_id == id && n.language_id == 0x409)
        .find_map(|n| n.to_string());
    english.or_else(|| {
        names
            .into_iter()
            .filter(|n| n.name_id == id)
            .find_map(|n| n.to_string())
    })
}

fn be16(d: &[u8], at: usize) -> Option<usize> {
    Some(usize::from(u16::from_be_bytes([
        *d.get(at)?,
        *d.get(at + 1)?,
    ])))
}

/// Instancias con nombre de la tabla `fvar` (`ttf-parser` no las expone).
fn read_instances(
    face: &ttf_parser::Face,
    axes: &[ttf_parser::VariationAxis],
) -> Vec<InstanceInfo> {
    let Some(data) = face.raw_face().table(Tag::from_bytes(b"fvar")) else {
        return Vec::new();
    };
    let (Some(axes_at), Some(axis_count), Some(axis_size), Some(count), Some(size)) = (
        be16(data, 4),
        be16(data, 8),
        be16(data, 10),
        be16(data, 12),
        be16(data, 14),
    ) else {
        return Vec::new();
    };
    if axis_count != axes.len() {
        return Vec::new();
    }
    let first = axes_at + axis_count * axis_size;
    let mut out = Vec::new();
    for i in 0..count {
        let at = first + i * size;
        let Some(name_id) = be16(data, at) else { break };
        let mut coords = Vec::new();
        for (k, axis) in axes.iter().enumerate() {
            let off = at + 4 + k * 4;
            let Some(bytes) = data.get(off..off + 4) else {
                break;
            };
            let fixed = i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            coords.push(FontAxis {
                tag: axis_tag(axis),
                value: fixed as f32 / 65536.0,
            });
        }
        if coords.len() != axes.len() {
            break;
        }
        let name = name_string(face, name_id as u16).unwrap_or_else(|| format!("#{}", i + 1));
        out.push(InstanceInfo { name, coords });
    }
    out
}

fn axis_tag(axis: &ttf_parser::VariationAxis) -> String {
    String::from_utf8_lossy(&axis.tag.to_bytes()).into_owned()
}

/// Funciones y ejes de la cara que `spec` elige (familia, negrita, cursiva). Las vistas previas
/// usan la posición de ejes de `spec` para que se vean como saldrá el texto.
pub fn font_capabilities(lib: &FontLibrary, spec: &TextSpec) -> Result<FontCapabilities, String> {
    if lib.line_font(&spec.font_family).is_some() {
        return Ok(FontCapabilities::default());
    }
    let choice = lib.resolve(&spec.font_family, spec.bold, spec.italic)?;
    let variations = selected_variations(&spec.variations);
    let found = lib
        .with_face_data(choice.id, |data, index| {
            let mut face = rustybuzz::Face::from_slice(data, index)?;
            let axes_raw: Vec<ttf_parser::VariationAxis> =
                face.variation_axes().into_iter().collect();
            let instances = read_instances(&face, &axes_raw);
            if face.is_variable() {
                face.set_variations(&variations);
            }
            let axes: Vec<AxisInfo> = axes_raw
                .iter()
                .filter(|a| !a.hidden)
                .map(|a| AxisInfo {
                    tag: axis_tag(a),
                    name: name_string(&face, a.name_id),
                    min: a.min_value,
                    default: a.def_value,
                    max: a.max_value,
                })
                .collect();
            let mut probes = Vec::new();
            for bytes in layout_tags(&face) {
                let tag_text = String::from_utf8_lossy(&bytes).into_owned();
                if !is_user_feature(&tag_text) {
                    continue;
                }
                let (sample, changes) = find_sample(&face, Tag::from_bytes(&bytes));
                probes.push(Probe {
                    default_on: DEFAULT_ON.contains(&tag_text.as_str()),
                    tag: tag_text,
                    sample,
                    changes,
                });
            }
            Some((probes, axes, instances))
        })
        .flatten()
        .ok_or_else(|| tr!("text.layout.face_damaged"))?;
    let (probes, axes, instances) = found;

    let sample_paths = |text: &str, tag: &str, value: u32| -> Vec<BezPath> {
        let s = TextSpec {
            text: text.to_string(),
            font_family: spec.font_family.clone(),
            bold: spec.bold,
            italic: spec.italic,
            size_mm: PREVIEW_SIZE_MM,
            variations: spec.variations.clone(),
            features: vec![FontFeature {
                tag: tag.to_string(),
                value,
            }],
            ..TextSpec::default()
        };
        outline(lib, &s, None).map(|o| o.paths).unwrap_or_default()
    };
    let features = probes
        .into_iter()
        .map(|p| FeatureInfo {
            on: sample_paths(&p.sample, &p.tag, 1),
            off: sample_paths(&p.sample, &p.tag, 0),
            tag: p.tag,
            default_on: p.default_on,
            sample: p.sample,
            changes: p.changes,
        })
        .collect();
    Ok(FontCapabilities {
        features,
        axes,
        instances,
    })
}
