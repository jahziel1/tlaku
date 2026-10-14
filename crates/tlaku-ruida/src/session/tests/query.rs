//! Pruebas contra el emulador UDP y de lecturas (información del controlador, documentos).

use super::*;

/// Sesión completa contra el emulador UDP en localhost con puertos efímeros.
#[test]
fn full_session_over_udp_probe_send_status_pause_resume_stop() {
    let emu = UdpEmulator::spawn(EmuConfig::default()).unwrap();
    let link = UdpLink::to_controller_ports(emu.addr().ip(), emu.addr().port(), 0).unwrap();
    let mut s = RuidaSession::new(
        link,
        SessionConfig {
            max_data_bytes: 400,
            ..fast_cfg()
        },
    );
    let info = s.probe().unwrap();
    assert_eq!(info.model, Some("RDC6442S"));
    assert_eq!(info.bed_mm, Some([600.0, 400.0]));
    assert!(s.status().unwrap().is_idle());

    let job = square_job();
    let mut last = None;
    let r = s
        .send_job(&job, &SendMode::Run, &AtomicBool::new(false), &mut |p| {
            last = Some(p)
        })
        .unwrap();
    assert!(r.packets >= 2);
    assert_eq!(last.unwrap().packet, r.packets);
    {
        let e = emu.emulator();
        assert!(e.running && e.run_complete());
        assert_eq!(e.rejected, 0);
        // Lo que llegó es el trabajo íntegro (más las lecturas de antes).
        let tail = &e.stream()[e.stream().len() - job.len()..];
        assert_eq!(tail, &job[..]);
        let back = e.to_toolpath().unwrap();
        assert_eq!(back.blocks.len(), 1);
        assert_eq!(back.blocks[0].moves.len(), 5);
    }
    let st = s.status().unwrap();
    assert!(st.running && !st.paused);
    s.pause().unwrap();
    assert!(s.status().unwrap().paused);
    s.resume().unwrap();
    assert!(!s.status().unwrap().paused);
    s.stop().unwrap();
    assert!(s.status().unwrap().is_idle());
    let e = emu.emulator();
    assert_eq!((e.pauses, e.resumes, e.stops), (1, 1, 1));
}

#[test]
fn controller_info_reads_settings_counters_and_only_reads() {
    let (mut s, _link, emu) = mem_session(EmuConfig::default(), fast_cfg());
    // Un documento guardado para que la cuenta no sea 0.
    s.send_job(
        &square_job(),
        &SendMode::Store { name: "UNO".into() },
        &AtomicBool::new(false),
        &mut no_progress(),
    )
    .unwrap();
    let before = emu.lock().unwrap().received;
    let info = s.controller_info().unwrap();
    assert_eq!(info.probe.model, Some("RDC6442S"));
    assert_eq!(info.probe.bed_mm, Some([600.0, 400.0]));
    assert_eq!(info.rapid_mm_s, Some(200.0));
    assert_eq!(info.home_mm_s, Some(20.0));
    assert_eq!(info.axis_max_mm_s, [Some(400.0), Some(400.0)]);
    assert_eq!(info.axis_accel_mm_s2, [Some(3000.0), Some(3000.0)]);
    assert_eq!(
        (info.total_on_s, info.total_work_s, info.total_jobs),
        (Some(7200), Some(3600), Some(42))
    );
    assert_eq!(info.travel_m, [Some(120), Some(95)]);
    assert_eq!(info.documents, Some(1));
    let e = emu.lock().unwrap();
    assert!(e.received > before);
    // Todo lo que se envió desde antes de pedir la información son lecturas.
    let cmds = e.commands().unwrap();
    assert!(
        cmds.iter()
            .rev()
            .take(e.received - before)
            .all(|c| matches!(c, Command::MemRead { .. }))
    );
}

#[test]
fn a_zero_setting_is_reported_as_unknown() {
    let (mut s, _link, emu) = mem_session(EmuConfig::default(), fast_cfg());
    emu.lock().unwrap().mem.insert(addr::G0_SPEED, 0);
    let info = s.controller_info().unwrap();
    assert_eq!(info.rapid_mm_s, None);
    assert_eq!(info.home_mm_s, Some(20.0));
}

#[test]
fn list_documents_reads_the_names_of_the_stored_documents() {
    let (mut s, _link, emu) = mem_session(EmuConfig::default(), fast_cfg());
    assert!(s.list_documents().unwrap().is_empty());
    for name in ["PRIMERO", "SEGUNDO"] {
        s.send_job(
            &square_job(),
            &SendMode::Store { name: name.into() },
            &AtomicBool::new(false),
            &mut no_progress(),
        )
        .unwrap();
    }
    let files = s.list_documents().unwrap();
    assert_eq!(
        files,
        vec![
            ControllerFile {
                index: 1,
                name: Some("PRIMERO".into())
            },
            ControllerFile {
                index: 2,
                name: Some("SEGUNDO".into())
            },
        ]
    );
    // Listar no altera nada: no hay trabajo ni se borró nada.
    let e = emu.lock().unwrap();
    assert_eq!(e.stored.len(), 2);
    assert!(!e.running);
}

#[test]
fn listing_over_udp_works_and_a_silent_machine_gives_nameless_entries() {
    let emu = UdpEmulator::spawn(EmuConfig::default()).unwrap();
    let link = UdpLink::to_controller_ports(emu.addr().ip(), emu.addr().port(), 0).unwrap();
    let mut s = RuidaSession::new(link, fast_cfg());
    s.send_job(
        &square_job(),
        &SendMode::Store { name: "UDP".into() },
        &AtomicBool::new(false),
        &mut no_progress(),
    )
    .unwrap();
    assert_eq!(s.list_documents().unwrap()[0].name.as_deref(), Some("UDP"));
    // Una máquina que deja de contestar a los nombres: entradas sin nombre, sin esperar N veces.
    let silent = EmuConfig {
        faults: EmuFaults {
            // 0 y 1: dos lecturas de la cuenta que no hace falta repetir; todo después calla.
            silent_from: Some(1),
            ..Default::default()
        },
        ..Default::default()
    };
    let (mut s2, _l, emu2) = mem_session(silent, fast_cfg());
    emu2.lock().unwrap().stored.push(crate::emu::StoredDoc {
        name: "X".into(),
        stream: vec![],
    });
    // La lectura de la cuenta (datagrama 0) llega; los nombres no.
    let files = s2.list_documents().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, None);
}

#[test]
fn udp_session_finds_the_magic_and_survives_a_lost_ack_in_a_read() {
    let emu = UdpEmulator::spawn(EmuConfig {
        packets: PacketConfig {
            magic: Magic(0x33),
            ..Default::default()
        },
        // La lectura 0 (primer sondeo) se pierde del todo.
        faults: EmuFaults {
            lose_packet: Some(0),
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    let link = UdpLink::to_controller_ports(emu.addr().ip(), emu.addr().port(), 0).unwrap();
    let mut s = RuidaSession::new(link, fast_cfg());
    let info = s.probe().unwrap();
    assert_eq!(info.magic, Magic(0x33));
    assert!(info.magic_changed);
    assert!(s.status().unwrap().is_idle());
}
