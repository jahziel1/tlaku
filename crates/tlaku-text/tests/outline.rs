//! Pruebas con una fuente OFL incluida en el repositorio (no dependen del sistema).

use std::path::PathBuf;
use tlaku_core::bezpath::{BezPath, PathSeg};
use tlaku_core::{Bounds, Point};
use tlaku_text::{Align, FontLibrary, OnPathSpec, TextSpec, outline};

fn fixture_lib() -> FontLibrary {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/BebasNeue-Regular.ttf");
    let mut lib = FontLibrary::new();
    lib.load_font_file(&path)
        .expect("la fuente de prueba debe cargar");
    lib
}

fn spec(text: &str) -> TextSpec {
    TextSpec {
        text: text.into(),
        font_family: "Bebas Neue".into(),
        size_mm: 10.0,
        ..TextSpec::default()
    }
}

fn contours(path: &BezPath) -> Vec<Vec<Point>> {
    path.flatten(0.005).into_iter().map(|p| p.points).collect()
}

fn signed_area(pts: &[Point]) -> f64 {
    (0..pts.len())
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
        / 2.0
}

fn bounds_of(path: &BezPath) -> Bounds {
    let pts: Vec<Point> = contours(path).into_iter().flatten().collect();
    let min = |f: fn(&Point) -> f64| pts.iter().map(f).fold(f64::INFINITY, f64::min);
    let max = |f: fn(&Point) -> f64| pts.iter().map(f).fold(f64::NEG_INFINITY, f64::max);
    Bounds {
        min: Point::new(min(|p| p.x), min(|p| p.y)),
        max: Point::new(max(|p| p.x), max(|p| p.y)),
    }
}

fn move_count(path: &BezPath) -> usize {
    path.0
        .iter()
        .filter(|s| matches!(s, PathSeg::MoveTo { .. }))
        .count()
}

#[test]
fn o_has_a_hole_with_opposite_winding() {
    let lib = fixture_lib();
    let out = outline(&lib, &spec("O"), None).unwrap();
    assert_eq!(out.paths.len(), 1);
    let path = &out.paths[0];
    assert_eq!(
        move_count(path),
        2,
        "la O tiene contorno exterior e interior"
    );

    // Las curvas se conservan sin aplanar.
    assert!(
        path.0
            .iter()
            .any(|s| matches!(s, PathSeg::QuadTo { .. } | PathSeg::CubicTo { .. }))
    );

    let cs = contours(path);
    let (a0, a1) = (signed_area(&cs[0]), signed_area(&cs[1]));
    assert!(a0 * a1 < 0.0, "sentidos opuestos: {a0} y {a1}");
    // Con par-impar o non-zero, el área rellena es la exterior menos el hueco.
    let (outer, inner) = if a0.abs() > a1.abs() {
        (&cs[0], &cs[1])
    } else {
        (&cs[1], &cs[0])
    };
    let (min_o, max_o) = (
        outer.iter().map(|p| p.x).fold(f64::INFINITY, f64::min),
        outer.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max),
    );
    assert!(inner.iter().all(|p| p.x > min_o && p.x < max_o));
    assert!(signed_area(inner).abs() < signed_area(outer).abs());
}

#[test]
fn b_has_two_holes() {
    let lib = fixture_lib();
    let out = outline(&lib, &spec("B"), None).unwrap();
    assert_eq!(move_count(&out.paths[0]), 3);
}

#[test]
fn size_is_em_and_baseline_is_zero() {
    let lib = fixture_lib();
    let mut s = spec("I");
    s.size_mm = 100.0;
    let b = outline(&lib, &s, None).unwrap().bounds.unwrap();
    // La "I" apoya en la línea base y su altura es la de la mayúscula (< em).
    assert!(b.min.y.abs() < 0.01, "{b:?}");
    assert!(b.max.y > 60.0 && b.max.y < 100.0, "{b:?}");
    s.size_mm = 50.0;
    let b2 = outline(&lib, &s, None).unwrap().bounds.unwrap();
    assert!((b2.max.y * 2.0 - b.max.y).abs() < 1e-6);
}

#[test]
fn kerning_makes_av_narrower() {
    let lib = fixture_lib();
    let adv = |t: &str| outline(&lib, &spec(t), None).unwrap().advance_mm;
    let sum = adv("A") + adv("V");
    assert!(adv("AV") < sum - 0.05, "AV={} A+V={}", adv("AV"), sum);
}

#[test]
fn letter_spacing_adds_between_characters() {
    let lib = fixture_lib();
    let base = outline(&lib, &spec("AAA"), None).unwrap();
    let mut s = spec("AAA");
    s.letter_spacing_mm = 2.0;
    let spaced = outline(&lib, &s, None).unwrap();
    // Dos huecos entre tres letras; no se añade espacio tras la última.
    assert!((spaced.advance_mm - base.advance_mm - 4.0).abs() < 1e-6);
    let x = |o: &tlaku_text::TextOutline, i: usize| bounds_of(&o.paths[i]).min.x;
    assert!((x(&spaced, 2) - x(&base, 2) - 4.0).abs() < 1e-6);
    s.letter_spacing_mm = -1.0;
    let tight = outline(&lib, &s, None).unwrap();
    assert!((base.advance_mm - tight.advance_mm - 2.0).abs() < 1e-6);
}

#[test]
fn multiline_and_alignment() {
    let lib = fixture_lib();
    let mut s = spec("AAAA\nA");
    s.size_mm = 20.0;
    s.line_spacing = 1.5;

    let left = outline(&lib, &s, None).unwrap();
    assert_eq!(left.paths.len(), 5);
    let first = bounds_of(&left.paths[0]);
    let last = bounds_of(&left.paths[4]);
    // Segunda línea: línea base 1.5 em más abajo (la A apoya sobre ella).
    assert!((last.min.y + 30.0).abs() < 0.01, "{last:?}");
    assert!((first.min.y).abs() < 0.01);
    assert!((last.min.x - first.min.x).abs() < 1e-6);
    // Una línea más corta no cambia el ancho del bloque.
    assert!(
        (left.advance_mm
            - outline(&lib, &spec_with(&s, "AAAA"), None)
                .unwrap()
                .advance_mm)
            .abs()
            < 1e-9
    );

    s.align = Align::Right;
    let right = outline(&lib, &s, None).unwrap();
    let r1 = bounds_of(&right.paths[3]);
    let r2 = bounds_of(&right.paths[4]);
    assert!((r1.max.x - r2.max.x).abs() < 1e-6, "{r1:?} {r2:?}");
    assert!(r1.max.x < 0.0 + 1e-6, "la derecha ancla en x=0");

    s.align = Align::Center;
    let center = outline(&lib, &s, None).unwrap();
    let c1 = bounds_of(&center.paths[0]).min.x;
    let c4 = bounds_of(&center.paths[3]).max.x;
    let c5 = bounds_of(&center.paths[4]);
    assert!(((c1 + c4) / 2.0 - (c5.min.x + c5.max.x) / 2.0).abs() < 1e-6);
}

fn spec_with(base: &TextSpec, text: &str) -> TextSpec {
    TextSpec {
        text: text.into(),
        ..base.clone()
    }
}

#[test]
fn empty_and_blank_text_give_no_geometry() {
    let lib = fixture_lib();
    let out = outline(&lib, &spec(""), None).unwrap();
    assert!(out.paths.is_empty() && out.bounds.is_none());
    let out = outline(&lib, &spec("   "), None).unwrap();
    assert!(out.paths.is_empty() && out.advance_mm > 0.0);
}

#[test]
fn contours_have_no_degenerate_segments() {
    let lib = fixture_lib();
    let out = outline(&lib, &spec("OB&@8% ÁÑ0QRS"), None).unwrap();
    assert!(!out.paths.is_empty());
    for path in &out.paths {
        let mut cur = Point::new(0.0, 0.0);
        let mut start = cur;
        let mut count = 0;
        for seg in &path.0 {
            match *seg {
                PathSeg::MoveTo { to } => {
                    cur = to;
                    start = to;
                    count = 0;
                }
                PathSeg::LineTo { to } => {
                    assert!(cur.dist(to) > 1e-6, "línea de largo cero");
                    cur = to;
                    count += 1;
                }
                PathSeg::QuadTo { ctrl, to } => {
                    assert!(cur.dist(to) > 1e-6 || cur.dist(ctrl) > 1e-6);
                    cur = to;
                    count += 1;
                }
                PathSeg::CubicTo { ctrl1, ctrl2, to } => {
                    assert!(
                        cur.dist(to) > 1e-6 || cur.dist(ctrl1) > 1e-6 || cur.dist(ctrl2) > 1e-6
                    );
                    cur = to;
                    count += 1;
                }
                PathSeg::Close => {
                    assert!(count >= 2, "contorno con menos de dos segmentos");
                    cur = start;
                }
            }
        }
    }
}

/// Arco de radio `r` centrado en el origen, de 180 a 0 grados por arriba (sentido horario).
fn top_arc(r: f64) -> BezPath {
    let k = 0.552_284_749_8 * r;
    BezPath(vec![
        PathSeg::MoveTo {
            to: Point::new(-r, 0.0),
        },
        PathSeg::CubicTo {
            ctrl1: Point::new(-r, k),
            ctrl2: Point::new(-k, r),
            to: Point::new(0.0, r),
        },
        PathSeg::CubicTo {
            ctrl1: Point::new(k, r),
            ctrl2: Point::new(r, k),
            to: Point::new(r, 0.0),
        },
    ])
}

fn angle_deg(x: f64, y: f64) -> f64 {
    y.atan2(x).to_degrees()
}

/// Punto más cercano y más lejano al origen de un glifo.
fn near_far(path: &BezPath) -> (Point, Point) {
    let pts: Vec<Point> = contours(path).into_iter().flatten().collect();
    let o = Point::new(0.0, 0.0);
    let near = pts
        .iter()
        .copied()
        .min_by(|a, b| a.dist(o).total_cmp(&b.dist(o)))
        .unwrap();
    let far = pts
        .iter()
        .copied()
        .max_by(|a, b| a.dist(o).total_cmp(&b.dist(o)))
        .unwrap();
    (near, far)
}

#[test]
fn text_on_arc_rotates_glyphs_with_tangent() {
    let lib = fixture_lib();
    let r = 50.0;
    let mut s = spec("IIIII");
    s.size_mm = 8.0;
    s.letter_spacing_mm = 4.0;
    s.on_path = Some(OnPathSpec {
        offset_mm: 20.0,
        flip: false,
    });
    let out = outline(&lib, &s, Some(&top_arc(r))).unwrap();
    assert!(out.warnings.is_empty(), "{:?}", out.warnings);
    assert_eq!(out.paths.len(), 5);

    let mut last_angle = f64::INFINITY;
    for path in &out.paths {
        let (near, far) = near_far(path);
        // La base de la I apoya sobre la circunferencia y la cabeza apunta hacia fuera.
        assert!(
            (near.dist(Point::new(0.0, 0.0)) - r).abs() < 0.1,
            "{near:?}"
        );
        assert!(far.dist(Point::new(0.0, 0.0)) > r + 3.0);
        // El eje de la I (su lado más largo) coincide con la dirección radial de su centro.
        let pts = &contours(path)[0];
        let n = pts.len();
        let (a, b) = (0..n)
            .map(|i| (pts[i], pts[(i + 1) % n]))
            .max_by(|(a1, b1), (a2, b2)| a1.dist(*b1).total_cmp(&a2.dist(*b2)))
            .unwrap();
        let axis = angle_deg(b.x - a.x, b.y - a.y);
        let cx = pts.iter().map(|p| p.x).sum::<f64>() / n as f64;
        let cy = pts.iter().map(|p| p.y).sum::<f64>() / n as f64;
        let radial = angle_deg(cx, cy);
        let diff = ((axis - radial + 90.0).rem_euclid(180.0)) - 90.0;
        assert!(diff.abs() < 3.0, "eje {axis} vs radial {radial}");
        // Los glifos avanzan en sentido horario por el arco.
        assert!(radial < last_angle);
        last_angle = radial;
    }
    // Las laterales y la cima no son la misma orientación: el primero está inclinado a la izquierda.
    let (n0, _) = near_far(&out.paths[0]);
    assert!(n0.x < 0.0);
}

#[test]
fn flip_reverses_direction_and_side() {
    let lib = fixture_lib();
    let r = 50.0;
    let mut s = spec("III");
    s.size_mm = 8.0;
    s.letter_spacing_mm = 4.0;
    s.on_path = Some(OnPathSpec {
        offset_mm: 20.0,
        flip: true,
    });
    let out = outline(&lib, &s, Some(&top_arc(r))).unwrap();
    assert_eq!(out.paths.len(), 3);
    let mut last = f64::NEG_INFINITY;
    for path in &out.paths {
        let (_, far) = near_far(path);
        let c = bounds_of(path);
        // Empieza por el final del arco (a la derecha) y avanza hacia la izquierda...
        let mid = (c.min.x + c.max.x) / 2.0;
        // ...y la cabeza queda del lado interior.
        assert!(far.dist(Point::new(0.0, 0.0)) < r + 0.1);
        let ang = angle_deg(mid, (c.min.y + c.max.y) / 2.0);
        assert!(
            ang > last,
            "los glifos deben avanzar hacia la izquierda por la cima"
        );
        last = ang;
    }
    let first = bounds_of(&out.paths[0]);
    assert!(
        first.min.x > 0.0,
        "el texto invertido empieza en el extremo derecho"
    );
}

#[test]
fn text_longer_than_open_path_warns() {
    let lib = fixture_lib();
    let line = BezPath(vec![
        PathSeg::MoveTo {
            to: Point::new(0.0, 0.0),
        },
        PathSeg::LineTo {
            to: Point::new(20.0, 0.0),
        },
    ]);
    let mut s = spec("HOLA MUNDO LARGO");
    s.on_path = Some(OnPathSpec::default());
    let out = outline(&lib, &s, Some(&line)).unwrap();
    assert!(
        out.warnings.iter().any(|w| w.contains("no cabe")),
        "{:?}",
        out.warnings
    );
    assert!(out.paths.len() < 14);
    // Los glifos colocados quedan dentro del largo de la trayectoria (más su propio ancho).
    let b = out.bounds.unwrap();
    assert!(b.max.x < 20.0 + 10.0);

    // Con la trayectoria cerrada y larga, sin advertencia.
    let mut s2 = spec("HOLA");
    s2.on_path = Some(OnPathSpec::default());
    let ok = outline(&lib, &s2, Some(&line)).unwrap();
    assert!(ok.warnings.is_empty(), "{:?}", ok.warnings);
}

#[test]
fn text_on_path_requires_path() {
    let lib = fixture_lib();
    let mut s = spec("A");
    s.on_path = Some(OnPathSpec::default());
    let err = outline(&lib, &s, None).unwrap_err();
    assert!(err.contains("trayectoria"));
    // Una trayectoria sin uso se ignora si el texto no va en trayecto.
    assert!(outline(&lib, &spec("A"), Some(&top_arc(10.0))).is_ok());
}

#[test]
fn unknown_font_suggests_similar_families() {
    let lib = fixture_lib();
    let mut s = spec("A");
    s.font_family = "Bebas Nue".into();
    let err = outline(&lib, &s, None).unwrap_err();
    assert!(
        err.contains("Bebas Nue") && err.contains("Bebas Neue"),
        "{err}"
    );

    s.font_family = "Zzzzzzzzzzz".into();
    let err = outline(&lib, &s, None).unwrap_err();
    assert!(err.contains("no hay familias parecidas"), "{err}");

    s.font_family = "  ".into();
    assert!(outline(&lib, &s, None).unwrap_err().contains("Falta"));
}

#[test]
fn missing_bold_and_italic_fall_back_with_warning() {
    let lib = fixture_lib();
    let mut s = spec("A");
    s.font_family = "bebas neue".into(); // sin distinguir mayúsculas
    s.bold = true;
    s.italic = true;
    let out = outline(&lib, &s, None).unwrap();
    assert_eq!(out.paths.len(), 1);
    assert!(
        out.warnings.iter().any(|w| w.contains("cursiva")),
        "{:?}",
        out.warnings
    );
    assert!(
        out.warnings.iter().any(|w| w.contains("negrita")),
        "{:?}",
        out.warnings
    );
    // La geometría es la misma que la de la cara regular: no hay estilos sintéticos.
    let plain = outline(&lib, &spec("A"), None).unwrap();
    assert_eq!(plain.paths, out.paths);
    assert!(plain.warnings.is_empty());
}

#[test]
fn glyphs_missing_from_font_are_reported() {
    let lib = fixture_lib();
    let out = outline(&lib, &spec("A漢B"), None).unwrap();
    assert_eq!(out.paths.len(), 2);
    assert!(
        out.warnings.iter().any(|w| w.contains('漢')),
        "{:?}",
        out.warnings
    );
}

#[test]
fn invalid_parameters_are_rejected() {
    let lib = fixture_lib();
    for bad in [0.0, -3.0, f64::NAN] {
        let mut s = spec("A");
        s.size_mm = bad;
        assert!(outline(&lib, &s, None).is_err());
    }
    let mut s = spec("A");
    s.line_spacing = 0.0;
    assert!(outline(&lib, &s, None).is_err());
}

#[test]
fn families_lists_loaded_font() {
    let lib = fixture_lib();
    let fams = lib.families();
    assert_eq!(fams.len(), 1);
    assert_eq!(fams[0].family, "Bebas Neue");
    assert_eq!(fams[0].styles, vec!["Regular".to_string()]);
}

/// Depende de las fuentes instaladas; ejecútala con `cargo test -- --ignored --nocapture`.
#[test]
#[ignore = "depende de las fuentes instaladas en el sistema"]
fn lists_system_families() {
    let lib = FontLibrary::system_shared();
    let fams = lib.families();
    println!("{} familias en el sistema", fams.len());
    for f in fams.iter().take(15) {
        println!("  {} {:?}", f.family, f.styles);
    }
    assert!(!fams.is_empty());
    assert!(
        fams.windows(2)
            .all(|w| w[0].family.to_lowercase() <= w[1].family.to_lowercase())
    );
}

// ---------------------------------------------------------------------------
// Fuentes de una línea (Hershey), curvado y soldado
// ---------------------------------------------------------------------------

const SIMPLEX: &str = "Hershey Roman Simplex";

fn line_spec(text: &str) -> TextSpec {
    TextSpec {
        text: text.into(),
        font_family: SIMPLEX.into(),
        size_mm: 10.0,
        ..TextSpec::default()
    }
}

fn is_open(path: &BezPath) -> bool {
    !path.0.iter().any(|s| matches!(s, PathSeg::Close))
        && path
            .0
            .iter()
            .all(|s| matches!(s, PathSeg::MoveTo { .. } | PathSeg::LineTo { .. }))
}

#[test]
fn hershey_text_is_open_paths_with_the_right_size() {
    // No necesita ninguna fuente cargada: las Hershey vienen incluidas.
    let lib = FontLibrary::new();
    let out = outline(&lib, &line_spec("H"), None).unwrap();
    assert_eq!(out.paths.len(), 1);
    assert!(is_open(&out.paths[0]), "{:?}", out.paths[0]);
    assert!(out.warnings.is_empty(), "{:?}", out.warnings);
    let b = bounds_of(&out.paths[0]);
    // Mayúscula = 0.72 del em; la línea base queda en y = 0.
    assert!((b.max.y - 7.2).abs() < 1e-9, "{b:?}");
    assert!(b.min.y.abs() < 1e-9);
    // 'H' de Roman Simplex: 14 unidades de ancho de 21 de alto.
    assert!((b.max.x - b.min.x - 14.0 * 7.2 / 21.0).abs() < 1e-9);
}

#[test]
fn hershey_size_scales_linearly_and_spacing_applies() {
    let lib = FontLibrary::new();
    let base = outline(&lib, &line_spec("HH"), None).unwrap();
    let mut big = line_spec("HH");
    big.size_mm = 20.0;
    let big = outline(&lib, &big, None).unwrap();
    assert!((big.advance_mm - 2.0 * base.advance_mm).abs() < 1e-9);
    let mut spaced = line_spec("HH");
    spaced.letter_spacing_mm = 1.5;
    let spaced = outline(&lib, &spaced, None).unwrap();
    assert!((spaced.advance_mm - base.advance_mm - 1.5).abs() < 1e-9);
    assert_eq!(base.paths.len(), 2);
}

#[test]
fn hershey_handles_spanish_and_reports_missing_glyphs() {
    let lib = FontLibrary::new();
    let ok = outline(&lib, &line_spec("¿Año? ¡Sí! é ü ç"), None).unwrap();
    assert!(ok.warnings.is_empty(), "{:?}", ok.warnings);
    assert!(ok.paths.iter().all(is_open));
    let missing = outline(&lib, &line_spec("a€b"), None).unwrap();
    assert_eq!(missing.paths.len(), 2);
    assert!(missing.warnings.iter().any(|w| w.contains('€')));
}

#[test]
fn hershey_bold_and_italic_only_warn() {
    let lib = FontLibrary::new();
    let mut s = line_spec("A");
    s.bold = true;
    let out = outline(&lib, &s, None).unwrap();
    assert_eq!(out.paths.len(), 1);
    assert!(out.warnings.iter().any(|w| w.contains("negrita")));
}

#[test]
fn hershey_multiline_and_on_path_work() {
    let lib = FontLibrary::new();
    let two = outline(&lib, &line_spec("H\nH"), None).unwrap();
    let (a, b) = (bounds_of(&two.paths[0]), bounds_of(&two.paths[1]));
    assert!((a.min.y - b.min.y - 1.2 * 10.0).abs() < 1e-9);

    let mut s = line_spec("HH");
    s.on_path = Some(OnPathSpec::default());
    let guide = BezPath(vec![
        PathSeg::MoveTo {
            to: Point::new(0.0, 0.0),
        },
        PathSeg::LineTo {
            to: Point::new(100.0, 0.0),
        },
    ]);
    let out = outline(&lib, &s, Some(&guide)).unwrap();
    assert_eq!(out.paths.len(), 2);
    assert!(out.paths.iter().all(is_open));
}

#[test]
fn user_line_fonts_appear_in_the_catalog() {
    let lib = FontLibrary::new();
    let names: Vec<String> = lib.line_families().into_iter().map(|f| f.family).collect();
    assert!(names.iter().any(|n| n == SIMPLEX));
    assert!(names.len() >= 12, "{names:?}");
    assert!(lib.line_families().iter().all(|f| f.single_line));
    assert!(lib.families().is_empty());
    assert!(lib.line_font("hershey cursive").is_some());
    assert!(lib.line_font("Arial").is_none());
}

fn distance_from_arc_center(out: &[BezPath], x_mid: f64, radius: f64) -> (f64, f64) {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for p in out {
        for l in p.flatten(0.001) {
            for q in l.points {
                let d = (q.x - x_mid).hypot(q.y + radius);
                lo = lo.min(d);
                hi = hi.max(d);
            }
        }
    }
    (lo, hi)
}

#[test]
fn bend_wraps_text_around_an_arc() {
    let lib = FontLibrary::new();
    let mut s = line_spec("HHHH");
    s.align = Align::Center;
    let flat = outline(&lib, &s, None).unwrap();
    s.bend_deg = 180.0;
    let bent = outline(&lib, &s, None).unwrap();
    assert_eq!(bent.paths.len(), flat.paths.len());
    assert!(bent.warnings.is_empty(), "{:?}", bent.warnings);
    // 180° en una línea de ancho w: radio = w / π; centro en (0, -radio).
    let radius = flat.advance_mm / std::f64::consts::PI;
    let (lo, hi) = distance_from_arc_center(&bent.paths, 0.0, radius);
    assert!((lo - radius).abs() < 1e-6, "{lo} vs {radius}");
    assert!((hi - (radius + 7.2)).abs() < 1e-6, "{hi}");
    // Positivo = arco hacia arriba: los extremos caen por debajo de la línea base central.
    let b = bent.bounds.unwrap();
    assert!(b.min.y < -radius * 0.5, "{b:?}");
    // Las H centrales quedan rectas: su base está en y ≈ 0.
    assert!(b.max.y > 7.0);
}

#[test]
fn negative_bend_is_the_mirror_image() {
    let lib = FontLibrary::new();
    let mut s = line_spec("HHHH");
    s.align = Align::Center;
    s.bend_deg = 90.0;
    let up = outline(&lib, &s, None).unwrap().bounds.unwrap();
    s.bend_deg = -90.0;
    let down = outline(&lib, &s, None).unwrap();
    let down = down.bounds.unwrap();
    // Positivo: sube el centro y bajan los extremos; negativo: al revés.
    assert!(up.min.y < -2.0 && up.max.y < 7.3, "{up:?}");
    assert!(down.max.y > 8.0 && down.min.y > -0.1, "{down:?}");
}

#[test]
fn bend_keeps_alignment_anchor_and_ignores_zero() {
    let lib = FontLibrary::new();
    let mut s = line_spec("HOLA");
    let flat = outline(&lib, &s, None).unwrap();
    s.bend_deg = 1e-9;
    let same = outline(&lib, &s, None).unwrap();
    assert_eq!(flat.paths.len(), same.paths.len());
    s.bend_deg = 60.0;
    s.align = Align::Left;
    let left = outline(&lib, &s, None).unwrap().bounds.unwrap();
    // Alineado a la izquierda el arco se centra en el medio del bloque: el texto no se va lejos.
    assert!(
        left.min.x > -5.0 && left.max.x < flat.advance_mm + 5.0,
        "{left:?}"
    );
}

#[test]
fn bend_is_clamped_and_yields_to_text_on_path() {
    let lib = FontLibrary::new();
    let mut s = line_spec("HHHH");
    s.bend_deg = 720.0;
    let out = outline(&lib, &s, None).unwrap();
    assert!(
        out.warnings.iter().any(|w| w.contains("360")),
        "{:?}",
        out.warnings
    );
    s.bend_deg = f64::NAN;
    assert!(outline(&lib, &s, None).is_err());

    let mut s = line_spec("HH");
    s.bend_deg = 90.0;
    s.on_path = Some(OnPathSpec::default());
    let guide = BezPath(vec![
        PathSeg::MoveTo {
            to: Point::new(0.0, 0.0),
        },
        PathSeg::LineTo {
            to: Point::new(100.0, 0.0),
        },
    ]);
    let out = outline(&lib, &s, Some(&guide)).unwrap();
    assert!(out.warnings.iter().any(|w| w.contains("curvatura")));
    // Con la guía recta no se dobló nada: la base sigue en y = 0.
    assert!(out.bounds.unwrap().min.y.abs() < 1e-6);
}

#[test]
fn bend_works_on_filled_fonts_too() {
    let lib = fixture_lib();
    let mut s = spec("OLA");
    s.align = Align::Center;
    s.bend_deg = 120.0;
    let flat = outline(&lib, &spec("OLA"), None).unwrap();
    let bent = outline(&lib, &s, None).unwrap();
    assert_eq!(bent.paths.len(), 3);
    // Siguen siendo contornos cerrados.
    assert!(
        bent.paths
            .iter()
            .all(|p| p.0.iter().any(|s| matches!(s, PathSeg::Close)))
    );
    // Una "O" conserva su agujero.
    assert_eq!(move_count(&bent.paths[0]), move_count(&flat.paths[0]));
    // Al curvarse el texto gana altura (los extremos suben o bajan).
    let (fh, bh) = (
        flat.bounds.unwrap().max.y - flat.bounds.unwrap().min.y,
        bent.bounds.unwrap().max.y - bent.bounds.unwrap().min.y,
    );
    assert!(bh > fh * 1.2, "{fh} {bh}");
}

#[test]
fn welded_text_merges_touching_letters() {
    let lib = fixture_lib();
    let mut s = spec("AV");
    // Espaciado negativo: las letras se solapan.
    s.letter_spacing_mm = -2.0;
    let apart = outline(&lib, &s, None).unwrap();
    assert_eq!(apart.paths.len(), 2);
    s.welded = true;
    let welded = outline(&lib, &s, None).unwrap();
    assert_eq!(
        welded.paths.len(),
        1,
        "las letras que se tocan deben unirse"
    );
    assert!(
        welded.paths[0]
            .0
            .iter()
            .any(|s| matches!(s, PathSeg::Close))
    );
    // El área no puede crecer ni encogerse más que el solape.
    let a = |ps: &[BezPath]| -> f64 {
        ps.iter()
            .flat_map(contours)
            .map(|c| signed_area(&c).abs())
            .sum()
    };
    let (aw, aa) = (a(&welded.paths), a(&apart.paths));
    assert!(aw < aa && aw > aa * 0.5, "soldado {aw}, suelto {aa}");
}

#[test]
fn welded_text_keeps_separate_letters_and_counters() {
    let lib = fixture_lib();
    let mut s = spec("OO");
    s.welded = true;
    s.letter_spacing_mm = 5.0;
    let far = outline(&lib, &s, None).unwrap();
    assert_eq!(far.paths.len(), 2, "sin tocarse no se unen");
    assert_eq!(move_count(&far.paths[0]), 2, "la O conserva su agujero");
    s.letter_spacing_mm = -1.0;
    let near = outline(&lib, &s, None).unwrap();
    assert_eq!(near.paths.len(), 1);
    assert_eq!(move_count(&near.paths[0]), 3, "un exterior y dos agujeros");
}

#[test]
fn welding_is_skipped_for_single_line_fonts_with_a_warning() {
    let lib = FontLibrary::new();
    let mut s = line_spec("HH");
    s.welded = true;
    let out = outline(&lib, &s, None).unwrap();
    assert_eq!(out.paths.len(), 2);
    assert!(out.paths.iter().all(is_open));
    assert!(out.warnings.iter().any(|w| w.contains("soldado")));
}

#[test]
fn welded_and_bent_text_combine() {
    let lib = fixture_lib();
    let mut s = spec("AV");
    s.letter_spacing_mm = -2.0;
    s.welded = true;
    s.align = Align::Center;
    s.bend_deg = 90.0;
    let out = outline(&lib, &s, None).unwrap();
    assert_eq!(out.paths.len(), 1);
    assert!(out.bounds.is_some());
}

#[test]
fn user_shx_font_is_registered_and_used() {
    // unifont mínimo con la 'L' (arriba 4, derecha 3) y un espacio.
    let mut shx = b"AutoCAD-86 unifont 1.0\r\n\x1a".to_vec();
    shx.extend([3, 0, 0, 0, 20, 0]);
    shx.extend(b"Prueba\0");
    shx.extend([4, 1, 0, 0, 0, 0]);
    for (n, codes) in [
        (u16::from(b'L'), vec![0x44u8, 0x4C, 0x30, 0]),
        (32, vec![2, 8, 3, 0, 0]),
    ] {
        shx.extend(n.to_le_bytes());
        shx.extend(((codes.len() + 2) as u16).to_le_bytes());
        shx.extend([b'x', 0]);
        shx.extend(codes);
    }
    let lib = FontLibrary::new();
    assert_eq!(lib.load_shx_data("Mi SHX", &shx).unwrap(), "Mi SHX");
    assert!(lib.line_families().iter().any(|f| f.family == "Mi SHX"));
    let mut s = line_spec("L L");
    s.font_family = "mi shx".into();
    let out = outline(&lib, &s, None).unwrap();
    assert_eq!(out.paths.len(), 2);
    assert!(out.paths.iter().all(is_open));
    // Mayúscula de 4 unidades -> 7.2 mm; avance 3 unidades por 'L' y por espacio.
    let b = out.bounds.unwrap();
    assert!((b.max.y - 7.2).abs() < 1e-9);
    assert!(
        (out.advance_mm - 9.0 * 1.8).abs() < 1e-9,
        "{}",
        out.advance_mm
    );
    assert!(lib.load_shx_data("malo", b"nada").is_err());
}
