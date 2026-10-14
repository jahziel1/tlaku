//! Relleno por regiones (flood): cada región conexa se barre por separado.
//!
//! El barrido global ([`crate::FillPattern::Scan`]) recorre cada línea de lado a lado y cruza con
//! potencia 0 los huecos entre islas, y salta de una isla a otra en cada línea. Con texto o
//! con muchas piezas pequeñas eso son miles de viajes largos. Aquí:
//!
//! 1. Se calculan los mismos tramos que el barrido (misma cobertura exacta).
//! 2. Dos tramos de líneas contiguas se enlazan si se solapan en x. Un tramo con más de un
//!    vecino (bifurcación) o un vecino con más de un tramo encima (unión) corta la cadena.
//!    Así cada cadena es una tira "monótona": un solo tramo por línea, que se recorre en
//!    serpentina continua, sin saltos ni cruces por huecos (descomposición boustrófedon).
//! 3. Las cadenas se ordenan con el vecino más cercano sobre sus extremos (hay cuatro puntos
//!    de entrada por cadena: arriba o abajo, a la izquierda o a la derecha). Una cadena se
//!    puede recorrer hacia arriba o hacia abajo, de modo que un anillo o una "U" se
//!    resuelven sin regresar a lo largo.

use crate::FillRule;
use crate::scan::{Edge, Frame, scan};
use tlaku_core::Point;
use tlaku_core::toolpath::Move;
use tlaku_i18n::tr;

/// Solape mínimo en x (mm) para considerar enlazados dos tramos de líneas contiguas.
const EPS: f64 = 1e-9;

/// Tope de tramos en memoria. Evita agotar RAM con intervalos muy finos sobre áreas enormes.
pub const MAX_SPANS: usize = 20_000_000;

const NONE: u32 = u32::MAX;

/// Parámetros que usa el flood (el resto de `FillSettings` no influye).
#[derive(Debug, Clone, Copy)]
pub(crate) struct FloodParams {
    pub interval: f64,
    pub rule: FillRule,
    pub overscan: f64,
    pub power: f64,
    pub bidirectional: bool,
}

#[derive(Debug, Clone, Copy)]
struct Span {
    line: i64,
    y: f64,
    x0: f64,
    x1: f64,
}

/// Una tira de tramos enlazados 1 a 1, de la línea más baja (`head`) a la más alta (`tail`).
#[derive(Debug, Clone, Copy)]
struct Chain {
    head: u32,
    tail: u32,
}

/// Rellena los bordes `edges` (ya en el marco de barrido) y agrega los movimientos a `moves`.
pub(crate) fn flood(
    edges: Vec<Edge>,
    frame: Frame,
    p: FloodParams,
    moves: &mut Vec<Move>,
) -> Result<(), String> {
    let spans = collect_spans(edges, p)?;
    if spans.is_empty() {
        return Ok(());
    }
    let (next, prev) = link(&spans);
    let chains = chains(&next, &prev);
    tour(&spans, &next, &prev, &chains, frame, p, moves);
    Ok(())
}

fn collect_spans(edges: Vec<Edge>, p: FloodParams) -> Result<Vec<Span>, String> {
    let mut spans: Vec<Span> = Vec::new();
    let mut overflow = false;
    scan(edges, p.interval, p.rule, |y, line| {
        let k = (y / p.interval - 0.5).round() as i64;
        for &(x0, x1) in line {
            if spans.len() >= MAX_SPANS {
                overflow = true;
                return;
            }
            spans.push(Span { line: k, y, x0, x1 });
        }
    })?;
    if overflow {
        return Err(tr!("fill.flood.too_many_spans", max = MAX_SPANS));
    }
    Ok(spans)
}

/// Enlaces 1 a 1 entre tramos de líneas contiguas: `next[i]` es el único tramo de la línea
/// siguiente que se solapa con `i`, siempre que `i` sea también el único que lo toca desde
/// abajo. `NONE` si no hay enlace.
fn link(spans: &[Span]) -> (Vec<u32>, Vec<u32>) {
    let n = spans.len();
    let mut down_cnt = vec![0u32; n];
    let mut up_cnt = vec![0u32; n];
    let mut down_any = vec![NONE; n];
    let mut up_any = vec![NONE; n];

    // Rangos [inicio, fin) de cada línea (los tramos vienen ordenados por línea y por x).
    let mut start = 0;
    let mut prev_range: Option<(usize, usize)> = None;
    while start < n {
        let mut end = start + 1;
        while end < n && spans[end].line == spans[start].line {
            end += 1;
        }
        if let Some((ps, pe)) = prev_range
            && spans[ps].line + 1 == spans[start].line
        {
            let (mut i, mut j) = (ps, start);
            while i < pe && j < end {
                let (a, b) = (spans[i], spans[j]);
                if a.x1.min(b.x1) - a.x0.max(b.x0) > EPS {
                    down_cnt[i] += 1;
                    up_cnt[j] += 1;
                    down_any[i] = j as u32;
                    up_any[j] = i as u32;
                }
                if a.x1 < b.x1 {
                    i += 1;
                } else {
                    j += 1;
                }
            }
        }
        prev_range = Some((start, end));
        start = end;
    }

    let mut next = vec![NONE; n];
    let mut prev = vec![NONE; n];
    for i in 0..n {
        if down_cnt[i] == 1 {
            let j = down_any[i] as usize;
            if up_cnt[j] == 1 && up_any[j] as usize == i {
                next[i] = j as u32;
                prev[j] = i as u32;
            }
        }
    }
    (next, prev)
}

fn chains(next: &[u32], prev: &[u32]) -> Vec<Chain> {
    let mut out = Vec::new();
    for (head, &before) in prev.iter().enumerate() {
        if before != NONE {
            continue;
        }
        let mut tail = head;
        while next[tail] != NONE {
            tail = next[tail] as usize;
        }
        out.push(Chain {
            head: head as u32,
            tail: tail as u32,
        });
    }
    out
}

/// Índice espacial de los puntos de entrada de las cadenas, para el vecino más cercano.
struct Grid {
    min_x: f64,
    min_y: f64,
    cell: f64,
    cols: usize,
    rows: usize,
    cells: Vec<Vec<u32>>,
}

impl Grid {
    fn new(points: &[Point]) -> Self {
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for p in points {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
        let (w, h) = ((max_x - min_x).max(1e-6), (max_y - min_y).max(1e-6));
        // Unos dos puntos por celda, con un tope de celdas por eje.
        let target = (points.len() as f64 / 2.0).max(1.0);
        let cell = ((w * h) / target).sqrt().max(w.max(h) / 2048.0);
        let cols = ((w / cell).floor() as usize + 1).max(1);
        let rows = ((h / cell).floor() as usize + 1).max(1);
        let mut grid = Self {
            min_x,
            min_y,
            cell,
            cols,
            rows,
            cells: vec![Vec::new(); cols * rows],
        };
        for (id, p) in points.iter().enumerate() {
            let (cx, cy) = grid.locate(*p);
            grid.cells[cy * cols + cx].push(id as u32);
        }
        grid
    }

    fn locate(&self, p: Point) -> (usize, usize) {
        let cx = ((p.x - self.min_x) / self.cell).floor();
        let cy = ((p.y - self.min_y) / self.cell).floor();
        (
            cx.clamp(0.0, (self.cols - 1) as f64) as usize,
            cy.clamp(0.0, (self.rows - 1) as f64) as usize,
        )
    }

    fn remove(&mut self, id: u32, p: Point) {
        let (cx, cy) = self.locate(p);
        let cell = &mut self.cells[cy * self.cols + cx];
        if let Some(pos) = cell.iter().position(|&q| q == id) {
            cell.swap_remove(pos);
        }
    }

    /// Punto más cercano a `from` entre los que quedan, o `None` si no queda ninguno.
    fn nearest(&self, from: Point, points: &[Point]) -> Option<u32> {
        let (cx, cy) = self.locate(from);
        let mut best: Option<(f64, u32)> = None;
        let max_r = self.cols.max(self.rows);
        for r in 0..=max_r {
            let (x0, x1) = (cx as i64 - r as i64, cx as i64 + r as i64);
            let (y0, y1) = (cy as i64 - r as i64, cy as i64 + r as i64);
            let mut visit = |gx: i64, gy: i64| {
                for &id in &self.cells[gy as usize * self.cols + gx as usize] {
                    let d = points[id as usize].dist(from);
                    if best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, id));
                    }
                }
            };
            for gy in y0.max(0)..=y1.min(self.rows as i64 - 1) {
                if gy == y0 || gy == y1 {
                    for gx in x0.max(0)..=x1.min(self.cols as i64 - 1) {
                        visit(gx, gy);
                    }
                } else {
                    // En las filas intermedias del anillo solo cuentan sus dos extremos.
                    for gx in [x0, x1] {
                        if gx >= 0 && gx < self.cols as i64 {
                            visit(gx, gy);
                        }
                    }
                }
            }
            if let Some((d, _)) = best
                && d <= r as f64 * self.cell
            {
                break;
            }
        }
        best.map(|(_, id)| id)
    }
}

/// Punto de entrada `opt` de la cadena `c`: bit 0 = extremo derecho, bit 1 = desde `tail`.
fn entry(spans: &[Span], c: Chain, opt: u32) -> Point {
    let s = spans[if opt & 2 == 0 { c.head } else { c.tail } as usize];
    Point::new(if opt & 1 == 0 { s.x0 } else { s.x1 }, s.y)
}

fn tour(
    spans: &[Span],
    next: &[u32],
    prev: &[u32],
    chains: &[Chain],
    frame: Frame,
    p: FloodParams,
    moves: &mut Vec<Move>,
) {
    // Sin barrido bidireccional todos los tramos van de izquierda a derecha: solo hay
    // entradas por la izquierda.
    let opts: &[u32] = if p.bidirectional {
        &[0, 1, 2, 3]
    } else {
        &[0, 2]
    };
    let mut points = Vec::with_capacity(chains.len() * 4);
    for c in chains {
        for opt in 0..4 {
            points.push(entry(spans, *c, opt));
        }
    }
    let mut grid = Grid::new(&points);
    // Los puntos no permitidos se retiran para que nunca se elijan.
    for (ci, c) in chains.iter().enumerate() {
        for opt in 0..4u32 {
            if !opts.contains(&opt) {
                let id = (ci * 4) as u32 + opt;
                grid.remove(id, entry(spans, *c, opt));
            }
        }
    }

    let mut cur = Point::new(
        spans.iter().fold(f64::MAX, |m, s| m.min(s.x0)),
        spans.iter().fold(f64::MAX, |m, s| m.min(s.y)),
    );
    let mut remaining = chains.len();
    while remaining > 0 {
        let Some(id) = grid.nearest(cur, &points) else {
            break;
        };
        let (ci, opt) = ((id / 4) as usize, id % 4);
        let c = chains[ci];
        for o in 0..4u32 {
            if opts.contains(&o) {
                let pid = (ci * 4) as u32 + o;
                grid.remove(pid, points[pid as usize]);
            }
        }
        remaining -= 1;

        let forward = opt & 2 == 0;
        let mut at = if forward { c.head } else { c.tail };
        let mut left_to_right = opt & 1 == 0;
        loop {
            let s = spans[at as usize];
            let (from, to) = if left_to_right {
                (s.x0, s.x1)
            } else {
                (s.x1, s.x0)
            };
            emit_span(moves, frame, s.y, from, to, p);
            cur = Point::new(to, s.y);
            let nx = if forward {
                next[at as usize]
            } else {
                prev[at as usize]
            };
            if nx == NONE {
                break;
            }
            at = nx;
            let n = spans[at as usize];
            // Entra por el extremo más cercano al punto donde terminó el tramo anterior.
            left_to_right = !p.bidirectional || (cur.x - n.x0).abs() < (cur.x - n.x1).abs();
        }
    }
}

/// Un tramo: viaje al inicio (con overscan), potencia 0 hasta el borde, quema y overscan final.
fn emit_span(moves: &mut Vec<Move>, frame: Frame, y: f64, from: f64, to: f64, p: FloodParams) {
    let sign = if to >= from { 1.0 } else { -1.0 };
    let at = |x: f64| frame.to_world(x, y);
    moves.push(Move::Travel {
        to: at(from - sign * p.overscan),
    });
    if p.overscan > 0.0 {
        moves.push(Move::Cut {
            to: at(from),
            power_pct: 0.0,
        });
    }
    moves.push(Move::Cut {
        to: at(to),
        power_pct: p.power,
    });
    if p.overscan > 0.0 {
        moves.push(Move::Cut {
            to: at(to + sign * p.overscan),
            power_pct: 0.0,
        });
    }
}
