//! Máquina virtual (`tlaku-emu` con reloj de pared).

use super::*;

#[test]
fn virtual_ports_come_with_a_prefix_that_no_real_port_uses() {
    let ports = virtual_port::ports();
    assert!(ports.len() >= 4);
    assert!(ports.iter().all(|p| virtual_port::is_virtual(p)));
    assert!(ports.contains(&"virtual:falcon2".to_string()));
    assert!(!virtual_port::is_virtual("/dev/cu.usbserial-1420"));
    assert!(!virtual_port::is_virtual("COM3"));
    // La velocidad va en el id; lo absurdo se rechaza con un mensaje en español.
    assert!(virtual_port::open("virtual:falcon2@5").is_ok());
    for bad in [
        "virtual:nada",
        "virtual:falcon2@0",
        "virtual:falcon2@x",
        "virtual:falcon2@1000",
    ] {
        let e = virtual_port::open(bad).err().unwrap_or_default();
        assert!(e.contains("virtual"), "{bad}: {e}");
    }
}

#[test]
fn connecting_to_each_virtual_machine_identifies_its_firmware() {
    for (port, fw) in [
        ("virtual:falcon2", "Grbl 1.1f"),
        ("virtual:grbl11", "Grbl 1.1h"),
        ("virtual:grblhal", "grblHAL"),
        ("virtual:fluidnc", "FluidNC"),
        ("virtual:marlin", "Marlin"),
        ("virtual:smoothie", "Smoothie"),
    ] {
        let sink = Arc::new(Collect::default());
        let (conn, info) = Connection::start_virtual(port, sink.clone(), fast()).expect(port);
        assert!(info.firmware.starts_with(fw), "{port}: {}", info.firmware);
        assert!(conn.handle().is_virtual());
        sink.wait("estado", |s| !s.statuses().is_empty());
        // El registro lo deja claro: no hay hardware real.
        assert!(
            sink.logs()
                .iter()
                .any(|(_, t)| t.contains("Máquina virtual")),
            "{:?}",
            sink.logs()
        );
        conn.close();
    }
}

#[test]
fn a_short_job_runs_to_the_end_on_the_virtual_machine() {
    let rig = connect_virtual("virtual:grbl11@50");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.sink.wait("trabajo terminado", |s| {
        s.jobs().contains(&JobState::Finished)
    });
    assert_eq!(rig.sink.jobs(), [JobState::Started, JobState::Finished]);
    let last = rig.sink.progress().pop().unwrap();
    assert_eq!((last.acked, last.total), (6, 6));
    // La virtual recibió el G-code sin espacios, y el láser terminó apagado.
    let e = emu(&rig);
    let got = e.received_lines();
    for l in ["G90", "G21", "G0X1Y1", "G1X5Y5S100F1000"] {
        assert!(got.contains(&l.to_string()), "falta {l} en {got:?}");
    }
    e.wait_idle(Duration::from_secs(3));
    assert!(!e.laser_on());
    // Todo lo demás sigue funcionando: consola y jog.
    assert_eq!(rig.handle.command("$$").unwrap().last().unwrap(), "ok");
    rig.handle.jog(2.0, 0.0, 1000.0).unwrap();
}

#[test]
fn pause_resume_and_cancel_work_on_the_virtual_machine() {
    let rig = connect_virtual("virtual:falcon2@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.start_job(long_job(), 127).unwrap();
    let e = emu(&rig);
    assert!(e.wait_state(tlaku_emu::StateKind::Run, Duration::from_secs(3)));

    rig.handle.pause().unwrap();
    rig.sink
        .wait("pausa", |s| s.jobs().contains(&JobState::Paused));
    rig.sink.wait("Hold en la interfaz", |s| {
        s.statuses()
            .last()
            .is_some_and(|x| x.state == MachineState::Hold)
    });
    rig.handle.resume().unwrap();
    rig.sink.wait("reanudado", |s| s.jobs().len() >= 3);

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
    // Frenó antes del reset: la posición no se perdió y el láser quedó apagado.
    assert!(!e.laser_on());
    assert!(!e.position_lost());
    let cancelled = rig.sink.all().into_iter().find_map(|x| match x {
        Emit::Job(j) if j.state == JobState::Cancelled => Some(j),
        _ => None,
    });
    assert!(!cancelled.unwrap().position_lost);
    // La conexión sigue y vuelve a estar libre.
    assert!(!rig.handle.is_closed());
    assert_eq!(rig.handle.command("$$").unwrap().last().unwrap(), "ok");
}

#[test]
fn an_injected_creality_error_stops_the_virtual_job_and_explains_it() {
    let rig = connect_virtual("virtual:falcon2@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.start_job(long_job(), 127).unwrap();
    let e = emu(&rig);
    assert!(e.wait_state(tlaku_emu::StateKind::Run, Duration::from_secs(3)));
    // El mismo `ERROR:05` que inyectaría un fallo real de llama (por el mando de la interfaz).
    rig.handle.simulate_fault(SimulatedFault::Flame).unwrap();
    rig.sink
        .wait("falla", |s| s.jobs().contains(&JobState::Error));
    let m = rig.sink.job_messages().pop().unwrap();
    assert!(m.contains("ERROR:05") && m.contains("llama"), "{m}");
    assert!(
        rig.sink
            .logs()
            .iter()
            .any(|(d, t)| *d == LogDir::In && t.starts_with("ERROR:05")),
        "{:?}",
        rig.sink.logs()
    );
    // Se detuvo de forma segura: láser apagado y la conexión sigue viva.
    assert!(!e.laser_on());
    assert!(!rig.handle.is_closed());
}

#[test]
fn an_injected_error_05_is_explained_with_the_emulators_own_wording() {
    // Sin pasar por `simulate_fault`: se inyecta directo en el emulador, como lo haría una
    // prueba de la controladora.
    let rig = connect_virtual("virtual:falcon2@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.start_job(long_job(), 127).unwrap();
    let e = emu(&rig);
    assert!(e.wait_state(tlaku_emu::StateKind::Run, Duration::from_secs(3)));
    e.inject(
        tlaku_emu::Trigger::Now,
        tlaku_emu::Fault::CrealityError {
            code: 5,
            halt: true,
        },
    );
    rig.sink
        .wait("falla", |s| s.jobs().contains(&JobState::Error));
    let m = rig.sink.job_messages().pop().unwrap();
    assert!(m.contains("ERROR:05"), "{m}");
}

#[test]
fn alarm_3_during_a_virtual_job_reports_a_possibly_lost_position() {
    let rig = connect_virtual("virtual:grbl11@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.start_job(long_job(), 127).unwrap();
    let e = emu(&rig);
    assert!(e.wait_state(tlaku_emu::StateKind::Run, Duration::from_secs(3)));
    rig.handle.simulate_fault(SimulatedFault::Alarm3).unwrap();
    rig.sink
        .wait("alarma", |s| s.jobs().contains(&JobState::Error));
    let ev = rig
        .sink
        .all()
        .into_iter()
        .find_map(|x| match x {
            Emit::Job(j) if j.state == JobState::Error => Some(j),
            _ => None,
        })
        .unwrap();
    assert!(ev.message.contains("ALARM:3"), "{}", ev.message);
    assert!(ev.position_lost, "{ev:?}");
}

#[test]
fn a_virtual_disconnect_is_reported_as_a_lost_connection() {
    let rig = connect_virtual("virtual:grbl11@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle
        .simulate_fault(SimulatedFault::Disconnect)
        .unwrap();
    rig.sink.wait("conexión perdida", |s| {
        s.connections().iter().any(|c| !c.connected)
    });
    assert!(rig.handle.is_closed());
    assert_eq!(
        rig.handle
            .simulate_fault(SimulatedFault::Flame)
            .unwrap_err(),
        not_connected()
    );
}

#[test]
fn faults_cannot_be_simulated_on_a_real_machine() {
    let rig = connect();
    assert!(!rig.handle.is_virtual());
    let e = rig
        .handle
        .simulate_fault(SimulatedFault::Flame)
        .unwrap_err();
    assert!(e.contains("máquina virtual"), "{e}");
}
