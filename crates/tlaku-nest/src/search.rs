//! Búsqueda sobre el orden y las rotaciones de las piezas: recocido simulado con tope de
//! tiempo y de evaluaciones. La primera solución es la voraz con las piezas de mayor a menor
//! área y probando varias rotaciones en cada una; desde ahí se mutan el orden (intercambiar,
//! mover) y las rotaciones o espejos, y se acepta lo mejor o, con probabilidad decreciente,
//! algo un poco peor.

use crate::engine::{Ctl, Engine, Gene, Solution};
use std::time::Instant;

/// Generador pseudoaleatorio SplitMix64: pequeño, rápido y reproducible con la semilla.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Entero en `0..n` (n > 0).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    /// Real en `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Qué ve quien observa la búsqueda.
pub struct Step<'a> {
    pub solution: &'a Solution,
    pub evaluations: u64,
    pub improved: bool,
}

pub struct Limits {
    pub max_evaluations: Option<u64>,
    pub started: Instant,
    pub time_limit_s: f64,
}

pub struct Found {
    pub solution: Solution,
    pub evaluations: u64,
    /// Terminó porque se agotó el tiempo o se pidió parar (no por llegar al tope de evaluaciones
    /// ni por estancamiento).
    pub interrupted: bool,
}

fn mutate(order: &mut Vec<Gene>, n_angles: usize, allow_mirror: bool, rng: &mut Rng) {
    let n = order.len();
    if n == 0 {
        return;
    }
    let ops = if n >= 2 { 4 } else { 2 };
    let times = if rng.unit() < 0.7 { 1 } else { 2 };
    for _ in 0..times {
        match rng.below(ops) {
            0 => {
                let k = rng.below(n);
                order[k].rot = if n_angles > 1 && rng.unit() < 0.8 {
                    Some(rng.below(n_angles))
                } else {
                    None
                };
            }
            1 => {
                let k = rng.below(n);
                if allow_mirror {
                    order[k].mirror = !order[k].mirror;
                } else {
                    order[k].rot = None;
                }
            }
            2 => {
                let (i, j) = (rng.below(n), rng.below(n));
                order.swap(i, j);
            }
            _ => {
                let (i, j) = (rng.below(n), rng.below(n));
                let g = order.remove(i);
                order.insert(j.min(order.len()), g);
            }
        }
    }
}

pub fn search(
    engine: &mut Engine,
    start: Vec<Gene>,
    ctl: &Ctl,
    limits: &Limits,
    seed: u64,
    observer: &mut dyn FnMut(&Engine, &Step),
) -> Found {
    let n = start.len();
    let mut evaluations = 1u64;
    let first = engine.greedy(&start, ctl);
    let mut best = first.clone();
    observer(
        engine,
        &Step {
            solution: &best,
            evaluations,
            improved: true,
        },
    );
    if !first.complete {
        return Found {
            solution: best,
            evaluations,
            interrupted: true,
        };
    }

    let mut rng = Rng::new(seed);
    let mut cur = first;
    let mut cur_order = start;
    let t0 = (cur.cost * 0.01).max(1.0);
    let mut since_improve = 0u64;
    let stagnation = 60 + 2 * n as u64;
    let n_angles = engine.angles.len();
    let allow_mirror = engine.allow_mirror;

    let interrupted = loop {
        if ctl.should_stop() {
            break true;
        }
        if limits.max_evaluations.is_some_and(|m| evaluations >= m) {
            break false;
        }
        if best.unplaced.is_empty() && since_improve > stagnation {
            break false;
        }
        let mut cand_order = cur_order.clone();
        mutate(&mut cand_order, n_angles, allow_mirror, &mut rng);
        let cand = engine.greedy(&cand_order, ctl);
        if !cand.complete {
            break true;
        }
        evaluations += 1;
        let progress = match limits.max_evaluations {
            Some(m) => evaluations as f64 / m as f64,
            None => limits.started.elapsed().as_secs_f64() / limits.time_limit_s.max(1e-3),
        };
        let temp = t0 * (1.0 - progress).clamp(0.0, 1.0);
        let delta = cand.cost - cur.cost;
        let accept = delta <= 0.0 || (temp > 1e-9 && rng.unit() < (-delta / temp).exp());
        let improved = cand.cost < best.cost - 1e-9;
        if accept {
            cur = cand.clone();
            cur_order = cand_order.clone();
        }
        if improved {
            best = cand;
            since_improve = 0;
        } else {
            since_improve += 1;
        }
        observer(
            engine,
            &Step {
                solution: &best,
                evaluations,
                improved,
            },
        );
    };
    Found {
        solution: best,
        evaluations,
        interrupted,
    }
}
