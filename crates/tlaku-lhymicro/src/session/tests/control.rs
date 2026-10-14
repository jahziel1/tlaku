//! Pruebas de detener, pausar y reanudar.

use super::*;
use std::sync::Arc;
use std::time::Instant;

#[test]
fn stop_mid_job_cancels_turns_the_laser_off_and_forgets_the_position() {
    let (t, b) = emulator(EmuConfig::default());
    let mut s = LhySession::new(t, fast_cfg());
    s.assume_at_home();
    // Trabajo largo: lados de 150 mm a 20 mm/s.
    let program = s.encode(&square(10.0, 20.0, 150.0, 1200.0)).unwrap();
    let control = s.control();
    let b2 = Arc::clone(&b);
    let mut laser_was_on = false;
    let err = s
        .send_job(&program, &mut |p| {
            if p.packet == 4 {
                laser_was_on = b2.lock().unwrap().laser_on();
                control.request_stop();
            }
        })
        .unwrap_err();
    assert!(
        laser_was_on,
        "la prueba debe detener con el láser disparando"
    );
    assert!(matches!(err.cause, SessionError::Cancelled), "{err}");
    assert_eq!(err.stop, StopOutcome::Acked);
    assert!(!err.laser_may_be_on());
    assert_eq!(err.packets_sent, 4);
    let mut e = board(&b);
    assert!(!e.laser_on(), "el láser quedó apagado");
    e.run_to_idle();
    assert!(e.burn_length_mm() < 4.0 * 150.0, "no terminó el cuadrado");
    assert!(!e.laser_on() && !e.busy());
    assert!(e.aborts >= 1);
    drop(e);
    assert_eq!(s.position_mil(), None);
    assert_eq!(s.control().state(), JobState::Idle);
    // Tras detener hay que hacer home antes de otro trabajo.
    assert!(matches!(
        s.send_job(&program, &mut no_progress()).unwrap_err().cause,
        SessionError::UnknownPosition
    ));
    s.home().unwrap();
    assert_eq!(s.position_mil(), Some((0, 0)));
}

#[test]
fn pause_and_resume_from_another_thread() {
    let (t, b) = emulator(EmuConfig::default());
    let mut s = LhySession::new(t, fast_cfg());
    s.assume_at_home();
    let program = s.encode(&square(10.0, 20.0, 30.0, 3000.0)).unwrap();
    let control = s.control();
    let (c2, b2) = (control.clone(), Arc::clone(&b));
    let helper = std::thread::spawn(move || {
        // Espera a que la sesión haya mandado la pausa.
        let t0 = Instant::now();
        while c2.state() != JobState::Paused {
            assert!(t0.elapsed() < Duration::from_secs(5), "la pausa no llegó");
            std::thread::yield_now();
        }
        {
            let e = b2.lock().unwrap();
            assert!(e.is_paused());
            assert!(!e.laser_on(), "en pausa el láser está apagado");
        }
        std::thread::sleep(Duration::from_millis(50));
        c2.request_resume();
    });
    let r = s
        .send_job(&program, &mut |p| {
            if p.packet == 3 {
                control.request_pause();
            }
        })
        .unwrap();
    helper.join().unwrap();
    assert!(r.finish_confirmed);
    let mut e = board(&b);
    e.run_to_idle();
    assert_eq!(e.pause_toggles, 2);
    assert!(!e.is_paused());
    assert!(
        (e.burn_length_mm() - 120.0).abs() < 0.5,
        "{}",
        e.burn_length_mm()
    );
    drop(e);
    assert_eq!(s.control().state(), JobState::Idle);
}

#[test]
fn stop_while_paused_works() {
    let (t, b) = emulator(EmuConfig::default());
    let mut s = LhySession::new(t, fast_cfg());
    s.assume_at_home();
    let program = s.encode(&square(10.0, 20.0, 100.0, 1200.0)).unwrap();
    let control = s.control();
    let c2 = control.clone();
    let helper = std::thread::spawn(move || {
        let t0 = Instant::now();
        while c2.state() != JobState::Paused {
            assert!(t0.elapsed() < Duration::from_secs(5));
            std::thread::yield_now();
        }
        c2.request_stop();
    });
    let err = s
        .send_job(&program, &mut |p| {
            if p.packet == 3 {
                control.request_pause();
            }
        })
        .unwrap_err();
    helper.join().unwrap();
    assert!(matches!(err.cause, SessionError::Cancelled));
    assert_eq!(err.stop, StopOutcome::Acked);
    let e = board(&b);
    assert!(!e.is_paused() && !e.laser_on());
}
