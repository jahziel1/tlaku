//! El idioma global (proceso aparte para que ninguna otra prueba lo vea cambiar).

use tlaku_i18n::{Locale, locale, set_locale, with_locale};

#[test]
fn global_locale_defaults_to_spanish_and_is_settable() {
    assert_eq!(locale(), Locale::Es);
    set_locale(Locale::En);
    assert_eq!(locale(), Locale::En);
    std::thread::spawn(|| assert_eq!(locale(), Locale::En))
        .join()
        .unwrap();
    // El del hilo manda sobre el global.
    with_locale(Locale::Es, || assert_eq!(locale(), Locale::Es));
    set_locale(Locale::Es);
    assert_eq!(locale(), Locale::Es);
}
