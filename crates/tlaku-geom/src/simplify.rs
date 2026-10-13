//! Optimizar formas: menos nodos con un error máximo conocido.
//!
//! Pasos por trayectoria (todos acotados por `tolerance`, en mm):
//! 1. Quita puntos repetidos y colineales (`remove_redundant_points`).
//! 2. Simplifica con Douglas-Peucker entre las esquinas: ningún punto original queda a más de
//!    `tolerance` de la polilínea resultante.
//! 3. Opcional (`smooth`): ajusta curvas cúbicas a los tramos suaves entre esquinas. Una curva
//!    se acepta solo si el ajuste queda dentro de `tolerance` (comprobado también en sentido
//!    contrario, de la curva a los puntos) y usa menos nodos que la polilínea simplificada. Al
//!    ajustar una sola cúbica sobre tramos que ya eran casi tangentes, estos se funden en una
//!    curva continua (así se "unen" segmentos casi tangentes).
//!
//! El ajuste de cúbicas sigue la idea clásica de mínimos cuadrados con reparametrización y
//! partición en el punto de mayor error (Schneider, "An Algorithm for Automatically Fitting
//! Digitized Curves", Graphics Gems, 1990); es una implementación propia a partir de la
//! descripción del método.

use crate::util::remove_redundant_points;
use tlaku_core::bezpath::{BezPath, PathSeg};
use tlaku_core::{Point, Polyline};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OptimizeOptions {
    /// Error máximo permitido (mm). Con 0 solo se quitan puntos repetidos y colineales.
    pub tolerance: f64,
    /// Ajustar curvas cúbicas a los tramos suaves.
    pub smooth: bool,
    /// Un vértice cuyo giro supera este ángulo (grados) es una esquina que se conserva.
    pub corner_angle_deg: f64,
}

impl Default for OptimizeOptions {
    fn default() -> Self {
        Self {
            tolerance: 0.02,
            smooth: false,
            corner_angle_deg: 35.0,
        }
    }
}

/// Nodos de una trayectoria: el inicio de cada subtrayecto más el destino de cada segmento, sin
/// contar dos veces el punto donde un subtrayecto cerrado vuelve a su inicio.
pub fn node_count(path: &BezPath) -> usize {
    // Por subtrayecto: (nodos, inicio, último destino, cerrado).
    let mut subs: Vec<(usize, Point, Point, bool)> = Vec::new();
    for seg in &path.0 {
        match *seg {
            PathSeg::MoveTo { to } => subs.push((1, to, to, false)),
            PathSeg::LineTo { to } | PathSeg::QuadTo { to, .. } | PathSeg::CubicTo { to, .. } => {
                if let Some(s) = subs.last_mut() {
                    s.0 += 1;
                    s.2 = to;
                }
            }
            PathSeg::Close => {
                if let Some(s) = subs.last_mut() {
                    s.3 = true;
                }
            }
        }
    }
    subs.iter()
        .map(|&(n, start, last, closed)| {
            if closed && n > 1 && start.dist(last) < 1e-9 {
                n - 1
            } else {
                n
            }
        })
        .sum()
}

fn sub(a: Point, b: Point) -> Point {
    Point::new(a.x - b.x, a.y - b.y)
}
fn add(a: Point, b: Point) -> Point {
    Point::new(a.x + b.x, a.y + b.y)
}
fn mul(a: Point, k: f64) -> Point {
    Point::new(a.x * k, a.y * k)
}
fn dot(a: Point, b: Point) -> f64 {
    a.x * b.x + a.y * b.y
}
fn len(a: Point) -> f64 {
    a.x.hypot(a.y)
}
fn unit(a: Point) -> Point {
    let l = len(a);
    if l < 1e-12 {
        Point::new(1.0, 0.0)
    } else {
        mul(a, 1.0 / l)
    }
}

fn dist_to_seg(p: Point, a: Point, b: Point) -> f64 {
    let d = sub(b, a);
    let l2 = dot(d, d);
    if l2 == 0.0 {
        return p.dist(a);
    }
    let t = (dot(sub(p, a), d) / l2).clamp(0.0, 1.0);
    p.dist(add(a, mul(d, t)))
}

/// Douglas-Peucker sobre `pts[lo..=hi]` (tramo abierto). Devuelve los índices que se conservan,
/// ordenados, incluidos `lo` y `hi`. Iterativo: sirve con millones de puntos.
fn douglas_peucker(pts: &[Point], lo: usize, hi: usize, tol: f64) -> Vec<usize> {
    let mut keep = vec![false; hi - lo + 1];
    keep[0] = true;
    keep[hi - lo] = true;
    let mut stack = vec![(lo, hi)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let (mut worst, mut at) = (0.0, a);
        for (i, &p) in pts.iter().enumerate().take(b).skip(a + 1) {
            let d = dist_to_seg(p, pts[a], pts[b]);
            if d > worst {
                worst = d;
                at = i;
            }
        }
        if worst > tol {
            keep[at - lo] = true;
            stack.push((a, at));
            stack.push((at, b));
        }
    }
    keep.iter()
        .enumerate()
        .filter_map(|(i, &k)| k.then_some(lo + i))
        .collect()
}

// --- ajuste de cúbicas ---

type Cubic = [Point; 4];

fn bernstein(t: f64) -> [f64; 4] {
    let u = 1.0 - t;
    [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t]
}

fn eval(c: &Cubic, t: f64) -> Point {
    let b = bernstein(t);
    Point::new(
        b[0] * c[0].x + b[1] * c[1].x + b[2] * c[2].x + b[3] * c[3].x,
        b[0] * c[0].y + b[1] * c[1].y + b[2] * c[2].y + b[3] * c[3].y,
    )
}

fn eval_d1(c: &Cubic, t: f64) -> Point {
    let u = 1.0 - t;
    let d = [sub(c[1], c[0]), sub(c[2], c[1]), sub(c[3], c[2])];
    add(
        add(mul(d[0], 3.0 * u * u), mul(d[1], 6.0 * u * t)),
        mul(d[2], 3.0 * t * t),
    )
}

fn eval_d2(c: &Cubic, t: f64) -> Point {
    let u = 1.0 - t;
    let d = [
        add(sub(c[2], mul(c[1], 2.0)), c[0]),
        add(sub(c[3], mul(c[2], 2.0)), c[1]),
    ];
    add(mul(d[0], 6.0 * u), mul(d[1], 6.0 * t))
}

fn chord_params(pts: &[Point]) -> Vec<f64> {
    let mut u = vec![0.0; pts.len()];
    for i in 1..pts.len() {
        u[i] = u[i - 1] + pts[i].dist(pts[i - 1]);
    }
    let total = u[pts.len() - 1];
    if total > 0.0 {
        for v in &mut u {
            *v /= total;
        }
    }
    u
}

/// Cúbica por mínimos cuadrados con extremos fijos y tangentes unitarias `t1` (saliendo del
/// inicio) y `t2` (saliendo del final hacia dentro).
fn generate(pts: &[Point], u: &[f64], t1: Point, t2: Point) -> Cubic {
    let n = pts.len();
    let (p0, p3) = (pts[0], pts[n - 1]);
    let seg = p0.dist(p3);
    let (mut c00, mut c01, mut c11, mut x0, mut x1) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for i in 0..n {
        let b = bernstein(u[i]);
        let a1 = mul(t1, b[1]);
        let a2 = mul(t2, b[2]);
        c00 += dot(a1, a1);
        c01 += dot(a1, a2);
        c11 += dot(a2, a2);
        let base = add(mul(p0, b[0] + b[1]), mul(p3, b[2] + b[3]));
        let tmp = sub(pts[i], base);
        x0 += dot(a1, tmp);
        x1 += dot(a2, tmp);
    }
    let det = c00 * c11 - c01 * c01;
    let (mut al, mut ar) = if det.abs() > 1e-12 {
        ((x0 * c11 - x1 * c01) / det, (c00 * x1 - c01 * x0) / det)
    } else {
        (0.0, 0.0)
    };
    // Si el resultado es absurdo (negativo o diminuto) se usa la heurística del tercio.
    let floor = 1e-6 * seg;
    if al < floor || ar < floor || !al.is_finite() || !ar.is_finite() {
        al = seg / 3.0;
        ar = seg / 3.0;
    }
    [p0, add(p0, mul(t1, al)), add(p3, mul(t2, ar)), p3]
}

fn reparametrize(c: &Cubic, pts: &[Point], u: &[f64]) -> Vec<f64> {
    pts.iter()
        .zip(u)
        .map(|(&p, &t)| {
            let q = eval(c, t);
            let d1 = eval_d1(c, t);
            let d2 = eval_d2(c, t);
            let num = dot(sub(q, p), d1);
            let den = dot(d1, d1) + dot(sub(q, p), d2);
            if den.abs() < 1e-12 {
                t
            } else {
                (t - num / den).clamp(0.0, 1.0)
            }
        })
        .collect()
}

/// Mayor distancia (al cuadrado) entre los puntos y la curva a su parámetro, y el índice donde ocurre.
fn max_error(c: &Cubic, pts: &[Point], u: &[f64]) -> (f64, usize) {
    let mut worst = (0.0, pts.len() / 2);
    for i in 1..pts.len() - 1 {
        let d = eval(c, u[i]);
        let e = (d.x - pts[i].x).powi(2) + (d.y - pts[i].y).powi(2);
        if e > worst.0 {
            worst = (e, i);
        }
    }
    worst
}

fn fit_cubic(pts: &[Point], t1: Point, t2: Point, tol: f64, out: &mut Vec<Cubic>, depth: u32) {
    let n = pts.len();
    if n == 2 {
        let d = pts[0].dist(pts[1]) / 3.0;
        out.push([
            pts[0],
            add(pts[0], mul(t1, d)),
            add(pts[1], mul(t2, d)),
            pts[1],
        ]);
        return;
    }
    let mut u = chord_params(pts);
    let mut c = generate(pts, &u, t1, t2);
    let (mut err, mut split) = max_error(&c, pts, &u);
    if err <= tol * tol {
        out.push(c);
        return;
    }
    // Reparametrizar mejora el ajuste cuando el error ya es moderado.
    if err <= 16.0 * tol * tol {
        for _ in 0..4 {
            u = reparametrize(&c, pts, &u);
            c = generate(pts, &u, t1, t2);
            (err, split) = max_error(&c, pts, &u);
            if err <= tol * tol {
                out.push(c);
                return;
            }
        }
    }
    if depth > 40 || n < 4 {
        // No converge: una recta por tramo (el llamador comprueba el error de todos modos).
        out.push([pts[0], pts[0], pts[n - 1], pts[n - 1]]);
        return;
    }
    let split = split.clamp(1, n - 2);
    let center = unit(sub(pts[split - 1], pts[split + 1]));
    fit_cubic(&pts[..=split], t1, center, tol, out, depth + 1);
    fit_cubic(&pts[split..], mul(center, -1.0), t2, tol, out, depth + 1);
}

/// Distancia máxima de la cúbica a la polilínea `run` (muestreada), y viceversa de `run` a la curva.
fn fit_error(curves: &[Cubic], run: &[Point]) -> f64 {
    let mut worst: f64 = 0.0;
    let mut samples: Vec<Point> = Vec::new();
    for c in curves {
        for k in 0..=24 {
            samples.push(eval(c, k as f64 / 24.0));
        }
    }
    for s in &samples {
        let d = run
            .windows(2)
            .map(|w| dist_to_seg(*s, w[0], w[1]))
            .fold(f64::INFINITY, f64::min);
        worst = worst.max(d);
    }
    for &p in run {
        let d = samples
            .windows(2)
            .map(|w| dist_to_seg(p, w[0], w[1]))
            .fold(f64::INFINITY, f64::min);
        worst = worst.max(d);
    }
    worst
}

fn turning_deg(prev: Point, p: Point, next: Point) -> f64 {
    let (a, b) = (unit(sub(p, prev)), unit(sub(next, p)));
    dot(a, b).clamp(-1.0, 1.0).acos().to_degrees()
}

/// Optimiza una polilínea y devuelve la trayectoria resultante (rectas y, con `smooth`, cúbicas).
/// Las degeneradas (menos de 2 puntos, no finitas) se devuelven como están, en rectas.
pub fn optimize_polyline(path: &Polyline, opts: &OptimizeOptions) -> BezPath {
    let tol = if opts.tolerance.is_finite() && opts.tolerance > 0.0 {
        opts.tolerance
    } else {
        0.0
    };
    if path
        .points
        .iter()
        .any(|p| !p.x.is_finite() || !p.y.is_finite())
    {
        return BezPath::from(path);
    }
    let cleaned = remove_redundant_points(path, 1e-6_f64.min(tol.max(1e-9)));
    let min_pts = if cleaned.closed { 3 } else { 2 };
    if tol == 0.0 || cleaned.points.len() <= min_pts {
        return BezPath::from(&cleaned);
    }
    let pts = &cleaned.points;
    let n = pts.len();
    let closed = cleaned.closed;
    let corner_deg = opts.corner_angle_deg.clamp(1.0, 179.0);

    // Anclas: extremos (abiertas) y esquinas. Cerradas sin esquinas: dos puntos opuestos.
    let mut is_corner = vec![false; n];
    let mut anchors: Vec<usize> = Vec::new();
    for i in 0..n {
        let endpoint = !closed && (i == 0 || i == n - 1);
        let corner = !endpoint && {
            let (a, b) = (pts[(i + n - 1) % n], pts[(i + 1) % n]);
            turning_deg(a, pts[i], b) > corner_deg
        };
        is_corner[i] = endpoint || corner;
        if is_corner[i] {
            anchors.push(i);
        }
    }
    if closed && anchors.len() < 2 {
        let first = anchors.first().copied().unwrap_or(0);
        let far = (0..n)
            .max_by(|&a, &b| pts[first].dist(pts[a]).total_cmp(&pts[first].dist(pts[b])))
            .unwrap_or(0);
        for a in [first, far] {
            if !anchors.contains(&a) {
                anchors.push(a);
            }
        }
        anchors.sort_unstable();
        anchors.dedup();
    }

    // Cada tramo entre anclas consecutivas (en cerradas, también el que da la vuelta).
    let spans: Vec<(usize, usize)> = if closed {
        (0..anchors.len())
            .map(|k| (anchors[k], anchors[(k + 1) % anchors.len()]))
            .collect()
    } else {
        anchors.windows(2).map(|w| (w[0], w[1])).collect()
    };

    // Puntos del tramo (a, b) siguiendo el contorno; `b <= a` da la vuelta.
    let run_points = |a: usize, b: usize| -> Vec<Point> {
        let mut run = vec![pts[a]];
        let mut i = a;
        loop {
            i = (i + 1) % n;
            run.push(pts[i]);
            if i == b {
                break;
            }
        }
        run
    };
    let tangent_at = |i: usize, forward: bool| -> Point {
        // En una esquina o extremo, el tangente es el del lado del tramo; si no, el central.
        let (prev, next) = (pts[(i + n - 1) % n], pts[(i + 1) % n]);
        let smooth = !is_corner[i];
        if smooth {
            let c = unit(sub(next, prev));
            if forward { c } else { mul(c, -1.0) }
        } else if forward {
            unit(sub(next, pts[i]))
        } else {
            unit(sub(prev, pts[i]))
        }
    };

    let mut segs: Vec<PathSeg> = vec![PathSeg::MoveTo {
        to: pts[spans[0].0],
    }];
    for &(a, b) in &spans {
        let run = run_points(a, b);
        let keep = douglas_peucker(&run, 0, run.len() - 1, tol);
        let line_segs = keep.len() - 1;
        let mut done = false;
        if opts.smooth && run.len() > 3 && line_segs >= 2 {
            let (t1, t2) = (tangent_at(a, true), tangent_at(b, false));
            let mut cubics: Vec<Cubic> = Vec::new();
            fit_cubic(&run, t1, t2, tol * 0.8, &mut cubics, 0);
            if cubics.len() < line_segs && fit_error(&cubics, &run) <= tol {
                for c in &cubics {
                    segs.push(PathSeg::CubicTo {
                        ctrl1: c[1],
                        ctrl2: c[2],
                        to: c[3],
                    });
                }
                done = true;
            }
        }
        if !done {
            for &k in &keep[1..] {
                segs.push(PathSeg::LineTo { to: run[k] });
            }
        }
    }
    if closed {
        // El último tramo termina en el inicio: el cierre lo dibuja `Close`.
        if let Some(PathSeg::LineTo { .. }) = segs.last() {
            segs.pop();
        }
        segs.push(PathSeg::Close);
    }
    BezPath(segs)
}
