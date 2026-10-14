//! Los errores de Ruida salen en el idioma activo: inglés con `with_locale(Locale::En, ...)`.

use tlaku_i18n::{Locale, with_locale};
use tlaku_ruida::cmd::{self, DecodeError};
use tlaku_ruida::num;
use tlaku_ruida::packet::{PacketConfig, PacketError};
use tlaku_ruida::safety::SafetyLimits;
use tlaku_ruida::session::SessionError;

#[test]
fn number_errors_in_english() {
    let e = num::power_from_pct(150.0).unwrap_err();
    assert_eq!(
        e.to_string(),
        "potencia (%): el valor 150 no cabe en el formato Ruida"
    );
    let e = with_locale(Locale::En, || {
        num::power_from_pct(150.0).unwrap_err().to_string()
    });
    assert_eq!(e, "power (%): the value 150 does not fit the Ruida format");
    let e = with_locale(Locale::En, || {
        num::mm_to_um(f64::NAN).unwrap_err().to_string()
    });
    assert_eq!(e, "coordinate: the value is not finite");
}

#[test]
fn safety_and_session_errors_in_english() {
    with_locale(Locale::En, || {
        let bad = SafetyLimits {
            bed_mm: [0.0, 100.0],
            ..SafetyLimits::default()
        };
        let e = bad.validate().unwrap_err().to_string();
        assert!(e.starts_with("invalid safety limits:"), "{e}");
        assert!(e.contains("the bed must be larger than 0 mm"), "{e}");
        let e = SessionError::Timeout { what: "sondeo" }.to_string();
        assert_eq!(e, "no response from the machine (probe)");
        assert_eq!(SessionError::Cancelled.to_string(), "cancelled by the user");
        let e = PacketError::TooShort(1).to_string();
        assert_eq!(e, "1-byte datagram: the checksum is missing");
        let _ = PacketConfig::default();
        let e = DecodeError::Invalid {
            offset: 3,
            what: "flujo vacío",
        }
        .to_string();
        assert_eq!(e, "byte 3: empty stream");
        let e = cmd::decode_stream(&[0x00]).unwrap_err().to_string();
        assert!(e.contains("is not an opcode"), "{e}");
    });
}

#[test]
fn settings_and_document_errors_in_english() {
    use tlaku_ruida::settings::{self, SettingsError};
    with_locale(Locale::En, || {
        let e = settings::check_write(0x200, 1).unwrap_err().to_string();
        assert_eq!(
            e,
            "address 0x200 is not in the list of settings Tlaku can write"
        );
        let e = settings::check_write(0x005, 5).unwrap_err().to_string();
        assert!(e.contains("outside the allowed range"), "{e}");
        let e = settings::calibrate_step(6350, 100.0, 50.0)
            .unwrap_err()
            .to_string();
        assert!(e.contains("measuring mistake"), "{e}");
        assert!(matches!(
            settings::calibrate_step(6350, 0.0, 1.0),
            Err(SettingsError::BadMeasure(_))
        ));
        assert!(
            SessionError::NotConfirmed
                .to_string()
                .contains("confirmation")
        );
        let e = SessionError::DocumentMismatch {
            index: 2,
            expected: "A".into(),
            found: Some("B".into()),
        }
        .to_string();
        assert!(e.contains("is named \"B\", not \"A\""), "{e}");
    });
    // En español por defecto.
    assert!(
        SessionError::NotConfirmed
            .to_string()
            .contains("confirmación")
    );
}
