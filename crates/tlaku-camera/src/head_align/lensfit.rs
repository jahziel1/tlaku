//! Lente aproximada cuando no hay calibración: busca la distorsión radial que hace que los
//! puntos del cabezal sean una homografía de la cama.
//!
//! Se prueban dos familias con dos parámetros cada una (Brown-Conrady `k1, k2` con la focal igual
//! al ancho de la imagen, y ojo de pez `focal, k1`). Para cada candidato se corrigen los píxeles,
//! se ajusta la homografía a las posiciones de la máquina y se mide el error (mm) recortado: el
//! 20 % peor no cuenta, para que un punto mal detectado no tuerza la lente. La búsqueda es
//! Nelder-Mead desde varios arranques. Solo se adopta si mejora claramente el ajuste sin lente.

use crate::calibration::{LensCalibration, LensModel};
use crate::homography::estimate_homography;
use tlaku_core::Point;

/// Puntos mínimos para estimar una lente (dos parámetros más los 8 de la homografía).
pub(super) const MIN_POINTS_FOR_LENS: usize = 9;
/// La lente se adopta si el error baja de esta fracción del que da el ajuste sin lente.
const MIN_GAIN: f64 = 0.85;
/// Y si el ajuste sin lente tiene al menos este error (mm): con menos no hay nada que arreglar.
const MIN_PLAIN_RMS_MM: f64 = 0.4;
/// Fracción de puntos (los mejores) que cuenta en el error.
const KEEP: f64 = 0.8;

pub(super) struct LensGuess {
    pub lens: LensCalibration,
    pub rms_before_mm: f64,
    pub rms_after_mm: f64,
}

/// Píxeles de la imagen cruda a píxeles de la imagen corregida con `lens` (centro del píxel).
pub(super) fn correct_all(lens: &LensCalibration, raw: &[Point]) -> Vec<Point> {
    let (zfx, zfy) = lens.corrected_focal();
    raw.iter()
        .map(|&p| {
            let (x, y) = lens.pixel_to_normalized(p);
            Point::new(zfx * x + lens.cx, zfy * y + lens.cy)
        })
        .collect()
}

/// Error rms (mm) recortado de la homografía píxel a máquina; infinito si no se puede ajustar.
fn trimmed_rms(px: &[Point], machine: &[Point]) -> f64 {
    if px.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return f64::INFINITY;
    }
    let pairs: Vec<(Point, Point)> = px.iter().copied().zip(machine.iter().copied()).collect();
    let Ok(h) = estimate_homography(&pairs) else {
        return f64::INFINITY;
    };
    let mut sq: Vec<f64> = pairs
        .iter()
        .map(|&(a, b)| {
            let q = h.apply(a);
            (q.x - b.x).powi(2) + (q.y - b.y).powi(2)
        })
        .collect();
    if sq.iter().any(|v| !v.is_finite()) {
        return f64::INFINITY;
    }
    sq.sort_by(f64::total_cmp);
    let keep = ((sq.len() as f64 * KEEP).ceil() as usize).clamp(1, sq.len());
    (sq[..keep].iter().sum::<f64>() / keep as f64).sqrt()
}

fn base_lens(size: (u32, u32)) -> LensCalibration {
    let (w, h) = (f64::from(size.0), f64::from(size.1));
    LensCalibration {
        width: size.0,
        height: size.1,
        fx: w,
        fy: w,
        cx: (w - 1.0) / 2.0,
        cy: (h - 1.0) / 2.0,
        k1: 0.0,
        k2: 0.0,
        p1: 0.0,
        p2: 0.0,
        k3: 0.0,
        model: LensModel::BrownConrady,
        k4: 0.0,
        view_zoom: 1.0,
        rms_error_px: 0.0,
        views_used: 0,
    }
}

fn brown(size: (u32, u32), p: [f64; 2]) -> LensCalibration {
    LensCalibration {
        k1: p[0],
        k2: p[1],
        ..base_lens(size)
    }
}

fn fisheye(size: (u32, u32), p: [f64; 2]) -> LensCalibration {
    let mut l = base_lens(size);
    let f = p[0] * f64::from(size.0);
    l.model = LensModel::Fisheye;
    l.fx = f;
    l.fy = f;
    l.k1 = p[1];
    l
}

/// Factor de la focal de la imagen corregida del ojo de pez: que el borde horizontal de la
/// imagen cruda (hasta 60° del eje) quepa. Mismo criterio que la calibración con tablero.
fn fisheye_zoom(l: &LensCalibration) -> f64 {
    let half_w = l.cx.max(f64::from(l.width) - 1.0 - l.cx);
    let (x, _) = l.undistort_normalized(half_w / l.fx, 0.0);
    let theta = x.abs().atan().min(60f64.to_radians());
    if theta < 1e-3 {
        return 1.0;
    }
    (half_w / theta.tan() / l.fx).clamp(0.25, 1.0)
}

/// Nelder-Mead en 2D desde `x0` con simplex inicial de lado `step`.
fn nelder_mead(f: &dyn Fn([f64; 2]) -> f64, x0: [f64; 2], step: [f64; 2]) -> ([f64; 2], f64) {
    let mut s = [x0, [x0[0] + step[0], x0[1]], [x0[0], x0[1] + step[1]]];
    let mut v = [f(s[0]), f(s[1]), f(s[2])];
    for _ in 0..120 {
        let mut idx = [0usize, 1, 2];
        idx.sort_by(|&a, &b| v[a].total_cmp(&v[b]));
        s = [s[idx[0]], s[idx[1]], s[idx[2]]];
        v = [v[idx[0]], v[idx[1]], v[idx[2]]];
        if !v[0].is_finite() && !v[1].is_finite() {
            break;
        }
        if v[2].is_finite() && (v[2] - v[0]).abs() < 1e-7 {
            break;
        }
        let c = [(s[0][0] + s[1][0]) / 2.0, (s[0][1] + s[1][1]) / 2.0];
        let along = |t: f64| [c[0] + t * (s[2][0] - c[0]), c[1] + t * (s[2][1] - c[1])];
        let xr = along(-1.0);
        let fr = f(xr);
        if fr < v[0] {
            let xe = along(-2.0);
            let fe = f(xe);
            if fe < fr {
                (s[2], v[2]) = (xe, fe);
            } else {
                (s[2], v[2]) = (xr, fr);
            }
        } else if fr < v[1] {
            (s[2], v[2]) = (xr, fr);
        } else {
            let xc = if fr < v[2] { along(-0.5) } else { along(0.5) };
            let fc = f(xc);
            if fc < v[2].min(fr) {
                (s[2], v[2]) = (xc, fc);
            } else {
                for i in 1..3 {
                    s[i] = [(s[0][0] + s[i][0]) / 2.0, (s[0][1] + s[i][1]) / 2.0];
                    v[i] = f(s[i]);
                }
            }
        }
    }
    let best = (0..3).min_by(|&a, &b| v[a].total_cmp(&v[b])).unwrap_or(0);
    (s[best], v[best])
}

/// Busca la lente aproximada. `None` si el ajuste sin lente ya es bueno o la lente no mejora lo
/// bastante.
pub(super) fn estimate_lens(
    raw: &[Point],
    machine: &[Point],
    size: (u32, u32),
) -> Option<LensGuess> {
    let plain = trimmed_rms(raw, machine);
    if !plain.is_finite() || plain < MIN_PLAIN_RMS_MM {
        return None;
    }
    let cost_brown = |p: [f64; 2]| {
        if p[0].abs() > 3.0 || p[1].abs() > 10.0 {
            return f64::INFINITY;
        }
        trimmed_rms(&correct_all(&brown(size, p), raw), machine)
    };
    let cost_fish = |p: [f64; 2]| {
        if !(0.1..=3.0).contains(&p[0]) || p[1].abs() > 2.0 {
            return f64::INFINITY;
        }
        trimmed_rms(&correct_all(&fisheye(size, p), raw), machine)
    };
    let mut best: Option<(LensCalibration, f64)> = None;
    let mut consider = |lens: LensCalibration, cost: f64| {
        if cost.is_finite() && best.as_ref().is_none_or(|(_, b)| cost < *b) {
            best = Some((lens, cost));
        }
    };
    for x0 in [[0.0, 0.0], [-0.3, 0.0], [-0.6, 0.3], [0.2, 0.0]] {
        let (p, c) = nelder_mead(&cost_brown, x0, [0.15, 0.1]);
        consider(brown(size, p), c);
    }
    for f0 in [0.3, 0.45, 0.7] {
        let (p, c) = nelder_mead(&cost_fish, [f0, 0.0], [0.08, 0.05]);
        let mut lens = fisheye(size, p);
        lens.view_zoom = fisheye_zoom(&lens);
        consider(lens, c);
    }
    let (lens, after) = best?;
    (after < MIN_GAIN * plain).then_some(LensGuess {
        lens,
        rms_before_mm: plain,
        rms_after_mm: after,
    })
}
