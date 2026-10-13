//! Ajustes de capa a ajustes del motor, clasificación y orden automático de capas.

use tlaku_core::{CutSettings, Rgb};
use tlaku_doc::{FillStyle, ImageMode, LayerMode, LayerOrder, LayerSettings, Overscan};
use tlaku_fill::{FillPattern, FillSettings};
use tlaku_raster::{BayerSize, ImageAdjust, RasterMode, RasterSettings};

/// Tope de pasadas: protege contra errores de dedo que generarían G-code gigantesco.
pub const MAX_PASSES: u32 = 1000;

/// Potencia (%) desde la que una línea con varias pasadas se considera corte.
const CUT_POWER_PCT: f64 = 80.0;

pub(crate) const KERF_REASON: &str = "kerf";
pub(crate) const POWER_REASON: &str = "potencia ≥ 80 % con varias pasadas";

/// Capa line o fill ya resuelta, lista para ejecutarse.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedLayer {
    Line {
        settings: CutSettings,
        /// Desplazamiento de cada trayectoria (0 = sin compensar).
        kerf_mm: f64,
    },
    Fill {
        /// Con `overscan_mm` en 0 si `overscan` es `Auto`: se calcula al armar el trabajo.
        settings: FillSettings,
        overscan: Overscan,
        passes: u32,
    },
}

/// Parámetros de una capa de imagen ya resueltos.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedImage {
    pub adjust: ImageAdjust,
    pub mode: RasterMode,
    pub interval_mm: f64,
    pub raster: RasterSettings,
    pub overscan: Overscan,
    pub passes: u32,
}

fn check_passes(passes: u32) -> Result<(), String> {
    if passes == 0 {
        return Err("las pasadas deben ser al menos 1".into());
    }
    if passes > MAX_PASSES {
        return Err(format!(
            "demasiadas pasadas ({passes}); el máximo es {MAX_PASSES}"
        ));
    }
    Ok(())
}

fn overscan_mm(o: Overscan) -> f64 {
    match o {
        Overscan::Auto => 0.0,
        Overscan::Mm(v) => v,
    }
}

/// Convierte y valida una capa line o fill. Los errores salen con el prefijo `capa #rrggbb:`.
pub fn resolve_layer(l: &LayerSettings) -> Result<ResolvedLayer, String> {
    let err = |m: String| format!("capa {}: {m}", l.color);
    match l.mode {
        LayerMode::Line => {
            let settings = CutSettings {
                speed_mm_min: l.speed,
                power_pct: l.power,
                passes: l.passes,
                air_assist: l.air,
            };
            settings.validate().map_err(err)?;
            check_passes(settings.passes).map_err(err)?;
            if !l.kerf.is_finite() {
                return Err(err(format!("kerf inválido: {}", l.kerf)));
            }
            Ok(ResolvedLayer::Line {
                settings,
                kerf_mm: l.kerf,
            })
        }
        LayerMode::Fill => {
            check_passes(l.passes).map_err(err)?;
            let settings = FillSettings {
                interval_mm: l.interval,
                angle_deg: l.angle,
                bidirectional: l.bidirectional,
                overscan_mm: overscan_mm(l.overscan),
                crosshatch: l.crosshatch,
                speed_mm_min: l.speed,
                power_pct: l.power,
                air_assist: l.air,
                pattern: match l.fill_style {
                    FillStyle::Scan => FillPattern::Scan,
                    FillStyle::Offset => FillPattern::Offset,
                    // TODO(ola2): relleno por regiones.
                    FillStyle::Flood => FillPattern::Scan,
                },
                ..FillSettings::default()
            };
            settings.validate().map_err(err)?;
            Ok(ResolvedLayer::Fill {
                settings,
                overscan: l.overscan,
                passes: l.passes,
            })
        }
        LayerMode::Image => Err(err("es una capa de imagen".into())),
    }
}

fn raster_mode(l: &LayerSettings) -> RasterMode {
    match l.image.mode {
        ImageMode::Threshold => RasterMode::Threshold(l.image.threshold as f32),
        ImageMode::Floyd => RasterMode::FloydSteinberg,
        ImageMode::Jarvis => RasterMode::Jarvis,
        ImageMode::Stucki => RasterMode::Stucki,
        ImageMode::Atkinson => RasterMode::Atkinson,
        ImageMode::Bayer4 => RasterMode::Bayer(BayerSize::N4),
        ImageMode::Grayscale => RasterMode::Grayscale,
        ImageMode::Halftone => RasterMode::Halftone {
            cell_mm: l.image.halftone_cell,
            angle_deg: l.image.halftone_angle,
        },
        ImageMode::Newsprint => RasterMode::Newsprint {
            cell_mm: l.image.halftone_cell,
            angle_deg: l.image.halftone_angle,
        },
        ImageMode::Sketch => RasterMode::Sketch,
    }
}

/// Convierte y valida una capa de imagen. Los errores salen con el prefijo `imagen:`.
pub fn resolve_image(l: &LayerSettings) -> Result<ResolvedImage, String> {
    let img = &l.image;
    let adjust = ImageAdjust {
        brightness: img.brightness as f32,
        contrast: img.contrast as f32,
        gamma: img.gamma as f32,
        invert: img.invert,
        ..ImageAdjust::default()
    };
    adjust.validate().map_err(|e| format!("imagen: {e}"))?;
    if img.mode == ImageMode::Threshold && !(0.0..=1.0).contains(&img.threshold) {
        return Err(format!("imagen: umbral fuera de 0-1: {}", img.threshold));
    }
    let raster = RasterSettings {
        speed_mm_min: l.speed,
        min_power_pct: l.min_power,
        max_power_pct: l.power,
        bidirectional: l.bidirectional,
        overscan_mm: overscan_mm(l.overscan),
        dot_width_mm: img.dot_width,
        air_assist: l.air,
        angle_deg: l.angle,
        ..RasterSettings::default()
    };
    raster.validate().map_err(|e| format!("imagen: {e}"))?;
    if !(l.interval.is_finite() && l.interval > 0.0) {
        return Err(format!("imagen: intervalo inválido: {}", l.interval));
    }
    check_passes(l.passes).map_err(|e| format!("imagen: {e}"))?;
    Ok(ResolvedImage {
        adjust,
        mode: raster_mode(l),
        interval_mm: l.interval,
        raster,
        overscan: l.overscan,
        passes: l.passes,
    })
}

/// Función de una capa en el orden automático. El orden de las variantes es el de ejecución.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LayerRole {
    /// Relleno o imagen.
    Engrave,
    /// Línea que no corta la pieza (marcado, trazos).
    Line,
    /// Línea que separa la pieza del material: debe ir al final.
    Cut,
}

/// Clasifica una capa ya resuelta; para los cortes devuelve además el motivo.
pub fn classify(layer: &ResolvedLayer) -> (LayerRole, Option<&'static str>) {
    match layer {
        ResolvedLayer::Fill { .. } => (LayerRole::Engrave, None),
        ResolvedLayer::Line { settings, kerf_mm } => {
            if *kerf_mm != 0.0 {
                (LayerRole::Cut, Some(KERF_REASON))
            } else if settings.power_pct >= CUT_POWER_PCT && settings.passes > 1 {
                (LayerRole::Cut, Some(POWER_REASON))
            } else {
                (LayerRole::Line, None)
            }
        }
    }
}

/// Índices de las capas en el orden de ejecución. Las capas con posición explícita
/// (`forced`, desde 1) se colocan después de ordenar las demás; una posición mayor que el
/// número de capas equivale a "al final". El orden automático es estable: dentro de cada
/// grupo se conserva el del archivo.
pub fn layer_sequence(
    roles: &[LayerRole],
    forced: &[Option<usize>],
    mode: LayerOrder,
) -> Vec<usize> {
    let mut seq: Vec<usize> = (0..roles.len()).filter(|&i| forced[i].is_none()).collect();
    if mode == LayerOrder::Auto {
        seq.sort_by_key(|&i| roles[i]);
    }
    let mut pinned: Vec<usize> = (0..roles.len()).filter(|&i| forced[i].is_some()).collect();
    pinned.sort_by_key(|&i| forced[i]);
    for i in pinned {
        let at = forced[i].map_or(0, |n| n - 1).min(seq.len());
        seq.insert(at, i);
    }
    seq
}

/// Frase que explica el orden elegido, para mostrarla en pantalla. `roles` y `cuts` son solo
/// de las capas sin posición explícita (`forced` lista las que sí la tienen).
pub(crate) fn order_note(
    mode: LayerOrder,
    roles: &[LayerRole],
    cuts: &[(Rgb, &'static str)],
    forced: &[(Rgb, usize)],
) -> String {
    let mut note = match mode {
        LayerOrder::File => "orden del archivo (--layer-order file)".to_string(),
        LayerOrder::Auto => {
            let names = [
                (LayerRole::Engrave, "grabados"),
                (LayerRole::Line, "líneas"),
                (LayerRole::Cut, "cortes"),
            ];
            let present: Vec<&str> = names
                .iter()
                .filter(|(r, _)| roles.contains(r))
                .map(|(_, n)| *n)
                .collect();
            if roles.is_empty() {
                "orden fijado con order=".to_string()
            } else if present.len() < 2 {
                let why = if roles.len() > 1 {
                    " (todas las capas son del mismo tipo)"
                } else {
                    ""
                };
                format!("orden automático: se conserva el orden del archivo{why}")
            } else {
                let last = present.len() - 1;
                let parts: Vec<String> = present
                    .iter()
                    .enumerate()
                    .map(|(i, n)| match i {
                        0 => format!("{n} primero"),
                        i if i == last => format!("{n} al final"),
                        _ => format!("{n} después"),
                    })
                    .collect();
                format!("orden automático: {}", parts.join(", "))
            }
        }
    };
    if mode == LayerOrder::Auto && !cuts.is_empty() {
        let list: Vec<String> = cuts
            .iter()
            .map(|(c, why)| format!("{c} por {why}"))
            .collect();
        note.push_str(&format!(" (corte: {})", list.join("; ")));
    }
    if !forced.is_empty() {
        let list: Vec<String> = forced
            .iter()
            .map(|(c, n)| format!("{c} en la posición {n}"))
            .collect();
        let sep = if roles.is_empty() {
            ": "
        } else {
            "; order= explícito: "
        };
        note.push_str(&format!("{sep}{}", list.join(", ")));
    }
    note
}
