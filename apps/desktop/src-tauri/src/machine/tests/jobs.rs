//! Trabajos: inicio, pausa, cancelación, encuadre y paradas seguras durante un trabajo.

use super::*;

#[test]
fn start_job_flows_from_connection_to_finished_with_progress() {
    let rig = connect();
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.sink.wait("trabajo terminado", |s| {
        s.jobs().contains(&JobState::Finished)
    });
    assert_eq!(rig.sink.jobs(), [JobState::Started, JobState::Finished]);
    let progress = rig.sink.progress();
    assert_eq!(progress.first().unwrap().acked, 0);
    let last = progress.last().unwrap();
    assert_eq!((last.acked, last.total), (6, 6));
    assert!(progress.windows(2).all(|w| w[0].acked <= w[1].acked));
    // La máquina recibió todo el G-code, sin espacios.
    let lines = rig.fake.lines();
    for l in ["G90", "G21", "M5", "G0X1Y1", "G1X5Y5S100F1000"] {
        assert!(lines.contains(&l.to_string()), "falta {l} en {lines:?}");
    }
    // Y la conexión sigue usable después.
    assert_eq!(rig.handle.command("$$").unwrap().last().unwrap(), "ok");
    rig.sink
        .wait("sondeo tras el trabajo", |s| s.statuses().len() > 5);
}

#[test]
fn start_job_refuses_alarm_hold_door_and_run_without_sending_gcode() {
    for (state, needle) in [
        ("Alarm", "ALARMA"),
        ("Hold:0", "pausa"),
        ("Door:1", "puerta"),
        ("Run", "trabajo en curso"),
    ] {
        let rig = connect();
        rig.fake.set_state(state);
        let e = rig.handle.start_job(job_lines(), 127).unwrap_err();
        assert!(e.contains(needle), "{state}: {e}");
        assert!(e.contains("No se transmitió nada"), "{state}: {e}");
        assert!(rig.sink.jobs().is_empty(), "{state}");
        // Ni una línea de G-code: solo la identificación y las consultas.
        assert!(
            rig.fake.lines().iter().all(|l| l == "$I" || l == "$$"),
            "{state}: {:?}",
            rig.fake.lines()
        );
        // La conexión sigue viva y el sondeo continúa.
        let n = rig.sink.statuses().len();
        rig.sink.wait("sondeo", |s| s.statuses().len() > n + 2);
    }
}

#[test]
fn alarm_message_points_to_the_console_instead_of_the_cli() {
    let rig = connect();
    rig.fake.set_state("Alarm");
    let e = rig.handle.start_job(job_lines(), 127).unwrap_err();
    assert!(e.contains("consola de la pestaña Máquina"), "{e}");
    assert!(!e.contains("tlaku"), "{e}");
}

#[test]
fn invalid_gcode_is_rejected_before_touching_the_machine() {
    let rig = connect();
    let e = rig
        .handle
        .start_job(vec!["G1 X1 ?".into()], 127)
        .unwrap_err();
    assert!(e.contains("tiempo real"), "{e}");
    let e = rig.handle.start_job(vec![], 127).unwrap_err();
    assert!(e.contains("ninguna línea"), "{e}");
    assert!(rig.fake.lines().iter().all(|l| l == "$I"));
}

#[test]
fn pause_resume_and_cancel_drive_the_running_job() {
    let rig = connect();
    rig.fake.hold_ok.store(true, Ordering::SeqCst);
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.sink.wait("inicio", |s| s.jobs() == [JobState::Started]);
    // Mientras corre, no se aceptan comandos sueltos ni otro trabajo.
    assert!(
        rig.handle
            .command("$$")
            .unwrap_err()
            .contains("trabajo en curso")
    );
    assert!(rig.handle.jog(1.0, 0.0, 100.0).is_err());
    assert!(rig.handle.start_job(job_lines(), 127).is_err());

    rig.handle.pause().unwrap();
    rig.handle.resume().unwrap();
    rig.handle.cancel().unwrap();
    rig.sink
        .wait("cancelado", |s| s.jobs().contains(&JobState::Cancelled));
    assert_eq!(
        rig.sink.jobs(),
        [
            JobState::Started,
            JobState::Paused,
            JobState::Started,
            JobState::Cancelled
        ]
    );
    let rt = rig.fake.realtime();
    assert!(rt.starts_with(b"!~"), "{rt:?}");
    assert!(rt.contains(&b'!') && rt.contains(&0x18), "{rt:?}");
    // Después de cancelar la conexión sigue funcionando.
    rig.fake.hold_ok.store(false, Ordering::SeqCst);
    assert_eq!(rig.handle.command("$$").unwrap().last().unwrap(), "ok");
}

#[test]
fn reset_during_a_job_cancels_it() {
    let rig = connect();
    rig.fake.hold_ok.store(true, Ordering::SeqCst);
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.handle.realtime(RealtimeCommand::Reset).unwrap();
    rig.sink
        .wait("cancelado", |s| s.jobs().contains(&JobState::Cancelled));
}

#[test]
fn frame_runs_without_job_events_or_progress() {
    let rig = connect();
    let lines = tlaku_grbl::frame::frame_gcode(
        tlaku_core::Bounds {
            min: tlaku_core::Point::new(0.0, 0.0),
            max: tlaku_core::Point::new(10.0, 5.0),
        },
        1500.0,
    );
    rig.handle.frame(lines, 127).unwrap();
    rig.sink.wait("encuadre", |s| {
        s.logs()
            .iter()
            .any(|(_, t)| tlaku_i18n::split_code(t) == (Some("frame_done"), "; encuadre terminado"))
    });
    assert!(rig.sink.jobs().is_empty());
    assert!(rig.sink.progress().is_empty());
    assert!(rig.fake.lines().contains(&"G1X10Y5S0F1500".to_string()));
    // Con la máquina en alarma tampoco se encuadra.
    rig.fake.set_state("Alarm");
    assert!(rig.handle.frame(vec!["G0 X1".into()], 127).is_err());
}

#[test]
fn a_creality_fault_stops_the_job_safely_and_explains_it() {
    let rig = connect();
    rig.fake.brake.store(true, Ordering::SeqCst);
    rig.fake.run_on_gcode.store(true, Ordering::SeqCst);
    *rig.fake.fault_at.lock().unwrap() = Some((3, "ERROR:05".into()));
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.sink
        .wait("falla", |s| s.jobs().contains(&JobState::Error));
    let msgs = rig.sink.job_messages();
    let m = msgs.last().unwrap();
    assert!(m.contains("ERROR:05") && m.contains("llama"), "{m}");
    assert!(m.contains("Reset") && !m.contains("error:5"), "{m}");
    // Feed hold, espera a que frene y solo entonces el reset.
    assert_eq!(rig.fake.realtime(), [b'!', 0x18]);
    assert!(!rig.fake.reset_in_motion.load(Ordering::SeqCst));
    // La conexión sigue y vuelve a estar disponible.
    assert!(!rig.handle.is_closed());
    assert_eq!(rig.handle.command("$$").unwrap().last().unwrap(), "ok");
}

#[test]
fn an_error_with_the_stop_policy_really_stops_the_machine() {
    let rig = connect();
    rig.fake.brake.store(true, Ordering::SeqCst);
    rig.fake.run_on_gcode.store(true, Ordering::SeqCst);
    let lines = ["G90", "G21", "BAD", "G0 X1 Y1", "M5"]
        .map(String::from)
        .to_vec();
    rig.handle.start_job(lines, 127).unwrap();
    rig.sink
        .wait("error", |s| s.jobs().contains(&JobState::Error));
    let m = rig.sink.job_messages().pop().unwrap();
    assert!(
        m.contains("error:20") && m.contains("línea 3") && m.contains("se detuvo"),
        "{m}"
    );
    // Antes solo se registraba y la máquina seguía con lo que tenía en el búfer.
    assert_eq!(rig.fake.realtime(), [b'!', 0x18]);
    assert!(!rig.fake.reset_in_motion.load(Ordering::SeqCst));
}

#[test]
fn cancel_waits_for_the_hold_to_finish_before_resetting() {
    let rig = connect();
    rig.fake.brake.store(true, Ordering::SeqCst);
    rig.fake.run_on_gcode.store(true, Ordering::SeqCst);
    rig.fake.hold_ok.store(true, Ordering::SeqCst);
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.handle.cancel().unwrap();
    rig.sink
        .wait("cancelado", |s| s.jobs().contains(&JobState::Cancelled));
    assert_eq!(rig.fake.realtime(), [b'!', 0x18]);
    // El reset llegó con la máquina ya quieta: sin ALARM:3 ni aviso de posición.
    assert!(!rig.fake.reset_in_motion.load(Ordering::SeqCst));
    let m = rig.sink.job_messages().pop().unwrap();
    assert!(m.contains("esperó a que frenara"), "{m}");
    assert!(!m.contains("posición"), "{m}");
}

#[test]
fn cancel_that_cannot_wait_warns_that_the_position_may_be_lost() {
    let rig = connect();
    rig.fake.brake.store(true, Ordering::SeqCst);
    rig.fake.run_on_gcode.store(true, Ordering::SeqCst);
    rig.fake.never_settle.store(true, Ordering::SeqCst);
    rig.fake.hold_ok.store(true, Ordering::SeqCst);
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.handle.cancel().unwrap();
    rig.sink
        .wait("cancelado", |s| s.jobs().contains(&JobState::Cancelled));
    assert!(rig.fake.reset_in_motion.load(Ordering::SeqCst));
    let m = rig.sink.job_messages().pop().unwrap();
    assert!(m.contains("posición puede haberse perdido"), "{m}");
    assert!(
        rig.sink
            .logs()
            .iter()
            .any(|(_, t)| t.contains("posición puede haberse perdido")),
        "{:?}",
        rig.sink.logs()
    );
}

#[test]
fn a_saturated_serial_link_is_warned_in_the_log_but_does_not_block_the_job() {
    use tlaku_grbl::link::LinkProfile;
    use tlaku_grbl::preflight::LinkLoad;
    let rig = connect();
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    // 1 500 líneas por segundo de 19 bytes a 115 200 baudios no caben (unas 600).
    let load = LinkLoad::new(115_200, 1500.0, 100, 1900);
    assert!(load.warning().is_some());
    rig.handle
        .start_job_with_load(
            job_lines(),
            LinkProfile::grbl_with_buffer(127),
            None,
            Some(load),
        )
        .unwrap();
    rig.sink.wait("trabajo terminado", |s| {
        s.jobs().contains(&JobState::Finished)
    });
    assert!(
        rig.sink
            .logs()
            .iter()
            .any(|(_, t)| t.starts_with("Aviso") && t.contains("baudios")),
        "{:?}",
        rig.sink.logs()
    );
}
