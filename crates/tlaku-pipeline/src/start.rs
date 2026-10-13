//! Punto de inicio y sentido elegidos a mano (`DesignObject::start`).
//!
//! Se aplican al colocar el objeto: el contorno cerrado se rota (insertando un vértice si el
//! punto cae a mitad de un segmento) para que empiece donde pidió el usuario, y se invierte si
//! `reverse`. Esas trayectorias quedan marcadas (`PathMeta::locked`) y el planificador no las
//! toca (ver `tlaku_plan::PlanItem::locked`). La compensación de kerf rehace los contornos; ahí
//! [`follow`] vuelve a dejar el nuevo contorno con el inicio y el sentido del original.

use tlaku_core::{Point, Polyline};
use tlaku_doc::StartSpec;
use tlaku_geom::signed_area;

/// Distancia (mm) bajo la cual el punto elegido cuenta como un vértice que ya existe.
const SAME_VERTEX_MM: f64 = 1e-6;

/// Distancia del punto `p` al segmento `a`-`b` y el punto más cercano del segmento.
fn closest_on_segment(p: Point, a: Point, b: Point) -> (f64, Point) {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let q = Point::new(a.x + t * dx, a.y + t * dy);
    (p.dist(q), q)
}

/// Rota el contorno cerrado `path` para que empiece en el punto de su borde más cercano a `p`.
/// Si ese punto cae dentro de un segmento se agrega como vértice nuevo.
fn rotate_to(path: &mut Polyline, p: Point) {
    let pts = &path.points;
    let n = pts.len();
    if n < 2 {
        return;
    }
    let (mut best_seg, mut best_d, mut best_q) = (0, f64::INFINITY, pts[0]);
    for i in 0..n {
        let (d, q) = closest_on_segment(p, pts[i], pts[(i + 1) % n]);
        if d < best_d {
            (best_seg, best_d, best_q) = (i, d, q);
        }
    }
    let mut points = std::mem::take(&mut path.points);
    if let Some(v) = points.iter().position(|v| v.dist(best_q) <= SAME_VERTEX_MM) {
        points.rotate_left(v);
    } else {
        // El punto cae dentro del segmento `best_seg`..`best_seg + 1`.
        points.insert(best_seg + 1, best_q);
        points.rotate_left(best_seg + 1);
    }
    path.points = points;
}

/// Distancia de `p` al borde de `path` (a su polilínea abierta si no es cerrada).
fn distance_to(path: &Polyline, p: Point) -> f64 {
    let pts = &path.points;
    let n = pts.len();
    if n == 1 {
        return p.dist(pts[0]);
    }
    let count = if path.closed { n } else { n - 1 };
    (0..count)
        .map(|i| closest_on_segment(p, pts[i], pts[(i + 1) % n]).0)
        .fold(f64::INFINITY, f64::min)
}

/// Invierte el sentido de `path`; en una cerrada conserva el primer vértice como inicio.
fn reverse(path: &mut Polyline) {
    if path.closed {
        path.points[1..].reverse();
    } else {
        path.points.reverse();
    }
}

/// Aplica `spec` a las trayectorias de un objeto y devuelve, para cada una, si quedó fijada.
///
/// - `reverse` invierte todas las trayectorias del objeto, que quedan fijadas.
/// - `point` rota a ese punto **una** cerrada: la de borde más cercano. Esa también queda fijada.
///
/// Sin `spec`, nada cambia y ninguna queda fijada.
pub(crate) fn lock_paths(paths: &mut [Polyline], spec: Option<&StartSpec>) -> Vec<bool> {
    let mut locked = vec![false; paths.len()];
    let Some(spec) = spec else { return locked };
    if spec.reverse {
        for (path, lock) in paths.iter_mut().zip(&mut locked) {
            if path.points.len() >= 2 {
                reverse(path);
                *lock = true;
            }
        }
    }
    let target = spec
        .point
        .filter(|p| p[0].is_finite() && p[1].is_finite())
        .map(|p| Point::new(p[0], p[1]));
    if let Some(p) = target {
        let nearest = paths
            .iter()
            .enumerate()
            .filter(|(_, path)| path.closed && path.points.len() >= 3)
            .map(|(i, path)| (i, distance_to(path, p)))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((i, _)) = nearest {
            rotate_to(&mut paths[i], p);
            locked[i] = true;
        }
    }
    locked
}

/// Deja `after` (el contorno que resulta de compensar el kerf de `before`) con el sentido de
/// `before` y empezando en su vértice más cercano al inicio de `before`.
pub(crate) fn follow(before: &Polyline, after: &mut Polyline) {
    if !(before.closed && after.closed) || before.points.is_empty() || after.points.len() < 3 {
        return;
    }
    if signed_area(&before.points) * signed_area(&after.points) < 0.0 {
        after.points.reverse();
    }
    let start = before.points[0];
    if let Some(v) = after
        .points
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.dist(start).total_cmp(&b.1.dist(start)))
        .map(|(v, _)| v)
    {
        after.points.rotate_left(v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Polyline {
        Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(10.0, 0.0),
                Point::new(10.0, 10.0),
                Point::new(0.0, 10.0),
            ],
            closed: true,
        }
    }

    fn spec(point: Option<[f64; 2]>, reverse: bool) -> StartSpec {
        StartSpec { point, reverse }
    }

    #[test]
    fn without_spec_nothing_changes() {
        let mut p = [square()];
        assert_eq!(lock_paths(&mut p, None), vec![false]);
        assert_eq!(p[0], square());
    }

    #[test]
    fn a_vertex_becomes_the_start() {
        let mut p = [square()];
        let locked = lock_paths(&mut p, Some(&spec(Some([10.2, 10.1]), false)));
        assert_eq!(locked, vec![true]);
        assert_eq!(p[0].points[0], Point::new(10.0, 10.0));
        assert_eq!(p[0].points.len(), 4);
        assert_eq!(p[0].points[1], Point::new(0.0, 10.0));
    }

    #[test]
    fn a_point_inside_a_segment_adds_a_vertex() {
        let mut p = [square()];
        lock_paths(&mut p, Some(&spec(Some([4.0, -3.0]), false)));
        assert_eq!(p[0].points.len(), 5);
        assert_eq!(p[0].points[0], Point::new(4.0, 0.0));
        assert_eq!(p[0].points[1], Point::new(10.0, 0.0));
        assert_eq!(p[0].points[4], Point::new(0.0, 0.0));
    }

    #[test]
    fn reverse_flips_direction_and_keeps_the_start() {
        let mut p = [square()];
        let locked = lock_paths(&mut p, Some(&spec(None, true)));
        assert_eq!(locked, vec![true]);
        assert_eq!(p[0].points[0], Point::new(0.0, 0.0));
        assert_eq!(p[0].points[1], Point::new(0.0, 10.0));
        assert!(signed_area(&p[0].points) < 0.0);
    }

    #[test]
    fn reverse_and_point_combine() {
        let mut p = [square()];
        lock_paths(&mut p, Some(&spec(Some([10.0, 10.0]), true)));
        assert_eq!(p[0].points[0], Point::new(10.0, 10.0));
        // Al revés: del (10,10) va hacia (10,0).
        assert_eq!(p[0].points[1], Point::new(10.0, 0.0));
    }

    #[test]
    fn only_the_nearest_closed_path_gets_the_point() {
        let mut far = square();
        for q in &mut far.points {
            q.x += 100.0;
        }
        let open = Polyline {
            points: vec![Point::new(0.0, 0.0), Point::new(5.0, 5.0)],
            closed: false,
        };
        let mut p = [far.clone(), open.clone(), square()];
        let locked = lock_paths(&mut p, Some(&spec(Some([10.0, 10.0]), false)));
        assert_eq!(locked, vec![false, false, true]);
        assert_eq!(p[0], far);
        assert_eq!(p[1], open);
        assert_eq!(p[2].points[0], Point::new(10.0, 10.0));
    }

    #[test]
    fn open_paths_only_reverse() {
        let mut p = [Polyline {
            points: vec![Point::new(0.0, 0.0), Point::new(5.0, 0.0)],
            closed: false,
        }];
        let locked = lock_paths(&mut p, Some(&spec(Some([5.0, 0.0]), true)));
        assert_eq!(locked, vec![true]);
        assert_eq!(p[0].points[0], Point::new(5.0, 0.0));
    }

    #[test]
    fn follow_restores_start_and_direction_after_kerf() {
        let before = {
            let mut p = [square()];
            lock_paths(&mut p, Some(&spec(Some([10.0, 10.0]), true)));
            let [p] = p;
            p
        };
        // El kerf lo devuelve en otro orden y sentido (antihorario, empezando en otro vértice).
        let mut after = Polyline {
            points: vec![
                Point::new(-1.0, -1.0),
                Point::new(11.0, -1.0),
                Point::new(11.0, 11.0),
                Point::new(-1.0, 11.0),
            ],
            closed: true,
        };
        follow(&before, &mut after);
        assert_eq!(after.points[0], Point::new(11.0, 11.0));
        assert_eq!(after.points[1], Point::new(11.0, -1.0));
    }
}
