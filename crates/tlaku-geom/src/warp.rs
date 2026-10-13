//! Deformación por envolvente (Warp / Deform): una región rectangular de origen se lleva a una
//! forma de cuatro bordes Bézier cúbicos (parche de Coons). Cada borde de la envolvente se puede
//! curvar libremente y las cuatro esquinas se pueden mover: con eso se hacen arcos, abombados,
//! pellizcos y perspectivas.
//!
//! Las curvas no se deforman moviendo solo sus puntos de control (eso no es la deformación real):
//! cada tramo se parte en trozos, se evalúa la deformación en los extremos de cada trozo y en su
//! tangente (derivada numérica de la deformación) y se vuelve a ajustar una cúbica de Hermite. Si la
//! cúbica se aparta más de `tolerance` de la curva deformada de verdad, el trozo se parte otra vez.

use tlaku_core::bezpath::{BezPath, PathSeg};
use tlaku_core::{Bounds, Point};

/// Borde Bézier cúbico: `[inicio, control 1, control 2, fin]`.
pub type Edge = [Point; 4];

/// Hasta cuántas veces se parte un trozo para ajustar la deformación.
const MAX_DEPTH: u32 = 7;
/// Trozos iniciales por segmento (los lados rectos también se curvan al deformar).
const BASE_PIECES: usize = 4;
/// Paso (en el espacio normalizado del origen) de la derivada numérica.
const DELTA: f64 = 1e-5;

/// Envolvente de cuatro bordes. `bottom` y `top` van de izquierda a derecha, `left` y `right` de
/// abajo hacia arriba. Las esquinas las mandan `bottom` y `top`: los extremos de `left` y `right`
/// se ajustan a ellas.
#[derive(Debug, Clone, PartialEq)]
pub struct Envelope {
    pub bottom: Edge,
    pub top: Edge,
    pub left: Edge,
    pub right: Edge,
}

fn cubic(e: &Edge, t: f64) -> Point {
    let u = 1.0 - t;
    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    Point::new(
        a * e[0].x + b * e[1].x + c * e[2].x + d * e[3].x,
        a * e[0].y + b * e[1].y + c * e[2].y + d * e[3].y,
    )
}

fn shift(p: Point, by: Point) -> Point {
    Point::new(p.x + by.x, p.y + by.y)
}

fn diff(a: Point, b: Point) -> Point {
    Point::new(a.x - b.x, a.y - b.y)
}

impl Envelope {
    /// Envolvente que deja la región `b` tal cual (identidad).
    pub fn identity(b: Bounds) -> Self {
        let line = |a: Point, z: Point| {
            [
                a,
                Point::new(a.x + (z.x - a.x) / 3.0, a.y + (z.y - a.y) / 3.0),
                Point::new(a.x + 2.0 * (z.x - a.x) / 3.0, a.y + 2.0 * (z.y - a.y) / 3.0),
                z,
            ]
        };
        let (bl, br) = (b.min, Point::new(b.max.x, b.min.y));
        let (tl, tr) = (Point::new(b.min.x, b.max.y), b.max);
        Self {
            bottom: line(bl, br),
            top: line(tl, tr),
            left: line(bl, tl),
            right: line(br, tr),
        }
    }

    /// Ajusta los extremos de `left` y `right` a las esquinas de `bottom` y `top`, moviendo con
    /// ellos sus puntos de control para que la forma del borde no cambie.
    pub fn snapped(mut self) -> Self {
        let fix = |e: &mut Edge, start: Point, end: Point| {
            let (ds, de) = (diff(start, e[0]), diff(end, e[3]));
            e[0] = start;
            e[1] = shift(e[1], ds);
            e[2] = shift(e[2], de);
            e[3] = end;
        };
        fix(&mut self.left, self.bottom[0], self.top[0]);
        fix(&mut self.right, self.bottom[3], self.top[3]);
        self
    }

    /// Imagen del punto `(u, v)` del cuadrado unidad: parche de Coons.
    pub fn at(&self, u: f64, v: f64) -> Point {
        let (b, t, l, r) = (
            cubic(&self.bottom, u),
            cubic(&self.top, u),
            cubic(&self.left, v),
            cubic(&self.right, v),
        );
        let (c00, c10, c01, c11) = (self.bottom[0], self.bottom[3], self.top[0], self.top[3]);
        let corner = |c: Point, w: f64| (c.x * w, c.y * w);
        let parts = [
            corner(c00, (1.0 - u) * (1.0 - v)),
            corner(c10, u * (1.0 - v)),
            corner(c01, (1.0 - u) * v),
            corner(c11, u * v),
        ];
        let (cx, cy) = parts
            .iter()
            .fold((0.0, 0.0), |(sx, sy), (x, y)| (sx + x, sy + y));
        Point::new(
            (1.0 - v) * b.x + v * t.x + (1.0 - u) * l.x + u * r.x - cx,
            (1.0 - v) * b.y + v * t.y + (1.0 - u) * l.y + u * r.y - cy,
        )
    }
}

/// Deformación de la región `source` hacia la envolvente `env`.
pub struct Warp {
    env: Envelope,
    source: Bounds,
    w: f64,
    h: f64,
}

impl Warp {
    /// `None` si la región de origen no tiene ancho y alto positivos y finitos.
    pub fn new(source: Bounds, env: Envelope) -> Option<Self> {
        let (w, h) = (source.max.x - source.min.x, source.max.y - source.min.y);
        let ok = w.is_finite() && h.is_finite() && w > 1e-9 && h > 1e-9;
        ok.then(|| Self {
            env: env.snapped(),
            source,
            w,
            h,
        })
    }

    fn uv(&self, p: Point) -> (f64, f64) {
        (
            (p.x - self.source.min.x) / self.w,
            (p.y - self.source.min.y) / self.h,
        )
    }

    /// Imagen de un punto del documento.
    pub fn map(&self, p: Point) -> Point {
        let (u, v) = self.uv(p);
        self.env.at(u, v)
    }

    /// Jacobiana de la deformación en `p`: `(∂/∂x, ∂/∂y)` de la imagen.
    fn jacobian(&self, p: Point) -> (Point, Point) {
        let (u, v) = self.uv(p);
        let du = self.env.at(u + DELTA, v);
        let dm = self.env.at(u - DELTA, v);
        let dv = self.env.at(u, v + DELTA);
        let dn = self.env.at(u, v - DELTA);
        let kx = 1.0 / (2.0 * DELTA * self.w);
        let ky = 1.0 / (2.0 * DELTA * self.h);
        (
            Point::new((du.x - dm.x) * kx, (du.y - dm.y) * kx),
            Point::new((dv.x - dn.x) * ky, (dv.y - dn.y) * ky),
        )
    }

    /// Imagen del vector tangente `d` aplicado en `p`.
    fn tangent(&self, p: Point, d: Point) -> Point {
        let (jx, jy) = self.jacobian(p);
        Point::new(jx.x * d.x + jy.x * d.y, jx.y * d.x + jy.y * d.y)
    }
}

/// Segmento de origen en forma cúbica con su derivada, para evaluar con `t` en `[0, 1]`.
#[derive(Clone, Copy)]
struct Src {
    p: [Point; 4],
}

impl Src {
    fn line(a: Point, b: Point) -> Self {
        let third = |t: f64| Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
        Self {
            p: [a, third(1.0 / 3.0), third(2.0 / 3.0), b],
        }
    }

    fn quad(a: Point, c: Point, b: Point) -> Self {
        let k = 2.0 / 3.0;
        Self {
            p: [
                a,
                Point::new(a.x + (c.x - a.x) * k, a.y + (c.y - a.y) * k),
                Point::new(b.x + (c.x - b.x) * k, b.y + (c.y - b.y) * k),
                b,
            ],
        }
    }

    fn at(&self, t: f64) -> Point {
        cubic(&self.p, t)
    }

    fn deriv(&self, t: f64) -> Point {
        let u = 1.0 - t;
        let q = &self.p;
        let (a, b, c) = (3.0 * u * u, 6.0 * u * t, 3.0 * t * t);
        Point::new(
            a * (q[1].x - q[0].x) + b * (q[2].x - q[1].x) + c * (q[3].x - q[2].x),
            a * (q[1].y - q[0].y) + b * (q[2].y - q[1].y) + c * (q[3].y - q[2].y),
        )
    }
}

fn hermite(w: &Warp, s: &Src, t0: f64, t1: f64) -> [Point; 4] {
    let dt = (t1 - t0) / 3.0;
    let (a, b) = (s.at(t0), s.at(t1));
    let (da, db) = (w.tangent(a, s.deriv(t0)), w.tangent(b, s.deriv(t1)));
    let (pa, pb) = (w.map(a), w.map(b));
    [
        pa,
        Point::new(pa.x + da.x * dt, pa.y + da.y * dt),
        Point::new(pb.x - db.x * dt, pb.y - db.y * dt),
        pb,
    ]
}

/// ¿La cúbica ajustada se aparta de la curva deformada en 1/4, 1/2 y 3/4 del trozo?
fn fits(w: &Warp, s: &Src, t0: f64, t1: f64, c: &[Point; 4], tol: f64) -> bool {
    [0.25, 0.5, 0.75].iter().all(|&k| {
        let truth = w.map(s.at(t0 + (t1 - t0) * k));
        cubic(c, k).dist(truth) <= tol
    })
}

fn emit(w: &Warp, s: &Src, t0: f64, t1: f64, tol: f64, depth: u32, out: &mut Vec<PathSeg>) {
    let c = hermite(w, s, t0, t1);
    if depth >= MAX_DEPTH || fits(w, s, t0, t1, &c, tol) {
        out.push(PathSeg::CubicTo {
            ctrl1: c[1],
            ctrl2: c[2],
            to: c[3],
        });
    } else {
        let mid = (t0 + t1) / 2.0;
        emit(w, s, t0, mid, tol, depth + 1, out);
        emit(w, s, mid, t1, tol, depth + 1, out);
    }
}

fn warp_segment(w: &Warp, s: &Src, tol: f64, out: &mut Vec<PathSeg>) {
    for i in 0..BASE_PIECES {
        let (t0, t1) = (
            i as f64 / BASE_PIECES as f64,
            (i + 1) as f64 / BASE_PIECES as f64,
        );
        emit(w, s, t0, t1, tol, 0, out);
    }
}

/// Deforma una trayectoria con curvas: cada tramo se vuelve una o más cúbicas que siguen la
/// deformación con error menor que `tolerance` (mm). Conserva los subtrayectos y los cierres.
pub fn warp_path(path: &BezPath, w: &Warp, tolerance: f64) -> BezPath {
    let tol = tolerance.max(1e-6);
    let mut out = Vec::with_capacity(path.0.len() * 4);
    let (mut start, mut cur): (Option<Point>, Option<Point>) = (None, None);
    for seg in &path.0 {
        match *seg {
            PathSeg::MoveTo { to } => {
                out.push(PathSeg::MoveTo { to: w.map(to) });
                (start, cur) = (Some(to), Some(to));
            }
            PathSeg::LineTo { to } => {
                if let Some(a) = cur {
                    warp_segment(w, &Src::line(a, to), tol, &mut out);
                }
                cur = Some(to);
            }
            PathSeg::QuadTo { ctrl, to } => {
                if let Some(a) = cur {
                    warp_segment(w, &Src::quad(a, ctrl, to), tol, &mut out);
                }
                cur = Some(to);
            }
            PathSeg::CubicTo { ctrl1, ctrl2, to } => {
                if let Some(a) = cur {
                    let s = Src {
                        p: [a, ctrl1, ctrl2, to],
                    };
                    warp_segment(w, &s, tol, &mut out);
                }
                cur = Some(to);
            }
            PathSeg::Close => {
                // El cierre dibuja una recta hasta el inicio: al deformarse deja de ser recta.
                if let (Some(a), Some(s)) = (cur, start)
                    && a.dist(s) > 1e-9
                {
                    warp_segment(w, &Src::line(a, s), tol, &mut out);
                }
                out.push(PathSeg::Close);
                cur = start;
            }
        }
    }
    BezPath(out)
}
