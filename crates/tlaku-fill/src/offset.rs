//! Relleno por contornos concéntricos (offset fill).
//!
//! # Geometría
//!
//! 1. Cada trayectoria cerrada se simplifica una sola vez (Douglas-Peucker con tolerancia
//!    `interval / 20`, máximo 10 µm; es decir, muy por debajo del ancho del haz).
//! 2. Todas las formas se unen en una región con la regla de relleno pedida (par-impar o no
//!    cero), igual que el barrido: un anillo conserva su hueco, un solape par-impar queda
//!    vacío. Cada forma resultante (exterior con sus agujeros) se trata por separado.
//! 3. Cada nivel es el offset hacia dentro del nivel anterior, con esquinas redondeadas
//!    (flecha máxima `interval / 20`, tope 10 µm). Entre niveles se quitan los vértices
//!    redundantes con una tolerancia diminuta (`interval / 100`, tope 2 µm); en la prueba de
//!    deriva (200 niveles sobre un círculo) el error acumulado es de unos 4 µm.
//! 4. Las formas son independientes y se calculan en paralelo (hasta 8 hilos) cuando hay
//!    suficientes; el resultado es el mismo que en un solo hilo.
//!
//! # Posición del primer contorno
//!
//! El primero va a `interval / 2` del borde, no sobre él. El haz tiene un ancho cercano al
//! intervalo, así que una línea centrada a `interval / 2` quema justo hasta el borde y el
//! relleno no se sale de la forma; es el mismo criterio que usa el barrido (`(k + 0,5) *
//! interval`). Los contornos siguientes se separan `interval` entre sí.
//!
//! # Formas delgadas
//!
//! Si una forma es más angosta que `interval`, el offset de `interval / 2` desaparece. En ese
//! caso se busca (bisección) el offset más profundo que todavía sobrevive: un contorno casi
//! degenerado que recorre la línea central de ida y vuelta. Lo mismo ocurre al final de
//! una forma gruesa: si tras el último contorno queda un núcleo sin cubrir (más ancho que
//! `0,2 * interval`), se añade esa línea central. Las ramas finas de una forma que se cierran
//! antes que el resto no reciben línea central propia; la última rama sí.
//!
//! # Orden
//!
//! Dentro de cada forma se recorre de afuera hacia adentro: el borde se graba primero, con el
//! material aún intacto alrededor que lo sostiene y disipa el calor, y el contorno que define
//! la silueta queda limpio en lugar de recibir el calor acumulado de los anillos interiores.
//! Además cada contorno nace a `interval` del anterior, así que el viaje entre ellos es
//! mínimo (el anillo siguiente empieza en su vértice más cercano al punto donde terminó el
//! anterior). Con ramificaciones (un anillo que se parte en dos) se sigue cada rama completa
//! antes de pasar a la siguiente, y las raíces y formas se encadenan por vecino más cercano.
//!
//! # Límites
//!
//! Topes deterministas (no dependen del reloj) evitan cuelgues con diseños patológicos:
//! [`MAX_RINGS`] contornos, [`MAX_POINTS`] vértices emitidos, [`MAX_WORK`] vértices procesados
//! por el motor de offset y [`MAX_LEVELS`] niveles por forma. Si se superan, `fill` falla con un
//! mensaje que sugiere subir el intervalo. Además: los contornos de entrada se simplifican
//! antes de nada (los de 50 000 puntos bajan a unos pocos miles), hay una salida inmediata
//! cuando el disco de radio `d` no cabe en la caja de la forma, se descartan las coordenadas
//! con módulo mayor que 1e7 mm, y el orden por vecino más cercano pasa a franjas cuando
//! hay demasiados candidatos.

use crate::FillRule;
use i_overlay::core::fill_rule::FillRule as EngineRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::overlay::FloatOverlay;
use i_overlay::mesh::float::outline::offset::OutlineOffset;
use i_overlay::mesh::float::style::{LineJoin, OutlineStyle};
use std::collections::HashMap;
use std::f64::consts::PI;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tlaku_core::Point;
use tlaku_core::toolpath::Move;

/// Máximo de contornos que produce una llamada.
pub const MAX_RINGS: usize = 100_000;
/// Máximo de puntos acumulados (suma de los vértices de todos los contornos y de las
/// muestras auxiliares), que acota la memoria y el tiempo.
pub const MAX_POINTS: usize = 6_000_000;
/// Máximo de vértices procesados por el motor de offset (suma sobre todos los niveles).
pub const MAX_WORK: usize = 60_000_000;
/// Máximo de niveles concéntricos de una misma forma.
pub const MAX_LEVELS: usize = 20_000;

/// Coordenadas más allá de este valor (mm) se descartan como inválidas.
const MAX_COORD: f64 = 1e7;
/// Hilos máximos y formas mínimas por hilo para repartir el trabajo.
const MAX_THREADS: usize = 8;
const MIN_SHAPES_PER_THREAD: usize = 16;
/// Hasta este número de candidatos el orden es vecino más cercano exacto (O(m²)).
const NN_EXACT_MAX: usize = 1500;
/// Producto contornos nuevos x vértices previos hasta el que el enlace usa fuerza bruta.
const BRUTE_LINK_MAX: usize = 20_000;
/// Pasos de bisección para la línea central de un núcleo delgado.
const CORE_STEPS: usize = 4;
/// Un núcleo sin cubrir solo recibe línea central si su semiancho supera esta fracción del
/// intervalo (evita una pasada extra por un hueco despreciable).
const CORE_MIN: f64 = 0.6;

type Contour = Vec<[f64; 2]>;
type Shape = Vec<Contour>;
type Shapes = Vec<Shape>;

/// Cuenta de trabajo compartida entre todas las formas (y los hilos) de una llamada.
#[derive(Default)]
struct Budget {
    /// Vértices de los contornos emitidos.
    points: AtomicUsize,
    rings: AtomicUsize,
    /// Vértices procesados por el motor de offset y por la búsqueda de enlaces.
    work: AtomicUsize,
}

fn add(counter: &AtomicUsize, n: usize) -> usize {
    counter.fetch_add(n, Ordering::Relaxed).saturating_add(n)
}

impl Budget {
    fn emit(&self, points: usize, rings: usize) -> Result<(), String> {
        let (p, r) = (add(&self.points, points), add(&self.rings, rings));
        if p > MAX_POINTS || r > MAX_RINGS {
            return Err(format!(
                "el relleno por contornos es demasiado complejo ({r} contornos, {p} puntos); \
                 aumenta el intervalo o simplifica el diseño"
            ));
        }
        Ok(())
    }

    fn work(&self, points: usize) -> Result<(), String> {
        let w = add(&self.work, points);
        if w > MAX_WORK {
            return Err(format!(
                "el relleno por contornos es demasiado complejo ({w} puntos de trabajo); \
                 aumenta el intervalo o simplifica el diseño"
            ));
        }
        Ok(())
    }
}

/// Contornos ya calculados por un hilo, con el índice de su forma.
type Done = Vec<(usize, Vec<Ring>)>;

/// Un contorno cerrado generado, con su enlace al nivel anterior.
struct Ring {
    pts: Vec<Point>,
    parent: Option<usize>,
}

/// Parámetros geométricos de una llamada.
struct Geometry {
    interval: f64,
    /// Tolerancia de arcos y de la simplificación de entrada.
    tol: f64,
    /// Tolerancia de la simplificación entre niveles.
    chain_tol: f64,
}

impl Geometry {
    fn style(&self, distance: f64) -> OutlineStyle<f64> {
        // Flecha del arco: r (1 - cos(paso / 2)) <= tol.
        let ratio = (self.tol / distance.abs()).min(1.0);
        let step = (2.0 * (1.0 - ratio).acos()).clamp(1e-3, PI);
        OutlineStyle::new(-distance.abs()).line_join(LineJoin::Round(step))
    }

    /// Offset hacia dentro `distance` mm (positivo). Vacío si la forma desaparece.
    fn erode(&self, shapes: &Shapes, distance: f64, budget: &Budget) -> Result<Shapes, String> {
        // Un disco de radio `distance` no cabe en la caja: desaparece todo, sin llamar al motor.
        let fits = shapes.iter().any(|s| {
            s.first().is_some_and(|outer| {
                let (w, h) = extent(outer);
                w.min(h) > 2.0 * distance
            })
        });
        if !fits {
            return Ok(Vec::new());
        }
        let n: usize = shapes.iter().flatten().map(Vec::len).sum();
        budget.work(n)?;
        Ok(shapes.outline(&self.style(distance)))
    }
}

fn extent(c: &[[f64; 2]]) -> (f64, f64) {
    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for p in c {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    (hi[0] - lo[0], hi[1] - lo[1])
}

/// Línea central de un núcleo delgado: el offset más profundo en `(lo, hi)` que sobrevive.
/// `None` si ni siquiera `lo` sobrevive (no queda nada por cubrir).
fn thin_core(
    geo: &Geometry,
    shapes: &Shapes,
    mut lo: f64,
    mut hi: f64,
    budget: &Budget,
) -> Result<Option<Shapes>, String> {
    let mut best = if lo <= 0.0 {
        shapes.clone()
    } else {
        let r = geo.erode(shapes, lo, budget)?;
        if r.is_empty() {
            return Ok(None);
        }
        r
    };
    for _ in 0..CORE_STEPS {
        let mid = 0.5 * (lo + hi);
        let r = geo.erode(shapes, mid, budget)?;
        if r.is_empty() {
            hi = mid;
        } else {
            best = r;
            lo = mid;
        }
    }
    Ok(Some(best))
}

pub(crate) fn offset_fill(
    polys: &[&[Point]],
    interval: f64,
    rule: FillRule,
    power_pct: f64,
    moves: &mut Vec<Move>,
) -> Result<(), String> {
    let tol = (interval * 0.05).clamp(1e-6, 0.01);
    let geo = Geometry {
        interval,
        tol,
        chain_tol: (interval * 0.01).clamp(1e-7, 0.002),
    };
    let budget = Budget::default();

    let subject: Vec<Contour> = polys
        .iter()
        .filter(|p| {
            p.iter()
                .all(|q| q.x.abs() <= MAX_COORD && q.y.abs() <= MAX_COORD)
        })
        .filter_map(|p| {
            let s = simplify_ring(p, tol);
            (s.len() >= 3).then(|| s.iter().map(|q| [q.x, q.y]).collect())
        })
        .collect();
    if subject.is_empty() {
        return Ok(());
    }
    let engine_rule = match rule {
        FillRule::EvenOdd => EngineRule::EvenOdd,
        FillRule::NonZero => EngineRule::NonZero,
    };
    let region: Shapes =
        FloatOverlay::<[f64; 2]>::with_subj(&subject).overlay(OverlayRule::Subject, engine_rule);
    let shapes: Vec<Shape> = region
        .into_iter()
        .filter(|s| s.first().is_some_and(|c| c.len() >= 3))
        .collect();

    // Las formas se encadenan por vecino más cercano desde su primer vértice.
    let reps: Vec<Point> = shapes
        .iter()
        .map(|s| Point::new(s[0][0][0], s[0][0][1]))
        .collect();
    let mut traced = trace_all(&geo, &shapes, &budget)?;
    let mut pen = Point::new(0.0, 0.0);
    for idx in nn_order(&reps, pen) {
        emit_shape(&traced[idx], power_pct, &mut pen, moves);
        traced[idx] = Vec::new();
    }
    Ok(())
}

/// Calcula los contornos de todas las formas. Son independientes entre sí, así que con
/// suficientes formas se reparten entre hilos; el resultado no depende del reparto.
fn trace_all(geo: &Geometry, shapes: &[Shape], budget: &Budget) -> Result<Vec<Vec<Ring>>, String> {
    let threads = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(MAX_THREADS)
        .min(shapes.len() / MIN_SHAPES_PER_THREAD);
    if threads <= 1 {
        return shapes.iter().map(|s| trace_shape(geo, s, budget)).collect();
    }
    let next = AtomicUsize::new(0);
    let failed = AtomicBool::new(false);
    let mut slots: Vec<Vec<Ring>> = shapes.iter().map(|_| Vec::new()).collect();
    let parts: Vec<Result<Done, String>> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut done = Vec::new();
                    while !failed.load(Ordering::Relaxed) {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(shape) = shapes.get(i) else { break };
                        match trace_shape(geo, shape, budget) {
                            Ok(rings) => done.push((i, rings)),
                            Err(e) => {
                                failed.store(true, Ordering::Relaxed);
                                return Err(e);
                            }
                        }
                    }
                    Ok(done)
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|w| {
                w.join()
                    .unwrap_or_else(|_| Err("fallo interno en el relleno por contornos".into()))
            })
            .collect()
    });
    for part in parts {
        for (i, rings) in part? {
            slots[i] = rings;
        }
    }
    Ok(slots)
}

/// Todos los contornos de una forma, nivel por nivel, con su enlace al nivel previo.
///
/// Cada nivel es el offset del anterior. Partir siempre de la forma original sería exacto pero
/// carísimo con contornos ruidosos: el arco de cada esquina cóncava mide `d` y a distancias
/// grandes se solapan miles de arcos. Encadenar tiene el problema contrario: cada esquina deja
/// dos vértices casi colineales y el contorno se duplica en cada vuelta. Por eso entre niveles se
/// quitan los vértices redundantes con una tolerancia `chain_tol` diminuta (una fracción
/// del intervalo, muy por debajo de la tolerancia de entrada), que frena el crecimiento sin
/// acumular una deriva apreciable.
fn trace_shape(geo: &Geometry, shape: &Shape, budget: &Budget) -> Result<Vec<Ring>, String> {
    let mut rings: Vec<Ring> = Vec::new();
    let mut current: Shapes = vec![shape.clone()];
    let mut prev: Range<usize> = 0..0;
    // Primer nivel: a medio intervalo del borde; la línea central se busca desde el borde mismo.
    let (mut step, mut core_lo) = (geo.interval / 2.0, 0.0);

    for _ in 0..MAX_LEVELS {
        let mut eroded = geo.erode(&current, step, budget)?;
        simplify_shapes(&mut eroded, geo.chain_tol);
        let (next, last) = if eroded.is_empty() {
            match thin_core(geo, &current, core_lo, step, budget)? {
                Some(core) => (core, true),
                None => return Ok(rings),
            }
        } else {
            (eroded, false)
        };

        let start = rings.len();
        for contour in next.iter().flatten() {
            let pts: Vec<Point> = contour.iter().map(|p| Point::new(p[0], p[1])).collect();
            if pts.len() >= 3 {
                rings.push(Ring { pts, parent: None });
            }
        }
        let cur = start..rings.len();
        budget.emit(
            rings[cur.clone()].iter().map(|r| r.pts.len()).sum(),
            cur.len(),
        )?;
        link_parents(&mut rings, prev.clone(), cur.clone(), geo.interval, budget)?;

        if last || cur.is_empty() {
            return Ok(rings);
        }
        prev = cur;
        current = next;
        step = geo.interval;
        core_lo = CORE_MIN * geo.interval;
    }
    Err(format!(
        "demasiados niveles concéntricos ({MAX_LEVELS}); aumenta el intervalo"
    ))
}

/// Simplifica cada contorno; descarta los que quedan degenerados y las formas sin exterior.
fn simplify_shapes(shapes: &mut Shapes, tol: f64) {
    for shape in shapes.iter_mut() {
        let outer_ok = shape.first().is_some_and(|c| c.len() >= 3);
        if !outer_ok {
            shape.clear();
            continue;
        }
        for (i, contour) in shape.iter_mut().enumerate() {
            let pts: Vec<Point> = contour.iter().map(|p| Point::new(p[0], p[1])).collect();
            let s = simplify_ring(&pts, tol);
            if s.len() >= 3 || i == 0 {
                *contour = s.iter().map(|p| [p.x, p.y]).collect();
            } else {
                contour.clear();
            }
        }
        shape.retain(|c| !c.is_empty());
        if shape.first().is_some_and(|c| c.len() < 3) {
            shape.clear();
        }
    }
    shapes.retain(|s| !s.is_empty());
}

/// Asigna a cada contorno nuevo el contorno del nivel anterior más cercano. Es solo para
/// ordenar el recorrido, así que basta una búsqueda aproximada sobre muestras de los
/// contornos previos cada `interval`.
fn link_parents(
    rings: &mut [Ring],
    prev: Range<usize>,
    cur: Range<usize>,
    interval: f64,
    budget: &Budget,
) -> Result<(), String> {
    if prev.is_empty() || cur.is_empty() {
        return Ok(());
    }
    if prev.len() == 1 {
        for i in cur {
            rings[i].parent = Some(prev.start);
        }
        return Ok(());
    }
    // Pocos contornos y vértices: la distancia exacta a los segmentos sale más barata que un índice.
    let prev_pts: usize = rings[prev.clone()].iter().map(|r| r.pts.len()).sum();
    if cur.len() * prev_pts <= BRUTE_LINK_MAX {
        budget.work(cur.len() * prev_pts)?;
        for i in cur {
            let q = rings[i].pts[0];
            let mut best: Option<(f64, usize)> = None;
            for j in prev.clone() {
                let pts = &rings[j].pts;
                for k in 0..pts.len() {
                    let d = seg_dist(q, pts[k], pts[(k + 1) % pts.len()]);
                    if best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, j));
                    }
                }
            }
            rings[i].parent = best.map(|(_, j)| j);
        }
        return Ok(());
    }
    let cell = 2.0 * interval;
    let key = |p: Point| ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64);
    let mut grid: HashMap<(i64, i64), Vec<(Point, usize)>> = HashMap::new();
    for i in prev {
        let pts = &rings[i].pts;
        for k in 0..pts.len() {
            let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
            let n = ((a.dist(b) / interval).ceil() as usize).max(1);
            budget.work(n)?;
            for j in 0..n {
                let t = j as f64 / n as f64;
                let p = Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
                grid.entry(key(p)).or_default().push((p, i));
            }
        }
    }
    for i in cur {
        let q = rings[i].pts[0];
        let (cx, cy) = key(q);
        let mut best: Option<(f64, usize)> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for &(p, owner) in grid.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                    let d = p.dist(q);
                    if best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, owner));
                    }
                }
            }
        }
        rings[i].parent = best.map(|(_, o)| o);
    }
    Ok(())
}

/// Emite una forma completa: cada raíz con todo su subárbol (de afuera hacia adentro).
fn emit_shape(rings: &[Ring], power: f64, pen: &mut Point, moves: &mut Vec<Move>) {
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); rings.len()];
    let mut roots: Vec<usize> = Vec::new();
    for (i, r) in rings.iter().enumerate() {
        match r.parent {
            Some(p) => children[p].push(i),
            None => roots.push(i),
        }
    }

    let reps = |ids: &[usize]| -> Vec<Point> { ids.iter().map(|&i| rings[i].pts[0]).collect() };
    let mut stack: Vec<usize> = nn_order(&reps(&roots), *pen)
        .into_iter()
        .rev()
        .map(|k| roots[k])
        .collect();

    while let Some(i) = stack.pop() {
        let start = emit_ring(&rings[i].pts, *pen, power, moves);
        *pen = start;
        let kids = &children[i];
        if !kids.is_empty() {
            let order = nn_order(&reps(kids), start);
            stack.extend(order.into_iter().rev().map(|k| kids[k]));
        }
    }
}

/// Recorre un contorno cerrado empezando por el vértice más cercano a `from`. Devuelve ese
/// vértice, donde queda el cabezal.
fn emit_ring(pts: &[Point], from: Point, power: f64, moves: &mut Vec<Move>) -> Point {
    let n = pts.len();
    let s = (0..n)
        .min_by(|&a, &b| from.dist(pts[a]).total_cmp(&from.dist(pts[b])))
        .unwrap_or(0);
    moves.push(Move::Travel { to: pts[s] });
    for k in 1..=n {
        moves.push(Move::Cut {
            to: pts[(s + k) % n],
            power_pct: power,
        });
    }
    pts[s]
}

/// Orden de visita por vecino más cercano partiendo de `from`. Con muchos candidatos pasa a
/// franjas horizontales en serpentina (O(m log m)) para no ser cuadrático.
fn nn_order(reps: &[Point], from: Point) -> Vec<usize> {
    let m = reps.len();
    if m <= 1 {
        return (0..m).collect();
    }
    if m <= NN_EXACT_MAX {
        let mut left: Vec<usize> = (0..m).collect();
        let mut out = Vec::with_capacity(m);
        let mut at = from;
        while !left.is_empty() {
            let (pos, _) = left
                .iter()
                .enumerate()
                .map(|(pos, &i)| (pos, at.dist(reps[i])))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .expect("left no está vacío");
            let i = left.swap_remove(pos);
            at = reps[i];
            out.push(i);
        }
        return out;
    }
    let (mut lo, mut hi) = (reps[0], reps[0]);
    for p in reps {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let (w, h) = (hi.x - lo.x, hi.y - lo.y);
    let band = ((w * h / m as f64).sqrt() * 1.5)
        .max((w.max(h)) / m as f64)
        .max(1e-9);
    let mut order: Vec<usize> = (0..m).collect();
    let band_of = |i: usize| ((reps[i].y - lo.y) / band).floor() as i64;
    order.sort_by(|&a, &b| {
        let (ba, bb) = (band_of(a), band_of(b));
        ba.cmp(&bb).then_with(|| {
            let o = reps[a].x.total_cmp(&reps[b].x);
            if ba % 2 == 0 { o } else { o.reverse() }
        })
    });
    order
}

/// Simplifica un anillo cerrado con Douglas-Peucker iterativo (sin recursión, con un tope de
/// trabajo: si se agota, los tramos pendientes se conservan intactos).
pub(crate) fn simplify_ring(pts: &[Point], tol: f64) -> Vec<Point> {
    let mut v: Vec<Point> = Vec::with_capacity(pts.len());
    for &p in pts {
        if v.last().is_none_or(|&q| q.dist(p) > 1e-9) {
            v.push(p);
        }
    }
    while v.len() > 1 && v[0].dist(v[v.len() - 1]) <= 1e-9 {
        v.pop();
    }
    let n = v.len();
    if n < 4 {
        return v;
    }
    // Se parte el anillo por su primer punto y el más lejano a él.
    let far = (1..n)
        .max_by(|&a, &b| v[0].dist(v[a]).total_cmp(&v[0].dist(v[b])))
        .unwrap_or(1);
    let at = |i: usize| v[i % n];
    let mut keep = vec![false; n + 1];
    keep[0] = true;
    keep[far] = true;
    let mut budget = 64 * n + 1000;
    let mut stack = vec![(0usize, far), (far, n)];
    while let Some((a, b)) = stack.pop() {
        if b - a < 2 {
            continue;
        }
        if budget < b - a {
            keep[a..=b].fill(true);
            continue;
        }
        budget -= b - a;
        let (pa, pb) = (at(a), at(b));
        let mut worst = (0.0, a);
        for i in a + 1..b {
            let d = seg_dist(at(i), pa, pb);
            if d > worst.0 {
                worst = (d, i);
            }
        }
        if worst.0 > tol {
            keep[worst.1] = true;
            stack.push((a, worst.1));
            stack.push((worst.1, b));
        }
    }
    (0..n).filter(|&i| keep[i]).map(|i| v[i]).collect()
}

fn seg_dist(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 == 0.0 {
        return p.dist(a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    p.dist(Point::new(a.x + t * dx, a.y + t * dy))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simplify_removes_collinear_and_keeps_corners() {
        let pts: Vec<Point> = (0..=10)
            .map(|i| Point::new(i as f64, 0.0))
            .chain((1..=10).map(|i| Point::new(10.0, i as f64)))
            .chain((1..=10).map(|i| Point::new(10.0 - i as f64, 10.0)))
            .chain((1..10).map(|i| Point::new(0.0, 10.0 - i as f64)))
            .collect();
        assert_eq!(simplify_ring(&pts, 0.001).len(), 4);
    }

    #[test]
    fn simplify_handles_degenerate_input() {
        assert!(simplify_ring(&[Point::new(1.0, 1.0); 5], 0.01).len() < 3);
        let line: Vec<Point> = (0..20).map(|i| Point::new(i as f64, 0.0)).collect();
        assert!(simplify_ring(&line, 0.01).len() <= 3);
    }

    #[test]
    fn nn_order_visits_everything_once() {
        for m in [0, 1, 5, 40, 3000] {
            let reps: Vec<Point> = (0..m)
                .map(|i| Point::new((i * 7 % 13) as f64, (i * 3 % 17) as f64))
                .collect();
            let mut o = nn_order(&reps, Point::new(0.0, 0.0));
            o.sort_unstable();
            assert_eq!(o, (0..m).collect::<Vec<_>>());
        }
    }
}
