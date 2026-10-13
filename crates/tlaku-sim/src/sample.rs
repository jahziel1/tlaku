//! Muestreo en el tiempo del movimiento simulado, para animar la vista previa.

use crate::plan::Seg;
use tlaku_core::Point;

/// Posición del cabezal cada `dt` segundos. Ver [`crate::sample`].
pub struct Sampler {
    segs: Vec<Seg>,
    /// Instante en que empieza cada segmento.
    starts: Vec<f64>,
    total: f64,
    end: Point,
    dt: f64,
    step: u64,
    idx: usize,
    done: bool,
}

impl Sampler {
    pub(crate) fn new(segs: Vec<Seg>, start: Point, dt: f64) -> Self {
        let mut starts = Vec::with_capacity(segs.len());
        let mut t = 0.0;
        for s in &segs {
            starts.push(t);
            t += s.seconds();
        }
        let valid = dt.is_finite() && dt > 0.0 && !segs.is_empty();
        Self {
            end: segs.last().map_or(start, |s| s.to),
            segs,
            starts,
            total: t,
            dt,
            step: 0,
            idx: 0,
            done: !valid,
        }
    }

    /// Distancia recorrida dentro del segmento `s` tras `t` segundos de empezarlo.
    fn distance_at(s: &Seg, t: f64) -> f64 {
        let total = s.seconds();
        let d = if t <= s.t_acc {
            s.v_in * t + 0.5 * s.accel * t * t
        } else if t <= s.t_acc + s.t_cruise {
            let d_acc = s.v_in * s.t_acc + 0.5 * s.accel * s.t_acc * s.t_acc;
            d_acc + s.v_peak * (t - s.t_acc)
        } else {
            let rem = (total - t).max(0.0);
            s.len - (s.v_out * rem + 0.5 * s.accel * rem * rem)
        };
        d.clamp(0.0, s.len)
    }
}

impl Iterator for Sampler {
    /// (tiempo en s, posición en mm, láser encendido)
    type Item = (f64, Point, bool);

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let t = self.step as f64 * self.dt;
        if t >= self.total {
            // Última muestra: el instante final exacto, con el cabezal detenido.
            self.done = true;
            return Some((self.total, self.end, false));
        }
        self.step += 1;
        while self.idx + 1 < self.segs.len()
            && t >= self.starts[self.idx] + self.segs[self.idx].seconds()
        {
            self.idx += 1;
        }
        let s = &self.segs[self.idx];
        let f = Self::distance_at(s, t - self.starts[self.idx]) / s.len;
        let p = Point::new(
            s.from.x + (s.to.x - s.from.x) * f,
            s.from.y + (s.to.y - s.from.y) * f,
        );
        Some((t, p, s.laser_on))
    }
}
