//! Orden de corte: menos recorrido en vacío significa trabajos más rápidos.

use crate::{Point, Polyline};

/// Ordena por vecino más cercano. Las trayectorias abiertas se pueden invertir y
/// las cerradas empiezan en el vértice más cercano a la posición actual.
pub fn order_paths(mut remaining: Vec<Polyline>, from: Point) -> Vec<Polyline> {
    let mut ordered = Vec::with_capacity(remaining.len());
    let mut pos = from;
    while !remaining.is_empty() {
        let mut best = (0, f64::INFINITY);
        for (i, path) in remaining.iter().enumerate() {
            let d = entry_distance(path, pos);
            if d < best.1 {
                best = (i, d);
            }
        }
        let mut path = remaining.swap_remove(best.0);
        enter_from(&mut path, pos);
        pos = path.end();
        ordered.push(path);
    }
    ordered
}

fn nearest_vertex(path: &Polyline, pos: Point) -> (usize, f64) {
    path.points
        .iter()
        .enumerate()
        .map(|(i, p)| (i, p.dist(pos)))
        .fold((0, f64::INFINITY), |a, b| if b.1 < a.1 { b } else { a })
}

fn entry_distance(path: &Polyline, pos: Point) -> f64 {
    if path.closed {
        nearest_vertex(path, pos).1
    } else {
        path.start().dist(pos).min(path.end().dist(pos))
    }
}

fn enter_from(path: &mut Polyline, pos: Point) {
    if path.closed {
        let (i, _) = nearest_vertex(path, pos);
        path.points.rotate_left(i);
    } else if path.end().dist(pos) < path.start().dist(pos) {
        path.points.reverse();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(a: (f64, f64), b: (f64, f64)) -> Polyline {
        Polyline {
            points: vec![Point::new(a.0, a.1), Point::new(b.0, b.1)],
            closed: false,
        }
    }

    #[test]
    fn picks_nearest_and_reverses_open_paths() {
        let far = line((100.0, 0.0), (110.0, 0.0));
        let near_reversed = line((20.0, 0.0), (1.0, 0.0));
        let out = order_paths(vec![far, near_reversed], Point::new(0.0, 0.0));
        assert_eq!(out[0].start(), Point::new(1.0, 0.0));
        assert_eq!(out[1].start(), Point::new(100.0, 0.0));
    }

    #[test]
    fn closed_paths_start_at_nearest_vertex() {
        let square = Polyline {
            points: vec![
                Point::new(10.0, 10.0),
                Point::new(20.0, 10.0),
                Point::new(20.0, 20.0),
                Point::new(10.0, 20.0),
            ],
            closed: true,
        };
        let out = order_paths(vec![square], Point::new(25.0, 25.0));
        assert_eq!(out[0].start(), Point::new(20.0, 20.0));
        assert_eq!(out[0].points.len(), 4);
    }
}
