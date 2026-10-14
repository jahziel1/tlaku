//! Ajustes de trazo de las capas line (ola 2: C13, C15, C16, C17, C18, C20 y la escala de
//! potencia de C31): perforación, modo punto, entradas y salidas, overcut, rampa, pausas y
//! corte pasante.
//!
//! # Orden de aplicación
//!
//! Cada forma ya planificada pasa por esta cadena (los pasos que no estén activos se saltan):
//!
//! 1. **kerf** y **plan** (en `build::line_layer`, antes de llegar aquí).
//! 2. **puentes** ([`crate::tabs`]): el trazo es el contorno entero o sus tramos, unidos por
//!    huecos a potencia 0.
//! 3. **escala de potencia** del objeto (`DesignObject.power_scale`): ya viene en `power`.
//! 4. **modo punto** *o* **perforación**: el modo punto reemplaza la línea por disparos; la
//!    perforación modula el trazo con tramos de corte y de hueco medidos desde su inicio.
//! 5. **overcut**, **salida** y **entrada** (en ese orden de cálculo; el resultado es entrada,
//!    contorno, overcut, salida). Solo en contornos cerrados enteros (sin puentes).
//! 6. **rampa**: sobre el trazo completo, entrada y salida incluidas.
//! 7. **pausas** de inicio y fin, y **corte pasante**: pausas con el láser encendido en el
//!    primer punto (entrada incluida) y en el último. La pausa de inicio va antes del corte
//!    pasante.
//!
//! # Combinaciones
//!
//! Gana el ajuste más específico y el otro se ignora con un aviso (`capa #rrggbb: ...`):
//!
//! - modo punto + perforación: se ignora la perforación (los puntos ya son intermitentes);
//! - modo punto + rampa: se ignora la rampa (cada punto dispara a la potencia de la capa);
//! - modo punto + entrada, salida u overcut: se ignoran (no hay una línea que alargar);
//! - modo punto + pausas o corte pasante: se ignoran (cada punto ya tiene su pausa).
//!
//! Los puentes sí se combinan con todo lo demás, salvo entrada, salida y overcut: una forma con
//! puentes no es un contorno cerrado entero (se avisa).

use tlaku_core::toolpath::Move;
use tlaku_core::{Point, Polyline};
use tlaku_doc::{LayerSettings, LeadKind};
use tlaku_geom::{PointLocation, locate_point, signed_area};
use tlaku_i18n::tr;

/// Período mínimo (mm) de la perforación y separación mínima del modo punto.
const MIN_PERIOD_MM: f64 = 0.01;
/// Tope de tramos o puntos por trazo: protege contra G-code gigantesco por un dedo de más.
const MAX_PIECES: f64 = 200_000.0;
/// Paso (mm) de la escalera con que se aproxima la rampa lineal de potencia.
const RAMP_STEP_MM: f64 = 0.25;
/// Paso angular (grados) con que se discretiza un arco de entrada o salida.
const ARC_STEP_DEG: f64 = 5.0;
/// Tolerancia (mm) para decidir si dos puntos coinciden.
const EPS: f64 = 1e-9;
/// Fracciones de la longitud que se prueban antes de renunciar a una entrada o salida.
const LEAD_TRIES: [f64; 3] = [1.0, 0.5, 0.25];

#[derive(Debug, Clone, Copy, PartialEq)]
struct LeadSpec {
    kind: LeadKind,
    length: f64,
    angle: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RampSpec {
    length_mm: f64,
    /// 0..1.
    start: f64,
    outer_only: bool,
}

/// Ajustes de trazo ya validados de una capa line. Solo existe si hay algo activo.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StrokeSpec {
    perforation: Option<(f64, f64)>,
    lead_in: Option<LeadSpec>,
    lead_out: Option<LeadSpec>,
    overcut: f64,
    start_delay_ms: f64,
    end_delay_ms: f64,
    /// (% de potencia, ms).
    cut_through: Option<(f64, f64)>,
    /// (ms por punto, separación mm).
    dot: Option<(f64, f64)>,
    ramp: Option<RampSpec>,
    /// Combinaciones incompatibles, ya sin los ajustes descartados.
    conflicts: Vec<String>,
}

fn finite_in(v: f64, lo: f64, hi: f64) -> bool {
    v.is_finite() && (lo..=hi).contains(&v)
}

fn resolve_lead(l: &tlaku_doc::Lead, what: &str) -> Result<Option<LeadSpec>, String> {
    if !l.enabled {
        return Ok(None);
    }
    if !(l.length.is_finite() && l.length > 0.0) {
        return Err(tr!(
            "pipeline.stroke.lead_bad_length",
            what = what,
            value = l.length
        ));
    }
    let (max, unit) = match l.kind {
        LeadKind::Straight => (90.0, tr!("pipeline.stroke.lead_angle")),
        LeadKind::Arc => (180.0, tr!("pipeline.stroke.lead_sweep")),
    };
    if !(l.angle.is_finite() && l.angle > 0.0 && l.angle <= max) {
        return Err(tr!(
            "pipeline.stroke.lead_bad_angle",
            what = what,
            unit = unit,
            max = max,
            value = l.angle
        ));
    }
    Ok(Some(LeadSpec {
        kind: l.kind,
        length: l.length,
        angle: l.angle,
    }))
}

/// Lee y valida los ajustes de trazo de una capa line. `None` si no hay ninguno activo. Los
/// ajustes desactivados no se validan (como los puentes).
pub(crate) fn resolve(l: &LayerSettings) -> Result<Option<StrokeSpec>, String> {
    let err = |m: String| tr!("pipeline.stroke.layer_error", layer = l.color, error = m);
    let perforation = if l.perforation.enabled {
        let p = &l.perforation;
        if !(p.cut_mm.is_finite() && p.cut_mm > 0.0 && p.skip_mm.is_finite() && p.skip_mm > 0.0) {
            return Err(err(tr!(
                "pipeline.stroke.perforation_bad",
                cut = p.cut_mm,
                skip = p.skip_mm
            )));
        }
        if p.cut_mm + p.skip_mm < MIN_PERIOD_MM {
            return Err(err(tr!(
                "pipeline.stroke.perforation_too_fine",
                period = p.cut_mm + p.skip_mm,
                min = MIN_PERIOD_MM
            )));
        }
        Some((p.cut_mm, p.skip_mm))
    } else {
        None
    };
    let lead_in = resolve_lead(&l.lead_in, &tr!("pipeline.stroke.lead_in")).map_err(&err)?;
    let lead_out = resolve_lead(&l.lead_out, &tr!("pipeline.stroke.lead_out")).map_err(&err)?;
    if !(l.overcut.is_finite() && l.overcut >= 0.0) {
        return Err(err(tr!("pipeline.stroke.bad_overcut", value = l.overcut)));
    }
    if !(l.start_delay_ms.is_finite() && l.start_delay_ms >= 0.0) {
        return Err(err(tr!(
            "pipeline.stroke.bad_start_delay",
            value = l.start_delay_ms
        )));
    }
    if !(l.end_delay_ms.is_finite() && l.end_delay_ms >= 0.0) {
        return Err(err(tr!(
            "pipeline.stroke.bad_end_delay",
            value = l.end_delay_ms
        )));
    }
    let cut_through = if l.cut_through.enabled {
        let c = &l.cut_through;
        if !(finite_in(c.power, 0.0, 100.0) && c.ms.is_finite() && c.ms > 0.0) {
            return Err(err(tr!(
                "pipeline.stroke.cut_through_bad",
                power = c.power,
                ms = c.ms
            )));
        }
        Some((c.power, c.ms))
    } else {
        None
    };
    let dot = if l.dot_mode.enabled {
        let d = &l.dot_mode;
        let ok = d.dwell_ms.is_finite()
            && d.dwell_ms > 0.0
            && d.spacing_mm.is_finite()
            && d.spacing_mm >= MIN_PERIOD_MM;
        if !ok {
            return Err(err(tr!(
                "pipeline.stroke.dot_bad",
                min = MIN_PERIOD_MM,
                ms = d.dwell_ms,
                mm = d.spacing_mm
            )));
        }
        Some((d.dwell_ms, d.spacing_mm))
    } else {
        None
    };
    let ramp = if l.ramp.enabled {
        let r = &l.ramp;
        if !(r.length_mm.is_finite() && r.length_mm > 0.0 && finite_in(r.start_pct, 0.0, 100.0)) {
            return Err(err(tr!(
                "pipeline.stroke.ramp_bad",
                mm = r.length_mm,
                pct = r.start_pct
            )));
        }
        Some(RampSpec {
            length_mm: r.length_mm,
            start: r.start_pct / 100.0,
            outer_only: r.outer_only,
        })
    } else {
        None
    };
    let mut spec = StrokeSpec {
        perforation,
        lead_in,
        lead_out,
        overcut: l.overcut,
        start_delay_ms: l.start_delay_ms,
        end_delay_ms: l.end_delay_ms,
        cut_through,
        dot,
        ramp,
        conflicts: Vec::new(),
    };
    let any = spec.perforation.is_some()
        || spec.lead_in.is_some()
        || spec.lead_out.is_some()
        || spec.overcut > 0.0
        || spec.start_delay_ms > 0.0
        || spec.end_delay_ms > 0.0
        || spec.cut_through.is_some()
        || spec.dot.is_some()
        || spec.ramp.is_some();
    if !any {
        return Ok(None);
    }
    spec.drop_conflicts();
    Ok(Some(spec))
}

impl StrokeSpec {
    /// El modo punto gana: descarta lo que no tiene sentido junto a él y lo anota.
    fn drop_conflicts(&mut self) {
        if self.dot.is_none() {
            return;
        }
        let mut note = |message: String| self.conflicts.push(message);
        if self.perforation.take().is_some() {
            note(tr!("pipeline.stroke.dot_ignores_perforation"));
        }
        if self.ramp.take().is_some() {
            note(tr!("pipeline.stroke.dot_ignores_ramp"));
        }
        let leads = [
            self.lead_in.take().is_some(),
            self.lead_out.take().is_some(),
            std::mem::take(&mut self.overcut) > 0.0,
        ];
        if leads.contains(&true) {
            note(tr!("pipeline.stroke.dot_ignores_leads"));
        }
        let pauses = [
            self.cut_through.take().is_some(),
            std::mem::take(&mut self.start_delay_ms) > 0.0,
            std::mem::take(&mut self.end_delay_ms) > 0.0,
        ];
        if pauses.contains(&true) {
            note(tr!("pipeline.stroke.dot_ignores_pauses"));
        }
    }

    fn wants_closed_ops(&self) -> bool {
        self.lead_in.is_some() || self.lead_out.is_some() || self.overcut > 0.0
    }
}

/// Lo que pasó al aplicar los ajustes a una capa, para el aviso.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct StrokeStats {
    /// Entradas o salidas que hubo que acortar para no cruzar la forma.
    pub shortened: usize,
    /// Entradas o salidas que no cupieron ni acortadas y se omitieron.
    pub dropped: usize,
    /// Trayectorias abiertas a las que no se les puso entrada, salida ni overcut.
    pub open_skipped: usize,
    /// Formas con puentes a las que no se les puso entrada, salida ni overcut.
    pub tabbed_skipped: usize,
    /// Overcut mayor que el perímetro, limitado a una vuelta.
    pub overcut_clamped: usize,
    /// Trazos tan largos para su patrón que se dejaron sin perforar o sin puntos.
    pub too_fine: usize,
    /// Trayectorias cuya potencia escalada pasó de 100 % y se limitó.
    pub power_clamped: usize,
}

impl StrokeStats {
    /// Avisos de la capa `name`, con el prefijo de la capa.
    pub fn warnings(&self, name: &str, spec: Option<&StrokeSpec>) -> Vec<String> {
        let mut out: Vec<String> = spec
            .map(|s| {
                s.conflicts
                    .iter()
                    .map(|c| tr!("pipeline.stroke.layer_warning", name = name, message = c))
                    .collect()
            })
            .unwrap_or_default();
        let mut push = |n: usize, msg: String| {
            if n > 0 {
                out.push(tr!(
                    "pipeline.stroke.layer_warning",
                    name = name,
                    message = msg
                ));
            }
        };
        push(
            self.shortened,
            tr!("pipeline.stroke.shortened", n = self.shortened),
        );
        push(
            self.dropped,
            tr!("pipeline.stroke.dropped", n = self.dropped),
        );
        push(
            self.open_skipped,
            tr!("pipeline.stroke.open_skipped", n = self.open_skipped),
        );
        push(
            self.tabbed_skipped,
            tr!("pipeline.stroke.tabbed_skipped", n = self.tabbed_skipped),
        );
        push(
            self.overcut_clamped,
            tr!("pipeline.stroke.overcut_clamped", n = self.overcut_clamped),
        );
        push(
            self.too_fine,
            tr!(
                "pipeline.stroke.too_fine",
                n = self.too_fine,
                max = MAX_PIECES
            ),
        );
        push(
            self.power_clamped,
            tr!("pipeline.stroke.power_clamped", n = self.power_clamped),
        );
        out
    }
}

/// Potencia de un objeto: la de la capa por su escala (`power_scale / 100`), limitada a 100 %.
pub(crate) fn scaled_power(layer_pct: f64, scale: f64, stats: &mut StrokeStats) -> f64 {
    let p = layer_pct * scale;
    if p > 100.0 {
        stats.power_clamped += 1;
        100.0
    } else {
        p
    }
}

/// Trazo en construcción: `pts[0]` es donde entra el láser y `pw[i]` la potencia (%) del
/// segmento `pts[i] -> pts[i + 1]`; 0 = hueco que se cruza con el láser apagado.
#[derive(Debug, Clone)]
struct Stroke {
    pts: Vec<Point>,
    pw: Vec<f64>,
}

impl Stroke {
    /// Une los tramos como hace `tabs::push_shape`: los huecos entre tramos son segmentos a
    /// `gap` (0 = láser apagado; la potencia de puente si es mayor).
    fn from_pieces(pieces: &[Polyline], power: f64, gap: f64) -> Option<Stroke> {
        let mut pts: Vec<Point> = Vec::new();
        let mut pw = Vec::new();
        for piece in pieces.iter().filter(|p| !p.points.is_empty()) {
            if pts.is_empty() {
                pts.push(piece.start());
            } else {
                pts.push(piece.start());
                pw.push(gap);
            }
            let rest = piece
                .points
                .iter()
                .skip(1)
                .chain(piece.closed.then_some(&piece.points[0]));
            for &p in rest {
                pts.push(p);
                pw.push(power);
            }
        }
        (!pts.is_empty()).then_some(Stroke { pts, pw })
    }

    fn length(&self) -> f64 {
        self.pts.windows(2).map(|w| w[0].dist(w[1])).sum()
    }

    /// Parte el trazo en las longitudes `cuts` (ascendentes, medidas desde el inicio) y
    /// multiplica la potencia de cada trocito por `factor(punto medio)`.
    fn modulate(&self, cuts: &[f64], factor: impl Fn(f64) -> f64) -> Stroke {
        let mut pts = vec![self.pts[0]];
        let mut pw = Vec::new();
        let (mut s0, mut ci) = (0.0, 0);
        for (i, &power) in self.pw.iter().enumerate() {
            let (a, b) = (self.pts[i], self.pts[i + 1]);
            let len = a.dist(b);
            let s1 = s0 + len;
            let mut sf = s0;
            while ci < cuts.len() && cuts[ci] < s1 - EPS {
                let c = cuts[ci];
                ci += 1;
                if c <= sf + EPS {
                    continue;
                }
                let t = (c - s0) / len;
                pts.push(Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t));
                pw.push(power * factor((sf + c) / 2.0));
                sf = c;
            }
            pts.push(b);
            pw.push(power * factor((sf + s1) / 2.0));
            s0 = s1;
        }
        Stroke { pts, pw }
    }

    /// Posición y potencia del segmento en la longitud `s` (la del segmento que empieza ahí).
    fn at(&self, s: f64) -> (Point, f64) {
        let mut acc = 0.0;
        for (i, &power) in self.pw.iter().enumerate() {
            let (a, b) = (self.pts[i], self.pts[i + 1]);
            let len = a.dist(b);
            if s < acc + len || i + 1 == self.pw.len() {
                let t = if len > 0.0 {
                    ((s - acc) / len).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                return (
                    Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t),
                    power,
                );
            }
            acc += len;
        }
        (self.pts[0], 0.0)
    }

    /// Los primeros `mm` del trazo (puntos y potencias), sin pasar de una vuelta.
    fn head(&self, mm: f64) -> (Vec<Point>, Vec<f64>) {
        let (mut pts, mut pw) = (Vec::new(), Vec::new());
        let mut left = mm;
        for (i, &power) in self.pw.iter().enumerate() {
            let (a, b) = (self.pts[i], self.pts[i + 1]);
            let len = a.dist(b);
            if left <= EPS {
                break;
            }
            if len <= left {
                pts.push(b);
                left -= len;
            } else {
                let t = left / len;
                pts.push(Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t));
                left = 0.0;
            }
            pw.push(power);
        }
        (pts, pw)
    }

    fn first_on(&self) -> Option<f64> {
        self.pw.iter().copied().find(|p| *p > 0.0)
    }

    fn last_on(&self) -> Option<f64> {
        self.pw.iter().rev().copied().find(|p| *p > 0.0)
    }
}

type V = (f64, f64);

fn unit(a: Point, b: Point) -> Option<V> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len = dx.hypot(dy);
    (len > EPS).then_some((dx / len, dy / len))
}

/// Dirección media de dos tangentes unitarias (la segunda si van justo en contra).
fn bisector(a: V, b: V) -> V {
    let (x, y) = (a.0 + b.0, a.1 + b.1);
    let len = x.hypot(y);
    if len < 1e-9 { b } else { (x / len, y / len) }
}

fn cross(a: V, b: V) -> f64 {
    a.0 * b.1 - a.1 * b.0
}

fn offset_pt(p: Point, v: V, k: f64) -> Point {
    Point::new(p.x + v.0 * k, p.y + v.1 * k)
}

fn rot(v: V, rad: f64) -> V {
    let (s, c) = rad.sin_cos();
    (v.0 * c - v.1 * s, v.0 * s + v.1 * c)
}

/// Puntos de una entrada (`incoming`: terminan en `p`) o salida (empiezan en `p`) de largo
/// `length`, con `t` la tangente del contorno en `p` y `nw` la normal hacia el lado sobrante.
fn lead_points(spec: LeadSpec, length: f64, p: Point, t: V, nw: V, incoming: bool) -> Vec<Point> {
    let a = spec.angle.to_radians();
    match spec.kind {
        LeadKind::Straight => {
            let q = if incoming {
                // Llega a `p` con el ángulo `a` respecto a la tangente.
                offset_pt(offset_pt(p, t, -length * a.cos()), nw, length * a.sin())
            } else {
                offset_pt(offset_pt(p, t, length * a.cos()), nw, length * a.sin())
            };
            if incoming { vec![q, p] } else { vec![p, q] }
        }
        LeadKind::Arc => {
            // `length` es el largo del arco; el radio sale del barrido.
            let r = length / a;
            let c = offset_pt(p, nw, r);
            let up = (p.x - c.x, p.y - c.y);
            let sigma = if cross(up, t) >= 0.0 { 1.0 } else { -1.0 };
            let n = (spec.angle / ARC_STEP_DEG).ceil().max(1.0) as usize;
            let on_arc = |theta: f64| {
                let v = rot(up, sigma * theta);
                Point::new(c.x + v.0, c.y + v.1)
            };
            let mut pts: Vec<Point> = if incoming {
                // Del extremo del arco hasta `p`, girando en el sentido de la marcha.
                (0..=n)
                    .map(|k| on_arc(-a * (n - k) as f64 / n as f64))
                    .collect()
            } else {
                (0..=n).map(|k| on_arc(a * k as f64 / n as f64)).collect()
            };
            // El punto de contacto es exactamente `p`, sin error de redondeo.
            if incoming {
                *pts.last_mut().unwrap() = p;
            } else {
                pts[0] = p;
            }
            pts
        }
    }
}

/// Verdadero si el segmento `a-b` toca al `c-d` en algún punto distinto de `anchor`.
fn seg_hits(a: Point, b: Point, c: Point, d: Point, anchor: Point) -> bool {
    let r = (b.x - a.x, b.y - a.y);
    let s = (d.x - c.x, d.y - c.y);
    let ca = (c.x - a.x, c.y - a.y);
    let rxs = cross(r, s);
    let scale = r.0.hypot(r.1) * s.0.hypot(s.1);
    if rxs.abs() <= 1e-12 * scale.max(1.0) {
        // Paralelos: solo cuenta si son colineales y se encimen en un tramo.
        if cross(ca, r).abs() > 1e-9 * r.0.hypot(r.1).max(1.0) {
            return false;
        }
        let rr = r.0 * r.0 + r.1 * r.1;
        if rr <= 0.0 {
            return false;
        }
        let t0 = (ca.0 * r.0 + ca.1 * r.1) / rr;
        let t1 = t0 + (s.0 * r.0 + s.1 * r.1) / rr;
        let (lo, hi) = (t0.min(t1).max(0.0), t0.max(t1).min(1.0));
        return (hi - lo) * rr.sqrt() > 1e-7;
    }
    let t = cross(ca, s) / rxs;
    let u = cross(ca, r) / rxs;
    if !((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)) {
        return false;
    }
    let hit = Point::new(a.x + r.0 * t, a.y + r.1 * t);
    hit.dist(anchor) > 1e-7
}

/// La entrada o salida no cruza el contorno `poly` y queda en el lado sobrante (`waste_outside`:
/// fuera del polígono; si no, dentro).
fn lead_ok(lead: &[Point], anchor: Point, poly: &[Point], waste_outside: bool) -> bool {
    let n = poly.len();
    for w in lead.windows(2) {
        if (0..n).any(|i| seg_hits(w[0], w[1], poly[i], poly[(i + 1) % n], anchor)) {
            return false;
        }
        let mid = Point::new((w[0].x + w[1].x) / 2.0, (w[0].y + w[1].y) / 2.0);
        for p in [w[0], mid] {
            if p.dist(anchor) <= 1e-7 {
                continue;
            }
            let side = locate_point(p, poly);
            let good = if waste_outside {
                side == PointLocation::Outside
            } else {
                side == PointLocation::Inside
            };
            if !good {
                return false;
            }
        }
    }
    true
}

/// Contorno cerrado entero sobre el que se calculan entrada, salida y overcut.
struct Closed<'a> {
    poly: &'a [Point],
    /// Profundidad par/impar de la forma: par = pieza (lo sobrante queda fuera), impar = hueco.
    waste_outside: bool,
    ccw: bool,
}

impl Closed<'_> {
    /// Normal unitaria hacia el lado sobrante para un tramo con tangente `t`.
    fn waste_normal(&self, t: V) -> V {
        let right = (t.1, -t.0);
        // Con giro antihorario el material está a la izquierda: el exterior es la derecha.
        if self.ccw == self.waste_outside {
            right
        } else {
            (-right.0, -right.1)
        }
    }

    /// Entrada o salida en `p` con tangente `t`; la acorta si cruza la forma. `None` (y se
    /// cuenta) si ni acortada cabe.
    fn lead(
        &self,
        spec: LeadSpec,
        p: Point,
        t: V,
        incoming: bool,
        stats: &mut StrokeStats,
    ) -> Option<Vec<Point>> {
        let nw = self.waste_normal(t);
        for (k, f) in LEAD_TRIES.iter().enumerate() {
            let pts = lead_points(spec, spec.length * f, p, t, nw, incoming);
            if lead_ok(&pts, p, self.poly, self.waste_outside) {
                if k > 0 {
                    stats.shortened += 1;
                }
                return Some(pts);
            }
        }
        stats.dropped += 1;
        None
    }
}

fn dwell(to: Point, ms: f64, power_pct: f64) -> Move {
    Move::Dwell { to, ms, power_pct }
}

/// Una forma planificada y lo que hace falta saber de ella para trazarla.
pub(crate) struct ShapeCtx<'a> {
    /// La forma tal como la planificó el plan.
    pub shape: &'a Polyline,
    /// El contorno entero (un solo elemento) o los tramos que dejan los puentes.
    pub pieces: &'a [Polyline],
    /// Potencia de la capa por la escala del objeto, %.
    pub power: f64,
    /// Potencia de los huecos entre tramos de puente, % (0 = láser apagado; nunca mayor que `power`).
    pub tab_power: f64,
    /// La escala sola (`power_scale / 100`); también escala el corte pasante.
    pub scale: f64,
    /// Profundidad de anidamiento (par = exterior, impar = hueco).
    pub depth: usize,
}

/// Agrega a `out` los movimientos de una forma con los ajustes de `spec`.
pub(crate) fn push_shape(
    spec: &StrokeSpec,
    ctx: &ShapeCtx,
    stats: &mut StrokeStats,
    out: &mut Vec<Move>,
) {
    let ShapeCtx {
        shape,
        pieces,
        power,
        tab_power,
        scale,
        depth,
    } = *ctx;
    let Some(mut stroke) = Stroke::from_pieces(pieces, power, tab_power) else {
        return;
    };
    let total = stroke.length();

    if let Some((dwell_ms, spacing)) = spec.dot {
        push_dots(
            &stroke,
            shape.closed && pieces.len() == 1,
            dwell_ms,
            spacing,
            stats,
            out,
        );
        return;
    }

    if let Some((cut, skip)) = spec.perforation {
        let period = cut + skip;
        if total / period > MAX_PIECES {
            stats.too_fine += 1;
        } else {
            let mut cuts = Vec::new();
            let mut k = 0.0;
            while k * period < total {
                cuts.push(k * period + cut);
                cuts.push((k + 1.0) * period);
                k += 1.0;
            }
            stroke = stroke.modulate(&cuts, |s| if s % period < cut { 1.0 } else { 0.0 });
        }
    }

    let whole_closed = pieces.len() == 1 && shape.closed && shape.points.len() >= 3;
    if spec.wants_closed_ops() {
        if whole_closed {
            apply_closed(spec, shape, depth, &mut stroke, stats);
        } else if shape.closed {
            stats.tabbed_skipped += 1;
        } else {
            stats.open_skipped += 1;
        }
    }

    if let Some(r) = spec.ramp
        && (!r.outer_only || (whole_closed && depth.is_multiple_of(2)))
    {
        stroke = ramp(&stroke, r);
    }

    // Pausas con el láser encendido: a la potencia del primer y del último tramo que corta.
    let first = stroke.first_on().unwrap_or(power);
    let last = stroke.last_on().unwrap_or(power);
    let begin = stroke.pts[0];
    out.push(Move::Travel { to: begin });
    if spec.start_delay_ms > 0.0 {
        out.push(dwell(begin, spec.start_delay_ms, first));
    }
    if let Some((pct, ms)) = spec.cut_through
        && shape.closed
    {
        out.push(dwell(begin, ms, (pct * scale).min(100.0)));
    }
    for (p, w) in stroke.pts.iter().skip(1).zip(&stroke.pw) {
        out.push(Move::Cut {
            to: *p,
            power_pct: *w,
        });
    }
    if spec.end_delay_ms > 0.0 {
        out.push(dwell(*stroke.pts.last().unwrap(), spec.end_delay_ms, last));
    }
}

/// Overcut, salida y entrada sobre un contorno cerrado entero.
fn apply_closed(
    spec: &StrokeSpec,
    shape: &Polyline,
    depth: usize,
    stroke: &mut Stroke,
    stats: &mut StrokeStats,
) {
    let area = signed_area(&shape.points);
    if area == 0.0 || !area.is_finite() {
        stats.dropped += usize::from(spec.lead_in.is_some()) + usize::from(spec.lead_out.is_some());
        return;
    }
    let closed = Closed {
        poly: &shape.points,
        waste_outside: depth.is_multiple_of(2),
        ccw: area > 0.0,
    };
    let total = stroke.length();
    // En el punto de inicio la tangente es la bisectriz de la llegada y la salida: así la
    // entrada cabe también en una esquina (donde la tangente de un solo lado se saldría).
    let poly = &shape.points;
    let n = poly.len();
    let corner = match (unit(poly[n - 1], poly[0]), unit(poly[0], poly[1])) {
        (Some(a), Some(b)) => Some(bisector(a, b)),
        (a, b) => a.or(b),
    };
    if spec.overcut > 0.0 {
        let mm = if spec.overcut > total {
            stats.overcut_clamped += 1;
            total
        } else {
            spec.overcut
        };
        let (pts, pw) = stroke.head(mm);
        stroke.pts.extend(pts);
        stroke.pw.extend(pw);
    }
    if let Some(lead) = spec.lead_out {
        let n = stroke.pts.len();
        // Sin overcut se sale desde el inicio (la esquina); con overcut, a mitad de un lado,
        // con la tangente del último tramo que avanzó.
        let t = if spec.overcut > 0.0 {
            (1..n)
                .rev()
                .find_map(|i| unit(stroke.pts[i - 1], stroke.pts[i]))
        } else {
            corner
        };
        if let (Some(t), Some(p)) = (t, stroke.pts.last().copied())
            && let Some(pts) = closed.lead(lead, p, t, false, stats)
        {
            let power = stroke.last_on().unwrap_or(0.0);
            for q in pts.into_iter().skip(1) {
                stroke.pts.push(q);
                stroke.pw.push(power);
            }
        }
    }
    if let Some(lead) = spec.lead_in
        && let Some(t) = corner
        && let Some(pts) = closed.lead(lead, stroke.pts[0], t, true, stats)
    {
        let power = stroke.first_on().unwrap_or(0.0);
        let n = pts.len() - 1;
        let mut new_pts: Vec<Point> = pts[..n].to_vec();
        new_pts.extend(stroke.pts.iter().copied());
        let mut new_pw = vec![power; n];
        new_pw.extend(stroke.pw.iter().copied());
        stroke.pts = new_pts;
        stroke.pw = new_pw;
    }
}

/// Rampa lineal de potencia al empezar y al terminar, como escalera de pasos de
/// [`RAMP_STEP_MM`] (mínimo 4 y máximo 40 por rampa).
fn ramp(stroke: &Stroke, r: RampSpec) -> Stroke {
    let total = stroke.length();
    let len = r.length_mm.min(total);
    if len <= EPS {
        return stroke.clone();
    }
    let steps = (len / RAMP_STEP_MM).ceil().clamp(4.0, 40.0) as usize;
    let mut cuts: Vec<f64> = Vec::new();
    for i in 1..=steps {
        let d = len * i as f64 / steps as f64;
        cuts.push(d);
        cuts.push(total - d);
    }
    cuts.retain(|c| *c > EPS && *c < total - EPS);
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() < EPS);
    stroke.modulate(&cuts, |s| {
        let d = s.min(total - s);
        if d >= r.length_mm {
            1.0
        } else {
            r.start + (1.0 - r.start) * d / r.length_mm
        }
    })
}

/// Modo punto: un disparo de `dwell_ms` cada `spacing` mm. Entre disparos la cabeza avanza con
/// un `Cut` a potencia 0 (mantiene la velocidad de la capa, como los huecos de los puentes). Los
/// puntos que caen sobre un hueco de puente no disparan, salvo que el puente tenga potencia de
/// puente: entonces disparan con ella.
fn push_dots(
    stroke: &Stroke,
    closed: bool,
    dwell_ms: f64,
    spacing: f64,
    stats: &mut StrokeStats,
    out: &mut Vec<Move>,
) {
    let total = stroke.length();
    if total / spacing > MAX_PIECES {
        stats.too_fine += 1;
        return;
    }
    let mut at_s: Vec<f64> = if closed {
        // La vuelta se reparte en partes iguales y el punto de cierre no se repite.
        let n = (total / spacing).round().max(1.0) as usize;
        (0..n).map(|i| total * i as f64 / n as f64).collect()
    } else {
        let n = ((total + EPS) / spacing).floor() as usize;
        (0..=n).map(|i| i as f64 * spacing).collect()
    };
    if !closed && total - at_s.last().copied().unwrap_or(0.0) > spacing * 0.01 {
        at_s.push(total);
    }
    let mut first = true;
    for s in at_s {
        let (p, w) = stroke.at(s);
        if first {
            out.push(Move::Travel { to: p });
            first = false;
        } else {
            out.push(Move::Cut {
                to: p,
                power_pct: 0.0,
            });
        }
        if w > 0.0 {
            out.push(dwell(p, dwell_ms, w));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_core::Rgb;

    fn pt(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn poly(pts: &[(f64, f64)], closed: bool) -> Polyline {
        Polyline {
            points: pts.iter().map(|&(x, y)| pt(x, y)).collect(),
            closed,
        }
    }

    fn square(x: f64, y: f64, s: f64) -> Polyline {
        poly(&[(x, y), (x + s, y), (x + s, y + s), (x, y + s)], true)
    }

    fn layer(edit: impl FnOnce(&mut LayerSettings)) -> LayerSettings {
        let mut l = LayerSettings::for_color(Rgb(255, 0, 0));
        edit(&mut l);
        l
    }

    fn spec(edit: impl FnOnce(&mut LayerSettings)) -> StrokeSpec {
        resolve(&layer(edit)).unwrap().unwrap()
    }

    fn run(spec: &StrokeSpec, shape: &Polyline, depth: usize) -> (Vec<Move>, StrokeStats) {
        run_scaled(spec, shape, depth, 50.0, 1.0)
    }

    fn run_scaled(
        spec: &StrokeSpec,
        shape: &Polyline,
        depth: usize,
        power: f64,
        scale: f64,
    ) -> (Vec<Move>, StrokeStats) {
        let mut out = Vec::new();
        let mut stats = StrokeStats::default();
        let ctx = ShapeCtx {
            shape,
            pieces: std::slice::from_ref(shape),
            power,
            tab_power: 0.0,
            scale,
            depth,
        };
        push_shape(spec, &ctx, &mut stats, &mut out);
        (out, stats)
    }

    fn near(p: Point, x: f64, y: f64) -> bool {
        (p.x - x).abs() < 1e-9 && (p.y - y).abs() < 1e-9
    }

    fn cut_len(moves: &[Move], on: bool) -> f64 {
        let mut at = None;
        let mut sum = 0.0;
        for m in moves {
            if let Move::Cut { to, power_pct } = *m
                && let Some(a) = at
                && (power_pct > 0.0) == on
            {
                sum += Point::dist(a, to);
            }
            at = Some(m.to());
        }
        sum
    }

    #[test]
    fn nothing_active_resolves_to_none_even_with_bad_values() {
        let l = layer(|l| {
            l.lead_in.length = -3.0;
            l.cut_through.ms = f64::NAN;
        });
        assert_eq!(resolve(&l).unwrap(), None);
    }

    #[test]
    fn bad_values_name_the_layer() {
        for l in [
            layer(|l| {
                l.perforation.enabled = true;
                l.perforation.cut_mm = 0.0;
            }),
            layer(|l| {
                l.lead_in.enabled = true;
                l.lead_in.length = 0.0;
            }),
            layer(|l| {
                l.lead_out.enabled = true;
                l.lead_out.angle = 120.0;
            }),
            layer(|l| l.overcut = -1.0),
            layer(|l| l.start_delay_ms = f64::NAN),
            layer(|l| {
                l.cut_through.enabled = true;
                l.cut_through.power = 150.0;
            }),
            layer(|l| {
                l.dot_mode.enabled = true;
                l.dot_mode.spacing_mm = 0.0;
            }),
            layer(|l| {
                l.ramp.enabled = true;
                l.ramp.start_pct = -5.0;
            }),
        ] {
            let e = resolve(&l).unwrap_err();
            assert!(e.starts_with("capa #ff0000: "), "{e}");
        }
    }

    #[test]
    fn dot_mode_drops_incompatible_settings_with_a_warning() {
        let s = spec(|l| {
            l.dot_mode.enabled = true;
            l.perforation.enabled = true;
            l.ramp.enabled = true;
            l.lead_in.enabled = true;
            l.overcut = 1.0;
            l.start_delay_ms = 5.0;
        });
        assert_eq!(s.conflicts.len(), 4);
        assert!(s.perforation.is_none() && s.ramp.is_none() && s.lead_in.is_none());
        assert_eq!(s.overcut, 0.0);
        assert_eq!(s.start_delay_ms, 0.0);
        let w = StrokeStats::default().warnings("#ff0000", Some(&s));
        assert!(w.iter().all(|m| m.starts_with("capa #ff0000: ")));
    }

    #[test]
    fn delays_surround_the_stroke_with_the_laser_on() {
        let s = spec(|l| {
            l.start_delay_ms = 20.0;
            l.end_delay_ms = 30.0;
        });
        let line = poly(&[(0.0, 0.0), (10.0, 0.0)], false);
        let (m, _) = run(&s, &line, 0);
        assert_eq!(
            m,
            vec![
                Move::Travel { to: pt(0.0, 0.0) },
                dwell(pt(0.0, 0.0), 20.0, 50.0),
                Move::Cut {
                    to: pt(10.0, 0.0),
                    power_pct: 50.0
                },
                dwell(pt(10.0, 0.0), 30.0, 50.0),
            ]
        );
    }

    #[test]
    fn cut_through_only_on_closed_shapes_and_scales_with_the_object() {
        let s = spec(|l| {
            l.cut_through.enabled = true;
            l.cut_through.power = 80.0;
            l.cut_through.ms = 200.0;
        });
        let (m, _) = run(&s, &square(0.0, 0.0, 10.0), 0);
        assert_eq!(m[1], dwell(pt(0.0, 0.0), 200.0, 80.0));
        let (m, _) = run(&s, &poly(&[(0.0, 0.0), (5.0, 0.0)], false), 0);
        assert!(m.iter().all(|m| !matches!(m, Move::Dwell { .. })));
        // Con escala del 50 % el pasante también baja a la mitad.
        let (m, _) = run_scaled(&s, &square(0.0, 0.0, 10.0), 0, 25.0, 0.5);
        assert_eq!(m[1], dwell(pt(0.0, 0.0), 200.0, 40.0));
    }

    #[test]
    fn perforation_alternates_cut_and_gap_and_restarts_per_stroke() {
        let s = spec(|l| {
            l.perforation.enabled = true;
            l.perforation.cut_mm = 2.0;
            l.perforation.skip_mm = 1.0;
        });
        // 10 mm: 2 on, 1 off, 2 on, 1 off, 2 on, 1 off, 1 on.
        let (m, _) = run(&s, &poly(&[(0.0, 0.0), (10.0, 0.0)], false), 0);
        assert!((cut_len(&m, true) - 7.0).abs() < 1e-9);
        assert!((cut_len(&m, false) - 3.0).abs() < 1e-9);
        // El patrón sigue a lo largo de las esquinas: 20 mm de perímetro en un cuadrado de 5.
        let (m, _) = run(&s, &square(0.0, 0.0, 5.0), 0);
        let on = cut_len(&m, true);
        let off = cut_len(&m, false);
        assert!((on + off - 20.0).abs() < 1e-9);
        assert!((off - 6.0).abs() < 1e-9, "{off}");
        // Otro trazo reinicia el patrón: empieza encendido.
        let (m, _) = run(&s, &poly(&[(3.0, 3.0), (13.0, 3.0)], false), 0);
        assert!(matches!(m[1], Move::Cut { power_pct, .. } if power_pct > 0.0));
    }

    #[test]
    fn dot_mode_fires_every_spacing_without_moving() {
        let s = spec(|l| {
            l.dot_mode.enabled = true;
            l.dot_mode.dwell_ms = 5.0;
            l.dot_mode.spacing_mm = 2.0;
        });
        let (m, _) = run(&s, &poly(&[(0.0, 0.0), (6.0, 0.0)], false), 0);
        let dwells: Vec<_> = m
            .iter()
            .filter(|m| matches!(m, Move::Dwell { .. }))
            .collect();
        assert_eq!(dwells.len(), 4);
        // Entre disparos, solo Cut a potencia 0.
        assert!(
            m.iter()
                .all(|m| !matches!(m, Move::Cut { power_pct, .. } if *power_pct > 0.0))
        );
        // Contorno cerrado de 20 mm: 10 puntos, sin repetir el de cierre.
        let (m, _) = run(&s, &square(0.0, 0.0, 5.0), 0);
        assert_eq!(
            m.iter().filter(|m| matches!(m, Move::Dwell { .. })).count(),
            10
        );
    }

    #[test]
    fn straight_lead_in_goes_outside_pieces_and_inside_holes() {
        let s = spec(|l| {
            l.lead_in.enabled = true;
            l.lead_in.length = 2.0;
            l.lead_in.angle = 90.0;
        });
        let d = 2.0_f64.sqrt();
        // Cuadrado CCW de 10 que empieza en la esquina (0,0): la entrada llega por la
        // bisectriz desde fuera, abajo a la izquierda.
        let (m, st) = run(&s, &square(0.0, 0.0, 10.0), 0);
        let Move::Travel { to } = m[0] else { panic!() };
        assert!(near(to, -d, -d), "{to:?}");
        assert_eq!(m[1].to(), pt(0.0, 0.0));
        assert_eq!(st, StrokeStats::default());
        // Hueco de 20 (profundidad 1): entra por dentro, arriba a la derecha.
        let (m, _) = run(&s, &square(0.0, 0.0, 20.0), 1);
        let Move::Travel { to } = m[0] else { panic!() };
        assert!(near(to, d, d), "{to:?}");
        // El sentido horario no cambia el lado de la entrada.
        let cw = poly(&[(0.0, 0.0), (0.0, 10.0), (10.0, 10.0), (10.0, 0.0)], true);
        let (m, _) = run(&s, &cw, 0);
        let Move::Travel { to } = m[0] else { panic!() };
        assert!(near(to, -d, -d), "{to:?}");
    }

    #[test]
    fn lead_out_leaves_after_closing_on_the_waste_side() {
        let s = spec(|l| {
            l.lead_out.enabled = true;
            l.lead_out.length = 2.0;
            l.lead_out.angle = 90.0;
        });
        let (m, _) = run(&s, &square(0.0, 0.0, 10.0), 0);
        let n = m.len();
        let d = 2.0_f64.sqrt();
        assert_eq!(m[n - 2].to(), pt(0.0, 0.0));
        assert!(near(m[n - 1].to(), -d, -d), "{:?}", m[n - 1]);
        assert!(matches!(m[n - 1], Move::Cut { power_pct, .. } if power_pct == 50.0));
    }

    #[test]
    fn lead_that_would_cross_the_shape_is_shortened_then_dropped() {
        // Hueco estrecho (2 mm de ancho): una entrada de 5 mm saldría por el otro lado.
        let s = spec(|l| {
            l.lead_in.enabled = true;
            l.lead_in.length = 5.0;
            l.lead_in.angle = 90.0;
        });
        let slot = poly(&[(0.0, 0.0), (20.0, 0.0), (20.0, 2.0), (0.0, 2.0)], true);
        let (m, st) = run(&s, &slot, 1);
        assert_eq!(st.shortened, 1);
        let Move::Travel { to } = m[0] else { panic!() };
        let half = 2.5 / 2.0_f64.sqrt();
        assert!(near(to, half, half), "{to:?}");
        // En un hueco de 0,1 mm no cabe ni a un cuarto.
        let thin = poly(&[(0.0, 0.0), (20.0, 0.0), (20.0, 0.1), (0.0, 0.1)], true);
        let (m, st) = run(&s, &thin, 1);
        assert_eq!(st.dropped, 1);
        assert_eq!(m[0], Move::Travel { to: pt(0.0, 0.0) });
        let w = st.warnings("#ff0000", None);
        assert!(w[0].starts_with("capa #ff0000: ") && w[0].contains("omitieron"));
    }

    #[test]
    fn arc_lead_ends_at_the_start_on_the_waste_side() {
        let s = spec(|l| {
            l.lead_in.enabled = true;
            l.lead_in.kind = LeadKind::Arc;
            l.lead_in.length = 3.0;
            l.lead_in.angle = 90.0;
        });
        let sq = square(0.0, 0.0, 10.0);
        let (m, st) = run(&s, &sq, 0);
        assert_eq!(st, StrokeStats::default());
        let arc: Vec<Point> = m
            .iter()
            .take_while(|m| m.to() != pt(0.0, 0.0))
            .map(Move::to)
            .collect();
        assert!(arc.len() >= 18, "{}", arc.len());
        // Todo el arco queda fuera del cuadrado y mide 3 mm.
        assert!(
            arc.iter()
                .all(|p| locate_point(*p, &sq.points) == PointLocation::Outside)
        );
        let len: f64 = arc.windows(2).map(|w| w[0].dist(w[1])).sum::<f64>()
            + arc.last().unwrap().dist(pt(0.0, 0.0));
        assert!((len - 3.0).abs() < 0.02, "{len}");
        // Un hueco la lleva por dentro.
        let (m, _) = run(&s, &square(0.0, 0.0, 20.0), 1);
        let inside = m
            .iter()
            .take_while(|m| m.to() != pt(0.0, 0.0))
            .all(|m| locate_point(m.to(), &square(0.0, 0.0, 20.0).points) == PointLocation::Inside);
        assert!(inside);
    }

    #[test]
    fn overcut_retraces_the_start_of_the_contour() {
        let s = spec(|l| l.overcut = 3.0);
        let (m, _) = run(&s, &square(0.0, 0.0, 10.0), 0);
        let n = m.len();
        assert_eq!(m[n - 1].to(), pt(3.0, 0.0));
        assert_eq!(m[n - 2].to(), pt(0.0, 0.0));
        // Un overcut mayor que el perímetro se limita a una vuelta y avisa.
        let big = spec(|l| l.overcut = 500.0);
        let (m, st) = run(&big, &square(0.0, 0.0, 10.0), 0);
        assert_eq!(st.overcut_clamped, 1);
        assert_eq!(m.last().unwrap().to(), pt(0.0, 0.0));
        assert!((cut_len(&m, true) - 80.0).abs() < 1e-9);
    }

    #[test]
    fn leads_and_overcut_skip_open_paths_and_tabbed_shapes() {
        let s = spec(|l| {
            l.lead_in.enabled = true;
            l.overcut = 1.0;
        });
        let (m, st) = run(&s, &poly(&[(0.0, 0.0), (5.0, 0.0)], false), 0);
        assert_eq!(m.len(), 2);
        assert_eq!(st.open_skipped, 1);
        let sq = square(0.0, 0.0, 10.0);
        let halves = [
            poly(&[(0.0, 0.0), (10.0, 0.0), (10.0, 5.0)], false),
            poly(&[(10.0, 7.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0)], false),
        ];
        let mut st = StrokeStats::default();
        let ctx = ShapeCtx {
            shape: &sq,
            pieces: &halves,
            power: 50.0,
            tab_power: 0.0,
            scale: 1.0,
            depth: 0,
        };
        push_shape(&s, &ctx, &mut st, &mut Vec::new());
        assert_eq!(st.tabbed_skipped, 1);
    }

    #[test]
    fn tab_gaps_use_the_bridge_power_through_perforation() {
        let s = spec(|l| {
            l.perforation.enabled = true;
            l.perforation.cut_mm = 2.0;
            l.perforation.skip_mm = 2.0;
        });
        let line = poly(&[(0.0, 0.0), (10.0, 0.0)], false);
        let pieces = [
            poly(&[(0.0, 0.0), (4.0, 0.0)], false),
            poly(&[(6.0, 0.0), (10.0, 0.0)], false),
        ];
        let ctx = ShapeCtx {
            shape: &line,
            pieces: &pieces,
            power: 50.0,
            tab_power: 10.0,
            scale: 1.0,
            depth: 0,
        };
        let mut out = Vec::new();
        push_shape(&s, &ctx, &mut StrokeStats::default(), &mut out);
        let powers: Vec<f64> = out
            .iter()
            .filter_map(|m| match m {
                Move::Cut { power_pct, .. } => Some(*power_pct),
                _ => None,
            })
            .collect();
        // El corte va a 50, la pausa de la perforación a 0 y el hueco del puente a 10.
        assert!(
            powers.contains(&50.0) && powers.contains(&0.0),
            "{powers:?}"
        );
        assert!(
            powers.iter().any(|p| (*p - 10.0).abs() < 1e-9),
            "{powers:?}"
        );
        assert!(powers.iter().all(|p| *p <= 50.0));
    }

    #[test]
    fn ramp_rises_and_falls_linearly_and_respects_outer_only() {
        let s = spec(|l| {
            l.ramp.enabled = true;
            l.ramp.length_mm = 2.0;
            l.ramp.start_pct = 20.0;
        });
        let line = poly(&[(0.0, 0.0), (10.0, 0.0)], false);
        let (m, _) = run(&s, &line, 0);
        let powers: Vec<f64> = m
            .iter()
            .filter_map(|m| match m {
                Move::Cut { power_pct, .. } => Some(*power_pct),
                _ => None,
            })
            .collect();
        // Sube, se mantiene al 100 % de la capa (50) y baja simétrico.
        assert!(powers[0] > 10.0 && powers[0] < 20.0, "{powers:?}");
        assert!(powers.windows(2).take(7).all(|w| w[1] >= w[0]));
        assert!(powers.iter().any(|p| (*p - 50.0).abs() < 1e-9));
        for (a, b) in powers.iter().zip(powers.iter().rev()) {
            assert!((a - b).abs() < 1e-9, "{powers:?}");
        }
        assert_eq!(cut_len(&m, true), 10.0);
        // Con outer_only, un hueco y una abierta salen a potencia constante.
        let so = spec(|l| {
            l.ramp.enabled = true;
            l.ramp.outer_only = true;
        });
        let (m, _) = run(&so, &square(0.0, 0.0, 10.0), 1);
        assert_eq!(m.len(), 5);
        let (m, _) = run(&so, &line, 0);
        assert_eq!(m.len(), 2);
        let (m, _) = run(&so, &square(0.0, 0.0, 10.0), 0);
        assert!(m.len() > 5);
    }

    #[test]
    fn full_chain_orders_lead_overcut_ramp_and_pauses() {
        let s = spec(|l| {
            l.lead_in.enabled = true;
            l.lead_in.length = 2.0;
            l.lead_in.angle = 90.0;
            l.overcut = 1.0;
            l.ramp.enabled = true;
            l.ramp.length_mm = 1.0;
            l.start_delay_ms = 10.0;
            l.end_delay_ms = 20.0;
            l.cut_through.enabled = true;
            l.cut_through.power = 90.0;
            l.cut_through.ms = 50.0;
        });
        let (m, _) = run(&s, &square(0.0, 0.0, 10.0), 0);
        let d = 2.0_f64.sqrt();
        let begin = m[0].to();
        assert!(near(begin, -d, -d), "{begin:?}");
        assert!(matches!(m[0], Move::Travel { .. }));
        // La pausa de inicio usa la potencia (de rampa) del primer tramo, antes del pasante.
        let Move::Dwell { ms, power_pct, .. } = m[1] else {
            panic!()
        };
        assert_eq!(ms, 10.0);
        assert!(power_pct > 0.0 && power_pct < 50.0, "{power_pct}");
        assert_eq!(m[2], dwell(begin, 50.0, 90.0));
        let Move::Dwell { ms, to, .. } = *m.last().unwrap() else {
            panic!()
        };
        assert_eq!((ms, to), (20.0, pt(1.0, 0.0)));
    }

    #[test]
    fn scaled_power_clamps_and_counts() {
        let mut st = StrokeStats::default();
        assert_eq!(scaled_power(40.0, 0.5, &mut st), 20.0);
        assert_eq!(scaled_power(80.0, 1.5, &mut st), 100.0);
        assert_eq!(st.power_clamped, 1);
    }
}
