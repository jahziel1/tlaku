//! El `Streamer` real de `tlaku-grbl` contra la controladora emulada.
//!
//! La sección final reúne las pruebas de los defectos del streamer que se corrigieron en
//! paralelo (pánico con bytes inválidos, `ERROR:NN` de Creality, cancelación en `Hold:1` y
//! silencio durante el homing). Se escribieron con `#[ignore = "pendiente: ..."]` y se
//! activaron al comprobar que pasan con el `tlaku-grbl` corregido.

mod common;

use std::time::Duration;

use common::*;
use tlaku_emu::{Fault, LineEnding, Profile, StateKind, Trigger};
use tlaku_grbl::serial::Transport;
use tlaku_grbl::streamer::{ErrorPolicy, Event, Outcome, Realtime, Streamer, StreamerConfig};

const DEADLINE: Duration = Duration::from_secs(20);

fn classic(speed: f64) -> (tlaku_emu::VirtualMachine, tlaku_emu::Handle) {
    scaled_vm(Profile::grbl_1_1h(), speed, |b| b)
}

// ---------------------------------------------------------------------------
// Trabajo completo
// ---------------------------------------------------------------------------

#[test]
fn complete_job_runs_and_respects_the_rx_budget() {
    let (vm, h) = classic(50.0);
    let lines = square_job(10.0, 10.0, 20.0, 3000, 500);
    let run = run_job(vm, fast_config(), &lines, DEADLINE);
    assert!(!run.panicked && !run.timed_out);
    assert_eq!(run.outcome, Some(Outcome::Completed { errors: 0 }));
    assert_eq!(h.state(), StateKind::Idle);
    assert_eq!(h.mpos(), [0.0, 0.0, 0.0]);
    // Lo que llegó es lo enviado, sin comentarios ni espacios y en orden.
    let sent: Vec<String> = lines
        .iter()
        .map(|l| tlaku_grbl::streamer::prepare_line(l).unwrap())
        .collect();
    assert_eq!(h.received_lines(), sent);
    assert_eq!(h.rx_overflow_bytes(), 0);
    assert!(h.max_in_flight() <= 127, "{}", h.max_in_flight());
    assert!(run.max_buffer_used() <= 127);
    // El láser solo quemó sobre el cuadrado.
    let t = h.trace();
    assert!(t.burned_during_rapids().is_empty());
    assert!(
        (t.total_length() - 80.0).abs() < 0.1,
        "{}",
        t.total_length()
    );
    assert!(!h.laser_on());
}

#[test]
fn long_job_with_planner_backpressure_never_overflows() {
    let (vm, h) = classic(100.0);
    // Una circunferencia de 300 segmentos: el planificador (15 bloques) se llena y el
    // conteo de caracteres tiene que contener al anfitrión.
    let mut lines = gl(&["G90", "G21", "M4 S0", "G0 X50 Y25"]);
    for i in 1..=300 {
        let a = f64::from(i) * std::f64::consts::TAU / 300.0;
        let (x, y) = (25.0 + 25.0 * a.cos(), 25.0 + 25.0 * a.sin());
        if i == 1 {
            lines.push(format!("G1 X{x:.3} Y{y:.3} S600 F6000"));
        } else {
            lines.push(format!("G1 X{x:.3} Y{y:.3}"));
        }
    }
    lines.push("M5".into());
    let run = run_job(vm, fast_config(), &lines, DEADLINE);
    assert_eq!(
        run.outcome,
        Some(Outcome::Completed { errors: 0 }),
        "{:?}",
        run.outcome
    );
    assert_eq!(h.rx_overflow_bytes(), 0);
    assert!(h.max_in_flight() <= 127);
    assert_eq!(h.lines_received(), lines.len());
    assert!(h.trace().burned_during_rapids().is_empty());
    let len = h.trace().total_length();
    assert!((len - 157.08).abs() < 1.0, "{len}");
}

#[test]
fn line_error_reports_source_line_and_text() {
    let (vm, h) = classic(50.0);
    let lines = gl(&["G90", "G21", "M4 S0", "G99 X1", "G1 X5 F600", "M5"]);
    let run = run_job(vm, fast_config(), &lines, DEADLINE);
    assert_eq!(
        run.outcome,
        Some(Outcome::LineError {
            code: 20,
            source_line: 4
        })
    );
    assert!(run.events.iter().any(|e| matches!(
        e,
        Event::Error { code: 20, source_line: Some(4), text: Some(t) } if t == "G99X1"
    )));
    // La línea inválida no se ejecutó y, con la política `Stop`, el streamer detiene la
    // máquina (feed hold y reset) en vez de dejar que termine lo que ya tenía en el búfer.
    assert!(wait_for(Duration::from_secs(5), || h.planner_used() == 0));
    assert!(h.mpos()[0] < 5.0, "el G1 X5 posterior no debió completarse");
    assert!(!h.laser_on());
    let rt = h.realtime_bytes();
    assert!(rt.contains(&b'!') && rt.contains(&0x18), "{rt:?}");
}

#[test]
fn continue_policy_tolerates_errors() {
    let (vm, _h) = classic(50.0);
    let lines = gl(&["G90", "G99", "G0 X5", "G0 X1Y1X2", "G0 X0"]);
    let cfg = StreamerConfig {
        error_policy: ErrorPolicy::Continue,
        ..fast_config()
    };
    let run = run_job(vm, cfg, &lines, DEADLINE);
    assert_eq!(run.outcome, Some(Outcome::Completed { errors: 2 }));
}

#[test]
fn lf_only_and_crlf_responses_are_equivalent() {
    for le in [LineEnding::Lf, LineEnding::CrLf] {
        let (vm, h) = scaled_vm(Profile::grbl_1_1h(), 50.0, |b| b.line_ending(le));
        let run = run_job(
            vm,
            fast_config(),
            &square_job(5.0, 5.0, 10.0, 3000, 400),
            DEADLINE,
        );
        assert_eq!(
            run.outcome,
            Some(Outcome::Completed { errors: 0 }),
            "{le:?}"
        );
        assert!((h.trace().total_length() - 40.0).abs() < 0.1);
    }
}

#[test]
fn every_grbl_family_profile_runs_the_square() {
    for (p, homed) in [
        (Profile::grbl_1_1h(), false),
        (Profile::falcon2_pro(), true),
        (Profile::grbl_hal(), false),
        (Profile::fluidnc(), false),
        (Profile::grbl_lpc().with_setting(30, 1000.0), false),
    ] {
        let name = p.name.clone();
        let (vm, h) = scaled_vm(p, 50.0, |b| b.homed(homed));
        let run = run_job(
            vm,
            fast_config(),
            &square_job(10.0, 10.0, 20.0, 3000, 500),
            DEADLINE,
        );
        assert_eq!(
            run.outcome,
            Some(Outcome::Completed { errors: 0 }),
            "{name}"
        );
        assert!((h.trace().total_length() - 80.0).abs() < 0.1, "{name}");
        assert!(h.trace().burned_during_rapids().is_empty(), "{name}");
    }
}

// ---------------------------------------------------------------------------
// Pausa, reanudación, cancelación
// ---------------------------------------------------------------------------

fn long_line_job() -> Vec<String> {
    gl(&[
        "G90",
        "G21",
        "M4 S0",
        "G0 X0 Y0",
        "G1 X300 S500 F600",
        "M5",
        "G0 X0 Y0",
    ])
}

#[test]
fn pause_holds_and_resume_finishes() {
    let (vm, h) = classic(50.0);
    let mut paused = false;
    let mut resumed = false;
    let mut x_at_pause = 0.0;
    let mut resume_at = None;
    let run = run_job_with(vm, fast_config(), &long_line_job(), DEADLINE, |c| {
        if !paused && h.state() == StateKind::Run && h.mpos()[0] > 5.0 {
            c.pause();
            paused = true;
        }
        if paused && !resumed && h.state() == StateKind::Hold(0) {
            let t = *resume_at.get_or_insert_with(std::time::Instant::now);
            if t.elapsed() > Duration::from_millis(100) {
                // Cien milisegundos reales (cinco simulados) parado: no se movió ni quemó.
                assert_eq!(h.mpos()[0], x_at_pause);
                assert!(!h.laser_on(), "M4 parado no debe quemar");
                c.resume();
                resumed = true;
            } else if x_at_pause == 0.0 {
                x_at_pause = h.mpos()[0];
            }
        }
    });
    assert!(paused && resumed);
    assert_eq!(run.outcome, Some(Outcome::Completed { errors: 0 }));
    let rt = h.realtime_bytes();
    let hold = rt.iter().position(|&b| b == b'!').expect("se envió !");
    let start = rt.iter().position(|&b| b == b'~').expect("se envió ~");
    assert!(hold < start);
    assert!((h.trace().total_length() - 300.0).abs() < 0.5);
    assert!(h.trace().burned_during_rapids().is_empty());
}

#[test]
fn cancel_stops_the_machine_and_turns_the_laser_off() {
    let (vm, h) = classic(50.0);
    let mut cancelled = false;
    let run = run_job_with(vm, fast_config(), &long_line_job(), DEADLINE, |c| {
        if !cancelled && h.state() == StateKind::Run && h.mpos()[0] > 5.0 {
            c.cancel();
            cancelled = true;
        }
    });
    assert_eq!(run.outcome, Some(Outcome::Cancelled));
    assert_eq!(h.resets(), 1);
    assert_eq!(h.planner_used(), 0);
    assert!(!h.laser_on());
    assert!(h.mpos()[0] < 300.0);
    let rt = h.realtime_bytes();
    assert!(rt.contains(&b'!') && rt.contains(&0x18));
}

#[test]
fn cancel_waits_for_hold_complete_so_no_alarm3() {
    // Tiempo real (x1) y 100 mm/s: frenar desde esa velocidad tarda ~100 ms, más que el
    // retardo de 50 ms entre `!` y `0x18`.
    let (vm, h) = scaled_vm(Profile::grbl_1_1h(), 1.0, |b| b);
    let lines = gl(&[
        "G90",
        "G21",
        "M4 S0",
        "G0 X0 Y0",
        "G1 X300 S500 F6000",
        "M5",
    ]);
    let mut cancelled = false;
    let run = run_job_with(vm, fast_config(), &lines, DEADLINE, |c| {
        if !cancelled && h.mpos()[0] > 30.0 {
            c.cancel();
            cancelled = true;
        }
    });
    assert_eq!(run.outcome, Some(Outcome::Cancelled));
    assert_eq!(h.alarm(), None, "tras Hold:0 el reset no debe dar alarma");
    assert!(!h.position_lost());
    assert_eq!(h.state(), StateKind::Idle);
}

#[test]
fn soft_reset_realtime_cancels_and_machine_prints_banner() {
    let (vm, h) = classic(50.0);
    let mut sent = false;
    let run = run_job_with(vm, fast_config(), &long_line_job(), DEADLINE, |c| {
        if !sent && h.state() == StateKind::Run && h.mpos()[0] > 5.0 {
            c.realtime(Realtime::SoftReset);
            sent = true;
        }
    });
    assert_eq!(run.outcome, Some(Outcome::Cancelled));
    // Fuera de un trabajo el banner posterior al reset llega al anfitrión intacto.
    let mut s = run.streamer.expect("el streamer volvió");
    s.send_realtime(Realtime::SoftReset).unwrap();
    let mut vm = s.into_transport();
    let mut buf = [0u8; 128];
    let n = vm
        .read_timeout(&mut buf, Duration::from_millis(200))
        .unwrap();
    let text = String::from_utf8_lossy(&buf[..n]).into_owned();
    assert!(text.contains("Grbl 1.1h ['$' for help]"), "{text:?}");
}

// ---------------------------------------------------------------------------
// Jog
// ---------------------------------------------------------------------------

#[test]
fn jog_through_the_streamer_moves_and_leaves_parser_alone() {
    let (vm, h) = classic(50.0);
    let lines = gl(&["$J=G91 G21 X10 Y5 F3000"]);
    let run = run_job(vm, fast_config(), &lines, DEADLINE);
    assert_eq!(run.outcome, Some(Outcome::Completed { errors: 0 }));
    assert_eq!(h.mpos(), [10.0, 5.0, 0.0]);
    assert!(!h.laser_on());
    assert_eq!(h.state(), StateKind::Idle);
    // Un trabajo posterior sigue en G90: el jog no tocó el parser.
    let run = run_job(
        run.streamer.unwrap().into_transport(),
        fast_config(),
        &gl(&["M4 S0", "G1 X20 Y20 F3000"]),
        DEADLINE,
    );
    assert_eq!(run.outcome, Some(Outcome::Completed { errors: 0 }));
    assert_eq!(h.mpos(), [20.0, 20.0, 0.0]);
}

#[test]
fn jog_cancel_stops_a_long_jog() {
    let (vm, h) = classic(50.0);
    let lines = gl(&["$J=G91 G21 X500 F600"]);
    let mut sent = false;
    let run = run_job_with(vm, fast_config(), &lines, DEADLINE, |c| {
        if !sent && h.state() == StateKind::Jog && h.mpos()[0] > 5.0 {
            c.realtime(Realtime::JogCancel);
            sent = true;
        }
    });
    assert_eq!(run.outcome, Some(Outcome::Completed { errors: 0 }));
    assert_eq!(h.state(), StateKind::Idle);
    let x = h.mpos()[0];
    assert!(x > 5.0 && x < 500.0, "{x}");
    assert_eq!(h.resets(), 0, "el jog cancel no reinicia la placa");
    assert!(h.realtime_bytes().contains(&0x85));
}

#[test]
fn jog_out_of_travel_is_an_error_not_an_alarm() {
    let (vm, h) = scaled_vm(Profile::falcon2_pro(), 50.0, |b| {
        b.homed(true).setting(20, 1.0)
    });
    let run = run_job(vm, fast_config(), &gl(&["$J=G91 G21 X999 F1000"]), DEADLINE);
    assert_eq!(
        run.outcome,
        Some(Outcome::LineError {
            code: 15,
            source_line: 1
        })
    );
    assert_eq!(h.alarm(), None);
}

// ---------------------------------------------------------------------------
// Desborde del búfer
// ---------------------------------------------------------------------------

fn slow_lines(n: usize) -> Vec<String> {
    let mut v = gl(&["G90", "G21", "M4 S0", "G91"]);
    for _ in 0..n {
        v.push("G1 X0.5 Y0.5 S100 F600".into());
    }
    v.push("M5".into());
    v
}

#[test]
fn greedy_host_overflows_and_the_emulator_reports_it() {
    let (vm, h) = classic(50.0);
    let cfg = StreamerConfig {
        rx_buffer: 4096, // miente: la controladora tiene 127
        ..fast_config()
    };
    let run = run_job(vm, cfg, &slow_lines(120), Duration::from_secs(3));
    assert!(h.rx_overflow_bytes() > 0, "el RX debió desbordarse");
    assert!(h.max_in_flight() > 127);
    // Se perdieron bytes: líneas cortadas o fundidas, de modo que faltan respuestas y el
    // trabajo no puede haber terminado limpio.
    assert_ne!(run.outcome, Some(Outcome::Completed { errors: 0 }));
}

#[test]
fn honest_host_does_not_overflow_the_same_job() {
    let (vm, h) = classic(100.0);
    let run = run_job(vm, fast_config(), &slow_lines(120), DEADLINE);
    assert_eq!(run.outcome, Some(Outcome::Completed { errors: 0 }));
    assert_eq!(h.rx_overflow_bytes(), 0);
    assert!(h.max_in_flight() <= 127);
}

// ---------------------------------------------------------------------------
// Reinicios, alarmas y desconexión
// ---------------------------------------------------------------------------

#[test]
fn banner_in_the_middle_of_a_job_is_a_reset() {
    let (vm, h) = classic(50.0);
    h.inject(Trigger::At(Duration::from_secs(2)), Fault::Reboot);
    let run = run_job(vm, fast_config(), &long_line_job(), DEADLINE);
    match run.outcome {
        Some(Outcome::Reset(ref s)) => assert!(s.starts_with("Grbl 1.1h"), "{s}"),
        ref o => panic!("se esperaba Reset, salió {o:?}"),
    }
    assert!(!h.laser_on());
}

#[test]
fn falcon2_reboot_in_the_middle_of_a_job_is_a_reset_with_a_safe_stop() {
    let (vm, h) = scaled_vm(Profile::falcon2_pro(), 50.0, |b| b.homed(true));
    h.inject(Trigger::At(Duration::from_secs(2)), Fault::Reboot);
    let run = run_job(vm, fast_config(), &long_line_job(), DEADLINE);
    // Se nota con la primera línea del arranque de Creality, antes del `Grbl 1.1f`.
    match run.outcome {
        Some(Outcome::Reset(ref s)) => assert!(s.starts_with("[MSG:Using cpu_map"), "{s}"),
        ref o => panic!("se esperaba Reset, salió {o:?}"),
    }
    // Parada segura: feed hold y soft reset tras el reinicio, láser apagado.
    let rt = h.realtime_bytes();
    assert!(rt.contains(&b'!') && rt.contains(&0x18), "{rt:?}");
    assert!(!h.laser_on());
}

#[test]
fn alarm3_then_banner_ends_as_alarm() {
    let (vm, h) = scaled_vm(Profile::falcon2_pro(), 50.0, |b| b.homed(true));
    h.inject(Trigger::At(Duration::from_secs(2)), Fault::Alarm3WithBanner);
    let run = run_job(vm, fast_config(), &long_line_job(), DEADLINE);
    assert_eq!(run.outcome, Some(Outcome::Alarm(3)));
    assert!(h.position_lost());
}

#[test]
fn hard_limit_alarm_ends_the_job() {
    let (vm, h) = classic(50.0);
    h.inject(Trigger::At(Duration::from_secs(2)), Fault::Alarm(1));
    let run = run_job(vm, fast_config(), &long_line_job(), DEADLINE);
    assert_eq!(run.outcome, Some(Outcome::Alarm(1)));
    assert!(run.events.iter().any(
        |e| matches!(e, Event::Message(m) if m == "[MSG:Reset to continue]")
            || matches!(e, Event::Alarm(1))
    ));
    assert!(!h.laser_on());
}

#[test]
fn disconnect_mid_job_is_reported() {
    let (vm, h) = classic(50.0);
    h.inject(Trigger::At(Duration::from_secs(2)), Fault::Disconnect);
    let run = run_job(vm, fast_config(), &long_line_job(), DEADLINE);
    assert!(
        matches!(run.outcome, Some(Outcome::Disconnected(_))),
        "{:?}",
        run.outcome
    );
}

#[test]
fn prolonged_silence_is_reported_as_disconnection() {
    // Tiempo real (x1): el streamer mide el silencio con su propio reloj.
    let (vm, h) = scaled_vm(Profile::grbl_1_1h(), 1.0, |b| b);
    h.inject(
        Trigger::At(Duration::from_millis(300)),
        Fault::Silence {
            duration: Duration::from_secs(30),
            drop_output: true,
        },
    );
    let cfg = StreamerConfig {
        silence_timeout: Duration::from_millis(500),
        ..fast_config()
    };
    let run = run_job(vm, cfg, &gl(&["G90", "G1 X100 F600", "G0 X0"]), DEADLINE);
    assert!(
        matches!(run.outcome, Some(Outcome::Disconnected(_))),
        "{:?}",
        run.outcome
    );
}

// ---------------------------------------------------------------------------
// Defectos conocidos del streamer (activar al integrar las correcciones)
// ---------------------------------------------------------------------------

#[test]
fn invalid_bytes_on_the_line_do_not_panic_the_streamer() {
    let (vm, h) = scaled_vm(Profile::grbl_1_1h(), 50.0, |b| b);
    h.inject(
        Trigger::At(Duration::from_secs(1)),
        Fault::Noise(b"ab\xff\xfe\r\n".to_vec()),
    );
    let run = run_job(vm, fast_config(), &long_line_job(), DEADLINE);
    assert!(
        !run.panicked,
        "el hilo del streamer murió con la línea inválida"
    );
    assert_eq!(run.outcome, Some(Outcome::Completed { errors: 0 }));
}

#[test]
fn creality_error_line_ends_the_job_instead_of_hanging() {
    let (vm, h) = scaled_vm(Profile::falcon2_pro(), 50.0, |b| b.homed(true));
    h.inject(
        Trigger::At(Duration::from_secs(2)),
        Fault::CrealityError {
            code: 5,
            halt: true,
        },
    );
    let run = run_job(vm, fast_config(), &long_line_job(), Duration::from_secs(5));
    assert!(!run.panicked);
    assert!(
        !run.timed_out,
        "el trabajo quedó colgado esperando ok tras ERROR:05"
    );
    assert_eq!(run.outcome, Some(Outcome::MachineFault(5)));
    assert!(!h.laser_on());
}

#[test]
fn homing_inside_a_job_is_not_mistaken_for_a_dead_link() {
    // Clásico con $22=1: `?` no se contesta durante el homing (que dura más que el silencio).
    let (vm, h) = scaled_vm(Profile::grbl_1_1h().with_setting(22, 1.0), 1.0, |b| {
        b.homing_duration(Duration::from_millis(1500))
            .status_during_home(false)
    });
    let cfg = StreamerConfig {
        silence_timeout: Duration::from_millis(400),
        ..fast_config()
    };
    let run = run_job(vm, cfg, &gl(&["$H", "G90", "G0 X5 Y5"]), DEADLINE);
    assert_eq!(
        run.outcome,
        Some(Outcome::Completed { errors: 0 }),
        "{:?}",
        run.outcome
    );
    assert_eq!(h.mpos(), [5.0, 5.0, 0.0]);
}

#[test]
fn homing_inside_a_job_works_when_status_is_answered() {
    // Control de la prueba anterior: la Falcon sí contesta a ? durante el homing.
    let (vm, h) = scaled_vm(Profile::falcon2_pro(), 1.0, |b| {
        b.homing_duration(Duration::from_millis(800))
    });
    let cfg = StreamerConfig {
        silence_timeout: Duration::from_millis(400),
        ..fast_config()
    };
    let run = run_job(vm, cfg, &gl(&["$H", "G90", "G0 X5 Y5"]), DEADLINE);
    assert_eq!(
        run.outcome,
        Some(Outcome::Completed { errors: 0 }),
        "{:?}",
        run.outcome
    );
    assert_eq!(h.mpos(), [5.0, 5.0, 0.0]);
}

#[test]
fn streamer_type_check() {
    // Garantiza que `Streamer<VirtualMachine>` compila (el transporte es `Send + 'static`).
    fn _assert<T: Send + 'static>() {}
    _assert::<Streamer<tlaku_emu::VirtualMachine>>();
}

#[test]
fn default_auto_clock_runs_a_job_without_wall_time() {
    // `VirtualMachine::new` usa el reloj automático: 20 s simulados no tardan 20 s reales.
    let (mut vm, h) = tlaku_emu::VirtualMachine::new(Profile::grbl_1_1h());
    vm.clear_input().unwrap();
    let cfg = StreamerConfig {
        status_interval: Some(Duration::from_millis(2)),
        ..StreamerConfig::default()
    };
    let start = std::time::Instant::now();
    let run = run_job(
        vm,
        cfg,
        &gl(&["G90", "M4 S0", "G1 X20 S300 F60", "M5"]),
        DEADLINE,
    );
    assert_eq!(run.outcome, Some(Outcome::Completed { errors: 0 }));
    assert!(h.now() >= Duration::from_secs(19), "{:?}", h.now());
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "{:?}",
        start.elapsed()
    );
}
