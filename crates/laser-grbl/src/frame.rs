//! Encuadre: recorrido del rectángulo del trabajo con el láser apagado.

use laser_core::Bounds;

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
    let corners = frame_corners(b);
    let mut lines = vec!["G90".to_string(), "G21".into(), "M5".into()];
    let (_, x0, y0) = corners[0];
    lines.push(format!("G0 X{} Y{}", fmt(x0), fmt(y0)));
    for (_, x, y) in corners.iter().skip(1).chain(corners.iter().take(1)) {
        lines.push(format!(
            "G1 X{} Y{} S0 F{}",
            fmt(*x),
            fmt(*y),
            fmt(speed_mm_min)
        ));
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
