//! Ola 6C del galvo: encuadre (casco convexo, objetos sueltos, ajuste en vivo, desfase del
//! puntero), overscan de rellenos y enclavamiento por entrada del perfil. Todo contra la tarjeta
//! simulada; nada de esto está verificado con hardware (docs/hardware/galvo.md, sección 9).

use tlaku_core::Point;
use tlaku_core::toolpath::{Move, PowerMode, Toolpath, ToolpathBlock};
use tlaku_doc::{GalvoDevice, InputRequirement};
use tlaku_galvo::calib::FieldMap;
use tlaku_galvo::codes::{imm, list};
use tlaku_galvo::compile::{
    self, CompileOptions, LightAdjust, LightOptions, LightShape, params_from_device,
};
use tlaku_galvo::emu::{EmuConfig, EmuHandle, SegKind, emulator, lock};
use tlaku_galvo::safety::{ListKind, SafetyError, SafetyLimits};
use tlaku_galvo::session::{GalvoSession, JobControl, Progress, SessionConfig, SessionError};
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

fn session(dev: &GalvoDevice) -> (GalvoSession<impl Transport>, EmuHandle) {
    let (t, h) = emulator(EmuConfig {
        source: dev.source,
        field_mm: dev.field_mm,
        ..EmuConfig::default()
    });
    let mut cfg = SessionConfig::new(dev.clone()).unwrap();
    cfg.poll_interval = std::time::Duration::ZERO;
    (GalvoSession::new(t, cfg).unwrap(), h)
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn travel(x: f64, y: f64) -> Move {
    Move::Travel { to: pt(x, y) }
}

fn cut(x: f64, y: f64, p: f64) -> Move {
    Move::Cut {
        to: pt(x, y),
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

fn square(x: f64, y: f64, side: f64) -> Vec<Move> {
    vec![
        travel(x, y),
        cut(x + side, y, 40.0),
        cut(x + side, y + side, 40.0),
        cut(x, y + side, 40.0),
        cut(x, y, 40.0),
    ]
}

fn units(dev: &GalvoDevice, x: f64, y: f64) -> (u16, u16) {
    FieldMap::new(dev.field_mm, &dev.correction)
        .unwrap()
        .to_units((x, y))
        .unwrap()
}

fn noop(_: Progress) {}

// ---- casco convexo y objetos sueltos ------------------------------------------------------

#[test]
fn the_hull_wraps_what_is_marked_and_closes() {
    let dev = device();
    // Un triángulo con un punto interior: el casco son sus tres vértices.
    let tp = toolpath(
        &dev,
        vec![
            travel(0.0, 0.0),
            cut(20.0, 0.0, 30.0),
            cut(0.0, 20.0, 30.0),
            cut(0.0, 0.0, 30.0),
            travel(5.0, 5.0),
            cut(6.0, 6.0, 30.0),
        ],
    );
    let Some(LightShape::Contour(lines)) = LightShape::hull_of(&tp) else {
        panic!("el casco debe ser un contorno");
    };
    assert_eq!(lines.len(), 1);
    let ring = &lines[0];
    assert_eq!(ring.len(), 4, "tres vértices más el cierre: {ring:?}");
    assert_eq!(ring.first(), ring.last());
    for v in [pt(0.0, 0.0), pt(20.0, 0.0), pt(0.0, 20.0)] {
        assert!(ring.contains(&v), "falta {v:?}");
    }
}

#[test]
fn the_hull_ignores_travel_and_unpowered_moves() {
    let dev = device();
    let tp = toolpath(
        &dev,
        vec![
            travel(-40.0, -40.0),
            cut(-30.0, -30.0, 0.0), // overscan: no marca
            cut(0.0, 0.0, 30.0),
            cut(10.0, 0.0, 30.0),
            cut(10.0, 10.0, 30.0),
        ],
    );
    let Some(LightShape::Contour(lines)) = LightShape::hull_of(&tp) else {
        panic!()
    };
    let xs: Vec<f64> = lines[0].iter().map(|p| p.x).collect();
    assert!(xs.iter().all(|x| *x >= -30.0), "{xs:?}");
    // El punto de partida del primer tramo con potencia (-30, -30) sí cuenta como marcado.
    assert!(lines[0].contains(&pt(-30.0, -30.0)));
    assert!(!lines[0].contains(&pt(-40.0, -40.0)));
}

#[test]
fn degenerate_hulls_are_a_segment_or_nothing() {
    let dev = device();
    assert!(LightShape::hull_of(&toolpath(&dev, vec![travel(1.0, 1.0)])).is_none());
    let line = toolpath(
        &dev,
        vec![travel(0.0, 0.0), cut(5.0, 0.0, 10.0), cut(9.0, 0.0, 10.0)],
    );
    let Some(LightShape::Contour(lines)) = LightShape::hull_of(&line) else {
        panic!()
    };
    assert_eq!(lines, vec![vec![pt(0.0, 0.0), pt(9.0, 0.0)]]);
}

#[test]
fn loose_objects_are_framed_one_by_one() {
    let dev = device();
    let mut moves = square(-40.0, -40.0, 10.0);
    moves.extend(square(-34.0, -34.0, 6.0)); // se solapa con el primero: mismo objeto
    moves.extend(square(20.0, 20.0, 10.0)); // suelto
    let tp = toolpath(&dev, moves);
    let (shape, n) = LightShape::objects_of(&tp, 0.0).unwrap();
    assert_eq!(n, 2);
    let LightShape::Contour(rects) = shape else {
        panic!()
    };
    assert_eq!(rects.len(), 2);
    // Primero el de abajo a la izquierda.
    assert_eq!(rects[0][0], pt(-40.0, -40.0));
    assert_eq!(rects[0][2], pt(-28.0, -28.0));
    assert_eq!(rects[1][0], pt(20.0, 20.0));
    for r in &rects {
        assert_eq!(r.first(), r.last());
    }
}

#[test]
fn the_gap_joins_objects_that_are_close() {
    let dev = device();
    let mut moves = square(0.0, 0.0, 10.0);
    moves.extend(square(12.0, 0.0, 10.0));
    let tp = toolpath(&dev, moves);
    assert_eq!(LightShape::objects_of(&tp, 0.0).unwrap().1, 2);
    assert_eq!(LightShape::objects_of(&tp, 3.0).unwrap().1, 1);
    assert!(LightShape::objects_of(&toolpath(&dev, vec![travel(0.0, 0.0)]), 0.0).is_none());
}

// ---- ajuste en vivo ------------------------------------------------------------------------

#[test]
fn an_adjust_scales_from_the_center_then_moves() {
    let rect = LightShape::Rect {
        min: pt(-10.0, -10.0),
        max: pt(10.0, 10.0),
    };
    let a = LightAdjust {
        dx: 5.0,
        dy: -2.0,
        scale: 2.0,
    };
    assert_eq!(
        rect.adjusted(&a),
        LightShape::Rect {
            min: pt(-15.0, -22.0),
            max: pt(25.0, 18.0)
        }
    );
    assert_eq!(rect.adjusted(&LightAdjust::default()), rect);
    assert!(LightAdjust::default().is_neutral());
    for bad in [
        LightAdjust {
            scale: 0.0,
            ..LightAdjust::default()
        },
        LightAdjust {
            scale: f64::NAN,
            ..LightAdjust::default()
        },
        LightAdjust {
            dx: 1e6,
            ..LightAdjust::default()
        },
        LightAdjust {
            dy: f64::INFINITY,
            ..LightAdjust::default()
        },
    ] {
        assert!(bad.validate().is_err(), "{bad:?}");
    }
}

#[test]
fn a_live_adjust_moves_the_next_loops_without_arming() {
    let dev = device();
    let (mut s, h) = session(&dev);
    let ctl = s.control();
    let c2 = ctl.clone();
    let shape = LightShape::Rect {
        min: pt(-10.0, -10.0),
        max: pt(10.0, 10.0),
    };
    let mut seen = Vec::new();
    let rep = s
        .light(&shape, &ctl, Some(3), &mut |p| {
            seen.push((p.loops, p.adjust_gen, p.adjust_rejected));
            if p.loops == 1 {
                c2.set_light_adjust(LightAdjust {
                    dx: 5.0,
                    dy: 0.0,
                    scale: 1.0,
                })
                .unwrap();
            }
        })
        .unwrap();
    assert_eq!(rep.loops, 3);
    assert_eq!(seen[0], (1, 0, false));
    assert_eq!(seen[1], (2, 1, false), "el ajuste se aplicó en la vuelta 2");
    assert_eq!(seen[2], (3, 1, false));
    let st = lock(&h);
    assert_eq!(st.trace.len(), 15);
    // Vuelta 1: la esquina inicial en (-10, -10); vuelta 2: movida 5 mm en X.
    assert_eq!(st.trace[0].to, units(&dev, -10.0, -10.0));
    assert_eq!(st.trace[5].to, units(&dev, -5.0, -10.0));
    assert_eq!(st.trace[10].to, units(&dev, -5.0, -10.0));
    assert!(
        st.trace
            .iter()
            .all(|s| !s.emitting && s.kind == SegKind::Jump)
    );
    assert!(st.armed_history.iter().all(|a| !a));
    assert_eq!(st.count_immediates(imm::ENABLE_LASER), 0);
    assert!(st.laser_safe() && !st.red_on());
}

#[test]
fn an_adjust_outside_the_field_is_rejected_and_the_frame_keeps_going() {
    let dev = device();
    let (mut s, h) = session(&dev);
    let ctl = s.control();
    let c2 = ctl.clone();
    let shape = LightShape::Rect {
        min: pt(-10.0, -10.0),
        max: pt(10.0, 10.0),
    };
    let mut seen = Vec::new();
    s.light(&shape, &ctl, Some(2), &mut |p| {
        seen.push((p.adjust_gen, p.adjust_rejected));
        if p.loops == 1 {
            // Con escala 10 el encuadre mediría 200 mm: no cabe en un campo de 100 mm.
            c2.set_light_adjust(LightAdjust {
                dx: 0.0,
                dy: 0.0,
                scale: 10.0,
            })
            .unwrap();
        }
    })
    .unwrap();
    assert_eq!(seen, vec![(0, false), (0, true)]);
    let st = lock(&h);
    assert_eq!(st.trace[0].to, st.trace[5].to, "el encuadre no cambió");
    assert!(st.laser_safe());
}

#[test]
fn an_invalid_adjust_never_reaches_the_session() {
    let ctl = JobControl::new();
    assert!(
        ctl.set_light_adjust(LightAdjust {
            dx: f64::NAN,
            dy: 0.0,
            scale: 1.0
        })
        .is_err()
    );
    assert_eq!(ctl.light_adjust().1, 0);
}

// ---- desfase del puntero --------------------------------------------------------------------

#[test]
fn the_pointer_offset_shifts_the_red_pointer_but_never_a_mark() {
    let dev = device();
    let offset = [2.0, -1.0];
    let tp = toolpath(&dev, square(-10.0, -10.0, 20.0));
    let session_with = |dev: &GalvoDevice| {
        let (t, h) = emulator(EmuConfig {
            field_mm: dev.field_mm,
            ..EmuConfig::default()
        });
        let mut cfg = SessionConfig::new(dev.clone()).unwrap();
        cfg.poll_interval = std::time::Duration::ZERO;
        cfg.pointer_offset = offset;
        (GalvoSession::new(t, cfg).unwrap(), h)
    };

    // Encuadre: el puntero va a (punto - desfase) para caer donde marcará el láser.
    let list = compile::compile_light(
        &LightShape::Rect {
            min: pt(-10.0, -10.0),
            max: pt(10.0, 10.0),
        },
        &dev,
        &LightOptions {
            pointer_offset: offset,
            ..LightOptions::default()
        },
    )
    .unwrap();
    assert_eq!(list.kind, ListKind::Light);
    let (mut s, h) = session_with(&dev);
    s.frame(
        pt(-10.0, -10.0),
        pt(10.0, 10.0),
        &JobControl::new(),
        Some(1),
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(lock(&h).trace[0].to, units(&dev, -12.0, -9.0));
    drop(s);

    // Marca real: las coordenadas no llevan desfase.
    let (mut s, h) = session_with(&dev);
    s.mark(&tp, false, &JobControl::new(), &mut noop).unwrap();
    {
        let st = lock(&h);
        let first_mark = st.trace.iter().find(|x| x.kind == SegKind::Mark).unwrap();
        assert_eq!(first_mark.to, units(&dev, 10.0, -10.0));
    }
    drop(s);

    // Recorrido en rojo del mismo trabajo: sí lleva el desfase.
    let (mut s, h) = session_with(&dev);
    s.mark(&tp, true, &JobControl::new(), &mut noop).unwrap();
    let st = lock(&h);
    let first_mark = st.trace.iter().find(|x| x.kind == SegKind::Mark).unwrap();
    assert_eq!(first_mark.to, units(&dev, 8.0, -9.0));
    assert!(st.emitted().is_empty());
}

#[test]
fn an_absurd_pointer_offset_is_refused_before_any_output() {
    let dev = device();
    let shape = LightShape::Rect {
        min: pt(-5.0, -5.0),
        max: pt(5.0, 5.0),
    };
    for bad in [[51.0, 0.0], [0.0, -60.0], [f64::NAN, 0.0]] {
        let opts = LightOptions {
            pointer_offset: bad,
            ..LightOptions::default()
        };
        assert!(
            compile::compile_light(&shape, &dev, &opts).is_err(),
            "{bad:?}"
        );
    }
}

#[test]
fn an_absurd_overscan_or_interlock_is_a_bad_profile() {
    let mut d = device();
    d.overscan_mm = 21.0;
    assert!(matches!(
        SafetyLimits::from_device(&d),
        Err(SafetyError::BadLimits(_))
    ));
    d.overscan_mm = -1.0;
    assert!(SafetyLimits::from_device(&d).is_err());
    d.overscan_mm = f64::NAN;
    assert!(SafetyLimits::from_device(&d).is_err());
    d.overscan_mm = 2.0;
    assert!(SafetyLimits::from_device(&d).is_ok());
    let mut d = device();
    d.require_input = Some(InputRequirement {
        mask: 0x0001,
        value: 0x0002,
    });
    assert!(SafetyLimits::from_device(&d).is_err());
}

// ---- overscan -------------------------------------------------------------------------------

/// Una línea de relleno con su entrada y salida a potencia 0.
fn scan_line(dev: &GalvoDevice) -> Toolpath {
    toolpath(
        dev,
        vec![
            travel(-20.0, 0.0),
            cut(-18.0, 0.0, 0.0),
            cut(18.0, 0.0, 40.0),
            cut(20.0, 0.0, 0.0),
        ],
    )
}

#[test]
fn without_overscan_the_lead_in_is_an_ordinary_jump() {
    let dev = device();
    let l = compile::compile(&scan_line(&dev), &dev, &CompileOptions::default()).unwrap();
    assert_eq!(l.stats.zero_power_cuts, 2);
    assert_eq!(l.stats.overscan_passes, 0);
}

#[test]
fn overscan_runs_the_lead_in_and_out_at_mark_speed_with_the_laser_off() {
    let mut dev = device();
    dev.overscan_mm = 2.0;
    let tp = scan_line(&dev);
    let l = compile::compile(&tp, &dev, &CompileOptions::default()).unwrap();
    assert_eq!(l.stats.overscan_passes, 2);
    // Nunca hay una marca más que la del tramo con potencia.
    assert_eq!(l.stats.marks, 1);
    // Se cambia la velocidad de salto a la de marcado para el tramo y se vuelve a la del bloque.
    let codes: Vec<u16> = l.commands.iter().map(|r| r.code).collect();
    assert!(codes.iter().filter(|c| **c == list::JUMP_SPEED).count() >= 2);

    // En la tarjeta: el tramo de entrada tarda lo de la velocidad de marcado (1000 mm/s), no lo
    // de la de salto (4000 mm/s); el láser no sale en ninguno de los dos tramos de overscan.
    let (mut s, h) = session(&dev);
    s.mark(&tp, false, &JobControl::new(), &mut noop).unwrap();
    let st = lock(&h);
    let lead_in = st
        .trace
        .iter()
        .find(|x| x.kind == SegKind::Jump && x.to == units(&dev, -18.0, 0.0))
        .expect("tramo de entrada");
    assert!(!lead_in.emitting);
    let plain_dev = device();
    let (mut s2, h2) = session(&plain_dev);
    s2.mark(&scan_line(&plain_dev), false, &JobControl::new(), &mut noop)
        .unwrap();
    let st2 = lock(&h2);
    let plain = st2
        .trace
        .iter()
        .find(|x| x.kind == SegKind::Jump && x.to == units(&plain_dev, -18.0, 0.0))
        .unwrap();
    assert!(
        lead_in.us > plain.us * 2.0,
        "a velocidad de marcado ({:.0} us) tarda más que a la de salto ({:.0} us)",
        lead_in.us,
        plain.us
    );
    assert_eq!(st.emitted().len(), 1);
    assert!(st.laser_safe());
    assert_eq!(st.count_immediates(imm::ENABLE_LASER), 1);
}

#[test]
fn after_an_overscan_pass_the_next_jump_goes_back_to_jump_speed() {
    let mut dev = device();
    dev.overscan_mm = 2.0;
    let mut moves = vec![
        travel(-20.0, 0.0),
        cut(-18.0, 0.0, 0.0),
        cut(18.0, 0.0, 40.0),
        cut(20.0, 0.0, 0.0),
    ];
    moves.extend([
        travel(-20.0, 5.0),
        cut(-18.0, 5.0, 0.0),
        cut(18.0, 5.0, 40.0),
    ]);
    let tp = toolpath(&dev, moves);
    let l = compile::compile(&tp, &dev, &CompileOptions::default()).unwrap();
    // JumpSpeed del bloque, del overscan, de vuelta al del bloque, del overscan siguiente.
    let speeds: Vec<u16> = l
        .commands
        .iter()
        .filter(|r| r.code == list::JUMP_SPEED)
        .map(|r| r.p[0])
        .collect();
    assert!(speeds.len() >= 4, "{speeds:?}");
    assert_eq!(speeds[0], speeds[2]);
    assert_ne!(speeds[0], speeds[1]);
}

// ---- enclavamiento por entrada del perfil ----------------------------------------------------

#[test]
fn the_profile_interlock_travels_with_the_session() {
    let mut dev = device();
    dev.require_input = Some(InputRequirement {
        mask: 0x0008,
        value: 0x0008,
    });
    let cfg = SessionConfig::new(dev.clone()).unwrap();
    assert_eq!(cfg.inputs_required.map(|c| (c.mask, c.value)), Some((8, 8)));
    assert!(
        SessionConfig::new(device())
            .unwrap()
            .inputs_required
            .is_none()
    );

    // Abierto: no se marca ni se encuadra.
    let (t, h) = emulator(EmuConfig {
        field_mm: dev.field_mm,
        inputs: 0,
        ..EmuConfig::default()
    });
    let mut s = GalvoSession::new(t, cfg.clone()).unwrap();
    let tp = toolpath(&dev, square(-5.0, -5.0, 10.0));
    let e = s
        .mark(&tp, false, &JobControl::new(), &mut noop)
        .unwrap_err();
    assert!(matches!(e.cause, SessionError::InterlockOpen { .. }), "{e}");
    let e = s
        .frame(
            pt(-5.0, -5.0),
            pt(5.0, 5.0),
            &JobControl::new(),
            Some(1),
            &mut |_| {},
        )
        .unwrap_err();
    assert!(matches!(e.cause, SessionError::InterlockOpen { .. }), "{e}");
    assert!(lock(&h).armed_history.iter().all(|a| !a));
}

#[test]
fn opening_the_interlock_during_a_frame_stops_it() {
    let mut dev = device();
    dev.require_input = Some(InputRequirement {
        mask: 0x0008,
        value: 0x0008,
    });
    let mut cfg = SessionConfig::new(dev.clone()).unwrap();
    cfg.poll_interval = std::time::Duration::ZERO;
    let (t, h) = emulator(EmuConfig {
        field_mm: dev.field_mm,
        inputs: 0x0008,
        ..EmuConfig::default()
    });
    let mut s = GalvoSession::new(t, cfg).unwrap();
    let h2 = h.clone();
    let e = s
        .frame(
            pt(-5.0, -5.0),
            pt(5.0, 5.0),
            &JobControl::new(),
            None,
            &mut |p| {
                if p.loops == 2 {
                    lock(&h2).cfg.inputs = 0;
                }
            },
        )
        .unwrap_err();
    assert!(matches!(e.cause, SessionError::InterlockOpen { .. }), "{e}");
    let st = lock(&h);
    assert!(st.laser_safe() && !st.red_on());
}
