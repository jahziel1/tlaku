//! Revertir un redondeo: devuelve a punta una esquina que `fillet_path` redondeó (o mordió).
//!
//! El motor no guarda de dónde salió una esquina redondeada: el objeto nuevo no lleva su radio ni
//! su vértice original. Se recupera mirando la geometría, y solo cuando no hay duda:
//!
//! - **Redondeo**: una cadena de cúbicas entre dos rectas, que es un arco de círculo tangente a
//!   ambas. Las dos rectas, prolongadas, se cortan en el vértice original.
//! - **Mordisco**: una cadena de cúbicas cuyos puntos están todos a la misma distancia del punto
//!   donde se cortan las dos rectas (un arco centrado en la esquina).
//! - **Chaflán**: NO se revierte. Un tramo recto entre dos rectas es indistinguible de cualquier
//!   lado de un polígono (un octágono regular tiene la misma forma que un cuadrado achaflanado),
//!   así que adivinar daría resultados destructivos. Para ese caso queda el deshacer normal.
//!
//! Tampoco se revierte lo que pasó por otra operación que ya no deja un arco limpio (por ejemplo
//! un nodo movido después): si la cadena no es un arco exacto, no se toca.

use crate::fillet::{Edge, Sub, parse, unit_from};
use crate::split::split_subpaths;
use tlaku_core::Point;
use tlaku_core::bezpath::{BezPath, PathSeg};

/// Seno del menor giro (1 grado) que cuenta como esquina.
const MIN_TURN_SIN: f64 = 0.017;
/// Error relativo admitido para que una cadena de cúbicas cuente como arco.
const ARC_TOL: f64 = 1e-3;
/// Muestras por cúbica al medir la distancia del clic.
const SAMPLES: usize = 8;
/// Una cadena de arco tiene a lo más cuatro cúbicas (cada una cubre 90 grados o menos).
const MAX_RUN: usize = 4;

/// Resultado de revertir una esquina.
#[derive(Debug, Clone, PartialEq)]
pub struct Unfillet {
    /// Trayectoria con la esquina devuelta a punta (el resto, intacto).
    pub path: BezPath,
    /// Distancia (mm) del punto pedido a la esquina encontrada.
    pub distance: f64,
}

/// Cadena de cúbicas: `len` aristas desde la `start`.
struct Run {
    start: usize,
    len: usize,
}

fn is_cubic(sub: &Sub, e: usize) -> bool {
    matches!(sub.edges[e], Edge::Cubic(..))
}

fn cubic_at(a: Point, c1: Point, c2: Point, b: Point, t: f64) -> Point {
    let u = 1.0 - t;
    let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    Point::new(
        w0 * a.x + w1 * c1.x + w2 * c2.x + w3 * b.x,
        w0 * a.y + w1 * c1.y + w2 * c2.y + w3 * b.y,
    )
}

/// Cadenas de cúbicas con una recta antes y otra después.
fn runs(sub: &Sub) -> Vec<Run> {
    let m = sub.edges.len();
    let mut out = Vec::new();
    for e in 0..m {
        if !is_cubic(sub, e) {
            continue;
        }
        let prev = if sub.closed {
            (e + m - 1) % m
        } else if e == 0 {
            continue;
        } else {
            e - 1
        };
        if is_cubic(sub, prev) {
            continue;
        }
        let mut len = 1;
        while len <= MAX_RUN {
            let next = e + len;
            let idx = if sub.closed { next % m } else { next };
            if idx >= m || (sub.closed && len >= m) {
                len = 0;
                break;
            }
            if !is_cubic(sub, idx) {
                break;
            }
            len += 1;
        }
        if len == 0 || len > MAX_RUN {
            continue;
        }
        out.push(Run { start: e, len });
    }
    out
}

/// Vértice original de una cadena y puntos de muestra de su arco, si es un redondeo o un mordisco.
fn corner_of(sub: &Sub, run: &Run) -> Option<(Point, Vec<Point>)> {
    let n = sub.nodes.len();
    let a = run.start;
    let b = (a + run.len) % n;
    let (p, na, nb, nn) = (
        sub.nodes[(a + n - 1) % n],
        sub.nodes[a],
        sub.nodes[b],
        sub.nodes[(b + 1) % n],
    );
    if p.dist(na) < 1e-9 || nb.dist(nn) < 1e-9 {
        return None;
    }
    let (d1, d2) = (unit_from(p, na), unit_from(nb, nn));
    // na + s·d1 = nb - t·d2: las dos rectas, prolongadas, se cortan en el vértice.
    let cross = d1.x * d2.y - d1.y * d2.x;
    if cross.abs() < MIN_TURN_SIN {
        return None;
    }
    let (rx, ry) = (nb.x - na.x, nb.y - na.y);
    let s = (rx * d2.y - ry * d2.x) / cross;
    let t = (d1.x * ry - d1.y * rx) / cross;
    if !(s > 1e-6 && t > 1e-6) || (s - t).abs() > ARC_TOL * s.max(t) + 1e-6 {
        return None;
    }
    let v = Point::new(na.x + d1.x * s, na.y + d1.y * s);

    // Puntos de la cadena: nodos y mitad de cada cúbica, más las muestras para medir el clic.
    let mut checks: Vec<Point> = Vec::new();
    let mut samples: Vec<Point> = vec![v];
    for k in 0..run.len {
        let e = (a + k) % sub.edges.len();
        let Edge::Cubic(c1, c2) = sub.edges[e] else {
            return None;
        };
        let (from, to) = (sub.nodes[e], sub.nodes[(e + 1) % n]);
        checks.push(to);
        checks.push(cubic_at(from, c1, c2, to, 0.5));
        for i in 0..=SAMPLES {
            samples.push(cubic_at(from, c1, c2, to, i as f64 / SAMPLES as f64));
        }
    }
    checks.push(na);

    let (u, w) = (unit_from(v, na), unit_from(v, nb));
    let th = (u.x * w.x + u.y * w.y).clamp(-1.0, 1.0).acos();
    let on_circle = |c: Point, r: f64| {
        checks
            .iter()
            .all(|q| (q.dist(c) - r).abs() <= ARC_TOL * r + 1e-6)
    };
    // Redondeo: círculo tangente a las dos rectas.
    let half = th / 2.0;
    let r = s * half.tan();
    let bis = {
        let (bx, by) = (u.x + w.x, u.y + w.y);
        let l = bx.hypot(by);
        Point::new(bx / l, by / l)
    };
    let dist = r / half.sin();
    let center = Point::new(v.x + bis.x * dist, v.y + bis.y * dist);
    if th > 1e-3 && th < std::f64::consts::PI - 1e-3 && on_circle(center, r) {
        return Some((v, samples));
    }
    // Mordisco: círculo centrado en el vértice, de radio igual a lo que se consumió de cada recta.
    if on_circle(v, s) {
        return Some((v, samples));
    }
    None
}

/// Sub-trayecto con la cadena `run` sustituida por el vértice `v`.
fn restore(sub: &Sub, run: &Run, v: Point) -> Option<BezPath> {
    let n = sub.nodes.len();
    let m = sub.edges.len();
    // Se gira para que la cadena empiece en la arista 1 (el nodo 0 es el anterior a la esquina).
    let shift = if sub.closed {
        (run.start + n - 1) % n
    } else {
        0
    };
    let node = |k: usize| sub.nodes[(k + shift) % n];
    let edge = |k: usize| sub.edges[(k + shift) % m];
    let a = if sub.closed { 1 } else { run.start };
    let after = a + run.len + 1; // primer nodo tras el final de la cadena
    let mut nodes: Vec<Point> = (0..a).map(node).collect();
    nodes.push(v);
    let mut edges: Vec<Edge> = (0..a).map(edge).collect();
    // La arista que llega al vértice y la que sale de él ya son rectas.
    nodes.extend((after..n).map(node));
    edges.extend(((after - 1)..m).map(edge));
    if sub.closed && nodes.len() < 3 {
        return None;
    }
    if nodes.len() < 2 {
        return None;
    }

    let count = nodes.len();
    let mut segs = vec![PathSeg::MoveTo { to: nodes[0] }];
    let last_edge = edges.len();
    for (k, e) in edges.iter().enumerate() {
        let to = nodes[(k + 1) % count];
        let closing = sub.closed && k + 1 == last_edge;
        match *e {
            Edge::Line => {
                if !closing {
                    segs.push(PathSeg::LineTo { to });
                }
            }
            Edge::Cubic(c1, c2) => segs.push(PathSeg::CubicTo {
                ctrl1: c1,
                ctrl2: c2,
                to,
            }),
        }
    }
    if sub.closed {
        segs.push(PathSeg::Close);
    }
    Some(BezPath(segs))
}

/// Devuelve a punta la esquina redondeada (o mordida) más cercana a `point`, a lo más a
/// `max_dist` mm de su arco o de su vértice. `None` si no hay ninguna que se pueda revertir de
/// forma fiable (ver el comentario del módulo). Solo cambia esa esquina.
pub fn unfillet_path(path: &BezPath, point: Point, max_dist: f64) -> Option<Unfillet> {
    if !(max_dist.is_finite() && max_dist > 0.0 && point.x.is_finite() && point.y.is_finite()) {
        return None;
    }
    let subs = split_subpaths(path);
    let mut best: Option<(usize, Sub, Run, Point, f64)> = None;
    for (i, raw) in subs.iter().enumerate() {
        let Some(sub) = parse(raw) else { continue };
        for run in runs(&sub) {
            let Some((v, samples)) = corner_of(&sub, &run) else {
                continue;
            };
            let d = samples
                .iter()
                .map(|q| q.dist(point))
                .fold(f64::INFINITY, f64::min);
            if d <= max_dist && best.as_ref().is_none_or(|b| d < b.4) {
                best = Some((i, parse(raw)?, run, v, d));
            }
        }
    }
    let (index, sub, run, v, distance) = best?;
    let restored = restore(&sub, &run, v)?;
    let mut out = BezPath(Vec::new());
    for (i, raw) in subs.iter().enumerate() {
        if i == index {
            out.0.extend(restored.0.iter().copied());
        } else {
            out.0.extend(raw.0.iter().copied());
        }
    }
    Some(Unfillet {
        path: out,
        distance,
    })
}
