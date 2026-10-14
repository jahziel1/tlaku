//! Redondear esquinas (Fillet) y chaflanes.
//!
//! Opera sobre trayectorias de Bézier (`BezPath`) y solo toca las esquinas formadas por **dos
//! rectas** (como exige el comportamiento de referencia, F60 de `docs/spec/01-diseno-edicion.md`):
//! las curvas y las rectas que no llegan a una esquina se conservan intactas. El redondeo es un
//! arco circular exacto (cúbicas de a lo más 90 grados) tangente a las dos rectas.
//!
//! Estilos:
//! - `Round`: arco tangente de radio `radius` (la esquina se redondea).
//! - `Chamfer`: corte recto que une los puntos a `radius` mm de la esquina sobre cada recta.
//! - `Bite`: "mordisco" cóncavo; un arco de radio `radius` centrado en la esquina entre los
//!   puntos a `radius` mm de ella sobre cada recta (equivale a un radio negativo).
//!
//! Espacio: un redondeo consume `d = radius / tan(ángulo/2)` de cada recta. Si dos esquinas
//! vecinas se disputan una misma recta, ambas se reducen en proporción hasta caber (se cuenta
//! en `clamped`). Si una esquina necesita más recta de la que hay (porque el vecino no se
//! redondea), se deja viva y se cuenta en `no_space`.

use crate::split::split_subpaths;
use std::f64::consts::PI;
use tlaku_core::Point;
use tlaku_core::bezpath::{BezPath, PathSeg};

/// Distancia (mm) a la que un nodo seleccionado coincide con una esquina.
const PICK_TOL: f64 = 0.01;
/// Ángulos de giro menores que esto (grados) no son una esquina.
const MIN_TURN_DEG: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilletStyle {
    Round,
    Chamfer,
    Bite,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FilletOptions {
    /// Radio (o distancia del chaflán / mordisco) en mm; positivo.
    pub radius: f64,
    pub style: FilletStyle,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FilletOutcome {
    pub path: BezPath,
    /// Esquinas modificadas.
    pub rounded: usize,
    /// De ellas, las que se hicieron con un radio menor al pedido para caber.
    pub clamped: usize,
    /// Esquinas candidatas que se dejaron vivas por falta de recta.
    pub no_space: usize,
}

#[derive(Clone, Copy)]
pub(crate) enum Edge {
    Line,
    Cubic(Point, Point),
}

pub(crate) struct Sub {
    pub(crate) nodes: Vec<Point>,
    /// `edges[i]` une `nodes[i]` con `nodes[(i + 1) % n]`.
    pub(crate) edges: Vec<Edge>,
    pub(crate) closed: bool,
}

pub(crate) fn parse(sub: &BezPath) -> Option<Sub> {
    let mut nodes: Vec<Point> = Vec::new();
    let mut edges: Vec<Edge> = Vec::new();
    let mut closed = false;
    for seg in &sub.0 {
        match *seg {
            PathSeg::MoveTo { to } => {
                if !nodes.is_empty() {
                    return None;
                }
                nodes.push(to);
            }
            PathSeg::LineTo { to } => {
                let last = *nodes.last()?;
                if last.dist(to) > 1e-9 {
                    nodes.push(to);
                    edges.push(Edge::Line);
                }
            }
            PathSeg::QuadTo { ctrl, to } => {
                let p0 = *nodes.last()?;
                let c1 = Point::new(
                    p0.x + 2.0 / 3.0 * (ctrl.x - p0.x),
                    p0.y + 2.0 / 3.0 * (ctrl.y - p0.y),
                );
                let c2 = Point::new(
                    to.x + 2.0 / 3.0 * (ctrl.x - to.x),
                    to.y + 2.0 / 3.0 * (ctrl.y - to.y),
                );
                nodes.push(to);
                edges.push(Edge::Cubic(c1, c2));
            }
            PathSeg::CubicTo { ctrl1, ctrl2, to } => {
                nodes.push(to);
                edges.push(Edge::Cubic(ctrl1, ctrl2));
            }
            PathSeg::Close => closed = true,
        }
    }
    if nodes.len() < 2 {
        return None;
    }
    // Un subtrayecto que vuelve a su inicio cuenta como cerrado.
    if nodes.len() >= 4 && nodes[0].dist(nodes[nodes.len() - 1]) < 1e-9 {
        nodes.pop();
        closed = true;
    } else if closed {
        edges.push(Edge::Line);
    }
    if closed && nodes.len() < 3 {
        return None;
    }
    Some(Sub {
        nodes,
        edges,
        closed,
    })
}

pub(crate) fn unit_from(a: Point, b: Point) -> Point {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l = dx.hypot(dy);
    Point::new(dx / l, dy / l)
}

/// Arco `center, radius` desde el ángulo `a0` barriendo `sweep` (con signo), en cúbicas de ≤ 90°.
fn arc_cubics(center: Point, radius: f64, a0: f64, sweep: f64) -> Vec<[Point; 3]> {
    let pieces = (sweep.abs() / (PI / 2.0) - 1e-9).ceil().max(1.0) as usize;
    let step = sweep / pieces as f64;
    let h = 4.0 / 3.0 * (step / 4.0).tan() * radius;
    let at = |a: f64| Point::new(center.x + radius * a.cos(), center.y + radius * a.sin());
    let tangent = |a: f64| Point::new(-a.sin(), a.cos());
    (0..pieces)
        .map(|k| {
            let (s, e) = (a0 + step * k as f64, a0 + step * (k + 1) as f64);
            let (p0, p3) = (at(s), at(e));
            let (t0, t3) = (tangent(s), tangent(e));
            [
                Point::new(p0.x + h * t0.x, p0.y + h * t0.y),
                Point::new(p3.x - h * t3.x, p3.y - h * t3.y),
                p3,
            ]
        })
        .collect()
}

fn normalize_angle(a: f64) -> f64 {
    let mut x = a % (2.0 * PI);
    if x > PI {
        x -= 2.0 * PI;
    } else if x <= -PI {
        x += 2.0 * PI;
    }
    x
}

enum Join {
    None,
    Line,
    Arc(Vec<[Point; 3]>),
}

struct NodeOut {
    enter: Point,
    exit: Point,
    join: Join,
}

/// Redondea las esquinas de recta con recta de `path`. Si `only` es `Some`, solo las esquinas
/// que coinciden (a 0,01 mm) con alguno de esos puntos; los puntos van en las mismas
/// coordenadas que `path`. Radios no positivos o no finitos dejan la trayectoria como está.
pub fn fillet_path(path: &BezPath, opts: &FilletOptions, only: Option<&[Point]>) -> FilletOutcome {
    let mut out = FilletOutcome {
        path: BezPath(Vec::new()),
        rounded: 0,
        clamped: 0,
        no_space: 0,
    };
    let valid = opts.radius.is_finite() && opts.radius > 0.0;
    for subpath in split_subpaths(path) {
        let parsed = valid.then(|| parse(&subpath)).flatten();
        let Some(sub) = parsed else {
            out.path.0.extend(subpath.0);
            continue;
        };
        match fillet_sub(&sub, opts, only) {
            Some((segs, rounded, clamped, no_space)) => {
                out.rounded += rounded;
                out.clamped += clamped;
                out.no_space += no_space;
                if rounded == 0 {
                    out.path.0.extend(subpath.0);
                } else {
                    out.path.0.extend(segs);
                }
            }
            None => out.path.0.extend(subpath.0),
        }
    }
    out
}

fn fillet_sub(
    sub: &Sub,
    opts: &FilletOptions,
    only: Option<&[Point]>,
) -> Option<(Vec<PathSeg>, usize, usize, usize)> {
    let n = sub.nodes.len();
    let edge_count = sub.edges.len();
    let is_line = |e: usize| matches!(sub.edges[e], Edge::Line);
    let edge_len = |e: usize| sub.nodes[e].dist(sub.nodes[(e + 1) % n]);

    // Esquinas candidatas: dos rectas y un giro apreciable.
    let mut theta = vec![0.0f64; n];
    let mut want = vec![0.0f64; n];
    let mut candidate = vec![false; n];
    for i in 0..n {
        let (e_in, e_out) = if sub.closed {
            ((i + n - 1) % n, i)
        } else if i == 0 || i == n - 1 {
            continue;
        } else {
            (i - 1, i)
        };
        if !is_line(e_in) || !is_line(e_out) {
            continue;
        }
        let v = sub.nodes[i];
        let (prev, next) = (sub.nodes[(i + n - 1) % n], sub.nodes[(i + 1) % n]);
        let (u, w) = (unit_from(v, prev), unit_from(v, next));
        let th = (u.x * w.x + u.y * w.y).clamp(-1.0, 1.0).acos();
        if th > PI - MIN_TURN_DEG.to_radians() || th < 1e-6 {
            continue;
        }
        if let Some(points) = only
            && !points.iter().any(|p| p.dist(v) <= PICK_TOL)
        {
            continue;
        }
        candidate[i] = true;
        theta[i] = th;
        want[i] = match opts.style {
            FilletStyle::Round => opts.radius / (th / 2.0).tan(),
            FilletStyle::Chamfer | FilletStyle::Bite => opts.radius,
        };
    }
    let total = candidate.iter().filter(|&&c| c).count();
    if total == 0 {
        return Some((Vec::new(), 0, 0, 0));
    }

    // Reparto del espacio de cada recta entre sus dos extremos.
    let mut d = want.clone();
    let mut scale = vec![1.0f64; n];
    for e in 0..edge_count {
        let (a, b) = (e, (e + 1) % n);
        if !(is_line(e) && candidate[a] && candidate[b]) {
            continue;
        }
        let sum = want[a] + want[b];
        let len = edge_len(e);
        if sum > len * (1.0 + 1e-9) {
            let f = len / sum;
            scale[a] = scale[a].min(f);
            scale[b] = scale[b].min(f);
        }
    }
    let mut clamped = 0usize;
    for i in 0..n {
        if candidate[i] && scale[i] < 1.0 {
            d[i] *= scale[i];
            clamped += 1;
        }
    }
    let mut no_space = 0usize;
    for e in 0..edge_count {
        let (a, b) = (e, (e + 1) % n);
        if !is_line(e) {
            continue;
        }
        if d[a] + d[b] > edge_len(e) * (1.0 + 1e-9) {
            // Solo falla un lado (el otro es 0): ese no cabe.
            for i in [a, b] {
                if d[i] > 0.0 && candidate[i] {
                    d[i] = 0.0;
                    no_space += 1;
                    candidate[i] = false;
                }
            }
        }
    }

    let mut outs: Vec<NodeOut> = Vec::with_capacity(n);
    let mut rounded = 0usize;
    for i in 0..n {
        let v = sub.nodes[i];
        if !candidate[i] || d[i] < 1e-7 {
            outs.push(NodeOut {
                enter: v,
                exit: v,
                join: Join::None,
            });
            continue;
        }
        let (prev, next) = (sub.nodes[(i + n - 1) % n], sub.nodes[(i + 1) % n]);
        let (u, w) = (unit_from(v, prev), unit_from(v, next));
        let (t1, t2) = (
            Point::new(v.x + u.x * d[i], v.y + u.y * d[i]),
            Point::new(v.x + w.x * d[i], v.y + w.y * d[i]),
        );
        let join = match opts.style {
            FilletStyle::Chamfer => Join::Line,
            FilletStyle::Bite => {
                let a0 = (t1.y - v.y).atan2(t1.x - v.x);
                let a1 = (t2.y - v.y).atan2(t2.x - v.x);
                Join::Arc(arc_cubics(v, d[i], a0, normalize_angle(a1 - a0)))
            }
            FilletStyle::Round => {
                let th = theta[i];
                let r = d[i] * (th / 2.0).tan();
                let bis = {
                    let (bx, by) = (u.x + w.x, u.y + w.y);
                    let l = bx.hypot(by);
                    Point::new(bx / l, by / l)
                };
                let dist = r / (th / 2.0).sin();
                let c = Point::new(v.x + bis.x * dist, v.y + bis.y * dist);
                let a0 = (t1.y - c.y).atan2(t1.x - c.x);
                let a1 = (t2.y - c.y).atan2(t2.x - c.x);
                Join::Arc(arc_cubics(c, r, a0, normalize_angle(a1 - a0)))
            }
        };
        rounded += 1;
        outs.push(NodeOut {
            enter: t1,
            exit: t2,
            join,
        });
    }

    let push_join = |segs: &mut Vec<PathSeg>, o: &NodeOut| match &o.join {
        Join::None => {}
        Join::Line => segs.push(PathSeg::LineTo { to: o.exit }),
        Join::Arc(cubics) => {
            for c in cubics {
                segs.push(PathSeg::CubicTo {
                    ctrl1: c[0],
                    ctrl2: c[1],
                    to: c[2],
                });
            }
            // El último punto del arco es `exit` salvo por redondeo: se fuerza para cerrar bien.
            if let Some(PathSeg::CubicTo { to, .. }) = segs.last_mut() {
                *to = o.exit;
            }
        }
    };

    let start = outs[0].exit;
    let mut segs: Vec<PathSeg> = vec![PathSeg::MoveTo { to: start }];
    for e in 0..edge_count {
        let j = (e + 1) % n;
        match sub.edges[e] {
            Edge::Line => {
                let skip = sub.closed && j == 0 && matches!(outs[0].join, Join::None);
                if !skip {
                    segs.push(PathSeg::LineTo { to: outs[j].enter });
                }
            }
            Edge::Cubic(c1, c2) => segs.push(PathSeg::CubicTo {
                ctrl1: c1,
                ctrl2: c2,
                to: outs[j].enter,
            }),
        }
        push_join(&mut segs, &outs[j]);
    }
    if sub.closed {
        segs.push(PathSeg::Close);
    }
    Some((segs, rounded, clamped, no_space))
}
