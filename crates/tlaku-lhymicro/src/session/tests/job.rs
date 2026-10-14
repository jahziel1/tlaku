//! Pruebas de sondeo y de trabajos completos.

use super::*;
use crate::status::Status;

#[test]
fn probe_reads_only_the_status() {
    let (mut s, b) = session(EmuConfig::default());
    let info = s.probe().unwrap();
    assert!(info.ready && info.notes.is_empty());
    assert_eq!(info.status.status, Status::Ok);
    assert_eq!(info.transport.chip_version, Some(0x30));
    assert_eq!(board(&b).accepted, 0, "el sondeo no escribe paquetes");
}

#[test]
fn a_moshiboard_is_detected_and_rejected() {
    let (mut s, _b) = session(EmuConfig {
        moshiboard: true,
        ..Default::default()
    });
    match s.probe() {
        Err(SessionError::NotM2Nano { code: 205 }) => {}
        other => panic!("{other:?}"),
    }
    let e = s.probe().unwrap_err();
    assert!(e.to_string().contains("Moshiboard"));
}

#[test]
fn jobs_need_a_known_position() {
    let (mut s, _b) = session(EmuConfig::default());
    assert!(matches!(
        s.encode(&square(10.0, 150.0, 20.0, 1200.0)),
        Err(SessionError::UnknownPosition)
    ));
    assert!(matches!(
        s.jog_by(1.0, 1.0),
        Err(SessionError::UnknownPosition)
    ));
}

#[test]
fn full_job_in_the_emulator_burns_the_square_and_never_overruns() {
    let (mut s, b) = session(EmuConfig::default());
    s.probe().unwrap();
    s.assume_at_home();
    let program = s.encode(&square(10.0, 150.0, 20.0, 1200.0)).unwrap();
    let mut seen = Vec::new();
    let report = s.send_job(&program, &mut |p| seen.push(p)).unwrap();
    assert_eq!(report.packets, program.frames().unwrap().len());
    assert!(report.finish_confirmed);
    assert_eq!(report.resends, 0);
    assert_eq!(seen.len(), report.packets);
    assert!(seen.windows(2).all(|w| w[0].bytes_sent < w[1].bytes_sent));
    assert_eq!(seen.last().unwrap().packet, report.packets);
    assert!(!report.warnings.is_empty(), "potencia ignorada");
    assert!(
        report
            .warnings
            .iter()
            .any(|w| matches!(w, Warning::PowerIgnored { .. }))
    );

    let mut e = board(&b);
    e.run_to_idle();
    assert_eq!(e.overruns, 0, "nunca se escribe con el búfer lleno");
    assert!(
        e.busy_replies > 0,
        "la prueba debe haber pasado por «ocupada»"
    );
    assert_eq!(e.rejected, 0);
    assert!(!e.laser_on());
    // El láser quemó el cuadrado de 20 mm y nada más.
    assert!(
        (e.burn_length_mm() - 80.0).abs() < 0.3,
        "{}",
        e.burn_length_mm()
    );
    let (lo, hi) = e.burn_bbox_mm().unwrap();
    // En ejes de la placa (home arriba-izquierda): x 10..30, y = 200-150-20 .. 200-150.
    assert!(
        (lo.0 - 10.0).abs() < 0.1 && (hi.0 - 30.0).abs() < 0.1,
        "{lo:?} {hi:?}"
    );
    assert!(
        (lo.1 - 30.0).abs() < 0.1 && (hi.1 - 50.0).abs() < 0.1,
        "{lo:?} {hi:?}"
    );
    drop(e);
    // El cabezal terminó donde dice la sesión.
    let end = s.position_mil().unwrap();
    let head = board(&b).head_mil();
    assert!((head.0 - end.0 as f64).abs() < 1.0 && (head.1 - end.1 as f64).abs() < 1.0);
    assert_eq!(s.control().state(), JobState::Idle);
}

#[test]
fn the_emulator_trace_never_burns_during_travels() {
    // Dos cuadrados separados: el salto entre ellos y los viajes cortos no queman.
    let mut tp = square(10.0, 150.0, 10.0, 1200.0);
    let second = square(60.0, 100.0, 10.0, 1200.0);
    tp.blocks[0]
        .moves
        .extend(second.blocks[0].moves.iter().copied());
    // Un viaje corto (3 mm) también.
    tp.blocks[0].moves.push(Move::Travel {
        to: Point::new(63.0, 100.0),
    });
    tp.blocks[0].moves.push(Move::Cut {
        to: Point::new(68.0, 100.0),
        power_pct: 50.0,
    });
    let (mut s, b) = session(EmuConfig::default());
    s.assume_at_home();
    let program = s.encode(&tp).unwrap();
    s.send_job(&program, &mut no_progress()).unwrap();
    let mut e = board(&b);
    e.run_to_idle();
    let burn_toolpath = 40.0 + 40.0 + 5.0;
    assert!(
        (e.burn_length_mm() - burn_toolpath).abs() < 0.5,
        "{}",
        e.burn_length_mm()
    );
    // Cada tramo quemado está sobre algún segmento cortado del Toolpath (ejes de la placa).
    let segs: Vec<((f64, f64), (f64, f64))> = {
        let mut v = Vec::new();
        let mut cur = (0.0, 0.0);
        for m in &tp.blocks[0].moves {
            let to = m.to();
            let mm = (to.x, 200.0 - to.y);
            if let Move::Cut { power_pct, .. } = m
                && *power_pct > 0.0
            {
                v.push((cur, mm));
            }
            cur = mm;
        }
        v
    };
    for (a, bb) in e.burn_pieces_mm() {
        for p in [a, bb] {
            let d = segs
                .iter()
                .map(|&(s0, s1)| dist_point_segment(p, s0, s1))
                .fold(f64::INFINITY, f64::min);
            assert!(d < 0.06, "quemó a {d} mm del trazado en {p:?}");
        }
    }
}

fn dist_point_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let l2 = abx * abx + aby * aby;
    let t = if l2 == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * abx + (p.1 - a.1) * aby) / l2).clamp(0.0, 1.0)
    };
    ((p.0 - (a.0 + t * abx)).powi(2) + (p.1 - (a.1 + t * aby)).powi(2)).sqrt()
}
