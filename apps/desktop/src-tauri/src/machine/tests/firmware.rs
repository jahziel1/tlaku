//! Firmware, banners reales y reportes raros; preflight de modo láser y escala de potencia.

use super::*;

#[test]
fn firmware_name_and_family_cover_grbl_fluidnc_grblhal_and_the_falcon2() {
    let id = |lines: &[&str]| identify(&lines.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    assert_eq!(
        id(&["[VER:1.1h.20190825:]", "[OPT:V,15,128]"]),
        ("Grbl 1.1h".into(), Firmware::Grbl)
    );
    // Falcon2 Pro: líneas propias de Creality alrededor de `[VER:]`.
    assert_eq!(
        id(&[
            "[VER:1.1f.20220810:]",
            "[OPT:VHL,127,65536]",
            "[software versions: CV50-Pro-MASTER-Release V1.0.0 20231129-1024]",
            "inquire_info->power:22W ,CV50-LASER-22W-Release V3.0.5 20231129-1001",
        ]),
        ("Grbl 1.1f".into(), Firmware::Grbl)
    );
    assert_eq!(
        id(&["[VER:3.9 FluidNC v3.9.7:]", "[OPT:MPHS]"]),
        ("FluidNC v3.9.7".into(), Firmware::FluidNc)
    );
    assert_eq!(
        id(&[
            "[VER:1.1f.20240101:]",
            "[OPT:VNM,35,1024,0]",
            "[NEWOPT:ENUMS,RT+]"
        ]),
        ("grblHAL 1.1f".into(), Firmware::GrblHal)
    );
    // `$I` que solo devuelve `ok`: nombre genérico.
    assert_eq!(id(&[]), ("GRBL".into(), Firmware::Unknown));
    assert_eq!(id(&["[OPT:V,15,128]"]).0, "GRBL");
}

#[test]
fn connecting_through_the_whole_falcon2_banner_and_an_info_reply_of_only_ok() {
    // Banner real de la Falcon2 Pro (falcon2.md, sección 3), llegado antes de la respuesta de `$I`.
    let banner = "usb connect succeed\r\n\
        [OK]system start-up CV50-Pro-MASTER-Release V1.0.1 20240325-1042\r\n\
        [OK] ILmp_app_init OK\r\n[OK] gas_init OK\r\n[MSG:Using cpu_map...esp32-s3]\r\n\
        Grbl 1.1f ['$' for help]\r\n[OK] ILmp Link connect ok\r\n";
    let info = "[VER:1.1f.20220810:]\r\n[OPT:VHL,127,65536]\r\n\
        [software versions: CV50-Pro-MASTER-Release V1.0.0 20231129-1024]\r\n\
        inquire_info->power:22W ,CV50-LASER-22W-Release V3.0.5 20231129-1001\r\nok\r\n";
    for (reply, firmware) in [(info, "Grbl 1.1f"), ("ok\r\n", "GRBL")] {
        let (host, mut dev) = memory_pair();
        let sink = Arc::new(Collect::default());
        dev.write_all(banner.as_bytes()).unwrap();
        dev.write_all(reply.as_bytes()).unwrap();
        let (conn, result) = Connection::start(Box::new(host), "p", sink.clone(), fast()).unwrap();
        assert_eq!(result.firmware, firmware);
        // Ninguna de las líneas de Creality se tomó por el final del comando.
        assert!(
            sink.logs()
                .iter()
                .any(|(_, t)| t.contains("ILmp Link connect ok") || t == "ok")
        );
        conn.close();
    }
}

#[test]
fn a_status_report_with_nan_never_becomes_a_position() {
    let (host, mut dev) = memory_pair();
    let sink = Arc::new(Collect::default());
    dev.write_all(b"[VER:1.1f.20220810:]\r\nok\r\n").unwrap();
    let (_conn, _) = Connection::start(Box::new(host), "p", sink.clone(), fast()).unwrap();
    // WCO con nan (G54 dañado en la Falcon2) y luego una MPos con nan.
    dev.write_all(b"<Run|MPos:5.000,6.000,0.000|FS:100,5|WCO:-nan,1.579,1.619>\r\n")
        .unwrap();
    dev.write_all(b"<Run|MPos:nan,6.000,0.000|FS:100,5>\r\n")
        .unwrap();
    sink.wait("estado", |s| {
        s.statuses().iter().any(|x| x.state == MachineState::Run)
    });
    for s in sink.statuses() {
        assert!(
            s.machine_pos
                .iter()
                .chain(&s.work_pos)
                .all(|v| v.is_finite()),
            "{s:?}"
        );
    }
    let s = sink
        .statuses()
        .into_iter()
        .find(|x| x.state == MachineState::Run)
        .unwrap();
    // Sin WCO válido, la posición de trabajo es la de máquina.
    assert_eq!((s.machine_pos, s.work_pos), ([5.0, 6.0], [5.0, 6.0]));
}

#[test]
fn an_unknown_grblhal_state_is_shown_as_busy_and_blocks_a_new_job() {
    let rig = connect();
    rig.fake.set_state("Tool");
    rig.sink.wait("estado ocupado", |s| {
        s.statuses()
            .last()
            .is_some_and(|x| x.state == MachineState::Other)
    });
    // La interfaz recibe el nombre original del estado para mostrarlo.
    let last = rig.sink.statuses().pop().unwrap();
    assert_eq!(last.other_state.as_deref(), Some("Tool"));
    let e = rig.handle.start_job(job_lines(), 127).unwrap_err();
    assert!(
        e.contains("no reconoce") && e.contains("No se transmitió nada"),
        "{e}"
    );
}

#[test]
fn extended_codes_are_read_with_the_firmware_family() {
    // grblHAL: `[NEWOPT:]` en `$I`; ALARM:10 es un paro de emergencia.
    let (host, mut dev) = memory_pair();
    let sink = Arc::new(Collect::default());
    dev.write_all(b"[VER:1.1f.20240101:]\r\n[NEWOPT:ENUMS,RT+]\r\nok\r\n")
        .unwrap();
    let (_conn, r) = Connection::start(Box::new(host), "p", sink.clone(), fast()).unwrap();
    assert_eq!(r.firmware, "grblHAL 1.1f");
    dev.write_all(b"ALARM:10\r\nerror:50\r\nERROR:06.\r\n")
        .unwrap();
    sink.wait("registro", |s| {
        s.logs().iter().any(|(_, t)| t.starts_with("ERROR:06"))
    });
    let logs = sink.logs();
    let has = |needle: &str| logs.iter().any(|(_, t)| t.contains(needle));
    assert!(has("ALARM:10: parada de emergencia"), "{logs:?}");
    assert!(has("error:50: parada de emergencia activa"), "{logs:?}");
    // `ERROR:06.` de Creality se explica como tal y se normaliza.
    assert!(has("ERROR:06: lente contaminada"), "{logs:?}");

    // GRBL 1.1 identificado: el mismo ALARM:10 es el autoescuadre.
    let rig = connect();
    rig.fake.say("ALARM:10\r\n");
    rig.sink.wait("alarma", |s| {
        s.logs().iter().any(|(_, t)| t.starts_with("ALARM:10"))
    });
    assert!(
        rig.sink
            .logs()
            .iter()
            .any(|(_, t)| t.starts_with("ALARM:10") && t.contains("autoescuadre"))
    );

    // Sin saber el firmware (`$I` solo devuelve `ok`), el texto dice que depende de él.
    let (host, mut dev) = memory_pair();
    let sink = Arc::new(Collect::default());
    dev.write_all(b"ok\r\n").unwrap();
    let (_conn, r) = Connection::start(Box::new(host), "p", sink.clone(), fast()).unwrap();
    assert_eq!(r.firmware, "GRBL");
    dev.write_all(b"ALARM:10\r\n").unwrap();
    sink.wait("alarma", |s| {
        s.logs().iter().any(|(_, t)| t.starts_with("ALARM:10"))
    });
    assert!(
        sink.logs()
            .iter()
            .any(|(_, t)| t.starts_with("ALARM:10") && t.contains("depende del firmware"))
    );
}

#[test]
fn laser_mode_off_blocks_the_job_but_not_the_frame() {
    let rig = connect();
    *rig.fake.settings.lock().unwrap() = "$22=1\r\n$30=1000\r\n$32=0\r\n".into();
    let e = rig.handle.start_job(job_lines(), 127).unwrap_err();
    assert!(e.contains("$32=0") && e.contains("$32=1"), "{e}");
    assert!(
        e.contains("G0") && e.contains("No se transmitió nada"),
        "{e}"
    );
    assert!(e.contains("consola de la pestaña Máquina"), "{e}");
    assert!(rig.sink.jobs().is_empty());
    assert!(
        rig.fake.lines().iter().all(|l| l.starts_with('$')),
        "{:?}",
        rig.fake.lines()
    );
    // El encuadre va con el láser apagado: no necesita el modo láser.
    rig.handle
        .frame(vec!["M5".into(), "G0 X1 Y1".into()], 127)
        .unwrap();
    rig.sink.wait("encuadre", |s| {
        s.logs()
            .iter()
            .any(|(_, t)| tlaku_i18n::split_code(t) == (Some("frame_done"), "; encuadre terminado"))
    });
    // Y la conexión sigue viva para corregirlo desde la consola.
    assert!(!rig.handle.is_closed());
}

#[test]
fn an_s_max_that_differs_from_the_controller_only_warns_in_the_log() {
    let rig = connect();
    // GRBL-LPC: `$30=1`.
    *rig.fake.settings.lock().unwrap() = "$22=0\r\n$30=1.000\r\n$32=1\r\n".into();
    rig.handle
        .start_job_for(job_lines(), 127, Some(1000.0))
        .unwrap();
    rig.sink.wait("trabajo terminado", |s| {
        s.jobs().contains(&JobState::Finished)
    });
    let logs = rig.sink.logs();
    assert!(
        logs.iter().any(|(d, t)| *d == LogDir::In
            && t.starts_with("Aviso: $30=1 ")
            && t.contains("S máx = 1000")),
        "{logs:?}"
    );
}

#[test]
fn nan_in_the_offsets_is_warned_about_before_the_job() {
    let rig = connect();
    *rig.fake.params.lock().unwrap() =
        "[G54:-nan,1.579,1.619]\r\n[G55:0.000,0.000,0.000]\r\n".into();
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.sink.wait("trabajo terminado", |s| {
        s.jobs().contains(&JobState::Finished)
    });
    assert!(
        rig.sink
            .logs()
            .iter()
            .any(|(_, t)| t.contains("(nan) en G54") && t.contains("$RST=#")),
        "{:?}",
        rig.sink.logs()
    );
}
