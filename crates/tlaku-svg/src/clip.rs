//! Recorte de geometría contra una región: lo que necesitan `clip-path` y `mask`.
//!
//! Una región es un conjunto de anillos cerrados en mm (Y hacia abajo) con la convención de
//! `tlaku-geom`: exteriores antihorarios y huecos horarios, de modo que un punto está dentro si
//! está dentro de un número impar de anillos. Las formas rellenas se recortan con una
//! intersección booleana; las trayectorias (trazos, abiertas o cerradas) se parten en las
//! entradas y salidas de la región. Todo opera sobre curvas ya aplanadas, así que lo recortado
//! queda en segmentos rectos con la tolerancia de la importación.

use tlaku_core::{Point, Polyline};
use tlaku_geom::{BoolOp, PointLocation, boolean, locate_point, signed_area, weld};

/// Cuánto (mm) puede quedar un punto fuera del borde y contar como borde. Las coordenadas de usvg
/// son `f32`, así que un corte que coincide con el borde del recorte difiere unos 1e-5 mm.
const EDGE: f64 = 1e-4;

/// Anillos cerrados que describen una región (ver el módulo).
pub(crate) type Region = Vec<Polyline>;

/// Distancia (mm) por debajo de la cual dos puntos recortados se consideran el mismo.
const EPS: f64 = 1e-7;

pub(crate) fn intersect(a: &Region, b: &Region) -> Region {
    boolean(a, b, BoolOp::Intersection)
}

pub(crate) fn union(a: &Region, b: &Region) -> Region {
    if a.is_empty() {
        return b.clone();
    }
    if b.is_empty() {
        return a.clone();
    }
    boolean(a, b, BoolOp::Union)
}

pub(crate) fn subtract(a: &Region, b: &Region) -> Region {
    boolean(a, b, BoolOp::Difference)
}

/// Región que cubren las trayectorias de un mismo elemento SVG: las abiertas se cierran de
/// forma implícita, como al rellenar, y los subtrayectos que se solapan se unen.
pub(crate) fn region_of(paths: &[Polyline]) -> Region {
    let closed: Vec<Polyline> = paths
        .iter()
        .filter(|p| p.points.len() >= 3)
        .map(|p| Polyline {
            points: p.points.clone(),
            closed: true,
        })
        .collect();
    weld(&closed)
}

/// Hay algún hueco (anillo horario): la región no es simplemente "lo que encierran los
/// exteriores".
pub(crate) fn has_holes(region: &Region) -> bool {
    region.iter().any(|r| signed_area(&r.points) < 0.0)
}

/// Resultado de recortar una polilínea contra una región.
#[derive(Debug)]
pub(crate) enum LineClip {
    /// Queda entera dentro: no hay que tocarla (se conservan sus curvas).
    Untouched,
    /// Los tramos que quedan dentro; vacío si nada queda.
    Pieces(Vec<Polyline>),
}

/// Distancia de `p` al segmento `a`-`b`.
fn dist_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 == 0.0 {
        return p.dist(a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    p.dist(Point::new(a.x + t * dx, a.y + t * dy))
}

/// Parte par-impar: el punto está estrictamente dentro de la región.
fn inside_parity(region: &Region, p: Point) -> bool {
    let count = region
        .iter()
        .filter(|ring| locate_point(p, &ring.points) == PointLocation::Inside)
        .count();
    count % 2 == 1
}

/// El punto está a menos de `EDGE` de algún lado de la región.
fn on_border(region: &Region, p: Point) -> bool {
    region.iter().any(|ring| {
        let pts = &ring.points;
        let n = pts.len();
        (0..n).any(|i| dist_to_segment(p, pts[i], pts[(i + 1) % n]) <= EDGE)
    })
}

/// El tramo `a`-`b` (sin cortes en su interior) queda dentro de la región. Un tramo que corre a
/// lo largo del borde del recorte cuenta como dentro (su extremos y su medio están en el borde);
/// uno que solo roza el borde en un punto (tangente) no.
fn inside(region: &Region, a: Point, b: Point) -> bool {
    let mid = lerp(a, b, 0.5);
    inside_parity(region, mid)
        || (on_border(region, mid) && on_border(region, a) && on_border(region, b))
}

/// Parámetros `t` (0..1) donde el segmento `a`-`b` cruza algún lado de la región.
fn cut_params(a: Point, b: Point, region: &Region, out: &mut Vec<f64>) {
    let (sx, sy) = (b.x - a.x, b.y - a.y);
    let (min_x, max_x) = (a.x.min(b.x) - EPS, a.x.max(b.x) + EPS);
    let (min_y, max_y) = (a.y.min(b.y) - EPS, a.y.max(b.y) + EPS);
    for ring in region {
        let pts = &ring.points;
        let n = pts.len();
        for i in 0..n {
            let (c, d) = (pts[i], pts[(i + 1) % n]);
            if c.x.max(d.x) < min_x || c.x.min(d.x) > max_x {
                continue;
            }
            if c.y.max(d.y) < min_y || c.y.min(d.y) > max_y {
                continue;
            }
            let (rx, ry) = (d.x - c.x, d.y - c.y);
            let denom = sx * ry - sy * rx;
            if denom.abs() < 1e-12 {
                continue; // paralelos o colineales: sus extremos los aportan los lados vecinos
            }
            let (qx, qy) = (c.x - a.x, c.y - a.y);
            let t = (qx * ry - qy * rx) / denom;
            let u = (qx * sy - qy * sx) / denom;
            let tol_t = EPS / (sx.hypot(sy)).max(EPS);
            let tol_u = EPS / (rx.hypot(ry)).max(EPS);
            if t > tol_t && t < 1.0 - tol_t && u > -tol_u && u < 1.0 + tol_u {
                out.push(t);
            }
        }
    }
}

fn lerp(a: Point, b: Point, t: f64) -> Point {
    Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

/// Parte la polilínea (abierta o cerrada) y conserva solo lo que cae dentro de la región. En una
/// cerrada, si el inicio queda dentro, el último tramo se une con el primero.
pub(crate) fn clip_polyline(line: &Polyline, region: &Region) -> LineClip {
    let pts = &line.points;
    if pts.len() < 2 || region.is_empty() {
        return LineClip::Pieces(Vec::new());
    }
    let seg_count = if line.closed {
        pts.len()
    } else {
        pts.len() - 1
    };
    let mut pieces: Vec<Vec<Point>> = Vec::new();
    let mut cur: Vec<Point> = Vec::new();
    let mut changed = false;
    let mut cuts: Vec<f64> = Vec::new();

    let flush = |cur: &mut Vec<Point>, pieces: &mut Vec<Vec<Point>>| {
        let piece = std::mem::take(cur);
        if piece.len() >= 2 {
            pieces.push(piece);
        }
    };

    for i in 0..seg_count {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        cuts.clear();
        cuts.push(0.0);
        cut_params(a, b, region, &mut cuts);
        cuts.push(1.0);
        cuts.sort_by(|x, y| x.total_cmp(y));
        cuts.dedup_by(|x, y| (*x - *y).abs() < 1e-12);
        for w in cuts.windows(2) {
            let p0 = if w[0] == 0.0 { a } else { lerp(a, b, w[0]) };
            let p1 = if w[1] == 1.0 { b } else { lerp(a, b, w[1]) };
            if p0.dist(p1) < EPS {
                continue;
            }
            if inside(region, p0, p1) {
                if cur.last().is_none_or(|l| l.dist(p0) > EPS) {
                    flush(&mut cur, &mut pieces);
                    cur.push(p0);
                }
                cur.push(p1);
            } else {
                changed = true;
                flush(&mut cur, &mut pieces);
            }
        }
    }
    flush(&mut cur, &mut pieces);

    if !changed {
        return LineClip::Untouched;
    }
    if line.closed && pieces.len() > 1 {
        let start = pts[0];
        let joins = pieces
            .first()
            .and_then(|p| p.first())
            .is_some_and(|p| p.dist(start) <= EPS)
            && pieces
                .last()
                .and_then(|p| p.last())
                .is_some_and(|p| p.dist(start) <= EPS);
        if joins {
            let first = pieces.remove(0);
            if let Some(last) = pieces.last_mut() {
                last.extend(first.into_iter().skip(1));
            }
        }
    }
    LineClip::Pieces(
        pieces
            .into_iter()
            .map(|points| Polyline {
                points,
                closed: false,
            })
            .collect(),
    )
}
