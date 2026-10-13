//! Puentes de las capas line: de los ajustes de la capa a los movimientos de la trayectoria.
//!
//! La pieza se planifica como forma cerrada (el planificador decide el orden y por dónde
//! entra) y recién entonces se parte con [`tlaku_geom::tab_segments`], así el orden de corte
//! no cambia por los puentes.
//!
//! El hueco entre dos tramos es un `Cut` y no un `Travel`. Con potencia de puente 0 los dos
//! dejan el láser apagado en GRBL (G1 con S0 / G0), pero un `Travel` es un G0 a velocidad
//! rápida: la máquina frena al cambiar de tipo de movimiento (así lo modela `tlaku-sim`), con un
//! hueco de 0,5 mm se sale del régimen de corte a cada puente y la estimación de tiempo se
//! infla. Con `Cut` la cabeza conserva la velocidad de corte, igual que el overscan, y la vista
//! previa lo dibuja como láser apagado (potencia 0) o como marcado (potencia > 0). El hueco que
//! cae justo al final del recorrido (cuando cubre el punto de partida) no se cruza.
//!
//! **Potencia de puente** (`TabSettings.power`): es un porcentaje **absoluto** de la potencia de
//! la máquina (0..=100), no relativo a la capa, y 0 la apaga. Si es mayor que 0 el hueco se
//! marca (sin atravesar la pieza) con esa potencia, pero nunca supera la del corte: se usa
//! `min(potencia de puente, potencia de corte del objeto)` (ver [`TabSpec::gap_power`]).

use std::collections::HashMap;
use tlaku_core::toolpath::Move;
use tlaku_core::{Point, Polyline};
use tlaku_doc::LayerSettings;
use tlaku_geom::{MAX_TABS_PER_SHAPE, TabOptions, TabResult, tab_segments};

/// Puentes ya validados de una capa line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TabSpec {
    pub options: TabOptions,
    pub skip_inner: bool,
    /// Potencia de los huecos entre tramos, % absoluto de la máquina (0 = láser apagado).
    pub power: f64,
}

impl TabSpec {
    /// Potencia con la que se cruzan los huecos de un objeto cuyo corte va a `cut_power` (%):
    /// la de puente, sin pasar nunca de la del corte.
    pub fn gap_power(&self, cut_power: f64) -> f64 {
        self.power.min(cut_power).max(0.0)
    }
}

/// Lee y valida los puentes de una capa line. `None` si no están activos.
pub(crate) fn resolve(l: &LayerSettings) -> Result<Option<TabSpec>, String> {
    let t = &l.tabs;
    if !t.enabled {
        return Ok(None);
    }
    let err = |m: String| format!("capa {}: puentes: {m}", l.color);
    if !(t.width.is_finite() && t.width > 0.0) {
        return Err(err(format!("el ancho debe ser mayor que 0: {}", t.width)));
    }
    if !(t.spacing.is_finite() && t.spacing >= 0.0) {
        return Err(err(format!(
            "la separación debe ser 0 o mayor: {}",
            t.spacing
        )));
    }
    if !(t.power.is_finite() && (0.0..=100.0).contains(&t.power)) {
        return Err(err(format!(
            "la potencia de puente debe estar entre 0 y 100: {}",
            t.power
        )));
    }
    if t.count as usize > MAX_TABS_PER_SHAPE {
        return Err(err(format!(
            "demasiados puentes por forma ({}); el máximo es {MAX_TABS_PER_SHAPE}",
            t.count
        )));
    }
    if t.count == 0 && t.spacing == 0.0 {
        return Err(err(
            "indica cuántos puentes (count) o cada cuántos mm (spacing)".into(),
        ));
    }
    Ok(Some(TabSpec {
        options: TabOptions {
            width_mm: t.width,
            count: t.count,
            spacing_mm: t.spacing,
        },
        skip_inner: t.skip_inner,
        power: t.power,
    }))
}

/// Resumen de los puentes de una capa, para el informe y las advertencias.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct TabStats {
    /// Puentes colocados en total.
    pub placed: usize,
    /// Formas donde no cupieron todos los puentes pedidos: (pedidos, colocados, perímetro mm).
    pub short: Vec<(usize, usize, f64)>,
    /// Formas cerradas con puentes activos que se procesaron (colocaran o no).
    pub shapes: usize,
}

impl TabStats {
    pub fn record(&mut self, r: &TabResult) {
        if r.requested == 0 {
            return;
        }
        self.shapes += 1;
        self.placed += r.placed;
        if !r.fits() {
            self.short.push((r.requested, r.placed, r.length_mm));
        }
    }

    /// Advertencia (o ninguna) con el estado de los puentes de la capa `name`.
    pub fn warning(&self, name: &str, width_mm: f64) -> Option<String> {
        if self.short.is_empty() {
            return None;
        }
        let none = self.short.iter().filter(|s| s.1 == 0).count();
        let shortest = self.short.iter().map(|s| s.2).fold(f64::INFINITY, f64::min);
        Some(format!(
            "capa {name}: los puentes de {width_mm} mm no caben en {} forma(s) (el contorno más \
             corto mide {shortest:.2} mm); {none} se cortan sin puentes y el resto con menos de los \
             pedidos. Los puentes no ocupan más de la mitad del contorno",
            self.short.len()
        ))
    }
}

/// Aviso del ancho de los puentes frente al kerf de la capa. `kerf_mm` es el desplazamiento por
/// lado (la mitad del ancho del corte), así que el haz se come `2 * |kerf|` del puente. Fuerte si
/// `width < 2 * |kerf|` (no queda puente: las piezas se sueltan); suave si `width < 4 * |kerf|`
/// (queda un puente de menos de un ancho de corte, que se rompe al manipular la placa).
pub(crate) fn kerf_warning(layer: &str, width_mm: f64, kerf_mm: f64) -> Option<String> {
    let k = kerf_mm.abs();
    if !(k.is_finite() && k > 0.0 && width_mm.is_finite()) {
        return None;
    }
    let cut = 2.0 * k;
    if width_mm < cut {
        Some(format!(
            "capa {layer}: los puentes de {width_mm} mm no sobreviven al corte: con kerf {kerf_mm} mm \
             por lado el haz quita {cut} mm y no queda puente (las piezas se soltarán); usa un ancho \
             de al menos {} mm",
            2.0 * cut
        ))
    } else if width_mm < 2.0 * cut {
        Some(format!(
            "capa {layer}: puentes frágiles: de {width_mm} mm quedan unos {:.2} mm tras el corte \
             (kerf {kerf_mm} mm por lado); se recomienda un ancho de al menos {} mm",
            width_mm - cut,
            2.0 * cut
        ))
    } else {
        None
    }
}

/// Agrega a `moves` el recorrido de una forma: `pieces` es el contorno entero (un solo
/// elemento) o los tramos que dejan los puentes. El primer tramo entra con un `Travel`; los
/// huecos entre tramos se cruzan con un `Cut` a `gap_pct` (0 = láser apagado).
pub(crate) fn push_shape(moves: &mut Vec<Move>, pieces: &[Polyline], power_pct: f64, gap_pct: f64) {
    let mut first = true;
    for piece in pieces.iter().filter(|p| !p.points.is_empty()) {
        let start: Point = piece.start();
        moves.push(if first {
            Move::Travel { to: start }
        } else {
            Move::Cut {
                to: start,
                power_pct: gap_pct,
            }
        });
        first = false;
        let rest = piece.points[1..]
            .iter()
            .chain(piece.closed.then_some(&piece.points[0]));
        moves.extend(rest.map(|&to| Move::Cut { to, power_pct }));
    }
}

/// Tramos de `path` según los puentes; el contorno entero si no corresponde partirlo.
pub(crate) fn split(path: &Polyline, spec: &TabSpec, stats: &mut TabStats) -> Vec<Polyline> {
    let r = tab_segments(path, &spec.options);
    stats.record(&r);
    r.segments
}

// ---------------------------------------------------------------------------
// Puentes manuales (`DesignObject::manual_tabs`)
// ---------------------------------------------------------------------------
//
// Si un objeto tiene puentes puestos a mano, éstos sustituyen a los automáticos de la capa para
// ese objeto (aunque sean menos, y sin la regla de omitir interiores). Cada punto se lleva al
// punto más cercano del contorno y ahí se centra un puente con el ancho de la capa.

/// Quita vértices repetidos consecutivos y el último si repite al primero.
fn clean(points: &[Point]) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(points.len());
    for &p in points {
        if out.last().is_none_or(|l| l.dist(p) > 1e-9) {
            out.push(p);
        }
    }
    while out.len() > 1 && out[0].dist(out[out.len() - 1]) <= 1e-9 {
        out.pop();
    }
    out
}

/// Posición (longitud de arco desde el primer vértice) del punto del contorno más cercano a `p`
/// y distancia a él. `pts` es el contorno cerrado ya limpio.
fn arc_position(pts: &[Point], cum: &[f64], p: Point) -> (f64, f64) {
    let n = pts.len();
    let mut best = (f64::INFINITY, 0.0);
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len2 = dx * dx + dy * dy;
        let t = if len2 > 0.0 {
            (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let q = Point::new(a.x + t * dx, a.y + t * dy);
        let d = p.dist(q);
        if d < best.0 {
            best = (d, cum[i] + t * (cum[i + 1] - cum[i]));
        }
    }
    (best.1, best.0)
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

/// Tramo abierto del contorno entre las longitudes de arco `u0 < u1` (con `u1 <= u0 + perímetro`).
fn piece(pts: &[Point], cum: &[f64], u0: f64, u1: f64) -> Polyline {
    let total = cum[pts.len()];
    let u0n = u0.rem_euclid(total);
    let u1n = u0n + (u1 - u0);
    let mut points = vec![point_at(pts, cum, u0n)];
    for lap in 0..2 {
        for (j, p) in pts.iter().enumerate() {
            let pos = cum[j] + f64::from(lap) * total;
            if pos > u0n + 1e-9 && pos < u1n - 1e-9 {
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

/// Tramos del contorno que quedan entre los huecos de ancho `width` centrados en `centers`
/// (ordenados). Igual que los puentes automáticos: si un hueco cubre el inicio del recorrido,
/// éste empieza al final de ese hueco y termina donde empieza.
fn split_at(pts: &[Point], cum: &[f64], centers: &[f64], width: f64) -> Vec<Polyline> {
    let total = cum[pts.len()];
    let half = width / 2.0;
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
        if a - from > 1e-9 {
            segments.push(piece(pts, cum, from, a));
        }
        from = b;
    }
    if start + total - from > 1e-9 {
        segments.push(piece(pts, cum, from, start + total));
    }
    segments
}

/// Para cada trayectoria, los puentes manuales que le tocan: `None` si su objeto no tiene (rige
/// la regla automática de la capa) y `Some(puntos)` si los tiene, aunque a ese contorno no le
/// caiga ninguno. Cada punto va a la trayectoria **cerrada** de su objeto más cercana.
/// `objects[i]` es el objeto de `paths[i]`.
pub(crate) fn assign_manual(
    paths: &[Polyline],
    objects: &[usize],
    manual: &HashMap<usize, Vec<Point>>,
) -> Vec<Option<Vec<Point>>> {
    let mut out: Vec<Option<Vec<Point>>> = vec![None; paths.len()];
    if manual.is_empty() {
        return out;
    }
    let mut by_object: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, &o) in objects.iter().enumerate().take(paths.len()) {
        if manual.contains_key(&o) {
            by_object.entry(o).or_default().push(i);
            out[i] = Some(Vec::new());
        }
    }
    for (object, indices) in &by_object {
        let closed: Vec<(usize, Vec<Point>, Vec<f64>)> = indices
            .iter()
            .filter(|&&i| paths[i].closed)
            .filter_map(|&i| {
                let pts = clean(&paths[i].points);
                (pts.len() >= 3).then(|| {
                    let cum = cumulative(&pts);
                    (i, pts, cum)
                })
            })
            .collect();
        for &p in &manual[object] {
            let nearest = closed
                .iter()
                .map(|(i, pts, cum)| (*i, arc_position(pts, cum, p).1))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((i, _)) = nearest {
                out[i].get_or_insert_with(Vec::new).push(p);
            }
        }
    }
    out
}

/// Longitud acumulada hasta cada vértice; el elemento `n` es el perímetro (con la arista de cierre).
fn cumulative(pts: &[Point]) -> Vec<f64> {
    let n = pts.len();
    let mut cum = Vec::with_capacity(n + 1);
    cum.push(0.0);
    for i in 0..n {
        cum.push(cum[i] + pts[i].dist(pts[(i + 1) % n]));
    }
    cum
}

/// Parte el contorno cerrado `path` con puentes de `width_mm` centrados en los puntos de
/// `points` (el más cercano del contorno a cada uno). Los puentes no se pisan ni ocupan más de la
/// mitad del perímetro: si sobran, se conservan los primeros de la lista. Un contorno abierto o
/// sin puntos se devuelve entero.
pub(crate) fn split_manual(
    path: &Polyline,
    points: &[Point],
    width_mm: f64,
    stats: &mut TabStats,
) -> Vec<Polyline> {
    let pts = clean(&path.points);
    if !path.closed
        || pts.len() < 3
        || points.is_empty()
        || !(width_mm.is_finite() && width_mm > 0.0)
    {
        return vec![path.clone()];
    }
    let cum = cumulative(&pts);
    let total = cum[pts.len()];
    if total <= 1e-9 {
        return vec![path.clone()];
    }
    let room = (total / (2.0 * width_mm)).floor() as usize;
    let cyclic = |a: f64, b: f64| {
        let d = (a - b).abs() % total;
        d.min(total - d)
    };
    let mut centers: Vec<f64> = Vec::new();
    let mut requested = 0;
    for &p in points {
        let u = arc_position(&pts, &cum, p).0;
        // Dos puntos tan juntos que sus puentes se pisarían son el mismo puente.
        if centers.iter().any(|&c| cyclic(c, u) < width_mm - 1e-9) {
            continue;
        }
        requested += 1;
        if centers.len() < room.min(MAX_TABS_PER_SHAPE) {
            centers.push(u);
        }
    }
    stats.record(&TabResult {
        segments: Vec::new(),
        requested,
        placed: centers.len(),
        length_mm: total,
    });
    if centers.is_empty() {
        return vec![path.clone()];
    }
    centers.sort_by(f64::total_cmp);
    split_at(&pts, &cum, &centers, width_mm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_core::Rgb;

    fn layer(edit: impl FnOnce(&mut LayerSettings)) -> LayerSettings {
        let mut l = LayerSettings::for_color(Rgb(255, 0, 0));
        l.tabs.enabled = true;
        edit(&mut l);
        l
    }

    #[test]
    fn disabled_tabs_resolve_to_none_even_with_bad_values() {
        let mut l = LayerSettings::for_color(Rgb(255, 0, 0));
        l.tabs.width = -3.0;
        assert_eq!(resolve(&l).unwrap(), None);
    }

    #[test]
    fn bad_values_name_the_layer() {
        for l in [
            layer(|l| l.tabs.width = 0.0),
            layer(|l| l.tabs.width = f64::NAN),
            layer(|l| l.tabs.spacing = -1.0),
            layer(|l| l.tabs.count = 5000),
            layer(|l| l.tabs.power = -1.0),
            layer(|l| l.tabs.power = 101.0),
            layer(|l| l.tabs.power = f64::NAN),
            layer(|l| {
                l.tabs.count = 0;
                l.tabs.spacing = 0.0
            }),
        ] {
            let e = resolve(&l).unwrap_err();
            assert!(e.starts_with("capa #ff0000: puentes:"), "{e}");
        }
        let ok = resolve(&layer(|l| {
            l.tabs.count = 0;
            l.tabs.spacing = 20.0
        }))
        .unwrap()
        .unwrap();
        assert_eq!(ok.options.spacing_mm, 20.0);
    }

    #[test]
    fn bridge_power_is_absolute_and_capped_by_the_cut() {
        let spec = resolve(&layer(|l| l.tabs.power = 25.0)).unwrap().unwrap();
        assert_eq!(spec.power, 25.0);
        assert_eq!(spec.gap_power(80.0), 25.0);
        // Nunca supera el corte (p. ej. con una escala de potencia por objeto baja).
        assert_eq!(spec.gap_power(10.0), 10.0);
        let off = resolve(&layer(|_| {})).unwrap().unwrap();
        assert_eq!(off.gap_power(80.0), 0.0);
    }

    #[test]
    fn push_shape_crosses_gaps_with_laser_off_cuts() {
        let piece = |pts: &[(f64, f64)], closed| Polyline {
            points: pts.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            closed,
        };
        let mut moves = Vec::new();
        push_shape(
            &mut moves,
            &[
                piece(&[(0.0, 0.0), (4.0, 0.0)], false),
                piece(&[(6.0, 0.0), (10.0, 0.0)], false),
            ],
            30.0,
            0.0,
        );
        let mut marked = Vec::new();
        push_shape(
            &mut marked,
            &[
                piece(&[(0.0, 0.0), (4.0, 0.0)], false),
                piece(&[(6.0, 0.0), (10.0, 0.0)], false),
            ],
            30.0,
            12.0,
        );
        assert_eq!(
            marked[2],
            Move::Cut {
                to: Point::new(6.0, 0.0),
                power_pct: 12.0
            }
        );
        assert_eq!(
            moves,
            vec![
                Move::Travel {
                    to: Point::new(0.0, 0.0)
                },
                Move::Cut {
                    to: Point::new(4.0, 0.0),
                    power_pct: 30.0
                },
                Move::Cut {
                    to: Point::new(6.0, 0.0),
                    power_pct: 0.0
                },
                Move::Cut {
                    to: Point::new(10.0, 0.0),
                    power_pct: 30.0
                },
            ]
        );
        // Un contorno cerrado sin partir se cierra solo.
        let mut closed = Vec::new();
        push_shape(
            &mut closed,
            &[piece(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)], true)],
            10.0,
            0.0,
        );
        assert_eq!(closed.len(), 4);
        assert_eq!(closed[3].to(), Point::new(0.0, 0.0));
    }

    #[test]
    fn warning_counts_shapes_and_mentions_the_shortest() {
        let mut s = TabStats::default();
        assert_eq!(s.warning("#ff0000", 2.0), None);
        s.record(&TabResult {
            segments: vec![],
            requested: 4,
            placed: 0,
            length_mm: 3.0,
        });
        s.record(&TabResult {
            segments: vec![],
            requested: 4,
            placed: 2,
            length_mm: 9.0,
        });
        s.record(&TabResult {
            segments: vec![],
            requested: 4,
            placed: 4,
            length_mm: 90.0,
        });
        assert_eq!((s.shapes, s.placed), (3, 6));
        let w = s.warning("#ff0000", 2.0).unwrap();
        assert!(
            w.contains("2 forma(s)") && w.contains("3.00 mm") && w.contains("1 se cortan sin"),
            "{w}"
        );
    }
}
