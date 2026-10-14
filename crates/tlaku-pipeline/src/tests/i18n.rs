//! Avisos y errores del motor en inglés (`with_locale`) y errores con código estable.

use super::*;
use tlaku_doc::JobOptions;
use tlaku_i18n::{Locale, split_code, with_locale};

fn fast_doc(speed: f64) -> Document {
    let layer = LayerSettings {
        speed,
        ..line_layer(RED)
    };
    doc(vec![rect_object("a", RED, 0.0, 0.0, 5.0, 5.0)], vec![layer])
}

#[test]
fn speed_warning_in_english() {
    let d = DeviceProfile::default();
    let job = compute(&fast_doc(48_000.0), &d, &tlaku_doc::JobOptions::default()).unwrap();
    // Por omisión, español.
    assert!(
        job.preview
            .warnings
            .iter()
            .any(|w| w.contains("llega como máximo")),
        "{:?}",
        job.preview.warnings
    );
    let warnings = with_locale(Locale::En, || {
        compute(&fast_doc(48_000.0), &d, &tlaku_doc::JobOptions::default())
            .unwrap()
            .preview
            .warnings
    });
    let w = warnings
        .iter()
        .find(|w| w.contains("reaches at most"))
        .unwrap_or_else(|| panic!("falta el aviso en inglés: {warnings:?}"));
    for needle in [
        "layer #ff0000",
        "48000 mm/min",
        "12000 mm/min",
        "darker or more burnt",
    ] {
        assert!(w.contains(needle), "falta {needle:?} en {w}");
    }
    assert!(
        !w.contains("capa") && !w.contains("máximo"),
        "quedó español: {w}"
    );
}

#[test]
fn empty_design_error_in_english_has_a_stable_code() {
    let empty = doc(vec![], vec![]);
    let es = compute(&empty, &device(), &JobOptions::default()).unwrap_err();
    let (code, text) = split_code(&es);
    assert_eq!(code, Some("nothing_to_send"));
    assert!(text.starts_with("no hay nada que producir"), "{es}");

    let en = with_locale(Locale::En, || {
        compute(&empty, &device(), &JobOptions::default()).unwrap_err()
    });
    let (code, text) = split_code(&en);
    assert_eq!(
        code,
        Some("nothing_to_send"),
        "el código no cambia con el idioma"
    );
    assert!(text.contains("nothing to produce"), "{en}");
    assert!(text.contains("the design is empty"), "{en}");
    assert!(!text.contains("vacío") && !text.contains("diseño"), "{en}");
}

#[test]
fn bed_overflow_error_has_a_stable_code_in_both_languages() {
    let job = compute(
        &doc(
            vec![rect_object("a", RED, 0.0, 0.0, 50.0, 50.0)],
            vec![line_layer(RED)],
        ),
        &device(),
        &JobOptions::default(),
    )
    .unwrap();
    let small = [10.0, 10.0];
    let es = crate::check_bounds(&job, small).unwrap_err();
    assert_eq!(split_code(&es).0, Some("bed_overflow"), "{es}");
    assert!(es.contains("se sale del área de trabajo"), "{es}");
    let en = with_locale(Locale::En, || crate::check_bounds(&job, small).unwrap_err());
    assert_eq!(split_code(&en).0, Some("bed_overflow"), "{en}");
    assert!(en.contains("goes outside the 10x10 mm work area"), "{en}");
    assert!(en.contains("move the design inside the bed"), "{en}");
}

#[test]
fn ruida_warning_in_english() {
    let d = DeviceProfile {
        kind: tlaku_doc::DeviceKind::Ruida,
        bed: [600.0, 400.0],
        max_rate: [60_000.0, 60_000.0],
        ..DeviceProfile::default()
    };
    let job = with_locale(Locale::En, || {
        compute(&fast_doc(48_000.0), &d, &tlaku_doc::JobOptions::default()).unwrap()
    });
    let w = job
        .preview
        .warnings
        .iter()
        .find(|w| w.contains("Ruida allows at most 36000"))
        .unwrap_or_else(|| panic!("falta el aviso de Ruida: {:?}", job.preview.warnings));
    assert!(w.contains("the .rd file will use 36000 mm/min"), "{w}");
    assert!(!w.contains("admite"), "quedó español: {w}");
}
