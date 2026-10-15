//! Escribe SVG de las piezas de cada generador (con sus valores de fábrica y los de la línea de
//! comandos) para revisarlas a ojo: `cargo run -p tlaku-parts --example svg -- <carpeta>`.
//!
//! Cortes en rojo y grabados en azul, a 1 px por mm; Y se invierte porque en SVG crece hacia abajo.

use serde_json::json;
use std::fmt::Write;
use tlaku_parts::{GenerateResult, generate};

fn svg(r: &GenerateResult) -> String {
    let (w, h) = (r.width + 10.0, r.height + 10.0);
    let mut s = String::new();
    let _ = writeln!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}mm" height="{h}mm" viewBox="0 0 {w} {h}">"#
    );
    let _ = writeln!(s, r##"<rect width="100%" height="100%" fill="#fff"/>"##);
    for (paths, color) in [(true, "#e00000"), (false, "#0050d0")] {
        for part in &r.parts {
            for p in if paths { &part.cuts } else { &part.engraves } {
                let pts: Vec<String> = p
                    .points
                    .iter()
                    .map(|q| format!("{:.3},{:.3}", q[0] + 5.0, h - (q[1] + 5.0)))
                    .collect();
                let tag = if p.closed { "polygon" } else { "polyline" };
                let _ = writeln!(
                    s,
                    r#"<{tag} points="{}" fill="none" stroke="{color}" stroke-width="0.2"/>"#,
                    pts.join(" ")
                );
            }
        }
    }
    s.push_str("</svg>\n");
    s
}

fn main() {
    let dir = std::env::args().nth(1).expect("uso: svg <carpeta>");
    std::fs::create_dir_all(&dir).unwrap();
    let cases = [
        (
            "caja-100x80x60",
            "box",
            json!({"width_mm": 100, "depth_mm": 80, "height_mm": 60, "labels": true}),
        ),
        (
            "caja-150x100x50-divisores-2x1",
            "box",
            json!({"width_mm": 150, "depth_mm": 100, "height_mm": 50, "dividers_x": 2, "dividers_y": 1}),
        ),
        (
            "caja-abierta",
            "box",
            json!({"kind": "open", "labels": true}),
        ),
        (
            "caja-tapa-deslizante",
            "box",
            json!({"kind": "sliding_lid", "labels": true}),
        ),
        ("bisagra-recta", "hinge", json!({})),
        (
            "bisagra-onda",
            "hinge",
            json!({"pattern": "wave", "corner_radius_mm": 8}),
        ),
        (
            "bisagra-zigzag",
            "hinge",
            json!({"pattern": "zigzag", "orientation": "vertical"}),
        ),
        ("prueba-encaje-grosor", "fit_test", json!({})),
        (
            "prueba-encaje-diente",
            "fit_test",
            json!({"mode": "finger"}),
        ),
    ];
    for (name, id, params) in cases {
        let r = generate(id, &params, Some(420.0), 5.0).unwrap();
        assert!(r.error.is_none(), "{name}: {:?}", r.error);
        let path = format!("{dir}/{name}.svg");
        std::fs::write(&path, svg(&r)).unwrap();
        println!(
            "{path}: {} piezas, {:.0} x {:.0} mm",
            r.parts.len(),
            r.width,
            r.height
        );
    }
}
