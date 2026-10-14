//! Generadores con semilla fija para las pruebas que comparan la versión rápida con la lenta.

use tlaku_core::{Point, Polyline};

use crate::PlanItem;

pub(crate) struct Lcg(pub u64);

impl Lcg {
    pub fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn below(&mut self, n: usize) -> usize {
        ((self.next() * n as f64) as usize).min(n - 1)
    }
}

/// Polígono estrellado (no convexo) de `n` vértices alrededor de `c`.
pub(crate) fn star(r: &mut Lcg, c: Point, rad: f64, n: usize) -> Polyline {
    let phase = r.next() * std::f64::consts::TAU;
    let points = (0..n)
        .map(|k| {
            let a = phase + k as f64 / n as f64 * std::f64::consts::TAU;
            let rr = rad * (0.55 + 0.45 * r.next());
            Point::new(c.x + rr * a.cos(), c.y + rr * a.sin())
        })
        .collect();
    Polyline {
        points,
        closed: true,
    }
}

/// Figuras variadas: cerradas con pocos y muchos vértices, anidadas, abiertas, repetidas y
/// alguna sobre una rejilla entera (empates de distancia).
pub(crate) fn random_items(seed: u64, n: usize, layers: usize) -> Vec<PlanItem> {
    let mut r = Lcg(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut items: Vec<PlanItem> = Vec::with_capacity(n);
    for i in 0..n {
        let kind = r.below(10);
        let path = match kind {
            // Contenida en una cerrada anterior (si hay).
            0..=2 if !items.is_empty() => {
                let base = &items[r.below(items.len())].path;
                let (cx, cy) = base
                    .points
                    .iter()
                    .fold((0.0, 0.0), |a, p| (a.0 + p.x, a.1 + p.y));
                let k = base.points.len() as f64;
                let c = Point::new(cx / k, cy / k);
                let rad = 0.5 + 3.0 * r.next();
                if r.below(3) == 0 {
                    Polyline {
                        points: vec![c, Point::new(c.x + rad, c.y + rad * 0.3)],
                        closed: false,
                    }
                } else {
                    let n = 3 + r.below(70);
                    star(&mut r, c, rad, n)
                }
            }
            3..=5 => {
                let c = Point::new(r.next() * 100.0, r.next() * 100.0);
                let (rad, n) = (2.0 + 25.0 * r.next(), 3 + r.below(90));
                star(&mut r, c, rad, n)
            }
            6 => {
                // Cuadrado sobre rejilla entera.
                let (x, y) = (r.below(20) as f64 * 5.0, r.below(20) as f64 * 5.0);
                let s = (1 + r.below(4)) as f64 * 5.0;
                Polyline {
                    points: vec![
                        Point::new(x, y),
                        Point::new(x + s, y),
                        Point::new(x + s, y + s),
                        Point::new(x, y + s),
                    ],
                    closed: true,
                }
            }
            7 => {
                let a = Point::new(r.next() * 100.0, r.next() * 100.0);
                let b = Point::new(a.x + r.next() * 20.0 - 10.0, a.y + r.next() * 20.0 - 10.0);
                Polyline {
                    points: vec![a, b],
                    closed: false,
                }
            }
            8 => {
                let k = 3 + r.below(5);
                let (x, y) = (r.next() * 100.0, r.next() * 100.0);
                Polyline {
                    points: (0..k)
                        .map(|j| Point::new(x + j as f64 * 2.0, y + r.next() * 6.0))
                        .collect(),
                    closed: false,
                }
            }
            _ => {
                // Copia exacta de una anterior.
                if items.is_empty() {
                    Polyline {
                        points: vec![Point::new(1.0, 1.0), Point::new(2.0, 2.0)],
                        closed: false,
                    }
                } else {
                    items[r.below(items.len())].path.clone()
                }
            }
        };
        items.push(PlanItem {
            path,
            layer_order: r.below(layers.max(1)),
            group: None,
            priority: 0,
            locked: r.below(20) == 0,
        });
        let _ = i;
    }
    items
}
