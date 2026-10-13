//! Recortar formas (Trim): dado un punto y un conjunto de trayectorias, borra el tramo de la
//! trayectoria más cercana que queda entre dos intersecciones consecutivas.
//!
//! Comportamiento (descrito en `docs/spec/01-diseno-edicion.md`, F49):
//! - El tramo borrado va desde la intersección anterior hasta la siguiente al punto de clic
//!   (medidas a lo largo de la trayectoria). Las intersecciones pueden ser con cualquier
//!   trayectoria del conjunto, incluida la propia (autointersecciones).
//! - Si la trayectoria no cruza nada, se borra completa.
//! - En una abierta quedan hasta dos piezas (antes y después del tramo). En una cerrada con dos
//!   o más cortes queda una sola pieza abierta (el resto del contorno, que "da la vuelta").
//! - Los extremos de una abierta que tocan otra línea también cuentan como intersección, y las
//!   líneas colineales que se solapan cortan donde empieza y termina el solape.
//!
//! La posición en una trayectoria se mide con un parámetro `i + t`: el segmento `i` y la
//! fracción `t` (0 a 1) recorrida de él.

use tlaku_core::{Point, Polyline};

/// Distancia (mm) bajo la cual dos puntos o una recta y un punto se consideran coincidentes.
const EPS: f64 = 1e-7;

/// Resultado de recortar un tramo.
#[derive(Debug, Clone, PartialEq)]
pub struct TrimResult {
    /// Índice (en la lista de entrada) de la trayectoria recortada.
    pub path_index: usize,
    /// Tramo borrado (abierto salvo que fuera la trayectoria entera, que conserva `closed`).
    pub removed: Polyline,
    /// Piezas que quedan de la trayectoria recortada (0, 1 o 2), todas abiertas.
    pub remaining: Vec<Polyline>,
    /// La trayectoria se borró completa porque no cruza nada.
    pub whole: bool,
}

fn lerp(a: Point, b: Point, t: f64) -> Point {
    Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

/// Segmentos de una trayectoria como pares de puntos (el cierre incluido).
pub(crate) fn segment_count(p: &Polyline) -> usize {
    let n = p.points.len();
    if n < 2 {
        0
    } else if p.closed {
        n
    } else {
        n - 1
    }
}

pub(crate) fn segment(p: &Polyline, i: usize) -> (Point, Point) {
    let n = p.points.len();
    (p.points[i % n], p.points[(i + 1) % n])
}

/// Punto de la trayectoria en el parámetro `param` (cíclico si es cerrada).
pub(crate) fn point_at(p: &Polyline, param: f64) -> Point {
    let segs = segment_count(p);
    if segs == 0 {
        return p.points.first().copied().unwrap_or(Point::new(0.0, 0.0));
    }
    let param = if p.closed {
        param.rem_euclid(segs as f64)
    } else {
        param.clamp(0.0, segs as f64)
    };
    let i = (param.floor() as usize).min(segs - 1);
    let (a, b) = segment(p, i);
    lerp(a, b, param - i as f64)
}

fn cross(a: Point, b: Point) -> f64 {
    a.x * b.y - a.y * b.x
}

/// Parámetros `t` (0 a 1) sobre el segmento `p-q` donde corta o se solapa con `a-b`.
pub(crate) fn segment_cuts(p: Point, q: Point, a: Point, b: Point, out: &mut Vec<f64>) {
    let d1 = Point::new(q.x - p.x, q.y - p.y);
    let d2 = Point::new(b.x - a.x, b.y - a.y);
    let (l1, l2) = (d1.x.hypot(d1.y), d2.x.hypot(d2.y));
    if l1 <= EPS || l2 <= EPS {
        return;
    }
    let ap = Point::new(a.x - p.x, a.y - p.y);
    let denom = cross(d1, d2);
    if denom.abs() > 1e-9 * l1 * l2 {
        let t = cross(ap, d2) / denom;
        let u = cross(ap, d1) / denom;
        let (et, eu) = (EPS / l1, EPS / l2);
        if t >= -et && t <= 1.0 + et && u >= -eu && u <= 1.0 + eu {
            out.push(t.clamp(0.0, 1.0));
        }
        return;
    }
    // Paralelas: solo cortan si son colineales; los extremos de `a-b` dentro de `p-q` son cortes.
    if cross(ap, d1).abs() / l1 > EPS {
        return;
    }
    for e in [a, b] {
        let t = ((e.x - p.x) * d1.x + (e.y - p.y) * d1.y) / (l1 * l1);
        if (0.0..=1.0).contains(&t) {
            out.push(t);
        }
    }
}

/// Parámetros (ordenados y sin repetidos) donde `path` se cruza con las demás trayectorias de
/// `others` (que pueden incluir a la propia: se salta el cruce con sus segmentos vecinos).
pub(crate) fn cut_params(
    path: &Polyline,
    own_index: Option<usize>,
    others: &[Polyline],
) -> Vec<f64> {
    let segs = segment_count(path);
    let mut params: Vec<f64> = Vec::new();
    let mut ts: Vec<f64> = Vec::new();
    for i in 0..segs {
        let (p, q) = segment(path, i);
        let (lox, hix) = (p.x.min(q.x) - EPS, p.x.max(q.x) + EPS);
        let (loy, hiy) = (p.y.min(q.y) - EPS, p.y.max(q.y) + EPS);
        for (k, other) in others.iter().enumerate() {
            let same = own_index == Some(k);
            for j in 0..segment_count(other) {
                if same {
                    // Vecinos (incluido el cierre) comparten un vértice: no son un cruce.
                    let d = i.abs_diff(j);
                    if d <= 1 || (path.closed && d == segs - 1) {
                        continue;
                    }
                }
                let (a, b) = segment(other, j);
                if a.x.max(b.x) < lox
                    || a.x.min(b.x) > hix
                    || a.y.max(b.y) < loy
                    || a.y.min(b.y) > hiy
                {
                    continue;
                }
                ts.clear();
                segment_cuts(p, q, a, b, &mut ts);
                params.extend(ts.iter().map(|t| i as f64 + t));
            }
        }
    }
    params.sort_by(|a, b| a.total_cmp(b));
    // Cortes a menos de EPS mm uno de otro son el mismo.
    let mut out: Vec<(f64, Point)> = Vec::with_capacity(params.len());
    for t in params {
        let pt = point_at(path, t);
        if out.last().is_none_or(|(_, q)| q.dist(pt) > EPS) {
            out.push((t, pt));
        }
    }
    if path.closed {
        // El parámetro `segs` es el mismo punto que el 0.
        let segs = segs as f64;
        let mut cuts: Vec<(f64, Point)> = out.into_iter().map(|(t, p)| (t % segs, p)).collect();
        cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
        cuts.dedup_by(|b, a| a.1.dist(b.1) <= EPS);
        return cuts.into_iter().map(|(t, _)| t).collect();
    }
    out.into_iter().map(|(t, _)| t).collect()
}

/// Tramo de `path` desde `from` hasta `from + len` (en parámetro). En cerradas el tramo puede
/// dar la vuelta; en abiertas `from + len` no debe pasar del final. Sin puntos repetidos.
pub(crate) fn sub_path(path: &Polyline, from: f64, len: f64) -> Option<Polyline> {
    let segs = segment_count(path) as f64;
    let n = path.points.len();
    let mut pts = vec![point_at(path, from)];
    // Vértices enteros estrictamente dentro del tramo.
    let first = (from + 1e-12).floor() as i64 + 1;
    let last_excl = from + len - 1e-12;
    let mut k = first;
    while (k as f64) < last_excl {
        let idx = if path.closed {
            (k.rem_euclid(segs as i64)) as usize
        } else {
            (k as usize).min(n - 1)
        };
        pts.push(path.points[idx % n]);
        k += 1;
    }
    pts.push(point_at(path, from + len));
    pts.dedup_by(|a, b| a.dist(*b) <= 1e-9);
    (pts.len() >= 2).then_some(Polyline {
        points: pts,
        closed: false,
    })
}

/// Distancia de `p` al segmento `a-b` y fracción del punto más cercano.
fn dist_to_segment(p: Point, a: Point, b: Point) -> (f64, f64) {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    };
    (p.dist(lerp(a, b, t)), t)
}

/// Trayectoria más cercana a `click`: índice, parámetro del punto más cercano y distancia.
/// Las trayectorias de menos de 2 puntos o con coordenadas no finitas se ignoran.
pub fn nearest_on_paths(paths: &[Polyline], click: Point) -> Option<(usize, f64, f64)> {
    let mut best: Option<(usize, f64, f64)> = None;
    for (k, p) in paths.iter().enumerate() {
        if p.points
            .iter()
            .any(|q| !q.x.is_finite() || !q.y.is_finite())
        {
            continue;
        }
        for i in 0..segment_count(p) {
            let (a, b) = segment(p, i);
            let (d, t) = dist_to_segment(click, a, b);
            if best.is_none_or(|(_, _, bd)| d < bd) {
                best = Some((k, i as f64 + t, d));
            }
        }
    }
    best
}

/// Recorta el tramo más cercano a `click` entre dos intersecciones. `max_dist` (mm) es la
/// distancia máxima del clic a la trayectoria (`None` = sin límite). Devuelve `None` si no hay
/// ninguna trayectoria a esa distancia o si `click` no es finito.
pub fn trim_at(paths: &[Polyline], click: Point, max_dist: Option<f64>) -> Option<TrimResult> {
    if !click.x.is_finite() || !click.y.is_finite() {
        return None;
    }
    let (index, param, dist) = nearest_on_paths(paths, click)?;
    if max_dist.is_some_and(|m| dist > m) {
        return None;
    }
    let path = &paths[index];
    let segs = segment_count(path) as f64;
    let cuts = cut_params(path, Some(index), paths);

    if path.closed {
        if cuts.len() < 2 {
            return Some(TrimResult {
                path_index: index,
                removed: path.clone(),
                remaining: Vec::new(),
                whole: true,
            });
        }
        // Corte anterior (≤ param, o el último desplazado una vuelta) y siguiente.
        let m = cuts.len();
        let (prev, next) = match cuts.iter().rposition(|&c| c <= param + 1e-12) {
            Some(i) if i + 1 < m => (cuts[i], cuts[i + 1]),
            Some(i) => (cuts[i], cuts[0] + segs),
            None => (cuts[m - 1] - segs, cuts[0]),
        };
        let removed = sub_path(path, prev, next - prev)?;
        let remaining = sub_path(path, next, segs - (next - prev))
            .into_iter()
            .collect();
        return Some(TrimResult {
            path_index: index,
            removed,
            remaining,
            whole: false,
        });
    }

    let prev = cuts
        .iter()
        .copied()
        .rfind(|&c| c <= param + 1e-12)
        .unwrap_or(0.0);
    let next = cuts
        .iter()
        .copied()
        .find(|&c| c > param + 1e-12)
        .unwrap_or(segs);
    if prev <= 0.0 && next >= segs {
        return Some(TrimResult {
            path_index: index,
            removed: path.clone(),
            remaining: Vec::new(),
            whole: true,
        });
    }
    let removed = sub_path(path, prev, next - prev)?;
    let mut remaining = Vec::new();
    if prev > 0.0 {
        remaining.extend(sub_path(path, 0.0, prev));
    }
    if next < segs {
        remaining.extend(sub_path(path, next, segs - next));
    }
    Some(TrimResult {
        path_index: index,
        removed,
        remaining,
        whole: false,
    })
}
