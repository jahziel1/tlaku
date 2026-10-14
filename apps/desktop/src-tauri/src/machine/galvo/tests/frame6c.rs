//! Ola 6C: formas de encuadre, ajuste en vivo, enclavamiento por entrada, desfase del puntero,
//! overscan y verificación sin láser. Todo contra la tarjeta simulada.

use super::*;
use tlaku_galvo::calib::FieldMap;

fn units(dev: &DeviceProfile, x: f64, y: f64) -> (u16, u16) {
    FieldMap::new(dev.galvo.field_mm, &dev.galvo.correction)
        .unwrap()
        .to_units((x, y))
        .unwrap()
}

fn jumps(rig: &Rig) -> usize {
    lock(&rig.emu)
        .trace
        .iter()
        .filter(|s| s.kind == tlaku_galvo::emu::SegKind::Jump)
        .count()
}

fn wait_idle(rig: &Rig) {
    wait_until("el encuadre termina", || !rig.handle.state().working);
}

#[test]
fn a_frame_with_a_loop_count_ends_by_itself_and_never_fires() {
    let rig = rig();
    rig.handle
        .frame(
            &square_doc(),
            &rig.device,
            &JobOptions::default(),
            FrameKind::Rect,
            Some(2),
            None,
        )
        .unwrap();
    wait_idle(&rig);
    assert_eq!(rig.sink.loops(), 2);
    assert!(!rig.sink.job_states().contains(&JobState::Error));
    let e = lock(&rig.emu);
    assert!(e.emitted().is_empty() && e.laser_safe() && !e.red_on());
    assert!(!e.armed_history.contains(&true));
    drop(e);
    assert!(!rig.handle.state().framing);
    // Cero vueltas no tiene sentido.
    assert!(
        rig.handle
            .frame(
                &square_doc(),
                &rig.device,
                &JobOptions::default(),
                FrameKind::Rect,
                Some(0),
                None
            )
            .unwrap_err()
            .contains("al menos 1")
    );
    rig.conn.close();
}

#[test]
fn hull_and_loose_objects_are_framed_with_their_own_shapes() {
    let rig = rig();
    let opts = JobOptions::default();
    // Dos cuadrados sueltos: 2 rectángulos de 5 puntos cada uno por vuelta.
    let two = doc_of(vec![
        rect("a", 20.0, 20.0, 30.0, 30.0),
        rect("b", 70.0, 70.0, 80.0, 80.0),
    ]);
    rig.handle
        .frame(
            &two,
            &rig.device,
            &opts,
            FrameKind::Objects,
            Some(1),
            Some(1.0),
        )
        .unwrap();
    wait_idle(&rig);
    assert_eq!(jumps(&rig), 10);
    // El casco de los mismos dos cuadrados es un polígono de 6 vértices más el cierre.
    let before = jumps(&rig);
    rig.handle
        .frame(&two, &rig.device, &opts, FrameKind::Hull, Some(1), None)
        .unwrap();
    wait_idle(&rig);
    assert_eq!(jumps(&rig) - before, 7);
    // Con una holgura enorme los dos objetos se funden en uno.
    let before = jumps(&rig);
    rig.handle
        .frame(
            &two,
            &rig.device,
            &opts,
            FrameKind::Objects,
            Some(1),
            Some(100.0),
        )
        .unwrap();
    wait_idle(&rig);
    assert_eq!(jumps(&rig) - before, 5);
    // Una holgura inválida se rechaza antes de tocar la tarjeta.
    let sent = lock(&rig.emu).immediates.len();
    assert!(
        rig.handle
            .frame(
                &two,
                &rig.device,
                &opts,
                FrameKind::Objects,
                None,
                Some(-1.0)
            )
            .is_err()
    );
    assert_eq!(lock(&rig.emu).immediates.len(), sent);
    assert!(lock(&rig.emu).emitted().is_empty());
    rig.conn.close();
}

#[test]
fn the_frame_can_be_moved_and_scaled_live_and_a_bad_adjust_is_reported() {
    let rig = rig_with(device(), Faults::default(), Duration::from_millis(1)).unwrap();
    // Sin encuadre en curso no hay nada que ajustar.
    assert!(
        rig.handle
            .adjust_frame(1.0, 0.0, 1.0)
            .unwrap_err()
            .contains("No hay un encuadre")
    );
    rig.handle
        .frame_job(&square_doc(), &rig.device, &JobOptions::default(), false)
        .unwrap();
    rig.sink.wait("una vuelta", |s| s.loops() >= 1);
    assert!(rig.handle.state().framing);
    assert!(rig.handle.adjust_frame(f64::NAN, 0.0, 1.0).is_err());
    assert!(rig.handle.adjust_frame(0.0, 0.0, 0.0).is_err());
    rig.handle.adjust_frame(10.0, 0.0, 1.0).unwrap();
    rig.sink.wait("el ajuste aplicado", |s| {
        s.galvo()
            .iter()
            .any(|g| matches!(g, GalvoEvent::Light { adjust_gen, .. } if *adjust_gen >= 1))
    });
    // El cuadrado va de (45, 45) a (65, 65); centrado en la cama (55, 55): de -10 a 10. Movido 10
    // en X: de 0 a 20.
    let right = units(&rig.device, 20.0, 10.0);
    wait_until("un salto al borde movido", || {
        lock(&rig.emu).trace.iter().any(|s| s.to == right)
    });
    // Escala 20 sacaría el encuadre del campo: se descarta y se avisa.
    rig.handle.adjust_frame(0.0, 0.0, 20.0).unwrap();
    rig.sink.wait("el aviso de descartado", |s| {
        s.galvo().iter().any(|g| {
            matches!(
                g,
                GalvoEvent::Light {
                    adjust_rejected: true,
                    ..
                }
            )
        })
    });
    rig.handle.cancel().unwrap();
    wait_idle(&rig);
    let e = lock(&rig.emu);
    assert!(e.violations.is_empty(), "{:?}", e.violations);
    assert!(e.emitted().is_empty() && e.laser_safe() && !e.red_on());
    drop(e);
    // Terminado el encuadre ya no se puede ajustar.
    assert!(rig.handle.adjust_frame(1.0, 0.0, 1.0).is_err());
    rig.conn.close();
}

#[test]
fn the_profile_interlock_is_enforced_and_shown_in_the_state() {
    let mut d = device();
    d.galvo.require_input = Some(tlaku_doc::InputRequirement {
        mask: 0x0008,
        value: 0x0008,
    });
    let rig = rig_with(d, Faults::default(), Duration::ZERO).unwrap();
    let i = rig.handle.state().interlock.expect("hay enclavamiento");
    assert_eq!((i.mask, i.value, i.met), (8, 8, false));
    // Abierto: ni se marca ni se encuadra, y la tarjeta no recibe ninguna lista.
    let err = rig.start(&square_doc()).unwrap_err();
    assert!(err.contains("enclavamiento"), "{err}");
    let err = rig
        .handle
        .frame_job(&square_doc(), &rig.device, &JobOptions::default(), false)
        .unwrap_err();
    assert!(err.contains("enclavamiento"), "{err}");
    {
        let e = lock(&rig.emu);
        assert_eq!(e.batches_received, 0);
        assert!(!e.armed_history.contains(&true));
    }
    // Cerrado: el estado lo refleja tras el siguiente sondeo y el trabajo corre.
    lock(&rig.emu).cfg.inputs = 0x0008;
    wait_until("el enclavamiento cumplido", || {
        rig.handle.state().interlock.is_some_and(|i| i.met)
    });
    rig.start(&square_doc()).unwrap();
    wait_until("el trabajo termina", || {
        rig.sink.job_states().contains(&JobState::Finished)
    });
    assert!(lock(&rig.emu).laser_safe());
    // Sin enclavamiento en el perfil el estado no trae ninguno.
    let free = rig_with(device(), Faults::default(), Duration::ZERO).unwrap();
    assert!(free.handle.state().interlock.is_none());
    free.conn.close();
    rig.conn.close();
}

#[test]
fn the_pointer_offset_moves_the_red_pointer_and_needs_a_reconnection_to_change() {
    let mut d = device();
    d.pointer_offset = [2.0, -1.0];
    let rig = rig_with(d, Faults::default(), Duration::from_millis(1)).unwrap();
    rig.handle
        .frame(
            &square_doc(),
            &rig.device,
            &JobOptions::default(),
            FrameKind::Rect,
            Some(1),
            None,
        )
        .unwrap();
    wait_idle(&rig);
    // El cuadrado va de (45, 45) a (65, 65): en el campo, de -10 a 10; el primer punto es la
    // esquina inferior izquierda menos el desfase.
    assert_eq!(lock(&rig.emu).trace[0].to, units(&rig.device, -12.0, -9.0));
    assert!(lock(&rig.emu).emitted().is_empty());
    // Cambiar el desfase pide reconectar: la sesión conserva el de la conexión.
    let mut other = rig.device.clone();
    other.pointer_offset = [0.0, 0.0];
    let err = rig
        .handle
        .frame(
            &square_doc(),
            &other,
            &JobOptions::default(),
            FrameKind::Rect,
            Some(1),
            None,
        )
        .unwrap_err();
    assert!(err.contains("desfase del puntero"), "{err}");
    assert_eq!(rig.handle.state().pointer_offset, [2.0, -1.0]);
    rig.conn.close();
}

#[test]
fn profile_overscan_runs_the_fill_lead_ins_with_the_laser_off() {
    let mut d = device();
    d.galvo.overscan_mm = 2.0;
    let rig = rig_with(d, Faults::default(), Duration::ZERO).unwrap();
    let mut doc = doc_of(vec![rect("a", 40.0, 40.0, 70.0, 70.0)]);
    doc.layers[0].mode = LayerMode::Fill;
    doc.layers[0].interval = 1.0;
    rig.start(&doc).unwrap();
    wait_until("el trabajo termina", || {
        rig.sink.job_states().contains(&JobState::Finished)
    });
    let e = lock(&rig.emu);
    assert!(e.violations.is_empty(), "{:?}", e.violations);
    assert!(!e.emitted().is_empty());
    assert!(e.laser_safe());
    // El haz solo salió en tramos de marca: los de overscan son saltos.
    assert!(
        e.trace
            .iter()
            .all(|s| !s.emitting || s.kind != tlaku_galvo::emu::SegKind::Jump)
    );
    drop(e);
    assert_eq!(rig.handle.state().overscan_mm, 2.0);
    rig.conn.close();
}

#[test]
fn verify_reads_the_board_without_lighting_anything() {
    let d = device();
    let r = verify(&d, "virtual:galvo", None).unwrap();
    assert!(r.all_ok(), "{:?}", r.checks);
    assert!(r.virtual_machine);
    assert_eq!((r.vendor_id, r.product_id), (0x9588, 0x9899));
    let ids: Vec<&str> = r.checks.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["board", "idle", "frame"]);
    // Con un enclavamiento que la tarjeta simulada no cumple (entradas en 0), avisa.
    let mut d = device();
    d.galvo.require_input = Some(tlaku_doc::InputRequirement {
        mask: 0x0001,
        value: 0x0001,
    });
    let r = verify(&d, "virtual:galvo", None).unwrap();
    assert!(!r.all_ok());
    let i = r.checks.iter().find(|c| c.id == "interlock").unwrap();
    assert!(!i.ok);
    assert_eq!(r.interlock.map(|i| i.met), Some(false));
}

#[test]
fn verify_needs_the_safety_acknowledgement_for_a_real_board_and_refuses_the_rest() {
    let d = device();
    // Una tarjeta USB sin reconocimiento, o con uno incompleto, falla antes de abrir nada.
    assert_eq!(verify(&d, "usb:galvo:0", None).unwrap_err(), ACK_REQUIRED);
    let partial = SafetyAck {
        eyewear: true,
        ..SafetyAck::default()
    };
    assert_eq!(
        verify(&d, "usb:galvo:0", Some(&partial)).unwrap_err(),
        ACK_REQUIRED
    );
    // Puertos que no son del galvo.
    assert!(
        verify(&d, "/dev/ttyUSB0", None)
            .unwrap_err()
            .contains("no es un puerto galvo")
    );
    // Un perfil que no es galvo.
    let grbl = DeviceProfile::default();
    assert!(verify(&grbl, "virtual:galvo", None).is_err());
    // Un origen distinto de «abajo a la izquierda» se rechaza, como al conectar.
    let mut bad = device();
    bad.origin = Origin::TopRight;
    assert!(verify(&bad, "virtual:galvo", None).is_err());
}

#[test]
fn the_new_state_fields_reach_the_interface() {
    let rig = rig();
    let st = serde_json::to_value(rig.handle.state()).unwrap();
    for key in ["framing", "interlock", "pointer_offset", "overscan_mm"] {
        assert!(st.get(key).is_some(), "falta {key} en GalvoState");
    }
    assert_eq!(st["framing"], false);
    assert!(st["interlock"].is_null());
    let ev = serde_json::to_value(GalvoEvent::Light {
        loops: 2,
        adjust_gen: 1,
        adjust_rejected: true,
    })
    .unwrap();
    assert_eq!(
        (
            ev["kind"].as_str(),
            ev["adjust_gen"].as_u64(),
            ev["adjust_rejected"].as_bool()
        ),
        (Some("light"), Some(1), Some(true))
    );
    rig.conn.close();
}

#[test]
fn verify_only_reads_and_rejects_unknown_boards_before_sending_anything() {
    let d = device();
    let cfg = || session_config(&d).unwrap();
    // La tarjeta conocida: solo se hacen lecturas (serie, versión, entradas), ninguna lista.
    let (t, emu) = emulator(EmuConfig::default());
    let r = verify_with(Box::new(t), cfg(), &d, true).unwrap();
    assert!(r.all_ok());
    {
        let e = lock(&emu);
        assert_eq!(e.batches_received, 0);
        assert_eq!(e.writes, e.immediates.len(), "solo comandos inmediatos");
        assert!(!e.armed_history.contains(&true) && !e.red_on() && e.laser_safe());
        use tlaku_galvo::codes::imm;
        assert!(
            e.immediates
                .iter()
                .all(|r| [imm::GET_SERIAL_NO, imm::GET_VERSION, imm::READ_PORT].contains(&r.code)),
            "{:?}",
            e.immediates
        );
    }
    // VID:PID desconocidos (clones, tarjeta sin firmware, otro fabricante): no se envía nada.
    for (v, p) in [
        (0x9588u16, 0x9980u16),
        (0x9588, 0x9990),
        (0x9588, 0x9880),
        (0x1A86, 0x5512),
        (0, 0),
    ] {
        let (t, emu) = emulator(EmuConfig {
            vendor_id: v,
            product_id: p,
            ..EmuConfig::default()
        });
        let err = verify_with(Box::new(t), cfg(), &d, true).unwrap_err();
        assert!(err.contains("9588:9899"), "{err}");
        assert_eq!(lock(&emu).writes, 0, "{v:04X}:{p:04X}");
    }
}
