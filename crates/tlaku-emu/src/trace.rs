//! Rastro del láser: dónde y cuánto estuvo encendido. Las pruebas lo usan para comprobar que no
//! se quema en los G0 ni fuera de la figura.

use std::time::Duration;

/// Tipo de movimiento durante el que ardió el láser (o estaba la cabeza parada).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveKind {
    /// G0, homing.
    Rapid,
    /// G1, G2, G3.
    Feed,
    /// `$J=`.
    Jog,
    /// `G38.x`: el láser nunca dispara.
    Probe,
    /// Cabeza parada (pausa, `G4`, fin de bloque, `M3` en reposo).
    Stationary,
}

/// Modo de husillo con el que se encendió.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpindleMode {
    Off,
    /// M3, potencia constante.
    Cw,
    /// M4, potencia dinámica.
    Ccw,
}

/// Tramo continuo con el láser encendido dentro de un mismo bloque de movimiento. Las
/// posiciones son de máquina (MPos); con el origen de trabajo en cero coinciden con las del
/// G-code.
#[derive(Debug, Clone, PartialEq)]
pub struct LaserSegment {
    pub start: [f64; 3],
    pub end: [f64; 3],
    pub t0: Duration,
    pub t1: Duration,
    pub kind: MoveKind,
    pub mode: SpindleMode,
    /// `S` programada del bloque.
    pub s_programmed: f64,
    /// Mayor potencia efectiva (con M4 y overrides) vista en el tramo.
    pub s_peak: f64,
    /// Integral de la potencia efectiva en el tiempo (S·s).
    pub energy: f64,
    /// Número (desde 1) de la línea no vacía que originó el bloque; 0 si no hay.
    pub line: usize,
}

impl LaserSegment {
    /// Longitud recorrida en el plano XY más Z.
    pub fn length(&self) -> f64 {
        dist3(self.start, self.end)
    }

    pub fn duration(&self) -> Duration {
        self.t1.saturating_sub(self.t0)
    }

    /// Potencia efectiva media (energía / tiempo).
    pub fn s_avg(&self) -> f64 {
        let t = self.duration().as_secs_f64();
        if t > 0.0 { self.energy / t } else { 0.0 }
    }

    /// `true` si la cabeza no se movió durante el tramo (quema un punto).
    pub fn is_stationary(&self) -> bool {
        self.length() < 1e-9
    }
}

/// Lista de tramos con el láser encendido, en orden cronológico.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LaserTrace {
    segments: Vec<LaserSegment>,
}

fn dist3(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Distancia de un punto a un segmento, en el plano XY.
pub fn dist_point_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((p[0] - (a[0] + t * dx)).powi(2) + (p[1] - (a[1] + t * dy)).powi(2)).sqrt()
}

impl LaserTrace {
    pub(crate) fn push(&mut self, seg: LaserSegment) -> usize {
        self.segments.push(seg);
        self.segments.len() - 1
    }

    pub(crate) fn last_mut(&mut self, idx: usize) -> Option<&mut LaserSegment> {
        self.segments.get_mut(idx)
    }

    pub fn segments(&self) -> &[LaserSegment] {
        &self.segments
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn clear(&mut self) {
        self.segments.clear();
    }

    /// Longitud total con el láser encendido (mm).
    pub fn total_length(&self) -> f64 {
        self.segments.iter().map(LaserSegment::length).sum()
    }

    /// Tramos en que ardió el láser durante un movimiento rápido (G0, homing): no debería haber
    /// ninguno en modo láser.
    pub fn burned_during_rapids(&self) -> Vec<&LaserSegment> {
        self.segments
            .iter()
            .filter(|s| s.kind == MoveKind::Rapid)
            .collect()
    }

    /// Tramos con la cabeza parada y el láser encendido.
    pub fn stationary_burns(&self) -> Vec<&LaserSegment> {
        self.segments.iter().filter(|s| s.is_stationary()).collect()
    }

    /// Caja envolvente XY de todo lo quemado: `(min, max)`.
    pub fn bounds(&self) -> Option<([f64; 2], [f64; 2])> {
        let mut it = self.segments.iter().flat_map(|s| [s.start, s.end]);
        let first = it.next()?;
        let mut lo = [first[0], first[1]];
        let mut hi = lo;
        for p in it {
            lo = [lo[0].min(p[0]), lo[1].min(p[1])];
            hi = [hi[0].max(p[0]), hi[1].max(p[1])];
        }
        Some((lo, hi))
    }

    /// Mayor distancia (mm) de un punto quemado al conjunto de segmentos `path` (lista de
    /// puntos consecutivos; si `closed`, se une el último con el primero). Se muestrea cada
    /// tramo en 11 puntos. `None` si no hay rastro.
    pub fn max_distance_from_path(&self, path: &[[f64; 2]], closed: bool) -> Option<f64> {
        if self.segments.is_empty() || path.is_empty() {
            return None;
        }
        let mut edges: Vec<([f64; 2], [f64; 2])> = path.windows(2).map(|w| (w[0], w[1])).collect();
        if closed && path.len() > 2 {
            edges.push((path[path.len() - 1], path[0]));
        }
        if edges.is_empty() {
            edges.push((path[0], path[0]));
        }
        let mut worst = 0.0f64;
        for seg in &self.segments {
            for i in 0..=10 {
                let f = f64::from(i) / 10.0;
                let p = [
                    seg.start[0] + (seg.end[0] - seg.start[0]) * f,
                    seg.start[1] + (seg.end[1] - seg.start[1]) * f,
                ];
                let d = edges
                    .iter()
                    .map(|&(a, b)| dist_point_segment(p, a, b))
                    .fold(f64::INFINITY, f64::min);
                worst = worst.max(d);
            }
        }
        Some(worst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(a: [f64; 3], b: [f64; 3], kind: MoveKind) -> LaserSegment {
        LaserSegment {
            start: a,
            end: b,
            t0: Duration::ZERO,
            t1: Duration::from_secs(1),
            kind,
            mode: SpindleMode::Ccw,
            s_programmed: 500.0,
            s_peak: 500.0,
            energy: 250.0,
            line: 1,
        }
    }

    #[test]
    fn distances_and_bounds() {
        assert!((dist_point_segment([5.0, 3.0], [0.0, 0.0], [10.0, 0.0]) - 3.0).abs() < 1e-12);
        assert!((dist_point_segment([-4.0, 3.0], [0.0, 0.0], [10.0, 0.0]) - 5.0).abs() < 1e-12);
        let mut t = LaserTrace::default();
        t.push(seg([0.0; 3], [10.0, 0.0, 0.0], MoveKind::Feed));
        t.push(seg([10.0, 0.0, 0.0], [10.0, 5.0, 0.0], MoveKind::Rapid));
        assert_eq!(t.bounds(), Some(([0.0, 0.0], [10.0, 5.0])));
        assert_eq!(t.burned_during_rapids().len(), 1);
        assert!((t.total_length() - 15.0).abs() < 1e-12);
        let d = t.max_distance_from_path(&[[0.0, 0.0], [10.0, 0.0]], false);
        assert!((d.unwrap() - 5.0).abs() < 1e-9);
        assert_eq!(t.segments()[0].s_avg(), 250.0);
    }
}
