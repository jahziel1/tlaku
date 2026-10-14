//! Pruebas de punta a punta con la tarjeta simulada: compilar, enviar con control de flujo,
//! comprobar el rastro del haz y que, pase lo que pase, la fuente queda apagada.
//!
//! Recordatorio de método: la tarjeta simulada solo prueba que nuestro código es consistente con
//! lo que creemos del protocolo; no sustituye la prueba con hardware (docs/hardware/galvo.md,
//! sección 9).

use tlaku_core::Point;
use tlaku_core::toolpath::{Move, PowerMode, Toolpath, ToolpathBlock};
use tlaku_doc::{GalvoDevice, LaserSource};
use tlaku_galvo::calib::FieldMap;
use tlaku_galvo::codes::imm;
use tlaku_galvo::compile::{LightShape, params_from_device};
use tlaku_galvo::emu::{EmuConfig, EmuHandle, Faults, SegKind, emulator, lock};
use tlaku_galvo::session::{
    GalvoSession, JobControl, LightProgress, Progress, SessionConfig, SessionError, StopOutcome,
};
use tlaku_galvo::transport::Transport;

fn device() -> GalvoDevice {
    GalvoDevice {
        field_mm: 100.0,
        freq_min_khz: 20.0,
        freq_max_khz: 80.0,
        max_power_pct: 60.0,
        ..GalvoDevice::default()
    }
}

fn cfg(dev: GalvoDevice) -> SessionConfig {
    let mut c = SessionConfig::new(dev).unwrap();
    c.poll_interval = std::time::Duration::ZERO;
    c
}

fn session(dev: &GalvoDevice, emu: EmuConfig) -> (GalvoSession<impl Transport>, EmuHandle) {
    let (t, h) = emulator(EmuConfig {
        source: dev.source,
        field_mm: dev.field_mm,
        ..emu
    });
    (GalvoSession::new(t, cfg(dev.clone())).unwrap(), h)
}

fn travel(x: f64, y: f64) -> Move {
    Move::Travel {
        to: Point::new(x, y),
    }
}

fn cut(x: f64, y: f64, p: f64) -> Move {
    Move::Cut {
        to: Point::new(x, y),
        power_pct: p,
    }
}

fn toolpath(dev: &GalvoDevice, moves: Vec<Move>) -> Toolpath {
    Toolpath {
        blocks: vec![ToolpathBlock {
            label: "prueba".into(),
            speed_mm_min: 60_000.0,
            air_assist: false,
            power_mode: PowerMode::Constant,
            moves,
            z_mm: None,
            galvo: Some(params_from_device(dev, 30.0, None)),
        }],
    }
}

/// Dos cuadrados con un viaje en medio.
fn squares(dev: &GalvoDevice) -> Toolpath {
    let sq = |x: f64, y: f64| {
        vec![
            travel(x, y),
            cut(x + 10.0, y, 40.0),
            cut(x + 10.0, y + 10.0, 40.0),
            cut(x, y + 10.0, 40.0),
            cut(x, y, 40.0),
        ]
    };
    let mut m = sq(-30.0, -30.0);
    m.extend(sq(10.0, 10.0));
    toolpath(dev, m)
}

/// Segmentos que debe dibujar el haz según el `Toolpath` (independiente del compilador):
/// cada `Cut` con potencia y longitud no nula, en unidades de la tarjeta.
fn expected_marks(dev: &GalvoDevice, tp: &Toolpath) -> Vec<((u16, u16), (u16, u16))> {
    let map = FieldMap::new(dev.field_mm, &dev.correction).unwrap();
    let u = |p: Point| map.to_units((p.x, p.y)).unwrap();
    let mut pos = (0x8000u16, 0x8000u16);
    let mut out = Vec::new();
    for b in &tp.blocks {
        for m in &b.moves {
            let to = u(m.to());
            if let Move::Cut { power_pct, .. } = m
                && *power_pct > 0.0
                && to != pos
            {
                out.push((pos, to));
            }
            pos = to;
        }
    }
    out
}

fn noop(_: Progress) {}

#[test]
fn a_job_marks_exactly_the_cuts_and_never_fires_on_jumps() {
    let dev = device();
    let tp = squares(&dev);
    let (mut s, h) = session(&dev, EmuConfig::default());
    let info = s.probe().unwrap();
    assert!(info.transport.emulated);
    assert!(!info.status.busy && info.status.ready);

    let mut last = None;
    let rep = s
        .mark(&tp, false, &JobControl::new(), &mut |p| last = Some(p))
        .unwrap();
    assert!(rep.armed);
    assert_eq!(rep.stats.marks, 8);
    assert_eq!(last.unwrap().batch, rep.batches);

    let st = lock(&h);
    assert!(st.violations.is_empty(), "{:?}", st.violations);
    // Marcas: exactamente los cortes, en orden.
    let got: Vec<_> = st
        .emitted()
        .iter()
        .map(|s| (s.kind, s.from, s.to))
        .collect();
    let want: Vec<_> = expected_marks(&dev, &tp)
        .into_iter()
        .map(|(a, b)| (SegKind::Mark, a, b))
        .collect();
    assert_eq!(got, want);
    // Los saltos existen (viajes) y ninguno dispara.
    assert!(!st.jumps().is_empty());
    assert!(st.jumps().iter().all(|j| !j.emitting));
    // Al final la fuente está apagada y el puerto en 0.
    assert!(st.laser_safe());
    assert_eq!(st.port, 0);
    assert!(!st.armed());
    // Se armó una vez y solo durante el trabajo.
    assert_eq!(st.count_immediates(imm::ENABLE_LASER), 1);
    assert!(st.armed_history.iter().any(|a| *a));
    assert!(st.armed_history.last().is_some_and(|a| !*a));
}

#[test]
fn a_long_job_streams_through_flow_control() {
    // Más de 8 tandas (la capacidad simulada): la sesión debe esperar "lista" antes de cada una.
    let dev = device();
    let mut moves = vec![travel(-40.0, -40.0)];
    for i in 0..3000 {
        let y = -40.0 + 80.0 * f64::from(i % 100) / 99.0;
        let x = -40.0 + 80.0 * f64::from(i / 100) / 30.0;
        moves.push(cut(x, y, 20.0 + f64::from(i % 3)));
    }
    let tp = toolpath(&dev, moves);
    let (mut s, h) = session(
        &dev,
        EmuConfig {
            cmds_per_poll: 64,
            ..EmuConfig::default()
        },
    );
    let mut progress = Vec::new();
    let rep = s
        .mark(&tp, false, &JobControl::new(), &mut |p| progress.push(p))
        .unwrap();
    assert!(rep.batches > 8, "{} tandas", rep.batches);
    assert_eq!(progress.len(), rep.batches);
    assert_eq!(progress.last().unwrap().bytes_sent, rep.bytes);
    let st = lock(&h);
    assert!(st.violations.is_empty(), "{:?}", st.violations);
    assert_eq!(st.batches_received, rep.batches);
    let want = expected_marks(&dev, &tp);
    assert_eq!(st.emitted().len(), want.len());
    assert!(st.laser_safe());
}

#[test]
fn nothing_but_the_whitelist_ever_reaches_the_board() {
    let dev = device();
    let (mut s, h) = session(&dev, EmuConfig::default());
    s.probe().unwrap();
    s.mark(&squares(&dev), false, &JobControl::new(), &mut noop)
        .unwrap();
    s.frame(
        Point::new(-10.0, -10.0),
        Point::new(10.0, 10.0),
        &JobControl::new(),
        Some(1),
        &mut |_| {},
    )
    .unwrap();
    let st = lock(&h);
    assert!(st.violations.is_empty());
    for raw in &st.immediates {
        assert!(
            tlaku_galvo::codes::is_sent(raw.code),
            "se envió {:#06x}",
            raw.code
        );
    }
    for forbidden in [
        imm::WRITE_COR_TABLE,
        imm::WRITE_COR_LINE,
        imm::LASER_SIGNAL_ON,
        imm::RESET,
        imm::SET_STANDBY,
        imm::STOP_LIST,
    ] {
        assert_eq!(st.count_immediates(forbidden), 0, "{forbidden:#06x}");
    }
}

#[test]
fn stopping_at_any_batch_leaves_the_source_off() {
    let dev = device();
    let mut moves = vec![travel(-40.0, -40.0)];
    for i in 0..1500 {
        moves.push(cut(
            -40.0 + f64::from(i % 80),
            -40.0 + f64::from(i % 70),
            30.0,
        ));
    }
    let tp = toolpath(&dev, moves);
    for stop_at in 1..=6 {
        let (mut s, h) = session(&dev, EmuConfig::default());
        let ctl = s.control();
        let ctl2 = ctl.clone();
        let err = s
            .mark(&tp, false, &ctl, &mut |p| {
                if p.batch == stop_at {
                    ctl2.request_stop();
                }
            })
            .unwrap_err();
        assert!(matches!(err.cause, SessionError::Cancelled), "{err}");
        assert_eq!(err.stop, StopOutcome::Acked);
        assert!(!err.laser_may_be_on());
        let st = lock(&h);
        assert!(st.laser_safe(), "parada tras la tanda {stop_at}");
        assert!(!st.armed());
        assert!(st.armed_history.last().is_some_and(|a| !*a));
        assert!(st.violations.is_empty());
    }
}

#[test]
fn any_single_transient_fault_ends_with_the_source_off() {
    let dev = device();
    let tp = squares(&dev);
    // Una pasada limpia para saber cuántas escrituras y lecturas hay.
    let (mut s, h) = session(&dev, EmuConfig::default());
    s.mark(&tp, false, &JobControl::new(), &mut noop).unwrap();
    drop(s);
    let (writes, reads) = {
        let st = lock(&h);
        (st.writes, st.reads)
    };
    assert!(writes > 15, "{writes} escrituras");

    let mut failures = 0;
    for k in 1..=writes {
        for fault in [
            Faults {
                fail_writes: vec![k],
                ..Faults::default()
            },
            Faults {
                fail_reads: vec![k.min(reads)],
                ..Faults::default()
            },
        ] {
            let (mut s, h) = session(
                &dev,
                EmuConfig {
                    faults: fault,
                    ..EmuConfig::default()
                },
            );
            let r = s.mark(&tp, false, &JobControl::new(), &mut noop);
            if let Err(e) = &r {
                failures += 1;
                assert_eq!(e.stop, StopOutcome::Acked, "{e}");
            }
            drop(s);
            let st = lock(&h);
            assert!(
                st.laser_safe(),
                "fallo en la llamada {k}: {:?}",
                r.as_ref().err()
            );
            assert!(!st.armed());
            assert!(st.violations.is_empty(), "{:?}", st.violations);
            // Nunca emitió sobre un salto ni fuera de las marcas esperadas.
            let want = expected_marks(&dev, &tp);
            for seg in st.emitted() {
                assert!(want.contains(&(seg.from, seg.to)));
            }
        }
    }
    assert!(
        failures > 10,
        "el barrido debe provocar errores reales ({failures})"
    );
}

#[test]
fn losing_the_usb_is_reported_honestly() {
    let dev = device();
    let tp = squares(&dev);
    let (mut s, h) = session(&dev, EmuConfig::default());
    s.mark(&tp, false, &JobControl::new(), &mut noop).unwrap();
    drop(s);
    let writes = lock(&h).writes;

    let mut armed_at_loss = 0;
    for k in 0..writes {
        let (mut s, h) = session(
            &dev,
            EmuConfig {
                faults: Faults {
                    disconnect_after_writes: Some(k),
                    ..Faults::default()
                },
                ..EmuConfig::default()
            },
        );
        let r = s.mark(&tp, false, &JobControl::new(), &mut noop);
        drop(s);
        let st = lock(&h);
        let err = r.expect_err("el USB se cayó");
        if st.armed() {
            // Sin USB no se puede apagar: lo que importa es que el error NO diga que está bien.
            armed_at_loss += 1;
            assert!(
                err.laser_may_be_on(),
                "la tarjeta quedó armada y el error no lo avisa: {err}"
            );
        }
        if err.stop == StopOutcome::Acked {
            assert!(st.laser_safe());
        }
        // En ningún caso hay comandos prohibidos.
        assert!(st.violations.is_empty());
    }
    assert!(
        armed_at_loss > 0,
        "el barrido debe cubrir cortes con la fuente armada"
    );
}

#[test]
fn dropping_a_dirty_session_switches_everything_off() {
    let dev = device();
    let (mut s, h) = session(&dev, EmuConfig::default());
    s.set_red_light(true).unwrap();
    assert!(lock(&h).red_on());
    drop(s);
    let st = lock(&h);
    assert!(!st.red_on());
    assert!(st.laser_safe());
}

#[test]
fn a_session_that_only_probed_sends_only_reads() {
    let dev = device();
    let (mut s, h) = session(&dev, EmuConfig::default());
    s.probe().unwrap();
    s.status().unwrap();
    s.read_inputs().unwrap();
    drop(s);
    let st = lock(&h);
    for r in &st.immediates {
        assert!(
            [imm::GET_SERIAL_NO, imm::GET_VERSION, imm::READ_PORT].contains(&r.code),
            "{:#06x}",
            r.code
        );
    }
}

#[test]
fn light_framing_never_arms_nor_fires() {
    let dev = device();
    let (mut s, h) = session(&dev, EmuConfig::default());
    let mut loops = Vec::new();
    let rep = s
        .frame(
            Point::new(-20.0, -10.0),
            Point::new(20.0, 10.0),
            &JobControl::new(),
            Some(3),
            &mut |p: LightProgress| loops.push(p.loops),
        )
        .unwrap();
    assert_eq!(rep.loops, 3);
    assert_eq!(loops, vec![1, 2, 3]);
    let st = lock(&h);
    assert!(
        st.armed_history.iter().all(|a| !a),
        "el encuadre nunca arma"
    );
    assert_eq!(st.count_immediates(imm::ENABLE_LASER), 0);
    assert!(
        st.trace
            .iter()
            .all(|s| !s.emitting && s.kind == SegKind::Jump)
    );
    assert!(
        st.trace.iter().any(|s| s.red),
        "el puntero rojo estaba encendido"
    );
    // 4 esquinas más el cierre: 5 saltos por vuelta, el primero desde el centro.
    assert_eq!(st.trace.len(), 3 * 5);
    assert!(st.laser_safe());
    assert!(!st.red_on(), "al terminar el puntero se apaga");
}

#[test]
fn light_loops_until_stopped() {
    let dev = device();
    let (mut s, h) = session(&dev, EmuConfig::default());
    let ctl = s.control();
    let ctl2 = ctl.clone();
    let shape = LightShape::Contour(vec![vec![
        Point::new(-10.0, -10.0),
        Point::new(10.0, -10.0),
        Point::new(0.0, 10.0),
        Point::new(-10.0, -10.0),
    ]]);
    let rep = s
        .light(&shape, &ctl, None, &mut |p| {
            if p.loops == 4 {
                ctl2.request_stop();
            }
        })
        .unwrap();
    assert!(rep.stopped);
    assert_eq!(rep.loops, 4);
    let st = lock(&h);
    assert!(st.laser_safe() && !st.red_on());
    assert!(st.trace.iter().all(|s| !s.emitting));
}

#[test]
fn dry_run_runs_the_whole_job_without_arming() {
    let dev = device();
    let tp = squares(&dev);
    let (mut s, h) = session(&dev, EmuConfig::default());
    let rep = s.mark(&tp, true, &JobControl::new(), &mut noop).unwrap();
    assert!(!rep.armed);
    let st = lock(&h);
    assert_eq!(st.count_immediates(imm::ENABLE_LASER), 0);
    assert!(st.armed_history.iter().all(|a| !a));
    // Hace el mismo recorrido (marcas incluidas) pero sin emitir, con el puntero rojo encendido.
    assert_eq!(
        st.trace.iter().filter(|s| s.kind == SegKind::Mark).count(),
        8
    );
    assert!(st.emitted().is_empty());
    assert!(st.trace.iter().any(|s| s.red));
    assert!(st.laser_safe());
}

#[test]
fn invalid_jobs_never_touch_the_board() {
    let dev = device();
    let (mut s, h) = session(&dev, EmuConfig::default());
    let before = lock(&h).writes;
    // Potencia por encima del perfil (60 %).
    let hot = toolpath(&dev, vec![travel(0.0, 0.0), cut(5.0, 0.0, 61.0)]);
    let e = s
        .mark(&hot, false, &JobControl::new(), &mut noop)
        .unwrap_err();
    assert!(matches!(e.cause, SessionError::Compile(_)), "{e}");
    assert_eq!(e.stop, StopOutcome::NotNeeded);
    // Fuera del campo.
    let far = toolpath(&dev, vec![travel(0.0, 0.0), cut(70.0, 0.0, 10.0)]);
    assert!(s.mark(&far, false, &JobControl::new(), &mut noop).is_err());
    // Frecuencia fuera del perfil.
    let mut bad_freq = toolpath(&dev, vec![travel(0.0, 0.0), cut(5.0, 0.0, 10.0)]);
    bad_freq.blocks[0].galvo.as_mut().unwrap().frequency_khz = 5.0;
    assert!(
        s.mark(&bad_freq, false, &JobControl::new(), &mut noop)
            .is_err()
    );
    // MOPA sin pulso de la lista.
    assert_eq!(lock(&h).writes, before, "no se escribió nada");
}

#[test]
fn mopa_job_sets_the_pulse_width_from_the_profile_list() {
    let mut dev = device();
    dev.source = LaserSource::FiberMopa;
    dev.pulse_widths_ns = vec![4, 9, 13];
    let mut tp = squares(&dev);
    tp.blocks[0].galvo.as_mut().unwrap().pulse_width_ns = Some(9);
    let (mut s, h) = session(&dev, EmuConfig::default());
    s.mark(&tp, false, &JobControl::new(), &mut noop).unwrap();
    assert!(lock(&h).laser_safe());
    tp.blocks[0].galvo.as_mut().unwrap().pulse_width_ns = Some(10);
    assert!(s.mark(&tp, false, &JobControl::new(), &mut noop).is_err());
}

#[test]
fn co2_jobs_never_touch_the_fiber_mo() {
    let mut dev = device();
    dev.source = LaserSource::Co2;
    let (mut s, h) = session(&dev, EmuConfig::default());
    s.mark(&squares(&dev), false, &JobControl::new(), &mut noop)
        .unwrap();
    let st = lock(&h);
    assert_eq!(st.count_immediates(imm::FIBER_SET_MO), 0);
    assert!(st.laser_safe());
    assert_eq!(st.emitted().len(), 8);
}

#[test]
fn unknown_boards_are_rejected_before_any_command() {
    let dev = device();
    for (v, p) in [
        (0x1A86u16, 0x5512u16),
        (0x9588, 0x0001),
        (0x9588, 0x9980),
        (0, 0),
    ] {
        let (t, h) = emulator(EmuConfig {
            vendor_id: v,
            product_id: p,
            ..EmuConfig::default()
        });
        let err = GalvoSession::new(t, cfg(dev.clone()))
            .err()
            .expect("debe rechazar");
        assert!(matches!(err, SessionError::UnknownBoard { .. }), "{err}");
        assert_eq!(lock(&h).writes, 0);
    }
}

#[test]
fn a_closed_interlock_is_required_before_each_job() {
    let dev = device();
    let mut c = cfg(dev.clone());
    // Se exige el bit 3 de las entradas en alto.
    c.inputs_required = Some(tlaku_galvo::session::InputCheck {
        mask: 0x0008,
        value: 0x0008,
    });
    let (t, h) = emulator(EmuConfig {
        inputs: 0,
        ..EmuConfig::default()
    });
    let mut s = GalvoSession::new(t, c.clone()).unwrap();
    let e = s
        .mark(&squares(&dev), false, &JobControl::new(), &mut noop)
        .unwrap_err();
    assert!(matches!(e.cause, SessionError::InterlockOpen { .. }), "{e}");
    assert!(lock(&h).armed_history.iter().all(|a| !a));
    drop(s);

    let (t, h) = emulator(EmuConfig {
        inputs: 0x0008,
        ..EmuConfig::default()
    });
    let mut s = GalvoSession::new(t, c).unwrap();
    s.mark(&squares(&dev), false, &JobControl::new(), &mut noop)
        .unwrap();
    assert!(lock(&h).laser_safe());
}

#[test]
fn a_busy_board_is_not_started() {
    let dev = device();
    let (mut s, h) = session(&dev, EmuConfig::default());
    {
        // Deja a la tarjeta ejecutando una lista sin terminar.
        let mut st = lock(&h);
        st.executing = true;
    }
    let e = s
        .mark(&squares(&dev), false, &JobControl::new(), &mut noop)
        .unwrap_err();
    assert!(matches!(e.cause, SessionError::Busy), "{e}");
    assert!(!lock(&h).armed());
}

#[test]
fn jog_moves_without_arming_and_refuses_points_outside_the_field() {
    let dev = device();
    let (mut s, h) = session(&dev, EmuConfig::default());
    s.jog_to(Point::new(10.0, -20.0)).unwrap();
    {
        let st = lock(&h);
        assert_eq!(st.trace.len(), 1);
        assert!(!st.trace[0].emitting);
        assert_eq!(st.pos, (0x8000 + 6554, 0x8000 - 13107));
    }
    assert!(s.jog_to(Point::new(80.0, 0.0)).is_err());
}

#[test]
fn into_transport_switches_off_first() {
    let dev = device();
    let (mut s, h) = session(&dev, EmuConfig::default());
    s.set_red_light(true).unwrap();
    let _t = s.into_transport();
    assert!(lock(&h).laser_safe());
    assert!(!lock(&h).red_on());
}

#[test]
fn random_jobs_with_field_correction_mark_only_the_cuts() {
    let mut dev = device();
    dev.correction.scale = [1.01, 0.98];
    dev.correction.rotation_deg = 2.0;
    dev.correction.bulge = [0.01, -0.01];
    dev.correction.trapezoid = [0.005, 0.0];
    dev.correction.offset = [0.3, -0.2];
    dev.correction.invert = [false, true];
    let mut seed = 42u64;
    let mut rnd = move || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 33) as f64) / f64::from(1u32 << 31)
    };
    for _ in 0..25 {
        let mut moves = vec![travel(0.0, 0.0)];
        for _ in 0..150 {
            let (x, y) = (rnd() * 70.0 - 35.0, rnd() * 70.0 - 35.0);
            moves.push(match (rnd() * 5.0) as u8 {
                0 => travel(x, y),
                1 => cut(x, y, 0.0),
                _ => cut(x, y, 1.0 + (rnd() * 59.0).floor()),
            });
        }
        let tp = toolpath(&dev, moves);
        let (mut s, h) = session(&dev, EmuConfig::default());
        s.mark(&tp, false, &JobControl::new(), &mut noop).unwrap();
        drop(s);
        let st = lock(&h);
        assert!(st.violations.is_empty());
        let got: Vec<_> = st.emitted().iter().map(|s| (s.from, s.to)).collect();
        assert_eq!(got, expected_marks(&dev, &tp));
        assert!(st.jumps().iter().all(|j| !j.emitting));
        assert!(st.laser_safe());
    }
}
