//! Contención entre trayectorias: quién está dentro de quién.

use laser_core::{Point, Polyline};

use crate::geom::{Bbox, TOUCH, area, inside};

const MAX_SIDE: usize = 64;

/// Para cada trayectoria, el contenedor más pequeño que la rodea (o `None`).
///
/// Solo las cerradas con al menos 3 puntos y área positiva contienen. La contenida puede ser
/// abierta o cerrada. Para evitar ciclos (formas idénticas, solapadas) el contenedor debe
/// tener un área estrictamente mayor. La prueba filtra por caja envolvente en una rejilla
/// gruesa y luego exige que el primer, medio y último vértice estén dentro del polígono.
pub(crate) fn find_parents(paths: &[&Polyline]) -> Vec<Option<usize>> {
    let n = paths.len();
    let boxes: Vec<Bbox> = paths.iter().map(|p| Bbox::of(&p.points)).collect();
    let areas: Vec<f64> = paths
        .iter()
        .map(|p| if p.closed { area(&p.points) } else { 0.0 })
        .collect();
    let containers: Vec<usize> = (0..n)
        .filter(|&i| paths[i].closed && paths[i].points.len() >= 3 && areas[i] > TOUCH * TOUCH)
        .collect();
    let mut parents = vec![None; n];
    if containers.is_empty() {
        return parents;
    }

    let side = ((containers.len() as f64).sqrt().ceil() as usize).clamp(1, MAX_SIDE);
    let mut scene = Bbox {
        min_x: f64::INFINITY,
        min_y: f64::INFINITY,
        max_x: f64::NEG_INFINITY,
        max_y: f64::NEG_INFINITY,
    };
    for &c in &containers {
        let b = boxes[c];
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

    let mut grid: Vec<Vec<u32>> = vec![Vec::new(); side * side];
    for &c in &containers {
        let b = boxes[c];
        let (x0, x1) = (cell(b.min_x, scene.min_x, w), cell(b.max_x, scene.min_x, w));
        let (y0, y1) = (cell(b.min_y, scene.min_y, h), cell(b.max_y, scene.min_y, h));
        for y in y0..=y1 {
            for x in x0..=x1 {
                grid[y * side + x].push(c as u32);
            }
        }
    }

    for i in 0..n {
        // Si i cabe en un contenedor, la celda de su esquina mínima está en la lista de ese
        // contenedor (el cálculo de celda es monótono).
        let (cx, cy) = (
            cell(boxes[i].min_x, scene.min_x, w),
            cell(boxes[i].min_y, scene.min_y, h),
        );
        let pts = &paths[i].points;
        let samples = [pts[0], pts[pts.len() / 2], pts[pts.len() - 1]];
        let mut best: Option<(f64, usize)> = None;
        for &c in &grid[cy * side + cx] {
            let c = c as usize;
            if c == i || areas[c] <= areas[i] * (1.0 + 1e-9) {
                continue;
            }
            if best.is_some_and(|(a, bi)| (areas[c], c) >= (a, bi)) {
                continue;
            }
            if !boxes[c].contains_box(&boxes[i], TOUCH) {
                continue;
            }
            if samples.iter().all(|&s: &Point| inside(s, &paths[c].points)) {
                best = Some((areas[c], c));
            }
        }
        parents[i] = best.map(|(_, c)| c);
    }
    parents
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

    fn open(a: (f64, f64), b: (f64, f64)) -> Polyline {
        Polyline {
            points: vec![Point::new(a.0, a.1), Point::new(b.0, b.1)],
            closed: false,
        }
    }

    #[test]
    fn nested_squares_pick_smallest_container() {
        let (a, b, c) = (
            square(0.0, 0.0, 100.0),
            square(10.0, 10.0, 50.0),
            square(20.0, 20.0, 10.0),
        );
        let p = find_parents(&[&a, &b, &c]);
        assert_eq!(p, vec![None, Some(0), Some(1)]);
    }

    #[test]
    fn open_path_inside_and_crossing() {
        let sq = square(0.0, 0.0, 100.0);
        let inner = open((10.0, 10.0), (20.0, 20.0));
        let crossing = open((50.0, 50.0), (150.0, 50.0));
        let p = find_parents(&[&sq, &inner, &crossing]);
        assert_eq!(p, vec![None, Some(0), None]);
    }

    #[test]
    fn open_path_cannot_contain_and_siblings_are_independent() {
        let a = square(0.0, 0.0, 10.0);
        let b = square(20.0, 0.0, 10.0);
        let line = open((0.0, 0.0), (100.0, 0.0));
        assert_eq!(find_parents(&[&a, &b, &line]), vec![None, None, None]);
    }

    #[test]
    fn identical_shapes_do_not_form_cycles() {
        let a = square(0.0, 0.0, 10.0);
        let b = square(0.0, 0.0, 10.0);
        assert_eq!(find_parents(&[&a, &b]), vec![None, None]);
    }

    #[test]
    fn degenerate_closed_paths_are_not_containers() {
        let two = Polyline {
            points: vec![Point::new(0.0, 0.0), Point::new(10.0, 10.0)],
            closed: true,
        };
        let dot = open((5.0, 5.0), (5.0, 5.0));
        assert_eq!(find_parents(&[&two, &dot]), vec![None, None]);
    }

    #[test]
    fn concave_container_does_not_swallow_the_notch() {
        let l = Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(10.0, 0.0),
                Point::new(10.0, 2.0),
                Point::new(2.0, 2.0),
                Point::new(2.0, 10.0),
                Point::new(0.0, 10.0),
            ],
            closed: true,
        };
        let in_notch = square(5.0, 5.0, 2.0);
        let in_arm = square(0.5, 5.0, 1.0);
        assert_eq!(
            find_parents(&[&l, &in_notch, &in_arm]),
            vec![None, None, Some(0)]
        );
    }
}
