//! Medición de una trayectoria aplanada: posición y dirección según la distancia recorrida.

use tlaku_core::Point;
use tlaku_core::bezpath::BezPath;
use tlaku_i18n::tr;

/// Tolerancia de aplanado de la trayectoria (mm).
const FLATTEN_TOL_MM: f64 = 0.005;

pub struct PathSampler {
    points: Vec<Point>,
    /// Longitud acumulada hasta cada punto; `cum[0] == 0`.
    cum: Vec<f64>,
    pub closed: bool,
}

impl PathSampler {
    /// Usa el primer subtrayecto; el segundo valor indica que había más y se ignoraron.
    pub fn new(path: &BezPath, flip: bool) -> Result<(Self, bool), String> {
        let lines = path.flatten(FLATTEN_TOL_MM);
        let ignored_extra = lines.len() > 1;
        let Some(line) = lines.into_iter().next() else {
            return Err(tr!("text.path_sampler.no_segments"));
        };
        let mut points = line.points;
        if line.closed {
            points.push(points[0]);
        }
        if flip {
            points.reverse();
        }
        let mut cum = Vec::with_capacity(points.len());
        let mut acc = 0.0;
        cum.push(0.0);
        for w in points.windows(2) {
            acc += w[0].dist(w[1]);
            cum.push(acc);
        }
        if acc <= 1e-9 {
            return Err(tr!("text.path_sampler.zero_length"));
        }
        Ok((
            Self {
                points,
                cum,
                closed: line.closed,
            },
            ignored_extra,
        ))
    }

    pub fn length(&self) -> f64 {
        *self.cum.last().unwrap_or(&0.0)
    }

    /// Normaliza `s`: en trayectorias cerradas da la vuelta, en abiertas se queda en los extremos.
    fn normalize(&self, s: f64) -> f64 {
        let len = self.length();
        if self.closed {
            s.rem_euclid(len)
        } else {
            s.clamp(0.0, len)
        }
    }

    pub fn point_at(&self, s: f64) -> Point {
        let s = self.normalize(s);
        let i = self.segment_index(s);
        let (a, b) = (self.points[i], self.points[i + 1]);
        let seg_len = self.cum[i + 1] - self.cum[i];
        let t = if seg_len > 0.0 {
            (s - self.cum[i]) / seg_len
        } else {
            0.0
        };
        Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
    }

    fn segment_index(&self, s: f64) -> usize {
        // Último segmento cuyo inicio es <= s (y con longitud positiva si es posible).
        let n = self.points.len() - 1;
        self.cum
            .partition_point(|&c| c <= s)
            .saturating_sub(1)
            .min(n - 1)
    }

    /// Dirección unitaria alrededor de `s`: cuerda entre `s - half` y `s + half`, lo que
    /// suaviza el aplanado y es la tangente exacta en el punto medio de un arco.
    pub fn direction_at(&self, s: f64, half: f64) -> (f64, f64) {
        let half = half.max(1e-3);
        let (a, b) = if self.closed {
            (self.point_at(s - half), self.point_at(s + half))
        } else {
            // Cerca de los extremos se desplaza la ventana para no perder dirección.
            let len = self.length();
            let lo = (s - half).clamp(0.0, len);
            let hi = (s + half).clamp(0.0, len);
            (self.point_at(lo), self.point_at(hi))
        };
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let n = dx.hypot(dy);
        if n > 1e-12 {
            return (dx / n, dy / n);
        }
        let i = self.segment_index(self.normalize(s));
        let (a, b) = (self.points[i], self.points[i + 1]);
        let n = (b.x - a.x).hypot(b.y - a.y).max(1e-12);
        ((b.x - a.x) / n, (b.y - a.y) / n)
    }
}
