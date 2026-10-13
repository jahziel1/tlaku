//! Solapes parciales: figuras que no se contienen del todo pero comparten material.
//!
//! La contención estricta ([`crate::contain`]) solo ve "A cabe entera dentro de B". Cuando dos
//! figuras se cruzan, la pieza que más se hunde en la otra sigue colgando del material de ésta,
//! así que conviene cortarla antes. La regla (ver la documentación del crate):
//!
//! - Se mide qué parte de una figura `i` cae dentro de otra figura cerrada `j` con una muestra
//!   fija de puntos: una cuadrícula de 8 x 8 sobre el área de `i` (o 16 puntos repartidos por la
//!   longitud si es una trayectoria abierta o sin área).
//! - `i` va antes que `j` si **más de la mitad** de su muestra cae dentro de `j`, o si más de la
//!   mitad cae dentro de la unión de varias figuras mayores y `j` aporta al menos una cuarta
//!   parte (una figura que cruza la costura entre dos piezas).
//! - Solo cuentan las `j` de mayor rango que `i`: el rango es (sin área < con área, área, índice).
//!   Así las relaciones siempre van de menor a mayor rango, no hay ciclos y el empate entre dos
//!   figuras que se solapan mutuamente se rompe de forma determinista (la menor área, y a igual
//!   área la de menor índice, va primero).
//! - Los contenedores que ya son ancestros de `i` en la contención estricta se omiten: ya están
//!   en el árbol.
//!
//! El análisis tiene un presupuesto de trabajo fijo para no volverse cuadrático con miles de
//! figuras grandes superpuestas; si se agota se devuelve lo encontrado hasta ahí y se avisa.

use tlaku_core::{Point, Polyline};

use crate::geom::{Bbox, TOUCH, area, inside};

const GRID: usize = 8;
const LINE_SAMPLES: usize = 16;
const MAX_SIDE: usize = 64;
/// Una figura que cubre más celdas que esto se trata aparte (no entra en la rejilla).
const LARGE_CELLS: usize = 16;
/// Presupuesto total de operaciones elementales (pruebas de caja y de lados).
const BUDGET: u64 = 40_000_000;

/// Una precedencia por solape: la figura debe cortarse antes que `outer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Extra {
    pub outer: usize,
    /// Porcentaje de la muestra que cae dentro de `outer` (0-100).
    pub share_pct: u8,
    /// La relación se decidió junto con otras figuras (la unión), no por `outer` sola.
    pub with_others: bool,
}

pub(crate) struct Overlaps {
    /// Por figura (índice de la entrada), sus precedencias por solape, ordenadas por `outer`.
    pub extras: Vec<Vec<Extra>>,
    /// Se agotó el presupuesto: faltan relaciones por analizar.
    pub truncated: bool,
}

/// `parents[i]` es el contenedor inmediato de `paths[i]` (de [`crate::contain::find_parents`]).
pub(crate) fn find_overlaps(paths: &[&Polyline], parents: &[Option<usize>]) -> Overlaps {
    let n = paths.len();
    let mut out = Overlaps {
        extras: vec![Vec::new(); n],
        truncated: false,
    };
    let boxes: Vec<Bbox> = paths.iter().map(|p| Bbox::of(&p.points)).collect();
    let areas: Vec<f64> = paths
        .iter()
        .map(|p| {
            if p.closed && p.points.len() >= 3 {
                area(&p.points)
            } else {
                0.0
            }
        })
        .collect();
    let has_area = |i: usize| areas[i] > TOUCH * TOUCH;
    let containers: Vec<usize> = (0..n).filter(|&i| has_area(i)).collect();
    if containers.is_empty() {
        return out;
    }
    // Rango: sin área < con área; luego área; luego índice.
    let rank_gt = |j: usize, i: usize| -> bool {
        match (has_area(j), has_area(i)) {
            (true, false) => true,
            (false, _) => false,
            (true, true) => (areas[j], j) > (areas[i], i),
        }
    };

    let mut work: u64 = 0;

    // Rejilla gruesa con los contenedores pequeños; los grandes van en una lista aparte.
    let side = ((containers.len() as f64).sqrt().ceil() as usize).clamp(1, MAX_SIDE);
    let mut scene = Bbox {
        min_x: f64::INFINITY,
        min_y: f64::INFINITY,
        max_x: f64::NEG_INFINITY,
        max_y: f64::NEG_INFINITY,
    };
    for b in boxes.iter().filter(|b| b.min_x.is_finite()) {
        scene.min_x = scene.min_x.min(b.min_x);
        scene.min_y = scene.min_y.min(b.min_y);
        scene.max_x = scene.max_x.max(b.max_x);
        scene.max_y = scene.max_y.max(b.max_y);
    }
    let (w, h) = (scene.max_x - scene.min_x, scene.max_y - scene.min_y);
    let cell = |v: f64, lo: f64, ext: f64| -> usize {
        if ext <= 0.0 {
            0
        } else {
            (((v - lo) / ext * side as f64).floor().max(0.0) as usize).min(side - 1)
        }
    };
    let cells_of = |b: &Bbox| {
        (
            cell(b.min_x, scene.min_x, w),
            cell(b.max_x, scene.min_x, w),
            cell(b.min_y, scene.min_y, h),
            cell(b.max_y, scene.min_y, h),
        )
    };
    let mut grid: Vec<Vec<u32>> = vec![Vec::new(); side * side];
    let mut large: Vec<usize> = Vec::new();
    for &c in &containers {
        let (x0, x1, y0, y1) = cells_of(&boxes[c]);
        if (x1 - x0 + 1) * (y1 - y0 + 1) > LARGE_CELLS {
            large.push(c);
            continue;
        }
        for y in y0..=y1 {
            for x in x0..=x1 {
                grid[y * side + x].push(c as u32);
            }
        }
    }

    // Marcas para no repetir candidatos ni contar ancestros.
    let mut seen = vec![u32::MAX; n];
    let mut anc = vec![u32::MAX; n];
    let mut cands: Vec<usize> = Vec::new();
    let mut counts: Vec<u32> = Vec::new();

    for i in 0..n {
        if boxes[i].min_x.is_infinite() {
            continue;
        }
        // Candidatos: cajas que se tocan, de mayor rango y que no son ancestros.
        cands.clear();
        let tag = i as u32;
        let mut a = parents[i];
        while let Some(p) = a {
            anc[p] = tag;
            a = parents[p];
        }
        let consider = |j: usize, cands: &mut Vec<usize>, seen: &mut Vec<u32>| {
            if j != i
                && seen[j] != tag
                && anc[j] != tag
                && rank_gt(j, i)
                && touches(&boxes[j], &boxes[i])
            {
                cands.push(j);
            }
            seen[j] = tag;
        };
        let (x0, x1, y0, y1) = cells_of(&boxes[i]);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let list = &grid[y * side + x];
                work += list.len() as u64 + 1;
                for &j in list {
                    consider(j as usize, &mut cands, &mut seen);
                }
            }
            if work > BUDGET {
                out.truncated = true;
                return out;
            }
        }
        work += large.len() as u64;
        for &j in &large {
            consider(j, &mut cands, &mut seen);
        }
        if work > BUDGET {
            out.truncated = true;
            return out;
        }
        if cands.is_empty() {
            continue;
        }
        cands.sort_unstable();

        let samples = sample_points(paths[i], &boxes[i], has_area(i));
        if samples.is_empty() {
            continue;
        }
        work += (samples.len() * paths[i].points.len().max(1)) as u64;
        counts.clear();
        counts.resize(cands.len(), 0);
        let mut union_hits = 0u32;
        for s in &samples {
            let mut any = false;
            for (k, &j) in cands.iter().enumerate() {
                let b = &boxes[j];
                if s.x < b.min_x - TOUCH
                    || s.x > b.max_x + TOUCH
                    || s.y < b.min_y - TOUCH
                    || s.y > b.max_y + TOUCH
                {
                    continue;
                }
                work += paths[j].points.len() as u64;
                if inside(*s, &paths[j].points) {
                    counts[k] += 1;
                    any = true;
                }
            }
            union_hits += any as u32;
            if work > BUDGET {
                out.truncated = true;
                return out;
            }
        }
        let total = samples.len() as u32;
        let union_mostly = union_hits * 2 > total;
        for (k, &j) in cands.iter().enumerate() {
            let cnt = counts[k];
            let single = cnt * 2 > total;
            let combined = union_mostly && cnt * 4 >= total && cnt > 0;
            if single || combined {
                out.extras[i].push(Extra {
                    outer: j,
                    share_pct: (cnt * 100 / total) as u8,
                    with_others: !single,
                });
            }
        }
    }
    out
}

fn touches(a: &Bbox, b: &Bbox) -> bool {
    a.min_x <= b.max_x + TOUCH
        && b.min_x <= a.max_x + TOUCH
        && a.min_y <= b.max_y + TOUCH
        && b.min_y <= a.max_y + TOUCH
}

/// Puntos que representan a la figura: cuadrícula sobre su área, o puntos repartidos por la
/// longitud si no tiene área. Determinista.
fn sample_points(path: &Polyline, b: &Bbox, has_area: bool) -> Vec<Point> {
    if has_area {
        let (w, h) = (b.max_x - b.min_x, b.max_y - b.min_y);
        let mut pts = Vec::with_capacity(GRID * GRID);
        for gy in 0..GRID {
            for gx in 0..GRID {
                let p = Point::new(
                    b.min_x + (gx as f64 + 0.5) / GRID as f64 * w,
                    b.min_y + (gy as f64 + 0.5) / GRID as f64 * h,
                );
                if inside(p, &path.points) {
                    pts.push(p);
                }
            }
        }
        if pts.len() >= 4 {
            return pts;
        }
        // Figura muy fina o en diagonal: la cuadrícula casi no entra. Se usan sus vértices.
        let step = (path.points.len() / LINE_SAMPLES).max(1);
        return path.points.iter().step_by(step).copied().collect();
    }
    along(path)
}

/// `LINE_SAMPLES` puntos a mitad de tramos iguales de la longitud de la trayectoria.
fn along(path: &Polyline) -> Vec<Point> {
    let pts = &path.points;
    if pts.is_empty() {
        return Vec::new();
    }
    let seg_count = if path.closed {
        pts.len()
    } else {
        pts.len() - 1
    };
    let seg = |k: usize| (pts[k], pts[(k + 1) % pts.len()]);
    let total: f64 = (0..seg_count)
        .map(|k| {
            let (a, b) = seg(k);
            a.dist(b)
        })
        .sum();
    if total <= 0.0 {
        return vec![pts[0]];
    }
    let mut out = Vec::with_capacity(LINE_SAMPLES);
    let (mut k, mut walked) = (0usize, 0.0f64);
    for s in 0..LINE_SAMPLES {
        let target = (s as f64 + 0.5) / LINE_SAMPLES as f64 * total;
        loop {
            let (a, b) = seg(k);
            let len = a.dist(b);
            if walked + len >= target || k + 1 >= seg_count {
                let t = if len > 0.0 {
                    ((target - walked) / len).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                out.push(Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t));
                break;
            }
            walked += len;
            k += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f64, y: f64, s: f64) -> Polyline {
        Polyline {
            points: vec![
                Point::new(x, y),
                Point::new(x + s, y),
                Point::new(x + s, y + s),
                Point::new(x, y + s),
            ],
            closed: true,
        }
    }

    fn run(shapes: &[Polyline]) -> Vec<Vec<Extra>> {
        let refs: Vec<&Polyline> = shapes.iter().collect();
        let parents = crate::contain::find_parents(&refs);
        find_overlaps(&refs, &parents).extras
    }

    #[test]
    fn mostly_overlapped_small_goes_before_big() {
        // 0 (grande) y 1 (menor, 75 % dentro de 0): 1 antes que 0.
        let e = run(&[square(0.0, 0.0, 20.0), square(10.0, 0.0, 14.0)]);
        assert!(e[0].is_empty());
        assert_eq!(e[1].len(), 1);
        assert_eq!(e[1][0].outer, 0);
        assert!(!e[1][0].with_others);
    }

    #[test]
    fn slight_overlap_is_not_a_relation() {
        let e = run(&[square(0.0, 0.0, 20.0), square(15.0, 15.0, 20.0)]);
        assert!(e.iter().all(Vec::is_empty));
    }

    #[test]
    fn equal_areas_break_ties_by_index() {
        // Dos cuadrados iguales con 80 % de solape: solo el de menor índice va antes.
        let e = run(&[square(0.0, 0.0, 10.0), square(2.0, 0.0, 10.0)]);
        assert_eq!(e[0].len(), 1);
        assert_eq!(e[0][0].outer, 1);
        assert!(e[1].is_empty());
    }

    #[test]
    fn straddling_figure_is_tied_to_both_pieces() {
        // La pequeña cruza la costura entre dos cuadrados que se tocan: 40 % en cada uno.
        let e = run(&[
            square(0.0, 0.0, 20.0),
            square(20.0, 0.0, 20.0),
            square(15.0, 5.0, 10.0),
        ]);
        let outers: Vec<usize> = e[2].iter().map(|x| x.outer).collect();
        assert_eq!(outers, vec![0, 1], "{e:?}");
        assert!(e[2].iter().all(|x| x.with_others));
    }

    #[test]
    fn no_cycles_ever() {
        // Anillo de cuadrados solapados: las relaciones solo van hacia mayor rango.
        let shapes: Vec<Polyline> = (0..12)
            .map(|k| {
                square(
                    (k % 4) as f64 * 6.0,
                    (k / 4) as f64 * 6.0,
                    10.0 + (k % 3) as f64,
                )
            })
            .collect();
        let e = run(&shapes);
        let areas: Vec<f64> = shapes.iter().map(|s| area(&s.points)).collect();
        for (i, list) in e.iter().enumerate() {
            for x in list {
                assert!((areas[x.outer], x.outer) > (areas[i], i));
            }
        }
    }
}
