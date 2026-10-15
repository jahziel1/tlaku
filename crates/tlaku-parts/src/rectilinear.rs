//! Regiones rectilíneas: un rectángulo al que se le restan rectángulos (muescas de los dientes,
//! ranuras). Se resuelve con una rejilla exacta y el trazado de su borde, sin booleanas con
//! redondeo: cada coordenada de salida es una de las de entrada.

use std::collections::HashMap;
use tlaku_core::{Point, Polyline};

/// Tolerancia (mm) con la que dos coordenadas se consideran la misma línea de la rejilla.
const SNAP: f64 = 1e-9;

/// Rectángulo alineado a los ejes, con `x0 <= x1` e `y0 <= y1`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Rect {
    /// Rectángulo con las esquinas en cualquier orden.
    pub fn new(xa: f64, ya: f64, xb: f64, yb: f64) -> Self {
        Self {
            x0: xa.min(xb),
            y0: ya.min(yb),
            x1: xa.max(xb),
            y1: ya.max(yb),
        }
    }

    /// ¿Se solapan con área positiva?
    pub fn overlaps(&self, o: &Rect) -> bool {
        self.x0 < o.x1 - SNAP
            && o.x0 < self.x1 - SNAP
            && self.y0 < o.y1 - SNAP
            && o.y0 < self.y1 - SNAP
    }
}

/// Valores ordenados y sin repetidos (dentro de `SNAP`).
fn axis(mut v: Vec<f64>) -> Vec<f64> {
    v.sort_by(f64::total_cmp);
    let mut out: Vec<f64> = Vec::with_capacity(v.len());
    for x in v {
        if out.last().is_none_or(|&l| x - l > SNAP) {
            out.push(x);
        }
    }
    out
}

/// Contornos de `[0, w] x [0, h]` menos la unión de `holes`.
///
/// Los contornos exteriores salen antihorarios y los agujeros horarios; no hay vértices
/// colineales. Las muescas que tocan el borde abren el contorno; las interiores son agujeros.
pub fn rect_minus_rects(w: f64, h: f64, holes: &[Rect]) -> Vec<Polyline> {
    let mut xs = vec![0.0, w];
    let mut ys = vec![0.0, h];
    for r in holes {
        xs.extend([r.x0.clamp(0.0, w), r.x1.clamp(0.0, w)]);
        ys.extend([r.y0.clamp(0.0, h), r.y1.clamp(0.0, h)]);
    }
    let xs = axis(xs);
    let ys = axis(ys);
    let (nx, ny) = (xs.len() - 1, ys.len() - 1);

    // Una celda es material si su centro no cae dentro de ningún hueco.
    let mut occ = vec![vec![false; ny]; nx];
    for (i, col) in occ.iter_mut().enumerate() {
        for (j, cell) in col.iter_mut().enumerate() {
            let (cx, cy) = ((xs[i] + xs[i + 1]) / 2.0, (ys[j] + ys[j + 1]) / 2.0);
            *cell = !holes
                .iter()
                .any(|r| cx > r.x0 && cx < r.x1 && cy > r.y0 && cy < r.y1);
        }
    }
    let solid = |i: isize, j: isize| {
        i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && occ[i as usize][j as usize]
    };

    // Aristas dirigidas entre vértices de la rejilla, con el material a la izquierda.
    type V = (usize, usize);
    let mut next: HashMap<V, Vec<V>> = HashMap::new();
    let mut add = |a: V, b: V| next.entry(a).or_default().push(b);
    #[allow(clippy::needless_range_loop)] // `i` y `j` son índices de la rejilla, no solo de `occ`
    for i in 0..nx {
        for j in 0..ny {
            if !occ[i][j] {
                continue;
            }
            let (ii, jj) = (i as isize, j as isize);
            if !solid(ii, jj - 1) {
                add((i, j), (i + 1, j));
            }
            if !solid(ii + 1, jj) {
                add((i + 1, j), (i + 1, j + 1));
            }
            if !solid(ii, jj + 1) {
                add((i + 1, j + 1), (i, j + 1));
            }
            if !solid(ii - 1, jj) {
                add((i, j + 1), (i, j));
            }
        }
    }

    let mut loops: Vec<Polyline> = Vec::new();
    let mut starts: Vec<V> = next.keys().copied().collect();
    starts.sort_unstable();
    for start in starts {
        while let Some(first) = next.get_mut(&start).and_then(|v| v.pop()) {
            let mut ring = vec![start];
            let (mut prev, mut cur) = (start, first);
            while cur != start {
                ring.push(cur);
                let outs = next.get_mut(&cur).expect("borde cerrado");
                // En un vértice con dos salidas (dos regiones que se tocan) se gira a la izquierda
                // para que cada región conserve su propio contorno.
                let pick = if outs.len() > 1 {
                    let (dx, dy) = (cur.0 as i64 - prev.0 as i64, cur.1 as i64 - prev.1 as i64);
                    let turn = |o: &V| {
                        let (ex, ey) = (o.0 as i64 - cur.0 as i64, o.1 as i64 - cur.1 as i64);
                        dx * ey - dy * ex
                    };
                    (0..outs.len()).max_by_key(|&k| turn(&outs[k])).unwrap_or(0)
                } else {
                    0
                };
                let nxt = outs.swap_remove(pick);
                prev = cur;
                cur = nxt;
            }
            // Fuera los vértices colineales.
            let n = ring.len();
            let keep: Vec<V> = (0..n)
                .filter(|&k| {
                    let (a, b, c) = (ring[(k + n - 1) % n], ring[k], ring[(k + 1) % n]);
                    let d1 =
                        (b.0 as i64 - a.0 as i64).signum() * 2 + (b.1 as i64 - a.1 as i64).signum();
                    let d2 =
                        (c.0 as i64 - b.0 as i64).signum() * 2 + (c.1 as i64 - b.1 as i64).signum();
                    d1 != d2
                })
                .map(|k| ring[k])
                .collect();
            if keep.len() >= 3 {
                loops.push(Polyline {
                    points: keep
                        .iter()
                        .map(|&(i, j)| Point::new(xs[i], ys[j]))
                        .collect(),
                    closed: true,
                });
            }
        }
    }
    loops
}

/// Área con signo de un contorno cerrado (positiva si es antihorario).
pub fn signed_area(p: &Polyline) -> f64 {
    let n = p.points.len();
    (0..n)
        .map(|i| {
            let (a, b) = (p.points[i], p.points[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
        / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_rectangle_has_four_corners() {
        let r = rect_minus_rects(10.0, 5.0, &[]);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].points.len(), 4);
        assert!((signed_area(&r[0]) - 50.0).abs() < 1e-12);
    }

    #[test]
    fn corner_notch_makes_l_shape() {
        let r = rect_minus_rects(10.0, 10.0, &[Rect::new(0.0, 0.0, 3.0, 4.0)]);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].points.len(), 6);
        assert!((signed_area(&r[0]) - (100.0 - 12.0)).abs() < 1e-9);
    }

    #[test]
    fn interior_hole_is_clockwise() {
        let r = rect_minus_rects(10.0, 10.0, &[Rect::new(2.0, 2.0, 4.0, 3.0)]);
        assert_eq!(r.len(), 2);
        let areas: Vec<f64> = r.iter().map(signed_area).collect();
        assert!(areas.iter().any(|a| (a - 100.0).abs() < 1e-9));
        assert!(areas.iter().any(|a| (a + 2.0).abs() < 1e-9));
    }

    #[test]
    fn adjacent_notches_merge() {
        let r = rect_minus_rects(
            10.0,
            10.0,
            &[Rect::new(0.0, 0.0, 3.0, 2.0), Rect::new(0.0, 2.0, 2.0, 5.0)],
        );
        assert_eq!(r.len(), 1);
        assert!((signed_area(&r[0]) - (100.0 - 6.0 - 6.0)).abs() < 1e-9);
    }

    #[test]
    fn separate_notches_keep_one_contour() {
        let r = rect_minus_rects(
            10.0,
            4.0,
            &[Rect::new(2.0, 0.0, 4.0, 1.0), Rect::new(6.0, 0.0, 8.0, 1.0)],
        );
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].points.len(), 4 + 4 + 4);
    }
}
