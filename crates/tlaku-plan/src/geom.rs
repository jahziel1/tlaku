//! Geometría mínima: cajas, área y punto en polígono.

use tlaku_core::Point;

/// Tolerancia en mm para tocar un borde.
pub(crate) const TOUCH: f64 = 1e-7;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Bbox {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Bbox {
    pub fn of(points: &[Point]) -> Bbox {
        let mut b = Bbox {
            min_x: f64::INFINITY,
            min_y: f64::INFINITY,
            max_x: f64::NEG_INFINITY,
            max_y: f64::NEG_INFINITY,
        };
        for p in points {
            b.min_x = b.min_x.min(p.x);
            b.min_y = b.min_y.min(p.y);
            b.max_x = b.max_x.max(p.x);
            b.max_y = b.max_y.max(p.y);
        }
        b
    }

    /// Distancia desde `p` a la caja (0 si está dentro).
    pub fn dist_to(&self, p: Point) -> f64 {
        let dx = (self.min_x - p.x).max(p.x - self.max_x).max(0.0);
        let dy = (self.min_y - p.y).max(p.y - self.max_y).max(0.0);
        dx.hypot(dy)
    }

    /// `other` cabe dentro de `self` con tolerancia `tol`.
    pub fn contains_box(&self, other: &Bbox, tol: f64) -> bool {
        other.min_x >= self.min_x - tol
            && other.min_y >= self.min_y - tol
            && other.max_x <= self.max_x + tol
            && other.max_y <= self.max_y + tol
    }
}

/// Área absoluta por la fórmula del zapatero.
pub(crate) fn area(points: &[Point]) -> f64 {
    let n = points.len();
    if n < 3 {
        return 0.0;
    }
    let mut s = 0.0;
    for i in 0..n {
        let (a, b) = (points[i], points[(i + 1) % n]);
        s += a.x * b.y - b.x * a.y;
    }
    s.abs() / 2.0
}

fn on_segment(p: Point, a: Point, b: Point) -> bool {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.dist(Point::new(a.x + t * dx, a.y + t * dy)) <= TOUCH
}

/// Margen con el que un segmento puede "tocar" un punto: `TOUCH` más holgura para el redondeo
/// de la distancia. Fuera de la caja del segmento ensanchada con esto, `on_segment` es falso.
const REACH: f64 = 2.0 * TOUCH;

/// Una arista decide algo para `p` solo si `p` está en su caja ensanchada por `REACH`
/// (toca el segmento) o si su rango en Y cruza la horizontal de `p`. Este rechazo barato es
/// exacto: no cambia ningún resultado de [`inside`].
#[inline]
fn edge_matters(p: Point, a: Point, b: Point) -> bool {
    let (lo_y, hi_y) = if a.y < b.y { (a.y, b.y) } else { (b.y, a.y) };
    if p.y < lo_y - REACH || p.y > hi_y + REACH {
        return false;
    }
    let hi_x = a.x.max(b.x);
    // Fuera de la caja ensanchada en X no toca; si no toca, solo importa si cruza hacia la
    // derecha de `p` (el rayo va hacia +X): una arista toda a la izquierda no alterna.
    p.x <= hi_x + REACH
}

/// Decide una arista: `Some(true)` si el punto está en el borde; si no, alterna `in_poly`
/// cuando el rayo hacia +X la cruza.
#[inline]
fn edge_step(p: Point, a: Point, b: Point, in_poly: &mut bool) -> bool {
    if edge_matters(p, a, b) {
        if on_segment(p, a, b) {
            return true;
        }
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            *in_poly = !*in_poly;
        }
    }
    false
}

/// Punto dentro del polígono cerrado `poly` (par-impar); el borde cuenta como dentro.
pub(crate) fn inside(p: Point, poly: &[Point]) -> bool {
    let n = poly.len();
    let mut in_poly = false;
    let mut j = n - 1;
    for i in 0..n {
        if edge_step(p, poly[j], poly[i], &mut in_poly) {
            return true;
        }
        j = i;
    }
    in_poly
}

/// Polígonos con menos vértices que esto no valen un índice.
const INDEX_MIN_VERTICES: usize = 48;

/// Índice de aristas por franjas horizontales (formato CSR). Responde igual que [`inside`]
/// pero mira solo las aristas cuya franja contiene la Y del punto: la paridad y el "en el
/// borde" no dependen del orden de las aristas, y una arista fuera de la franja de `p.y` no
/// puede tocarlo ni cruzar su horizontal.
pub(crate) struct PolyIndex {
    y0: f64,
    inv: f64,
    bins: usize,
    starts: Vec<u32>,
    edges: Vec<u32>,
}

impl PolyIndex {
    fn build(poly: &[Point]) -> PolyIndex {
        let n = poly.len();
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for p in poly {
            lo = lo.min(p.y);
            hi = hi.max(p.y);
        }
        let mut bins = (n / 8).clamp(4, 2048);
        loop {
            let ext = hi - lo + 2.0 * REACH;
            let y0 = lo - REACH;
            let inv = bins as f64 / ext;
            let bin =
                |y: f64| -> usize { (((y - y0) * inv).floor().max(0.0) as usize).min(bins - 1) };
            let span = |a: Point, b: Point| {
                let (l, h) = if a.y < b.y { (a.y, b.y) } else { (b.y, a.y) };
                (bin(l - REACH), bin(h + REACH))
            };
            let mut starts = vec![0u32; bins + 1];
            let mut total = 0usize;
            let mut j = n - 1;
            for i in 0..n {
                let (b0, b1) = span(poly[j], poly[i]);
                for slot in &mut starts[b0 + 1..=b1 + 1] {
                    *slot += 1;
                }
                total += b1 - b0 + 1;
                j = i;
            }
            // Aristas altísimas (zigzag de lado a lado) inflarían el índice: menos franjas.
            if total > 24 * n && bins > 1 {
                bins /= 2;
                continue;
            }
            for k in 0..bins {
                starts[k + 1] += starts[k];
            }
            let mut fill = starts.clone();
            let mut edges = vec![0u32; total];
            let mut j = n - 1;
            for i in 0..n {
                let (b0, b1) = span(poly[j], poly[i]);
                for b in b0..=b1 {
                    edges[fill[b] as usize] = i as u32;
                    fill[b] += 1;
                }
                j = i;
            }
            return PolyIndex {
                y0,
                inv,
                bins,
                starts,
                edges,
            };
        }
    }

    fn inside(&self, p: Point, poly: &[Point]) -> bool {
        let n = poly.len();
        let b = (((p.y - self.y0) * self.inv).floor().max(0.0) as usize).min(self.bins - 1);
        let mut in_poly = false;
        for &i in &self.edges[self.starts[b] as usize..self.starts[b + 1] as usize] {
            let i = i as usize;
            let j = if i == 0 { n - 1 } else { i - 1 };
            if edge_step(p, poly[j], poly[i], &mut in_poly) {
                return true;
            }
        }
        in_poly
    }
}

/// Punto en polígono sobre un conjunto de trayectorias, con índice perezoso para las de muchos
/// vértices. Con `indexed = false` usa siempre [`inside`] (referencia para las pruebas).
pub(crate) struct Polys<'a> {
    paths: &'a [&'a tlaku_core::Polyline],
    cells: Vec<std::cell::OnceCell<PolyIndex>>,
    indexed: bool,
}

impl<'a> Polys<'a> {
    pub fn new(paths: &'a [&'a tlaku_core::Polyline], indexed: bool) -> Polys<'a> {
        Polys {
            paths,
            cells: (0..paths.len())
                .map(|_| std::cell::OnceCell::new())
                .collect(),
            indexed,
        }
    }

    pub fn path(&self, j: usize) -> &'a tlaku_core::Polyline {
        self.paths[j]
    }

    /// `p` dentro de la trayectoria `j` (cerrada, con al menos 3 puntos).
    pub fn inside(&self, j: usize, p: Point) -> bool {
        let poly = &self.paths[j].points;
        if self.indexed && poly.len() >= INDEX_MIN_VERTICES {
            return self.cells[j]
                .get_or_init(|| PolyIndex::build(poly))
                .inside(p, poly);
        }
        inside(p, poly)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f64, y: f64, s: f64) -> Vec<Point> {
        vec![
            Point::new(x, y),
            Point::new(x + s, y),
            Point::new(x + s, y + s),
            Point::new(x, y + s),
        ]
    }

    #[test]
    fn area_of_square_and_degenerate() {
        assert!((area(&square(0.0, 0.0, 3.0)) - 9.0).abs() < 1e-12);
        assert_eq!(area(&square(0.0, 0.0, 3.0)[..2]), 0.0);
    }

    #[test]
    fn point_in_polygon_inside_outside_boundary() {
        let sq = square(0.0, 0.0, 10.0);
        assert!(inside(Point::new(5.0, 5.0), &sq));
        assert!(!inside(Point::new(15.0, 5.0), &sq));
        assert!(inside(Point::new(10.0, 5.0), &sq));
        assert!(inside(Point::new(0.0, 0.0), &sq));
    }

    #[test]
    fn concave_polygon() {
        let l = vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 2.0),
            Point::new(2.0, 2.0),
            Point::new(2.0, 10.0),
            Point::new(0.0, 10.0),
        ];
        assert!(inside(Point::new(1.0, 8.0), &l));
        assert!(!inside(Point::new(8.0, 8.0), &l));
    }

    #[test]
    fn poly_index_matches_plain_point_in_polygon() {
        use crate::testgen::{Lcg, star};
        let mut r = Lcg(77);
        for round in 0..60 {
            let n = 3 + (round * 7) % 400;
            let c = Point::new(50.0, 50.0);
            let rad = 5.0 + r.next() * 40.0;
            let poly = star(&mut r, c, rad, n).points;
            let idx = PolyIndex::build(&poly);
            let mut probes: Vec<Point> = (0..300)
                .map(|_| Point::new(r.next() * 120.0 - 10.0, r.next() * 120.0 - 10.0))
                .collect();
            // Vértices, puntos medios de aristas y puntos casi sobre el borde.
            for k in 0..poly.len() {
                let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
                probes.push(a);
                probes.push(Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0));
                probes.push(Point::new(a.x + 5e-8, a.y));
                probes.push(Point::new(a.x, a.y - 3e-7));
                // A la altura de un vértice, a ambos lados.
                probes.push(Point::new(c.x + 60.0, a.y));
                probes.push(Point::new(c.x - 60.0, a.y));
            }
            for p in probes {
                assert_eq!(
                    idx.inside(p, &poly),
                    inside(p, &poly),
                    "ronda {round}, {p:?}"
                );
            }
        }
    }

    #[test]
    fn edge_prefilter_keeps_axis_aligned_and_degenerate_cases() {
        // Cuadrado con aristas horizontales y verticales y un punto repetido.
        let sq = vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
        ];
        for &(x, y, want) in &[
            (5.0, 0.0, true),
            (10.0, 5.0, true),
            (11.0, 5.0, false),
            (-1.0, 0.0, false),
            (5.0, 10.0, true),
            (5.0, 10.0 + 1e-6, false),
            (0.0, 0.0, true),
        ] {
            assert_eq!(inside(Point::new(x, y), &sq), want, "({x}, {y})");
        }
    }
}
