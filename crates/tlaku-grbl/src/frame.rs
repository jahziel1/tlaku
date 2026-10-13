//! Encuadre: recorrido del rectángulo del trabajo con el láser apagado.

use tlaku_core::{Bounds, Point};

/// Esquinas del encuadre en el orden en que se recorren, con su nombre.
pub fn frame_corners(b: Bounds) -> [(&'static str, f64, f64); 4] {
    [
        ("inferior izquierda", b.min.x, b.min.y),
        ("inferior derecha", b.max.x, b.min.y),
        ("superior derecha", b.max.x, b.max.y),
        ("superior izquierda", b.min.x, b.max.y),
    ]
}

/// G-code del encuadre: va en G0 a la primera esquina y recorre el rectángulo con G1 S0 bajo
/// M5, porque GRBL ignora F en G0 y así sí se respeta la velocidad pedida.
pub fn frame_gcode(b: Bounds, speed_mm_min: f64) -> Vec<String> {
    frame_gcode_scaled(b, speed_mm_min, 1.0)
}

/// Como [`frame_gcode`] para un rotativo: `b` está en mm de superficie y `y_scale` convierte
/// la Y a mm de la controladora. Los lados con componente Y llevan una F reescalada para que
/// la velocidad sobre la superficie sea `speed_mm_min` (ver `GcodeOptions::y_scale`).
pub fn frame_gcode_scaled(b: Bounds, speed_mm_min: f64, y_scale: f64) -> Vec<String> {
    let k = if y_scale.is_finite() && y_scale > 0.0 {
        y_scale
    } else {
        1.0
    };
    let corners = frame_corners(b);
    let mut lines = vec!["G90".to_string(), "G21".into(), "M5".into()];
    let (_, x0, y0) = corners[0];
    lines.push(format!("G0 X{} Y{}", fmt(x0), fmt(y0 * k)));
    let mut from = (x0, y0);
    for (_, x, y) in corners.iter().skip(1).chain(corners.iter().take(1)) {
        let (dx, dy) = (x - from.0, y - from.1);
        let surface = dx.hypot(dy);
        let feed = if surface > 0.0 {
            speed_mm_min * dx.hypot(dy * k) / surface
        } else {
            speed_mm_min
        };
        lines.push(format!(
            "G1 X{} Y{} S0 F{}",
            fmt(*x),
            fmt(*y * k),
            fmt(feed)
        ));
        from = (*x, *y);
    }
    lines.push("M5".into());
    lines
}

/// Casco convexo de `points` (cadena monótona de Andrew), en sentido antihorario y sin puntos
/// repetidos ni alineados. Empieza en el punto más bajo (a igual Y, el de más a la izquierda), que
/// es también donde empieza el encuadre del rectángulo. Ignora los puntos que no son números.
/// Con todos los puntos iguales devuelve uno; con todos alineados, los dos extremos.
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
    // Producto cruzado de (b - a) x (c - a): > 0 si c queda a la izquierda de a -> b.
    let cross =
        |a: Point, b: Point, c: Point| (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    let mut hull: Vec<Point> = Vec::with_capacity(pts.len() * 2);
    for &p in &pts {
        while hull.len() >= 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
            hull.pop();
        }
        hull.push(p);
    }
    let lower = hull.len() + 1;
    for &p in pts.iter().rev().skip(1) {
        while hull.len() >= lower && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
            hull.pop();
        }
        hull.push(p);
    }
    hull.pop(); // el último repite al primero
    // Que empiece en el punto más bajo, como el encuadre del rectángulo.
    if let Some(start) = (0..hull.len()).min_by(|&i, &j| {
        hull[i]
            .y
            .total_cmp(&hull[j].y)
            .then(hull[i].x.total_cmp(&hull[j].x))
    }) {
        hull.rotate_left(start);
    }
    hull
}

/// G-code del encuadre de banda elástica: como [`frame_gcode_scaled`] pero siguiendo el casco
/// convexo de `points` (los puntos del trabajo, ya en coordenadas de la máquina) en lugar del
/// rectángulo. Va en G0 al primer vértice y recorre el casco con G1 S0 bajo M5, y vuelve al
/// primero. `y_scale` es la del rotativo (1 sin él). Sin puntos devuelve una lista vacía; con
/// uno solo (o todos alineados) va y vuelve sobre ese punto o ese segmento.
pub fn frame_hull_gcode(points: &[Point], speed_mm_min: f64, y_scale: f64) -> Vec<String> {
    let k = if y_scale.is_finite() && y_scale > 0.0 {
        y_scale
    } else {
        1.0
    };
    let hull = convex_hull(points);
    let Some(&first) = hull.first() else {
        return Vec::new();
    };
    let mut lines = vec!["G90".to_string(), "G21".into(), "M5".into()];
    lines.push(format!("G0 X{} Y{}", fmt(first.x), fmt(first.y * k)));
    let mut from = first;
    // Cierra el recorrido volviendo al primer vértice.
    for to in hull.iter().skip(1).chain(std::iter::once(&first)) {
        let (dx, dy) = (to.x - from.x, to.y - from.y);
        let surface = dx.hypot(dy);
        let feed = if surface > 0.0 {
            speed_mm_min * dx.hypot(dy * k) / surface
        } else {
            speed_mm_min
        };
        lines.push(format!(
            "G1 X{} Y{} S0 F{}",
            fmt(to.x),
            fmt(to.y * k),
            fmt(feed)
        ));
        from = *to;
    }
    lines.push("M5".into());
    lines
}

fn fmt(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_core::Point;

    fn bounds() -> Bounds {
        Bounds {
            min: Point::new(10.0, 20.0),
            max: Point::new(30.0, 25.0),
        }
    }

    #[test]
    fn plain_frame_walks_the_rectangle() {
        let g = frame_gcode(bounds(), 1500.0);
        assert_eq!(
            g,
            [
                "G90",
                "G21",
                "M5",
                "G0 X10 Y20",
                "G1 X30 Y20 S0 F1500",
                "G1 X30 Y25 S0 F1500",
                "G1 X10 Y25 S0 F1500",
                "G1 X10 Y20 S0 F1500",
                "M5"
            ]
        );
    }

    #[test]
    fn rotary_frame_scales_y_and_the_feed_of_the_y_sides() {
        let g = frame_gcode_scaled(bounds(), 1500.0, 2.0);
        assert_eq!(g[3], "G0 X10 Y40");
        assert_eq!(g[4], "G1 X30 Y40 S0 F1500");
        assert_eq!(g[5], "G1 X30 Y50 S0 F3000");
        assert_eq!(g[6], "G1 X10 Y50 S0 F1500");
        assert_eq!(g[7], "G1 X10 Y40 S0 F3000");
        assert_eq!(
            frame_gcode_scaled(bounds(), 1500.0, 1.0),
            frame_gcode(bounds(), 1500.0)
        );
    }

    fn pt(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn hull_of_a_square_with_interior_and_collinear_points() {
        let pts = [
            pt(10.0, 10.0),
            pt(20.0, 10.0),
            pt(30.0, 10.0), // alineado sobre un lado
            pt(30.0, 30.0),
            pt(10.0, 30.0),
            pt(20.0, 20.0), // interior
            pt(20.0, 20.0), // repetido
            pt(f64::NAN, 5.0),
        ];
        assert_eq!(
            convex_hull(&pts),
            [
                pt(10.0, 10.0),
                pt(30.0, 10.0),
                pt(30.0, 30.0),
                pt(10.0, 30.0)
            ]
        );
    }

    #[test]
    fn hull_is_counter_clockwise_and_starts_at_the_lowest_point() {
        let pts = [pt(0.0, 5.0), pt(10.0, 0.0), pt(5.0, 12.0), pt(4.0, 5.0)];
        let h = convex_hull(&pts);
        assert_eq!(h[0], pt(10.0, 0.0));
        // Área con signo positiva = antihorario.
        let area: f64 = (0..h.len())
            .map(|i| {
                let (a, b) = (h[i], h[(i + 1) % h.len()]);
                a.x * b.y - b.x * a.y
            })
            .sum();
        assert!(area > 0.0);
        assert!(
            !h.contains(&pt(4.0, 5.0)),
            "el punto interior no es del casco"
        );
    }

    #[test]
    fn hull_degenerate_cases() {
        assert!(convex_hull(&[]).is_empty());
        assert_eq!(convex_hull(&[pt(1.0, 2.0), pt(1.0, 2.0)]), [pt(1.0, 2.0)]);
        // Todos sobre una recta: solo los dos extremos.
        let line = [pt(0.0, 0.0), pt(5.0, 5.0), pt(10.0, 10.0), pt(2.0, 2.0)];
        assert_eq!(convex_hull(&line), [pt(0.0, 0.0), pt(10.0, 10.0)]);
    }

    #[test]
    fn hull_frame_walks_a_triangle_and_closes_it() {
        let g = frame_hull_gcode(
            &[pt(0.0, 0.0), pt(40.0, 0.0), pt(0.0, 30.0), pt(5.0, 5.0)],
            1200.0,
            1.0,
        );
        assert_eq!(
            g,
            [
                "G90",
                "G21",
                "M5",
                "G0 X0 Y0",
                "G1 X40 Y0 S0 F1200",
                "G1 X0 Y30 S0 F1200",
                "G1 X0 Y0 S0 F1200",
                "M5"
            ]
        );
    }

    #[test]
    fn hull_frame_of_a_square_matches_the_rectangle_frame() {
        let b = bounds();
        let corners = [
            pt(b.min.x, b.min.y),
            pt(b.max.x, b.min.y),
            pt(b.max.x, b.max.y),
            pt(b.min.x, b.max.y),
            pt(20.0, 22.0),
        ];
        assert_eq!(
            frame_hull_gcode(&corners, 1500.0, 1.0),
            frame_gcode(b, 1500.0)
        );
        assert_eq!(
            frame_hull_gcode(&corners, 1500.0, 2.0),
            frame_gcode_scaled(b, 1500.0, 2.0)
        );
    }

    #[test]
    fn hull_frame_of_nothing_or_a_single_point() {
        assert!(frame_hull_gcode(&[], 1000.0, 1.0).is_empty());
        let g = frame_hull_gcode(&[pt(3.0, 4.0)], 1000.0, 1.0);
        assert_eq!(g[3], "G0 X3 Y4");
        assert_eq!(g[4], "G1 X3 Y4 S0 F1000");
        assert_eq!(g.last().map(String::as_str), Some("M5"));
    }

    #[test]
    fn hull_frame_never_turns_the_laser_on() {
        let g = frame_hull_gcode(&[pt(0.0, 0.0), pt(9.0, 1.0), pt(4.0, 8.0)], 900.0, 1.0);
        assert!(g.iter().all(|l| !l.contains("M3") && !l.contains("M4")));
        assert!(
            g.iter()
                .filter(|l| l.starts_with("G1"))
                .all(|l| l.contains("S0"))
        );
    }
}
