//! Los textos del motor GRBL salen en el idioma activo: inglés con `with_locale(Locale::En, ...)`.
//! El español (idioma por omisión) lo cubren las pruebas de cada módulo.

use tlaku_grbl::net::{explain_connect_error, parse_ws_url};
use tlaku_grbl::preflight::{
    Advice, check_ready_with, is_warning_line, outcome_result_for, state_name,
};
use tlaku_grbl::streamer::{
    Firmware, MachineState, Outcome, StatusReport, StreamError, alarm_message_for,
    creality_error_message, error_message_for,
};
use tlaku_i18n::{Locale, with_locale};

fn en<T>(f: impl FnOnce() -> T) -> T {
    with_locale(Locale::En, f)
}

#[test]
fn grbl_alarm_texts_in_english() {
    en(|| {
        assert_eq!(alarm_message_for(Firmware::Grbl, 1), "hard limit triggered");
        assert_eq!(alarm_message_for(Firmware::Grbl, 99), "unknown alarm");
        assert!(alarm_message_for(Firmware::GrblHal, 10).contains("emergency stop"));
        assert!(alarm_message_for(Firmware::Unknown, 10).contains("depends on the firmware"));
        assert_eq!(
            error_message_for(Firmware::Unknown, 20),
            "unsupported or invalid G-code command"
        );
        assert!(error_message_for(Firmware::FluidNc, 39).contains("seconds"));
        assert!(creality_error_message(6).contains("lens"));
        let e = outcome_result_for(Firmware::Grbl, &Outcome::Alarm(1)).unwrap_err();
        assert!(e.starts_with("ALARM:1: hard limit triggered."), "{e}");
        let e = outcome_result_for(
            Firmware::Grbl,
            &Outcome::LineError {
                code: 9,
                source_line: 4,
            },
        )
        .unwrap_err();
        assert!(e.contains("on line 4 of the file"), "{e}");
        assert!(e.contains("unlock it with $X"), "{e}");
    });
    // Por omisión (español) no cambia.
    assert_eq!(
        alarm_message_for(Firmware::Grbl, 1),
        "límite físico activado"
    );
}

#[test]
fn preflight_advice_in_english() {
    let report = StatusReport::parse("<Alarm|MPos:0.000,0.000,0.000|FS:0,0>").unwrap();
    let msg = en(|| check_ready_with(&report, Some(6), Some(true), Advice::Console).unwrap_err());
    assert!(msg.contains("the machine is in ALARM"), "{msg}");
    assert!(
        msg.contains("Cause: ALARM:6, homing fail: reset during the cycle."),
        "{msg}"
    );
    assert!(
        msg.contains("type $H in the console of the Machine tab"),
        "{msg}"
    );
    assert!(!msg.contains("máquina"), "{msg}");
    let hold = StatusReport::parse("<Hold:0|MPos:0,0,0>").unwrap();
    let msg = en(|| check_ready_with(&hold, None, None, Advice::Console).unwrap_err());
    assert!(
        msg.starts_with("the machine is not idle, it is on hold"),
        "{msg}"
    );
    assert_eq!(en(|| state_name(MachineState::Alarm)), "ALARM");
    assert_eq!(en(|| state_name(MachineState::Idle)), "idle");
}

#[test]
fn warning_lines_are_recognised_in_any_language() {
    assert!(is_warning_line("Aviso: algo"));
    assert!(is_warning_line("Warning: something"));
    assert!(!is_warning_line("Firmware: Marlin"));
}

#[test]
fn network_and_stream_errors_in_english() {
    en(|| {
        let e = explain_connect_error(
            "192.168.1.5:23",
            &std::io::ErrorKind::ConnectionRefused.into(),
        );
        assert!(e.contains("refused the connection"), "{e}");
        let e = parse_ws_url("wss://host").unwrap_err();
        assert!(e.contains("Encrypted connections"), "{e}");
        let e = StreamError::LineTooLong {
            source_line: 3,
            len: 200,
            max: 127,
        }
        .to_string();
        assert!(e.contains("line 3 takes 200 bytes"), "{e}");
    });
}
