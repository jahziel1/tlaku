//! Pruebas de home, saltos, encuadre y trabajos encadenados.

use super::*;
use crate::encode::EncodeError;
use crate::speed::Board;

#[test]
fn home_jog_and_frame_move_the_head_with_the_laser_off() {
    let (mut s, b) = session(EmuConfig::default());
    s.home().unwrap();
    assert_eq!(s.position_mil(), Some((0, 0)));
    s.jog_to(Point::new(100.0, 100.0)).unwrap();
    {
        let mut e = board(&b);
        e.run_to_idle();
        // Home arriba a la izquierda: (100, 100) de Tlaku es x=100, y=200-100=100 de la placa.
        let h = e.head_mm();
        assert!(
            (h.0 - 100.0).abs() < 0.05 && (h.1 - 100.0).abs() < 0.05,
            "{h:?}"
        );
    }
    let p = s.position_mm().unwrap();
    assert!((p.x - 100.0).abs() < 0.05 && (p.y - 100.0).abs() < 0.05);
    s.jog_by(10.0, -20.0).unwrap();
    let p = s.position_mm().unwrap();
    assert!((p.x - 110.0).abs() < 0.05 && (p.y - 80.0).abs() < 0.05);
    s.frame(Point::new(20.0, 40.0), Point::new(60.0, 90.0))
        .unwrap();
    let mut e = board(&b);
    e.run_to_idle();
    assert_eq!(
        e.burn_length_mm(),
        0.0,
        "ningún movimiento de estos enciende el láser"
    );
    assert_eq!(e.overruns, 0);
    let h = e.head_mm();
    assert!(
        (h.0 - 20.0).abs() < 0.05 && (h.1 - 160.0).abs() < 0.05,
        "{h:?}"
    );
    drop(e);
    // Home de nuevo.
    s.home().unwrap();
    board(&b).run_to_idle();
    assert_eq!(board(&b).head_mil(), (0.0, 0.0));
}

#[test]
fn jogs_outside_the_bed_are_rejected_before_moving() {
    let (mut s, b) = session(EmuConfig::default());
    s.assume_at_home();
    let before = board(&b).accepted;
    assert!(matches!(
        s.jog_to(Point::new(301.0, 10.0)),
        Err(SessionError::OutOfBed { .. })
    ));
    assert!(matches!(
        s.jog_by(-5.0, 0.0).and(s.jog_to(Point::new(10.0, 201.0))),
        Err(SessionError::OutOfBed { .. })
    ));
    // El marco se valida completo antes de moverse.
    assert!(matches!(
        s.frame(Point::new(10.0, 10.0), Point::new(400.0, 50.0)),
        Err(SessionError::OutOfBed { .. })
    ));
    assert_eq!(board(&b).accepted, before);
    assert_eq!(s.position_mil(), Some((0, 0)));
}

#[test]
fn a_program_for_another_start_or_board_is_refused() {
    let (mut s, _b) = session(EmuConfig::default());
    s.assume_at_home();
    let program = s.encode(&square(10.0, 150.0, 20.0, 1200.0)).unwrap();
    s.jog_to(Point::new(50.0, 50.0)).unwrap();
    let e = s.send_job(&program, &mut no_progress()).unwrap_err();
    assert!(matches!(e.cause, SessionError::StartMismatch { .. }), "{e}");
    let mut other = program.clone();
    other.board = Board::B2;
    assert!(matches!(
        s.send_job(&other, &mut no_progress()).unwrap_err().cause,
        SessionError::BadProgram(_)
    ));
}

#[test]
fn two_jobs_in_a_row_chain_through_the_known_position() {
    let (mut s, b) = session(EmuConfig::default());
    s.assume_at_home();
    let p1 = s.encode(&square(10.0, 150.0, 20.0, 1200.0)).unwrap();
    s.send_job(&p1, &mut no_progress()).unwrap();
    let p2 = s.encode(&square(50.0, 100.0, 20.0, 1500.0)).unwrap();
    assert_eq!(p2.start_mil, p1.end_mil);
    s.send_job(&p2, &mut no_progress()).unwrap();
    let mut e = board(&b);
    e.run_to_idle();
    assert!((e.burn_length_mm() - 160.0).abs() < 0.5);
    assert_eq!(e.overruns, 0);
}

#[test]
fn disabling_strict_finish_still_completes() {
    let (t, b) = emulator(EmuConfig::default());
    let mut s = LhySession::new(
        t,
        SessionConfig {
            strict_finish: false,
            ..fast_cfg()
        },
    );
    s.assume_at_home();
    let program = s.encode(&square(10.0, 150.0, 20.0, 1200.0)).unwrap();
    s.send_job(&program, &mut no_progress()).unwrap();
    board(&b).run_to_idle();
    assert!((board(&b).burn_length_mm() - 80.0).abs() < 0.3);
}

#[test]
fn encode_errors_surface_before_any_write() {
    let (mut s, b) = session(EmuConfig::default());
    s.assume_at_home();
    let e = s.encode(&square(290.0, 150.0, 40.0, 1200.0)).unwrap_err();
    assert!(
        matches!(e, SessionError::Encode(EncodeError::OutOfBed { .. })),
        "{e}"
    );
    assert_eq!(board(&b).accepted, 0);
}
