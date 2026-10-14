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
use tlaku_doc::{DeviceProfile, Document, JobOptions, RotaryKind};
use tlaku_grbl::frame::frame_hull_gcode;
use tlaku_i18n::tr;

/// Pasa un punto del documento (origen abajo a la izquierda) a las coordenadas de la máquina:
/// esquina de origen de la cama e intercambio X/Y del perfil (igual que el G-code del trabajo).
fn to_machine(p: Point, device: &DeviceProfile) -> Point {
    tlaku_pipeline::doc_to_machine(p, device)
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

/// G-code del encuadre del casco convexo del trabajo (láser apagado), listo para transmitir. Con
/// `pointer`, es el puntero rojo el que recorre el casco (la cabeza va desfasada
/// `DeviceProfile::pointer_offset`) y falla si eso la saca del área de la máquina.
pub fn hull_lines(
    doc: &Document,
    device: &DeviceProfile,
    options: &JobOptions,
    speed: f64,
    head: Head,
    pointer: bool,
) -> Result<Vec<String>, String> {
    if !(speed.is_finite() && speed > 0.0) {
        return Err(tr!("desktop.common.frame_speed"));
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
    if pointer {
        let shift = tlaku_pipeline::pointer_shift(device);
        let k = y_scale(device);
        let (w, h) = if tlaku_pipeline::swap_active(device) {
            (device.bed[1], device.bed[0])
        } else {
            (device.bed[0], device.bed[1])
        };
        for p in &mut points {
            p.x -= shift.x;
            p.y -= shift.y;
        }
        let eps = 1e-6;
        if points
            .iter()
            .any(|p| p.x < -eps || p.y < -eps || p.x > w + eps || p.y > h / k + eps)
        {
            return Err(tr!(
                "desktop.frame.pointer_outside_bed",
                name = device.name,
                dx = device.pointer_offset[0],
                dy = device.pointer_offset[1]
            ));
        }
    }
    let lines = frame_hull_gcode(&points, speed, y_scale(device));
    if lines.is_empty() {
        return Err(tr!("desktop.common.nothing_to_frame"));
    }
    Ok(lines)
}

/// Potencia máxima del encuadre con el láser encendido, en % de la S máx. Es solo una guía
/// visible: no debe marcar el material.
pub const FRAME_MAX_PCT: f64 = 20.0;

/// Convierte las líneas de un encuadre (con `S0` en cada `G1` y el láser apagado) en un encuadre con
/// el láser a `power_pct` % de la S máx: enciende el láser (la orden `constant_on` del dialecto,
/// con `S0`) justo antes del primer `G1`, así el desplazamiento hasta la primera esquina sigue
/// siendo con el láser apagado, y cambia el `S0` de los `G1` por la potencia pedida. El `M5` final
/// de las líneas originales apaga el láser. Falla si el dialecto no lleva la potencia dentro de
/// cada movimiento (no hay forma de apagar el láser en el traslado) o si pasa del límite.
pub fn low_power(
    lines: Vec<String>,
    device: &DeviceProfile,
    power_pct: f64,
) -> Result<Vec<String>, String> {
    if !(power_pct.is_finite() && power_pct > 0.0 && power_pct <= FRAME_MAX_PCT) {
        return Err(tr!("desktop.frame.power_range", max = FRAME_MAX_PCT));
    }
    let d = &device.dialect;
    if !d.s_inline || d.constant_on.trim().is_empty() {
        return Err(tr!("desktop.frame.dialect_no_low_power"));
    }
    let s = device.s_max * power_pct / 100.0;
    if !(s.is_finite() && s > 0.0) {
        return Err(tr!("desktop.common.s_max_positive"));
    }
    let s_text = {
        let t = format!("{s:.*}", usize::from(d.s_decimals));
        if t.contains('.') {
            t.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            t
        }
    };
    let mut out = Vec::with_capacity(lines.len() + 1);
    let mut lit = false;
    for line in lines {
        if line.starts_with("G1") {
            if !lit {
                out.push(d.constant_on.clone());
                lit = true;
            }
            let replaced: Vec<String> = line
                .split(' ')
                .map(|w| {
                    if w == "S0" {
                        format!("S{s_text}")
                    } else {
                        w.to_string()
                    }
                })
                .collect();
            out.push(replaced.join(" "));
        } else {
            out.push(line);
        }
    }
    if !lit {
        return Err(tr!("desktop.common.nothing_to_frame"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_core::bezpath::BezPath;
    use tlaku_core::{Polyline, Rgb};
    use tlaku_doc::affine::IDENTITY;
    use tlaku_doc::{DesignObject, LayerSettings, ObjectKind, Origin, VectorObject};

    /// Un documento con un triángulo rectángulo cerrado en la capa roja.
    fn triangle_doc() -> Document {
        let red = Rgb(255, 0, 0);
        Document {
            objects: vec![DesignObject {
                id: "t".into(),
                layer: red,
                transform: IDENTITY,
                kind: ObjectKind::Vector(VectorObject::new(vec![BezPath::from(&Polyline {
                    points: vec![
                        Point::new(10.0, 10.0),
                        Point::new(50.0, 10.0),
                        Point::new(10.0, 40.0),
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
        let g = hull_lines(
            &triangle_doc(),
            &d,
            &JobOptions::default(),
            3000.0,
            None,
            false,
        )
        .unwrap();
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
        let g = hull_lines(
            &triangle_doc(),
            &d,
            &JobOptions::default(),
            3000.0,
            None,
            false,
        )
        .unwrap();
        // (10,10) del documento es (90,70) en una máquina con el cero arriba a la derecha.
        assert!(visits(&g, "90", "70"), "{g:?}");
        assert!(visits(&g, "50", "70"), "{g:?}");
        assert!(visits(&g, "90", "40"), "{g:?}");
    }

    #[test]
    fn hull_frame_with_the_red_pointer_moves_the_head_by_its_offset() {
        let d = DeviceProfile {
            bed: [100.0, 80.0],
            pointer_offset: [4.0, 2.0],
            ..DeviceProfile::default()
        };
        let o = JobOptions::default();
        // El puntero cae 4 mm a la derecha y 2 arriba del rayo: para que pase por (10, 10) la
        // cabeza va a (6, 8). Sin `pointer` el recorrido es el del rayo.
        let g = hull_lines(&triangle_doc(), &d, &o, 3000.0, None, true).unwrap();
        assert!(visits(&g, "6", "8"), "{g:?}");
        assert!(visits(&g, "46", "8"), "{g:?}");
        let beam = hull_lines(&triangle_doc(), &d, &o, 3000.0, None, false).unwrap();
        assert!(visits(&beam, "10", "10"), "{beam:?}");
        // Si el puntero obliga a salirse de la cama, falla en vez de provocar ALARM:2.
        let edge = DeviceProfile {
            pointer_offset: [15.0, 0.0],
            ..d
        };
        let e = hull_lines(&triangle_doc(), &edge, &o, 3000.0, None, true).unwrap_err();
        assert!(e.contains("puntero rojo"), "{e}");
    }

    #[test]
    fn hull_frame_rejects_bad_speed_empty_jobs_and_designs_outside_the_bed() {
        let d = DeviceProfile::default();
        let o = JobOptions::default();
        assert!(hull_lines(&triangle_doc(), &d, &o, 0.0, None, false).is_err());
        assert!(hull_lines(&Document::default(), &d, &o, 1000.0, None, false).is_err());
        let small = DeviceProfile {
            bed: [20.0, 20.0],
            ..DeviceProfile::default()
        };
        let e = hull_lines(&triangle_doc(), &small, &o, 1000.0, None, false).unwrap_err();
        assert!(e.contains("se sale del área de trabajo"), "{e}");
    }

    #[test]
    fn low_power_frame_lights_the_laser_only_for_the_edges() {
        let d = DeviceProfile::default();
        let base = tlaku_grbl::frame::frame_gcode(
            tlaku_core::Bounds {
                min: Point::new(0.0, 0.0),
                max: Point::new(10.0, 5.0),
            },
            1500.0,
        );
        let g = low_power(base.clone(), &d, 2.0).unwrap();
        let first_g1 = g.iter().position(|l| l.starts_with("G1")).unwrap();
        // El encendido va justo antes del primer G1, después del G0 de aproximación.
        assert_eq!(g[first_g1 - 1], "M3 S0");
        assert!(g.iter().position(|l| l.starts_with("G0")).unwrap() < first_g1);
        // S máx. 1000 y 2 % -> S20 en cada lado; ninguno queda en S0.
        let sides: Vec<_> = g.iter().filter(|l| l.starts_with("G1")).collect();
        assert_eq!(sides.len(), 4);
        assert!(sides.iter().all(|l| l.contains(" S20 ")), "{sides:?}");
        assert_eq!(g.last().map(String::as_str), Some("M5"));
        // Pasarse del límite, S inline apagado o un trabajo vacío fallan sin devolver G-code.
        assert!(low_power(base.clone(), &d, 50.0).is_err());
        assert!(low_power(base.clone(), &d, 0.0).is_err());
        assert!(low_power(vec!["M5".into()], &d, 2.0).is_err());
        let mut no_inline = d.clone();
        no_inline.dialect.s_inline = false;
        assert!(low_power(base, &no_inline, 2.0).is_err());
    }
}
