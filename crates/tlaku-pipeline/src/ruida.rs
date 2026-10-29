//! Del trabajo calculado al flujo `.rd` de una controladora Ruida.
//!
//! El flujo se genera en claro (sin swizzle); `tlaku-ruida` lo parte en paquetes y lo ofusca al
//! enviarlo. Lo comparten el CLI (`tlaku ruida`) y la app.
//!
//! El origen del perfil (`Origin`) es la esquina donde la máquina tiene su X0 Y0 y las
//! coordenadas crecen hacia dentro de la cama; en Ruida esa esquina es el home (ruida.md 12.10),
//! así que se traduce uno a uno a [`HomeCorner`].
//!
//! **Iniciar desde** (ruida.md 12.15). Con «Coordenadas absolutas» el `.rd` lleva `D8 10` y las
//! coordenadas de la cama. Con «Posición actual» (`D8 12`) y «Origen de usuario» (`D8 11`) la
//! controladora resuelve el punto de partida al arrancar, así que el archivo lleva el trabajo
//! con el ancla (el punto de los 9 de la rejilla) en (0, 0): [`compute_anchored`] lo calcula y
//! [`export_rd_for_device_ref`] lo escribe. Como el trabajo suele guardarse y arrancarse más
//! tarde desde el panel, la posición del cabezal al subir no interviene.

use crate::job::{ComputedJob, LayerReport};
use tlaku_core::Point;
use tlaku_core::toolpath::{Move, Toolpath};
use tlaku_doc::{DeviceProfile, JobOptions, Origin, StartFrom};
use tlaku_i18n::tr;
use tlaku_ruida::rd::{self, HomeCorner, LayerOptions, MAX_LAYERS, RefMode, RuidaJobOptions};
use tlaku_ruida::safety::{MAX_SPEED_MM_MIN, SafetyLimits};

/// Esquina del home de Ruida que corresponde al origen del perfil.
pub fn home_for_origin(origin: Origin) -> HomeCorner {
    match origin {
        Origin::BottomLeft => HomeCorner::BottomLeft,
        Origin::TopLeft => HomeCorner::TopLeft,
        Origin::BottomRight => HomeCorner::BottomRight,
        Origin::TopRight => HomeCorner::TopRight,
    }
}

/// Genera los bytes del `.rd` (en claro) de un trabajo ya calculado. Usa el color de cada capa y
/// la cama indicada; los demás ajustes son los de [`RuidaJobOptions`] por defecto.
pub fn export_rd(
    built: &ComputedJob,
    bed: (f64, f64),
    home: HomeCorner,
) -> Result<Vec<u8>, String> {
    rd_with_limits(
        built,
        home,
        SafetyLimits {
            bed_mm: [bed.0, bed.1],
            ..SafetyLimits::default()
        },
        false,
        RefMode::Machine,
    )
}

/// Límites de seguridad con la cama del perfil y el máximo del protocolo Ruida (36 000
/// mm/min). El `max_rate` del perfil no recorta: la controladora tiene su propio máximo. Las
/// capas más rápidas que el tope se recortan al generar el `.rd` (ver [`export_rd_for_device`]).
pub fn limits_for_device(device: &DeviceProfile) -> SafetyLimits {
    SafetyLimits {
        bed_mm: device.bed,
        max_speed_mm_min: MAX_SPEED_MM_MIN,
        ..SafetyLimits::default()
    }
}

/// Como [`export_rd`] con la cama, el home y el tope de velocidad del perfil de la máquina.
///
/// Las capas del láser 2 (`LayerSettings::laser2`) salen con `CA 01 11` solo si el perfil dice
/// que la máquina tiene segundo láser (`device.laser2`); si no, salen con el láser 1 y
/// [`layer_warnings`] lo avisa. **Láser 2 sin probar con una máquina de dos láseres (M32).**
pub fn export_rd_for_device(
    built: &ComputedJob,
    device: &DeviceProfile,
) -> Result<Vec<u8>, String> {
    export_rd_for_device_ref(built, device, RefMode::Machine)
}

/// Como [`export_rd_for_device`] con la referencia de las coordenadas. Con una referencia
/// relativa (`D8 11/12`) `built` debe venir de [`compute_anchored`].
pub fn export_rd_for_device_ref(
    built: &ComputedJob,
    device: &DeviceProfile,
    reference: RefMode,
) -> Result<Vec<u8>, String> {
    rd_with_limits(
        built,
        home_for_origin(device.origin),
        limits_for_device(device),
        device.laser2,
        reference,
    )
}

/// Referencia `D8 1x` que corresponde a «Iniciar desde».
pub fn ref_mode_for(start_from: StartFrom) -> RefMode {
    match start_from {
        StartFrom::Absolute => RefMode::Machine,
        StartFrom::CurrentPosition => RefMode::Current,
        StartFrom::UserOrigin => RefMode::UserOrigin,
    }
}

/// Calcula el trabajo con el ancla (`options.job_origin`, o la caja de la selección con
/// `use_selection_origin`) en el (0, 0) del documento, para una referencia relativa. No exige que
/// el trabajo esté dentro de la cama (en esas coordenadas lo normal es que tenga negativos): lo
/// que se comprueba es que **quepa** (ver [`head_range`]); dónde debe estar el cabezal al arrancar
/// es cosa de quien lo arranca.
pub fn compute_anchored(
    doc: &tlaku_doc::Document,
    device: &DeviceProfile,
    options: &JobOptions,
) -> Result<ComputedJob, String> {
    let zero = crate::doc_to_machine(Point::new(0.0, 0.0), device);
    let y_scale = crate::rotary::y_scale(device)?;
    // `build` recibe la posición en coordenadas de la máquina y deshace la escala del rotativo.
    let anchor = Point::new(zero.x, zero.y * y_scale);
    let options = JobOptions {
        start_from: StartFrom::CurrentPosition,
        ..options.clone()
    };
    let job = crate::build(doc, device, &options, Some(anchor))?;
    if head_range(&job, device).is_some_and(|r| r.iter().any(|(lo, hi)| lo > hi)) {
        let b = job.bounds().expect("hay rango, hay caja");
        return Err(tr!(
            "pipeline.ruida.anchored_too_big",
            w = format!("{:.2}", b.max.x - b.min.x),
            h = format!("{:.2}", b.max.y - b.min.y),
            bed_w = device.bed[0],
            bed_h = device.bed[1]
        ));
    }
    Ok(job)
}

/// Caja del trabajo respecto al ancla, en el sistema de la máquina (mm): el home en (0, 0) y las
/// coordenadas crecen hacia dentro de la cama. `(x0, x1, y0, y1)`.
fn machine_box(job: &ComputedJob, device: &DeviceProfile) -> Option<(f64, f64, f64, f64)> {
    let b = job.bounds()?;
    let home = home_for_origin(device.origin);
    let (ax, ay) = home.to_machine(b.min, [0.0, 0.0]);
    let (cx, cy) = home.to_machine(b.max, [0.0, 0.0]);
    Some((ax.min(cx), ax.max(cx), ay.min(cy), ay.max(cy)))
}

/// Dónde puede estar el cabezal al arrancar para que el trabajo (anclado con
/// [`compute_anchored`]) quepa en la cama: `[(x_min, x_max), (y_min, y_max)]` en mm de la
/// máquina. Un rango con mínimo mayor que máximo quiere decir que no cabe en ningún sitio. La Y
/// queda libre con rotativo (su eje no tiene cama). `None` si el trabajo está vacío.
pub fn head_range(job: &ComputedJob, device: &DeviceProfile) -> Option<[(f64, f64); 2]> {
    let (x0, x1, y0, y1) = machine_box(job, device)?;
    // El ancla (0, 0) forma parte del recorrido: el cabezal también ha de estar dentro.
    let (x0, x1, y0, y1) = (x0.min(0.0), x1.max(0.0), y0.min(0.0), y1.max(0.0));
    let y = if device.rotary.enabled {
        (f64::NEG_INFINITY, f64::INFINITY)
    } else {
        (-y0, device.bed[1] - y1)
    };
    Some([(-x0, device.bed[0] - x1), y])
}

/// Esquinas del encuadre (mm de la máquina) de un trabajo anclado con el cabezal en `head`,
/// cerrando el recorrido en la primera.
pub fn anchored_frame_corners(
    job: &ComputedJob,
    device: &DeviceProfile,
    head: [f64; 2],
) -> Option<Vec<(f64, f64)>> {
    let (x0, x1, y0, y1) = machine_box(job, device)?;
    let (x0, x1, y0, y1) = (head[0] + x0, head[0] + x1, head[1] + y0, head[1] + y1);
    Some(vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)])
}

fn job_options(
    layers: &[LayerReport],
    home: HomeCorner,
    limits: SafetyLimits,
    laser2: bool,
    reference: RefMode,
) -> RuidaJobOptions {
    RuidaJobOptions {
        limits,
        layers: layers
            .iter()
            .map(|l| LayerOptions {
                color: [l.color.0, l.color.1, l.color.2],
                min_power_pct: None,
                laser2: laser2 && l.laser2,
            })
            .collect(),
        home,
        reference,
        ..RuidaJobOptions::default()
    }
}

/// Genera el `.rd`: recorta la velocidad de las capas al máximo de `limits` y, si hay más de 128
/// capas, fusiona las vecinas de ajustes idénticos (los avisos salen en [`layer_warnings`]).
fn rd_with_limits(
    built: &ComputedJob,
    home: HomeCorner,
    limits: SafetyLimits,
    laser2: bool,
    reference: RefMode,
) -> Result<Vec<u8>, String> {
    let opts = job_options(&built.layers, home, limits, laser2, reference);
    let clamped = rd::clamp_speeds(&built.toolpath, limits.max_speed_mm_min);
    let toolpath = clamped.as_ref().unwrap_or(&built.toolpath);
    rd::build_rd(toolpath, &opts).map_err(|e| e.to_string())
}

/// Bloques del trabajo que llevan al menos un corte (los que son una capa del `.rd`).
fn cutting_blocks(tp: &Toolpath) -> usize {
    tp.blocks
        .iter()
        .filter(|b| b.moves.iter().any(|m| matches!(m, Move::Cut { .. })))
        .count()
}

/// Avisos del cálculo para una máquina Ruida: capas que el `.rd` fusionará o que no caben en las
/// 128 que admite Ruida. Los de velocidad salen de [`crate::layers::speed_warnings`].
pub(crate) fn layer_warnings(
    toolpath: &Toolpath,
    layers: &[LayerReport],
    device: &DeviceProfile,
) -> Vec<String> {
    let mut out = Vec::new();
    if !device.laser2 && layers.iter().any(|l| l.laser2) {
        out.push(tr!("pipeline.ruida.no_laser2", machine = device.name));
    }
    // Caso común: no hay nada más que decir y no se planifica el trabajo entero.
    if cutting_blocks(toolpath) <= MAX_LAYERS {
        return out;
    }
    let opts = job_options(
        layers,
        home_for_origin(device.origin),
        limits_for_device(device),
        device.laser2,
        RefMode::Machine,
    );
    out.extend(match rd::plan_summary(toolpath, &opts) {
        Ok(s) if s.fits() => vec![tr!(
            "pipeline.ruida.layers_merged",
            max = MAX_LAYERS,
            blocks = s.blocks,
            merged = s.merged,
            layers = s.layers
        )],
        Ok(s) => vec![tr!(
            "pipeline.ruida.layers_too_many",
            max = MAX_LAYERS,
            blocks = s.blocks,
            merged = s.merged,
            layers = s.layers
        )],
        Err(e) => vec![tr!("pipeline.ruida.cannot_export", error = e)],
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_core::bezpath::BezPath;
    use tlaku_core::{Polyline, Rgb};
    use tlaku_doc::{DesignObject, Document, LayerSettings, ObjectKind, VectorObject, affine};

    const RED: Rgb = Rgb(255, 0, 0);

    /// Cuadrado de 20 mm con la esquina inferior izquierda en (10, 10).
    fn square_doc() -> Document {
        let obj = DesignObject {
            id: "a".into(),
            layer: RED,
            transform: affine::IDENTITY,
            kind: ObjectKind::Vector(VectorObject::new(vec![BezPath::from(&Polyline {
                points: vec![
                    Point::new(10.0, 10.0),
                    Point::new(30.0, 10.0),
                    Point::new(30.0, 30.0),
                    Point::new(10.0, 30.0),
                ],
                closed: true,
            })])),
            locked: false,
            hidden: false,
            group: None,
            priority: None,
            power_scale: None,
            shape: None,
            start: None,
            manual_tabs: Vec::new(),
            array: None,
        };
        Document {
            objects: vec![obj],
            layers: vec![LayerSettings::for_color(RED)],
            ..Document::default()
        }
    }

    fn device(origin: Origin) -> DeviceProfile {
        DeviceProfile {
            bed: [600.0, 400.0],
            origin,
            ..DeviceProfile::default()
        }
    }

    fn opts(start_from: StartFrom, job_origin: u32) -> JobOptions {
        JobOptions {
            start_from,
            job_origin,
            ..JobOptions::default()
        }
    }

    #[test]
    fn origins_map_to_the_same_corner() {
        assert_eq!(home_for_origin(Origin::TopLeft), HomeCorner::TopLeft);
        assert_eq!(
            home_for_origin(Origin::BottomRight),
            HomeCorner::BottomRight
        );
        assert_eq!(home_for_origin(Origin::BottomLeft), HomeCorner::BottomLeft);
        assert_eq!(home_for_origin(Origin::TopRight), HomeCorner::TopRight);
    }

    #[test]
    fn start_from_maps_to_the_reference_command() {
        assert_eq!(ref_mode_for(StartFrom::Absolute), RefMode::Machine);
        assert_eq!(ref_mode_for(StartFrom::CurrentPosition), RefMode::Current);
        assert_eq!(ref_mode_for(StartFrom::UserOrigin), RefMode::UserOrigin);
    }

    #[test]
    fn the_anchor_goes_to_zero_and_the_job_needs_no_head_position() {
        let dev = device(Origin::BottomLeft);
        // Ancla al centro (4): el cuadrado queda de -10 a 10 en los dos ejes.
        let job =
            compute_anchored(&square_doc(), &dev, &opts(StartFrom::CurrentPosition, 4)).unwrap();
        let b = job.bounds().unwrap();
        assert!(
            (b.min.x + 10.0).abs() < 1e-6 && (b.max.y - 10.0).abs() < 1e-6,
            "{b:?}"
        );
        // Con el cabezal a 10 mm del home o más y a 10 del extremo contrario cabe.
        let [(x0, x1), (y0, y1)] = head_range(&job, &dev).unwrap();
        assert!((x0 - 10.0).abs() < 1e-6 && (x1 - 590.0).abs() < 1e-6);
        assert!((y0 - 10.0).abs() < 1e-6 && (y1 - 390.0).abs() < 1e-6);
        // Ancla abajo a la izquierda (6): el cuadrado queda de 0 a 20.
        let job = compute_anchored(&square_doc(), &dev, &opts(StartFrom::UserOrigin, 6)).unwrap();
        let [(x0, x1), _] = head_range(&job, &dev).unwrap();
        assert!(x0.abs() < 1e-6 && (x1 - 580.0).abs() < 1e-6);
    }

    #[test]
    fn the_home_corner_flips_the_range() {
        // Home arriba a la izquierda: arriba de Tlaku es "lejos del home" en Y.
        let dev = device(Origin::TopLeft);
        let job =
            compute_anchored(&square_doc(), &dev, &opts(StartFrom::CurrentPosition, 6)).unwrap();
        // Ancla abajo-izquierda; el cuadrado sube 20 mm: en la máquina eso es -Y (hacia el home).
        let [(x0, x1), (y0, y1)] = head_range(&job, &dev).unwrap();
        assert!(x0.abs() < 1e-6 && (x1 - 580.0).abs() < 1e-6);
        assert!((y0 - 20.0).abs() < 1e-6 && (y1 - 400.0).abs() < 1e-6);
        let corners = anchored_frame_corners(&job, &dev, [100.0, 50.0]).unwrap();
        assert_eq!(corners.len(), 5);
        assert_eq!(corners[0], corners[4]);
        let xs = corners.iter().map(|c| c.0);
        let ys = corners.iter().map(|c| c.1);
        assert!((xs.clone().fold(f64::MAX, f64::min) - 100.0).abs() < 1e-6);
        assert!((xs.fold(f64::MIN, f64::max) - 120.0).abs() < 1e-6);
        assert!((ys.clone().fold(f64::MAX, f64::min) - 30.0).abs() < 1e-6);
        assert!((ys.fold(f64::MIN, f64::max) - 50.0).abs() < 1e-6);
    }

    #[test]
    fn a_job_bigger_than_the_bed_is_rejected_whatever_the_head() {
        let mut dev = device(Origin::BottomLeft);
        dev.bed = [15.0, 400.0];
        let err = compute_anchored(&square_doc(), &dev, &opts(StartFrom::CurrentPosition, 4))
            .unwrap_err();
        assert!(err.contains("15"), "{err}");
    }

    #[test]
    fn exported_files_carry_the_reference_and_relative_coordinates() {
        let dev = device(Origin::BottomLeft);
        let job =
            compute_anchored(&square_doc(), &dev, &opts(StartFrom::CurrentPosition, 4)).unwrap();
        let rd = export_rd_for_device_ref(&job, &dev, RefMode::Current).unwrap();
        assert_eq!(&rd[..2], [0xD8, 0x12]);
        // Con la referencia de la máquina, ese mismo trabajo (con negativos) no se puede escribir.
        assert!(export_rd_for_device_ref(&job, &dev, RefMode::Machine).is_err());
        // Absoluto: D8 10 y las coordenadas de la cama.
        let abs = crate::compute(&square_doc(), &dev, &opts(StartFrom::Absolute, 4)).unwrap();
        let rd = export_rd_for_device(&abs, &dev).unwrap();
        assert_eq!(&rd[..2], [0xD8, 0x10]);
    }
}
