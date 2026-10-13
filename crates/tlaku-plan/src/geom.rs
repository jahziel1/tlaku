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

/// Punto dentro del polígono cerrado `poly` (par-impar); el borde cuenta como dentro.
pub(crate) fn inside(p: Point, poly: &[Point]) -> bool {
    let n = poly.len();
    let mut in_poly = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[j], poly[i]);
        if on_segment(p, a, b) {
            return true;
        }
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            in_poly = !in_poly;
        }
        j = i;
    }
    in_poly
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
}
