//! Texto de LightBurn a `TextObject`.
//!
//! Atributos de `<Shape Type="Text">`: `Font` (cadena de `QFont`), `Str` (el texto; `&#10;` es salto
//! de línea), `H` (tamaño del em en mm), `Ah`/`Av` (alineación horizontal 0/1/2 = izquierda/centro/
//! derecha y vertical 0/1/2 = arriba/centro/abajo), `LS`/`LnS` (espaciados) y, en los archivos que
//! lo guardan, `BackupPath`: el contorno ya convertido en curvas.
//!
//! El origen local de un `TextObject` es el inicio (según la alineación) de la línea base de la
//! primera línea; el de LightBurn es el punto de anclaje según `Ah`/`Av`. La distancia entre ambos
//! (`baseline_offset`) se midió en `BackupPath` de proyectos públicos con Arial (inferido; con
//! otras fuentes puede variar una fracción del tamaño).

use tlaku_core::Point;
use tlaku_core::bezpath::BezPath;
use tlaku_doc::affine::{self, Transform};
use tlaku_doc::{ObjectKind, TextAlign, TextObject};
use tlaku_lbrn::TextShape;
use tlaku_text::{Align, FontLibrary, TextSpec};

/// Fuentes de reserva (sin serif) cuando la del archivo no está instalada.
const FALLBACK_FAMILIES: [&str; 6] = [
    "Arial",
    "Helvetica",
    "Helvetica Neue",
    "DejaVu Sans",
    "Liberation Sans",
    "Noto Sans",
];
/// Interlineado por defecto del motor de texto (múltiplos del tamaño).
const LINE_SPACING: f64 = 1.2;
/// Altura de la línea base respecto al ancla, en múltiplos de `H`, con anclaje abajo/centro.
const BASELINE_BOTTOM: f64 = 0.084;
const BASELINE_MIDDLE: f64 = -0.584;
/// Con anclaje arriba (extrapolado linealmente de los dos anteriores).
const BASELINE_TOP: f64 = -1.252;

/// Resultado de convertir un texto.
pub(super) struct Converted {
    pub kind: ObjectKind,
    /// Transformación del objeto (la del texto de LightBurn desplazada a la línea base).
    pub transform: Transform,
    /// Avisos sin el texto concreto, para que el importador los cuente ("en N textos").
    pub warnings: Vec<String>,
}

/// Altura (Y local de LightBurn) de la línea base de la primera línea respecto al ancla.
fn baseline_offset(t: &TextShape, lines: usize) -> f64 {
    let h = t.height;
    let extra = (lines.saturating_sub(1)) as f64 * LINE_SPACING * h;
    match t.align_v {
        0 => BASELINE_TOP * h,
        1 => BASELINE_MIDDLE * h + extra / 2.0,
        _ => BASELINE_BOTTOM * h + extra,
    }
}

pub(super) fn convert(t: &TextShape, world: &Transform) -> Converted {
    let mut warnings = Vec::new();
    // Un salto de línea al final es una línea vacía que no se dibuja.
    let text = t
        .text
        .replace("\r\n", "\n")
        .trim_end_matches('\n')
        .to_string();
    let lines = text.split('\n').count();
    let b = baseline_offset(t, lines);
    let align = match t.align_h {
        0 => TextAlign::Left,
        2 => TextAlign::Right,
        _ => TextAlign::Center,
    };
    if t.letter_spacing != 0.0 || t.line_spacing != 0.0 {
        warnings.push(
            "el espaciado de letras/líneas de LightBurn (LS/LnS) no se importa (sus unidades no \
             están documentadas); se usa el espaciado normal"
                .into(),
        );
    }

    let mut outline: Vec<BezPath> = Vec::new();
    if let Some(saved) = &t.outline {
        // El contorno guardado está en el sistema local del texto (origen = ancla); se pasa al
        // de Tlaku (origen = línea base).
        outline.push(saved.map_points(|p| Point::new(p.x, p.y - b)));
    } else {
        let spec = TextSpec {
            text: text.clone(),
            font_family: t.family.clone(),
            bold: t.bold,
            italic: t.italic,
            size_mm: t.height,
            letter_spacing_mm: 0.0,
            line_spacing: LINE_SPACING,
            align: match align {
                TextAlign::Left => Align::Left,
                TextAlign::Center => Align::Center,
                TextAlign::Right => Align::Right,
            },
            on_path: None,
            ..TextSpec::default()
        };
        // Sin contorno guardado se genera con las fuentes del sistema (se cargan la primera vez).
        // Si la fuente del archivo no está instalada se prueban otras sin serif habituales.
        let lib = FontLibrary::system_shared();
        let mut generated = None;
        for family in std::iter::once(t.family.as_str()).chain(FALLBACK_FAMILIES) {
            let attempt = TextSpec {
                font_family: family.to_string(),
                ..spec.clone()
            };
            if let Ok(out) = tlaku_text::outline(lib, &attempt, None)
                && !out.paths.is_empty()
            {
                if family != t.family {
                    warnings.push(format!(
                        "la fuente «{}» no está instalada; el contorno se generó con «{family}»",
                        t.family
                    ));
                }
                generated = Some(out);
                break;
            }
        }
        match generated {
            Some(out) => {
                outline = out.paths;
                warnings.extend(out.warnings);
            }
            None => warnings.push(format!(
                "el archivo no trae el contorno y no se pudo generar con las fuentes instaladas \
                 (¿falta «{}»?); queda sin contorno hasta regenerarlo en el editor",
                t.family
            )),
        }
    }

    Converted {
        kind: ObjectKind::Text(TextObject {
            text,
            font_family: t.family.clone(),
            bold: t.bold,
            italic: t.italic,
            size_mm: t.height,
            letter_spacing_mm: 0.0,
            line_spacing: LINE_SPACING,
            align,
            on_path: None,
            bend_deg: None,
            welded: None,
            outline,
        }),
        transform: affine::compose(world, &affine::translate(0.0, b)),
        warnings,
    }
}
