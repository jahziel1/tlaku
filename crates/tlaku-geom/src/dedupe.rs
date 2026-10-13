use std::collections::HashMap;
use tlaku_core::{Point, Polyline};

use crate::util::{EPS, is_valid_point};

/// Resultado de [`dedupe_paths`].
#[derive(Debug, Clone, PartialEq)]
pub struct DedupeResult {
    /// Trayectorias resultantes. Las que no perdieron nada son idénticas a su
    /// entrada; las demás se reemplazan por las piezas que sobreviven (una
    /// entrada puede dar varias piezas abiertas o ninguna).
    pub paths: Vec<Polyline>,
    /// Índice (en la entrada) de la trayectoria de la que sale cada elemento
    /// de `paths`, en el mismo orden.
    pub origin: Vec<usize>,
    /// Para cada trayectoria de la entrada: `true` si perdió algún tramo.
    pub changed: Vec<bool>,
    /// Largo total (mm) de trazo eliminado.
    pub removed_length: f64,
}

/// Segmento de alguna trayectoria de la entrada.
struct Seg {
    a: Point,
    b: Point,
    len: f64,
    path: usize,
}

/// Distancia perpendicular de `p` a la recta que contiene a `s`.
fn dist_to_line(p: Point, s: &Seg) -> f64 {
    ((p.x - s.a.x) * (s.b.y - s.a.y) - (p.y - s.a.y) * (s.b.x - s.a.x)).abs() / s.len
}

/// Parámetro (0 en `a`, 1 en `b`) de la proyección de `p` sobre la recta de `s`.
fn param(p: Point, s: &Seg) -> f64 {
    ((p.x - s.a.x) * (s.b.x - s.a.x) + (p.y - s.a.y) * (s.b.y - s.a.y)) / (s.len * s.len)
}

/// Tramo de `j` (como parámetros 0..1) que queda sobre `i`, si son colineales
/// dentro de `tol`. Que se toquen en un punto o solapen menos de `tol` no
/// cuenta, salvo que `i` cubra entero a un `j` más corto que `tol`.
fn covered(i: &Seg, j: &Seg, tol: f64) -> Option<(f64, f64)> {
    let (long, short) = if i.len >= j.len { (i, j) } else { (j, i) };
    if dist_to_line(short.a, long) > tol || dist_to_line(short.b, long) > tol {
        return None;
    }
    let (t0, t1) = (param(i.a, j), param(i.b, j));
    let lo = t0.min(t1).max(0.0);
    let hi = t0.max(t1).min(1.0);
    let overlap = (hi - lo) * j.len;
    (overlap > tol || (overlap > EPS && overlap >= j.len - EPS)).then_some((lo, hi))
}

/// Partes de 0..1 que no cubre ninguna de `intervals`; descarta las de `tol` mm
/// o menos (astillas que deja el recorte).
fn remainder(intervals: &mut [(f64, f64)], len: f64, tol: f64) -> Vec<(f64, f64)> {
    intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = Vec::new();
    let mut cursor = 0.0_f64;
    for &(lo, hi) in intervals.iter() {
        if lo > cursor {
            out.push((cursor, lo));
        }
        cursor = cursor.max(hi);
    }
    if cursor < 1.0 {
        out.push((cursor, 1.0));
    }
    out.retain(|(a, b)| (b - a) * len > tol);
    out
}

/// Puntos que muestrean `s` cada `step` mm o menos (extremos incluidos).
fn samples(s: &Seg, step: f64) -> impl Iterator<Item = Point> + '_ {
    let n = ((s.len / step).ceil() as usize).clamp(1, 100_000);
    (0..=n).map(move |k| {
        let t = k as f64 / n as f64;
        Point::new(s.a.x + (s.b.x - s.a.x) * t, s.a.y + (s.b.y - s.a.y) * t)
    })
}

fn cell_of(v: f64, size: f64) -> i64 {
    (v / size).floor() as i64
}

fn same_point(a: Point, b: Point) -> bool {
    a.dist(b) < 1e-12
}

/// Quita los tramos repetidos o superpuestos: dos segmentos colineales (a
/// `tolerance` mm o menos de la misma recta) que se solapan más de `tolerance`
/// mm cubren el mismo trazo, y el láser lo cortaría dos veces.
///
/// - Se conserva el trazo que aparece **primero** en la entrada (orden de
///   trayectorias y, dentro de ellas, de puntos); del posterior se recorta
///   solo la parte solapada. Un solape parcial parte el segmento: lo que queda
///   fuera sigue existiendo. Si el posterior estaba contenido, desaparece; si
///   contenía al anterior, queda en dos mitades.
/// - Los segmentos no se desplazan: los recortes caen sobre el propio
///   segmento, así que la forma que se conserva es la del primero.
/// - Las trayectorias que no pierden nada pasan idénticas (también las
///   cerradas). Una cerrada que pierde un tramo se vuelve abierta; si el
///   hueco queda en mitad de su lista de puntos, el trazo se reconecta por el
///   punto de partida.
/// - Los restos de `tolerance` mm o menos que dejaría un recorte se descartan.
/// - `tolerance` <= 0 o NaN se trata como 1e-6 mm. Se usa una rejilla
///   espacial, así que el coste es aproximadamente lineal con pocos
///   solapamientos.
/// - Las trayectorias con menos de 2 puntos o puntos no finitos pasan sin
///   cambios.
pub fn dedupe_paths(paths: &[Polyline], tolerance: f64) -> DedupeResult {
    let tol = if tolerance.is_finite() {
        tolerance.max(1e-6)
    } else {
        1e-6
    };

    // Segmentos de cada trayectoria, en orden. `ranges[p]` = su tramo en `segs`.
    let mut segs: Vec<Seg> = Vec::new();
    let mut ranges: Vec<std::ops::Range<usize>> = Vec::with_capacity(paths.len());
    for (path_idx, p) in paths.iter().enumerate() {
        let start = segs.len();
        let n = p.points.len();
        if n >= 2 && p.points.iter().all(is_valid_point) {
            let count = if p.closed { n } else { n - 1 };
            for k in 0..count {
                let (a, b) = (p.points[k], p.points[(k + 1) % n]);
                segs.push(Seg {
                    a,
                    b,
                    len: a.dist(b),
                    path: path_idx,
                });
            }
        }
        ranges.push(start..segs.len());
    }

    // Rejilla: tamaño de celda ~ largo medio de segmento (mínimo 4·tol).
    let usable: Vec<f64> = segs.iter().map(|s| s.len).filter(|&l| l > EPS).collect();
    let avg = if usable.is_empty() {
        1.0
    } else {
        usable.iter().sum::<f64>() / usable.len() as f64
    };
    let cell = avg.clamp(4.0 * tol, 100.0_f64.max(4.0 * tol));
    let step = cell / 4.0;
    let margin = tol + step;
    let mut grid: HashMap<(i64, i64), Vec<u32>> = HashMap::new();

    // Tramos que sobreviven de cada segmento (parámetros 0..1).
    let mut kept: Vec<Vec<(f64, f64)>> = Vec::with_capacity(segs.len());
    let mut removed_length = 0.0;
    let mut changed = vec![false; paths.len()];
    let mut candidates: Vec<u32> = Vec::new();
    for (j, sj) in segs.iter().enumerate() {
        if sj.len <= EPS {
            kept.push(vec![(0.0, 1.0)]);
            continue;
        }
        candidates.clear();
        for p in samples(sj, step) {
            if let Some(ids) = grid.get(&(cell_of(p.x, cell), cell_of(p.y, cell))) {
                candidates.extend_from_slice(ids);
            }
        }
        candidates.sort_unstable();
        candidates.dedup();
        let mut cover: Vec<(f64, f64)> = candidates
            .iter()
            .filter_map(|&i| covered(&segs[i as usize], sj, tol))
            .collect();
        let pieces = if cover.is_empty() {
            vec![(0.0, 1.0)]
        } else {
            remainder(&mut cover, sj.len, tol)
        };
        let kept_len: f64 = pieces.iter().map(|(a, b)| (b - a) * sj.len).sum();
        if kept_len < sj.len - EPS {
            changed[sj.path] = true;
            removed_length += sj.len - kept_len;
        }
        kept.push(pieces);

        // Este segmento (entero) queda disponible para recortar a los siguientes.
        for p in samples(sj, step) {
            let (x0, x1) = (cell_of(p.x - margin, cell), cell_of(p.x + margin, cell));
            let (y0, y1) = (cell_of(p.y - margin, cell), cell_of(p.y + margin, cell));
            for cx in x0..=x1 {
                for cy in y0..=y1 {
                    let v = grid.entry((cx, cy)).or_default();
                    if v.last() != Some(&(j as u32)) {
                        v.push(j as u32);
                    }
                }
            }
        }
    }

    let mut out = Vec::new();
    let mut origin = Vec::new();
    for (p_idx, p) in paths.iter().enumerate() {
        if !changed[p_idx] {
            out.push(p.clone());
            origin.push(p_idx);
            continue;
        }
        let mut chains: Vec<Vec<Point>> = Vec::new();
        let mut cur: Vec<Point> = Vec::new();
        for k in ranges[p_idx].clone() {
            let s = &segs[k];
            let at = |t: f64| {
                if t <= 0.0 {
                    s.a
                } else if t >= 1.0 {
                    s.b
                } else {
                    Point::new(s.a.x + (s.b.x - s.a.x) * t, s.a.y + (s.b.y - s.a.y) * t)
                }
            };
            for &(t0, t1) in &kept[k] {
                let (p0, p1) = (at(t0), at(t1));
                if cur.last().is_some_and(|&l| same_point(l, p0)) {
                    cur.push(p1);
                } else {
                    if cur.len() >= 2 {
                        chains.push(std::mem::take(&mut cur));
                    }
                    cur = vec![p0, p1];
                }
            }
        }
        if cur.len() >= 2 {
            chains.push(cur);
        }
        // Anillo con el hueco a mitad de la lista: el último trozo continúa en el primero.
        if p.closed && chains.len() >= 2 {
            let wraps = match (chains.last(), chains.first()) {
                (Some(last), Some(first)) => same_point(last[last.len() - 1], first[0]),
                _ => false,
            };
            if wraps {
                let first = chains.remove(0);
                if let Some(last) = chains.last_mut() {
                    last.extend(first.into_iter().skip(1));
                }
            }
        }
        for points in chains {
            out.push(Polyline {
                points,
                closed: false,
            });
            origin.push(p_idx);
        }
    }

    DedupeResult {
        paths: out,
        origin,
        changed,
        removed_length,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(pts: &[(f64, f64)]) -> Polyline {
        Polyline {
            points: pts.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            closed: false,
        }
    }

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Polyline {
        Polyline {
            points: vec![
                Point::new(x0, y0),
                Point::new(x1, y0),
                Point::new(x1, y1),
                Point::new(x0, y1),
            ],
            closed: true,
        }
    }

    fn total_len(paths: &[Polyline]) -> f64 {
        paths
            .iter()
            .map(|p| {
                let n = p.points.len();
                let count = if p.closed { n } else { n.saturating_sub(1) };
                (0..count)
                    .map(|k| p.points[k].dist(p.points[(k + 1) % n]))
                    .sum::<f64>()
            })
            .sum()
    }

    #[test]
    fn linea_repetida_se_queda_una_vez() {
        let r = dedupe_paths(
            &[
                line(&[(0.0, 0.0), (10.0, 0.0)]),
                line(&[(10.0, 0.0), (0.0, 0.0)]),
            ],
            0.01,
        );
        assert_eq!(r.paths.len(), 1);
        assert_eq!(r.origin, vec![0]);
        assert_eq!(r.changed, vec![false, true]);
        assert!((r.removed_length - 10.0).abs() < 1e-9);
    }

    #[test]
    fn dentro_de_la_tolerancia_cuenta_y_fuera_no() {
        let a = line(&[(0.0, 0.0), (10.0, 0.0)]);
        let cerca = line(&[(0.0, 0.005), (10.0, 0.005)]);
        let lejos = line(&[(0.0, 0.5), (10.0, 0.5)]);
        let r = dedupe_paths(&[a.clone(), cerca], 0.01);
        assert_eq!(r.paths.len(), 1);
        let r = dedupe_paths(&[a, lejos], 0.01);
        assert_eq!(r.paths.len(), 2);
        assert!(r.changed.iter().all(|c| !c));
        assert_eq!(r.removed_length, 0.0);
    }

    #[test]
    fn solape_parcial_parte_el_segmento() {
        // [0,10] y [5,15]: del segundo solo sobrevive [10,15].
        let r = dedupe_paths(
            &[
                line(&[(0.0, 0.0), (10.0, 0.0)]),
                line(&[(5.0, 0.0), (15.0, 0.0)]),
            ],
            0.01,
        );
        assert_eq!(r.paths.len(), 2);
        assert_eq!(r.paths[0], line(&[(0.0, 0.0), (10.0, 0.0)]));
        assert_eq!(r.paths[1].points.len(), 2);
        assert!((r.paths[1].points[0].x - 10.0).abs() < 1e-9);
        assert!((r.paths[1].points[1].x - 15.0).abs() < 1e-9);
        assert!((r.removed_length - 5.0).abs() < 1e-9);
    }

    #[test]
    fn el_posterior_que_contiene_al_anterior_queda_en_dos() {
        let r = dedupe_paths(
            &[
                line(&[(4.0, 0.0), (6.0, 0.0)]),
                line(&[(0.0, 0.0), (10.0, 0.0)]),
            ],
            0.01,
        );
        assert_eq!(r.paths.len(), 3);
        assert_eq!(r.origin, vec![0, 1, 1]);
        assert!((total_len(&r.paths) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn diagonal_y_perpendicular_no_se_tocan() {
        let r = dedupe_paths(
            &[
                line(&[(0.0, 0.0), (10.0, 10.0)]),
                line(&[(0.0, 10.0), (10.0, 0.0)]),
                line(&[(5.0, 0.0), (5.0, 20.0)]),
            ],
            0.01,
        );
        assert!(r.changed.iter().all(|c| !c));
        assert_eq!(r.paths.len(), 3);
    }

    #[test]
    fn colineales_consecutivas_de_un_trazo_no_se_alteran() {
        let p = line(&[(0.0, 0.0), (5.0, 0.0), (10.0, 0.0)]);
        let r = dedupe_paths(std::slice::from_ref(&p), 0.01);
        assert_eq!(r.paths, vec![p]);
        assert!(!r.changed[0]);
    }

    #[test]
    fn dos_cuadrados_con_borde_comun_cortan_el_borde_una_vez() {
        let a = square(0.0, 0.0, 10.0, 10.0);
        let b = square(10.0, 0.0, 20.0, 10.0);
        let before = total_len(&[a.clone(), b.clone()]);
        let r = dedupe_paths(&[a.clone(), b], 0.01);
        assert_eq!(r.paths.len(), 2);
        assert_eq!(r.paths[0], a, "el primero queda igual y cerrado");
        assert!(!r.paths[1].closed);
        assert_eq!(r.origin, vec![0, 1]);
        assert!((total_len(&r.paths) - (before - 10.0)).abs() < 1e-9);
        // Las tres caras que quedan forman un solo trazo continuo.
        assert_eq!(r.paths[1].points.len(), 4);
    }

    #[test]
    fn el_hueco_a_mitad_del_anillo_se_reconecta() {
        // Lado inferior del segundo cuadrado = índice 2 de su lista (empieza arriba).
        let a = line(&[(0.0, 0.0), (10.0, 0.0)]);
        let b = Polyline {
            points: vec![
                Point::new(0.0, 5.0),
                Point::new(10.0, 5.0),
                Point::new(10.0, 0.0),
                Point::new(0.0, 0.0),
            ],
            closed: true,
        };
        let r = dedupe_paths(&[a, b], 0.01);
        // Segmento (10,0)-(0,0) eliminado: queda un trazo continuo (0,0)->(0,5)->(10,5)->(10,0).
        assert_eq!(r.paths.len(), 2);
        assert_eq!(r.paths[1].points.len(), 4);
        assert!(!r.paths[1].closed);
        assert_eq!(r.paths[1].points[0], Point::new(0.0, 0.0));
        assert_eq!(r.paths[1].points[3], Point::new(10.0, 0.0));
    }

    #[test]
    fn un_duplicado_exacto_desaparece_por_completo() {
        let a = square(0.0, 0.0, 10.0, 10.0);
        let r = dedupe_paths(&[a.clone(), a.clone()], 0.01);
        assert_eq!(r.paths, vec![a]);
        assert_eq!(r.origin, vec![0]);
        assert!((r.removed_length - 40.0).abs() < 1e-9);
    }

    #[test]
    fn entradas_degeneradas_pasan_sin_cambios() {
        let punto = line(&[(1.0, 1.0)]);
        let nan = line(&[(f64::NAN, 0.0), (1.0, 1.0)]);
        let r = dedupe_paths(&[punto.clone(), nan.clone()], f64::NAN);
        assert_eq!(r.paths.len(), 2);
        assert_eq!(r.paths[0], punto);
        assert!(dedupe_paths(&[], 0.01).paths.is_empty());
    }

    #[test]
    fn miles_de_segmentos_con_mitad_duplicados() {
        // Malla de 40x40 celdas de 2 mm: 41 líneas por eje; todas repetidas en sentido contrario.
        let mut paths = Vec::new();
        for i in 0..=40 {
            let v = i as f64 * 2.0;
            for k in 0..40 {
                let u = k as f64 * 2.0;
                paths.push(line(&[(u, v), (u + 2.0, v)]));
                paths.push(line(&[(v, u), (v, u + 2.0)]));
            }
        }
        let n = paths.len();
        let mut doble = paths.clone();
        for p in &paths {
            doble.push(Polyline {
                points: p.points.iter().rev().copied().collect(),
                closed: false,
            });
        }
        let r = dedupe_paths(&doble, 0.01);
        assert_eq!(r.paths.len(), n);
        assert!((r.removed_length - total_len(&paths)).abs() < 1e-6);
    }
}
