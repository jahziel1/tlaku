//! El emulador frena y acelera como GRBL: su duración en casos con cinemática calculable a mano
//! coincide con la fórmula y con el estimador (`tlaku-sim`) dentro de ±5 %.
//!
//! Se manda el G-code directamente a la `Machine` (sin hilos ni relojes) manteniendo el búfer RX
//! lleno, como hace un anfitrión con conteo de caracteres, y se mide el tiempo simulado hasta
//! que vuelve a `Idle`.

use std::time::Duration;

use tlaku_core::Point;
use tlaku_core::toolpath::{Move, PowerMode, Toolpath, ToolpathBlock};
use tlaku_emu::{Machine, Options, Profile, StateKind};
use tlaku_sim::MachineProfile;

/// Segundos simulados que tarda `profile` en ejecutar `lines`, con 12 000 mm/min y
/// 1 000 mm/s² por eje y `$11 = 0,01`, y la cabeza ya referenciada en el origen.
fn run_time(profile: Profile, lines: &[String]) -> f64 {
    let opts = Options {
        banner_on_connect: false,
        homed: true,
        ..Options::default()
    };
    let mut m = Machine::new(profile, opts);
    for (id, v) in [
        (110, 12_000.0),
        (111, 12_000.0),
        (120, 1_000.0),
        (121, 1_000.0),
        (11, 0.01),
        (32, 1.0),
    ] {
        m.set_setting(id, v);
    }
    let usable = m.profile().rx_size - 1;
    let mut next = 0;
    let mut scratch = [0u8; 4096];
    loop {
        while next < lines.len() && m.in_flight() + lines[next].len() < usable {
            m.receive(format!("{}\n", lines[next]).as_bytes());
            next += 1;
        }
        let t = m.now() + Duration::from_millis(1);
        m.advance_to(t);
        while m.take_output(&mut scratch) > 0 {}
        if next == lines.len() && m.in_flight() == 0 && m.state() == StateKind::Idle {
            return m.now().as_secs_f64();
        }
        assert!(m.now() < Duration::from_secs(3600), "no termina");
    }
}

fn header() -> Vec<String> {
    ["G90", "G21", "M4 S0"].map(String::from).to_vec()
}

fn fmt(x: f64) -> String {
    format!("{x:.5}")
}

/// Trabajo del estimador con el mismo recorrido: un bloque con los cortes dados.
fn sim_time(feed: f64, start: (f64, f64), points: &[(f64, f64)]) -> f64 {
    sim_time_window(feed, start, points, 15)
}

fn sim_time_window(feed: f64, start: (f64, f64), points: &[(f64, f64)], window: usize) -> f64 {
    let mut moves = vec![Move::Travel {
        to: Point::new(start.0, start.1),
    }];
    moves.extend(points.iter().map(|&(x, y)| Move::Cut {
        to: Point::new(x, y),
        power_pct: 50.0,
    }));
    let tp = Toolpath {
        blocks: vec![ToolpathBlock {
            label: "prueba".into(),
            speed_mm_min: feed,
            air_assist: false,
            power_mode: PowerMode::Dynamic,
            moves,
            z_mm: None,
            galvo: None,
        }],
    };
    let profile = MachineProfile {
        planner_blocks: Some(window),
        ..MachineProfile::default()
    };
    tlaku_sim::estimate(&tp, &profile).total_s
}

fn within(got: f64, want: f64, tol: f64, what: &str) {
    let err = (got - want) / want;
    println!(
        "{what}: emulador {got:.3} s, esperado {want:.3} s ({:+.2} %)",
        err * 100.0
    );
    assert!(err.abs() <= tol, "{what}: {got} contra {want} ({err:+.4})");
}

/// Vaivén de 1 mm x 400: GRBL llega a 0 en cada giro de 180 grados, así que cada tramo es un
/// triángulo de aceleración y dura `2 * sqrt(d / a)`.
fn reversal() -> (Vec<String>, Vec<(f64, f64)>) {
    let mut lines = header();
    lines.push("G0 X20 Y20".into());
    let mut pts = Vec::new();
    for i in 0..400 {
        let x = if i % 2 == 0 { 21.0 } else { 20.0 };
        lines.push(format!("G1 X{x} Y20 S500 F6000"));
        pts.push((x, 20.0));
    }
    (lines, pts)
}

/// Zigzag de giros de 30 grados con segmentos de 0,157 mm a 860 mm/min.
fn zigzag(n: usize) -> (Vec<String>, Vec<(f64, f64)>) {
    let mut lines = header();
    lines.push("G0 X10 Y10".into());
    let (mut x, mut y) = (10.0f64, 10.0f64);
    let mut pts = Vec::new();
    for i in 0..n {
        let a = (if i % 2 == 0 { 15.0f64 } else { -15.0 }).to_radians();
        x += 0.157 * a.cos();
        y += 0.157 * a.sin();
        lines.push(format!("G1 X{} Y{} S500 F860", fmt(x), fmt(y)));
        pts.push((x, y));
    }
    (lines, pts)
}

#[test]
fn reversing_one_mm_cuts_take_the_rest_to_rest_time() {
    let (lines, pts) = reversal();
    let t = run_time(Profile::grbl_1_1h(), &lines);
    // Aproximación G0 de (0,0) a (20,20): 28,28 mm, triangulo hasta 200*sqrt(2)? no: se detiene
    // por la unión de 135 grados, y el estimador la cuenta igual.
    let sim = sim_time(6000.0, (20.0, 20.0), &pts);
    within(t, sim, 0.05, "vaivén contra estimador");
    let approach = sim_time(6000.0, (20.0, 20.0), &[]);
    let hand = 400.0 * 2.0 * (1.0f64 / 1000.0).sqrt();
    within(t - approach, hand, 0.03, "vaivén contra fórmula");
}

#[test]
fn thirty_degree_zigzag_does_not_slow_a_slow_cut() {
    let (lines, pts) = zigzag(1500);
    let t = run_time(Profile::grbl_1_1h(), &lines);
    let sim = sim_time(860.0, (10.0, 10.0), &pts);
    within(t, sim, 0.05, "zigzag contra estimador");
    let approach = sim_time(860.0, (10.0, 10.0), &[]);
    let v = 860.0 / 60.0;
    let hand = 1500.0 * 0.157 / v + v / 1000.0;
    within(t - approach, hand, 0.03, "zigzag contra fórmula");
}

/// Segmentos de 0,1 mm en línea recta a 6 000 mm/min: con 15 bloques la cabeza solo ve 1,4 mm
/// por delante y no puede pasar de sqrt(2 a d); con una ventana de 127 sí llega a 100 mm/s.
#[test]
fn the_lookahead_window_comes_from_the_profile() {
    let mut lines = header();
    lines.push("G0 X10 Y10".into());
    let mut pts = Vec::new();
    for i in 1..=2000 {
        let x = 10.0 + 0.1 * f64::from(i);
        lines.push(format!("G1 X{} Y10 S500 F6000", fmt(x)));
        pts.push((x, 10.0));
    }
    let short = run_time(Profile::grbl_1_1h(), &lines);
    within(
        short,
        sim_time(6000.0, (10.0, 10.0), &pts),
        0.05,
        "ventana de 15 contra estimador",
    );
    let mut wide = Profile::grbl_1_1h();
    wide.planner_blocks = 127;
    let long = run_time(wide.clone(), &lines);
    within(
        long,
        sim_time_window(6000.0, (10.0, 10.0), &pts, 127),
        0.05,
        "ventana de 127 contra estimador",
    );
    assert!(long < short * 0.9, "{long} contra {short}");
    // Y una ventana propia, distinta del búfer, también se respeta.
    wide.lookahead_blocks = Some(15);
    let narrowed = run_time(wide, &lines);
    assert!(
        (narrowed - short).abs() / short < 0.03,
        "{narrowed} contra {short}"
    );
}

/// En diagonal manda el eje que primero llega a su tope (como en GRBL): 100 mm a 45 grados con
/// 12 000 mm/min por eje y 1 000 mm/s2 por eje se recorren a 200 * sqrt(2) mm/s con
/// a = 1 000 * sqrt(2), no a 200 mm/s con a = 1 000.
#[test]
fn a_diagonal_uses_the_per_axis_limits_projected_on_the_direction() {
    let mut lines = header();
    lines.push("G0 X10 Y10".into());
    // La pausa vacía el planificador: el G1 parte del reposo.
    lines.push("G4 P0.01".into());
    lines.push("G1 X110 Y110 S500 F30000".into());
    let t = run_time(Profile::grbl_1_1h(), &lines);
    let approach = run_time(Profile::grbl_1_1h(), &lines[..5]);
    let (v, a) = (200.0 * 2f64.sqrt(), 1000.0 * 2f64.sqrt());
    let hand = 100.0 * 2f64.sqrt() / v + v / a;
    within(t - approach, hand, 0.03, "diagonal contra fórmula");
    let solo = sim_time(30_000.0, (0.0, 0.0), &[(100.0, 100.0)]);
    within(t - approach, solo, 0.03, "diagonal contra estimador");
}
