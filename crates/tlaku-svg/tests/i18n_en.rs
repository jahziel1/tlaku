//! Los avisos del importador de SVG salen en el idioma activo y se reconocen por clave.

use tlaku_i18n::{Locale, msg_is, msg_matches, with_locale};
use tlaku_svg::import_svg;

const SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="5" height="5"/></svg>"#;

#[test]
fn size_warning_follows_the_locale_and_is_recognized() {
    let es = import_svg(SVG, 0.1).unwrap();
    assert!(
        es.warnings
            .iter()
            .any(|w| w.starts_with("El SVG no define width/height absolutos"))
    );
    let en = with_locale(Locale::En, || import_svg(SVG, 0.1).unwrap());
    assert!(
        en.warnings
            .iter()
            .any(|w| w.starts_with("The SVG does not define absolute width/height"))
    );
    for doc in [&es, &en] {
        assert!(
            doc.warnings
                .iter()
                .any(|w| msg_is(w, "svg.lib.no_size_viewbox"))
        );
    }
}

#[test]
fn matches_templates_that_start_with_a_placeholder() {
    for l in [Locale::Es, Locale::En] {
        let msg = tlaku_i18n::translate_in(
            l,
            "svg.lib.images_skipped",
            &[("n", tlaku_i18n::Arg::new(&3))],
        );
        assert!(msg_matches(&msg, "svg.lib.images_skipped"), "{msg}");
        assert!(!msg_is(&msg, "svg.lib.images_skipped"));
        assert!(!msg_matches(&msg, "svg.lib.no_paths"));
    }
}
