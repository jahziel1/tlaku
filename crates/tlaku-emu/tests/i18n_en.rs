//! Los textos de usuario del emulador salen en el idioma activo.

use tlaku_emu::profile::Profile;
use tlaku_i18n::{Locale, with_locale};

#[test]
fn profile_names_follow_the_language() {
    assert_eq!(Profile::marlin().name, "Marlin 2.x (láser)");
    with_locale(Locale::En, || {
        assert_eq!(Profile::marlin().name, "Marlin 2.x (laser)");
        assert_eq!(
            Profile::marlin_emergency_parser().name,
            "Marlin 2.x (laser, EMERGENCY_PARSER)"
        );
    });
}
