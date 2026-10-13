//! Efectos sobre el texto ya dispuesto: curvado (Bend) y soldado (Welded).

use tlaku_core::bezpath::{BezPath, PathSeg};
use tlaku_core::{Point, Polyline};

/// Tolerancia al aplanar las curvas antes de deformarlas o soldarlas (mm).
const FLATTEN_TOL_MM: f64 = 0.005;
/// Paso angular máximo entre puntos de un segmento recto al curvarlo (rad, ≈ 1,1°).
const BEND_STEP_RAD: f64 = 0.02;
/// Curvatura máxima: una vuelta completa.
pub const MAX_BEND_DEG: f64 = 360.0;
/// Curvaturas menores que esto (grados) se tratan como cero.
const MIN_BEND_DEG: f64 = 1e-6;

/// Radio del arco (mm) para un bloque de `width_mm` de ancho que abarca `bend_deg` grados.
/// Positivo: el centro del arco queda debajo del texto (el texto sube en el centro, como un
/// arco iris); negativo: queda encima (sonrisa).
pub fn bend_radius(width_mm: f64, bend_deg: f64) -> Option<f64> {
    if bend_deg.abs() < MIN_BEND_DEG || width_mm.is_nan() || width_mm <= 0.0 {
        return None;
    }
    Some(width_mm / bend_deg.to_radians())
}

/// Curva `path` sobre un arco de radio `radius` (con signo, ver [`bend_radius`]) alrededor de
/// `x_mid`. Un punto `(x, y)` pasa a la circunferencia de radio `radius + y`, en el ángulo
/// `(x - x_mid) / radius`; la línea base del centro del bloque queda en `y = 0`.
///
/// Las curvas se aplanan y los tramos rectos se subdividen para que el arco salga redondo.
pub fn bend_path(path: &BezPath, x_mid: f64, radius: f64) -> BezPath {
    let mut out = Vec::new();
    let map = |p: Point| {
        let theta = (p.x - x_mid) / radius;
        // Si la altura se come el radio, se aplasta contra el centro en vez de invertirse.
        let eps = 1e-6 * radius.abs().max(1.0);
        let rho = if radius > 0.0 {
            (radius + p.y).max(eps)
        } else {
            (radius + p.y).min(-eps)
        };
        Point::new(x_mid + rho * theta.sin(), rho * theta.cos() - radius)
    };
    for line in path.flatten(FLATTEN_TOL_MM) {
        let n = line.points.len();
        let count = if line.closed { n } else { n - 1 };
        let mut pts: Vec<Point> = Vec::with_capacity(n);
        pts.push(map(line.points[0]));
        for i in 0..count {
            let (a, b) = (line.points[i], line.points[(i + 1) % n]);
            let steps = (((b.x - a.x).abs() / radius.abs()) / BEND_STEP_RAD).ceil() as usize;
            let steps = steps.clamp(1, 4096);
            for k in 1..=steps {
                let t = k as f64 / steps as f64;
                let q = Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
                // El último tramo de un contorno cerrado vuelve al punto inicial: no se repite.
                if line.closed && i == count - 1 && k == steps {
                    break;
                }
                pts.push(map(q));
            }
        }
        out.extend(
            BezPath::from(&Polyline {
                points: pts,
                closed: line.closed,
            })
            .0,
        );
    }
    BezPath(out)
}

/// Une los contornos cerrados que se tocan o se solapan en uno solo. Cada resultado es un
/// exterior seguido de sus agujeros (un `BezPath`). Las trayectorias abiertas se devuelven
/// tal cual al final: el soldado solo tiene sentido con contornos.
pub fn weld_paths(paths: &[BezPath]) -> Vec<BezPath> {
    let mut closed: Vec<Polyline> = Vec::new();
    let mut open: Vec<BezPath> = Vec::new();
    for p in paths {
        for line in p.flatten(FLATTEN_TOL_MM) {
            if line.closed {
                closed.push(line);
            } else {
                open.push(BezPath::from(&line));
            }
        }
    }
    let welded = tlaku_geom::weld(&closed);
    // `weld` entrega cada exterior (antihorario) seguido de sus agujeros (horarios).
    let mut groups: Vec<BezPath> = Vec::new();
    for poly in &welded {
        let path = BezPath::from(poly);
        if tlaku_geom::signed_area(&poly.points) > 0.0 || groups.is_empty() {
            groups.push(path);
        } else if let Some(last) = groups.last_mut() {
            last.0.extend(path.0);
        }
    }
    groups.extend(open);
    groups
}

/// ¿Alguno de los trazos es un contorno cerrado?
pub fn has_closed(paths: &[BezPath]) -> bool {
    paths
        .iter()
        .any(|p| p.0.iter().any(|s| matches!(s, PathSeg::Close)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> BezPath {
        BezPath::from(&Polyline {
            points: vec![
                Point::new(x0, y0),
                Point::new(x1, y0),
                Point::new(x1, y1),
                Point::new(x0, y1),
            ],
            closed: true,
        })
    }

    #[test]
    fn radius_sign_follows_bend_sign() {
        assert_eq!(bend_radius(10.0, 0.0), None);
        let r = bend_radius(10.0, 90.0).unwrap();
        assert!((r - 10.0 / std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        assert!(bend_radius(10.0, -90.0).unwrap() < 0.0);
    }

    #[test]
    fn welding_merges_touching_rectangles_only() {
        let touching = [rect(0.0, 0.0, 2.0, 1.0), rect(1.0, 0.0, 3.0, 1.0)];
        assert_eq!(weld_paths(&touching).len(), 1);
        let apart = [rect(0.0, 0.0, 1.0, 1.0), rect(5.0, 0.0, 6.0, 1.0)];
        assert_eq!(weld_paths(&apart).len(), 2);
    }

    #[test]
    fn welding_keeps_holes_with_their_outline() {
        let ring = BezPath(
            rect(0.0, 0.0, 10.0, 10.0)
                .0
                .into_iter()
                .chain(rect(3.0, 3.0, 7.0, 7.0).0)
                .collect(),
        );
        let out = weld_paths(&[ring, rect(9.0, 4.0, 14.0, 6.0)]);
        assert_eq!(out.len(), 1);
        let moves = out[0]
            .0
            .iter()
            .filter(|s| matches!(s, PathSeg::MoveTo { .. }))
            .count();
        assert_eq!(moves, 2, "contorno + agujero");
    }

    #[test]
    fn bend_keeps_the_center_and_lifts_the_ends_down_for_positive_radius() {
        // Línea de 20 mm centrada en x = 0, sobre un arco de radio 10 (positivo).
        let line = BezPath::from(&Polyline {
            points: vec![Point::new(-10.0, 0.0), Point::new(10.0, 0.0)],
            closed: false,
        });
        let b = bend_path(&line, 0.0, 10.0);
        let pts: Vec<Point> = b
            .flatten(0.001)
            .into_iter()
            .flat_map(|l| l.points)
            .collect();
        let mid = pts
            .iter()
            .min_by(|a, b| a.x.abs().total_cmp(&b.x.abs()))
            .unwrap();
        assert!(mid.y.abs() < 0.01);
        // Todos los puntos están a 10 mm del centro (0, -10).
        for p in &pts {
            assert!((p.x.hypot(p.y + 10.0) - 10.0).abs() < 1e-6, "{p:?}");
        }
        assert!(pts.iter().all(|p| p.y <= 1e-9));
    }

    #[test]
    fn negative_radius_curves_the_other_way() {
        let line = BezPath::from(&Polyline {
            points: vec![Point::new(-5.0, 2.0), Point::new(5.0, 2.0)],
            closed: false,
        });
        let b = bend_path(&line, 0.0, -10.0);
        let pts: Vec<Point> = b
            .flatten(0.001)
            .into_iter()
            .flat_map(|l| l.points)
            .collect();
        // Centro del arco en (0, 10); la línea a y = 2 queda a 8 mm de él.
        for p in &pts {
            assert!((p.x.hypot(p.y - 10.0) - 8.0).abs() < 1e-6, "{p:?}");
        }
    }
}
