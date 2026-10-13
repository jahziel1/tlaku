//! Contorno envolvente de un conjunto de puntos ("banda elástica"): casco convexo y casco cóncavo.
//!
//! El casco cóncavo parte del convexo y lo "excava": toda arista más larga que `max_edge` se
//! cambia por dos aristas hacia un punto interior, el que ve la arista bajo el mayor ángulo (el
//! vecino de Delaunay de la arista, como en las formas alfa). Se repite hasta que ninguna arista
//! larga pueda excavarse. Con `max_edge` muy grande el resultado es el casco convexo.

use crate::util::signed_area;
use tlaku_core::Point;

/// Tope de puntos interiores que se consideran al excavar (más allá se submuestrea).
const MAX_INTERIOR: usize = 4000;

fn cross(a: Point, b: Point, c: Point) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// Casco convexo (cadena monótona de Andrew), antihorario, sin puntos repetidos ni alineados.
/// Ignora los puntos que no sean finitos. Con todos los puntos iguales devuelve uno; con todos
/// alineados, los dos extremos.
pub fn convex_hull(points: &[Point]) -> Vec<Point> {
    let mut pts: Vec<Point> = points
        .iter()
        .copied()
        .filter(|p| p.x.is_finite() && p.y.is_finite())
        .collect();
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    pts.dedup_by(|a, b| a.x == b.x && a.y == b.y);
    if pts.len() < 3 {
        return pts;
    }
    let mut hull: Vec<Point> = Vec::with_capacity(pts.len());
    let push = |p: Point, min_len: usize, hull: &mut Vec<Point>| {
        while hull.len() >= min_len && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
            hull.pop();
        }
        hull.push(p);
    };
    for &p in &pts {
        push(p, 2, &mut hull);
    }
    let lower = hull.len() + 1;
    for &p in pts.iter().rev().skip(1) {
        push(p, lower, &mut hull);
    }
    hull.pop();
    hull
}

/// ¿Se cruzan (propiamente) los segmentos `ab` y `cd`? Compartir un extremo no cuenta.
fn properly_cross(a: Point, b: Point, c: Point, d: Point) -> bool {
    let (d1, d2) = (cross(a, b, c), cross(a, b, d));
    let (d3, d4) = (cross(c, d, a), cross(c, d, b));
    ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
}

/// Ángulo en `p` bajo el que se ve la arista `ab`.
fn angle_at(p: Point, a: Point, b: Point) -> f64 {
    let (u, v) = ((a.x - p.x, a.y - p.y), (b.x - p.x, b.y - p.y));
    let dot = u.0 * v.0 + u.1 * v.1;
    let cr = u.0 * v.1 - u.1 * v.0;
    cr.abs().atan2(dot)
}

/// Casco cóncavo con aristas de a lo sumo `max_edge` mm donde se pueda (ver la nota del módulo).
/// `max_edge <= 0` o no finito devuelve el casco convexo. Siempre antihorario.
pub fn concave_hull(points: &[Point], max_edge: f64) -> Vec<Point> {
    let mut poly = convex_hull(points);
    if poly.len() < 3 || !(max_edge.is_finite() && max_edge > 0.0) {
        return poly;
    }
    let on_hull = |p: &Point, poly: &[Point]| poly.iter().any(|q| q == p);
    let mut interior: Vec<Point> = points
        .iter()
        .copied()
        .filter(|p| p.x.is_finite() && p.y.is_finite() && !on_hull(p, &poly))
        .collect();
    interior.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    interior.dedup();
    if interior.len() > MAX_INTERIOR {
        let step = interior.len().div_ceil(MAX_INTERIOR);
        interior = interior.into_iter().step_by(step).collect();
    }

    // Aristas largas que ya no se pueden excavar (se identifican por sus extremos).
    let mut stuck: Vec<(Point, Point)> = Vec::new();
    loop {
        // La arista más larga que aún se pueda intentar.
        let n = poly.len();
        let pick = (0..n)
            .map(|i| (i, poly[i].dist(poly[(i + 1) % n])))
            .filter(|&(i, len)| len > max_edge && !stuck.contains(&(poly[i], poly[(i + 1) % n])))
            .max_by(|a, b| a.1.total_cmp(&b.1));
        let Some((i, len)) = pick else { break };
        let (a, b) = (poly[i], poly[(i + 1) % n]);

        // Candidatos a la izquierda de a->b (el interior en un contorno antihorario), mejores primero.
        let mut cands: Vec<(f64, usize)> = interior
            .iter()
            .enumerate()
            .filter(|(_, p)| cross(a, b, **p) > 0.0 && p.dist(a) < len && p.dist(b) < len)
            .map(|(k, p)| (angle_at(*p, a, b), k))
            .collect();
        cands.sort_by(|x, y| y.0.total_cmp(&x.0));

        let chosen = cands.into_iter().find(|&(_, k)| {
            let p = interior[k];
            // Las dos aristas nuevas no pueden cruzar el resto del contorno.
            (0..n).all(|j| {
                let (c, d) = (poly[j], poly[(j + 1) % n]);
                j == i || !(properly_cross(a, p, c, d) || properly_cross(p, b, c, d))
            })
        });
        match chosen {
            Some((_, k)) => {
                let p = interior.swap_remove(k);
                poly.insert(i + 1, p);
            }
            None => stuck.push((a, b)),
        }
    }
    if signed_area(&poly) < 0.0 {
        poly.reverse();
    }
    poly
}
