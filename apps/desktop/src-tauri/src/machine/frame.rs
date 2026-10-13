//! Encuadre de banda elástica: el cabezal sigue el casco convexo del trabajo (con el láser
//! apagado) en lugar del rectángulo envolvente. Sirve para piezas largas y en diagonal, donde el
//! rectángulo recorre mucha cama vacía y no deja ver dónde caerá la pieza.
//!
//! Aquí se pasan los puntos del trabajo a coordenadas de la máquina (origen de la cama, espejo y
//! escala de Y del rotativo, igual que el G-code del trabajo) y se delega el recorrido en
//! `tlaku_grbl::frame::frame_hull_gcode`.

use crate::engine::Head;
use std::f64::consts::PI;
use tlaku_core::Point;
use tlaku_doc::{DeviceProfile, Document, JobOptions, Origin, RotaryKind};
use tlaku_grbl::frame::frame_hull_gcode;

/// Pasa un punto del documento (origen abajo a la izquierda) a la esquina de origen de la cama.
fn to_machine(p: Point, device: &DeviceProfile) -> Point {
    let (flip_x, flip_y) = match device.origin {
        Origin::BottomLeft => (false, false),
        Origin::TopLeft => (false, true),
        Origin::BottomRight => (true, false),
        Origin::TopRight => (true, true),
    };
    Point::new(
        if flip_x { device.bed[0] - p.x } else { p.x },
        if flip_y { device.bed[1] - p.y } else { p.y },
    )
}

/// mm de la controladora por mm de superficie en Y (1 sin rotativo). Un rotativo mal
/// configurado ya hizo fallar el cálculo del trabajo, así que aquí cae a 1.
fn y_scale(device: &DeviceProfile) -> f64 {
    let r = &device.rotary;
    if !r.enabled {
        return 1.0;
    }
    let diameter = match r.kind {
        RotaryKind::Roller => r.roller_diameter,
        RotaryKind::Chuck => r.object_diameter,
    };
    let k = r.mm_per_rotation / (PI * diameter);
    if k.is_finite() && k > 0.0 { k } else { 1.0 }
}

/// G-code del encuadre del casco convexo del trabajo (láser apagado), listo para transmitir.
pub fn hull_lines(
    doc: &Document,
    device: &DeviceProfile,
    options: &JobOptions,
    speed: f64,
    head: Head,
) -> Result<Vec<String>, String> {
    if !(speed.is_finite() && speed > 0.0) {
        return Err("La velocidad del encuadre debe ser mayor que 0.".into());
    }
    let current = head.map(|[x, y]| Point::new(x, y));
    let job = tlaku_pipeline::compute_at(doc, device, options, current)?;
    // Todos los movimientos, overscan incluido, como la caja del encuadre del rectángulo.
    let mut points: Vec<Point> = job
        .toolpath
        .blocks
        .iter()
        .flat_map(|b| &b.moves)
        .map(|m| to_machine(m.to(), device))
        .collect();
    if device.rotary.enabled && device.rotary.mirror {
        let ys = points.iter().map(|p| p.y).filter(|y| y.is_finite());
        let (lo, hi) = ys.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), y| {
            (lo.min(y), hi.max(y))
        });
        for p in &mut points {
            p.y = lo + hi - p.y;
        }
    }
    let lines = frame_hull_gcode(&points, speed, y_scale(device));
    if lines.is_empty() {
        return Err("No hay nada que encuadrar: el trabajo está vacío.".into());
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_core::bezpath::BezPath;
    use tlaku_core::{Polyline, Rgb};
    use tlaku_doc::affine::IDENTITY;
    use tlaku_doc::{DesignObject, LayerSettings, ObjectKind, VectorObject};

    /// Un documento con un triángulo rectángulo cerrado en la capa roja.
    fn triangle_doc() -> Document {
        let red = Rgb(255, 0, 0);
        Document {
            objects: vec![DesignObject {
                id: "t".into(),
                layer: red,
                transform: IDENTITY,
                kind: ObjectKind::Vector(VectorObject {
                    paths: vec![BezPath::from(&Polyline {
                        points: vec![
                            Point::new(10.0, 10.0),
                            Point::new(50.0, 10.0),
                            Point::new(10.0, 40.0),
                        ],
                        closed: true,
                    })],
                }),
                locked: false,
                hidden: false,
                group: None,
                priority: None,
                power_scale: None,
                shape: None,
            }],
            layers: vec![LayerSettings::for_color(red)],
            ..Document::default()
        }
    }

    fn visits(lines: &[String], x: &str, y: &str) -> bool {
        lines
            .iter()
            .any(|l| l.contains(&format!("X{x} Y{y}")) && (l.starts_with("G0") || l.contains("S0")))
    }

    #[test]
    fn hull_frame_follows_the_triangle_not_the_rectangle() {
        let d = DeviceProfile::default();
        let g = hull_lines(&triangle_doc(), &d, &JobOptions::default(), 3000.0, None).unwrap();
        assert_eq!(g.first().map(String::as_str), Some("G90"));
        assert!(visits(&g, "10", "10"), "{g:?}");
        assert!(visits(&g, "50", "10"), "{g:?}");
        assert!(visits(&g, "10", "40"), "{g:?}");
        // La esquina vacía del rectángulo no se recorre.
        assert!(!visits(&g, "50", "40"), "{g:?}");
        assert!(g.iter().all(|l| !l.contains("M3") && !l.contains("M4")));
    }

    #[test]
    fn hull_frame_uses_the_machine_origin_of_the_bed() {
        let d = DeviceProfile {
            origin: Origin::TopRight,
            bed: [100.0, 80.0],
            ..DeviceProfile::default()
        };
        let g = hull_lines(&triangle_doc(), &d, &JobOptions::default(), 3000.0, None).unwrap();
        // (10,10) del documento es (90,70) en una máquina con el cero arriba a la derecha.
        assert!(visits(&g, "90", "70"), "{g:?}");
        assert!(visits(&g, "50", "70"), "{g:?}");
        assert!(visits(&g, "90", "40"), "{g:?}");
    }

    #[test]
    fn hull_frame_rejects_bad_speed_empty_jobs_and_designs_outside_the_bed() {
        let d = DeviceProfile::default();
        let o = JobOptions::default();
        assert!(hull_lines(&triangle_doc(), &d, &o, 0.0, None).is_err());
        assert!(hull_lines(&Document::default(), &d, &o, 1000.0, None).is_err());
        let small = DeviceProfile {
            bed: [20.0, 20.0],
            ..DeviceProfile::default()
        };
        let e = hull_lines(&triangle_doc(), &small, &o, 1000.0, None).unwrap_err();
        assert!(e.contains("se sale del área de trabajo"), "{e}");
    }
}
