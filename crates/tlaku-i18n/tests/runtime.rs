//! Comportamiento de `tr!`, el idioma activo y el respaldo, con catálogo propio de prueba.

use tlaku_i18n::{
    Locale, coded, locale, locale_from_env, split_code, tr, translate_in, with_locale,
};

#[test]
fn locale_parse_understands_env_values() {
    assert_eq!(Locale::parse("en_US.UTF-8"), Some(Locale::En));
    assert_eq!(Locale::parse("en"), Some(Locale::En));
    assert_eq!(Locale::parse("es_MX.UTF-8"), Some(Locale::Es));
    assert_eq!(Locale::parse("ES-es"), Some(Locale::Es));
    assert_eq!(Locale::parse("C"), None);
    assert_eq!(Locale::parse("POSIX"), None);
    assert_eq!(Locale::parse("english"), None);
    // Aunque el entorno del que corre las pruebas sea cualquiera, no debe entrar en pánico.
    let _ = locale_from_env();
}

#[test]
fn with_locale_is_local_to_the_thread_and_restores() {
    let before = locale();
    with_locale(Locale::En, || {
        assert_eq!(locale(), Locale::En);
        with_locale(Locale::Es, || assert_eq!(locale(), Locale::Es));
        assert_eq!(locale(), Locale::En);
        std::thread::spawn(move || assert_eq!(locale(), before))
            .join()
            .unwrap();
    });
    assert_eq!(locale(), before);
    let r = std::panic::catch_unwind(|| with_locale(Locale::En, || panic!("x")));
    assert!(r.is_err());
    assert_eq!(locale(), before);
}

#[test]
fn missing_key_returns_the_key_and_never_panics() {
    assert_eq!(tr!("nada.no.existe"), "nada.no.existe");
    assert_eq!(
        translate_in(Locale::En, "nada.no.existe", &[]),
        "nada.no.existe"
    );
    assert!(
        tlaku_i18n::take_fallbacks()
            .iter()
            .any(|f| f == "?:nada.no.existe")
    );
}

#[test]
fn codes_round_trip() {
    let m = coded("gcode_zero_power", "a.nc: algo");
    assert_eq!(m, "[gcode_zero_power] a.nc: algo");
    assert_eq!(split_code(&m), (Some("gcode_zero_power"), "a.nc: algo"));
    assert_eq!(split_code("[Mayús] x"), (None, "[Mayús] x"));
    assert_eq!(split_code("sin código"), (None, "sin código"));
    assert_eq!(split_code("[] x"), (None, "[] x"));
}

#[test]
fn msg_is_matches_the_fixed_prefix_in_either_language() {
    use tlaku_i18n::msg_is;
    let key = "pipeline.import.dxf_unit_forced";
    assert!(msg_is(
        "El archivo no declara unidades: se interpretó cada unidad como 2 mm por la opción",
        key
    ));
    assert!(msg_is(
        "The file does not declare units: each unit was read as 2 mm",
        key
    ));
    assert!(!msg_is("Otro aviso", key));
    assert!(!msg_is("El archivo", "no.existe.clave"));
}
