//! Funciones OpenType, fuentes variables y opciones de texto avanzado.
//!
//! `TlakuTest-Var.ttf` es una fuente sintética del proyecto (la genera `make_test_font.py`):
//! cada letra es un rectángulo de altura conocida, con ligaduras, alternativas, versalitas,
//! numerales, fracciones, kerning y cinco ejes. Bebas Neue (OFL) hace de fuente real que no trae
//! varias de esas funciones y no es variable.

use std::path::PathBuf;
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Bounds, Point};
use tlaku_text::{
    Align, FontAxis, FontFeature, FontLibrary, OnPathSpec, TextOutline, TextSpec, VAlign,
    font_capabilities, outline,
};

fn lib_with(files: &[&str]) -> FontLibrary {
    let mut lib = FontLibrary::new();
    for f in files {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(f);
        lib.load_font_file(&path)
            .expect("la fuente de prueba debe cargar");
    }
    lib
}

fn lib() -> FontLibrary {
    lib_with(&["TlakuTest-Var.ttf", "BebasNeue-Regular.ttf"])
}

fn spec(text: &str) -> TextSpec {
    TextSpec {
        text: text.into(),
        font_family: "Tlaku Test".into(),
        size_mm: 10.0,
        ..TextSpec::default()
    }
}

fn feat(tag: &str, value: u32) -> FontFeature {
    FontFeature {
        tag: tag.into(),
        value,
    }
}

fn axis(tag: &str, value: f32) -> FontAxis {
    FontAxis {
        tag: tag.into(),
        value,
    }
}

fn with_feature(text: &str, tag: &str, value: u32) -> TextSpec {
    TextSpec {
        features: vec![feat(tag, value)],
        ..spec(text)
    }
}

fn run(s: &TextSpec) -> TextOutline {
    outline(&lib(), s, None).expect("el texto debe convertirse")
}

fn bounds(s: &TextSpec) -> Bounds {
    run(s).bounds.expect("hay curvas")
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.02
}

#[test]
fn capabilities_list_only_what_the_font_declares() {
    let caps = font_capabilities(&lib(), &spec("")).unwrap();
    let tags: Vec<&str> = caps.features.iter().map(|f| f.tag.as_str()).collect();
    for want in [
        "liga", "dlig", "salt", "ss01", "ss02", "swsh", "smcp", "lnum", "onum", "tnum", "pnum",
        "frac", "kern",
    ] {
        assert!(tags.contains(&want), "falta {want} en {tags:?}");
    }
    for absent in ["ss03", "c2sc", "zero", "calt", "ccmp"] {
        assert!(!tags.contains(&absent), "no debería ofrecerse {absent}");
    }
    let kern = caps.features.iter().find(|f| f.tag == "kern").unwrap();
    assert!(kern.default_on);
    let ss01 = caps.features.iter().find(|f| f.tag == "ss01").unwrap();
    assert!(!ss01.default_on);
    // Vista previa: letras que la función cambia, y curvas distintas encendida y apagada.
    assert!(ss01.changes && ss01.sample.contains('a'), "{}", ss01.sample);
    assert!(!ss01.on.is_empty() && ss01.on != ss01.off);
    let liga = caps.features.iter().find(|f| f.tag == "liga").unwrap();
    assert!(liga.sample.contains("fi"), "{}", liga.sample);
    assert!(liga.on.len() < liga.off.len(), "la ligadura junta letras");
}

#[test]
fn capabilities_read_axes_and_named_instances() {
    let caps = font_capabilities(&lib(), &spec("")).unwrap();
    let axes: Vec<(&str, f32, f32, f32)> = caps
        .axes
        .iter()
        .map(|a| (a.tag.as_str(), a.min, a.default, a.max))
        .collect();
    assert_eq!(
        axes,
        vec![
            ("wght", 400.0, 400.0, 900.0),
            ("wdth", 100.0, 100.0, 150.0),
            ("slnt", -12.0, 0.0, 0.0),
            ("opsz", 8.0, 8.0, 72.0),
            ("TLKX", 0.0, 0.0, 100.0),
        ]
    );
    assert_eq!(caps.axes[0].name.as_deref(), Some("Weight"));
    let names: Vec<&str> = caps.instances.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, vec!["Regular", "Bold Wide"]);
    let wide = &caps.instances[1];
    assert_eq!(wide.coords[0], axis("wght", 900.0));
    assert_eq!(wide.coords[1], axis("wdth", 150.0));
}

#[test]
fn font_without_the_feature_does_not_offer_it() {
    let mut s = spec("");
    s.font_family = "Bebas Neue".into();
    let caps = font_capabilities(&lib(), &s).unwrap();
    let tags: Vec<&str> = caps.features.iter().map(|f| f.tag.as_str()).collect();
    assert!(tags.contains(&"kern") && tags.contains(&"frac") && tags.contains(&"tnum"));
    for absent in ["liga", "smcp", "ss01", "swsh"] {
        assert!(!tags.contains(&absent), "Bebas no trae {absent}");
    }
    assert!(caps.axes.is_empty() && caps.instances.is_empty());
}

#[test]
fn requesting_a_missing_feature_warns_and_changes_nothing() {
    let mut plain = spec("Hola");
    plain.font_family = "Bebas Neue".into();
    let mut asked = plain.clone();
    asked.features = vec![feat("smcp", 1), feat("ss01", 0)];
    let (a, b) = (run(&plain), run(&asked));
    assert_eq!(a.paths, b.paths, "la forma no cambia");
    assert!(
        b.warnings.iter().any(|w| w.contains("smcp")),
        "{:?}",
        b.warnings
    );
    assert!(
        !b.warnings.iter().any(|w| w.contains("ss01")),
        "apagar no avisa"
    );
}

#[test]
fn standard_ligature_toggles() {
    // liga viene encendida: «fi» es una sola forma.
    assert_eq!(run(&spec("fi")).paths.len(), 1);
    assert_eq!(run(&with_feature("fi", "liga", 0)).paths.len(), 2);
    // El espaciado entre letras la apaga, salvo que el usuario la fuerce.
    let mut spaced = spec("fi");
    spaced.letter_spacing_mm = 1.0;
    assert_eq!(run(&spaced).paths.len(), 2);
    spaced.features = vec![feat("liga", 1)];
    assert_eq!(run(&spaced).paths.len(), 1);
}

#[test]
fn discretionary_ligature_is_off_until_requested() {
    assert_eq!(run(&spec("ab")).paths.len(), 2);
    let on = run(&with_feature("ab", "dlig", 1));
    assert_eq!(on.paths.len(), 1);
    // a_b mide 640/1000 de alto frente a 700 de la b.
    assert!(close(on.bounds.unwrap().max.y, 6.4));
}

#[test]
fn stylistic_alternates_and_swash() {
    let base = bounds(&spec("a")).max.y;
    assert!(close(base, 5.0));
    assert!(close(bounds(&with_feature("a", "ss01", 1)).max.y, 5.6));
    assert!(close(bounds(&with_feature("b", "ss02", 1)).max.y, 7.6));
    assert!(close(bounds(&with_feature("b", "swsh", 1)).min.y, -0.8));
    // salt acepta el número de la alternativa.
    assert!(close(bounds(&with_feature("a", "salt", 1)).max.y, 5.6));
    assert!(close(bounds(&with_feature("a", "salt", 2)).max.y, 4.4));
    // Apagar una función que ya estaba apagada no cambia nada.
    assert_eq!(
        run(&with_feature("a", "ss01", 0)).paths,
        run(&spec("a")).paths
    );
}

#[test]
fn small_caps() {
    assert!(close(bounds(&with_feature("ab", "smcp", 1)).max.y, 4.2));
    assert!(close(bounds(&spec("ab")).max.y, 7.0));
}

#[test]
fn kerning_can_be_turned_off() {
    let kerned = run(&spec("ab")).advance_mm;
    let free = run(&with_feature("ab", "kern", 0)).advance_mm;
    assert!(close(kerned, 8.8), "{kerned}");
    assert!(close(free, 10.0), "{free}");
    // Con una fuente real (Bebas Neue) también cambia el avance.
    let mut bebas = spec("AV");
    bebas.font_family = "Bebas Neue".into();
    let with = run(&bebas).advance_mm;
    bebas.features = vec![feat("kern", 0)];
    assert!(run(&bebas).advance_mm > with);
}

#[test]
fn figure_styles() {
    let default = bounds(&spec("2"));
    assert!(close(default.max.y, 7.0));
    assert!(close(bounds(&with_feature("2", "lnum", 1)).max.y, 7.1));
    assert!(close(bounds(&with_feature("2", "onum", 1)).max.y, 4.6));
    // Tabulares: «1» ocupa tanto como «2»; proporcionales (por omisión) el «1» es más estrecho.
    let prop = run(&spec("1")).advance_mm;
    let tab = run(&with_feature("1", "tnum", 1)).advance_mm;
    assert!(close(prop, 3.0) && close(tab, 5.6), "{prop} {tab}");
    assert!(close(run(&with_feature("1", "pnum", 1)).advance_mm, 3.0));
}

#[test]
fn fractions() {
    let plain = run(&spec("1/2"));
    let frac = run(&with_feature("1/2", "frac", 1));
    assert_ne!(plain.paths, frac.paths);
    // El numerador sube (y arranca en 3.4 mm) y el denominador baja (-0.4 mm).
    let b = frac.bounds.unwrap();
    assert!(close(b.min.y, -0.4), "{b:?}");
}

#[test]
fn features_apply_to_each_line() {
    let two = run(&with_feature("ab\nab", "smcp", 1));
    let one = run(&with_feature("ab", "smcp", 1));
    assert_eq!(two.paths.len(), 4);
    assert_eq!(two.paths[0], one.paths[0]);
}

#[test]
fn rejects_malformed_tags() {
    let err = outline(&lib(), &with_feature("a", "ss1", 1), None).unwrap_err();
    assert!(err.contains("ss1"), "{err}");
    let mut s = spec("a");
    s.variations = vec![axis("weight", 5.0)];
    assert!(outline(&lib(), &s, None).is_err());
    s.variations = vec![axis("wght", f32::NAN)];
    assert!(outline(&lib(), &s, None).is_err());
}

fn varied(text: &str, axes: &[(&str, f32)]) -> TextSpec {
    TextSpec {
        variations: axes.iter().map(|(t, v)| axis(t, *v)).collect(),
        ..spec(text)
    }
}

#[test]
fn variable_font_changes_the_outline_with_each_axis() {
    let base = run(&spec("AB"));
    for (tag, value) in [
        ("wght", 900.0),
        ("wdth", 150.0),
        ("slnt", -12.0),
        ("opsz", 72.0),
        ("TLKX", 100.0),
    ] {
        let other = run(&varied("AB", &[(tag, value)]));
        assert_ne!(
            base.paths, other.paths,
            "el eje {tag} no cambia el contorno"
        );
        assert!(other.warnings.is_empty(), "{:?}", other.warnings);
    }
}

#[test]
fn variable_font_two_weights_give_different_shapes_and_advances() {
    let light = run(&varied("A", &[("wght", 400.0)]));
    let heavy = run(&varied("A", &[("wght", 900.0)]));
    assert_ne!(light.paths, heavy.paths);
    // El avance de la A pasa de 6.2 mm a 6.2 * 1.25 (con HVAR), y el contorno se ensancha igual.
    assert!(close(light.advance_mm, 6.2), "{}", light.advance_mm);
    assert!(close(heavy.advance_mm, 7.75), "{}", heavy.advance_mm);
    let (wl, wh) = (light.bounds.unwrap(), heavy.bounds.unwrap());
    assert!(wh.max.x - wh.min.x > wl.max.x - wl.min.x + 1.0);
    // Una posición intermedia queda entre las dos.
    let mid = run(&varied("A", &[("wght", 650.0)])).advance_mm;
    assert!(mid > light.advance_mm && mid < heavy.advance_mm, "{mid}");
}

#[test]
fn variable_font_is_deterministic_and_clamped() {
    let a = run(&varied("AB", &[("wght", 700.0), ("wdth", 120.0)]));
    let b = run(&varied("AB", &[("wght", 700.0), ("wdth", 120.0)]));
    assert_eq!(a, b);
    // Valores fuera del eje se acotan a sus extremos.
    assert_eq!(
        run(&varied("AB", &[("wght", 5000.0)])).paths,
        run(&varied("AB", &[("wght", 900.0)])).paths
    );
    assert_eq!(
        run(&varied("AB", &[("wght", 1.0)])).paths,
        run(&spec("AB")).paths
    );
    // Un eje que la fuente no tiene se ignora.
    assert_eq!(
        run(&varied("AB", &[("XXXX", 3.0)])).paths,
        run(&spec("AB")).paths
    );
}

#[test]
fn variations_and_features_combine() {
    let s = TextSpec {
        features: vec![feat("smcp", 1)],
        variations: vec![axis("wght", 900.0)],
        ..spec("ab")
    };
    let both = run(&s);
    let only_weight = run(&varied("ab", &[("wght", 900.0)]));
    let only_feature = run(&with_feature("ab", "smcp", 1));
    assert_ne!(both.paths, only_weight.paths);
    assert_ne!(both.paths, only_feature.paths);
}

#[test]
fn variations_on_a_static_font_warn() {
    let mut s = varied("A", &[("wght", 700.0)]);
    s.font_family = "Bebas Neue".into();
    let out = run(&s);
    assert!(
        out.warnings.iter().any(|w| w.contains("variable")),
        "{:?}",
        out.warnings
    );
    s.variations.clear();
    assert_eq!(out.paths, run(&s).paths);
}

#[test]
fn capabilities_previews_follow_the_current_axes() {
    let plain = font_capabilities(&lib(), &spec("")).unwrap();
    let heavy = font_capabilities(&lib(), &varied("", &[("wght", 900.0)])).unwrap();
    let get = |c: &tlaku_text::FontCapabilities| {
        c.features
            .iter()
            .find(|f| f.tag == "ss01")
            .unwrap()
            .on
            .clone()
    };
    assert_ne!(get(&plain), get(&heavy));
}

#[test]
fn single_line_fonts_ignore_opentype_with_a_warning() {
    let lib = FontLibrary::new();
    let s = TextSpec {
        text: "ABC".into(),
        font_family: "Hershey Simplex".into(),
        size_mm: 10.0,
        features: vec![feat("smcp", 1)],
        ..TextSpec::default()
    };
    let name = lib.line_families()[0].family.clone();
    let s = TextSpec {
        font_family: name,
        ..s
    };
    let out = outline(&lib, &s, None).unwrap();
    assert!(!out.paths.is_empty());
    assert!(
        out.warnings
            .iter()
            .any(|w| w.contains("smcp") || w.contains("OpenType"))
    );
    let caps = font_capabilities(&lib, &s).unwrap();
    assert!(caps.features.is_empty() && caps.axes.is_empty());
}

#[test]
fn uppercase_converts_before_shaping() {
    let lower = bounds(&spec("ab"));
    let mut upper = spec("ab");
    upper.uppercase = true;
    let b = bounds(&upper);
    assert!(close(lower.max.y, 7.0) && close(b.max.y, 7.0));
    // «a» minúscula mide 5.0, «A» 7.0: la forma cambia.
    assert_ne!(run(&spec("ab")).paths, run(&upper).paths);
    assert_eq!(run(&upper).paths, run(&spec("AB")).paths);
}

#[test]
fn vertical_alignment_moves_the_whole_block() {
    let at = |v: VAlign, text: &str| {
        let mut s = spec(text);
        s.valign = v;
        bounds(&s)
    };
    // Una A mide 7.0 mm (altura de la mayúscula).
    let base = at(VAlign::Baseline, "A");
    assert!(close(base.min.y, 0.0) && close(base.max.y, 7.0));
    let top = at(VAlign::Top, "A");
    assert!(close(top.max.y, 0.0) && close(top.min.y, -7.0));
    let mid = at(VAlign::Middle, "A");
    assert!(close(mid.min.y, -3.5) && close(mid.max.y, 3.5));
    let bottom = at(VAlign::Bottom, "A");
    assert!(close(bottom.min.y, 0.0));
    // Con dos líneas (interlineado 1.2 -> 12 mm), la base de la última queda en y = 0.
    let two = at(VAlign::Bottom, "A\nA");
    assert!(close(two.min.y, 0.0) && close(two.max.y, 19.0), "{two:?}");
    let two_mid = at(VAlign::Middle, "A\nA");
    assert!(close(two_mid.min.y + two_mid.max.y, 0.0), "{two_mid:?}");
}

#[test]
fn vertical_alignment_on_a_path_hangs_or_sits_the_text() {
    let line = BezPath(vec![
        tlaku_core::bezpath::PathSeg::MoveTo {
            to: Point::new(0.0, 0.0),
        },
        tlaku_core::bezpath::PathSeg::LineTo {
            to: Point::new(100.0, 0.0),
        },
    ]);
    let lib = lib();
    let run_on = |v: VAlign| {
        let mut s = spec("AA");
        s.valign = v;
        s.on_path = Some(OnPathSpec::default());
        outline(&lib, &s, Some(&line)).unwrap().bounds.unwrap()
    };
    let sitting = run_on(VAlign::Baseline);
    assert!(close(sitting.min.y, 0.0) && close(sitting.max.y, 7.0));
    let hanging = run_on(VAlign::Top);
    assert!(close(hanging.max.y, 0.0) && close(hanging.min.y, -7.0));
    let centred = run_on(VAlign::Middle);
    assert!(close(centred.min.y, -3.5));
}

#[test]
fn word_spacing_only_widens_the_spaces() {
    let base = run(&spec("A A")).advance_mm;
    let mut wide = spec("A A");
    wide.word_spacing_mm = 2.0;
    assert!(close(run(&wide).advance_mm, base + 2.0));
    let mut no_space = spec("AA");
    no_space.word_spacing_mm = 2.0;
    assert!(close(
        run(&no_space).advance_mm,
        run(&spec("AA")).advance_mm
    ));
    wide.word_spacing_mm = f64::NAN;
    assert!(outline(&lib(), &wide, None).is_err());
}

#[test]
fn word_spacing_works_with_alignment_and_line_fonts() {
    let lib = FontLibrary::new();
    let name = lib.line_families()[0].family.clone();
    let mut s = TextSpec {
        text: "I I".into(),
        font_family: name,
        size_mm: 10.0,
        align: Align::Left,
        ..TextSpec::default()
    };
    let base = outline(&lib, &s, None).unwrap().advance_mm;
    s.word_spacing_mm = 3.0;
    let wide = outline(&lib, &s, None).unwrap().advance_mm;
    assert!(close(wide, base + 3.0), "{base} {wide}");
}

#[test]
fn defaults_leave_old_documents_unchanged() {
    // Un TextSpec sin nada nuevo da lo mismo que antes: ligaduras encendidas, kerning, base en 0.
    let out = run(&spec("fi"));
    assert_eq!(out.paths.len(), 1);
    let json = r#"{"text":"A","font_family":"Tlaku Test","size_mm":10.0}"#;
    let parsed: TextSpec = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.valign, VAlign::Baseline);
    assert!(parsed.features.is_empty() && parsed.variations.is_empty() && !parsed.uppercase);
}
