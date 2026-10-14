//! Pruebas del envío de trabajos.

use super::*;

#[test]
fn full_job_in_memory_with_progress_and_state() {
    let cfg = EmuConfig {
        packets: PacketConfig {
            max_data_bytes: 300,
            ..Default::default()
        },
        ..Default::default()
    };
    let (mut s, _l, emu) = mem_session(
        cfg,
        SessionConfig {
            max_data_bytes: 300,
            ..fast_cfg()
        },
    );
    s.probe().unwrap();
    let job = square_job();
    let mut seen = Vec::new();
    let report = s
        .send_job(&job, &SendMode::Run, &AtomicBool::new(false), &mut |p| {
            seen.push(p)
        })
        .unwrap();
    assert!(report.running && report.packets > 1 && report.resends == 0);
    assert_eq!(seen.len(), report.packets);
    assert_eq!(seen.last().unwrap().packet, report.packets);
    assert!(seen.windows(2).all(|w| w[0].bytes_sent < w[1].bytes_sent));
    {
        let e = emu.lock().unwrap();
        assert!(e.running && e.run_complete());
        assert_eq!(e.rejected, 0);
    }
    // Estado: en marcha, y no se puede enviar otro trabajo.
    let st = s.status().unwrap();
    assert!(st.running && !st.is_idle());
    assert!(matches!(
        s.send_job(&job, &SendMode::Run, &AtomicBool::new(false), &mut no_progress()),
        Err(e) if matches!(e.cause, SessionError::Busy(_))
    ));
    // Pausa, reanudar, detener.
    s.pause().unwrap();
    assert!(s.status().unwrap().paused);
    s.resume().unwrap();
    assert!(!s.status().unwrap().paused);
    s.stop().unwrap();
    let st = s.status().unwrap();
    assert!(st.is_idle() && !st.paused);
    let e = emu.lock().unwrap();
    assert_eq!((e.pauses, e.resumes, e.stops), (1, 1, 1));
}

#[test]
fn store_mode_saves_a_document_and_does_not_start() {
    let (mut s, link, emu) = mem_session(EmuConfig::default(), fast_cfg());
    let job = square_job();
    let r = s
        .send_job(
            &job,
            &SendMode::Store {
                name: "CUADRO".into(),
            },
            &AtomicBool::new(false),
            &mut no_progress(),
        )
        .unwrap();
    assert!(!r.running);
    let e = emu.lock().unwrap();
    assert!(!e.running, "un documento guardado no arranca");
    assert_eq!(e.stored.len(), 1);
    assert_eq!(e.stored[0].name, "CUADRO");
    assert_eq!(e.stored[0].stream, job);
    // Los dos primeros paquetes son E8 02 y E7 01 nombre.
    let sent = link.sent();
    let p = PacketConfig::default();
    let reads_before = 3; // estado: 0x200, 0x221, 0x231
    assert_eq!(
        packet::parse_packet(&sent[reads_before], &p).unwrap(),
        [0xE8, 0x02]
    );
    assert_eq!(
        cmd::decode_stream(&packet::parse_packet(&sent[reads_before + 1], &p).unwrap()).unwrap(),
        [Command::FileName("CUADRO".into())]
    );
}

#[test]
fn nak_is_resent_and_a_lost_packet_aborts_with_a_safe_stop() {
    // NAK en el 2.º paquete de datos: se reenvía el mismo y el trabajo llega entero.
    let cfg = EmuConfig {
        faults: EmuFaults {
            nak_packet: Some(2),
            ..Default::default()
        },
        ..Default::default()
    };
    let sc = SessionConfig {
        max_data_bytes: 150,
        require_idle: false,
        ..fast_cfg()
    };
    let (mut s, _l, emu) = mem_session(cfg, sc.clone());
    let job = square_job();
    let r = s
        .send_job(
            &job,
            &SendMode::Run,
            &AtomicBool::new(false),
            &mut no_progress(),
        )
        .unwrap();
    assert_eq!(r.resends, 1);
    {
        let e = emu.lock().unwrap();
        assert_eq!(e.rejected, 1);
        assert!(e.running && e.run_complete());
    }
    // Un paquete perdido (sin respuesta): sin reintento por timeout, aborta y manda D8 01.
    let cfg = EmuConfig {
        faults: EmuFaults {
            lose_packet: Some(2),
            ..Default::default()
        },
        ..Default::default()
    };
    let (mut s, _l, emu) = mem_session(cfg, sc);
    let e = s
        .send_job(
            &job,
            &SendMode::Run,
            &AtomicBool::new(false),
            &mut no_progress(),
        )
        .unwrap_err();
    assert!(matches!(e.cause, SessionError::Timeout { .. }));
    assert_eq!(e.packets_sent, 2);
    assert_eq!(e.stop, StopOutcome::Acked);
    assert!(!e.machine_may_be_running);
    let st = emu.lock().unwrap();
    assert_eq!(st.stops, 1);
    assert!(!st.running, "tras el fallo la máquina queda detenida");
    assert!(!st.run_complete(), "el trabajo no llegó entero");
}

#[test]
fn if_the_machine_ignores_the_network_the_user_is_warned() {
    // Tras el primer paquete (con D8 00) la máquina deja de contestar: el D8 01 no se confirma.
    let cfg = EmuConfig {
        faults: EmuFaults {
            silent_from: Some(2),
            ..Default::default()
        },
        ..Default::default()
    };
    let (mut s, _l, _e) = mem_session(
        cfg,
        SessionConfig {
            max_data_bytes: 200,
            require_idle: false,
            ..fast_cfg()
        },
    );
    let e = s
        .send_job(
            &square_job(),
            &SendMode::Run,
            &AtomicBool::new(false),
            &mut no_progress(),
        )
        .unwrap_err();
    assert_eq!(e.stop, StopOutcome::Unconfirmed);
    assert!(e.machine_may_be_running);
    let text = e.to_string();
    assert!(
        text.contains("pulsa Stop") && text.contains("paro de emergencia"),
        "{text}"
    );
}

#[test]
fn store_mode_failure_does_not_send_stop_and_says_so() {
    let cfg = EmuConfig {
        faults: EmuFaults {
            lose_packet: Some(3),
            ..Default::default()
        },
        ..Default::default()
    };
    let (mut s, link, emu) = mem_session(
        cfg,
        SessionConfig {
            require_idle: false,
            max_data_bytes: 200,
            ..fast_cfg()
        },
    );
    let e = s
        .send_job(
            &square_job(),
            &SendMode::Store { name: "X".into() },
            &AtomicBool::new(false),
            &mut no_progress(),
        )
        .unwrap_err();
    assert_eq!(e.stop, StopOutcome::NotNeeded);
    assert!(!e.machine_may_be_running);
    assert!(e.to_string().contains("bórralo desde el panel"));
    assert_eq!(emu.lock().unwrap().stops, 0);
    assert_eq!(link.sent().len(), 4);
}

#[test]
fn cancel_between_packets_stops_the_machine() {
    let (mut s, _l, emu) = mem_session(
        EmuConfig::default(),
        SessionConfig {
            max_data_bytes: 150,
            require_idle: false,
            ..fast_cfg()
        },
    );
    let cancel = AtomicBool::new(false);
    let mut n = 0;
    let e = s
        .send_job(&square_job(), &SendMode::Run, &cancel, &mut |_| {
            n += 1;
            if n == 2 {
                cancel.store(true, Ordering::SeqCst);
            }
        })
        .unwrap_err();
    assert!(matches!(e.cause, SessionError::Cancelled));
    assert_eq!(e.stop, StopOutcome::Acked);
    assert_eq!(emu.lock().unwrap().stops, 1);
}

#[test]
fn unsafe_or_invalid_jobs_are_refused_before_sending_anything() {
    let (mut s, link, _e) = mem_session(EmuConfig::default(), fast_cfg());
    // Sin D7.
    let mut job = square_job();
    job.pop();
    let e = s
        .send_job(
            &job,
            &SendMode::Run,
            &AtomicBool::new(false),
            &mut no_progress(),
        )
        .unwrap_err();
    assert!(matches!(e.cause, SessionError::Protocol(_)));
    // Fuera de la cama del perfil.
    let mut cfg = fast_cfg();
    cfg.limits.bed_mm = [20.0, 20.0];
    let (mut s2, link2, _e2) = mem_session(EmuConfig::default(), cfg);
    let e = s2
        .send_job(
            &square_job(),
            &SendMode::Run,
            &AtomicBool::new(false),
            &mut no_progress(),
        )
        .unwrap_err();
    assert!(matches!(e.cause, SessionError::Safety(_)));
    // Con escritura de memoria.
    let mut cmds = cmd::decode_stream(&square_job()).unwrap();
    let n = cmds.len();
    cmds.insert(
        n - 1,
        Command::MemWrite {
            addr: 0x320,
            values: vec![1, 1],
        },
    );
    let bytes = cmd::encode_stream(&cmds).unwrap();
    let e = s
        .send_job(
            &bytes,
            &SendMode::Run,
            &AtomicBool::new(false),
            &mut no_progress(),
        )
        .unwrap_err();
    assert!(matches!(
        e.cause,
        SessionError::Safety(SafetyError::ForbiddenCommand { .. })
    ));
    assert_eq!(link.sent().len(), 0);
    assert_eq!(link2.sent().len(), 0);
    // Nombre de documento inválido.
    let e = s
        .send_job(
            &square_job(),
            &SendMode::Store { name: "ñ".into() },
            &AtomicBool::new(false),
            &mut no_progress(),
        )
        .unwrap_err();
    assert!(matches!(e.cause, SessionError::Protocol(_)));
}
