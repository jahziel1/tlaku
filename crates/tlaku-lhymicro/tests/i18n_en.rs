//! Los errores de la placa K40 (M2 Nano) salen en el idioma activo: inglés con
//! `with_locale(Locale::En, ...)`.

use tlaku_i18n::{Locale, with_locale};
use tlaku_lhymicro::session::SessionError;
use tlaku_lhymicro::status::Status;
use tlaku_lhymicro::transport::TransportError;

#[test]
fn k40_errors_in_english() {
    assert_eq!(
        SessionError::LowPower
            .to_string()
            .split(':')
            .next()
            .unwrap(),
        "la placa informa energía insuficiente (239)"
    );
    with_locale(Locale::En, || {
        let e = SessionError::LowPower.to_string();
        assert!(
            e.starts_with("the board reports insufficient power (239)"),
            "{e}"
        );
        let e = SessionError::Busy(Status::Busy).to_string();
        assert_eq!(e, "the board is not idle (busy (238))");
        let e = SessionError::Timeout {
            what: "placa lista",
        }
        .to_string();
        assert_eq!(e, "no response from the board (board ready)");
        assert_eq!(
            TransportError::Disconnected.to_string(),
            "the board was disconnected from the USB"
        );
        assert_eq!(
            Status::LowPower.describe().split(':').next().unwrap(),
            "insufficient power (239)"
        );
    });
}
