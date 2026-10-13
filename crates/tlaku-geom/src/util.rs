use tlaku_core::{Point, Polyline};

/// Distancia (mm) bajo la cual un punto se considera sobre el borde.
pub(crate) const EPS: f64 = 1e-9;
/// Coordenadas más allá de este valor (mm) se consideran inválidas.
pub(crate) const MAX_COORD: f64 = 1e7;
/// Áreas (mm²) por debajo de esto se consideran cero.
pub(crate) const MIN_AREA: f64 = 1e-12;

/// Área con signo por la fórmula del cordón: positiva si los puntos van en
/// sentido antihorario (con Y hacia arriba), negativa si van en sentido horario.
/// Trata la lista como polígono cerrado. Con menos de 3 puntos devuelve 0.
pub fn signed_area(points: &[Point]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    let o = points[0];
    let sum: f64 = points
        .windows(2)
        .map(|w| (w[0].x - o.x) * (w[1].y - o.y) - (w[1].x - o.x) * (w[0].y - o.y))
        .sum();
    sum / 2.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Antihorario (área con signo positiva).
    Ccw,
    /// Horario (área con signo negativa).
    Cw,
    /// Menos de 3 puntos, área cero o coordenadas no finitas.
    Degenerate,
}

pub fn orientation(points: &[Point]) -> Orientation {
    let area = signed_area(points);
    if area > 0.0 {
        Orientation::Ccw
    } else if area < 0.0 {
        Orientation::Cw
    } else {
        Orientation::Degenerate
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointLocation {
    Inside,
    /// A menos de 1e-9 mm de algún lado.
    Boundary,
    Outside,
}

fn dist_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 == 0.0 {
        return p.dist(a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    p.dist(Point::new(a.x + t * dx, a.y + t * dy))
}

/// Ubica un punto respecto a un polígono cerrado (regla par-impar, así que
/// funciona también con polígonos autointersecantes). Con menos de 3 puntos
/// todo es `Outside`.
pub fn locate_point(p: Point, polygon: &[Point]) -> PointLocation {
    let n = polygon.len();
    if n < 3 {
        return PointLocation::Outside;
    }
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (polygon[i], polygon[(i + 1) % n]);
        if dist_to_segment(p, a, b) <= EPS {
            return PointLocation::Boundary;
        }
        if (a.y > p.y) != (b.y > p.y) {
            let x = a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y);
            if p.x < x {
                inside = !inside;
            }
        }
    }
    if inside {
        PointLocation::Inside
    } else {
        PointLocation::Outside
    }
}

/// Verdadero solo si el punto está estrictamente dentro; los puntos del borde
/// cuentan como fuera (usa [`locate_point`] para distinguirlos).
pub fn point_in_polygon(p: Point, polygon: &[Point]) -> bool {
    locate_point(p, polygon) == PointLocation::Inside
}

pub(crate) fn is_valid_point(p: &Point) -> bool {
    p.x.is_finite() && p.y.is_finite() && p.x.abs() <= MAX_COORD && p.y.abs() <= MAX_COORD
}

/// Quita puntos consecutivos a distancia <= `eps` (y, si es cerrada, el último
/// si coincide con el primero).
pub(crate) fn dedupe_points(points: &[Point], closed: bool, eps: f64) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(points.len());
    for &p in points {
        if out.last().is_none_or(|l| l.dist(p) > eps) {
            out.push(p);
        }
    }
    if closed {
        while out.len() > 1 && out[0].dist(out[out.len() - 1]) <= eps {
            out.pop();
        }
    }
    out
}

/// Verdadero si `b` está entre `a` y `c` sobre una recta (a distancia <= `eps`).
fn is_straight_through(a: Point, b: Point, c: Point, eps: f64) -> bool {
    let (dx, dy) = (c.x - a.x, c.y - a.y);
    let len = dx.hypot(dy);
    if len <= eps {
        return false;
    }
    let cross = ((b.x - a.x) * dy - (b.y - a.y) * dx).abs() / len;
    let t = ((b.x - a.x) * dx + (b.y - a.y) * dy) / (len * len);
    cross <= eps && t > 0.0 && t < 1.0
}

/// Elimina puntos repetidos (a distancia <= `epsilon`) y puntos colineales (el
/// punto medio queda a <= `epsilon` de la recta que une sus vecinos y entre
/// ellos). Las puntas que retroceden sobre sí mismas no se tocan. Se conservan
/// al menos 2 puntos en abiertas y 3 en cerradas. Con `epsilon` muy grande el
/// error se puede acumular a lo largo de curvas suaves; usa valores pequeños
/// (1e-6 mm o menos) para limpieza y no para simplificar.
pub fn remove_redundant_points(path: &Polyline, epsilon: f64) -> Polyline {
    let eps = if epsilon.is_finite() && epsilon > 0.0 {
        epsilon
    } else {
        0.0
    };
    let finite: Vec<Point> = path
        .points
        .iter()
        .copied()
        .filter(|p| p.x.is_finite() && p.y.is_finite())
        .collect();
    let mut pts = dedupe_points(&finite, path.closed, eps);
    let min_len = if path.closed { 3 } else { 2 };

    let mut stack: Vec<Point> = Vec::with_capacity(pts.len());
    for p in pts.drain(..) {
        stack.push(p);
        while stack.len() >= 3 {
            let n = stack.len();
            if is_straight_through(stack[n - 3], stack[n - 2], stack[n - 1], eps) {
                stack.remove(n - 2);
            } else {
                break;
            }
        }
    }
    if path.closed {
        loop {
            let n = stack.len();
            if n <= min_len {
                break;
            }
            if is_straight_through(stack[n - 2], stack[n - 1], stack[0], eps) {
                stack.pop();
            } else if is_straight_through(stack[n - 1], stack[0], stack[1], eps) {
                stack.remove(0);
            } else {
                break;
            }
        }
    }
    Polyline {
        points: stack,
        closed: path.closed,
    }
}

/// Cerradas válidas: todas las coordenadas finitas y razonables, sin repetidos
/// consecutivos, al menos 3 puntos y área no nula.
pub(crate) fn clean_closed(paths: &[Polyline]) -> Vec<Polyline> {
    paths
        .iter()
        .filter(|p| p.closed && p.points.iter().all(is_valid_point))
        .filter_map(|p| {
            let points = dedupe_points(&p.points, true, EPS);
            (points.len() >= 3 && signed_area(&points).abs() > MIN_AREA).then_some(Polyline {
                points,
                closed: true,
            })
        })
        .collect()
}
