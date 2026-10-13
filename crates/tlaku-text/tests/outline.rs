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
