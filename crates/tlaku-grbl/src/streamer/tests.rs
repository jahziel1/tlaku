use super::*;
use crate::link::{FlowControl, LinkFamily};
use crate::serial::memory_pair;
use tlaku_doc::OverrideKind;

#[test]
fn prepare_strips_comments_and_spaces() {
    assert_eq!(
        prepare_line("G1 X10.5 Y-2 F1000").unwrap(),
        "G1X10.5Y-2F1000"
    );
    assert_eq!(prepare_line("G0 X1 ; mover").unwrap(), "G0X1");
    assert_eq!(prepare_line("G0 (rápido) X1 (otro) Y2").unwrap(), "G0X1Y2");
    assert_eq!(prepare_line("(solo comentario)").unwrap(), "");
    assert_eq!(prepare_line("   \t ").unwrap(), "");
    assert_eq!(prepare_line("; todo ! ? ~").unwrap(), "");
    assert_eq!(prepare_line("G1 X1 (sin cerrar ! ?").unwrap(), "G1X1");
    assert_eq!(prepare_line("$J=G91 X10 F500").unwrap(), "$J=G91X10F500");
}

#[test]
fn prepare_rejects_realtime_lookalikes() {
    assert_eq!(prepare_line("G1 X1 !"), Err('!'));
    assert_eq!(prepare_line("G1 ~"), Err('~'));
    assert_eq!(prepare_line("?"), Err('?'));
    assert_eq!(prepare_line("G1 Xé"), Err('é'));
    assert_eq!(prepare_line("G1\u{18}"), Err('\u{18}'));
}

#[test]
fn prepare_line_with_keeps_single_spaces_for_reprap_firmwares() {
    assert_eq!(
        prepare_line_with("  M3   S0 I ; láser", false, false).unwrap(),
        "M3 S0 I"
    );
    assert_eq!(
        prepare_line_with("G1 (c) X1  Y2", false, false).unwrap(),
        "G1 X1 Y2"
    );
    assert_eq!(prepare_line_with("   ", false, false).unwrap(), "");
    // Marlin admite `?` y `!`; Smoothie y GRBL los rechazan.
    assert_eq!(
        prepare_line_with("M117 Hola!", false, false).unwrap(),
        "M117 Hola!"
    );
    assert_eq!(prepare_line_with("M117 Hola!", false, true), Err('!'));
    // Lo no ASCII se rechaza siempre.
    assert_eq!(prepare_line_with("M117 é", false, false), Err('é'));
    // Con `strip_spaces` es el de GRBL.
    assert_eq!(
        prepare_line_with("G1 X1", true, true).unwrap(),
        prepare_line("G1 X1").unwrap()
    );
}

/// Lee lo que llegue al extremo de la máquina en `ms` milisegundos.
fn read_for(dev: &mut crate::serial::MemoryTransport, ms: u64) -> String {
    let mut out = Vec::new();
    let mut buf = [0u8; 256];
    let end = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < end {
        let n = dev
            .read_timeout(&mut buf, Duration::from_millis(5))
            .unwrap();
        out.extend_from_slice(&buf[..n]);
    }
    String::from_utf8(out).unwrap()
}

#[test]
fn ping_pong_sends_one_line_per_ok_and_closes_with_m400() {
    use tlaku_doc::GcodeDialect;
    let (host, mut dev) = memory_pair();
    let link = LinkProfile::marlin(&GcodeDialect::marlin_inline());
    let cfg = StreamerConfig::for_link(&link);
    assert_eq!(cfg.status_interval, None);
    let job = Streamer::with_link(host, cfg, link)
        .spawn(&["G90", "G21 ; mm", "", "M3 S0 I"])
        .unwrap();
    assert_eq!(read_for(&mut dev, 60), "G90\n");
    // `ok` con texto (ADVANCED_OK) vale; `echo:` y `busy:` no cuentan.
    dev.write_all(b"echo:busy: processing\r\nok N0 P15 B3\r\n")
        .unwrap();
    assert_eq!(read_for(&mut dev, 60), "G21\n");
    dev.write_all(b"ok\r\n").unwrap();
    assert_eq!(read_for(&mut dev, 60), "M3 S0 I\n");
    dev.write_all(b"ok\r\n").unwrap();
    // Todas respondidas: cierra con M400 y espera su ok.
    assert_eq!(read_for(&mut dev, 60), "M400\n");
    assert!(job.events.recv_timeout(Duration::from_millis(1)).is_ok());
    dev.write_all(b"ok\r\n").unwrap();
    let (_, out) = job.join().unwrap();
    assert_eq!(out, Outcome::Completed { errors: 0 });
}

#[test]
fn ping_pong_pause_resume_cancel_use_the_link_actions() {
    use tlaku_doc::GcodeDialect;
    let (host, mut dev) = memory_pair();
    let link = LinkProfile::marlin(&GcodeDialect::marlin_inline());
    let mut cfg = StreamerConfig::for_link(&link);
    cfg.hold_timeout = Duration::from_millis(500);
    let job = Streamer::with_link(host, cfg, link)
        .spawn(&["G90", "G21", "G0 X1"])
        .unwrap();
    assert_eq!(read_for(&mut dev, 60), "G90\n");
    job.control.pause();
    dev.write_all(b"ok\r\n").unwrap();
    // En pausa no se envía la siguiente, y no se escribe `!`.
    assert_eq!(read_for(&mut dev, 80), "");
    job.control.resume();
    assert_eq!(read_for(&mut dev, 60), "G21\n");
    job.control.cancel();
    dev.write_all(b"ok\r\nok\r\nok\r\n").unwrap();
    let (streamer, out) = job.join().unwrap();
    assert_eq!(out, Outcome::Cancelled);
    assert_eq!(read_for(&mut dev, 20), "M410\nM5\n");
    let stop = streamer.last_stop().unwrap();
    assert!(stop.position_maybe_lost);
}

#[test]
fn grbl_link_from_new_matches_the_config_buffer() {
    let s = streamer(200);
    assert_eq!(s.link().flow, FlowControl::CharCounting { rx_buffer: 200 });
    assert_eq!(s.link().family, LinkFamily::Grbl);
}

fn streamer(rx_buffer: usize) -> Streamer<crate::serial::MemoryTransport> {
    let (host, _dev) = memory_pair();
    let config = StreamerConfig {
        rx_buffer,
        ..Default::default()
    };
    Streamer::new(host, config)
}

#[test]
fn line_must_fit_with_newline() {
    let s = streamer(8);
    assert!(s.prepare(&["G1X1Y22"]).is_ok()); // 7 + '\n' = 8
    assert_eq!(
        s.prepare(&["G0", "G1X1Y223"]).err(),
        Some(StreamError::LineTooLong {
            source_line: 2,
            len: 9,
            max: 8
        })
    );
}

#[test]
fn empty_lines_are_skipped_but_keep_source_numbers() {
    let s = streamer(128);
    let job = s.prepare(&["G0 X0", "", "; nada", "G1 X1"]).unwrap();
    let got: Vec<_> = job
        .iter()
        .map(|p| (p.source_line, p.text.as_str()))
        .collect();
    assert_eq!(got, [(1, "G0X0"), (4, "G1X1")]);
}

#[test]
fn empty_job_completes_without_io() {
    let mut s = streamer(128);
    let mut events = Vec::new();
    let out = s.run::<&str>(&[], |e| events.push(e)).unwrap();
    assert_eq!(out, Outcome::Completed { errors: 0 });
    assert_eq!(events, [Event::Finished(Outcome::Completed { errors: 0 })]);
}

#[test]
fn realtime_bytes_match_grbl_11() {
    assert_eq!(Realtime::SoftReset.byte(), 0x18);
    assert_eq!(Realtime::JogCancel.byte(), 0x85);
    assert_eq!(Realtime::FeedPlus10.byte(), 0x91);
    assert_eq!(Realtime::SpindleStop.byte(), 0x9E);
}

#[test]
fn status_edge_cases() {
    assert_eq!(StatusReport::parse("ok"), None);
    assert_eq!(StatusReport::parse("<>"), None);
    assert_eq!(StatusReport::parse("<|MPos:0,0,0>"), None);
    assert_eq!(StatusReport::parse("<Idle|MPos:0,0,0"), None);
    // Basura con forma de línea `<...>` pero sin un nombre de estado.
    assert_eq!(StatusReport::parse("<\u{fffd}\u{fffd}|MPos:0,0,0>"), None);
    assert_eq!(StatusReport::parse("<12345|MPos:0,0,0>"), None);
    assert_eq!(StatusReport::parse("<Nope>"), None);
    let r = StatusReport::parse("<Run|MPos:abc|Zz:1|Bf:1|FS:5,6>").unwrap();
    assert_eq!((r.mpos, r.buffer), (None, None));
    assert_eq!((r.feed, r.spindle), (Some(5.0), Some(6.0)));
    let r = StatusReport::parse(" <Idle|MPos:1.5,2.5>\r").unwrap();
    assert_eq!(r.mpos.unwrap().z, 0.0);
    let r = StatusReport::parse("<Idle|MPos:1,2,3,4,5>").unwrap();
    assert_eq!(r.mpos.unwrap().extra, [4.0, 5.0]);
    assert_eq!(
        StatusReport::parse("<Hold|MPos:0,0,0>").unwrap().state,
        MachineState::Hold(None)
    );
}

#[test]
fn override_plan_goes_from_the_known_value_to_the_wanted_one() {
    use Realtime::*;
    let feed = OverrideKind::Feed;
    assert_eq!(override_plan(feed, Some(100), 100), [FeedReset]);
    // Ya en 120: se reinicia y se vuelve a ajustar, por si el valor visto era atrasado.
    assert_eq!(
        override_plan(feed, Some(120), 120),
        [FeedReset, FeedPlus10, FeedPlus10]
    );
    assert_eq!(override_plan(feed, Some(100), 130), [FeedPlus10; 3]);
    assert_eq!(
        override_plan(feed, Some(100), 87),
        [FeedMinus10, FeedMinus1, FeedMinus1, FeedMinus1]
    );
    assert_eq!(override_plan(feed, Some(150), 100), [FeedReset]);
    assert_eq!(override_plan(feed, Some(40), 45), [FeedPlus1; 5]);
}

#[test]
fn override_plan_without_a_known_value_starts_from_a_reset() {
    use Realtime::*;
    assert_eq!(
        override_plan(OverrideKind::Spindle, None, 100),
        [SpindleReset]
    );
    assert_eq!(
        override_plan(OverrideKind::Spindle, None, 112),
        [SpindleReset, SpindlePlus10, SpindlePlus1, SpindlePlus1]
    );
}

#[test]
fn override_plan_clamps_to_the_range_of_grbl_and_uses_the_right_bytes() {
    let up = override_plan(OverrideKind::Feed, Some(100), 999);
    assert_eq!(up.len(), 10);
    assert!(up.iter().all(|r| r.byte() == 0x91));
    let down = override_plan(OverrideKind::Spindle, Some(100), 0);
    assert_eq!(down.len(), 9);
    assert!(down.iter().all(|r| r.byte() == 0x9B));
    // Aplicar el plan al valor de partida llega exactamente al pedido (el emulador usa la
    // misma regla de GRBL).
    for (from, to) in [(100, 37), (13, 200), (55, 56), (200, 10), (100, 100)] {
        let mut v = from as i64;
        for r in override_plan(OverrideKind::Feed, Some(from), to) {
            v = match r {
                Realtime::FeedReset => 100,
                Realtime::FeedPlus10 => v + 10,
                Realtime::FeedMinus10 => v - 10,
                Realtime::FeedPlus1 => v + 1,
                Realtime::FeedMinus1 => v - 1,
                other => panic!("byte inesperado: {other:?}"),
            };
        }
        assert_eq!(v, i64::from(to), "{from} -> {to}");
    }
}

#[test]
fn legacy_comma_report_parses_state_and_positions() {
    let r =
        StatusReport::parse_legacy("<Idle,MPos:1.5000,2.0000,0.0000,WPos:1.0000,2.0000,0.0000>")
            .unwrap();
    assert_eq!(r.state, MachineState::Idle);
    assert_eq!(r.mpos.unwrap().x, 1.5);
    assert_eq!(r.wpos.unwrap().y, 2.0);
    let r = StatusReport::parse_legacy("<Run,MPos:0,0,0>").unwrap();
    assert_eq!(r.state, MachineState::Run);
    assert_eq!(r.wpos, None);
    assert!(StatusReport::parse_legacy("<Nope>").is_none());
    assert!(StatusReport::parse_legacy("ok").is_none());
    // El informe de GRBL 1.1 no es de este formato.
    assert!(StatusReport::parse_legacy("<Idle|MPos:0,0,0>").is_none());
}

#[test]
fn unknown_states_do_not_discard_the_report() {
    // grblHAL: estado `Tool` con campos extra y FS de tres valores.
    let r =
        StatusReport::parse("<Tool|MPos:1.000,2.000,0.000|FS:0,0,0|WCS:G54|TLR:0|H:1>").unwrap();
    assert_eq!(r.state, MachineState::Other);
    assert_eq!(r.other_state.as_deref(), Some("Tool"));
    assert_eq!(r.mpos.unwrap().x, 1.0);
    assert!(!MachineState::Other.is_stationary());
    let r = StatusReport::parse("<Bogus|MPos:0,0,0>").unwrap();
    assert_eq!(r.other_state.as_deref(), Some("Bogus"));
    // Subcódigo ilegible: el estado se conserva sin él.
    assert_eq!(
        StatusReport::parse("<Hold:x|MPos:0,0,0>").unwrap().state,
        MachineState::Hold(None)
    );
    // Los conocidos no llevan nombre.
    assert_eq!(
        StatusReport::parse("<Idle|MPos:0,0,0>")
            .unwrap()
            .other_state,
        None
    );
}

#[test]
fn fs_accepts_two_or_three_values() {
    let r = StatusReport::parse("<Run|MPos:1.000,1.000,0.000|FS:1500,255,240>").unwrap();
    assert_eq!((r.feed, r.spindle), (Some(1500.0), Some(255.0)));
    let r = StatusReport::parse("<Alarm:1|MPos:0.000,0.000,0.000|FS:0,0,0|Pn:X>").unwrap();
    assert_eq!(r.state, MachineState::Alarm);
    assert_eq!(r.pins.as_deref(), Some("X"));
    // Uno o cuatro valores no son un FS válido.
    let r = StatusReport::parse("<Run|FS:1500>").unwrap();
    assert_eq!((r.feed, r.spindle), (None, None));
    let r = StatusReport::parse("<Run|FS:1,2,3,4>").unwrap();
    assert_eq!((r.feed, r.spindle), (None, None));
}

#[test]
fn nan_and_inf_are_unknown_not_positions() {
    let r = StatusReport::parse(
        "<Idle|MPos:0.000,0.000,0.000|Bf:127,65535|FS:0,0|WCO:nan,1.579,1.619>",
    )
    .unwrap();
    assert!(r.mpos.is_some());
    assert_eq!(r.wco, None);
    let r =
        StatusReport::parse("<Idle|MPos:-nan,1,2|WPos:inf,0,0|F:NaN|FS:nan,0|Bf:nan,1>").unwrap();
    assert_eq!(
        (r.mpos, r.wpos, r.feed, r.spindle, r.buffer),
        (None, None, None, None, None)
    );
    // Una línea de `$#` real de la Falcon2.
    assert_eq!(
        parse_parameter_line("[G54:-nan,1.579,1.619]"),
        Some(("G54".to_string(), None))
    );
    assert_eq!(
        parse_parameter_line("[G55:0.000,0.000,0.000]"),
        Some(("G55".to_string(), Some(vec![0.0, 0.0, 0.0])))
    );
    assert_eq!(
        parse_parameter_line("[PRB:1.000,2.000,3.000:1]"),
        Some(("PRB".to_string(), Some(vec![1.0, 2.0, 3.0])))
    );
    assert_eq!(parse_parameter_line("ok"), None);
}

#[test]
fn greeting_detection_never_slices_inside_a_character() {
    // Bytes inválidos llegan como U+FFFD (3 bytes cada uno): `line[..4]` haría panic.
    let lossy = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
    for raw in [
        &b"ab\xff\xfe"[..],
        b"\xff\xfe\xfd\xfc\xfb",
        b"gr\xff",
        b"grb\xe2\x82",
        b"\xc3",
        b"\xff",
        b"gr\xc3\xa9b",
        "ñandú".as_bytes(),
        b"",
    ] {
        let line = lossy(raw);
        assert!(!is_greeting(&line), "{line:?}");
        let _ = StatusReport::parse(&line);
        let _ = parse_creality_error(&line);
        let _ = writes_eeprom(&line);
        let _ = parse_parameter_line(&line);
    }
    assert!(is_greeting("Grbl 1.1h ['$' for help]"));
    assert!(is_greeting("GrblHAL 1.1f"));
    assert!(is_greeting("GRBL"));
    assert!(!is_greeting("grb"));
}

#[test]
fn creality_errors_are_not_grbl_errors() {
    assert_eq!(parse_creality_error("ERROR:05"), Some(5));
    assert_eq!(parse_creality_error("ERROR:06."), Some(6));
    assert_eq!(
        parse_creality_error("ERROR:24. Machine not sealed"),
        Some(24)
    );
    assert_eq!(parse_creality_error("  ERROR: 01 \r"), Some(1));
    // El `error:N` de GRBL (minúsculas) no es una falla de Creality.
    assert_eq!(parse_creality_error("error:5"), None);
    assert_eq!(parse_creality_error("Error:5"), None);
    assert_eq!(parse_creality_error("ERROR:"), None);
    assert_eq!(parse_creality_error("ERROR:0x"), None);
    assert_eq!(parse_creality_error("ERROR:12345"), None);
    assert_eq!(parse_creality_error("ERRORS:05"), None);
    assert!(creality_error_message(5).contains("llama"));
    assert!(creality_error_message(1).contains("inclinación"));
    assert!(creality_error_message(6).contains("lente"));
    assert!(creality_error_message(24).contains("tapa"));
    assert!(creality_error_message(4).contains("aire"));
    assert!(creality_error_message(99).contains("sin descripción"));
}

#[test]
fn eeprom_writing_lines_are_detected() {
    for yes in [
        "$100=80",
        "$N0=G54",
        "$I=Mi láser",
        "$RST=$",
        "G10L2P1X0Y0",
        "G10L20P1X0",
        "G28.1",
        "G30.1",
        "G90G10L2P1X0",
        "g10l2p1x0",
    ] {
        assert!(writes_eeprom(yes), "{yes}");
    }
    for no in [
        "G1X10F1000",
        "G0X1Y1",
        "G28",
        "G30",
        "G100",
        "$J=G91X10F500",
        "$$",
        "$H",
        "$X",
        "M3S500",
        "G1X10G10",
    ] {
        // `G1X10G10` sí tiene un G10 pegado; se comprueba aparte.
        if no == "G1X10G10" {
            assert!(writes_eeprom(no));
        } else {
            assert!(!writes_eeprom(no), "{no}");
        }
    }
    assert!(is_homing_line("$H") && is_homing_line("$HX") && is_homing_line("$h"));
    assert!(!is_homing_line("$X") && !is_homing_line("H"));
}

#[test]
fn firmware_family_is_detected_from_identification_lines() {
    let l = |v: &[&str]| Firmware::detect(v);
    assert_eq!(
        l(&["[VER:1.1h.20190825:]", "[OPT:V,15,128]"]),
        Firmware::Grbl
    );
    assert_eq!(
        l(&["[VER:3.9 FluidNC v3.9.7:]", "[OPT:MPHS]"]),
        Firmware::FluidNc
    );
    assert_eq!(
        l(&[
            "[VER:1.1f.20240101:]",
            "[OPT:VNM,35,1024,0]",
            "[NEWOPT:ENUMS,RT+]"
        ]),
        Firmware::GrblHal
    );
    assert_eq!(l(&["[FIRMWARE:grblHAL]"]), Firmware::GrblHal);
    assert_eq!(l(&[]), Firmware::Unknown);
    assert_eq!(
        Firmware::from_banner("GrblHAL 1.1f ['$' or '$HELP' for help]"),
        Some(Firmware::GrblHal)
    );
    assert_eq!(
        Firmware::from_banner("Grbl 3.4 [FluidNC v3.4.3 (wifi) '$' for help]"),
        Some(Firmware::FluidNc)
    );
    assert_eq!(Firmware::from_banner("Grbl 1.1h ['$' for help]"), None);
}

#[test]
fn error_tables_cover_known_codes() {
    assert_eq!(error_message(20), "comando G-code no soportado o no válido");
    assert_eq!(alarm_message(1), "límite físico activado");
    assert_eq!(alarm_message(99), "alarma desconocida");
    // 1 a 17 y 20 a 38 tienen texto en cualquier firmware.
    for c in (1..=17).chain(20..=38) {
        for fw in [
            Firmware::Unknown,
            Firmware::Grbl,
            Firmware::GrblHal,
            Firmware::FluidNc,
        ] {
            let m = error_message_for(fw, c);
            assert!(
                !m.contains("desconocido") && !m.contains("extendido"),
                "{fw:?} {c}: {m}"
            );
        }
    }
    for c in 1..=9 {
        assert_ne!(alarm_message(c), "alarma desconocida", "{c}");
    }
}

#[test]
fn extended_codes_depend_on_the_firmware_family() {
    // ALARM:10 es autoescuadre en GRBL 1.1 y paro de emergencia en grblHAL.
    assert!(alarm_message_for(Firmware::Grbl, 10).contains("autoescuadre"));
    assert!(alarm_message_for(Firmware::GrblHal, 10).contains("emergencia"));
    assert!(alarm_message(10).contains("depende del firmware"));
    for c in 11..=22 {
        assert!(
            alarm_message_for(Firmware::GrblHal, c).contains("grblHAL"),
            "{c}"
        );
    }
    // Errores extendidos: texto de grblHAL solo si se sabe que lo es.
    assert_eq!(
        error_message_for(Firmware::GrblHal, 50),
        "parada de emergencia activa"
    );
    assert!(error_message_for(Firmware::GrblHal, 46).contains("homing"));
    assert!(error_message_for(Firmware::GrblHal, 63).contains("SD"));
    assert!(error_message_for(Firmware::FluidNc, 63).contains("SD"));
    assert!(error_message_for(Firmware::FluidNc, 39).contains("segundos"));
    assert!(error_message_for(Firmware::GrblHal, 39).contains("fuera de rango"));
    assert!(error_message(50).contains("depende de la familia"));
    assert!(error_message(999).contains("depende de la familia"));
    // En GRBL 1.1 puro no existen: sin texto inventado.
    assert_eq!(error_message_for(Firmware::Grbl, 50), "error desconocido");
    assert_eq!(error_message(0), "error desconocido");
}
