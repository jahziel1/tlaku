//! Puentes (tabs): huecos que se dejan sin cortar en un contorno cerrado para que la pieza
//! no se caiga del material.
//!
//! [`tab_segments`] recibe un contorno cerrado y devuelve los tramos que sí se cortan, en el
//! orden de recorrido y empezando donde empieza el contorno (el planificador ya eligió ese
//! punto de entrada). Entre dos tramos consecutivos queda un hueco de `width_mm` de largo
//! medido sobre el contorno; quien llama decide cómo cruzarlo (en el pipeline, con el láser
//! apagado). El último hueco, si cae justo al final del recorrido, no se cruza.
//!
//! Reparto: los puentes se colocan a distancias iguales a lo largo del perímetro y cada uno
//! se desplaza, si hace falta, al tramo recto más cercano para no quedar sobre una esquina
//! aguda. El desplazamiento está acotado (30 % del paso entre puentes), así que en una forma
//! con esquinas por todas partes (un polígono pequeño) el puente puede quedar cerca de una.
//!
//! Cuántos: con `spacing_mm > 0` son `max(count, ceil(perímetro / spacing))` (la distancia entre
//! puentes nunca supera `spacing_mm`); si no, exactamente `count`. Los puentes no pueden ocupar
//! más de la mitad del perímetro: si no caben se reducen a `perímetro / (2 * width)` (o a
//! ninguno) y [`TabResult::fits`] lo dice, para que quien llama avise.

use crate::nesting_depth;
use crate::util::{EPS, is_valid_point};
use tlaku_core::{Point, Polyline};

/// Giro mínimo (grados) para considerar un vértice una esquina aguda. Las curvas aplanadas
/// giran muy poco por vértice, así que no cuentan.
const SHARP_TURN_DEG: f64 = 30.0;

/// Cuánto puede alejarse un puente de su posición ideal, como fracción del paso entre puentes.
const SHIFT_FRACTION: f64 = 0.3;

/// Distancia mínima entre centros de puentes vecinos, en anchos de puente (deja al menos un
/// cuarto de ancho de contorno cortado entre ambos).
const MIN_SEPARATION: f64 = 1.25;

/// Tope de puentes por forma; protege contra un `spacing` diminuto.
pub const MAX_TABS_PER_SHAPE: usize = 1000;

/// Parámetros del reparto de puentes (equivalen a `TabSettings` del documento, sin los
/// campos que no dependen de la geometría de un solo contorno).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TabOptions {
    /// Largo de cada hueco sobre el contorno, en mm.
    pub width_mm: f64,
    /// Puentes por forma si `spacing_mm` es 0; si no, el mínimo por forma.
    pub count: u32,
    /// Distancia máxima entre puentes a lo largo del contorno; 0 = usar `count`.
    pub spacing_mm: f64,
}

/// Resultado de partir un contorno.
#[derive(Debug, Clone, PartialEq)]
pub struct TabResult {
    /// Tramos abiertos a cortar, en orden de recorrido. Si no se pudo poner ningún puente es
    /// el contorno original (cerrado), sin tocar.
    pub segments: Vec<Polyline>,
    /// Puentes pedidos por los ajustes para esta forma (0 si la forma no admite puentes).
    pub requested: usize,
    /// Puentes que realmente quedaron; menos que `requested` cuando no caben.
    pub placed: usize,
    /// Perímetro del contorno, en mm (0 si la forma es degenerada).
    pub length_mm: f64,
}

impl TabResult {
    fn untouched(path: &Polyline, requested: usize, length_mm: f64) -> Self {
        Self {
            segments: vec![path.clone()],
            requested,
            placed: 0,
            length_mm,
        }
    }

    /// Verdadero si todos los puentes pedidos caben.
    pub fn fits(&self) -> bool {
        self.placed == self.requested
    }
}

/// Qué trayectorias de `paths` admiten puentes: las cerradas y, con `skip_inner`, solo las
/// que no están contenidas en otra (profundidad de anidamiento 0). Las abiertas nunca.
pub fn tab_eligible(paths: &[Polyline], skip_inner: bool) -> Vec<bool> {
    let depth = if skip_inner {
        nesting_depth(paths)
    } else {
        vec![0; paths.len()]
    };
    paths
        .iter()
        .zip(depth)
        .map(|(p, d)| p.closed && p.points.len() >= 3 && d == 0)
        .collect()
}

/// Parte el contorno cerrado `path` dejando huecos. Una trayectoria abierta, degenerada o con
/// ajustes que no piden puentes se devuelve intacta con `requested == 0`.
pub fn tab_segments(path: &Polyline, opts: &TabOptions) -> TabResult {
    let width = opts.width_mm;
    if !(path.closed && width.is_finite() && width > 0.0) {
        return TabResult::untouched(path, 0, 0.0);
    }
    let pts = clean(&path.points);
    if pts.len() < 3 || !pts.iter().all(is_valid_point) {
        return TabResult::untouched(path, 0, 0.0);
    }
    let cum = cumulative(&pts);
    let total = cum[pts.len()];
    if total <= EPS {
        return TabResult::untouched(path, 0, 0.0);
    }

    let by_distance = if opts.spacing_mm.is_finite() && opts.spacing_mm > 0.0 {
        // Distancia entre puentes nunca mayor que `spacing_mm`.
        ((total / opts.spacing_mm) - 1e-9).ceil().max(0.0) as usize
    } else {
        0
    };
    let requested = if opts.spacing_mm.is_finite() && opts.spacing_mm > 0.0 {
        (opts.count as usize).max(by_distance)
    } else {
        opts.count as usize
    }
    .min(MAX_TABS_PER_SHAPE);
    if requested == 0 {
        return TabResult::untouched(path, 0, total);
    }

    // Que los puentes no ocupen más de la mitad del contorno: el resto debe seguir cortando.
    let fitting = (total / (2.0 * width)).floor() as usize;
    let n = requested.min(fitting);
    if n == 0 {
        return TabResult::untouched(path, requested, total);
    }

    let centers = place_centers(total, n, width, &sharp_positions(&pts, &cum));
    TabResult {
        segments: split(&pts, &cum, &centers, width),
        requested,
        placed: n,
        length_mm: total,
    }
}

/// Quita vértices repetidos consecutivos y el último si repite al primero.
fn clean(points: &[Point]) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(points.len());
    for &p in points {
        if out.last().is_none_or(|l| l.dist(p) > EPS) {
            out.push(p);
        }
    }
    while out.len() > 1 && out[0].dist(out[out.len() - 1]) <= EPS {
        out.pop();
    }
    out
}

/// Longitud acumulada hasta cada vértice; el elemento `n` es el perímetro (con la arista de cierre).
fn cumulative(pts: &[Point]) -> Vec<f64> {
    let n = pts.len();
    let mut cum = Vec::with_capacity(n + 1);
    cum.push(0.0);
    for i in 0..n {
        let d = pts[i].dist(pts[(i + 1) % n]);
        cum.push(cum[i] + d);
    }
    cum
}

/// Posiciones (longitud de arco) de los vértices con un giro mayor que `SHARP_TURN_DEG`.
fn sharp_positions(pts: &[Point], cum: &[f64]) -> Vec<f64> {
    let n = pts.len();
    let limit = SHARP_TURN_DEG.to_radians();
    (0..n)
        .filter(|&i| {
            let (a, b, c) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
            let (u, v) = ((b.x - a.x, b.y - a.y), (c.x - b.x, c.y - b.y));
            let cross = u.0 * v.1 - u.1 * v.0;
            let dot = u.0 * v.0 + u.1 * v.1;
            cross.atan2(dot).abs() > limit
        })
        .map(|i| cum[i])
        .collect()
}

/// Distancia entre dos posiciones sobre un contorno cerrado de longitud `total`.
fn cyclic(a: f64, b: f64, total: f64) -> f64 {
    let d = (a - b).abs() % total;
    d.min(total - d)
}

/// Centros de los `n` puentes, ordenados, en `[0, total)`.
fn place_centers(total: f64, n: usize, width: f64, sharp: &[f64]) -> Vec<f64> {
    let pitch = total / n as f64;
    let reach = SHIFT_FRACTION * pitch;
    let free = |c: f64| sharp.iter().all(|&p| cyclic(c, p, total) >= width - 1e-9);
    let mut centers: Vec<f64> = Vec::with_capacity(n);
    for k in 0..n {
        let ideal = (k as f64 + 0.5) * pitch;
        let mut best = ideal;
        if !free(ideal) {
            // Los bordes de las zonas prohibidas son los únicos candidatos que hace falta mirar.
            let nearest = sharp
                .iter()
                .flat_map(|&p| [p - width, p + width])
                .map(|c| c.rem_euclid(total))
                .filter(|&c| free(c) && cyclic(c, ideal, total) <= reach)
                .min_by(|a, b| cyclic(*a, ideal, total).total_cmp(&cyclic(*b, ideal, total)));
            if let Some(c) = nearest {
                best = c;
            }
        }
        // El desplazamiento no debe juntar dos puentes; la posición ideal siempre es segura
        // porque `n` se limitó a `total / (2 * width)`.
        let apart = |c: f64| {
            let sep = MIN_SEPARATION * width;
            centers.last().is_none_or(|&o| cyclic(c, o, total) >= sep)
                && centers.first().is_none_or(|&o| cyclic(c, o, total) >= sep)
        };
        if !apart(best) {
            best = ideal;
        }
        centers.push(best);
    }
    centers
}

/// Punto del contorno a la longitud de arco `u` (se envuelve al perímetro).
fn point_at(pts: &[Point], cum: &[f64], u: f64) -> Point {
    let n = pts.len();
    let total = cum[n];
    let u = u.rem_euclid(total);
    let i = cum
        .partition_point(|&c| c <= u)
        .saturating_sub(1)
        .min(n - 1);
    let len = cum[i + 1] - cum[i];
    let t = if len > 0.0 { (u - cum[i]) / len } else { 0.0 };
    let (a, b) = (pts[i], pts[(i + 1) % n]);
    Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

/// Tramo del contorno entre las longitudes de arco `u0 < u1` (con `u1 <= u0 + perímetro`).
fn piece(pts: &[Point], cum: &[f64], u0: f64, u1: f64) -> Polyline {
    let n = pts.len();
    let total = cum[n];
    let u0n = u0.rem_euclid(total);
    let u1n = u0n + (u1 - u0);
    let mut points = vec![point_at(pts, cum, u0n)];
    for lap in 0..2 {
        for (j, p) in pts.iter().enumerate() {
            let pos = cum[j] + f64::from(lap) * total;
            if pos > u0n + EPS && pos < u1n - EPS {
                points.push(*p);
            }
        }
    }
    points.push(point_at(pts, cum, u1n));
    Polyline {
        points,
        closed: false,
    }
}

/// Construye los tramos que quedan entre los huecos centrados en `centers`.
fn split(pts: &[Point], cum: &[f64], centers: &[f64], width: f64) -> Vec<Polyline> {
    let total = cum[pts.len()];
    let half = width / 2.0;
    // Si un hueco cubre el punto de partida (longitud 0), el recorrido empieza al final de ese
    // hueco y termina al principio del mismo: el primero de la vuelta no se cruza.
    let wrapping = centers.iter().position(|&c| c < half || c > total - half);
    let start = wrapping.map_or(0.0, |i| {
        let c = centers[i];
        if c < half { c + half } else { c + half - total }
    });
    let mut gaps: Vec<f64> = centers
        .iter()
        .map(|&c| if c >= start { c } else { c + total })
        .collect();
    if let Some(i) = wrapping {
        gaps[i] = start + total - half;
    }
    gaps.sort_by(f64::total_cmp);

    let mut segments = Vec::with_capacity(gaps.len() + 1);
    let mut from = start;
    for c in gaps {
        let (a, b) = (c - half, c + half);
        if a - from > EPS {
            segments.push(piece(pts, cum, from, a));
        }
        from = b;
    }
    if start + total - from > EPS {
        segments.push(piece(pts, cum, from, start + total));
    }
    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(side: f64) -> Polyline {
        Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(side, 0.0),
                Point::new(side, side),
                Point::new(0.0, side),
            ],
            closed: true,
        }
    }

    fn circle(r: f64, n: usize) -> Polyline {
        Polyline {
            points: (0..n)
                .map(|i| {
                    let a = std::f64::consts::TAU * i as f64 / n as f64;
                    Point::new(r * a.cos(), r * a.sin())
                })
                .collect(),
            closed: true,
        }
    }

    fn length(p: &Polyline) -> f64 {
        p.points.windows(2).map(|w| w[0].dist(w[1])).sum()
    }

    fn opts(width: f64, count: u32, spacing: f64) -> TabOptions {
        TabOptions {
            width_mm: width,
            count,
            spacing_mm: spacing,
        }
    }

    /// Largo cortado más largo de los huecos: perímetro = cortado + huecos (sin contar las
    /// aristas que se saltan en una esquina, que aquí no hay).
    fn cut_length(r: &TabResult) -> f64 {
        r.segments.iter().map(length).sum()
    }

    #[test]
    fn exact_count_leaves_gaps_of_the_requested_width() {
        let r = tab_segments(&square(40.0), &opts(2.0, 4, 0.0));
        assert_eq!((r.requested, r.placed), (4, 4));
        assert!(r.fits());
        assert!((r.length_mm - 160.0).abs() < 1e-9);
        // Un puente en el centro de cada lado: 4 huecos de 2 mm. El recorrido empieza y
        // termina en la esquina de partida, así que el primer y el último tramo son dos.
        assert!((cut_length(&r) - (160.0 - 4.0 * 2.0)).abs() < 1e-6);
        assert_eq!(r.segments.len(), 5);
        assert!(r.segments.iter().all(|s| !s.closed));
    }

    #[test]
    fn tabs_avoid_sharp_corners_when_possible() {
        // Con 2 puentes en un cuadrado de 40 mm los centros ideales (40 y 120) caen justo en
        // dos esquinas; deben moverse a un tramo recto.
        let r = tab_segments(&square(40.0), &opts(2.0, 2, 0.0));
        assert_eq!(r.placed, 2);
        let corners = [0.0, 40.0, 80.0, 120.0, 160.0];
        // Cada hueco empieza donde termina un tramo: ninguno puede contener una esquina.
        let mut pos = 0.0;
        for s in &r.segments[..r.segments.len() - 1] {
            let end = pos + length(s);
            for c in corners {
                assert!(
                    !(c > end - 1e-6 && c < end + 2.0 + 1e-6),
                    "hueco [{end}, {}] sobre la esquina {c}",
                    end + 2.0
                );
            }
            pos = end + 2.0;
        }
    }

    #[test]
    fn spacing_sets_a_minimum_and_count_a_floor() {
        let sq = square(100.0); // perímetro 400
        assert_eq!(tab_segments(&sq, &opts(1.0, 2, 50.0)).placed, 8);
        assert_eq!(tab_segments(&sq, &opts(1.0, 12, 50.0)).placed, 12);
        // Sin spacing se usa exactamente count.
        assert_eq!(tab_segments(&sq, &opts(1.0, 3, 0.0)).placed, 3);
    }

    #[test]
    fn tabs_that_do_not_fit_are_reduced() {
        // Perímetro 40: caben como mucho 40 / (2 * 4) = 5 puentes de 4 mm.
        let r = tab_segments(&square(10.0), &opts(4.0, 8, 0.0));
        assert_eq!((r.requested, r.placed), (8, 5));
        assert!(!r.fits());
        // Un solo puente mayor que la mitad del contorno: no se pone ninguno.
        let r = tab_segments(&square(10.0), &opts(30.0, 1, 0.0));
        assert_eq!((r.requested, r.placed), (1, 0));
        assert_eq!(r.segments, vec![square(10.0)]);
    }

    #[test]
    fn open_degenerate_or_empty_requests_are_untouched() {
        let mut open = square(10.0);
        open.closed = false;
        assert_eq!(tab_segments(&open, &opts(1.0, 4, 0.0)).requested, 0);
        assert_eq!(tab_segments(&square(10.0), &opts(1.0, 0, 0.0)).requested, 0);
        assert_eq!(tab_segments(&square(10.0), &opts(0.0, 4, 0.0)).requested, 0);
        assert_eq!(
            tab_segments(&square(10.0), &opts(f64::NAN, 4, 0.0)).requested,
            0
        );
        let line = Polyline {
            points: vec![Point::new(0.0, 0.0), Point::new(5.0, 0.0)],
            closed: true,
        };
        assert_eq!(tab_segments(&line, &opts(1.0, 4, 0.0)).requested, 0);
    }

    #[test]
    fn segments_follow_the_path_and_cover_all_but_the_gaps() {
        let c = circle(20.0, 360);
        let r = tab_segments(&c, &opts(2.0, 6, 0.0));
        assert_eq!(r.placed, 6);
        // Todos los puntos de los tramos están sobre la circunferencia (los extremos se
        // interpolan sobre las aristas, de ahí la holgura).
        for p in r.segments.iter().flat_map(|s| &s.points) {
            assert!((p.x.hypot(p.y) - 20.0).abs() < 0.01, "{p:?}");
        }
        let gap_total = r.length_mm - cut_length(&r);
        assert!((gap_total - 12.0).abs() < 1e-6, "{gap_total}");
        // El recorrido empieza en el primer punto del contorno: el puente ideal no cae encima.
        assert_eq!(r.segments[0].points[0], c.points[0]);
    }

    #[test]
    fn a_gap_over_the_start_point_moves_the_start_after_it() {
        // Con centros repartidos el hueco casi nunca cae sobre el inicio; se prueba la rama
        // con `split` directamente: un hueco de 2 mm centrado en 0.5 y otro en 20.
        let pts = clean(&square(10.0).points);
        let cum = cumulative(&pts);
        let segs = split(&pts, &cum, &[0.5, 20.0], 2.0);
        // El hueco [-0.5, 1.5] cubre el inicio: el primer tramo arranca en 1.5.
        assert_eq!(segs.len(), 2);
        assert!((segs[0].points[0].x - 1.5).abs() < 1e-9);
        let covered: f64 = segs.iter().map(length).sum();
        assert!((covered - (40.0 - 4.0)).abs() < 1e-6, "{covered}");
        // El último tramo termina donde empieza el hueco que cubría el inicio (x = -0.5 -> 39.5).
        let last = segs.last().unwrap().points.last().unwrap();
        assert!(
            (last.x - 0.0).abs() < 1e-9 && (last.y - 0.5).abs() < 1e-9,
            "{last:?}"
        );
    }

    #[test]
    fn eligible_skips_open_and_inner_shapes() {
        let mut inner = square(5.0);
        inner.points.iter_mut().for_each(|p| {
            p.x += 10.0;
            p.y += 10.0;
        });
        let mut open = square(5.0);
        open.closed = false;
        let paths = vec![square(40.0), inner, open];
        assert_eq!(tab_eligible(&paths, true), vec![true, false, false]);
        assert_eq!(tab_eligible(&paths, false), vec![true, true, false]);
    }

    #[test]
    fn deterministic_and_never_panics_on_junk() {
        let a = tab_segments(&circle(15.0, 100), &opts(1.5, 5, 0.0));
        assert_eq!(a, tab_segments(&circle(15.0, 100), &opts(1.5, 5, 0.0)));
        let junk = Polyline {
            points: vec![
                Point::new(f64::NAN, 0.0),
                Point::new(1.0, 1.0),
                Point::new(2.0, 0.0),
            ],
            closed: true,
        };
        assert_eq!(tab_segments(&junk, &opts(1.0, 4, 0.0)).requested, 0);
        assert_eq!(
            tab_segments(
                &Polyline {
                    points: vec![],
                    closed: true
                },
                &opts(1.0, 4, 0.0)
            )
            .requested,
            0
        );
    }
}
