//! Pruebas de saltos, potencia cero, programas por velocidad y avisos.

use super::*;

#[test]
fn long_travels_use_a_rapid_and_do_not_burn() {
    let path = tp(vec![block(
        1200.0,
        vec![
            travel(10.0, 190.0),
            cut(20.0, 190.0),
            travel(100.0, 100.0),
            cut(110.0, 100.0),
        ],
    )]);
    let p = encode(&path, &cfg()).unwrap();
    let text = s(&p);
    assert_eq!(text.matches("FNSE-").count(), 2, "{text}");
    assert_eq!(p.stats.programs, 2);
    assert_eq!(p.stats.rapids, 2);
    let ev = decode_stream(&p.stream);
    for e in &ev {
        if let Event::Move { laser, mode, .. } = e
            && *mode == MoveMode::Rapid
        {
            assert!(!laser, "un salto rápido no puede quemar");
        }
    }
    assert!(p.stats.burn_mm > 19.0 && p.stats.burn_mm < 21.0);
}

#[test]
fn short_travels_stay_inside_the_program_with_the_laser_off() {
    // Dos trazos separados 2 mm: sin salir del modo compacto.
    let path = tp(vec![block(
        1200.0,
        vec![
            travel(10.0, 190.0),
            cut(20.0, 190.0),
            travel(22.0, 190.0),
            cut(30.0, 190.0),
        ],
    )]);
    let p = encode(&path, &cfg()).unwrap();
    let text = s(&p);
    assert_eq!(text.matches("S1E").count(), 1, "{text}");
    assert_eq!(p.stats.rapids, 1, "solo el salto inicial");
    assert!((p.stats.travel_mm - 2.0).abs() < 0.05);
    let ev = decode_stream(&p.stream);
    let on: Vec<_> = ev
        .iter()
        .filter(|e| matches!(e, Event::Move { laser: true, .. }))
        .collect();
    assert_eq!(on.len(), 2);
}

#[test]
fn power_zero_cuts_are_moves_with_the_laser_off() {
    let path = tp(vec![block(
        1200.0,
        vec![
            travel(10.0, 190.0),
            Move::Cut {
                to: Point::new(15.0, 190.0),
                power_pct: 0.0,
            },
            cut(25.0, 190.0),
            Move::Cut {
                to: Point::new(30.0, 190.0),
                power_pct: 0.0,
            },
        ],
    )]);
    let p = encode(&path, &cfg()).unwrap();
    let ev = decode_stream(&p.stream);
    let burned: f64 = ev
        .iter()
        .filter_map(|e| match e {
            Event::Move {
                from,
                to,
                laser: true,
                ..
            } => Some(mil_to_mm((to.0 - from.0).abs() + (to.1 - from.1).abs())),
            _ => None,
        })
        .sum();
    assert!((burned - 10.0).abs() < 0.1, "{burned}");
    assert!((p.stats.burn_mm - 10.0).abs() < 0.1);
    assert!((p.stats.travel_mm - 10.0).abs() < 0.1);
}

#[test]
fn an_explicit_u_precedes_the_exit_when_the_laser_is_on() {
    let path = tp(vec![block(
        600.0,
        vec![travel(10.0, 190.0), cut(20.0, 190.0)],
    )]);
    let p = encode(&path, &cfg()).unwrap();
    assert!(s(&p).ends_with("UFNSE-\n"), "{}", s(&p));
}

#[test]
fn blocks_with_the_same_speed_share_one_program_and_different_speeds_do_not() {
    let mk = |sp1: f64, sp2: f64| {
        tp(vec![
            block(sp1, vec![travel(10.0, 190.0), cut(20.0, 190.0)]),
            block(sp2, vec![travel(20.0, 188.0), cut(30.0, 188.0)]),
        ])
    };
    let same = encode(&mk(1200.0, 1200.0), &cfg()).unwrap();
    assert_eq!(same.stats.programs, 1);
    let diff = encode(&mk(1200.0, 3000.0), &cfg()).unwrap();
    assert_eq!(diff.stats.programs, 2);
    assert!(s(&diff).matches("FNSE-").count() == 2);
}

#[test]
fn warnings_for_what_the_board_cannot_do() {
    let mut b = block(
        1200.0,
        vec![
            travel(10.0, 190.0),
            Move::Cut {
                to: Point::new(20.0, 190.0),
                power_pct: 30.0,
            },
            Move::Cut {
                to: Point::new(30.0, 190.0),
                power_pct: 60.0,
            },
            Move::Dwell {
                to: Point::new(30.0, 190.0),
                ms: 50.0,
                power_pct: 40.0,
            },
        ],
    );
    b.air_assist = true;
    b.z_mm = Some(2.0);
    let p = encode(&tp(vec![b]), &cfg()).unwrap();
    let w = &p.warnings;
    assert!(w.contains(&Warning::PowerIgnored {
        block: 0,
        pct: 60.0
    }));
    assert!(w.contains(&Warning::GrayThresholded { block: 0 }));
    assert!(w.contains(&Warning::AirAssistIgnored { block: 0 }));
    assert!(w.contains(&Warning::ZIgnored { block: 0 }));
    assert!(w.contains(&Warning::DwellSkipped {
        block: 0,
        ms: 50.0,
        fires: true
    }));
    for x in w {
        assert!(!x.to_string().is_empty());
    }
    // Y el láser nunca queda encendido en el sitio.
    let ev = decode_stream(&p.stream);
    assert!(
        !ev.iter()
            .any(|e| matches!(e, Event::Move { from, to, laser: true, .. } if from == to))
    );
}

#[test]
fn binary_power_has_no_gray_warning_and_zero_power_has_no_power_warning() {
    let p = encode(
        &tp(vec![block(
            1200.0,
            vec![travel(10.0, 190.0), cut(20.0, 190.0)],
        )]),
        &cfg(),
    )
    .unwrap();
    assert!(
        !p.warnings
            .iter()
            .any(|w| matches!(w, Warning::GrayThresholded { .. }))
    );
    let p = encode(
        &tp(vec![block(
            1200.0,
            vec![
                travel(10.0, 190.0),
                Move::Cut {
                    to: Point::new(20.0, 190.0),
                    power_pct: 0.0,
                },
            ],
        )]),
        &cfg(),
    )
    .unwrap();
    assert!(p.warnings.is_empty());
}
