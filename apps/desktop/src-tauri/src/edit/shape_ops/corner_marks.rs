//! Esquinas guardadas por la herramienta de redondear/achaflanar (D60).
//!
//! Un chaflán es una recta entre dos rectas: por geometría no se distingue de un lado recto, así
//! que no se puede devolver a punta. Por eso, al aplicar el redondeo o el chaflán, el objeto
//! guarda en `VectorObject::corners` la punta original y los dos nodos donde empieza y termina la
//! forma nueva. «Revertir» usa esas marcas si hay alguna cerca del clic; si no, vuelve a la
//! detección geométrica de `tlaku_geom::unfillet_path`.
//!
//! Una marca solo vale mientras la geometría siga siendo la que dejó la herramienta (`find_run`):
//! los dos nodos existen y los une una sola recta o una cadena de cúbicas, y la recta que llega al
//! primero y la que sale del segundo siguen pasando por la punta. Si el usuario mueve, borra o
//! agrega nodos en esa zona, la marca deja de valer y se descarta (`prune`).
//!
//! Las marcas del objeto están en coordenadas locales; aquí se trabaja en coordenadas de
//! documento (las de `DesignObject::curves`), con `world_marks` para pasar de unas a otras.

use tlaku_core::Point;
use tlaku_core::bezpath::PathSeg;
use tlaku_doc::{BezPath, CornerMark, DesignObject, ObjectKind, affine};
use tlaku_geom::{FilletCorner, split_subpaths};

/// Tolerancia (mm) para decir que un nodo está donde la marca lo dejó.
const NODE_TOL: f64 = 1e-4;
/// Tolerancia (mm) para decir que la punta sigue sobre la recta que llega a la esquina.
const LINE_TOL: f64 = 1e-3;

#[derive(Clone, Copy)]
enum Edge {
    Line,
    Cubic(Point, Point),
}

/// Subtrayecto como nodos y aristas: `edges[i]` va de `nodes[i]` a `nodes[(i + 1) % n]` (en uno
/// abierto no existe la última).
struct Ring {
    nodes: Vec<Point>,
    edges: Vec<Edge>,
    closed: bool,
}

fn parse(sub: &BezPath) -> Option<Ring> {
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
                if nodes.last()?.dist(to) > 1e-9 {
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
    if nodes.len() >= 4 && nodes[0].dist(nodes[nodes.len() - 1]) < 1e-9 {
        nodes.pop();
        closed = true;
    } else if closed {
        edges.push(Edge::Line);
    }
    if closed && nodes.len() < 3 {
        return None;
    }
    Some(Ring {
        nodes,
        edges,
        closed,
    })
}

impl Ring {
    fn to_path(&self) -> BezPath {
        let n = self.nodes.len();
        let mut segs = vec![PathSeg::MoveTo { to: self.nodes[0] }];
        for (e, edge) in self.edges.iter().enumerate() {
            let to = self.nodes[(e + 1) % n];
            match *edge {
                // La última recta de un subtrayecto cerrado la hace `Close`.
                Edge::Line if self.closed && e + 1 == self.edges.len() => {}
                Edge::Line => segs.push(PathSeg::LineTo { to }),
                Edge::Cubic(ctrl1, ctrl2) => segs.push(PathSeg::CubicTo { ctrl1, ctrl2, to }),
            }
        }
        if self.closed {
            segs.push(PathSeg::Close);
        }
        BezPath(segs)
    }

    fn next(&self, i: usize) -> Option<usize> {
        let n = self.nodes.len();
        if i + 1 < n {
            Some(i + 1)
        } else if self.closed {
            Some(0)
        } else {
            None
        }
    }

    fn prev(&self, i: usize) -> Option<usize> {
        let n = self.nodes.len();
        if i > 0 {
            Some(i - 1)
        } else if self.closed {
            Some(n - 1)
        } else {
            None
        }
    }

    fn node_near(&self, p: Point) -> Option<usize> {
        self.nodes.iter().position(|q| q.dist(p) <= NODE_TOL)
    }
}

/// La forma que sustituyó a la esquina dentro de un subtrayecto: de `first` a `last` siguiendo el
/// sentido del trazo, con los nodos de en medio (cúbicas) o ninguno (un chaflán).
struct Run {
    first: usize,
    last: usize,
    /// Cuántas aristas hay de `first` a `last`.
    len: usize,
}

/// Distancia de `p` a la recta que pasa por `a` y `b`.
fn dist_to_line(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l = dx.hypot(dy);
    if l < 1e-12 {
        return p.dist(a);
    }
    ((p.x - a.x) * dy - (p.y - a.y) * dx).abs() / l
}

fn dot(a: Point, b: Point, c: Point) -> f64 {
    (b.x - a.x) * (c.x - a.x) + (b.y - a.y) * (c.y - a.y)
}

/// La punta cae en la prolongación de la recta `from -> to`, más allá de `to`.
fn vertex_ahead(from: Point, to: Point, v: Point) -> bool {
    dist_to_line(v, from, to) <= LINE_TOL && dot(to, to_dir(from, to), v) > 0.0
}

/// Punto `to + (to - from)`: sirve para medir el avance en el sentido de la recta.
fn to_dir(from: Point, to: Point) -> Point {
    Point::new(2.0 * to.x - from.x, 2.0 * to.y - from.y)
}

/// Busca en `ring` la forma de `mark` (con la punta en `v`): de `a` a `b` o de `b` a `a`.
fn find_run(ring: &Ring, a: Point, b: Point, v: Point) -> Option<Run> {
    let ia = ring.node_near(a)?;
    let ib = ring.node_near(b)?;
    if ia == ib {
        return None;
    }
    for (first, last) in [(ia, ib), (ib, ia)] {
        // Recorre aristas desde `first` hasta `last`.
        let mut i = first;
        let mut len = 0;
        let (mut lines, mut cubics) = (0, 0);
        let reached = loop {
            if i == last && len > 0 {
                break true;
            }
            if len > ring.nodes.len() || i >= ring.edges.len() {
                break false;
            }
            match ring.edges[i] {
                Edge::Line => lines += 1,
                Edge::Cubic(..) => cubics += 1,
            }
            len += 1;
            match ring.next(i) {
                Some(j) => i = j,
                None => break false,
            }
        };
        let shape_ok = (lines == 1 && cubics == 0) || (lines == 0 && cubics > 0);
        if !reached || !shape_ok {
            continue;
        }
        // La recta que llega a `first` y la que sale de `last` siguen pasando por la punta.
        let (Some(p), Some(q)) = (ring.prev(first), ring.next(last)) else {
            continue;
        };
        let line_in = ring.edges.get(p).is_some_and(|e| matches!(e, Edge::Line));
        let line_out = ring
            .edges
            .get(last)
            .is_some_and(|e| matches!(e, Edge::Line));
        if line_in
            && line_out
            && vertex_ahead(ring.nodes[p], ring.nodes[first], v)
            && vertex_ahead(ring.nodes[q], ring.nodes[last], v)
        {
            return Some(Run { first, last, len });
        }
    }
    None
}

fn point(p: [f64; 2]) -> Point {
    Point::new(p[0], p[1])
}

fn sub_rings(path: &BezPath) -> Vec<(BezPath, Option<Ring>)> {
    split_subpaths(path)
        .into_iter()
        .map(|s| {
            let ring = parse(&s);
            (s, ring)
        })
        .collect()
}

/// ¿Sigue valiendo la marca en estos trazos?
pub fn still_valid(paths: &[BezPath], mark: &CornerMark) -> bool {
    let (a, b, v) = (point(mark.from), point(mark.to), point(mark.vertex));
    paths.iter().any(|p| {
        sub_rings(p)
            .iter()
            .any(|(_, r)| r.as_ref().is_some_and(|r| find_run(r, a, b, v).is_some()))
    })
}

/// Las marcas que siguen valiendo en estos trazos.
pub fn prune(paths: &[BezPath], marks: Vec<CornerMark>) -> Vec<CornerMark> {
    marks
        .into_iter()
        .filter(|m| still_valid(paths, m))
        .collect()
}

/// Marcas del objeto en coordenadas de documento (le aplica su `transform`).
pub fn world_marks(o: &DesignObject) -> Vec<CornerMark> {
    let ObjectKind::Vector(v) = &o.kind else {
        return Vec::new();
    };
    let place = |p: [f64; 2]| affine::apply(&o.transform, p);
    v.corners
        .iter()
        .map(|m| CornerMark {
            vertex: place(m.vertex),
            from: place(m.from),
            to: place(m.to),
        })
        .collect()
}

/// Marca nueva de una esquina que acaba de modificar `fillet_path`.
pub fn mark_of(c: &FilletCorner) -> CornerMark {
    CornerMark {
        vertex: [c.vertex.x, c.vertex.y],
        from: [c.from.x, c.from.y],
        to: [c.to.x, c.to.y],
    }
}

/// Una esquina devuelta a punta con sus marcas.
pub struct Reverted {
    /// Los trazos con la esquina otra vez en punta.
    pub paths: Vec<BezPath>,
    /// Índice de la marca usada.
    pub mark: usize,
    /// Distancia (mm) del clic a la esquina.
    pub distance: f64,
}

fn dist_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    if l2 < 1e-18 {
        return p.dist(a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / l2).clamp(0.0, 1.0);
    p.dist(Point::new(a.x + t * dx, a.y + t * dy))
}

/// Distancia del clic a la forma que sustituyó la esquina (o a la punta original).
fn click_distance(ring: &Ring, run: &Run, v: Point, click: Point) -> f64 {
    let n = ring.nodes.len();
    let mut best = click.dist(v);
    let mut i = run.first;
    for _ in 0..run.len {
        let a = ring.nodes[i];
        let b = ring.nodes[(i + 1) % n];
        match ring.edges[i] {
            Edge::Line => best = best.min(dist_to_segment(click, a, b)),
            Edge::Cubic(c1, c2) => {
                // Muestras de la cúbica: bastan para decidir cuál esquina se tocó.
                let mut prev = a;
                for k in 1..=8 {
                    let t = f64::from(k) / 8.0;
                    let u = 1.0 - t;
                    let q = Point::new(
                        u * u * u * a.x
                            + 3.0 * u * u * t * c1.x
                            + 3.0 * u * t * t * c2.x
                            + t * t * t * b.x,
                        u * u * u * a.y
                            + 3.0 * u * u * t * c1.y
                            + 3.0 * u * t * t * c2.y
                            + t * t * t * b.y,
                    );
                    best = best.min(dist_to_segment(click, prev, q));
                    prev = q;
                }
            }
        }
        i = (i + 1) % n;
    }
    best
}

/// Pone la punta `v` en lugar de la forma `run`.
fn restore(ring: &Ring, run: &Run, v: Point) -> Ring {
    let n = ring.nodes.len();
    // Nodos de en medio (los que se quitan): los que quedan estrictamente entre `first` y `last`.
    let mut inner = vec![false; n];
    let mut i = ring.next(run.first);
    while let Some(k) = i {
        if k == run.last {
            break;
        }
        inner[k] = true;
        i = ring.next(k);
    }
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for (k, &skip) in inner.iter().enumerate().take(n) {
        if skip {
            continue;
        }
        nodes.push(ring.nodes[k]);
        if k == run.first {
            edges.push(Edge::Line);
            nodes.push(v);
            edges.push(Edge::Line);
        } else if k < ring.edges.len() {
            edges.push(ring.edges[k]);
        }
    }
    Ring {
        nodes,
        edges,
        closed: ring.closed,
    }
}

/// Devuelve a punta la esquina marcada más cercana al clic, a lo más a `max_dist` mm. Las marcas
/// que ya no valen se ignoran. `None` si ninguna sirve.
pub fn revert_nearest(
    paths: &[BezPath],
    marks: &[CornerMark],
    click: Point,
    max_dist: f64,
) -> Option<Reverted> {
    // (trayecto, subtrayecto, marca, distancia)
    let mut best: Option<(usize, usize, usize, f64)> = None;
    let rings: Vec<Vec<(BezPath, Option<Ring>)>> = paths.iter().map(sub_rings).collect();
    for (pi, subs) in rings.iter().enumerate() {
        for (si, (_, ring)) in subs.iter().enumerate() {
            let Some(ring) = ring else { continue };
            for (mi, m) in marks.iter().enumerate() {
                let v = point(m.vertex);
                if let Some(run) = find_run(ring, point(m.from), point(m.to), v) {
                    let d = click_distance(ring, &run, v, click);
                    if d <= max_dist && best.is_none_or(|b| d < b.3) {
                        best = Some((pi, si, mi, d));
                    }
                }
            }
        }
    }
    let (pi, si, mi, distance) = best?;
    let m = &marks[mi];
    let (a, b, v) = (point(m.from), point(m.to), point(m.vertex));
    let mut out = paths.to_vec();
    let mut joined = BezPath(Vec::new());
    for (k, (raw, ring)) in rings[pi].iter().enumerate() {
        if k == si {
            let ring = ring.as_ref()?;
            let run = find_run(ring, a, b, v)?;
            joined.0.extend(restore(ring, &run, v).to_path().0);
        } else {
            joined.0.extend(raw.0.iter().copied());
        }
    }
    out[pi] = joined;
    Some(Reverted {
        paths: out,
        mark: mi,
        distance,
    })
}
