//! Pruebas con archivos dorados: el G-code generado debe coincidir byte a byte
//! con el aprobado. Si un cambio es intencional, regenera con
//! `UPDATE_GOLDEN=1 cargo test` y revisa el diff antes de confirmarlo.

use std::path::Path;
use tlaku_core::toolpath::Toolpath;
use tlaku_core::{CutSettings, Rgb};
use tlaku_grbl::gcode::{GcodeOptions, generate};

fn check(name: &str) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let svg = std::fs::read(dir.join(format!("{name}.svg"))).unwrap();
    let mut job = tlaku_svg::import_svg(&svg, 0.05).unwrap().into_job(
        |c| match c {
            Rgb(255, 0, 0) => CutSettings {
                speed_mm_min: 300.0,
                power_pct: 100.0,
                passes: 2,
                air_assist: true,
            },
            _ => CutSettings {
                speed_mm_min: 3000.0,
                power_pct: 30.0,
                passes: 1,
                air_assist: false,
            },
        },
        true,
    );
    job.optimize();
    let actual = generate(&Toolpath::from_line_job(&job), &GcodeOptions::default());

    let golden = dir.join(format!("{name}.gcode"));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&golden, &actual).unwrap();
    }
    let expected = std::fs::read_to_string(&golden)
        .unwrap_or_else(|_| panic!("falta {}; ejecuta con UPDATE_GOLDEN=1", golden.display()));
    assert_eq!(actual, expected, "el G-code de '{name}' cambió");
}

#[test]
fn demo() {
    check("demo");
}
