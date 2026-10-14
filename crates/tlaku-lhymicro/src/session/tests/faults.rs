//! Pruebas de fallos: CRC, USB, placa ocupada, poca energía.

use super::*;
use crate::status::Status;
use crate::transport::TransportError;

#[test]
fn a_crc_error_is_resent_and_the_job_completes() {
    let (mut s, b) = session(EmuConfig {
        faults: EmuFaults {
            corrupt_packet: Some(1),
            ..Default::default()
        },
        ..Default::default()
    });
    s.assume_at_home();
    let program = s.encode(&square(10.0, 150.0, 20.0, 1200.0)).unwrap();
    let r = s.send_job(&program, &mut no_progress()).unwrap();
    assert_eq!(r.resends, 1);
    let mut e = board(&b);
    e.run_to_idle();
    assert_eq!(e.rejected, 1);
    assert!(
        (e.burn_length_mm() - 80.0).abs() < 0.3,
        "no se perdió ni duplicó nada"
    );
}

#[test]
fn a_late_crc_report_resends_the_previous_packet_in_order() {
    let (mut s, b) = session(EmuConfig {
        faults: EmuFaults {
            late_crc_packet: Some(2),
            ..Default::default()
        },
        ..Default::default()
    });
    s.assume_at_home();
    let program = s.encode(&square(10.0, 150.0, 20.0, 1200.0)).unwrap();
    let frames = program.frames().unwrap();
    s.send_job(&program, &mut no_progress()).unwrap();
    let mut e = board(&b);
    e.run_to_idle();
    // Las cargas aceptadas son exactamente las del trabajo, en orden y sin huecos.
    let accepted: Vec<_> = e.packets.clone();
    let want: Vec<_> = frames.iter().map(|f| f.payload).collect();
    assert_eq!(accepted, want);
    assert_eq!(e.rejected, 1);
}

#[test]
fn repeated_crc_errors_abort_the_job_and_stop_the_board() {
    let (mut s, b) = session(EmuConfig {
        faults: EmuFaults {
            corrupt_from: Some(2),
            ..Default::default()
        },
        ..Default::default()
    });
    s.assume_at_home();
    let program = s.encode(&square(10.0, 150.0, 20.0, 1200.0)).unwrap();
    let err = s.send_job(&program, &mut no_progress()).unwrap_err();
    assert!(
        matches!(err.cause, SessionError::CrcRepeated { .. }),
        "{err}"
    );
    assert_eq!(err.packets_sent, 2);
    // Con CRC malo el emulador rechaza todo, incluida la parada: no hay confirmación.
    assert_eq!(err.stop, StopOutcome::Unconfirmed);
    assert!(err.laser_may_be_on());
    assert!(err.to_string().contains("paro de emergencia"));
    assert_eq!(s.position_mil(), None);
    assert_eq!(board(&b).overruns, 0);
}

#[test]
fn a_usb_disconnect_mid_job_aborts_cleanly() {
    let (mut s, b) = session(EmuConfig {
        faults: EmuFaults {
            disconnect_after_packets: Some(2),
            ..Default::default()
        },
        ..Default::default()
    });
    s.assume_at_home();
    let program = s.encode(&square(10.0, 150.0, 20.0, 1200.0)).unwrap();
    let err = s.send_job(&program, &mut no_progress()).unwrap_err();
    assert!(
        matches!(
            err.cause,
            SessionError::Transport(TransportError::Disconnected)
        ),
        "{err}"
    );
    assert_eq!(err.stop, StopOutcome::Unconfirmed);
    assert_eq!(s.position_mil(), None);
    assert_eq!(s.control().state(), JobState::Idle);
    assert_eq!(board(&b).accepted, 2);
}

#[test]
fn a_board_stuck_busy_times_out_instead_of_writing() {
    let (t, b) = emulator(EmuConfig {
        faults: EmuFaults {
            busy_from_poll: Some(3),
            ..Default::default()
        },
        ..Default::default()
    });
    let mut s = LhySession::new(
        t,
        SessionConfig {
            busy_timeout: Duration::from_millis(100),
            ..fast_cfg()
        },
    );
    s.assume_at_home();
    let program = s.encode(&square(10.0, 150.0, 20.0, 1200.0)).unwrap();
    let err = s.send_job(&program, &mut no_progress()).unwrap_err();
    assert!(
        matches!(err.cause, SessionError::Timeout { .. })
            || matches!(err.cause, SessionError::Busy(_)),
        "{err}"
    );
    assert_eq!(board(&b).overruns, 0);
}

#[test]
fn low_power_aborts_the_job() {
    let (mut s, _b) = session(EmuConfig {
        faults: EmuFaults {
            low_power_after_packets: Some(1),
            ..Default::default()
        },
        ..Default::default()
    });
    s.assume_at_home();
    let program = s.encode(&square(10.0, 150.0, 20.0, 1200.0)).unwrap();
    let err = s.send_job(&program, &mut no_progress()).unwrap_err();
    assert!(matches!(err.cause, SessionError::LowPower), "{err}");
    assert!(err.to_string().contains("239"));
}

#[test]
fn a_status_without_data_aborts_after_many_polls() {
    let (t, _b) = emulator(EmuConfig {
        faults: EmuFaults {
            empty_from_poll: Some(3),
            ..Default::default()
        },
        ..Default::default()
    });
    let mut s = LhySession::new(
        t,
        SessionConfig {
            anomaly_polls: 20,
            ..fast_cfg()
        },
    );
    s.assume_at_home();
    let program = s.encode(&square(10.0, 150.0, 20.0, 1200.0)).unwrap();
    let err = s.send_job(&program, &mut no_progress()).unwrap_err();
    assert!(
        matches!(err.cause, SessionError::NoStatus(Status::NoReply)),
        "{err}"
    );
}
