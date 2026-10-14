//! Los avisos y errores del importador de DXF salen en el idioma activo.

use tlaku_dxf::import_dxf;
use tlaku_i18n::{Locale, msg_is, with_locale};

#[test]
fn errors_follow_the_locale() {
    assert_eq!(
        import_dxf(b"", 0.1).unwrap_err(),
        "el archivo DXF está vacío"
    );
    let en = with_locale(Locale::En, || import_dxf(b"", 0.1).unwrap_err());
    assert_eq!(en, "the DXF file is empty");
    let en = with_locale(Locale::En, || import_dxf(b"0\nEOF\n", -1.0).unwrap_err());
    assert_eq!(
        en,
        "invalid tolerance: -1 (it must be a number greater than 0)"
    );
}

#[test]
fn missing_units_warning_is_recognized_in_both_languages() {
    let es = import_dxf(b"0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n", 0.1).unwrap();
    assert!(es.warnings.iter().any(|w| msg_is(w, "dxf.lib.no_units")));
    let en = with_locale(Locale::En, || {
        import_dxf(b"0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n", 0.1).unwrap()
    });
    assert!(
        en.warnings
            .iter()
            .any(|w| w.starts_with("The file does not declare units"))
    );
    assert!(en.warnings.iter().any(|w| msg_is(w, "dxf.lib.no_units")));
}
