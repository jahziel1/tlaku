//! Encuadre: recorrido del rectángulo del trabajo con el láser apagado.

use tlaku_core::Bounds;

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
}
