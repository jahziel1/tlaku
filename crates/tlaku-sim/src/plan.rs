//! Planificador: perfil de velocidad de cada segmento con pasadas hacia atrás
//! y hacia adelante, como hacen los controladores GRBL.

use crate::profile::{MachineProfile, axis_limit};
use tlaku_core::Point;
use tlaku_core::toolpath::{Move, Toolpath};

/// Segmentos más cortos que esto (mm) se ignoran.
const MIN_LEN_MM: f64 = 1e-9;

/// Segmento con sus extremos y su perfil de velocidad completo; lo usa el muestreo.
#[derive(Debug, Clone)]
pub(crate) struct Seg {
    pub from: Point,
    pub to: Point,
    pub len: f64,
    pub laser_on: bool,
    /// Aceleración (mm/s²) en la dirección del segmento.
    pub accel: f64,
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

/// Segmento mínimo del planificador: lo que hace falta para calcular el tiempo (56 bytes por
/// movimiento; el `Seg` completo pasa de 150). La posición de los extremos solo la necesita el
/// muestreo y va aparte.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Lean {
    pub len: f64,
    /// Velocidad de crucero alcanzable (mm/s). En una pausa guarda su duración (s).
    pub vmax: f64,
    pub accel: f64,
    dir: (f64, f64),
    pub block: u32,
    /// Índice del movimiento (contando todos los bloques en orden) que originó el segmento.
    pub mv: u32,
    pub is_cut: bool,
    pub laser_on: bool,
    pub dwell: bool,
}

impl Lean {
    /// Segundos de una pausa (0 si es un movimiento).
    fn dwell_s(&self) -> f64 {
        if self.dwell { self.vmax } else { 0.0 }
    }
}

/// Reparto del tiempo de un segmento.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Timing {
    pub v_in: f64,
    pub v_peak: f64,
    pub v_out: f64,
    pub t_acc: f64,
    pub t_cruise: f64,
    pub t_dec: f64,
}

impl Timing {
    pub fn seconds(&self) -> f64 {
        self.t_acc + self.t_cruise + self.t_dec
    }
}

/// Plan de velocidades en forma compacta: segmentos y velocidad de entrada de cada uno
/// (`v[k]`; `v[n]` es la salida del último).
pub(crate) struct LeanPlan {
    pub segs: Vec<Lean>,
    v: Vec<f64>,
}

impl LeanPlan {
    /// Perfil del segmento `k`.
    pub fn timing(&self, k: usize) -> Timing {
        let s = &self.segs[k];
        if s.dwell {
            return Timing {
                v_in: 0.0,
                v_peak: 0.0,
                v_out: 0.0,
                t_acc: 0.0,
                t_cruise: s.dwell_s(),
                t_dec: 0.0,
            };
        }
        profile_segment(s, self.v[k], self.v[k + 1])
    }
}

/// Planifica sin guardar las posiciones: memoria mínima para estimar tiempos.
pub(crate) fn plan_lean(tp: &Toolpath, start: Point, profile: &MachineProfile) -> LeanPlan {
    let p = profile.sanitized();
    let (segs, _) = flatten(tp, start, &p, false);
    let v = entry_speeds(&segs, &p);
    LeanPlan { segs, v }
}

/// Aplana el trabajo a segmentos (sin los de longitud cero) y planifica sus
/// velocidades. La máquina parte de `start` y se detiene entre bloques y al
/// cambiar entre `Travel` y `Cut`.
pub(crate) fn plan(tp: &Toolpath, start: Point, profile: &MachineProfile) -> Vec<Seg> {
    let p = profile.sanitized();
    let (lean, ends) = flatten(tp, start, &p, true);
    let v = entry_speeds(&lean, &p);
    lean.iter()
        .zip(ends)
        .enumerate()
        .map(|(k, (s, (from, to)))| {
            let t = if s.dwell {
                Timing {
                    v_in: 0.0,
                    v_peak: 0.0,
                    v_out: 0.0,
                    t_acc: 0.0,
                    t_cruise: s.dwell_s(),
                    t_dec: 0.0,
                }
            } else {
                profile_segment(s, v[k], v[k + 1])
            };
            Seg {
                from,
                to,
                len: s.len,
                laser_on: s.laser_on,
                accel: s.accel,
                t_acc: t.t_acc,
                t_cruise: t.t_cruise,
                t_dec: t.t_dec,
                v_in: t.v_in,
                v_peak: t.v_peak,
                v_out: t.v_out,
            }
        })
        .collect()
}

/// Velocidad de entrada de cada segmento (`v[n]` = salida del último), con pasada hacia
/// atrás y hacia adelante. Usa un solo vector de trabajo además de `jmax`.
fn entry_speeds(segs: &[Lean], p: &MachineProfile) -> Vec<f64> {
    let n = segs.len();
    if n == 0 {
        return Vec::new();
    }

    // jmax[k]: velocidad máxima de entrada del segmento k (unión con k-1).
    let mut jmax = vec![0.0; n + 1];
    for k in 1..n {
        jmax[k] = junction_speed(
            &segs[k - 1],
            &segs[k],
            p.junction_deviation_mm,
            p.stop_on_cut_change,
        );
    }

    // Pasada hacia atrás con todo el programa visible.
    let mut v = vec![0.0; n + 1];
    for k in (1..n).rev() {
        v[k] = jmax[k].min(reach(v[k + 1], segs[k].accel, segs[k].len));
    }

    // Con ventana limitada, la entrada de k solo sabe frenar hasta el final del
    // último bloque que cabe en el búfer (k-1 en ejecución + los siguientes).
    if let Some(m) = p.planner_blocks {
        let m = m.max(1);
        for (k, entry) in v.iter_mut().enumerate().take(n).skip(1) {
            // Ventana: segmentos k-1 (en ejecución) hasta w = k+m-2.
            let w = k + m - 2;
            if w >= n - 1 {
                continue; // la ventana llega al final del programa
            }
            let mut speed = 0.0;
            if w >= k {
                for j in (k..=w).rev() {
                    speed = jmax[j].min(reach(speed, segs[j].accel, segs[j].len));
                }
            }
            *entry = speed;
        }
    }

    // Pasada hacia adelante (en el mismo vector: `v[k]` solo mira `v[k-1]` ya final).
    for k in 1..n {
        v[k] = v[k].min(reach(v[k - 1], segs[k - 1].accel, segs[k - 1].len));
    }
    v
}

/// Velocidad que se alcanza tras recorrer `len` acelerando desde `v0`.
fn reach(v0: f64, accel: f64, len: f64) -> f64 {
    (v0 * v0 + 2.0 * accel * len).sqrt()
}

/// Aplana a segmentos compactos. Con `ends`, devuelve también el origen y el destino de cada uno.
fn flatten(
    tp: &Toolpath,
    start: Point,
    p: &MachineProfile,
    ends: bool,
) -> (Vec<Lean>, Vec<(Point, Point)>) {
    let count: usize = tp.blocks.iter().map(|b| b.moves.len()).sum();
    let mut segs = Vec::with_capacity(count);
    let mut points = Vec::new();
    if ends {
        points.reserve(count);
    }
    let mut pos = start;
    let mut mv_index = 0u32;
    for (bi, block) in tp.blocks.iter().enumerate() {
        let block_speed = block.speed_mm_min / 60.0;
        for mv in &block.moves {
            let this_mv = mv_index;
            mv_index += 1;
            if let Move::Dwell { ms, power_pct, .. } = *mv {
                // La pausa se queda donde está la cabeza (el `to` de la IR no mueve nada).
                if ms.is_finite() && ms > 0.0 {
                    segs.push(Lean {
                        len: 0.0,
                        vmax: ms / 1000.0,
                        accel: 1.0,
                        dir: (0.0, 0.0),
                        block: bi as u32,
                        mv: this_mv,
                        is_cut: false,
                        laser_on: power_pct > 0.0,
                        dwell: true,
                    });
                    if ends {
                        points.push((pos, pos));
                    }
                }
                continue;
            }
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
                Move::Dwell { .. } => continue, // ya tratada arriba
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
            segs.push(Lean {
                len,
                vmax: nominal.min(axis_v),
                accel: axis_limit(p.accel_mm_s2, dir.0, dir.1),
                dir,
                block: bi as u32,
                mv: this_mv,
                is_cut,
                laser_on,
                dwell: false,
            });
            if ends {
                points.push((pos, to));
            }
            pos = to;
        }
    }
    (segs, points)
}

/// Velocidad máxima para pasar de `a` a `b` sin detenerse, por desviación de unión.
fn junction_speed(a: &Lean, b: &Lean, deviation: f64, stop_on_cut_change: bool) -> f64 {
    if a.block != b.block || a.dwell || b.dwell || (a.is_cut != b.is_cut && stop_on_cut_change) {
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
fn profile_segment(s: &Lean, v_in: f64, v_out: f64) -> Timing {
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
    let t_acc = (v_peak - v_in) / a;
    let t_dec = (v_peak - v_out) / a;
    let d_acc = (v_peak * v_peak - v_in * v_in) / (2.0 * a);
    let d_dec = (v_peak * v_peak - v_out * v_out) / (2.0 * a);
    Timing {
        v_in,
        v_peak,
        v_out,
        t_acc,
        t_cruise: ((len - d_acc - d_dec) / v_peak).max(0.0),
        t_dec,
    }
}
