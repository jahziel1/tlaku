//! Conexión y estado, desconexión y la regla del silencio (`$H` y bloqueo crítico).

use super::*;

#[test]
fn firmware_name_is_extracted_from_ver() {
    let v = |s: &str| firmware_name(&[s.to_string()]);
    assert_eq!(v("[VER:1.1h.20190825:]"), "Grbl 1.1h");
    assert_eq!(v("[VER:1.1f:Falcon]"), "Grbl 1.1f");
    assert_eq!(firmware_name(&["[OPT:V,15,128]".into()]), "GRBL");
}

#[test]
fn connect_emits_connection_and_status_without_logging_queries() {
    let rig = connect();
    rig.sink.wait("estado", |s| s.statuses().len() >= 3);
    let c = rig.sink.connections();
    assert_eq!(c.len(), 1);
    assert!(c[0].connected && c[0].port.as_deref() == Some("/dev/cu.fake") && c[0].error.is_none());
    let s = rig.sink.statuses().pop().unwrap();
    assert_eq!(s.state, MachineState::Idle);
    assert_eq!(s.machine_pos, [11.0, 22.0]);
    // Posición de trabajo = máquina - WCO.
    assert_eq!(s.work_pos, [10.0, 20.0]);
    assert_eq!(rig.handle.work_pos(), Some([10.0, 20.0]));
    // El registro tiene `$I` y su respuesta, pero ningún reporte de estado.
    let logs = rig.sink.logs();
    assert!(logs.contains(&(LogDir::Out, "$I".into())));
    assert!(logs.contains(&(LogDir::In, "[VER:1.1h.20190825:]".into())));
    assert!(
        logs.iter().all(|(_, t)| !t.starts_with('<') && t != "?"),
        "{logs:?}"
    );
}

#[test]
fn a_machine_that_does_not_answer_identification_fails_to_connect() {
    let (host, _silent) = memory_pair();
    let timing = Timing {
        identify_timeout: Duration::from_millis(100),
        ..fast()
    };
    let r = Connection::start(Box::new(host), "p", Arc::new(Collect::default()), timing);
    let e = r.err().expect("debe fallar");
    assert!(e.contains("no respondió") && e.contains("GRBL 1.1"), "{e}");
}

#[test]
fn commands_return_the_reply_up_to_ok_and_are_logged() {
    let rig = connect();
    let r = rig.handle.command("$$").unwrap();
    assert_eq!(r, ["$22=0", "$30=1000", "$32=1", "ok"]);
    let r = rig.handle.command("BAD").unwrap();
    assert_eq!(r, ["error:20: comando G-code no soportado o no válido"]);
    let logs = rig.sink.logs();
    assert!(logs.contains(&(LogDir::Out, "$$".into())));
    assert!(logs.contains(&(LogDir::In, "$22=0".into())));
    // Los caracteres de tiempo real no pasan como texto.
    let e = rig.handle.command("G1 X1 !").unwrap_err();
    assert!(e.contains("tiempo real"), "{e}");
    assert!(rig.handle.command("   ").unwrap_err().contains("vacía"));
}

#[test]
fn jog_sends_a_relative_jog_line_and_explains_rejections() {
    let rig = connect();
    rig.handle.jog(10.0, -5.5, 1000.0).unwrap();
    assert!(
        rig.fake
            .lines()
            .contains(&"$J=G91 G21 X10 Y-5.5 F1000".to_string()),
        "{:?}",
        rig.fake.lines()
    );
    let e = rig.handle.jog(999.0, 0.0, 1000.0).unwrap_err();
    assert!(e.contains("error:15") || e.contains("recorrido"), "{e}");
    assert!(
        rig.handle
            .jog(1.0, 1.0, 0.0)
            .unwrap_err()
            .contains("mayor que 0")
    );
    assert!(rig.handle.jog(f64::NAN, 1.0, 100.0).is_err());
}

#[test]
fn realtime_commands_send_single_bytes_and_cancel_idle_is_a_jog_cancel() {
    let rig = connect();
    rig.handle.realtime(RealtimeCommand::Hold).unwrap();
    rig.handle.realtime(RealtimeCommand::Resume).unwrap();
    rig.handle.cancel().unwrap();
    rig.handle.realtime(RealtimeCommand::Reset).unwrap();
    rig.sink.wait("saludo tras el reinicio", |s| {
        s.logs().iter().any(|(_, t)| t.starts_with("Grbl 1.1h"))
    });
    assert_eq!(rig.fake.realtime(), [b'!', b'~', 0x85, 0x18]);
}

#[test]
fn closing_disconnects_without_error() {
    let mut rig = connect();
    rig.conn.take().unwrap().close();
    let c = rig.sink.connections();
    assert_eq!(c.len(), 2);
    assert!(!c[1].connected && c[1].error.is_none());
    assert!(rig.handle.is_closed());
    assert_eq!(rig.handle.command("$$").unwrap_err(), NOT_CONNECTED);
}

#[test]
fn closing_during_a_job_cancels_it_first() {
    let mut rig = connect();
    rig.fake.hold_ok.store(true, Ordering::SeqCst);
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.conn.take().unwrap().close();
    assert_eq!(rig.sink.jobs(), [JobState::Started, JobState::Cancelled]);
    assert!(rig.fake.realtime().contains(&0x18));
    assert!(!rig.sink.connections().last().unwrap().connected);
}

#[test]
fn a_dropped_cable_is_reported_as_a_lost_connection() {
    let rig = connect();
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    // Se corta el cable: el otro extremo desaparece.
    let Rig {
        handle, sink, fake, ..
    } = rig;
    drop(fake);
    sink.wait("conexión perdida", |s| {
        s.connections().iter().any(|c| !c.connected)
    });
    let c = sink.connections().pop().unwrap();
    assert!(
        c.error.as_deref().is_some_and(|e| e.contains("conexión")),
        "{c:?}"
    );
    assert!(handle.is_closed());
    assert_eq!(
        handle.start_job(job_lines(), 127).unwrap_err(),
        NOT_CONNECTED
    );
}

#[test]
fn a_silent_machine_is_reported_as_lost() {
    let rig = connect();
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.fake.mute.store(true, Ordering::SeqCst);
    rig.sink
        .wait("silencio", |s| s.connections().iter().any(|c| !c.connected));
    let c = rig.sink.connections().pop().unwrap();
    assert!(
        c.error
            .as_deref()
            .is_some_and(|e| e.contains("dejó de responder")),
        "{c:?}"
    );
}

#[test]
fn status_uses_wpos_when_the_controller_reports_work_coordinates() {
    let (host, mut dev) = memory_pair();
    let sink = Arc::new(Collect::default());
    dev.write_all(b"[VER:1.1h.20190825:]\r\nok\r\n").unwrap();
    let (_conn, _) = Connection::start(Box::new(host), "p", sink.clone(), fast()).unwrap();
    dev.write_all(b"<Run|WPos:3.000,4.000,0.000|Bf:12,100|FS:1500,255|WCO:1.000,1.000,0.000>\r\n")
        .unwrap();
    sink.wait("estado Run", |s| {
        s.statuses().iter().any(|x| x.state == MachineState::Run)
    });
    let s = sink
        .statuses()
        .into_iter()
        .find(|x| x.state == MachineState::Run)
        .unwrap();
    assert_eq!(s.work_pos, [3.0, 4.0]);
    assert_eq!(s.machine_pos, [4.0, 5.0]);
    assert_eq!((s.feed, s.spindle), (1500.0, 255.0));
    assert_eq!(
        s.buffer,
        BufferStatus {
            planner: 12,
            rx: 100
        }
    );
}

#[test]
fn a_long_homing_silence_is_not_a_lost_connection() {
    let rig = connect();
    // 900 ms sin contestar nada (ni `?`), más del doble del plazo normal (400 ms).
    rig.fake.home_delay_ms.store(900, Ordering::SeqCst);
    let r = rig.handle.command("$H").unwrap();
    assert_eq!(r.last().unwrap(), "ok");
    assert!(!rig.handle.is_closed());
    assert!(rig.sink.connections().iter().all(|c| c.connected));
    // Tras el homing vale otra vez el plazo normal.
    rig.sink
        .wait("sondeo tras el homing", |s| s.statuses().len() > 3);
    rig.fake.mute.store(true, Ordering::SeqCst);
    rig.sink
        .wait("silencio", |s| s.connections().iter().any(|c| !c.connected));
}

#[test]
fn a_hard_limit_lock_is_tolerated_until_the_reset_brings_the_banner() {
    let rig = connect();
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.fake.locked.store(true, Ordering::SeqCst);
    rig.fake.say("ALARM:1\r\n[MSG:Reset to continue]\r\n");
    rig.sink.wait("alarma", |s| {
        s.logs().iter().any(|(_, t)| t.starts_with("ALARM:1"))
    });
    // Más del doble del plazo normal de silencio, sin una sola respuesta a `?`.
    thread::sleep(Duration::from_millis(1000));
    assert!(!rig.handle.is_closed(), "{:?}", rig.sink.connections());
    // Un soft reset desbloquea la placa; llega el saludo y vuelven los reportes.
    let n = rig.sink.statuses().len();
    rig.handle.realtime(RealtimeCommand::Reset).unwrap();
    rig.sink.wait("saludo", |s| {
        s.logs().iter().any(|(_, t)| t.starts_with("Grbl 1.1h"))
    });
    rig.sink
        .wait("sondeo tras el reset", |s| s.statuses().len() > n + 2);
    assert!(!rig.handle.is_closed());
}

#[test]
fn a_user_reset_while_the_machine_runs_warns_that_the_position_is_lost() {
    let rig = connect();
    rig.fake.set_state("Run");
    rig.sink.wait("estado Run", |s| {
        s.statuses()
            .last()
            .is_some_and(|x| x.state == MachineState::Run)
    });
    rig.handle.realtime(RealtimeCommand::Reset).unwrap();
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
fn a_hard_limit_alarm_that_ends_a_job_does_not_end_the_connection() {
    let rig = connect();
    *rig.fake.fault_at.lock().unwrap() = Some((3, "ALARM:1\r\n[MSG:Reset to continue]".into()));
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.sink
        .wait("alarma", |s| s.jobs().contains(&JobState::Error));
    let m = rig.sink.job_messages().pop().unwrap();
    assert!(m.contains("ALARM:1") && m.contains("límite físico"), "{m}");
    // La placa quedó bloqueada y muda: más del doble del plazo normal sin que se dé por perdida.
    thread::sleep(Duration::from_millis(1000));
    assert!(!rig.handle.is_closed(), "{:?}", rig.sink.connections());
    rig.handle.realtime(RealtimeCommand::Reset).unwrap();
    let n = rig.sink.statuses().len();
    rig.sink
        .wait("sondeo tras el reset", |s| s.statuses().len() > n + 2);
}
