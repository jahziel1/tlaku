use std::collections::{HashMap, VecDeque};
use tlaku_core::{Point, Polyline};

use crate::util::{EPS, is_valid_point};

fn cell(p: Point, size: f64) -> (i64, i64) {
    ((p.x / size).floor() as i64, (p.y / size).floor() as i64)
}

/// Une trayectorias abiertas cuyos extremos están a `tolerance` mm o menos
/// (Auto-Join), invirtiéndolas si hace falta, y cierra las que terminan donde
/// empiezan.
///
/// - Desempate: en cada paso se une el extremo más cercano; a igual distancia,
///   el de menor índice en la entrada. El resultado es determinista.
/// - Al unir, el punto del extremo que se agrega se descarta (queda el del
///   trazo que ya se estaba armando), así que un hueco de hasta `tolerance` se
///   sustituye por un segmento recto.
/// - Un trazo se marca cerrado si, tras unirlo, su último punto queda a
///   `tolerance` o menos del primero y quedan al menos 3 puntos; el último
///   punto se elimina (el primero no se repite).
/// - Las cerradas, las de menos de 2 puntos y las no finitas pasan sin
///   cambios. Cada resultado ocupa el lugar de la primera trayectoria de la
///   que proviene.
/// - `tolerance` <= 0 o NaN solo une extremos exactamente iguales. Coste
///   aproximadamente lineal (rejilla espacial).
pub fn auto_join(paths: &[Polyline], tolerance: f64) -> Vec<Polyline> {
    auto_join_tracked(paths, tolerance)
        .into_iter()
        .map(|(path, _)| path)
        .collect()
}

/// Como [`auto_join`], pero cada resultado va acompañado de los índices (de
/// `paths`) de las trayectorias de las que proviene, en el orden en que se
/// unieron: el primero es la semilla. Un resultado con una sola fuente que no
/// se cerró es idéntico a su entrada; así quien llama sabe qué cambió sin
/// comparar puntos.
pub fn auto_join_tracked(paths: &[Polyline], tolerance: f64) -> Vec<(Polyline, Vec<usize>)> {
    let tol = if tolerance.is_finite() && tolerance > 0.0 {
        tolerance
    } else {
        0.0
    };
    let cell_size = tol.max(1e-6);

    let joinable =
        |p: &Polyline| !p.closed && p.points.len() >= 2 && p.points.iter().all(is_valid_point);

    let mut grid: HashMap<(i64, i64), Vec<(usize, bool)>> = HashMap::new();
    for (i, p) in paths.iter().enumerate().filter(|(_, p)| joinable(p)) {
        grid.entry(cell(p.points[0], cell_size))
            .or_default()
            .push((i, false));
        grid.entry(cell(p.points[p.points.len() - 1], cell_size))
            .or_default()
            .push((i, true));
    }

    let endpoint = |i: usize, at_end: bool| {
        let pts = &paths[i].points;
        if at_end { pts[pts.len() - 1] } else { pts[0] }
    };
    // Candidato más cercano a `q`: (distancia, índice, extremo).
    let nearest = |q: Point, used: &[bool]| -> Option<(f64, usize, bool)> {
        let (cx, cy) = cell(q, cell_size);
        let mut best: Option<(f64, usize, bool)> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                let Some(list) = grid.get(&(cx.saturating_add(dx), cy.saturating_add(dy))) else {
                    continue;
                };
                for &(k, at_end) in list {
                    if used[k] {
                        continue;
                    }
                    let d = q.dist(endpoint(k, at_end));
                    if d > tol {
                        continue;
                    }
                    let better = best.is_none_or(|(bd, bk, be)| (d, k, at_end) < (bd, bk, be));
                    if better {
                        best = Some((d, k, at_end));
                    }
                }
            }
        }
        best
    };

    let mut used = vec![false; paths.len()];
    let mut out = Vec::new();
    for (seed, path) in paths.iter().enumerate() {
        if !joinable(path) {
            out.push((path.clone(), vec![seed]));
            continue;
        }
        if used[seed] {
            continue;
        }
        used[seed] = true;
        let mut sources = vec![seed];
        let mut chain: VecDeque<Point> = path.points.iter().copied().collect();
        loop {
            let tail = nearest(chain[chain.len() - 1], &used).map(|c| (c, true));
            let head = nearest(chain[0], &used).map(|c| (c, false));
            let pick = match (tail, head) {
                (Some(t), Some(h)) => Some(if (h.0.0, h.0.1) < (t.0.0, t.0.1) {
                    h
                } else {
                    t
                }),
                (t, h) => t.or(h),
            };
            let Some(((_, k, k_at_end), at_tail)) = pick else {
                break;
            };
            used[k] = true;
            sources.push(k);
            let pts = &paths[k].points;
            // Puntos de k en el orden en que se recorren desde el empalme hacia afuera.
            let from_joint: Vec<Point> = if k_at_end {
                pts.iter().rev().copied().collect()
            } else {
                pts.to_vec()
            };
            if at_tail {
                chain.extend(from_joint.into_iter().skip(1));
            } else {
                for p in from_joint.into_iter().skip(1) {
                    chain.push_front(p);
                }
            }
        }
        let mut points: Vec<Point> = chain.into();
        let closed = points.len() >= 4 && points[0].dist(points[points.len() - 1]) <= tol.max(EPS);
        if closed {
            points.pop();
        }
        out.push((Polyline { points, closed }, sources));
    }
    out
}

/// Cierra las abiertas de 3 o más puntos cuyos extremos distan `tolerance` mm
/// o menos (Close Path). No une trayectorias distintas; para eso está
/// [`auto_join`]. Si los extremos no coinciden exactamente, el cierre es el
/// segmento recto implícito entre el último punto y el primero (se añade una
/// línea, no se mueven puntos). Las demás pasan sin cambios.
pub fn close_paths(paths: &[Polyline], tolerance: f64) -> Vec<Polyline> {
    let tol = if tolerance.is_finite() && tolerance > 0.0 {
        tolerance
    } else {
        0.0
    };
    paths
        .iter()
        .map(|p| {
            let n = p.points.len();
            if p.closed || n < 3 || !p.points.iter().all(is_valid_point) {
                return p.clone();
            }
            let gap = p.points[0].dist(p.points[n - 1]);
            if gap > tol.max(EPS) {
                return p.clone();
            }
            let mut points = p.points.clone();
            if gap <= EPS && n > 3 {
                points.pop();
            }
            Polyline {
                points,
                closed: true,
            }
        })
        .collect()
}
