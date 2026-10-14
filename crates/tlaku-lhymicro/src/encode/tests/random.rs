//! Pruebas aleatorias de ida y vuelta y diagonales.

use super::*;

/// Generador congruencial para pruebas deterministas (sin dependencias).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) as f64) / ((1u64 << 31) as f64)
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
}

#[test]
fn random_toolpaths_round_trip_through_the_decoder() {
    let mut rng = Lcg(0x5EED);
    for case in 0..40 {
        // Trayectorias con tramos largos, cortos, diagonales y viajes de todos los tamaños.
        let mut moves = Vec::new();
        let mut cur = Point::new(rng.range(0.0, 300.0), rng.range(0.0, 200.0));
        moves.push(Move::Travel { to: cur });
        let mut cuts: Vec<((f64, f64), (f64, f64))> = Vec::new();
        for _ in 0..(5 + (rng.next() * 25.0) as usize) {
            let kind = rng.next();
            let next = if kind < 0.3 {
                // Tramo corto.
                Point::new(
                    (cur.x + rng.range(-1.5, 1.5)).clamp(0.0, 300.0),
                    (cur.y + rng.range(-1.5, 1.5)).clamp(0.0, 200.0),
                )
            } else if kind < 0.45 {
                // Diagonal exacta.
                let d = rng.range(-20.0, 20.0);
                Point::new((cur.x + d).clamp(0.0, 300.0), (cur.y + d).clamp(0.0, 200.0))
            } else {
                Point::new(rng.range(0.0, 300.0), rng.range(0.0, 200.0))
            };
            if rng.next() < 0.7 {
                let power = if rng.next() < 0.15 { 0.0 } else { 40.0 };
                moves.push(Move::Cut {
                    to: next,
                    power_pct: power,
                });
                if power > 0.0 {
                    cuts.push(((cur.x, 200.0 - cur.y), (next.x, 200.0 - next.y)));
                }
            } else {
                moves.push(Move::Travel { to: next });
            }
            cur = next;
        }
        let speed = rng.range(300.0, 6000.0);
        let path = tp(vec![block(speed, moves)]);
        let p = encode(&path, &cfg()).unwrap();
        // El flujo siempre se puede partir en paquetes válidos.
        let frames = p.frames().unwrap_or_else(|e| panic!("caso {case}: {e}"));
        assert!(
            frames
                .iter()
                .all(|f| f.payload.iter().all(|&b| b.is_ascii()))
        );
        let ev = decode_stream(&p.stream);
        // Posición final.
        let last = ev.iter().rev().find_map(|e| match e {
            Event::Move { to, .. } => Some(*to),
            _ => None,
        });
        if p.end_mil != p.start_mil {
            assert_eq!(last, Some(p.end_mil), "caso {case}");
        }
        // Ningún salto rápido enciende el láser; todo lo quemado está sobre un corte.
        let mut burned = 0.0;
        for e in &ev {
            if let Event::Move {
                from,
                to,
                laser,
                mode,
            } = e
            {
                if *mode == MoveMode::Rapid {
                    assert!(!laser, "caso {case}: salto rápido con láser");
                }
                if *laser {
                    let (f, t) = (
                        (mil_to_mm(from.0), mil_to_mm(from.1)),
                        (mil_to_mm(to.0), mil_to_mm(to.1)),
                    );
                    burned += ((t.0 - f.0).powi(2) + (t.1 - f.1).powi(2)).sqrt();
                    // Las rectas Bresenham pueden separarse hasta 1 mil de la ideal.
                    for q in [f, t, ((f.0 + t.0) / 2.0, (f.1 + t.1) / 2.0)] {
                        let d = cuts
                            .iter()
                            .map(|&(a, b)| dist_point_segment(q, a, b))
                            .fold(f64::INFINITY, f64::min);
                        assert!(d < 0.08, "caso {case}: quemó a {d} mm de un corte en {q:?}");
                    }
                }
            }
        }
        // Los extremos de cada corte los visita el cabezal.
        let ends: Vec<(f64, f64)> = ev
            .iter()
            .filter_map(|e| match e {
                Event::Move { from, to, .. } => Some([
                    (mil_to_mm(from.0), mil_to_mm(from.1)),
                    (mil_to_mm(to.0), mil_to_mm(to.1)),
                ]),
                _ => None,
            })
            .flatten()
            .collect();
        for &(a, b) in &cuts {
            for q in [a, b] {
                let d = ends
                    .iter()
                    .map(|e| ((e.0 - q.0).powi(2) + (e.1 - q.1).powi(2)).sqrt())
                    .fold(f64::INFINITY, f64::min);
                assert!(d < 0.05, "caso {case}: extremo {q:?} a {d} mm");
            }
        }
        // La longitud ideal que informa la estadística coincide con la de los cortes.
        let want: f64 = cuts
            .iter()
            .map(|&(a, b)| ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt())
            .sum();
        let tol = 0.04 * cuts.len() as f64 + 0.2;
        assert!(
            (p.stats.burn_mm - want).abs() <= tol,
            "caso {case}: {} vs {want}",
            p.stats.burn_mm
        );
        // La trayectoria real (con diagonales) no es más de ~42 % más larga que la recta.
        assert!(
            burned <= want * 1.42 + tol,
            "caso {case}: {burned} vs {want}"
        );
        assert!(
            burned + tol >= want * 0.99,
            "caso {case}: {burned} vs {want}"
        );
    }
}

#[test]
fn diagonal_cuts_use_the_m_command() {
    let p = encode(
        &tp(vec![block(
            1200.0,
            vec![travel(10.0, 190.0), cut(20.0, 180.0)],
        )]),
        &cfg(),
    )
    .unwrap();
    assert!(s(&p).contains("M"), "{}", s(&p));
    let ev = decode_stream(&p.stream);
    let end = ev
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::Move { to, .. } => Some(*to),
            _ => None,
        })
        .unwrap();
    assert_eq!(end, p.end_mil);
}
