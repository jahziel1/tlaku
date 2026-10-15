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

/// Dos posiciones del orden: casi siempre cercanas (ajuste fino: lo que se cambia al final del
/// orden altera poco lo ya colocado) y a veces cualquiera.
fn pair(n: usize, rng: &mut Rng) -> (usize, usize) {
    let i = rng.below(n);
    if rng.unit() < 0.3 {
        return (i, rng.below(n));
    }
    let reach = (n / 8).max(2);
    let lo = i.saturating_sub(reach);
    let hi = (i + reach).min(n - 1);
    (i, lo + rng.below(hi - lo + 1))
}

fn mutate(order: &mut Vec<Gene>, allowed: &[Vec<usize>], allow_mirror: bool, rng: &mut Rng) {
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
                let opts = &allowed[order[k].piece];
                order[k].rot = if opts.len() > 1 && rng.unit() < 0.8 {
                    Some(opts[rng.below(opts.len())])
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
                let (i, j) = pair(n, rng);
                order.swap(i, j);
            }
            _ => {
                let (i, j) = pair(n, rng);
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
    let mut best_order = cur_order.clone();
    let t0 = (cur.cost * 0.01).max(1.0);
    let mut since_improve = 0u64;
    let mut since_restart = 0u64;
    let mut improved_at = evaluations;
    // Mínimo de evaluaciones sin mejora para dar la búsqueda por terminada.
    let min_stall = (200 + 4 * n) as u64;
    let allow_mirror = engine.allow_mirror;

    let interrupted = loop {
        if ctl.should_stop() {
            break true;
        }
        if limits.max_evaluations.is_some_and(|m| evaluations >= m) {
            break false;
        }
        // Termina por estancamiento solo si ya se probó el doble de lo que costó encontrar la
        // mejor y no mejora: así una búsqueda que sigue progresando usa todo su tiempo.
        if best.unplaced.is_empty() && since_improve > min_stall.max(improved_at) {
            break false;
        }
        // A temperatura alta la solución actual se aleja de la mejor: se vuelve a ella de vez en
        // cuando para explorar su vecindad.
        if since_restart > (40 + n as u64) {
            cur = best.clone();
            cur_order = best_order.clone();
            since_restart = 0;
        }
        let mut cand_order = cur_order.clone();
        mutate(
            &mut cand_order,
            &engine.piece_angles,
            allow_mirror,
            &mut rng,
        );
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
        since_restart += 1;
        if improved {
            best_order = cand_order.clone();
            best = cand.clone();
            since_improve = 0;
            since_restart = 0;
            improved_at = evaluations;
        } else {
            since_improve += 1;
        }
        if accept {
            cur = cand;
            cur_order = cand_order;
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
