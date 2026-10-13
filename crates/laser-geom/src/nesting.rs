use crate::util::{EPS, MIN_AREA, PointLocation, is_valid_point, locate_point, signed_area};
use laser_core::Polyline;

struct Info {
    min: (f64, f64),
    max: (f64, f64),
    area: f64,
}

fn info(p: &Polyline) -> Option<Info> {
    if !p.closed || p.points.len() < 3 || !p.points.iter().all(is_valid_point) {
        return None;
    }
    let area = signed_area(&p.points).abs();
    if area <= MIN_AREA {
        return None;
    }
    let mut min = (f64::INFINITY, f64::INFINITY);
    let mut max = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for q in &p.points {
        min = (min.0.min(q.x), min.1.min(q.y));
        max = (max.0.max(q.x), max.1.max(q.y));
    }
    Some(Info { min, max, area })
}

/// `outer` contiene a `inner`: es estrictamente mayor, su caja envuelve a la
/// otra y ningún vértice de `inner` queda fuera (al menos uno dentro).
fn contains(outer: (&Info, &Polyline), inner: (&Info, &Polyline)) -> bool {
    let (oi, op) = outer;
    let (ii, ip) = inner;
    if oi.area <= ii.area
        || ii.min.0 < oi.min.0 - EPS
        || ii.min.1 < oi.min.1 - EPS
        || ii.max.0 > oi.max.0 + EPS
        || ii.max.1 > oi.max.1 + EPS
    {
        return false;
    }
    let mut any_inside = false;
    for &v in &ip.points {
        match locate_point(v, &op.points) {
            PointLocation::Outside => return false,
            PointLocation::Inside => any_inside = true,
            PointLocation::Boundary => {}
        }
    }
    any_inside
}

/// Profundidad y contenedor inmediato (el de menor área) de cada trayectoria.
pub(crate) fn analyze(paths: &[Polyline]) -> (Vec<usize>, Vec<Option<usize>>) {
    let infos: Vec<Option<Info>> = paths.iter().map(info).collect();
    let mut depth = vec![0; paths.len()];
    let mut parent = vec![None; paths.len()];
    for (i, ii) in infos.iter().enumerate() {
        let Some(ii) = ii else { continue };
        let mut best: Option<(usize, f64)> = None;
        for (j, oj) in infos.iter().enumerate() {
            let Some(oj) = oj else { continue };
            if i != j && contains((oj, &paths[j]), (ii, &paths[i])) {
                depth[i] += 1;
                if best.is_none_or(|(_, a)| oj.area < a) {
                    best = Some((j, oj.area));
                }
            }
        }
        parent[i] = best.map(|(j, _)| j);
    }
    (depth, parent)
}

/// Profundidad de contención de cada trayectoria: 0 = exterior, 1 = agujero,
/// 2 = isla dentro del agujero, etc. (regla par-impar: las profundidades pares
/// son material y las impares, huecos).
///
/// Una trayectoria contiene a otra si es de mayor área y todos los vértices de
/// la otra quedan dentro o sobre su borde. Las trayectorias que se cruzan, se
/// tocan sin contenerse o son idénticas no se contienen entre sí. Las abiertas
/// y las degeneradas (menos de 3 puntos, área cero, no finitas) devuelven 0 y
/// no cuentan como contenedores. Coste O(n²) con filtro por caja envolvente.
pub fn nesting_depth(paths: &[Polyline]) -> Vec<usize> {
    analyze(paths).0
}
