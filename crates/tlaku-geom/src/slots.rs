//! Redimensionar ranuras (Resize Slots): detecta ranuras y pestañas rectangulares de un ancho
//! dado y las cambia a otro ancho, para ajustar el diseño al grosor real del material
//! (F71 de `docs/spec/01-diseno-edicion.md`).
//!
//! Se reconocen tres patrones, siempre con esquinas vivas a 90° (tolerancia 1°) y en polilíneas
//! cerradas:
//! - **Muesca** (`notches`): una entrada en forma de U en el borde de una pieza. Su ancho es la
//!   distancia entre las dos paredes.
//! - **Agujero rectangular** (`holes`): un rectángulo cerrado dentro de otra forma (profundidad
//!   de anidamiento impar). Se cambia el lado cuyo largo coincide con el ancho buscado; si es
//!   un cuadrado que coincide, ambos lados.
//! - **Pestaña** (`tabs`): lo contrario de la muesca, un saliente en U de ese ancho.
//!
//! El ancho se cambia moviendo las paredes según el ancla (`SlotAnchor`): desde el centro
//! (cada pared se mueve la mitad), dejando fijo el lado inicial o dejando fijo el final. Los
//! vértices de la boca de una muesca se deslizan sobre el borde de la pieza, que se acorta o
//! se alarga. Si el cambio dejara un borde con largo cero o invertido, la trayectoria no se toca.

use crate::nesting::analyze;
use crate::util::{remove_redundant_points, signed_area};
use tlaku_core::{Point, Polyline};

/// Tolerancia angular de las esquinas: seno de 1°.
const RIGHT_ANGLE: f64 = 0.0175;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotAnchor {
    /// Cada pared se mueve la mitad del cambio.
    Center,
    /// Queda fija la pared inicial (la primera según el sentido del contorno).
    Start,
    /// Queda fija la pared final.
    End,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlotOptions {
    /// Ancho de las ranuras a buscar (mm).
    pub from_width: f64,
    /// Se aceptan anchos entre `from_width - tolerance` y `from_width + tolerance` (mm).
    pub tolerance: f64,
    /// Ancho nuevo (mm).
    pub to_width: f64,
    pub anchor: SlotAnchor,
    pub notches: bool,
    pub holes: bool,
    pub tabs: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SlotReport {
    /// Ranuras que coinciden con el ancho buscado.
    pub found: usize,
    /// Ranuras cuyo ancho se cambió.
    pub resized: usize,
    /// Ranuras que no se pudieron cambiar porque dejarían un borde invertido.
    pub blocked: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Notch,
    Tab,
}

/// Una muesca o pestaña: vértices A, B, C, D (índices) con B-C como fondo y `dir` de B hacia C.
struct Wall {
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    width: f64,
    dir: Point,
}

fn sub(a: Point, b: Point) -> Point {
    Point::new(a.x - b.x, a.y - b.y)
}
fn cross(a: Point, b: Point) -> f64 {
    a.x * b.y - a.y * b.x
}
fn dot(a: Point, b: Point) -> f64 {
    a.x * b.x + a.y * b.y
}
fn len(a: Point) -> f64 {
    a.x.hypot(a.y)
}
fn unit(a: Point) -> Point {
    let l = len(a);
    Point::new(a.x / l, a.y / l)
}
fn perpendicular(a: Point, b: Point) -> bool {
    dot(unit(a), unit(b)).abs() < RIGHT_ANGLE
}
fn parallel_same(a: Point, b: Point) -> bool {
    let (ua, ub) = (unit(a), unit(b));
    cross(ua, ub).abs() < RIGHT_ANGLE && dot(ua, ub) > 0.0
}

fn in_range(w: f64, filter: Option<(f64, f64)>) -> bool {
    filter.is_none_or(|(from, tol)| (w - from).abs() <= tol + 1e-9)
}

/// Trayectoria cerrada limpia para analizar (sin repetidos ni colineales); `None` si no lo es.
fn cleaned(p: &Polyline) -> Option<Vec<Point>> {
    if !p.closed
        || p.points
            .iter()
            .any(|q| !q.x.is_finite() || !q.y.is_finite())
    {
        return None;
    }
    let c = remove_redundant_points(p, 1e-6);
    (c.points.len() >= 4 && signed_area(&c.points).abs() > 1e-9).then_some(c.points)
}

/// Muescas y pestañas de un contorno que cumplen el filtro de ancho.
fn find_walls(pts: &[Point], tabs: bool, notches: bool, filter: Option<(f64, f64)>) -> Vec<Wall> {
    let n = pts.len();
    if n < 6 {
        return Vec::new();
    }
    let sign = signed_area(pts).signum();
    let at = |i: usize| pts[i % n];
    let mut out = Vec::new();
    let mut used = vec![false; n];
    for i in 0..n {
        let (a, b, c, d) = ((i + n - 1) % n, i, (i + 1) % n, (i + 2) % n);
        let (a0, d1) = ((i + n - 2) % n, (i + 3) % n);
        if [a, b, c, d].iter().any(|&k| used[k]) {
            continue;
        }
        let bc = sub(at(c), at(b));
        let width = len(bc);
        if width < 1e-9 || !in_range(width, filter) {
            continue;
        }
        let (ab, cd) = (sub(at(b), at(a)), sub(at(d), at(c)));
        if !perpendicular(ab, bc)
            || !perpendicular(cd, bc)
            || dot(unit(ab), unit(cd)) > -1.0 + RIGHT_ANGLE
        {
            continue;
        }
        // Giro en B y en C: ambos cóncavos (muesca) o ambos convexos (pestaña).
        let (tb, tc) = (sign * cross(ab, bc), sign * cross(bc, cd));
        let kind = if tb < 0.0 && tc < 0.0 {
            Kind::Notch
        } else if tb > 0.0 && tc > 0.0 {
            Kind::Tab
        } else {
            continue;
        };
        if (kind == Kind::Notch && !notches) || (kind == Kind::Tab && !tabs) {
            continue;
        }
        // Los bordes que llegan a la boca y salen de ella siguen la dirección del fondo.
        let (before, after) = (sub(at(a), at(a0)), sub(at(d1), at(d)));
        if len(before) < 1e-9
            || len(after) < 1e-9
            || !parallel_same(before, bc)
            || !parallel_same(after, bc)
        {
            continue;
        }
        for k in [a, b, c, d] {
            used[k] = true;
        }
        out.push(Wall {
            a,
            b,
            c,
            d,
            width,
            dir: unit(bc),
        });
    }
    out
}

/// Lados (largo del lado, índice del primer vértice) de un rectángulo de 4 vértices, o `None`.
fn rect_sides(pts: &[Point]) -> Option<[f64; 2]> {
    if pts.len() != 4 {
        return None;
    }
    let e: Vec<Point> = (0..4).map(|i| sub(pts[(i + 1) % 4], pts[i])).collect();
    if (0..4).any(|i| len(e[i]) < 1e-9 || !perpendicular(e[i], e[(i + 1) % 4])) {
        return None;
    }
    // Lados opuestos del mismo largo (a 1 µm).
    ((len(e[0]) - len(e[2])).abs() < 1e-6 && (len(e[1]) - len(e[3])).abs() < 1e-6)
        .then(|| [len(e[0]), len(e[1])])
}

/// Reparto del cambio entre la pared inicial y la final: (fracción de la inicial, de la final).
fn shares(anchor: SlotAnchor) -> (f64, f64) {
    match anchor {
        SlotAnchor::Center => (0.5, 0.5),
        SlotAnchor::Start => (0.0, 1.0),
        SlotAnchor::End => (1.0, 0.0),
    }
}

/// Anchos de las ranuras detectadas (redondeados a 0,01 mm) con cuántas hay de cada uno,
/// ordenados por ancho. Sirve para proponer el "ancho actual" al usuario.
pub fn slot_widths(
    paths: &[Polyline],
    notches: bool,
    holes: bool,
    tabs: bool,
) -> Vec<(f64, usize)> {
    let mut found: Vec<f64> = Vec::new();
    let (depth, _) = analyze(paths);
    for (k, p) in paths.iter().enumerate() {
        let Some(pts) = cleaned(p) else { continue };
        for w in find_walls(&pts, tabs, notches, None) {
            found.push(w.width);
        }
        if holes
            && depth[k] % 2 == 1
            && let Some(sides) = rect_sides(&pts)
        {
            found.extend(sides);
        }
    }
    let mut out: Vec<(f64, usize)> = Vec::new();
    found.sort_by(|a, b| a.total_cmp(b));
    for w in found {
        let r = (w * 100.0).round() / 100.0;
        match out.last_mut() {
            Some((last, count)) if (*last - r).abs() < 1e-9 => *count += 1,
            _ => out.push((r, 1)),
        }
    }
    out
}

/// Cambia las ranuras de ancho `from_width ± tolerance` a `to_width`. Devuelve las trayectorias
/// en el mismo orden (las que no cambian, idénticas) y el informe.
pub fn resize_slots(paths: &[Polyline], opts: &SlotOptions) -> (Vec<Polyline>, SlotReport) {
    let mut report = SlotReport::default();
    let valid = opts.from_width.is_finite()
        && opts.from_width > 0.0
        && opts.tolerance.is_finite()
        && opts.tolerance >= 0.0
        && opts.to_width.is_finite()
        && opts.to_width > 0.0;
    if !valid {
        return (paths.to_vec(), report);
    }
    let filter = Some((opts.from_width, opts.tolerance));
    let delta = opts.to_width - opts.from_width;
    let (depth, _) = analyze(paths);
    let (lo, hi) = shares(opts.anchor);
    let mut out = Vec::with_capacity(paths.len());
    for (k, original) in paths.iter().enumerate() {
        let Some(pts) = cleaned(original) else {
            out.push(original.clone());
            continue;
        };
        let mut moved = pts.clone();
        let mut count = 0usize;
        for w in find_walls(&pts, opts.tabs, opts.notches, filter) {
            // El ancho real puede diferir del buscado dentro de la tolerancia: se lleva al nuevo.
            let change = opts.to_width - w.width;
            let _ = delta;
            let (low, high) = (-change * lo, change * hi);
            for (idx, off) in [(w.a, low), (w.b, low), (w.c, high), (w.d, high)] {
                moved[idx].x += w.dir.x * off;
                moved[idx].y += w.dir.y * off;
            }
            count += 1;
        }
        if opts.holes
            && depth[k] % 2 == 1
            && let Some(sides) = rect_sides(&pts)
        {
            let e = |i: usize| sub(pts[(i + 1) % 4], pts[i]);
            // Lado 0 (vértices 0-1 y 3-2, dirección e0) y lado 1 (vértices 1-2 y 0-3, dirección e1).
            let dims = [
                (0usize, sides[0], unit(e(0)), [(0, 3), (1, 2)]),
                (1, sides[1], unit(e(1)), [(0, 1), (3, 2)]),
            ];
            for (_, side, dir, [(a1, a2), (b1, b2)]) in dims {
                if in_range(side, filter) {
                    let change = opts.to_width - side;
                    let (low, high) = (-change * lo, change * hi);
                    for (idx, off) in [(a1, low), (a2, low), (b1, high), (b2, high)] {
                        moved[idx].x += dir.x * off;
                        moved[idx].y += dir.y * off;
                    }
                    count += 1;
                }
            }
        }
        if count == 0 {
            out.push(original.clone());
            continue;
        }
        report.found += count;
        if keeps_edges(&pts, &moved) {
            report.resized += count;
            out.push(Polyline {
                points: moved,
                closed: true,
            });
        } else {
            report.blocked += count;
            out.push(original.clone());
        }
    }
    (out, report)
}

/// Cada borde conserva su dirección y no se reduce a nada.
fn keeps_edges(before: &[Point], after: &[Point]) -> bool {
    let n = before.len();
    (0..n).all(|i| {
        let (e0, e1) = (
            sub(before[(i + 1) % n], before[i]),
            sub(after[(i + 1) % n], after[i]),
        );
        len(e1) > 1e-6 && dot(e0, e1) > 0.0
    })
}
