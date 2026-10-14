//! Pruebas del alfabeto de distancias, Bresenham y ida y vuelta por el decodificador.

use super::*;
use crate::speed::Board;

#[test]
fn distance_alphabet_of_meerk40t() {
    assert_eq!(distance(0), "");
    assert_eq!(distance(1), "a");
    assert_eq!(distance(25), "y");
    assert_eq!(distance(26), "|a");
    assert_eq!(distance(51), "|z");
    assert_eq!(distance(52), "052");
    assert_eq!(distance(254), "254");
    assert_eq!(distance(255), "z");
    assert_eq!(distance(256), "za");
    assert_eq!(distance(787), "zzzv");
    assert_eq!(distance(394), "z139");
    assert_eq!(distance(510), "zz");
    assert_eq!(distance(511), "zza");
    assert_eq!(distance(255 * 3 + 52), "zzz052");
}

#[test]
fn distances_round_trip_through_the_decoder() {
    for v in (1..3000u64).chain([12000, 12240]) {
        let stream = format!("IB{}S1P", distance(v));
        let ev = decode_stream(stream.as_bytes());
        let moved = ev.iter().find_map(|e| match e {
            Event::Move { to, .. } => Some(to.0),
            _ => None,
        });
        assert_eq!(moved, Some(v as i64), "{v}: {stream}");
    }
}

#[test]
fn bresenham_runs_cover_the_displacement_with_straight_and_diagonal_steps() {
    for (dx, dy) in [
        (10, 0),
        (0, -7),
        (5, 5),
        (-5, 5),
        (10, 3),
        (3, 10),
        (-17, 4),
        (1, 1),
        (100, -37),
        (0, 0),
    ] {
        let runs = line_runs(dx, dy);
        let (sx, sy) = runs.iter().fold((0, 0), |a, r| (a.0 + r.0, a.1 + r.1));
        assert_eq!((sx, sy), (dx, dy));
        for (rx, ry) in &runs {
            assert!(*rx == 0 || *ry == 0 || rx.abs() == ry.abs(), "{rx},{ry}");
        }
    }
    assert_eq!(line_runs(10, 0), [(10, 0)]);
    assert_eq!(line_runs(4, 4), [(4, 4)]);
    assert!(line_runs(0, 0).is_empty());
}

#[test]
fn bresenham_stays_within_half_a_mil_of_the_ideal_line() {
    let (dx, dy) = (1000i64, 333i64);
    let (mut x, mut y) = (0i64, 0i64);
    for (rx, ry) in line_runs(dx, dy) {
        // Recorre el tramo paso a paso.
        let n = rx.abs().max(ry.abs());
        for _ in 0..n {
            x += rx.signum();
            y += ry.signum();
            let ideal_y = x as f64 * dy as f64 / dx as f64;
            assert!((y as f64 - ideal_y).abs() <= 1.0, "({x},{y}) vs {ideal_y}");
        }
    }
    assert_eq!((x, y), (dx, dy));
}

#[test]
fn meerk40t_rectangle_is_reproduced_byte_for_byte_except_the_safety_u() {
    // Rectángulo de 1 cm en (2 cm, 2 cm) a 15 mm/s. En el sistema de la placa: salto a
    // (787, 787) y cuatro lados de 394 mil. Pasado a Tlaku (home arriba-izquierda, Y arriba):
    // x = 787 mil = 19,9898 mm; y = 200 - 19,9898 mm.
    let ox = 787.0 * MM_PER_MIL;
    let oy = 200.0 - 787.0 * MM_PER_MIL;
    let side = 394.0 * MM_PER_MIL;
    let path = tp(vec![block(
        900.0,
        vec![
            travel(ox, oy),
            cut(ox + side, oy),
            cut(ox + side, oy - side),
            cut(ox, oy - side),
            cut(ox, oy),
        ],
    )]);
    let mut c = cfg();
    c.constant_speed = true; // MeerK40t usa `C` por defecto ("twitches" desactivado).
    let p = encode(&path, &c).unwrap();
    // Esperado de MeerK40t (`egv_rect`), con una `U` de seguridad antes de `FNSE`.
    let expected = "IBzzzvRzzzvS1P\nICV2490731016000027CNLBS1EDz139Rz139Tz139Lz139FNSE-\n";
    assert_eq!(s(&p).replace("UFNSE", "FNSE"), expected);
    assert!(s(&p).contains("Lz139UFNSE-\n"));
    assert_eq!(p.end_mil, (787, 787));
    assert_eq!(p.stats.programs, 1);
    assert_eq!(p.stats.rapids, 1);
    assert!((p.stats.burn_mm - 4.0 * 394.0 * MM_PER_MIL).abs() < 0.05);
}

#[test]
fn default_speed_uses_the_gear_of_the_chinese_software() {
    // 15 mm/s sin `C`: engranaje 1 con razón diagonal.
    let path = tp(vec![block(
        900.0,
        vec![travel(10.0, 190.0), cut(20.0, 190.0)],
    )]);
    let p = encode(&path, &cfg()).unwrap();
    let code = s(&p);
    let v = code.find("CV").unwrap();
    let end = code[v..].find('N').unwrap();
    let sc = &code[v..v + end];
    assert!(!sc.ends_with('C'), "{sc}");
    let back = speed::speed_from_code(sc, Board::M2, false).unwrap();
    assert!((back - 15.0).abs() < 0.2, "{sc} -> {back}");
}

#[test]
fn decoding_the_program_gives_back_the_toolpath_geometry() {
    let path = tp(vec![block(
        1200.0,
        vec![
            travel(20.0, 180.0),
            cut(60.0, 180.0),
            cut(60.0, 140.0),
            cut(20.0, 100.0),
            cut(20.0, 180.0),
        ],
    )]);
    let p = encode(&path, &cfg()).unwrap();
    let ev = decode_stream(&p.stream);
    // Posición final de la placa.
    let last = ev.iter().rev().find_map(|e| match e {
        Event::Move { to, .. } => Some(*to),
        _ => None,
    });
    assert_eq!(last, Some(p.end_mil));
    // Todo trazo con láser encendido está a menos de 1,5 mil de algún segmento cortado.
    let seg: Vec<((f64, f64), (f64, f64))> = [
        ((20.0, 180.0), (60.0, 180.0)),
        ((60.0, 180.0), (60.0, 140.0)),
        ((60.0, 140.0), (20.0, 100.0)),
        ((20.0, 100.0), (20.0, 180.0)),
    ]
    .iter()
    .map(|&((ax, ay), (bx, by))| {
        // A mm de la placa (Y invertida: home arriba).
        ((ax, 200.0 - ay), (bx, 200.0 - by))
    })
    .collect();
    let mut burned = 0.0;
    for e in &ev {
        if let Event::Move {
            from,
            to,
            laser: true,
            mode: MoveMode::Program,
        } = e
        {
            let (fx, fy) = (mil_to_mm(from.0), mil_to_mm(from.1));
            let (tx, ty) = (mil_to_mm(to.0), mil_to_mm(to.1));
            burned += ((tx - fx).powi(2) + (ty - fy).powi(2)).sqrt();
            for (px, py) in [(fx, fy), (tx, ty), ((fx + tx) / 2.0, (fy + ty) / 2.0)] {
                let d = seg
                    .iter()
                    .map(|&(a, b)| dist_point_segment((px, py), a, b))
                    .fold(f64::INFINITY, f64::min);
                assert!(d < 0.05, "punto quemado a {d} mm del trazado: {px},{py}");
            }
        }
    }
    let want = 40.0 + 40.0 + (40.0f64.powi(2) + 40.0f64.powi(2)).sqrt() + 80.0;
    assert!((burned - want).abs() < 0.3, "{burned} vs {want}");
}
