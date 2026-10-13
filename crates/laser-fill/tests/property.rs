//! Propiedad: todo tramo con potencia > 0 cae dentro del área según la regla elegida, y las
//! zonas interiores quedan cubiertas. Polígonos pseudoaleatorios con un LCG sin dependencias.

mod common;

use common::*;
use laser_core::toolpath::Move;
use laser_core::{Point, Polyline};
use laser_fill::{FillRule, FillSettings, Grouping, fill};

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
}

fn random_polys(rng: &mut Lcg) -> Vec<Polyline> {
    let count = 1 + (rng.next() * 6.0) as usize;
    (0..count)
        .map(|_| {
            let n = 3 + (rng.next() * 8.0) as usize;
            Polyline {
                points: (0..n)
                    .map(|_| Point::new(rng.range(-20.0, 20.0), rng.range(-20.0, 20.0)))
                    .collect(),
                closed: true,
            }
        })
        .collect()
}

#[test]
fn burned_midpoints_are_inside() {
    let mut rng = Lcg(0x5eed);
    let mut checked = 0usize;
    for case in 0..300 {
        let polys = random_polys(&mut rng);
        let angle = if case % 3 == 0 {
            0.0
        } else {
            rng.range(0.0, 360.0)
        };
        for rule in [FillRule::EvenOdd, FillRule::NonZero] {
            for grouping in [Grouping::All, Grouping::PerShape] {
                let s = FillSettings {
                    interval_mm: rng.range(0.4, 1.5),
                    angle_deg: angle,
                    bidirectional: case % 2 == 0,
                    overscan_mm: rng.range(0.0, 3.0),
                    crosshatch: case % 5 == 0,
                    fill_rule: rule,
                    grouping,
                    ..Default::default()
                };
                let r = fill(&polys, &s).unwrap();
                for (a, b, _) in burns(&r.block) {
                    if a.dist(b) < 1e-6 {
                        continue;
                    }
                    let mid = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
                    assert!(
                        is_inside(mid, &polys, rule == FillRule::NonZero),
                        "caso {case} {rule:?} {grouping:?}: punto medio {mid:?} fuera"
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 10_000, "{checked}");
}

/// Recíproco: los puntos interiores sobre una línea de barrido quedan cubiertos
/// (ángulo 0, donde las líneas están en y = (k + 0.5) * intervalo).
#[test]
fn interior_points_on_scanlines_are_covered() {
    let mut rng = Lcg(42);
    for case in 0..150 {
        let polys = random_polys(&mut rng);
        let interval = rng.range(0.5, 2.0);
        for rule in [FillRule::EvenOdd, FillRule::NonZero] {
            let s = FillSettings {
                interval_mm: interval,
                fill_rule: rule,
                ..Default::default()
            };
            let r = fill(&polys, &s).unwrap();
            let spans: Vec<(f64, f64, f64)> = burns(&r.block)
                .iter()
                .map(|(a, b, _)| (a.y, a.x.min(b.x), a.x.max(b.x)))
                .collect();
            for _ in 0..200 {
                let k = (rng.range(-20.0, 20.0) / interval).floor();
                let y = (k + 0.5) * interval;
                let p = Point::new(rng.range(-20.0, 20.0), y);
                let covered = spans.iter().any(|&(sy, x0, x1)| {
                    (sy - y).abs() < 1e-9 && p.x >= x0 - 1e-6 && p.x <= x1 + 1e-6
                });
                let inside = is_inside(p, &polys, rule == FillRule::NonZero);
                if inside != covered {
                    // Solo se tolera en el borde (a menos de 1e-6 de un tramo).
                    let near_edge = spans.iter().any(|&(sy, x0, x1)| {
                        (sy - y).abs() < 1e-9
                            && ((p.x - x0).abs() < 1e-6 || (p.x - x1).abs() < 1e-6)
                    }) || !inside && !covered;
                    assert!(
                        near_edge,
                        "caso {case} {rule:?}: {p:?} dentro={inside} cubierto={covered}"
                    );
                }
            }
        }
    }
}

#[test]
fn every_line_starts_with_travel_and_moves_are_finite() {
    let mut rng = Lcg(7);
    for _ in 0..100 {
        let polys = random_polys(&mut rng);
        let s = FillSettings {
            interval_mm: 0.7,
            angle_deg: rng.range(0.0, 360.0),
            overscan_mm: 1.5,
            crosshatch: true,
            ..Default::default()
        };
        let r = fill(&polys, &s).unwrap();
        if let Some(first) = r.block.moves.first() {
            assert!(matches!(first, Move::Travel { .. }));
        }
        assert!(
            r.block
                .moves
                .iter()
                .all(|m| m.to().x.is_finite() && m.to().y.is_finite())
        );
    }
}
