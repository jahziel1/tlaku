//! Núcleo del barrido: aristas, lista de aristas activas y tramos por línea.

use crate::FillRule;
use tlaku_core::Point;

/// Tolerancia (mm) bajo la cual un tramo se considera vacío y dos tramos contiguos se fusionan.
const EPS: f64 = 1e-9;

/// Cota de líneas de barrido por pasada, para no agotar memoria con intervalos absurdos.
pub(crate) const MAX_LINES: f64 = 5_000_000.0;

/// Rotación de ida (`-ángulo`, deja el barrido horizontal) y de vuelta (`+ángulo`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Frame {
    cos: f64,
    sin: f64,
}

impl Frame {
    pub(crate) fn new(angle_deg: f64) -> Self {
        // Múltiplos de 90° exactos: evita residuos como cos(90°) = 6e-17.
        let (cos, sin) = match angle_deg.rem_euclid(360.0) {
            0.0 => (1.0, 0.0),
            90.0 => (0.0, 1.0),
            180.0 => (-1.0, 0.0),
            270.0 => (0.0, -1.0),
            a => {
                let r = a.to_radians();
                (r.cos(), r.sin())
            }
        };
        Self { cos, sin }
    }

    pub(crate) fn to_local(self, p: Point) -> Point {
        Point::new(
            p.x * self.cos + p.y * self.sin,
            -p.x * self.sin + p.y * self.cos,
        )
    }

    pub(crate) fn to_world(self, x: f64, y: f64) -> Point {
        Point::new(x * self.cos - y * self.sin, x * self.sin + y * self.cos)
    }
}

/// Arista no horizontal, orientada de abajo hacia arriba.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Edge {
    ymin: f64,
    ymax: f64,
    x_at_ymin: f64,
    /// dx/dy
    slope: f64,
    /// +1 si el polígono la recorre hacia arriba, -1 hacia abajo.
    dir: i32,
}

impl Edge {
    fn new(a: Point, b: Point) -> Option<Self> {
        if a.y == b.y {
            return None;
        }
        let slope = (b.x - a.x) / (b.y - a.y);
        if !slope.is_finite() {
            return None;
        }
        let (lo, hi, dir) = if a.y < b.y { (a, b, 1) } else { (b, a, -1) };
        Some(Self {
            ymin: lo.y,
            ymax: hi.y,
            x_at_ymin: lo.x,
            slope,
            dir,
        })
    }

    fn x_at(&self, y: f64) -> f64 {
        self.x_at_ymin + (y - self.ymin) * self.slope
    }
}

/// Agrega las aristas de un polígono cerrado ya rotado (el último punto cierra con el primero).
pub(crate) fn push_edges(out: &mut Vec<Edge>, pts: &[Point]) {
    if pts.len() < 3 {
        return;
    }
    let mut prev = pts[pts.len() - 1];
    for &p in pts {
        out.extend(Edge::new(prev, p));
        prev = p;
    }
}

fn inside(winding: i32, rule: FillRule) -> bool {
    match rule {
        FillRule::EvenOdd => winding & 1 != 0,
        FillRule::NonZero => winding != 0,
    }
}

/// Barre líneas horizontales en `y = (k + 0.5) * interval` y llama a `emit(y, tramos)` por
/// cada línea con relleno. Los tramos vienen ordenados por x, sin solapes y sin vacíos.
///
/// Cada arista usa la regla semiabierta `[ymin, ymax)`: un vértice compartido por dos aristas
/// cuenta una sola vez o ninguna, y las aristas horizontales nunca cruzan.
pub(crate) fn scan(
    mut edges: Vec<Edge>,
    interval: f64,
    rule: FillRule,
    mut emit: impl FnMut(f64, &[(f64, f64)]),
) -> Result<(), String> {
    if edges.is_empty() {
        return Ok(());
    }
    edges.sort_unstable_by(|a, b| a.ymin.total_cmp(&b.ymin));
    let top = edges.iter().fold(f64::MIN, |m, e| m.max(e.ymax));
    let lines = (top - edges[0].ymin) / interval;
    if lines.is_nan() || lines > MAX_LINES {
        return Err(format!(
            "demasiadas líneas de barrido ({lines:.0}); aumenta el intervalo"
        ));
    }

    let first_line = |ymin: f64| (ymin / interval - 0.5).ceil();
    let mut k = first_line(edges[0].ymin);
    let mut next = 0;
    let mut active: Vec<Edge> = Vec::new();
    let mut crossings: Vec<(f64, i32)> = Vec::new();
    let mut spans: Vec<(f64, f64)> = Vec::new();

    loop {
        let y = (k + 0.5) * interval;
        while next < edges.len() && edges[next].ymin <= y {
            active.push(edges[next]);
            next += 1;
        }
        active.retain(|e| e.ymax > y);

        if active.is_empty() {
            if next >= edges.len() {
                break;
            }
            // Salta el hueco vacío hasta la siguiente arista.
            k = first_line(edges[next].ymin).max(k + 1.0);
            continue;
        }

        crossings.clear();
        crossings.extend(active.iter().map(|e| (e.x_at(y), e.dir)));
        crossings.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));

        spans.clear();
        let mut winding = 0;
        let mut start = 0.0;
        for &(x, dir) in &crossings {
            let was = inside(winding, rule);
            winding += dir;
            let now = inside(winding, rule);
            if !was && now {
                start = x;
            } else if was && !now {
                match spans.last_mut() {
                    Some(last) if start <= last.1 + EPS => last.1 = last.1.max(x),
                    _ => spans.push((start, x)),
                }
            }
        }
        spans.retain(|s| s.1 - s.0 > EPS);
        if !spans.is_empty() {
            emit(y, &spans);
        }
        k += 1.0;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_roundtrip_and_exact_quarters() {
        let p = Point::new(3.0, 4.0);
        let f = Frame::new(90.0);
        assert_eq!(f.to_local(p), Point::new(4.0, -3.0));
        let g = Frame::new(37.0);
        let q = g.to_world(g.to_local(p).x, g.to_local(p).y);
        assert!(q.dist(p) < 1e-12);
    }

    #[test]
    fn scan_square_collects_spans() {
        let mut edges = Vec::new();
        let sq = [
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 2.0),
            Point::new(0.0, 2.0),
        ];
        push_edges(&mut edges, &sq);
        let mut got = Vec::new();
        scan(edges, 1.0, FillRule::EvenOdd, |y, s| {
            got.push((y, s.to_vec()))
        })
        .unwrap();
        assert_eq!(got, vec![(0.5, vec![(0.0, 4.0)]), (1.5, vec![(0.0, 4.0)])]);
    }
}
