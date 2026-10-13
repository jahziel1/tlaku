use super::*;
use laser_core::toolpath::{Move, PowerMode, ToolpathBlock};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn cut(x: f64, y: f64) -> Move {
    Move::Cut {
        to: p(x, y),
        power_pct: 50.0,
    }
}

fn travel(x: f64, y: f64) -> Move {
    Move::Travel { to: p(x, y) }
}

fn block(speed_mm_min: f64, moves: Vec<Move>) -> ToolpathBlock {
    ToolpathBlock {
        label: "prueba".into(),
        speed_mm_min,
        air_assist: false,
        power_mode: PowerMode::Dynamic,
        moves,
    }
}

fn job(blocks: Vec<ToolpathBlock>) -> Toolpath {
    Toolpath { blocks }
}

/// 1000 mm/s² y 600000 mm/min de eje: solo mandan la aceleración y la velocidad del bloque.
fn profile() -> MachineProfile {
    MachineProfile {
        max_rate_mm_min: [600_000.0; 2],
        accel_mm_s2: [1000.0; 2],
        rapid_rate_mm_min: 600_000.0,
        junction_deviation_mm: 0.01,
        planner_blocks: None,
    }
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9 * (1.0 + b.abs()), "{a} != {b}");
}

#[test]
fn long_move_takes_d_over_v_plus_v_over_a() {
    // 100 mm a 100 mm/s con a = 1000: 100/100 + 100/1000 = 1.1 s
    let tp = job(vec![block(6000.0, vec![travel(0.0, 0.0), cut(100.0, 0.0)])]);
    let e = estimate(&tp, &profile());
    close(e.total_s, 1.1);
    close(e.blocks[0].cut_mm, 100.0);
    close(e.blocks[0].travel_mm, 0.0);
}

#[test]
fn short_move_is_triangular() {
    // 1 mm nunca llega a 100 mm/s: t = 2 sqrt(d/a)
    let tp = job(vec![block(6000.0, vec![cut(1.0, 0.0)])]);
    close(
        estimate(&tp, &profile()).total_s,
        2.0 * (1.0f64 / 1000.0).sqrt(),
    );
}

#[test]
fn collinear_subdivision_matches_single_segment() {
    let single = job(vec![block(6000.0, vec![cut(100.0, 0.0)])]);
    let split = job(vec![block(
        6000.0,
        (1..=1000).map(|i| cut(i as f64 * 0.1, 0.0)).collect(),
    )]);
    let a = estimate(&single, &profile()).total_s;
    let b = estimate(&split, &profile()).total_s;
    assert!((a - b).abs() / a < 1e-6, "{a} vs {b}");
}

#[test]
fn right_angle_corner_is_slower_than_straight_line() {
    let straight = job(vec![block(6000.0, vec![cut(50.0, 0.0), cut(100.0, 0.0)])]);
    let corner = job(vec![block(6000.0, vec![cut(50.0, 0.0), cut(50.0, 50.0)])]);
    let ts = estimate(&straight, &profile()).total_s;
    let tc = estimate(&corner, &profile()).total_s;
    assert!(tc > ts + 0.05, "esquina {tc} vs recta {ts}");
}

#[test]
fn smaller_junction_deviation_slows_corners() {
    let tp = job(vec![block(6000.0, vec![cut(50.0, 0.0), cut(50.0, 50.0)])]);
    let mut loose = profile();
    loose.junction_deviation_mm = 0.1;
    let mut tight = profile();
    tight.junction_deviation_mm = 0.0;
    assert!(estimate(&tp, &tight).total_s > estimate(&tp, &loose).total_s);
}

#[test]
fn raster_sweep_is_much_slower_than_naive_d_over_v() {
    // 100 filas de 10 mm, 0.1 mm de separación, ida y vuelta: giros de 180° en cada extremo.
    let mut moves = vec![];
    for i in 0..100 {
        let y = i as f64 * 0.1;
        let (x0, x1) = if i % 2 == 0 { (0.0, 10.0) } else { (10.0, 0.0) };
        moves.push(cut(x0, y));
        moves.push(cut(x1, y));
    }
    let tp = job(vec![block(6000.0, moves)]);
    let e = estimate(&tp, &profile());
    let naive = e.blocks[0].cut_mm / 100.0;
    assert!(e.total_s > 1.5 * naive, "{} vs naive {naive}", e.total_s);
}

#[test]
fn overscan_matches_formula() {
    let pr = profile();
    close(
        overscan_mm(6000.0, 0.0, &pr),
        100.0 * 100.0 / (2.0 * 1000.0),
    );
    // En diagonal con ejes distintos, manda la aceleración efectiva.
    let pr2 = MachineProfile {
        accel_mm_s2: [1000.0, 500.0],
        ..profile()
    };
    let a = 500.0 * 2f64.sqrt();
    close(
        overscan_mm(3000.0, FRAC_PI_4, &pr2),
        50.0 * 50.0 / (2.0 * a),
    );
    close(overscan_mm(3000.0, FRAC_PI_2, &pr2), 50.0 * 50.0 / 1000.0);
    assert_eq!(overscan_mm(0.0, 0.0, &pr), 0.0);
    assert_eq!(overscan_mm(f64::NAN, 0.0, &pr), 0.0);
}

#[test]
fn overscan_clamps_speed_to_axis_limit() {
    let pr = MachineProfile {
        max_rate_mm_min: [3000.0, 3000.0],
        ..profile()
    };
    close(
        overscan_mm(60_000.0, 0.0, &pr),
        overscan_mm(3000.0, 0.0, &pr),
    );
}

#[test]
fn limited_planner_window_is_slower_on_tiny_segments() {
    // 0.1 mm por segmento: 16 bloques son 1.6 mm, menos que lo que se necesita para frenar de 100 mm/s.
    let tp = job(vec![block(
        6000.0,
        (1..=1000).map(|i| cut(i as f64 * 0.1, 0.0)).collect(),
    )]);
    let full = estimate(&tp, &profile()).total_s;
    let mut grbl = profile();
    grbl.planner_blocks = Some(16);
    let limited = estimate(&tp, &grbl).total_s;
    assert!(limited > full * 1.5, "{limited} vs {full}");

    // Una ventana mayor que el programa equivale a mirar todo.
    grbl.planner_blocks = Some(5000);
    close(estimate(&tp, &grbl).total_s, full);
    // Más ventana nunca empeora.
    grbl.planner_blocks = Some(32);
    assert!(estimate(&tp, &grbl).total_s <= limited + 1e-12);
}

#[test]
fn window_of_one_stops_on_every_segment() {
    let tp = job(vec![block(
        6000.0,
        (1..=10).map(|i| cut(i as f64 * 0.5, 0.0)).collect(),
    )]);
    let mut pr = profile();
    pr.planner_blocks = Some(1);
    let each = 2.0 * (0.5f64 / 1000.0).sqrt();
    close(estimate(&tp, &pr).total_s, 10.0 * each);
}

#[test]
fn limited_window_keeps_long_segments_unchanged() {
    // Pocos segmentos largos: la ventana de 16 los ve todos.
    let tp = job(vec![block(
        6000.0,
        vec![cut(50.0, 0.0), cut(100.0, 0.0), cut(100.0, 50.0)],
    )]);
    let mut pr = profile();
    let full = estimate(&tp, &pr).total_s;
    pr.planner_blocks = Some(16);
    close(estimate(&tp, &pr).total_s, full);
}

#[test]
fn stops_between_travel_and_cut_and_between_blocks() {
    // Dos tramos colineales de 50 mm: seguidos en un bloque no frenan; si cambian de tipo, sí.
    let joined = job(vec![block(6000.0, vec![cut(50.0, 0.0), cut(100.0, 0.0)])]);
    let mixed = job(vec![block(
        6000.0,
        vec![travel(50.0, 0.0), cut(100.0, 0.0)],
    )]);
    let two_blocks = job(vec![
        block(6000.0, vec![cut(50.0, 0.0)]),
        block(6000.0, vec![cut(100.0, 0.0)]),
    ]);
    let pr = MachineProfile {
        rapid_rate_mm_min: 6000.0,
        ..profile()
    };
    let tj = estimate(&joined, &pr).total_s;
    let tm = estimate(&mixed, &pr).total_s;
    let tb = estimate(&two_blocks, &pr).total_s;
    close(tj, 1.1);
    assert!(tm > tj + 0.05);
    close(tb, 2.0 * (0.5 + 0.1));
    assert_eq!(estimate(&two_blocks, &pr).blocks.len(), 2);
}

#[test]
fn travel_uses_rapid_rate_and_counts_distance() {
    let tp = job(vec![block(1000.0, vec![travel(100.0, 0.0)])]);
    let mut pr = profile();
    pr.rapid_rate_mm_min = 12_000.0; // 200 mm/s
    let e = estimate(&tp, &pr);
    close(e.total_s, 100.0 / 200.0 + 200.0 / 1000.0);
    close(e.blocks[0].travel_mm, 100.0);
}

#[test]
fn diagonal_is_limited_by_axis_rates() {
    // 6000 mm/min por eje: en diagonal exacta se llega a 100·√2 mm/s, aunque el bloque pida más.
    let pr = MachineProfile {
        max_rate_mm_min: [6000.0; 2],
        ..profile()
    };
    let tp = job(vec![block(60_000.0, vec![cut(100.0, 100.0)])]);
    let v = 100.0 * 2f64.sqrt();
    let a = 1000.0 * 2f64.sqrt();
    let d = 100.0 * 2f64.sqrt();
    close(estimate(&tp, &pr).total_s, d / v + v / a);
}

#[test]
fn zero_length_and_degenerate_input_do_not_break() {
    let pr = profile();
    assert_eq!(estimate(&Toolpath::default(), &pr).total_s, 0.0);
    let empty_block = job(vec![block(6000.0, vec![])]);
    let e = estimate(&empty_block, &pr);
    assert_eq!(e.total_s, 0.0);
    assert_eq!(e.blocks.len(), 1);

    let with_zero = job(vec![block(
        6000.0,
        vec![
            cut(0.0, 0.0),
            cut(50.0, 0.0),
            cut(50.0, 0.0),
            cut(100.0, 0.0),
            cut(100.0, 0.0),
        ],
    )]);
    close(estimate(&with_zero, &pr).total_s, 1.1);

    let nan = job(vec![block(
        6000.0,
        vec![cut(f64::NAN, 0.0), cut(10.0, 0.0)],
    )]);
    assert!(estimate(&nan, &pr).total_s.is_finite());
}

#[test]
fn invalid_speed_and_profile_stay_finite() {
    let tp = job(vec![block(0.0, vec![cut(10.0, 0.0)])]);
    assert!(estimate(&tp, &profile()).total_s.is_finite());
    let bad = MachineProfile {
        accel_mm_s2: [0.0, f64::NAN],
        max_rate_mm_min: [-1.0, f64::INFINITY],
        rapid_rate_mm_min: 0.0,
        junction_deviation_mm: f64::NAN,
        planner_blocks: Some(0),
    };
    let tp = job(vec![block(6000.0, vec![cut(10.0, 0.0), cut(10.0, 10.0)])]);
    let t = estimate(&tp, &bad).total_s;
    assert!(t.is_finite() && t > 0.0);
}

#[test]
fn estimate_from_counts_the_approach() {
    let tp = job(vec![block(6000.0, vec![travel(100.0, 0.0)])]);
    let pr = profile();
    assert!(estimate(&tp, &pr).total_s > 0.0);
    assert_eq!(estimate_from(p(100.0, 0.0), &tp, &pr).total_s, 0.0);
}

#[test]
fn sample_follows_the_path_and_matches_estimate() {
    let tp = job(vec![block(
        6000.0,
        vec![travel(0.0, 0.0), cut(100.0, 0.0), cut(100.0, 20.0)],
    )]);
    let pr = profile();
    let total = estimate(&tp, &pr).total_s;
    let pts: Vec<_> = sample(&tp, &pr, 0.01).collect();
    let (t_last, p_last, on_last) = *pts.last().unwrap();
    close(t_last, total);
    assert_eq!(p_last, p(100.0, 20.0));
    assert!(!on_last);
    assert_eq!(pts[0].1, p(0.0, 0.0));
    // Tiempos crecientes, posición continua y sin saltos mayores a v_max·dt.
    for w in pts.windows(2) {
        assert!(w[1].0 > w[0].0);
        assert!(w[0].1.dist(w[1].1) <= 100.0 * 0.01 * (1.0 + 1e-6) + 1e-9);
    }
    assert!(pts.iter().any(|&(_, _, on)| on));
    // A mitad del primer tramo recto la x está entre 0 y 100.
    let mid = pts.iter().find(|&&(t, _, _)| t >= 0.5).unwrap();
    assert!(mid.1.x > 0.0 && mid.1.x < 100.0);
}

#[test]
fn sample_edge_cases() {
    let pr = profile();
    let tp = job(vec![block(6000.0, vec![cut(10.0, 0.0)])]);
    assert_eq!(sample(&tp, &pr, 0.0).count(), 0);
    assert_eq!(sample(&tp, &pr, f64::NAN).count(), 0);
    assert_eq!(sample(&Toolpath::default(), &pr, 0.01).count(), 0);
    // dt enorme: solo el inicio y el final.
    assert_eq!(sample(&tp, &pr, 1e6).count(), 2);
    // Potencia 0 (overscan): láser apagado.
    let dark = job(vec![block(
        6000.0,
        vec![Move::Cut {
            to: p(10.0, 0.0),
            power_pct: 0.0,
        }],
    )]);
    assert!(sample(&dark, &pr, 0.01).all(|(_, _, on)| !on));
}

#[test]
fn move_times_are_cumulative_and_end_at_the_total() {
    let tp = job(vec![
        block(
            6000.0,
            vec![travel(0.0, 0.0), cut(100.0, 0.0), cut(100.0, 0.0)],
        ),
        block(6000.0, vec![cut(100.0, 50.0)]),
    ]);
    let t = move_times(p(0.0, 0.0), &tp, &profile());
    assert_eq!(t.len(), 4);
    assert!(t.windows(2).all(|w| w[0] <= w[1]));
    // El movimiento de longitud cero repite el instante anterior.
    close(t[1], t[2]);
    close(*t.last().unwrap(), estimate(&tp, &profile()).total_s);
    assert!(move_times(p(0.0, 0.0), &job(vec![]), &profile()).is_empty());
}
