//! Planificador: perfil de velocidad de cada segmento con pasadas hacia atrás
//! y hacia adelante, como hacen los controladores GRBL.

use crate::profile::{MachineProfile, axis_limit};
use laser_core::Point;
use laser_core::toolpath::{Move, Toolpath};

/// Segmentos más cortos que esto (mm) se ignoran.
const MIN_LEN_MM: f64 = 1e-9;

#[derive(Debug, Clone)]
pub(crate) struct Seg {
    pub from: Point,
    pub to: Point,
    pub len: f64,
    pub block: usize,
    /// Índice del movimiento (contando todos los bloques en orden) que originó el segmento.
    pub mv: usize,
    pub is_cut: bool,
    pub laser_on: bool,
    /// Velocidad de crucero alcanzable (mm/s) y aceleración (mm/s²) en la dirección.
    pub vmax: f64,
    pub accel: f64,
    dir: (f64, f64),
    pub t_acc: f64,
    pub t_cruise: f64,
    pub t_dec: f64,
    pub v_in: f64,
    pub v_peak: f64,
    pub v_out: f64,
}

impl Seg {
    pub fn seconds(&self) -> f64 {
        self.t_acc + self.t_cruise + self.t_dec
    }
}

/// Aplana el trabajo a segmentos (sin los de longitud cero) y planifica sus
/// velocidades. La máquina parte de `start` y se detiene entre bloques y al
/// cambiar entre `Travel` y `Cut`.
pub(crate) fn plan(tp: &Toolpath, start: Point, profile: &MachineProfile) -> Vec<Seg> {
    let p = profile.sanitized();
    let mut segs = flatten(tp, start, &p);
    let n = segs.len();
    if n == 0 {
        return segs;
    }

    // jmax[k]: velocidad máxima de entrada del segmento k (unión con k-1).
    let mut jmax = vec![0.0; n + 1];
    for k in 1..n {
        jmax[k] = junction_speed(&segs[k - 1], &segs[k], p.junction_deviation_mm);
    }

    // Pasada hacia atrás con todo el programa visible.
    let mut full = vec![0.0; n + 1];
    for k in (1..n).rev() {
        full[k] = jmax[k].min(reach(full[k + 1], segs[k].accel, segs[k].len));
    }

    // Con ventana limitada, la entrada de k solo sabe frenar hasta el final del
    // último bloque que cabe en el búfer (k-1 en ejecución + los siguientes).
    let cap = match p.planner_blocks {
        None => full,
        Some(m) => {
            let m = m.max(1);
            let mut cap = full.clone();
            for (k, entry) in cap.iter_mut().enumerate().take(n).skip(1) {
                // Ventana: segmentos k-1 (en ejecución) hasta w = k+m-2.
                let w = k + m - 2;
                if w >= n - 1 {
                    continue; // la ventana llega al final del programa
                }
                let mut v = 0.0;
                if w >= k {
                    for j in (k..=w).rev() {
                        v = jmax[j].min(reach(v, segs[j].accel, segs[j].len));
                    }
                }
                *entry = v;
            }
            cap
        }
    };

    // Pasada hacia adelante.
    let mut v = vec![0.0; n + 1];
    for k in 1..n {
        v[k] = cap[k].min(reach(v[k - 1], segs[k - 1].accel, segs[k - 1].len));
    }

    for (k, s) in segs.iter_mut().enumerate() {
        profile_segment(s, v[k], v[k + 1]);
    }
    segs
}

/// Velocidad que se alcanza tras recorrer `len` acelerando desde `v0`.
fn reach(v0: f64, accel: f64, len: f64) -> f64 {
    (v0 * v0 + 2.0 * accel * len).sqrt()
}

fn flatten(tp: &Toolpath, start: Point, p: &MachineProfile) -> Vec<Seg> {
    let mut segs = Vec::new();
    let mut pos = start;
    let mut mv_index = 0;
    for (bi, block) in tp.blocks.iter().enumerate() {
        let block_speed = block.speed_mm_min / 60.0;
        for mv in &block.moves {
            let this_mv = mv_index;
            mv_index += 1;
            let to = mv.to();
            let len = pos.dist(to);
            if !(len.is_finite() && len > MIN_LEN_MM) {
                if to.x.is_finite() && to.y.is_finite() {
                    pos = to;
                }
                continue;
            }
            let dir = ((to.x - pos.x) / len, (to.y - pos.y) / len);
            let (is_cut, laser_on, nominal) = match *mv {
                Move::Travel { .. } => (false, false, p.rapid_rate_mm_min / 60.0),
                Move::Cut { power_pct, .. } => {
                    // Velocidad de bloque inválida: se recorre como rápido.
                    let v = if block_speed.is_finite() && block_speed > 0.0 {
                        block_speed
                    } else {
                        p.rapid_rate_mm_min / 60.0
                    };
                    (true, power_pct > 0.0, v)
                }
            };
            let axis_v = axis_limit(p.max_rate_mm_min.map(|r| r / 60.0), dir.0, dir.1);
            segs.push(Seg {
                from: pos,
                to,
                len,
                block: bi,
                mv: this_mv,
                is_cut,
                laser_on,
                vmax: nominal.min(axis_v),
                accel: axis_limit(p.accel_mm_s2, dir.0, dir.1),
                dir,
                t_acc: 0.0,
                t_cruise: 0.0,
                t_dec: 0.0,
                v_in: 0.0,
                v_peak: 0.0,
                v_out: 0.0,
            });
            pos = to;
        }
    }
    segs
}

/// Velocidad máxima para pasar de `a` a `b` sin detenerse, por desviación de unión.
fn junction_speed(a: &Seg, b: &Seg, deviation: f64) -> f64 {
    if a.block != b.block || a.is_cut != b.is_cut {
        return 0.0;
    }
    let cap = a.vmax.min(b.vmax);
    // Coseno del ángulo entre el tramo anterior invertido y el siguiente:
    // -1 es recta, +1 es media vuelta.
    let cos_theta = -(a.dir.0 * b.dir.0 + a.dir.1 * b.dir.1);
    let sin_half = (0.5 * (1.0 - cos_theta)).max(0.0).sqrt();
    if sin_half >= 1.0 - 1e-12 {
        return cap;
    }
    let accel = a.accel.min(b.accel);
    let v2 = accel * deviation * sin_half / (1.0 - sin_half);
    v2.sqrt().min(cap)
}

/// Reparte el segmento en aceleración, crucero y frenado (trapecio, o triángulo
/// si no alcanza la velocidad de crucero).
fn profile_segment(s: &mut Seg, v_in: f64, v_out: f64) {
    let (a, len) = (s.accel, s.len);
    let v_in = v_in.min(s.vmax);
    let v_out = v_out.min(s.vmax);
    let d_acc = ((s.vmax * s.vmax - v_in * v_in) / (2.0 * a)).max(0.0);
    let d_dec = ((s.vmax * s.vmax - v_out * v_out) / (2.0 * a)).max(0.0);
    let v_peak = if d_acc + d_dec <= len {
        s.vmax
    } else {
        let p2 = (2.0 * a * len + v_in * v_in + v_out * v_out) / 2.0;
        p2.max(v_in * v_in).max(v_out * v_out).sqrt().min(s.vmax)
    };
    s.v_in = v_in;
    s.v_out = v_out;
    s.v_peak = v_peak;
    s.t_acc = (v_peak - v_in) / a;
    s.t_dec = (v_peak - v_out) / a;
    let d_acc = (v_peak * v_peak - v_in * v_in) / (2.0 * a);
    let d_dec = (v_peak * v_peak - v_out * v_out) / (2.0 * a);
    s.t_cruise = ((len - d_acc - d_dec) / v_peak).max(0.0);
}
