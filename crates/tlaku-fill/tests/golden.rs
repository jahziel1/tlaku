//! Archivos dorados: el recorrido generado debe coincidir con el aprobado. Si un cambio es
//! intencional, regenera con `UPDATE_GOLDEN=1 cargo test` y revisa el diff.

mod common;

use common::*;
use std::path::Path;
use tlaku_fill::{FillRule, FillSettings, Grouping, fill};

fn check(name: &str, actual: &str) {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("{name}.txt"));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&golden, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&golden)
        .unwrap_or_else(|_| panic!("falta {}; ejecuta con UPDATE_GOLDEN=1", golden.display()));
    assert_eq!(actual, expected, "el recorrido de '{name}' cambió");
}

#[test]
fn donut() {
    let paths = [rect(0.0, 0.0, 6.0, 6.0), rect_cw(2.0, 2.0, 4.0, 4.0)];
    let s = FillSettings {
        interval_mm: 1.0,
        overscan_mm: 0.5,
        bidirectional: true,
        speed_mm_min: 3000.0,
        power_pct: 35.0,
        ..Default::default()
    };
    check("donut", &dump(&fill(&paths, &s).unwrap().block));
}

#[test]
fn triangle_rotated_crosshatch_per_shape() {
    let paths = [
        poly(&[(0.0, 0.0), (8.0, 0.0), (4.0, 6.0)]),
        rect(12.0, 0.0, 15.0, 3.0),
    ];
    let s = FillSettings {
        interval_mm: 1.5,
        angle_deg: 30.0,
        overscan_mm: 1.0,
        crosshatch: true,
        fill_rule: FillRule::NonZero,
        grouping: Grouping::PerShape,
        air_assist: true,
        ..Default::default()
    };
    check(
        "triangle_crosshatch",
        &dump(&fill(&paths, &s).unwrap().block),
    );
}
