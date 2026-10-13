//! Separar (Break Apart): partir un objeto con varios subtrayectos en un objeto por subtrayecto.

use crate::nesting::analyze;
use tlaku_core::bezpath::{BezPath, PathSeg};
use tlaku_core::{Point, Polyline};

/// Parte una trayectoria en sus subtrayectos. Cada uno empieza con `MoveTo`; un `Close` lo
/// termina, y si después de él siguen segmentos sin `MoveTo`, abren un subtrayecto nuevo desde
/// el inicio del anterior (como en SVG). Se descartan los subtrayectos sin ningún segmento
/// dibujado. Las curvas se conservan tal cual: no se aplanan.
pub fn split_subpaths(path: &BezPath) -> Vec<BezPath> {
    let mut out: Vec<BezPath> = Vec::new();
    let mut cur: Vec<PathSeg> = Vec::new();
    let mut start: Option<Point> = None;
    let mut drawn = false;
    let mut finish = |cur: &mut Vec<PathSeg>, drawn: &mut bool| {
        if *drawn && !cur.is_empty() {
            out.push(BezPath(std::mem::take(cur)));
        } else {
            cur.clear();
        }
        *drawn = false;
    };
    for seg in &path.0 {
        match *seg {
            PathSeg::MoveTo { to } => {
                finish(&mut cur, &mut drawn);
                start = Some(to);
                cur.push(*seg);
            }
            PathSeg::Close => {
                if !cur.is_empty() {
                    cur.push(*seg);
                    finish(&mut cur, &mut drawn);
                }
            }
            _ => {
                if cur.is_empty() {
                    // Segmentos tras un `Close`: siguen desde el inicio del subtrayecto anterior.
                    if let Some(s) = start {
                        cur.push(PathSeg::MoveTo { to: s });
                    }
                }
                cur.push(*seg);
                drawn = true;
            }
        }
    }
    finish(&mut cur, &mut drawn);
    out
}

/// Agrupa los índices de `paths` para el reparto de Separar. Sin `keep_holes` cada trayectoria
/// es su propio grupo. Con `keep_holes`, cada contorno exterior (profundidad par) se queda con
/// los huecos que contiene de forma inmediata (profundidad impar); una isla dentro de un hueco
/// es otro exterior. Las trayectorias abiertas van siempre solas.
///
/// Los grupos salen en el orden de su primer miembro; dentro de cada uno, el exterior primero.
pub fn break_groups(paths: &[Polyline], keep_holes: bool) -> Vec<Vec<usize>> {
    if !keep_holes {
        return (0..paths.len()).map(|i| vec![i]).collect();
    }
    let (depth, parent) = analyze(paths);
    // Cada trayectoria apunta a su grupo: los exteriores abren uno y los huecos se unen al de su padre.
    let mut group_of: Vec<usize> = vec![usize::MAX; paths.len()];
    let mut groups: Vec<Vec<usize>> = Vec::new();
    // Se procesa por profundidad creciente para que el padre ya tenga grupo.
    let mut order: Vec<usize> = (0..paths.len()).collect();
    order.sort_by_key(|&i| depth[i]);
    for i in order {
        let hole = depth[i] % 2 == 1 && paths[i].closed;
        match (hole, parent[i]) {
            (true, Some(p)) if group_of[p] != usize::MAX => {
                group_of[i] = group_of[p];
                groups[group_of[p]].push(i);
            }
            _ => {
                group_of[i] = groups.len();
                groups.push(vec![i]);
            }
        }
    }
    // Orden estable por el primer miembro de cada grupo.
    groups.sort_by_key(|g| g.iter().copied().min().unwrap_or(usize::MAX));
    for g in &mut groups {
        g[1..].sort_unstable();
    }
    groups
}
