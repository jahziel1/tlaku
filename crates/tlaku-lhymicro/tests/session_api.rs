//! Prueba de integración con la API pública, como la usaría la app: codificar, enviar a la placa
//! simulada con progreso, pausar y reanudar desde otro hilo, detener, y comprobar el rastro.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tlaku_core::Point;
use tlaku_core::toolpath::{Move, Toolpath, ToolpathBlock};
use tlaku_lhymicro::emu::{EmuConfig, emulator};
use tlaku_lhymicro::session::{JobState, LhySession, SessionConfig, SessionError, StopOutcome};

fn cfg() -> SessionConfig {
    SessionConfig {
        poll_backoff_max: Duration::ZERO,
        ..SessionConfig::default()
    }
}

/// Dos cuadrados de 20 mm separados, con un viaje largo entre ellos.
fn two_squares() -> Toolpath {
    let sq = |x0: f64, y0: f64| {
        let c = |x: f64, y: f64| Move::Cut {
            to: Point::new(x, y),
            power_pct: 80.0,
        };
        vec![
            Move::Travel {
                to: Point::new(x0, y0),
            },
            c(x0 + 20.0, y0),
            c(x0 + 20.0, y0 + 20.0),
            c(x0, y0 + 20.0),
            c(x0, y0),
        ]
    };
    let mut moves = sq(20.0, 150.0);
    moves.extend(sq(200.0, 40.0));
    Toolpath {
        blocks: vec![ToolpathBlock {
            speed_mm_min: 3000.0,
            moves,
            ..Default::default()
        }],
    }
}

#[test]
fn home_encode_send_and_verify_the_trace() {
    let (transport, board) = emulator(EmuConfig::default());
    let mut session = LhySession::new(transport, cfg());
    let info = session.probe().unwrap();
    assert!(info.ready);
    session.home().unwrap();
    let program = session.encode(&two_squares()).unwrap();
    assert!(!program.warnings.is_empty());
    let mut last = None;
    let report = session.send_job(&program, &mut |p| last = Some(p)).unwrap();
    assert_eq!(last.unwrap().packet, report.packets);
    assert!(report.finish_confirmed);

    let mut b = board.lock().unwrap();
    b.run_to_idle();
    // Dos cuadrados de 80 mm de perímetro; el viaje entre ellos no quema.
    assert!(
        (b.burn_length_mm() - 160.0).abs() < 1.0,
        "{}",
        b.burn_length_mm()
    );
    assert_eq!(b.overruns, 0);
    assert!(!b.laser_on());
    // Todo lo quemado queda dentro de las cajas de los dos cuadrados (ejes de la placa).
    let in_box = |p: (f64, f64), x0: f64, y0: f64| {
        p.0 >= x0 - 0.1 && p.0 <= x0 + 20.1 && p.1 >= y0 - 0.1 && p.1 <= y0 + 20.1
    };
    for (a, c) in b.burn_pieces_mm() {
        for p in [a, c] {
            // Primer cuadrado: Tlaku (20, 150..170) -> placa y = 30..50; segundo: (200, 40..60) -> y = 140..160.
            assert!(in_box(p, 20.0, 30.0) || in_box(p, 200.0, 140.0), "{p:?}");
        }
    }
}

#[test]
fn control_handle_pauses_resumes_and_stops_from_another_thread() {
    let (transport, board) = emulator(EmuConfig::default());
    let mut session = LhySession::new(transport, cfg());
    session.assume_at_home();
    let program = session.encode(&two_squares()).unwrap();
    let control = session.control();
    let (c2, b2) = (control.clone(), Arc::clone(&board));
    let helper = std::thread::spawn(move || {
        let t0 = Instant::now();
        while c2.state() != JobState::Paused {
            assert!(t0.elapsed() < Duration::from_secs(10), "la pausa no llegó");
            std::thread::yield_now();
        }
        assert!(b2.lock().unwrap().is_paused());
        c2.request_resume();
        let t0 = Instant::now();
        while c2.state() != JobState::Running {
            assert!(t0.elapsed() < Duration::from_secs(10), "no reanudó");
            std::thread::yield_now();
        }
        c2.request_stop();
    });
    let err = session
        .send_job(&program, &mut |p| {
            if p.packet == 5 {
                control.request_pause();
            }
        })
        .unwrap_err();
    helper.join().unwrap();
    assert!(matches!(err.cause, SessionError::Cancelled), "{err}");
    assert_eq!(err.stop, StopOutcome::Acked);
    assert_eq!(session.position_mil(), None);
    let b = board.lock().unwrap();
    assert!(!b.laser_on() && !b.is_paused());
    assert_eq!(b.overruns, 0);
}
