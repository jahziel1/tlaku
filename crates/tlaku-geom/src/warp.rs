//! Deformación por envolvente (Warp / Deform): una región rectangular de origen se lleva a una
//! forma de cuatro bordes Bézier cúbicos (parche de Coons). Cada borde de la envolvente se puede
//! curvar libremente y las cuatro esquinas se pueden mover: con eso se hacen arcos, abombados,
//! pellizcos y perspectivas.
//!
//! Las curvas no se deforman moviendo solo sus puntos de control (eso no es la deformación real):
//! cada tramo se parte en trozos, se evalúa la deformación en los extremos de cada trozo y en su
//! tangente (derivada numérica de la deformación) y se vuelve a ajustar una cúbica de Hermite. Si la
//! cúbica se aparta más de `tolerance` de la curva deformada de verdad, el trozo se parte otra vez.
//!
//! Además del parche de Coons hay otras dos superficies, que comparten ese ajuste de curvas:
//! - `Warp::perspective`: proyección de cuatro esquinas (las rectas siguen siendo rectas).
//! - `Warp::mesh`: malla de 4x4 puntos por los que pasa la superficie (interpolación cúbica de
//!   Lagrange en cada eje, determinista y sin pasos iterativos).

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

/// Malla de control de 4x4 puntos: `grid[j][i]` es la imagen del punto `(i/3, j/3)` del cuadrado
/// unidad de origen (`i` crece hacia la derecha y `j` hacia arriba).
pub type Grid = [[Point; 4]; 4];

/// Pesos de la interpolación cúbica de Lagrange en los nodos `0, 1/3, 2/3, 1`.
fn lagrange(t: f64) -> [f64; 4] {
    let (a, b, c) = (t - 1.0 / 3.0, t - 2.0 / 3.0, t - 1.0);
    [
        -4.5 * a * b * c,
        13.5 * t * b * c,
        -13.5 * t * a * c,
        4.5 * t * a * b,
    ]
}

fn mesh_at(g: &Grid, u: f64, v: f64) -> Point {
    let (lu, lv) = (lagrange(u), lagrange(v));
    let (mut x, mut y) = (0.0, 0.0);
    for (j, row) in g.iter().enumerate() {
        for (i, p) in row.iter().enumerate() {
            let w = lu[i] * lv[j];
            x += p.x * w;
            y += p.y * w;
        }
    }
    Point::new(x, y)
}

/// Coeficientes `[a, b, c, d, e, f, g, h]` de la proyección
/// `(x, y) = ((a u + b v + c) / (g u + h v + 1), (d u + e v + f) / (g u + h v + 1))`.
fn homography(q: &[Point; 4]) -> Option<[f64; 8]> {
    let (p0, p1, p2, p3) = (q[0], q[1], q[2], q[3]);
    let (dx1, dx2) = (p1.x - p2.x, p3.x - p2.x);
    let (dy1, dy2) = (p1.y - p2.y, p3.y - p2.y);
    let (sx, sy) = (p0.x - p1.x + p2.x - p3.x, p0.y - p1.y + p2.y - p3.y);
    let scale = [dx1, dx2, dy1, dy2]
        .iter()
        .fold(1.0f64, |m, v| m.max(v.abs()));
    let (g, h) = if sx.abs() <= 1e-12 * scale && sy.abs() <= 1e-12 * scale {
        (0.0, 0.0)
    } else {
        let den = dx1 * dy2 - dx2 * dy1;
        if den.abs() <= 1e-12 * scale * scale {
            return None;
        }
        ((sx * dy2 - dx2 * sy) / den, (dx1 * sy - sx * dy1) / den)
    };
    // El divisor es lineal en (u, v): basta mirarlo en las cuatro esquinas del origen.
    if [1.0, 1.0 + g, 1.0 + h, 1.0 + g + h]
        .iter()
        .any(|d| !d.is_finite() || *d <= 1e-6)
    {
        return None;
    }
    Some([
        p1.x - p0.x + g * p1.x,
        p3.x - p0.x + h * p3.x,
        p0.x,
        p1.y - p0.y + g * p1.y,
        p3.y - p0.y + h * p3.y,
        p0.y,
        g,
        h,
    ])
}

fn projective_at(m: &[f64; 8], u: f64, v: f64) -> Point {
    let den = m[6] * u + m[7] * v + 1.0;
    Point::new(
        (m[0] * u + m[1] * v + m[2]) / den,
        (m[3] * u + m[4] * v + m[5]) / den,
    )
}

/// Superficie a la que se lleva el cuadrado unidad de origen.
enum Surface {
    Coons(Envelope),
    Mesh(Box<Grid>),
    Perspective([f64; 8]),
}

impl Surface {
    fn at(&self, u: f64, v: f64) -> Point {
        match self {
            Surface::Coons(e) => e.at(u, v),
            Surface::Mesh(g) => mesh_at(g, u, v),
            Surface::Perspective(m) => projective_at(m, u, v),
        }
    }
}

/// Deformación de la región `source` hacia una superficie (envolvente, malla o perspectiva).
pub struct Warp {
    surface: Surface,
    source: Bounds,
    w: f64,
    h: f64,
}

impl Warp {
    /// `None` si la región de origen no tiene ancho y alto positivos y finitos.
    pub fn new(source: Bounds, env: Envelope) -> Option<Self> {
        Self::with_surface(source, Surface::Coons(env.snapped()))
    }

    /// Malla de 4x4 puntos: la superficie pasa por cada uno (ver [`Grid`]). `None` si la región de
    /// origen es degenerada o algún punto no es finito.
    pub fn mesh(source: Bounds, grid: Grid) -> Option<Self> {
        let finite = grid
            .iter()
            .flatten()
            .all(|p| p.x.is_finite() && p.y.is_finite());
        if !finite {
            return None;
        }
        Self::with_surface(source, Surface::Mesh(Box::new(grid)))
    }

    /// Proyección que lleva las esquinas de `source` (abajo-izquierda, abajo-derecha,
    /// arriba-derecha, arriba-izquierda) a `corners` en ese mismo orden. `None` si el
    /// cuadrilátero es degenerado o no es convexo (la proyección dejaría de estar definida).
    pub fn perspective(source: Bounds, corners: [Point; 4]) -> Option<Self> {
        let finite = corners.iter().all(|p| p.x.is_finite() && p.y.is_finite());
        if !finite {
            return None;
        }
        Self::with_surface(source, Surface::Perspective(homography(&corners)?))
    }

    fn with_surface(source: Bounds, surface: Surface) -> Option<Self> {
        let (w, h) = (source.max.x - source.min.x, source.max.y - source.min.y);
        let ok = w.is_finite() && h.is_finite() && w > 1e-9 && h > 1e-9;
        ok.then_some(Self {
            surface,
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
        self.surface.at(u, v)
    }

    /// Jacobiana de la deformación en `p`: `(∂/∂x, ∂/∂y)` de la imagen.
    fn jacobian(&self, p: Point) -> (Point, Point) {
        let (u, v) = self.uv(p);
        let du = self.surface.at(u + DELTA, v);
        let dm = self.surface.at(u - DELTA, v);
        let dv = self.surface.at(u, v + DELTA);
        let dn = self.surface.at(u, v - DELTA);
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
