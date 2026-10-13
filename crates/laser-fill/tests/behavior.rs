mod common;

use common::*;
use laser_core::toolpath::Move;
use laser_core::{Point, Polyline};
use laser_fill::{FillRule, FillSettings, Grouping, fill};

fn base() -> FillSettings {
    FillSettings {
        interval_mm: 1.0,
        bidirectional: false,
        ..Default::default()
    }
}

fn travels(moves: &[Move]) -> usize {
    moves
        .iter()
        .filter(|m| matches!(m, Move::Travel { .. }))
        .count()
}

fn area(block: &laser_core::toolpath::ToolpathBlock, interval: f64) -> f64 {
    burned_length(block) * interval
}

#[test]
fn square_has_one_line_per_interval() {
    let r = fill(&[rect(0.0, 0.0, 10.0, 10.0)], &base()).unwrap();
    assert!(r.ignored_open.is_empty());
    let b = burns(&r.block);
    assert_eq!(b.len(), 10);
    assert_eq!(travels(&r.block.moves), 10);
    for (i, (a, e, p)) in b.iter().enumerate() {
        assert_eq!((a.x, e.x), (0.0, 10.0));
        assert_eq!(a.y, i as f64 + 0.5);
        assert_eq!(e.y, a.y);
        assert_eq!(*p, 20.0);
    }
}

#[test]
fn settings_are_copied_to_block() {
    let s = FillSettings {
        speed_mm_min: 4321.0,
        air_assist: true,
        power_pct: 55.0,
        ..base()
    };
    let r = fill(&[rect(0.0, 0.0, 3.0, 3.0)], &s).unwrap();
    assert_eq!(r.block.speed_mm_min, 4321.0);
    assert!(r.block.air_assist);
    assert!(burns(&r.block).iter().all(|b| b.2 == 55.0));
}

#[test]
fn circle_area_matches() {
    let s = FillSettings {
        interval_mm: 0.1,
        ..base()
    };
    let c = circle(0.0, 0.0, 10.0, 128);
    let r = fill(std::slice::from_ref(&c), &s).unwrap();
    let expected = 128.0 / 2.0 * 100.0 * (std::f64::consts::TAU / 128.0).sin();
    let got = area(&r.block, 0.1);
    assert!(
        (got - expected).abs() / expected < 0.005,
        "{got} vs {expected}"
    );
    for (a, b, _) in burns(&r.block) {
        let mid = Point::new((a.x + b.x) / 2.0, a.y);
        assert!(mid.dist(Point::new(0.0, 0.0)) < 10.0);
    }
}

#[test]
fn donut_hole_is_not_burned() {
    let paths = [rect(0.0, 0.0, 10.0, 10.0), rect_cw(3.0, 3.0, 7.0, 7.0)];
    let r = fill(&paths, &base()).unwrap();
    for (a, b, _) in burns(&r.block) {
        let (lo, hi) = (a.x.min(b.x), a.x.max(b.x));
        // Ningún tramo con potencia atraviesa el hueco.
        assert!(
            !(a.y > 3.0 && a.y < 7.0) || hi <= 3.0 || lo >= 7.0,
            "{a:?} {b:?}"
        );
    }
    assert!((area(&r.block, 1.0) - (100.0 - 16.0)).abs() < 1e-9);
    // Una línea que cruza el hueco: dos tramos y un recorrido con potencia 0 en medio.
    let mut line_moves = r
        .block
        .moves
        .iter()
        .skip_while(|m| !matches!(m, Move::Travel { to } if to.y == 4.5));
    line_moves.next();
    let seq: Vec<_> = line_moves
        .take_while(|m| !matches!(m, Move::Travel { .. }))
        .collect();
    assert_eq!(seq.len(), 3);
    assert!(matches!(seq[1], Move::Cut { power_pct, to } if *power_pct == 0.0 && to.x == 7.0));
}

#[test]
fn donut_hole_same_orientation_depends_on_rule() {
    let paths = [rect(0.0, 0.0, 10.0, 10.0), rect(3.0, 3.0, 7.0, 7.0)];
    let eo = fill(&paths, &base()).unwrap();
    assert!((area(&eo.block, 1.0) - 84.0).abs() < 1e-9);
    let nz = fill(
        &paths,
        &FillSettings {
            fill_rule: FillRule::NonZero,
            ..base()
        },
    )
    .unwrap();
    assert!((area(&nz.block, 1.0) - 100.0).abs() < 1e-9);
}

#[test]
fn overlap_evenodd_vs_nonzero() {
    let paths = [rect(0.0, 0.0, 10.0, 10.0), rect(5.0, 0.0, 15.0, 10.0)];
    let eo = fill(&paths, &base()).unwrap();
    // Se quema 5+5 por línea y el solape (5 mm) queda vacío.
    assert!((area(&eo.block, 1.0) - 100.0).abs() < 1e-9);
    let nz = fill(
        &paths,
        &FillSettings {
            fill_rule: FillRule::NonZero,
            ..base()
        },
    )
    .unwrap();
    assert!((area(&nz.block, 1.0) - 150.0).abs() < 1e-9);
    // Con sentidos opuestos el giro en el solape es 0 (+1 -1) y NonZero coincide con par-impar.
    let opposite = [rect(0.0, 0.0, 10.0, 10.0), rect_cw(5.0, 0.0, 15.0, 10.0)];
    let nz2 = fill(
        &opposite,
        &FillSettings {
            fill_rule: FillRule::NonZero,
            ..base()
        },
    )
    .unwrap();
    assert!((area(&nz2.block, 1.0) - 100.0).abs() < 1e-9);
}

#[test]
fn angle_45_rotates_lines() {
    let s = FillSettings {
        angle_deg: 45.0,
        interval_mm: 0.5,
        ..base()
    };
    let sq = rect(0.0, 0.0, 10.0, 10.0);
    let r = fill(std::slice::from_ref(&sq), &s).unwrap();
    for (a, b, _) in burns(&r.block) {
        assert!(((b.x - a.x) - (b.y - a.y)).abs() < 1e-9, "no es diagonal");
        let mid = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        assert!(mid.x > 0.0 && mid.x < 10.0 && mid.y > 0.0 && mid.y < 10.0);
    }
    let got = area(&r.block, 0.5);
    assert!((got - 100.0).abs() < 2.0, "{got}");
}

#[test]
fn angle_90_is_vertical_and_exact() {
    let s = FillSettings {
        angle_deg: 90.0,
        ..base()
    };
    let r = fill(&[rect(0.0, 0.0, 10.0, 4.0)], &s).unwrap();
    let b = burns(&r.block);
    assert_eq!(b.len(), 10);
    for (a, e, _) in b {
        assert_eq!(a.x, e.x);
        assert_eq!((a.y, e.y), (0.0, 4.0));
    }
}

#[test]
fn crosshatch_adds_perpendicular_pass() {
    let sq = rect(0.0, 0.0, 10.0, 10.0);
    let single = fill(std::slice::from_ref(&sq), &base()).unwrap();
    let cross = fill(
        std::slice::from_ref(&sq),
        &FillSettings {
            crosshatch: true,
            ..base()
        },
    )
    .unwrap();
    assert!((burned_length(&cross.block) - 2.0 * burned_length(&single.block)).abs() < 1e-9);
    let b = burns(&cross.block);
    assert_eq!(b.iter().filter(|(a, e, _)| a.y == e.y).count(), 10);
    assert_eq!(b.iter().filter(|(a, e, _)| a.x == e.x).count(), 10);
    // Las 10 horizontales van primero.
    assert!(b[..10].iter().all(|(a, e, _)| a.y == e.y));
}

#[test]
fn bidirectional_alternates_direction() {
    let sq = rect(0.0, 0.0, 10.0, 4.0);
    let bi = fill(
        std::slice::from_ref(&sq),
        &FillSettings {
            bidirectional: true,
            ..base()
        },
    )
    .unwrap();
    let dirs: Vec<f64> = burns(&bi.block)
        .iter()
        .map(|(a, e, _)| (e.x - a.x).signum())
        .collect();
    assert_eq!(dirs, [1.0, -1.0, 1.0, -1.0]);
    let uni = fill(std::slice::from_ref(&sq), &base()).unwrap();
    assert!(burns(&uni.block).iter().all(|(a, e, _)| e.x > a.x));
}

#[test]
fn overscan_both_directions() {
    let s = FillSettings {
        bidirectional: true,
        overscan_mm: 2.0,
        ..base()
    };
    let r = fill(&[rect(0.0, 0.0, 10.0, 2.0)], &s).unwrap();
    let p = |x: f64, y: f64| Point::new(x, y);
    let expected = [
        Move::Travel { to: p(-2.0, 0.5) },
        Move::Cut {
            to: p(0.0, 0.5),
            power_pct: 0.0,
        },
        Move::Cut {
            to: p(10.0, 0.5),
            power_pct: 20.0,
        },
        Move::Cut {
            to: p(12.0, 0.5),
            power_pct: 0.0,
        },
        Move::Travel { to: p(12.0, 1.5) },
        Move::Cut {
            to: p(10.0, 1.5),
            power_pct: 0.0,
        },
        Move::Cut {
            to: p(0.0, 1.5),
            power_pct: 20.0,
        },
        Move::Cut {
            to: p(-2.0, 1.5),
            power_pct: 0.0,
        },
    ];
    assert_eq!(r.block.moves, expected);
}

#[test]
fn zero_overscan_has_no_idle_cuts() {
    let r = fill(&[rect(0.0, 0.0, 10.0, 3.0)], &base()).unwrap();
    assert_eq!(r.block.moves.len(), 6);
    assert!(burns(&r.block).len() == 3);
}

#[test]
fn open_paths_are_reported() {
    let open = Polyline {
        points: vec![
            Point::new(0.0, 0.0),
            Point::new(5.0, 0.0),
            Point::new(5.0, 5.0),
        ],
        closed: false,
    };
    let r = fill(&[open.clone(), rect(0.0, 0.0, 4.0, 4.0), open], &base()).unwrap();
    assert_eq!(r.ignored_open, vec![0, 2]);
    assert_eq!(burns(&r.block).len(), 4);
}

#[test]
fn degenerate_inputs_do_not_panic() {
    let nan = poly(&[(0.0, 0.0), (f64::NAN, 1.0), (3.0, 3.0)]);
    let inf = poly(&[(0.0, 0.0), (f64::INFINITY, 1.0), (3.0, 3.0)]);
    let cases: Vec<Vec<Polyline>> = vec![
        vec![],
        vec![poly(&[])],
        vec![poly(&[(1.0, 1.0)])],
        vec![poly(&[(1.0, 1.0), (4.0, 4.0)])],
        vec![poly(&[(1.0, 1.0), (1.0, 1.0), (1.0, 1.0), (1.0, 1.0)])],
        vec![poly(&[(0.0, 0.0), (5.0, 0.0), (10.0, 0.0)])],
        vec![poly(&[(0.0, 0.0), (5.0, 5.0), (10.0, 10.0)])],
        vec![poly(&[(0.0, 0.0), (10.0, 10.0), (10.0, 0.0), (0.0, 10.0)])],
        vec![Polyline {
            points: vec![],
            closed: false,
        }],
        vec![nan, inf, rect(0.0, 0.0, 2.0, 2.0)],
        vec![poly(&[(0.0, 0.0), (1e300, 0.0), (1e300, 1e300)])],
    ];
    for grouping in [Grouping::All, Grouping::PerShape] {
        for rule in [FillRule::EvenOdd, FillRule::NonZero] {
            for crosshatch in [false, true] {
                let s = FillSettings {
                    grouping,
                    fill_rule: rule,
                    crosshatch,
                    angle_deg: 33.0,
                    overscan_mm: 1.0,
                    ..base()
                };
                for c in &cases {
                    let _ = fill(c, &s);
                }
            }
        }
    }
    // Triángulo degenerado con puntos repetidos y una forma válida junto a él.
    let r = fill(
        &[
            poly(&[(0.0, 0.0), (0.0, 0.0), (5.0, 0.0), (5.0, 0.0)]),
            rect(0.0, 0.0, 2.0, 2.0),
        ],
        &base(),
    )
    .unwrap();
    assert_eq!(burns(&r.block).len(), 2);
}

#[test]
fn vertices_on_scanlines_do_not_duplicate_crossings() {
    // Rombo con vértices exactamente sobre líneas de barrido (y = 2.5) y una arista horizontal.
    let diamond = poly(&[(0.0, 2.5), (2.5, 5.0), (5.0, 2.5), (2.5, 0.0)]);
    let r = fill(std::slice::from_ref(&diamond), &base()).unwrap();
    let b = burns(&r.block);
    assert_eq!(b.len(), 5, "una sola pasada por línea");
    let mid = b.iter().find(|(a, _, _)| a.y == 2.5).unwrap();
    assert_eq!((mid.0.x, mid.1.x), (0.0, 5.0));
    // Escalera con aristas horizontales sobre una línea.
    let stairs = poly(&[
        (0.0, 0.0),
        (6.0, 0.0),
        (6.0, 1.5),
        (3.0, 1.5),
        (3.0, 3.0),
        (0.0, 3.0),
    ]);
    let r = fill(std::slice::from_ref(&stairs), &base()).unwrap();
    // La línea y = 1.5 coincide con la arista horizontal: [ymin, ymax) la asigna a la parte de arriba.
    let want = 6.0 + 3.0 + 3.0;
    assert!((area(&r.block, 1.0) - want).abs() < 1e-9);
}

#[test]
fn self_intersecting_star_rules() {
    let pts: Vec<(f64, f64)> = (0..5)
        .map(|i| {
            let a = std::f64::consts::TAU * (i * 2) as f64 / 5.0 + 1.0;
            (10.0 * a.cos(), 10.0 * a.sin())
        })
        .collect();
    let star = [poly(&pts)];
    let s = FillSettings {
        interval_mm: 0.05,
        ..base()
    };
    let eo = fill(&star, &s).unwrap();
    let nz = fill(
        &star,
        &FillSettings {
            fill_rule: FillRule::NonZero,
            ..s
        },
    )
    .unwrap();
    // El pentágono central cuenta con giro 2: par-impar lo deja vacío, non-zero lo llena.
    assert!(area(&nz.block, 0.05) > area(&eo.block, 0.05) * 1.3);
}

#[test]
fn per_shape_skips_empty_space() {
    let a = rect(0.0, 0.0, 10.0, 4.0);
    let b = rect(100.0, 0.0, 110.0, 4.0);
    let s = FillSettings {
        overscan_mm: 1.0,
        ..base()
    };
    let all = fill(&[a.clone(), b.clone()], &s).unwrap();
    let per = fill(
        &[a, b],
        &FillSettings {
            grouping: Grouping::PerShape,
            ..s
        },
    )
    .unwrap();
    assert_eq!(travels(&all.block.moves), 4);
    assert_eq!(travels(&per.block.moves), 8);
    assert!((burned_length(&all.block) - burned_length(&per.block)).abs() < 1e-9);
    let idle_cut = idle_length(&per.block);
    // Solo overscan: 1 mm a cada lado de cada línea.
    assert!((idle_cut - 8.0 * 2.0).abs() < 1e-9, "{idle_cut}");
    // En All, el hueco de 90 mm se recorre con láser apagado en cada línea.
    let idle_cut_all = idle_length(&all.block);
    assert!(idle_cut_all > 4.0 * 90.0);
}

#[test]
fn per_shape_keeps_holes_and_overlaps_together() {
    // Dona + isla dentro del hueco + dos cuadros solapados: el resultado coincide con All.
    let paths = [
        rect(0.0, 0.0, 20.0, 20.0),
        rect_cw(4.0, 4.0, 16.0, 16.0),
        rect(8.0, 8.0, 12.0, 12.0),
        rect(50.0, 0.0, 60.0, 10.0),
        rect(55.0, 5.0, 65.0, 15.0),
    ];
    for rule in [FillRule::EvenOdd, FillRule::NonZero] {
        let s = FillSettings {
            fill_rule: rule,
            interval_mm: 0.5,
            ..base()
        };
        let all = fill(&paths, &s).unwrap();
        let per = fill(
            &paths,
            &FillSettings {
                grouping: Grouping::PerShape,
                ..s
            },
        )
        .unwrap();
        assert!((burned_length(&all.block) - burned_length(&per.block)).abs() < 1e-9);
    }
}

#[test]
fn per_shape_serpentine_restarts_per_component() {
    let s = FillSettings {
        bidirectional: true,
        grouping: Grouping::PerShape,
        ..base()
    };
    let r = fill(&[rect(0.0, 0.0, 4.0, 2.0), rect(10.0, 0.0, 14.0, 2.0)], &s).unwrap();
    let dirs: Vec<f64> = burns(&r.block)
        .iter()
        .map(|(a, e, _)| (e.x - a.x).signum())
        .collect();
    assert_eq!(dirs, [1.0, -1.0, 1.0, -1.0]);
}

#[test]
fn invalid_settings_are_rejected() {
    let sq = [rect(0.0, 0.0, 1.0, 1.0)];
    let bad = [
        FillSettings {
            interval_mm: 0.0,
            ..base()
        },
        FillSettings {
            interval_mm: -1.0,
            ..base()
        },
        FillSettings {
            interval_mm: f64::NAN,
            ..base()
        },
        FillSettings {
            overscan_mm: -1.0,
            ..base()
        },
        FillSettings {
            angle_deg: f64::INFINITY,
            ..base()
        },
        FillSettings {
            power_pct: 101.0,
            ..base()
        },
        FillSettings {
            speed_mm_min: 0.0,
            ..base()
        },
    ];
    for s in bad {
        assert!(fill(&sq, &s).is_err(), "{s:?}");
    }
}

#[test]
fn absurd_line_count_is_an_error_not_a_hang() {
    let s = FillSettings {
        interval_mm: 1e-9,
        ..base()
    };
    let e = fill(&[rect(0.0, 0.0, 100.0, 100.0)], &s).unwrap_err();
    assert!(e.contains("demasiadas"));
}

#[test]
fn empty_input_gives_empty_block() {
    let r = fill(&[], &base()).unwrap();
    assert!(r.block.moves.is_empty());
}

#[test]
fn performance_many_polygons_and_lines() {
    let t = std::time::Instant::now();
    // 2500 círculos de 32 lados en rejilla, intervalo fino.
    let mut paths = Vec::new();
    for i in 0..50 {
        for j in 0..50 {
            paths.push(circle(i as f64 * 5.0, j as f64 * 5.0, 2.0, 32));
        }
    }
    let s = FillSettings {
        interval_mm: 0.05,
        overscan_mm: 1.0,
        ..base()
    };
    for grouping in [Grouping::All, Grouping::PerShape] {
        let r = fill(
            &paths,
            &FillSettings {
                grouping,
                ..s.clone()
            },
        )
        .unwrap();
        assert!(r.block.moves.len() > 100_000);
    }
    // Un solo polígono grande con decenas de miles de líneas.
    let big = fill(
        &[rect(0.0, 0.0, 200.0, 200.0)],
        &FillSettings {
            interval_mm: 0.005,
            ..base()
        },
    )
    .unwrap();
    assert_eq!(burns(&big.block).len(), 40_000);
    assert!(
        t.elapsed().as_secs() < 60,
        "demasiado lento: {:?}",
        t.elapsed()
    );
}
