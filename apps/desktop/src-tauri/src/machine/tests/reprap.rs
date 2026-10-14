//! Marlin y Smoothieware (una línea por `ok`, sin `?`).

use super::*;

#[test]
fn marlin_is_identified_with_m115_and_reports_its_position_without_a_question_mark() {
    let rig = connect_virtual("virtual:marlin@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    let e = emu(&rig);
    // Nunca se le manda `?`, que Marlin no entiende.
    assert!(e.realtime_bytes().is_empty());
    let sent = e.received_lines();
    assert!(sent.contains(&"M115".to_string()), "{sent:?}");
    assert!(sent.contains(&"M114".to_string()), "{sent:?}");
    // La consulta de posición no ensucia el registro.
    assert!(
        !rig.sink.logs().iter().any(|(_, t)| t.contains("M114")),
        "{:?}",
        rig.sink.logs()
    );
    // Y la conexión no se da por perdida por callar.
    thread::sleep(Duration::from_millis(600));
    assert!(!rig.handle.is_closed());
}

#[test]
fn a_job_runs_on_virtual_marlin_and_smoothie_one_line_per_ok() {
    for (port, link) in [
        ("virtual:marlin@50", marlin_link()),
        ("virtual:smoothie@50", smoothie_link()),
    ] {
        let rig = connect_virtual(port);
        rig.sink.wait("estado", |s| !s.statuses().is_empty());
        rig.handle
            .start_job_link(reprap_job(), link, Some(255.0))
            .unwrap_or_else(|e| panic!("{port}: {e}"));
        rig.sink.wait("trabajo terminado", |s| {
            s.jobs().contains(&JobState::Finished)
        });
        assert_eq!(
            rig.sink.jobs(),
            [JobState::Started, JobState::Finished],
            "{port}"
        );
        let last = rig.sink.progress().pop().unwrap();
        assert_eq!((last.acked, last.total), (6, 6), "{port}");
        // Los espacios se conservan (rompería `M3 S0 I`) y el G-code llega entero.
        let got = emu(&rig).received_lines();
        for l in ["M3 S0 I", "G1 X5 Y5 S100 F1000"] {
            assert!(got.contains(&l.to_string()), "{port}: falta {l} en {got:?}");
        }
        // El estado volvió a reposo y la consola sigue.
        rig.sink.wait("reposo", |s| {
            s.statuses()
                .last()
                .is_some_and(|x| x.state == MachineState::Idle)
        });
    }
}

#[test]
fn virtual_marlin_job_can_be_paused_resumed_and_cancelled_and_warns_about_the_position() {
    let rig = connect_virtual("virtual:marlin@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle
        .start_job_link(reprap_long_job(), marlin_link(), None)
        .unwrap();
    rig.sink.wait("en marcha", |s| {
        s.statuses()
            .last()
            .is_some_and(|x| x.state == MachineState::Run)
    });
    rig.handle.pause().unwrap();
    rig.sink
        .wait("pausa", |s| s.jobs().contains(&JobState::Paused));
    rig.sink.wait("Hold", |s| {
        s.statuses()
            .last()
            .is_some_and(|x| x.state == MachineState::Hold)
    });
    rig.handle.resume().unwrap();
    rig.sink.wait("reanudado", |s| s.jobs().len() >= 3);
    rig.handle.cancel().unwrap();
    rig.sink
        .wait("cancelado", |s| s.jobs().contains(&JobState::Cancelled));
    // Marlin no tiene reinicio en caliente: se avisa de que la posición puede estar perdida.
    let ev = rig
        .sink
        .all()
        .into_iter()
        .find_map(|x| match x {
            Emit::Job(j) if j.state == JobState::Cancelled => Some(j),
            _ => None,
        })
        .unwrap();
    assert!(ev.position_lost, "{ev:?}");
    // La parada rápida llega cuando la controladora termina la línea que tenía en curso (sin
    // `EMERGENCY_PARSER`, `M410` espera su turno) y el láser queda apagado.
    let e = emu(&rig);
    // (El tiempo de `run_until` es simulado; aquí se espera en tiempo real.)
    let end = Instant::now() + Duration::from_secs(5);
    while Instant::now() < end && !e.received_lines().contains(&"M410".to_string()) {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(
        e.received_lines().contains(&"M410".to_string()),
        "{:?} estado={:?}",
        e.received_lines(),
        e.state()
    );
    assert!(!e.laser_on());
    assert!(!rig.handle.is_closed());
}

#[test]
fn a_profile_of_another_family_is_refused_before_sending_anything() {
    let rig = connect_virtual("virtual:marlin@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    let e = rig
        .handle
        .start_job_link(job_lines(), LinkProfile::grbl_with_buffer(127), None)
        .unwrap_err();
    assert!(
        e.contains("Marlin") && e.contains("GRBL") && e.contains("No se transmitió nada"),
        "{e}"
    );
    let grbl = connect_virtual("virtual:grbl11@20");
    let e = grbl
        .handle
        .start_job_link(reprap_job(), marlin_link(), None)
        .unwrap_err();
    assert!(e.contains("Marlin") && e.contains("GRBL"), "{e}");
    assert!(
        emu(&grbl)
            .received_lines()
            .iter()
            .all(|l| !l.starts_with('G'))
    );
}

#[test]
fn jog_on_virtual_marlin_uses_relative_moves_and_restores_absolute_mode() {
    let rig = connect_virtual("virtual:marlin@50");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.jog(3.0, 2.0, 1000.0).unwrap();
    let got = emu(&rig).received_lines();
    let at = got.iter().position(|l| l == "G91").expect("G91");
    assert_eq!(&got[at..at + 3], ["G91", "G1 X3 Y2 F1000", "G90"]);
    assert!(got.iter().all(|l| !l.starts_with("$J")));
    // Los comandos en tiempo real de GRBL no existen aquí.
    let e = rig.handle.realtime(RealtimeCommand::Reset).unwrap_err();
    assert!(e.contains("tiempo real"), "{e}");
    assert!(emu(&rig).realtime_bytes().is_empty());
}
