//! Ayudantes de pruebas: dentro/fuera de una región y detección de autointersecciones.

use tlaku_core::{Point, Polyline};

/// ¿El punto está dentro de la región definida por contornos cerrados (regla par-impar)?
pub fn inside(polys: &[Polyline], x: f64, y: f64) -> bool {
    let mut odd = false;
    for p in polys.iter().filter(|p| p.closed) {
        let n = p.points.len();
        for i in 0..n {
            let (a, b) = (p.points[i], p.points[(i + 1) % n]);
            if (a.y > y) != (b.y > y) && x < (b.x - a.x) * (y - a.y) / (b.y - a.y) + a.x {
                odd = !odd;
            }
        }
    }
    odd
}

fn orient(a: Point, b: Point, c: Point) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// ¿Se cruzan (o se tocan) los segmentos `ab` y `cd`?
pub fn segments_touch(a: Point, b: Point, c: Point, d: Point) -> bool {
    let eps = 1e-9;
    let (o1, o2, o3, o4) = (
        orient(a, b, c),
        orient(a, b, d),
        orient(c, d, a),
        orient(c, d, b),
    );
    if ((o1 > eps && o2 < -eps) || (o1 < -eps && o2 > eps))
        && ((o3 > eps && o4 < -eps) || (o3 < -eps && o4 > eps))
    {
        return true;
    }
    let on = |p: Point, q: Point, r: Point| {
        orient(p, q, r).abs() <= eps
            && r.x >= p.x.min(q.x) - eps
            && r.x <= p.x.max(q.x) + eps
            && r.y >= p.y.min(q.y) - eps
            && r.y <= p.y.max(q.y) + eps
    };
    on(a, b, c) || on(a, b, d) || on(c, d, a) || on(c, d, b)
}

/// Segmentos de una trayectoria.
pub fn segments(p: &Polyline) -> Vec<(Point, Point)> {
    let n = p.points.len();
    let last = if p.closed { n } else { n.saturating_sub(1) };
    (0..last)
        .map(|i| (p.points[i], p.points[(i + 1) % n]))
        .collect()
}

/// Cuenta los cruces entre segmentos que no son vecinos, dentro de un conjunto de trayectorias
/// (incluye cruces entre trayectorias distintas). Los vecinos solo comparten su extremo.
pub fn crossings(paths: &[Polyline]) -> usize {
    let mut segs: Vec<(usize, usize, Point, Point, usize)> = Vec::new();
    for (pi, p) in paths.iter().enumerate() {
        let n = segments(p).len();
        for (si, (a, b)) in segments(p).into_iter().enumerate() {
            segs.push((pi, si, a, b, n));
        }
    }
    let mut count = 0;
    for i in 0..segs.len() {
        for j in i + 1..segs.len() {
            let (pi, si, a, b, ni) = segs[i];
            let (pj, sj, c, d, _) = segs[j];
            if pi == pj {
                let closed = paths[pi].closed;
                let adjacent = sj == si + 1 || (closed && si == 0 && sj == ni - 1);
                if adjacent {
                    // Vecinos: solo deben compartir el punto común (no superponerse).
                    let collinear_overlap = orient(a, b, d).abs() < 1e-9
                        && orient(a, b, c).abs() < 1e-9
                        && ((b.x - a.x) * (d.x - c.x) + (b.y - a.y) * (d.y - c.y)) < 0.0;
                    if collinear_overlap {
                        count += 1;
                    }
                    continue;
                }
            }
            if segments_touch(a, b, c, d) {
                count += 1;
            }
        }
    }
    count
}
