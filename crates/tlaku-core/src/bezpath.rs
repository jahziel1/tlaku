//! Trayectorias con curvas (rectas, cuadráticas y cúbicas), la forma editable de un diseño.
//!
//! El editor trabaja con curvas para poder mover nodos y manejadores; el motor las aplana a
//! polilíneas con una tolerancia justo antes de planificar el corte.

use crate::{Bounds, Point, Polyline};
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts")]
use ts_rs::TS;

/// Los puntos viajan como `[x, y]` en el JSON del contrato (al leer se acepta también `{x, y}`).
mod pt {
    use crate::Point;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(p: &Point, s: S) -> Result<S::Ok, S::Error> {
        [p.x, p.y].serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Point, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Pair([f64; 2]),
            Obj { x: f64, y: f64 },
        }
        Ok(match Repr::deserialize(d)? {
            Repr::Pair([x, y]) | Repr::Obj { x, y } => Point::new(x, y),
        })
    }
}

/// Un comando de trayectoria, como en SVG pero siempre en coordenadas absolutas (mm).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum PathSeg {
    MoveTo {
        #[serde(with = "pt")]
        #[cfg_attr(feature = "ts", ts(type = "[number, number]"))]
        to: Point,
    },
    LineTo {
        #[serde(with = "pt")]
        #[cfg_attr(feature = "ts", ts(type = "[number, number]"))]
        to: Point,
    },
    QuadTo {
        #[serde(with = "pt")]
        #[cfg_attr(feature = "ts", ts(type = "[number, number]"))]
        ctrl: Point,
        #[serde(with = "pt")]
        #[cfg_attr(feature = "ts", ts(type = "[number, number]"))]
        to: Point,
    },
    CubicTo {
        #[serde(with = "pt")]
        #[cfg_attr(feature = "ts", ts(type = "[number, number]"))]
        ctrl1: Point,
        #[serde(with = "pt")]
        #[cfg_attr(feature = "ts", ts(type = "[number, number]"))]
        ctrl2: Point,
        #[serde(with = "pt")]
        #[cfg_attr(feature = "ts", ts(type = "[number, number]"))]
        to: Point,
    },
    /// Vuelve al inicio del subtrayecto actual.
    Close,
}

/// Secuencia de comandos; puede tener varios subtrayectos (cada uno empieza con `MoveTo`).
/// En JSON es la lista de segmentos (un newtype se serializa como su contenido).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
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

/// Extremos (entre 0 y 1, exclusivos) de una cúbica 1D con coordenadas `a, b, c, d`.
fn cubic_extrema(a: f64, b: f64, c: f64, d: f64) -> [Option<f64>; 2] {
    // B'(t)/3 = (-a+3b-3c+d) t² + (2a-4b+2c) t + (b-a)
    let (qa, qb, qc) = (-a + 3.0 * b - 3.0 * c + d, 2.0 * (a - 2.0 * b + c), b - a);
    let inside = |t: f64| (t > 0.0 && t < 1.0).then_some(t);
    if qa.abs() < 1e-12 {
        return [(qb.abs() > 1e-12).then(|| -qc / qb).and_then(inside), None];
    }
    let disc = qb * qb - 4.0 * qa * qc;
    if disc < 0.0 {
        return [None, None];
    }
    let sq = disc.sqrt();
    [
        inside((-qb + sq) / (2.0 * qa)),
        inside((-qb - sq) / (2.0 * qa)),
    ]
}

impl BezPath {
    /// Caja envolvente **exacta** de la geometría dibujada (extremos de cada Bézier, no los
    /// puntos de control). Ignora los `MoveTo` sin trazo posterior. `None` si no dibuja nada.
    pub fn bounds(&self) -> Option<Bounds> {
        let mut b: Option<Bounds> = None;
        let mut add = |p: Point| {
            let b = b.get_or_insert(Bounds { min: p, max: p });
            b.min.x = b.min.x.min(p.x);
            b.min.y = b.min.y.min(p.y);
            b.max.x = b.max.x.max(p.x);
            b.max.y = b.max.y.max(p.y);
        };
        let mut cur: Option<Point> = None;
        let mut start: Option<Point> = None;
        let mut pending_move = false;
        for seg in &self.0 {
            match *seg {
                PathSeg::MoveTo { to } => {
                    cur = Some(to);
                    start = Some(to);
                    pending_move = true;
                }
                PathSeg::LineTo { to } => {
                    if let (true, Some(p)) = (pending_move, cur) {
                        add(p);
                    }
                    pending_move = false;
                    add(to);
                    cur = Some(to);
                }
                PathSeg::QuadTo { ctrl, to } => {
                    let p0 = cur.unwrap_or(to);
                    if pending_move {
                        add(p0);
                    }
                    pending_move = false;
                    add(to);
                    // Elevación a cúbica: mismos extremos.
                    let lerp = |a: Point, b: Point| {
                        Point::new(a.x + (b.x - a.x) * 2.0 / 3.0, a.y + (b.y - a.y) * 2.0 / 3.0)
                    };
                    add_cubic_extrema(&mut add, p0, lerp(p0, ctrl), lerp(to, ctrl), to);
                    cur = Some(to);
                }
                PathSeg::CubicTo { ctrl1, ctrl2, to } => {
                    let p0 = cur.unwrap_or(to);
                    if pending_move {
                        add(p0);
                    }
                    pending_move = false;
                    add(to);
                    add_cubic_extrema(&mut add, p0, ctrl1, ctrl2, to);
                    cur = Some(to);
                }
                PathSeg::Close => {
                    if let (false, Some(s)) = (pending_move, start) {
                        add(s);
                    }
                    cur = start;
                }
            }
        }
        b
    }

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

fn add_cubic_extrema(add: &mut impl FnMut(Point), p0: Point, c1: Point, c2: Point, p1: Point) {
    let at = |t: f64| {
        let u = 1.0 - t;
        let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        Point::new(
            a * p0.x + b * c1.x + c * c2.x + d * p1.x,
            a * p0.y + b * c1.y + c * c2.y + d * p1.y,
        )
    };
    for t in cubic_extrema(p0.x, c1.x, c2.x, p1.x)
        .into_iter()
        .chain(cubic_extrema(p0.y, c1.y, c2.y, p1.y))
        .flatten()
    {
        add(at(t));
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
    fn bounds_are_exact_for_curves() {
        let k = 0.552_284_749_8 * 10.0;
        let circle = BezPath(vec![
            PathSeg::MoveTo {
                to: Point::new(10.0, 0.0),
            },
            PathSeg::CubicTo {
                ctrl1: Point::new(10.0, k),
                ctrl2: Point::new(k, 10.0),
                to: Point::new(0.0, 10.0),
            },
        ]);
        let b = circle.bounds().unwrap();
        // Los controles llegan a 10, y la curva también: la caja es la del cuarto de círculo.
        assert!((b.min.x - 0.0).abs() < 1e-9 && (b.max.x - 10.0).abs() < 1e-9);
        assert!((b.max.y - 10.0).abs() < 1e-9 && b.min.y.abs() < 1e-9);
        // Una cuadrática: el pico de y=2·t(1-t)·10 es 5, no el control (10).
        let quad = BezPath(vec![
            PathSeg::MoveTo {
                to: Point::new(0.0, 0.0),
            },
            PathSeg::QuadTo {
                ctrl: Point::new(5.0, 10.0),
                to: Point::new(10.0, 0.0),
            },
        ]);
        let b = quad.bounds().unwrap();
        assert!((b.max.y - 5.0).abs() < 1e-9, "{b:?}");
        assert_eq!(BezPath::default().bounds(), None);
    }

    #[test]
    fn json_points_are_pairs() {
        let seg = PathSeg::LineTo {
            to: Point::new(1.0, 2.0),
        };
        let text = serde_json::to_string(&seg).unwrap();
        assert_eq!(text, r#"{"type":"line_to","to":[1.0,2.0]}"#);
        assert_eq!(serde_json::from_str::<PathSeg>(&text).unwrap(), seg);
        let obj = r#"{"type":"line_to","to":{"x":1.0,"y":2.0}}"#;
        assert_eq!(serde_json::from_str::<PathSeg>(obj).unwrap(), seg);
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
