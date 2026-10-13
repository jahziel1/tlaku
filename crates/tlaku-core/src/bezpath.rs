//! Trayectorias con curvas (rectas, cuadráticas y cúbicas), la forma editable de un diseño.
//!
//! El editor trabaja con curvas para poder mover nodos y manejadores; el motor las aplana a
//! polilíneas con una tolerancia justo antes de planificar el corte.

use crate::{Point, Polyline};
use serde::{Deserialize, Serialize};

/// Un comando de trayectoria, como en SVG pero siempre en coordenadas absolutas (mm).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum PathSeg {
    MoveTo {
        to: Point,
    },
    LineTo {
        to: Point,
    },
    QuadTo {
        ctrl: Point,
        to: Point,
    },
    CubicTo {
        ctrl1: Point,
        ctrl2: Point,
        to: Point,
    },
    /// Vuelve al inicio del subtrayecto actual.
    Close,
}

/// Secuencia de comandos; puede tener varios subtrayectos (cada uno empieza con `MoveTo`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BezPath(pub Vec<PathSeg>);

impl From<&Polyline> for BezPath {
    fn from(p: &Polyline) -> Self {
        let mut segs = Vec::with_capacity(p.points.len() + 1);
        let mut points = p.points.iter();
        if let Some(&first) = points.next() {
            segs.push(PathSeg::MoveTo { to: first });
            segs.extend(points.map(|&to| PathSeg::LineTo { to }));
            if p.closed {
                segs.push(PathSeg::Close);
            }
        }
        BezPath(segs)
    }
}

/// Número de segmentos para no exceder `tolerance`, según la fórmula de Wang.
fn segments_for(max_second_diff: f64, degree_factor: f64, tolerance: f64) -> usize {
    let n = (degree_factor * max_second_diff / tolerance).sqrt().ceil();
    if n.is_finite() {
        (n as usize).clamp(1, 1000)
    } else {
        1
    }
}

fn second_diff(a: Point, b: Point, c: Point) -> f64 {
    (a.x - 2.0 * b.x + c.x).hypot(a.y - 2.0 * b.y + c.y)
}

impl BezPath {
    /// Aplana las curvas con un error máximo de `tolerance` (mm). Un subtrayecto se marca
    /// cerrado si termina en `Close` o si su último punto coincide con el primero.
    pub fn flatten(&self, tolerance: f64) -> Vec<Polyline> {
        let tolerance = if tolerance > 0.0 { tolerance } else { 0.01 };
        let mut out = Vec::new();
        let mut current: Vec<Point> = Vec::new();
        let finish = |pts: &mut Vec<Point>, closed: bool, out: &mut Vec<Polyline>| {
            let mut pts = std::mem::take(pts);
            pts.dedup_by(|a, b| a.dist(*b) < 1e-9);
            let mut closed = closed;
            if pts.len() > 2 && pts[0].dist(pts[pts.len() - 1]) < 1e-9 {
                pts.pop();
                closed = true;
            }
            if pts.len() >= 2 {
                out.push(Polyline {
                    points: pts,
                    closed,
                });
            }
        };
        for seg in &self.0 {
            match *seg {
                PathSeg::MoveTo { to } => {
                    finish(&mut current, false, &mut out);
                    current.push(to);
                }
                PathSeg::LineTo { to } => current.push(to),
                PathSeg::QuadTo { ctrl, to } => {
                    let Some(&p0) = current.last() else {
                        current.push(to);
                        continue;
                    };
                    let n = segments_for(second_diff(p0, ctrl, to), 0.25, tolerance);
                    for i in 1..=n {
                        let t = i as f64 / n as f64;
                        let u = 1.0 - t;
                        current.push(Point::new(
                            u * u * p0.x + 2.0 * u * t * ctrl.x + t * t * to.x,
                            u * u * p0.y + 2.0 * u * t * ctrl.y + t * t * to.y,
                        ));
                    }
                }
                PathSeg::CubicTo { ctrl1, ctrl2, to } => {
                    let Some(&p0) = current.last() else {
                        current.push(to);
                        continue;
                    };
                    let d = second_diff(p0, ctrl1, ctrl2).max(second_diff(ctrl1, ctrl2, to));
                    let n = segments_for(d, 0.75, tolerance);
                    for i in 1..=n {
                        let t = i as f64 / n as f64;
                        let u = 1.0 - t;
                        let (a, b, c, e) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                        current.push(Point::new(
                            a * p0.x + b * ctrl1.x + c * ctrl2.x + e * to.x,
                            a * p0.y + b * ctrl1.y + c * ctrl2.y + e * to.y,
                        ));
                    }
                }
                PathSeg::Close => {
                    let start = current.first().copied();
                    finish(&mut current, true, &mut out);
                    current.extend(start);
                }
            }
        }
        finish(&mut current, false, &mut out);
        out
    }

    /// Aplica `f` a todos los puntos (destinos y controles).
    pub fn map_points(&self, mut f: impl FnMut(Point) -> Point) -> BezPath {
        BezPath(
            self.0
                .iter()
                .map(|s| match *s {
                    PathSeg::MoveTo { to } => PathSeg::MoveTo { to: f(to) },
                    PathSeg::LineTo { to } => PathSeg::LineTo { to: f(to) },
                    PathSeg::QuadTo { ctrl, to } => PathSeg::QuadTo {
                        ctrl: f(ctrl),
                        to: f(to),
                    },
                    PathSeg::CubicTo { ctrl1, ctrl2, to } => PathSeg::CubicTo {
                        ctrl1: f(ctrl1),
                        ctrl2: f(ctrl2),
                        to: f(to),
                    },
                    PathSeg::Close => PathSeg::Close,
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polyline_roundtrip() {
        let p = Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(10.0, 0.0),
                Point::new(10.0, 5.0),
            ],
            closed: true,
        };
        assert_eq!(BezPath::from(&p).flatten(0.01), vec![p]);
    }

    #[test]
    fn cubic_quarter_circle_respects_tolerance() {
        // Cuarto de círculo de radio 10 con la constante clásica de Bézier.
        let k = 0.552_284_749_8 * 10.0;
        let path = BezPath(vec![
            PathSeg::MoveTo {
                to: Point::new(10.0, 0.0),
            },
            PathSeg::CubicTo {
                ctrl1: Point::new(10.0, k),
                ctrl2: Point::new(k, 10.0),
                to: Point::new(0.0, 10.0),
            },
        ]);
        let lines = path.flatten(0.01);
        assert_eq!(lines.len(), 1);
        for p in &lines[0].points {
            // La aproximación de Bézier se desvía ~0.003 mm del círculo; más la tolerancia.
            assert!((p.x.hypot(p.y) - 10.0).abs() < 0.02, "{p:?}");
        }
    }
}
