//! Búsqueda de candidatos a cruz por el tensor de estructura y simetría de punto.

use crate::imgproc::{FloatImage, gaussian_blur};
use tlaku_core::Point;

// ---------------------------------------------------------------------------
// Candidatos
// ---------------------------------------------------------------------------

pub(super) struct Grad {
    pub(super) gx: Vec<f32>,
    pub(super) gy: Vec<f32>,
}

impl Grad {
    pub(super) fn new(b: &FloatImage) -> Self {
        let (w, h) = (b.w, b.h);
        let mut gx = vec![0.0f32; w * h];
        let mut gy = vec![0.0f32; w * h];
        for y in 1..h.saturating_sub(1) {
            for x in 1..w.saturating_sub(1) {
                gx[y * w + x] = 0.5 * (b.get(x + 1, y) - b.get(x - 1, y));
                gy[y * w + x] = 0.5 * (b.get(x, y + 1) - b.get(x, y - 1));
            }
        }
        Self { gx, gy }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Cand {
    pub(super) p: Point,
    /// Simetría de punto, de 0 a 1.
    pub(super) sym: f64,
    pub(super) score: f64,
}

/// Correlación entre la imagen y su giro de 180 grados alrededor de `c`, en un disco de
/// radio `r`: vale cerca de 1 en una cruz, un círculo o un punto, y poco en esquinas, puntas
/// de línea y ruido. Como función de `c` tiene su máximo en el centro de la cruz.
fn point_symmetry(b: &FloatImage, c: Point, r: f64) -> f64 {
    let ri = r.ceil() as i64;
    let (mut sa, mut sb, mut saa, mut sbb, mut sab, mut n) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for j in -ri..=ri {
        for i in -ri..=ri {
            // Solo medio plano: cada par se cuenta una vez.
            if j < 0 || (j == 0 && i <= 0) {
                continue;
            }
            let (dx, dy) = (i as f64, j as f64);
            let d = dx.hypot(dy);
            if !(1.5..=r).contains(&d) {
                continue;
            }
            let a = f64::from(b.sample(c.x + dx, c.y + dy));
            let o = f64::from(b.sample(c.x - dx, c.y - dy));
            sa += a;
            sb += o;
            saa += a * a;
            sbb += o * o;
            sab += a * o;
            n += 1.0;
        }
    }
    if n < 6.0 {
        return 0.0;
    }
    let (va, vb) = (saa / n - (sa / n).powi(2), sbb / n - (sb / n).powi(2));
    if va < 1e-6 || vb < 1e-6 {
        return 0.0;
    }
    ((sab / n - sa * sb / (n * n)) / (va * vb).sqrt()).clamp(-1.0, 1.0)
}

/// Centro de simetría cerca de `start`: búsqueda en rejilla de ±2.5 px (paso 0.5) y luego
/// de ±0.5 px (paso 0.1). Devuelve el centro y su simetría.
fn symmetry_center(b: &FloatImage, start: Point, r: f64) -> (Point, f64) {
    let mut best = (start, f64::NEG_INFINITY);
    let scan = |center: Point, half: f64, step: f64, best: &mut (Point, f64)| {
        let n = (half / step).round() as i64;
        for j in -n..=n {
            for i in -n..=n {
                let p = Point::new(center.x + i as f64 * step, center.y + j as f64 * step);
                let s = point_symmetry(b, p, r);
                if s > best.1 {
                    *best = (p, s);
                }
            }
        }
    };
    scan(start, 2.5, 0.5, &mut best);
    let coarse = best.0;
    scan(coarse, 0.5, 0.1, &mut best);
    best
}

/// Candidatos con forma de cruz, ordenados de mejor a peor. `want` es cuántos se conservan.
/// Los picos del tensor de estructura (dos direcciones fuertes de gradiente en una ventana
/// pequeña) dan la posición a unos 2 px; la simetría de punto la afina y descarta lo que no
/// es una cruz (puntas de línea, esquinas del cartón, veta).
pub(super) fn find_candidates(b: &FloatImage, g: &Grad, want: usize) -> Vec<Cand> {
    let (w, h) = (b.w, b.h);
    if w < 24 || h < 24 {
        return Vec::new();
    }
    let sq = |gv: &[f32], gv2: &[f32]| FloatImage {
        w,
        h,
        data: gv.iter().zip(gv2).map(|(a, c)| a * c).collect(),
    };
    let sxx = gaussian_blur(&sq(&g.gx, &g.gx), 2.0);
    let sxy = gaussian_blur(&sq(&g.gx, &g.gy), 2.0);
    let syy = gaussian_blur(&sq(&g.gy, &g.gy), 2.0);
    let l2: Vec<f32> = (0..w * h)
        .map(|i| {
            let (a, bq, c) = (sxx.data[i], sxy.data[i], syy.data[i]);
            0.5 * (a + c) - (0.25 * (a - c) * (a - c) + bq * bq).sqrt()
        })
        .collect();
    let max = l2.iter().copied().fold(0.0f32, f32::max);
    if max <= 0.0 {
        return Vec::new();
    }
    let mut sorted = l2.clone();
    let mid = sorted.len() / 2;
    let median = *sorted.select_nth_unstable_by(mid, f32::total_cmp).1;
    let thr = (25.0 * median).max(0.003 * max);
    // Máximos locales de 7x7.
    let mut peaks: Vec<(f32, usize, usize)> = Vec::new();
    for y in 4..h - 4 {
        for x in 4..w - 4 {
            let v = l2[y * w + x];
            if v < thr {
                continue;
            }
            let mut is_max = true;
            'win: for dy in -3i64..=3 {
                for dx in -3i64..=3 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let o = l2[(y as i64 + dy) as usize * w + (x as i64 + dx) as usize];
                    // Empates: gana el primero en orden de lectura.
                    if o > v || (o == v && (dy < 0 || (dy == 0 && dx < 0))) {
                        is_max = false;
                        break 'win;
                    }
                }
            }
            if is_max {
                peaks.push((v, x, y));
            }
        }
    }
    peaks.sort_by(|a, c| c.0.total_cmp(&a.0));
    peaks.truncate(120);
    let mut cands: Vec<Cand> = Vec::new();
    for &(v, x, y) in &peaks {
        let (p, sym) = symmetry_center(b, Point::new(x as f64, y as f64), 7.0);
        if sym < 0.5 {
            continue;
        }
        // Dos picos que convergen al mismo centro cuentan una vez (gana el más simétrico).
        if let Some(prev) = cands.iter_mut().find(|c| c.p.dist(p) < 4.0) {
            if sym > prev.sym {
                *prev = Cand {
                    p,
                    sym,
                    score: f64::from(v) * sym * sym,
                };
            }
            continue;
        }
        cands.push(Cand {
            p,
            sym,
            score: f64::from(v) * sym * sym,
        });
    }
    cands.sort_by(|a, c| c.score.total_cmp(&a.score));
    // Fuera lo mucho más débil que lo mejor (ruido de la veta o del cartón).
    if let Some(best) = cands.first().map(|c| c.score) {
        cands.retain(|c| c.score >= 0.02 * best);
    }
    cands.truncate(want);
    cands
}
