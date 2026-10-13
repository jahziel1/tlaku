//! Pruebas con archivos dorados en texto legible: un degradado generado aquí mismo se
//! procesa y su bitmap (en arte ASCII) y su toolpath deben coincidir con lo aprobado.
//! Si un cambio es intencional, regenera con `UPDATE_GOLDEN=1 cargo test` y revisa el diff.

use std::fmt::Write;
use std::path::Path;
use tlaku_core::Point;
use tlaku_core::toolpath::{Move, ToolpathBlock};
use tlaku_raster::{
    BayerSize, GrayF32, ProcessOptions, RasterBitmap, RasterMode, RasterSettings, Sampling,
    process, raster_to_block,
};

/// Degradado horizontal de blanco (izquierda) a negro (derecha) con un marco blanco
/// de 2 píxeles en los bordes superior e inferior para probar filas en blanco.
fn gradient() -> GrayF32 {
    GrayF32::from_fn(64, 20, |x, y| {
        if !(2..18).contains(&y) {
            1.0
        } else {
            1.0 - x as f32 / 63.0
        }
    })
}

fn ascii(b: &RasterBitmap) -> String {
    const SHADES: &[u8] = b" .:-=+*#%@";
    let mut s = String::new();
    for y in 0..b.height {
        let line: String = b
            .row(y)
            .iter()
            .map(|&v| SHADES[(v * (SHADES.len() - 1) as f32).round() as usize] as char)
            .collect();
        writeln!(s, "|{line}|").unwrap();
    }
    s
}

fn dump_block(block: &ToolpathBlock) -> String {
    let mut s = String::new();
    writeln!(s, "; {}", block.label).unwrap();
    writeln!(
        s,
        "; speed={} air={} mode={:?} moves={}",
        block.speed_mm_min,
        block.air_assist,
        block.power_mode,
        block.moves.len()
    )
    .unwrap();
    for m in &block.moves {
        match m {
            Move::Travel { to } => writeln!(s, "T {:.3} {:.3}", to.x, to.y),
            Move::Cut { to, power_pct } => {
                writeln!(s, "C {:.3} {:.3} S{:.1}", to.x, to.y, power_pct)
            }
            Move::Dwell { ms, power_pct, .. } => writeln!(s, "D {ms} S{power_pct:.1}"),
        }
        .unwrap();
    }
    s
}

fn check(name: &str, actual: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("{name}.txt"));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("falta {}; ejecuta con UPDATE_GOLDEN=1", path.display()));
    assert_eq!(actual, expected, "el resultado de '{name}' cambió");
}

fn run(name: &str, mode: RasterMode, settings: RasterSettings, sampling: Sampling) {
    let opts = ProcessOptions {
        sampling,
        mode,
        ..Default::default()
    };
    // 16 x 5 mm con puntos de 0.25 mm: 64 x 20 puntos; el degradado ya tiene esa resolución.
    let bitmap = process(&gradient(), 16.0, 5.0, &opts).unwrap();
    let block = raster_to_block(&bitmap, Point::new(10.0, 20.0), &settings);
    let out = format!("{}\n{}", ascii(&bitmap), dump_block(&block));
    check(name, &out);
}

#[test]
fn gradient_grayscale() {
    run(
        "gradient_grayscale",
        RasterMode::Grayscale,
        RasterSettings {
            speed_mm_min: 6000.0,
            min_power_pct: 5.0,
            max_power_pct: 40.0,
            overscan_mm: 1.5,
            ..Default::default()
        },
        Sampling::Interval(0.25),
    );
}

#[test]
fn gradient_floyd_steinberg_dot_width() {
    run(
        "gradient_floyd_steinberg",
        RasterMode::FloydSteinberg,
        RasterSettings {
            speed_mm_min: 4000.0,
            max_power_pct: 30.0,
            overscan_mm: 1.0,
            dot_width_mm: 0.1,
            ..Default::default()
        },
        Sampling::Interval(0.25),
    );
}

#[test]
fn gradient_bayer_unidirectional() {
    run(
        "gradient_bayer4",
        RasterMode::Bayer(BayerSize::N4),
        RasterSettings {
            bidirectional: false,
            overscan_mm: 0.5,
            max_power_pct: 25.0,
            ..Default::default()
        },
        Sampling::Interval(0.25),
    );
}
