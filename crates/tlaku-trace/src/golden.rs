//! Pruebas "doradas": comparan byte a byte la salida del vectorizado con archivos guardados en
//! `tests/fixtures/`. Detectan cualquier cambio en el motor de trazado (`crate::vtracer`).
//!
//! Para cada caso hay dos dorados: el SVG crudo que produce el vectorizador (`.svg`) y las
//! trayectorias finales en mm (`.paths.txt`, formato `Debug`). Para regenerarlos a propósito:
//! `TLAKU_UPDATE_GOLDEN=1 cargo test -p tlaku-trace golden`.

use crate::{TraceMode, TraceSettings, binarize, spline, trace};
use image::{DynamicImage, GrayImage, Luma};
use std::path::PathBuf;

fn disc(cx: f64, cy: f64, r: f64) -> impl Fn(u32, u32) -> bool {
    move |x, y| (f64::from(x) + 0.5 - cx).hypot(f64::from(y) + 0.5 - cy) <= r
}

fn gray(w: u32, h: u32, ink: impl Fn(u32, u32) -> bool) -> DynamicImage {
    DynamicImage::ImageLuma8(GrayImage::from_fn(w, h, |x, y| {
        Luma([if ink(x, y) { 0 } else { 255 }])
    }))
}

/// Ruido determinista (hash entero) suavizado a manchas: forma irregular con huecos.
fn blobs(w: u32, h: u32) -> DynamicImage {
    let hash = |x: u32, y: u32| -> u32 {
        let mut v = x.wrapping_mul(374_761_393) ^ y.wrapping_mul(668_265_263);
        v = (v ^ (v >> 13)).wrapping_mul(1_274_126_177);
        v ^ (v >> 16)
    };
    DynamicImage::ImageLuma8(GrayImage::from_fn(w, h, |x, y| {
        // Promedio de celdas de 8x8 con interpolación burda: manchas de ~8 px.
        let (cx, cy) = (x / 8, y / 8);
        let (fx, fy) = (x % 8, y % 8);
        let v = |i: u32, j: u32| f64::from(hash(i, j) & 0xFF);
        let top = v(cx, cy) * f64::from(8 - fx) + v(cx + 1, cy) * f64::from(fx);
        let bottom = v(cx, cy + 1) * f64::from(8 - fx) + v(cx + 1, cy + 1) * f64::from(fx);
        let value = (top * f64::from(8 - fy) + bottom * f64::from(fy)) / 64.0;
        Luma([value as u8])
    }))
}

/// Una "A" con hueco triangular y un trazo diagonal, más un cuadrado con muescas.
fn glyphs() -> DynamicImage {
    gray(240, 160, |x, y| {
        let (xf, yf) = (f64::from(x), f64::from(y));
        let leg_l = (yf - 140.0 + 2.0 * (xf - 20.0)).abs() < 5.0 && (20.0..70.0).contains(&xf);
        let leg_r = (yf - 140.0 - 2.0 * (xf - 120.0)).abs() < 5.0 && (70.0..120.0).contains(&xf);
        let bar = (95..105).contains(&y) && (45..95).contains(&x);
        let square = (140..220).contains(&x) && (30..130).contains(&y);
        let notch = (170..190).contains(&x) && (30..50).contains(&y);
        let hole = disc(180.0, 90.0, 12.0)(x, y);
        leg_l || leg_r || bar || (square && !notch && !hole)
    })
}

struct Case {
    name: &'static str,
    image: DynamicImage,
    settings: TraceSettings,
    size_mm: [f64; 2],
}

/// Casos cuyo ajuste de curvas toma decisiones distintas en otra arquitectura (ruido con desenfoque:
/// un decimal distinto mueve un punto de control hasta ~0,2 mm). Los dorados se generaron en macOS
/// ARM y sirven para comprobar que el código adaptado de vtracer da exactamente lo mismo; en x86 se
/// omiten (los demás casos se comparan en todas las arquitecturas).
const ARCH_SENSITIVE: &[&str] = &["blobs"];

fn compared_here(name: &str) -> bool {
    cfg!(target_arch = "aarch64") || !ARCH_SENSITIVE.contains(&name)
}

fn cases() -> Vec<Case> {
    let ring = {
        let (outer, inner) = (disc(200.0, 200.0, 150.0), disc(200.0, 200.0, 80.0));
        gray(400, 400, move |x, y| outer(x, y) && !inner(x, y))
    };
    let speckles = gray(200, 200, |x, y| {
        (50..150).contains(&x) && (50..150).contains(&y)
            || (5..7).contains(&x) && (5..7).contains(&y)
            || (190..192).contains(&x) && (10..12).contains(&y)
    });
    let default = TraceSettings::default;
    vec![
        Case {
            name: "circle",
            image: gray(400, 400, disc(200.0, 200.0, 150.0)),
            settings: default(),
            size_mm: [100.0, 100.0],
        },
        Case {
            name: "ring",
            image: ring,
            settings: default(),
            size_mm: [100.0, 100.0],
        },
        Case {
            name: "speckles",
            image: speckles,
            settings: TraceSettings {
                speckle_px: 0,
                ..default()
            },
            size_mm: [200.0, 200.0],
        },
        Case {
            name: "glyphs",
            image: glyphs(),
            settings: TraceSettings {
                corner_threshold_deg: 90.0,
                smoothing: 0.3,
                ..default()
            },
            size_mm: [120.0, 80.0],
        },
        Case {
            name: "blobs",
            image: blobs(160, 120),
            settings: TraceSettings {
                smoothing: 0.8,
                blur_px: 1.0,
                ..default()
            },
            size_mm: [80.0, 60.0],
        },
        Case {
            name: "blobs_inverted",
            image: blobs(160, 120),
            settings: TraceSettings {
                invert: true,
                threshold: 0.4,
                smoothing: 0.0,
                ..default()
            },
            size_mm: [80.0, 60.0],
        },
    ]
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn check(file: &str, actual: &str) {
    let path = fixtures().join(file);
    if std::env::var_os("TLAKU_UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("falta el dorado {}: {e}", path.display()));
    if let Err(why) = same_within(&expected, actual, 1e-3) {
        panic!(
            "el dorado {file} no coincide: {why} (regenerar solo a propósito con TLAKU_UPDATE_GOLDEN=1)"
        );
    }
}

/// Compara dos textos: lo que no es número debe ser idéntico y cada número puede diferir en
/// `tol` (absoluto, o relativo para valores grandes). El ajuste de curvas usa coma flotante y los
/// últimos decimales cambian entre procesadores (ARM frente a x86 en la CI), sin que cambie la forma.
fn same_within(expected: &str, actual: &str, tol: f64) -> Result<(), String> {
    fn tokens(s: &str) -> Vec<(bool, &str)> {
        let mut out = Vec::new();
        let b = s.as_bytes();
        let (mut i, mut start) = (0, 0);
        while i < b.len() {
            let num_start = b[i].is_ascii_digit()
                || (b[i] == b'-'
                    && i + 1 < b.len()
                    && (b[i + 1].is_ascii_digit() || b[i + 1] == b'.'))
                || (b[i] == b'.' && i + 1 < b.len() && b[i + 1].is_ascii_digit());
            if num_start {
                if start < i {
                    out.push((false, &s[start..i]));
                }
                let n0 = i;
                i += 1;
                while i < b.len()
                    && (b[i].is_ascii_digit()
                        || b[i] == b'.'
                        || b[i] == b'e'
                        || b[i] == b'E'
                        || ((b[i] == b'-' || b[i] == b'+') && matches!(b[i - 1], b'e' | b'E')))
                {
                    i += 1;
                }
                out.push((true, &s[n0..i]));
                start = i;
            } else {
                i += 1;
            }
        }
        if start < b.len() {
            out.push((false, &s[start..]));
        }
        out
    }
    let (e, a) = (tokens(expected), tokens(actual));
    if e.len() != a.len() {
        return Err(format!(
            "distinta estructura ({} contra {} piezas)",
            e.len(),
            a.len()
        ));
    }
    for (k, ((en, et), (an, at))) in e.iter().zip(&a).enumerate() {
        match (en, an) {
            (true, true) => {
                let (x, y): (f64, f64) = (
                    et.parse().unwrap_or(f64::NAN),
                    at.parse().unwrap_or(f64::NAN),
                );
                // Lo que parece número pero no lo es (una versión «0.6.5») se compara como texto.
                if x.is_nan() || y.is_nan() {
                    if et != at {
                        return Err(format!("pieza {k}: {et:?} contra {at:?}"));
                    }
                } else if (x - y).abs() > tol * x.abs().max(1.0) {
                    return Err(format!("número {k}: {et} contra {at}"));
                }
            }
            (false, false) if et == at => {}
            _ => return Err(format!("pieza {k}: {et:?} contra {at:?}")),
        }
    }
    Ok(())
}

#[test]
fn spline_svg_matches_golden() {
    for c in cases().into_iter().filter(|c| compared_here(c.name)) {
        let s = &c.settings;
        let mut bitmap = binarize::binarize(&c.image, s.threshold, s.invert, s.blur_px);
        let side = s.speckle_px as usize;
        bitmap.remove_specks(side * side);
        let params = spline::SplineParams {
            corner_threshold_deg: s.corner_threshold_deg.round() as i32,
            length_threshold: 3.5 + s.smoothing * 6.5,
        };
        let svg = spline::vectorize_svg(&bitmap, &params).unwrap();
        assert!(svg.contains("<path"), "{}: sin trayectorias", c.name);
        check(&format!("golden_{}.svg", c.name), &svg);
    }
}

#[test]
fn traced_paths_match_golden() {
    for c in cases().into_iter().filter(|c| compared_here(c.name)) {
        for mode in [TraceMode::Spline, TraceMode::Polygon] {
            let settings = TraceSettings {
                mode,
                ..c.settings.clone()
            };
            let res = trace(&c.image, &settings, c.size_mm).unwrap();
            let text = format!("{:?}\n{:?}\n", res.paths, res.warnings);
            check(&format!("golden_{}_{mode:?}.paths.txt", c.name), &text);
        }
    }
}
