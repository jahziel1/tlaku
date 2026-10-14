//! Ajustes de capa a ajustes del motor, clasificación y orden automático de capas.

use crate::place::LayerWork;
use tlaku_core::{CutSettings, Rgb};
use tlaku_doc::{
    DeviceKind, DeviceProfile, FillGrouping, FillStyle, ImageMode, LayerMode, LayerOrder,
    LayerSettings, Overscan,
};
use tlaku_fill::{FillPattern, FillSettings, Grouping};
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
                    FillStyle::Flood => FillPattern::Flood,
                },
                grouping: match l.fill_grouping {
                    FillGrouping::All => Grouping::All,
                    FillGrouping::ByShape => Grouping::PerShape,
                    // Sin ids de grupo por trayectoria el motor lo trata como `ByShape`
                    // (ver `tlaku_fill::fill_with_groups`).
                    FillGrouping::ByGroup => Grouping::ByGroup,
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
    // Pass-through: el gris de cada píxel es directamente la potencia (negro = máxima), sin
    // tramado; la imagen se remuestrea al intervalo como siempre.
    if l.image.pass_through {
        return RasterMode::Grayscale;
    }
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
    if !(img.denoise.is_finite() && img.denoise >= 0.0) {
        return Err(format!("imagen: quitar motas inválido: {}", img.denoise));
    }
    let adjust = ImageAdjust {
        brightness: img.brightness as f32,
        contrast: img.contrast as f32,
        gamma: img.gamma as f32,
        invert: img.invert,
        enhance_radius_px: img.enhance_radius as f32,
        enhance_amount: img.enhance_amount as f32,
        denoise_px: img.denoise.round().min(f64::from(u32::MAX)) as u32,
    };
    adjust.validate().map_err(|e| format!("imagen: {e}"))?;
    // Pass-through no aplica ningún ajuste: se envía la imagen tal cual.
    let adjust = if img.pass_through {
        ImageAdjust::default()
    } else {
        adjust
    };
    if !img.pass_through
        && img.mode == ImageMode::Threshold
        && !(0.0..=1.0).contains(&img.threshold)
    {
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

/// Velocidad máxima (mm/min) del perfil. `None` si no se usa `max_rate` (galvo, Lihuiyu).
pub(crate) fn speed_limit(device: &DeviceProfile) -> Option<f64> {
    if matches!(device.kind, DeviceKind::Galvo | DeviceKind::LhyMicro) {
        return None;
    }
    // Un eje más lento que el otro solo limita los movimientos en su dirección: el aviso
    // salta cuando la velocidad no la alcanza ningún eje.
    let machine = device.max_rate[0].max(device.max_rate[1]);
    (machine.is_finite() && machine > 0.0).then_some(machine)
}

/// Avisos de las capas que piden más velocidad de la que da la máquina. No cambian la salida:
/// GRBL limita el avance por su cuenta. En Ruida el `max_rate` del perfil es una suposición (la
/// controladora tiene el suyo), así que solo se avisa; lo único que se recorta es el máximo
/// del protocolo (36 000 mm/min) al generar el `.rd`.
pub(crate) fn speed_warnings(work: &[LayerWork], device: &DeviceProfile) -> Vec<String> {
    let machine = speed_limit(device);
    let ruida = device.kind == DeviceKind::Ruida;
    let mut out = Vec::new();
    for l in work {
        let v = l.settings.speed;
        if !v.is_finite() {
            continue;
        }
        let name = if l.settings.mode == LayerMode::Image {
            "imagen".to_string()
        } else {
            match l.position() {
                Some((k, n)) => format!("{} · {k}/{n}", l.color),
                None => l.color.to_string(),
            }
        };
        let cap = tlaku_ruida::safety::MAX_SPEED_MM_MIN;
        if ruida && v > cap {
            out.push(format!(
                "capa {name}: pide {v:.0} mm/min y Ruida admite como máximo {cap:.0} mm/min; el \
                 archivo .rd usará {cap:.0} mm/min, así que el resultado será más oscuro o más \
                 quemado de lo previsto"
            ));
        }
        if let Some(max) = machine
            && v > max
        {
            let tail = if ruida {
                "si tu controladora admite más, sube la velocidad máxima del perfil"
            } else {
                "baja la velocidad de la capa o corrige la velocidad máxima del perfil"
            };
            out.push(format!(
                "capa {name}: pide {v:.0} mm/min y la máquina '{}' llega como máximo a {max:.0} \
                 mm/min; la máquina irá al máximo, así que el resultado será más oscuro o más \
                 quemado de lo previsto ({tail})",
                device.name
            ));
        }
    }
    out
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

/// Una operación del trabajo para ordenar: una capa del documento o una de sus sub-capas.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SeqUnit {
    /// Capa del documento de la que sale.
    pub parent: usize,
    /// Es una sub-capa (no la principal).
    pub sub: bool,
    pub role: LayerRole,
    /// Posición forzada (`order=`, desde 1); solo la tienen las capas principales.
    pub forced: Option<usize>,
}

/// Rol efectivo de cada operación: el mayor entre el suyo y el de las anteriores de su capa si
/// es una sub-capa (ver [`unit_sequence`]).
pub(crate) fn effective_roles(units: &[SeqUnit]) -> Vec<LayerRole> {
    let mut effective: Vec<LayerRole> = Vec::with_capacity(units.len());
    for (i, u) in units.iter().enumerate() {
        let prev = (i > 0 && u.sub && units[i - 1].parent == u.parent).then(|| effective[i - 1]);
        effective.push(prev.map_or(u.role, |p| p.max(u.role)));
    }
    effective
}

/// Orden de ejecución de las operaciones del trabajo (capas principales y sub-capas, dadas en el
/// orden del documento: cada capa seguida de sus sub-capas). Devuelve los índices en el orden de
/// ejecución y cuáles se ordenaron libremente (`movable`; las demás son las sub-capas de una capa
/// con posición forzada).
///
/// Reglas (ver "Decisiones del taller" en `docs/arquitectura.md`):
/// - Una sub-capa **se ordena por su propio rol**, como cualquier otra capa, y no pegada a su
///   capa: así un contorno de corte en una sub-capa va al final con los demás cortes y no suelta
///   la pieza antes de que otras capas graben. Nunca va antes que la operación anterior de su
///   capa: su rol efectivo es el mayor entre el suyo y el de las anteriores, de modo que "capa de
///   corte + sub-capa de grabado" conserva el orden que puso el usuario.
/// - Una capa con posición forzada arrastra a sus sub-capas, que la siguen juntas. Las posiciones
///   cuentan solo las operaciones de `movable`.
/// - En el orden del archivo (`LayerOrder::File`) cada capa va seguida de sus sub-capas.
pub(crate) fn unit_sequence(units: &[SeqUnit], mode: LayerOrder) -> (Vec<usize>, Vec<usize>) {
    let pinned = |parent: usize| {
        units
            .iter()
            .any(|u| u.parent == parent && !u.sub && u.forced.is_some())
    };
    let effective = effective_roles(units);
    let movable: Vec<usize> = (0..units.len())
        .filter(|&i| !(units[i].sub && pinned(units[i].parent)))
        .collect();
    let roles: Vec<LayerRole> = movable.iter().map(|&i| effective[i]).collect();
    let forced: Vec<Option<usize>> = movable.iter().map(|&i| units[i].forced).collect();
    let mut seq = Vec::with_capacity(units.len());
    for j in layer_sequence(&roles, &forced, mode) {
        let i = movable[j];
        seq.push(i);
        if units[i].forced.is_some() {
            seq.extend(
                (i + 1..units.len())
                    .take_while(|&k| units[k].sub && units[k].parent == units[i].parent),
            );
        }
    }
    (seq, movable)
}

/// Dos operaciones hacen lo mismo si solo difieren en lo que no afecta a la salida (color,
/// nombre, visibilidad, posición y sub-capas).
pub(crate) fn same_operation(a: &LayerSettings, b: &LayerSettings) -> bool {
    let mut x = a.clone();
    x.color = b.color;
    x.name.clone_from(&b.name);
    x.visible = b.visible;
    x.order = b.order;
    x.sub_layers.clone_from(&b.sub_layers);
    x == *b
}

/// Agrupa operaciones de corte consecutivas (en el orden de ejecución) que hacen exactamente lo
/// mismo y no tienen posición forzada, para planificarlas juntas (`by_layer = false`). Cada
/// grupo es una lista de índices en `work`; el resto va solo.
pub(crate) fn merge_steps(
    order: &[usize],
    work: &[&LayerSettings],
    forced: &[Option<usize>],
) -> Vec<Vec<usize>> {
    let mut steps: Vec<Vec<usize>> = Vec::new();
    for &i in order {
        let can_join = steps.last().is_some_and(|last| {
            let j = last[0];
            work[i].mode == LayerMode::Line
                && work[j].mode == LayerMode::Line
                && forced[i].is_none()
                && forced[j].is_none()
                && same_operation(work[j], work[i])
        });
        if can_join {
            steps.last_mut().unwrap().push(i);
        } else {
            steps.push(vec![i]);
        }
    }
    steps
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

#[cfg(test)]
mod mapping_tests {
    use super::*;
    use tlaku_doc::{ImageSettings, LayerSettings};

    fn fill_layer() -> LayerSettings {
        LayerSettings {
            mode: LayerMode::Fill,
            ..LayerSettings::default()
        }
    }

    fn fill_settings(l: &LayerSettings) -> FillSettings {
        match resolve_layer(l).unwrap() {
            ResolvedLayer::Fill { settings, .. } => settings,
            _ => panic!("debía ser fill"),
        }
    }

    #[test]
    fn fill_style_and_grouping_reach_the_engine() {
        for (style, pattern) in [
            (FillStyle::Scan, FillPattern::Scan),
            (FillStyle::Offset, FillPattern::Offset),
            (FillStyle::Flood, FillPattern::Flood),
        ] {
            let l = LayerSettings {
                fill_style: style,
                ..fill_layer()
            };
            assert_eq!(fill_settings(&l).pattern, pattern);
        }
        for (grouping, want) in [
            (FillGrouping::All, Grouping::All),
            (FillGrouping::ByShape, Grouping::PerShape),
            (FillGrouping::ByGroup, Grouping::ByGroup),
        ] {
            let l = LayerSettings {
                fill_grouping: grouping,
                ..fill_layer()
            };
            assert_eq!(fill_settings(&l).grouping, want);
        }
    }

    fn image_layer(f: impl FnOnce(&mut ImageSettings)) -> LayerSettings {
        let mut l = LayerSettings::for_images();
        f(&mut l.image);
        l
    }

    #[test]
    fn enhance_and_denoise_reach_the_adjust() {
        let r = resolve_image(&image_layer(|i| {
            i.enhance_radius = 1.5;
            i.enhance_amount = 0.8;
            i.denoise = 2.0;
            i.brightness = 0.1;
        }))
        .unwrap();
        assert_eq!(r.adjust.enhance_radius_px, 1.5);
        assert_eq!(r.adjust.enhance_amount, 0.8);
        assert_eq!(r.adjust.denoise_px, 2);
        assert_eq!(r.adjust.brightness, 0.1f32);
    }

    #[test]
    fn pass_through_sends_gray_as_power_without_adjusts_or_dither() {
        let r = resolve_image(&image_layer(|i| {
            i.pass_through = true;
            i.mode = ImageMode::Floyd;
            i.brightness = 0.3;
            i.invert = true;
            i.enhance_radius = 1.0;
            i.enhance_amount = 1.0;
            i.denoise = 3.0;
        }))
        .unwrap();
        assert_eq!(r.mode, RasterMode::Grayscale);
        assert_eq!(r.adjust, ImageAdjust::default());
        // Se remuestrea al intervalo de la capa, no se conserva el tamaño del archivo.
        assert_eq!(r.interval_mm, 0.1);
    }

    #[test]
    fn bad_enhance_and_denoise_values_are_rejected() {
        let bad =
            |f: &dyn Fn(&mut ImageSettings)| resolve_image(&image_layer(|i| f(i))).unwrap_err();
        assert!(bad(&|i| i.enhance_radius = -1.0).contains("enfoque"));
        assert!(bad(&|i| i.enhance_amount = f64::NAN).contains("enfoque"));
        assert!(bad(&|i| i.denoise = -2.0).contains("motas"));
        assert!(bad(&|i| i.denoise = 99.0).contains("motas"));
    }
}
