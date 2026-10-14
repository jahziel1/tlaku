//! Círculos tangentes a dos o tres formas (rectas, círculos o contornos cualquiera).
//!
//! Un círculo de centro `c` y radio `r` es tangente a una forma cuando su centro está exactamente a
//! distancia `r` del contorno: así toca sin cruzar. Con eso:
//!
//! - **Dos formas y un radio.** El centro está a distancia `r` de las dos. El lugar de los puntos
//!   a distancia `r` de un tramo recto son dos paralelas y dos arcos (círculos de radio `r` en
//!   sus extremos); los centros son las intersecciones de esas piezas entre las dos formas, y se
//!   conservan solo las que de verdad quedan a distancia `r` de ambos contornos.
//! - **Tres formas.** El radio sale de las formas (problema de Apolonio): se siembra con las
//!   soluciones de dos formas para varios radios y se refina con Newton hasta que el centro
//!   equidista de las tres.
//!
//! Cubre los círculos que quedan fuera de las formas o dentro de una cerrada, tocándolas por
//! fuera; no incluye el círculo que contiene a una forma tocándola desde dentro.

use tlaku_core::{Point, Polyline};
use tlaku_i18n::tr;

/// Círculo tangente.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circle {
    pub center: Point,
    pub radius: f64,
}

/// Tope de círculos que se devuelven.
pub const MAX_CIRCLES: usize = 24;
/// Producto máximo de tramos entre dos formas (más allá, el cálculo tarda demasiado).
const MAX_PAIRS: usize = 40_000_000;
/// Radios con que se siembra el caso de tres formas.
const SEED_RADII: usize = 64;
/// Semillas que se refinan con Newton.
const MAX_SEEDS: usize = 300;

type Seg = (Point, Point);

fn segments(shape: &[Polyline]) -> Vec<Seg> {
    let mut out = Vec::new();
    for p in shape {
        let n = p.points.len();
        if n < 2 {
            continue;
        }
        for i in 0..n - 1 {
            out.push((p.points[i], p.points[i + 1]));
        }
        if p.closed && n > 2 {
            out.push((p.points[n - 1], p.points[0]));
        }
    }
    out.retain(|(a, b)| a.dist(*b) > 1e-12);
    out
}

fn dist_point_seg(p: Point, (a, b): Seg) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    p.dist(Point::new(a.x + t * dx, a.y + t * dy))
}

fn dist_to(p: Point, segs: &[Seg]) -> f64 {
    segs.iter()
        .map(|&s| dist_point_seg(p, s))
        .fold(f64::INFINITY, f64::min)
}

/// Pieza del lugar de los puntos a distancia `r` de una forma.
#[derive(Clone, Copy)]
enum Piece {
    Seg(Point, Point),
    Circ(Point),
}

fn pieces(segs: &[Seg], r: f64) -> Vec<(Piece, [f64; 4])> {
    let mut out = Vec::with_capacity(segs.len() * 3);
    let bbox = |pts: &[Point], pad: f64| {
        let mut b = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        for p in pts {
            b = [
                b[0].min(p.x - pad),
                b[1].min(p.y - pad),
                b[2].max(p.x + pad),
                b[3].max(p.y + pad),
            ];
        }
        b
    };
    for &(a, b) in segs {
        let len = a.dist(b);
        let (nx, ny) = (-(b.y - a.y) / len * r, (b.x - a.x) / len * r);
        for s in [1.0, -1.0] {
            let (pa, pb) = (
                Point::new(a.x + nx * s, a.y + ny * s),
                Point::new(b.x + nx * s, b.y + ny * s),
            );
            out.push((Piece::Seg(pa, pb), bbox(&[pa, pb], 0.0)));
        }
    }
    // Un círculo por vértice distinto (los cerrados los repiten).
    let mut verts: Vec<Point> = segs.iter().flat_map(|&(a, b)| [a, b]).collect();
    verts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    verts.dedup();
    out.extend(verts.into_iter().map(|v| (Piece::Circ(v), bbox(&[v], r))));
    out
}

fn overlap(a: &[f64; 4], b: &[f64; 4]) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3]
}

/// Intersección de dos segmentos (con un pequeño margen en los extremos).
fn seg_seg(a: Point, b: Point, c: Point, d: Point, out: &mut Vec<Point>) {
    let (r, s) = ((b.x - a.x, b.y - a.y), (d.x - c.x, d.y - c.y));
    let den = r.0 * s.1 - r.1 * s.0;
    if den.abs() < 1e-14 {
        return;
    }
    let (qp0, qp1) = (c.x - a.x, c.y - a.y);
    let t = (qp0 * s.1 - qp1 * s.0) / den;
    let u = (qp0 * r.1 - qp1 * r.0) / den;
    let eps = 1e-9;
    if (-eps..=1.0 + eps).contains(&t) && (-eps..=1.0 + eps).contains(&u) {
        out.push(Point::new(a.x + t * r.0, a.y + t * r.1));
    }
}

/// Intersección de un segmento con el círculo de centro `c` y radio `r`.
fn seg_circ(a: Point, b: Point, c: Point, r: f64, out: &mut Vec<Point>) {
    let d = (b.x - a.x, b.y - a.y);
    let f = (a.x - c.x, a.y - c.y);
    let (qa, qb, qc) = (
        d.0 * d.0 + d.1 * d.1,
        2.0 * (f.0 * d.0 + f.1 * d.1),
        f.0 * f.0 + f.1 * f.1 - r * r,
    );
    if qa < 1e-18 {
        return;
    }
    let disc = qb * qb - 4.0 * qa * qc;
    if disc < 0.0 {
        return;
    }
    let sq = disc.sqrt();
    let eps = 1e-9;
    for t in [(-qb - sq) / (2.0 * qa), (-qb + sq) / (2.0 * qa)] {
        if (-eps..=1.0 + eps).contains(&t) {
            out.push(Point::new(a.x + t * d.0, a.y + t * d.1));
        }
    }
}

/// Intersección de dos círculos del mismo radio `r`.
fn circ_circ(c1: Point, c2: Point, r: f64, out: &mut Vec<Point>) {
    let d = c1.dist(c2);
    if d < 1e-12 || d > 2.0 * r {
        return;
    }
    let h = (r * r - d * d / 4.0).max(0.0).sqrt();
    let (mx, my) = ((c1.x + c2.x) / 2.0, (c1.y + c2.y) / 2.0);
    let (ux, uy) = ((c2.x - c1.x) / d, (c2.y - c1.y) / d);
    out.push(Point::new(mx - uy * h, my + ux * h));
    out.push(Point::new(mx + uy * h, my - ux * h));
}

/// Candidatos a centro de un círculo de radio `r` tangente a las dos formas (sin filtrar).
fn pair_centers(a: &[Seg], b: &[Seg], r: f64) -> Vec<Point> {
    let (pa, pb) = (pieces(a, r), pieces(b, r));
    let mut out = Vec::new();
    for (x, bx) in &pa {
        for (y, by) in &pb {
            if !overlap(bx, by) {
                continue;
            }
            match (x, y) {
                (Piece::Seg(a1, a2), Piece::Seg(b1, b2)) => seg_seg(*a1, *a2, *b1, *b2, &mut out),
                (Piece::Seg(a1, a2), Piece::Circ(c)) | (Piece::Circ(c), Piece::Seg(a1, a2)) => {
                    seg_circ(*a1, *a2, *c, r, &mut out);
                }
                (Piece::Circ(c1), Piece::Circ(c2)) => circ_circ(*c1, *c2, r, &mut out),
            }
        }
    }
    out
}

/// Junta los puntos que distan menos de `tol` y devuelve el promedio de cada grupo.
fn cluster(points: Vec<Point>, tol: f64) -> Vec<Point> {
    let mut groups: Vec<(Point, usize)> = Vec::new();
    for p in points {
        match groups.iter_mut().find(|(c, _)| c.dist(p) <= tol) {
            Some((c, n)) => {
                let k = *n as f64;
                *c = Point::new((c.x * k + p.x) / (k + 1.0), (c.y * k + p.y) / (k + 1.0));
                *n += 1;
            }
            None => groups.push((p, 1)),
        }
    }
    groups.into_iter().map(|(c, _)| c).collect()
}

fn check_size(a: &[Seg], b: &[Seg]) -> Result<(), String> {
    if a.len().saturating_mul(b.len()) > MAX_PAIRS {
        return Err(tr!("geom.tangent.too_detailed"));
    }
    Ok(())
}

fn two_shapes(a: &[Seg], b: &[Seg], r: f64, scale: f64) -> Result<Vec<Circle>, String> {
    check_size(a, b)?;
    let eps = 1e-7 * scale.max(1.0);
    let valid: Vec<Point> = pair_centers(a, b, r)
        .into_iter()
        .filter(|c| (dist_to(*c, a) - r).abs() <= eps && (dist_to(*c, b) - r).abs() <= eps)
        .collect();
    let tol = (0.05 * r).clamp(0.02, 0.5);
    Ok(cluster(valid, tol)
        .into_iter()
        .map(|center| Circle { center, radius: r })
        .collect())
}

/// Resuelve 3x3 `m · x = rhs` (eliminación gaussiana con pivoteo parcial).
fn solve3(mut m: [[f64; 3]; 3], mut rhs: [f64; 3]) -> Option<[f64; 3]> {
    for col in 0..3 {
        let piv = (col..3).max_by(|&i, &j| m[i][col].abs().total_cmp(&m[j][col].abs()))?;
        if m[piv][col].abs() < 1e-14 {
            return None;
        }
        m.swap(col, piv);
        rhs.swap(col, piv);
        for row in col + 1..3 {
            let f = m[row][col] / m[col][col];
            let pivot_row = m[col];
            for (cell, p) in m[row].iter_mut().zip(pivot_row).skip(col) {
                *cell -= f * p;
            }
            rhs[row] -= f * rhs[col];
        }
    }
    let mut x = [0.0; 3];
    for row in (0..3).rev() {
        let s: f64 = (row + 1..3).map(|k| m[row][k] * x[k]).sum();
        x[row] = (rhs[row] - s) / m[row][row];
    }
    Some(x)
}

/// Newton sobre `(cx, cy, r)` para `dist_i(c) = r` en las tres formas.
fn refine(shapes: &[Vec<Seg>; 3], seed: Circle, scale: f64) -> Option<Circle> {
    let f = |x: [f64; 3]| -> [f64; 3] {
        let c = Point::new(x[0], x[1]);
        std::array::from_fn(|i| dist_to(c, &shapes[i]) - x[2])
    };
    let h = 1e-6 * scale.max(1.0);
    let mut x = [seed.center.x, seed.center.y, seed.radius];
    for _ in 0..60 {
        let fx = f(x);
        if fx.iter().all(|v| v.abs() < 1e-9 * scale.max(1.0)) {
            return (x[2] > 1e-9).then_some(Circle {
                center: Point::new(x[0], x[1]),
                radius: x[2],
            });
        }
        let mut jac = [[0.0; 3]; 3];
        for col in 0..3 {
            let (mut hi, mut lo) = (x, x);
            hi[col] += h;
            lo[col] -= h;
            let (fh, fl) = (f(hi), f(lo));
            for row in 0..3 {
                jac[row][col] = (fh[row] - fl[row]) / (2.0 * h);
            }
        }
        let step = solve3(jac, [-fx[0], -fx[1], -fx[2]])?;
        // Un paso demasiado grande suele ser un salto a otra solución o fuera de rango.
        let norm = step.iter().map(|v| v * v).sum::<f64>().sqrt();
        let k = if norm > 2.0 * scale {
            2.0 * scale / norm
        } else {
            1.0
        };
        for i in 0..3 {
            x[i] += step[i] * k;
        }
        if !x.iter().all(|v| v.is_finite()) || x[2] <= 0.0 {
            return None;
        }
    }
    None
}

fn three_shapes(shapes: &[Vec<Seg>; 3], scale: f64) -> Result<Vec<Circle>, String> {
    for (i, j) in [(0, 1), (0, 2), (1, 2)] {
        check_size(&shapes[i], &shapes[j])?;
    }
    let (rmin, rmax) = (scale * 1e-3, scale * 2.0);
    // Semillas: soluciones de dos formas (con la tercera como juez), a varios radios.
    let mut seeds: Vec<(f64, Circle)> = Vec::new();
    for k in 0..SEED_RADII {
        let r = rmin * (rmax / rmin).powf(k as f64 / (SEED_RADII - 1) as f64);
        for (i, j, other) in [(0, 1, 2), (0, 2, 1), (1, 2, 0)] {
            // Solo sirven los que de verdad equidistan de las dos formas del par.
            let exact: Vec<Point> = pair_centers(&shapes[i], &shapes[j], r)
                .into_iter()
                .filter(|c| {
                    (dist_to(*c, &shapes[i]) - r).abs() < 1e-7 * scale
                        && (dist_to(*c, &shapes[j]) - r).abs() < 1e-7 * scale
                })
                .collect();
            for center in cluster(exact, (0.05 * r).clamp(0.02, 0.5)) {
                let miss = (dist_to(center, &shapes[other]) - r).abs();
                seeds.push((miss, Circle { center, radius: r }));
            }
        }
    }
    // Las que ya casi equidistan de la tercera forma son las cercanas a una solución.
    seeds.sort_by(|a, b| a.0.total_cmp(&b.0));
    seeds.truncate(MAX_SEEDS);
    let seeds: Vec<Circle> = seeds.into_iter().map(|(_, c)| c).collect();
    let mut found: Vec<Circle> = Vec::new();
    for seed in seeds {
        let Some(c) = refine(shapes, seed, scale) else {
            continue;
        };
        let tol = (0.02 * c.radius).clamp(0.01, 0.5);
        if !found
            .iter()
            .any(|o| o.center.dist(c.center) <= tol && (o.radius - c.radius).abs() <= tol)
        {
            found.push(c);
        }
    }
    Ok(found)
}

/// Círculos tangentes a las formas dadas (cada una, un conjunto de trayectos, ya aplanados).
///
/// - Dos formas: `radius` es obligatorio y positivo.
/// - Tres formas: el radio sale de ellas y `radius` se ignora.
///
/// Devuelve hasta [`MAX_CIRCLES`] círculos, de menor a mayor radio y, a igual radio, por posición.
/// Vacío si no hay solución. Error si el número de formas no es 2 ni 3, si falta el radio o si
/// alguna forma no tiene trazo.
pub fn tangent_circles(
    shapes: &[Vec<Polyline>],
    radius: Option<f64>,
) -> Result<Vec<Circle>, String> {
    if !(2..=3).contains(&shapes.len()) {
        return Err(tr!("geom.tangent.select"));
    }
    let segs: Vec<Vec<Seg>> = shapes.iter().map(|s| segments(s)).collect();
    if segs.iter().any(Vec::is_empty) {
        return Err(tr!("geom.tangent.no_stroke"));
    }
    let all = segs.iter().flatten().flat_map(|(a, b)| [*a, *b]);
    let (mut lo, mut hi) = (
        Point::new(f64::INFINITY, f64::INFINITY),
        Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
    );
    for p in all {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let scale = lo.dist(hi);
    if !scale.is_finite() || scale <= 0.0 {
        return Err(tr!("geom.tangent.no_size"));
    }
    let mut out = if segs.len() == 2 {
        let r = radius.ok_or_else(|| tr!("geom.tangent.no_radius"))?;
        if !(r.is_finite() && r > 0.0) {
            return Err(tr!("geom.tangent.bad_radius"));
        }
        two_shapes(&segs[0], &segs[1], r, scale.max(r))?
    } else {
        three_shapes(&[segs[0].clone(), segs[1].clone(), segs[2].clone()], scale)?
    };
    out.sort_by(|a, b| {
        a.radius
            .total_cmp(&b.radius)
            .then(a.center.x.total_cmp(&b.center.x))
            .then(a.center.y.total_cmp(&b.center.y))
    });
    out.truncate(MAX_CIRCLES);
    Ok(out)
}
