//! Agrupación de polígonos en componentes independientes para el modo `PerShape`.
//!
//! Un componente es un contorno con sus agujeros (y lo que haya dentro de ellos) o un
//! conjunto de formas que se tocan o se cruzan. Dos formas solo se unen si de verdad
//! interactúan (aristas que se cortan o una dentro de la otra), no por el simple
//! solape de sus cajas: así dos letras con cajas que se montan ("AV") siguen siendo
//! formas distintas y cada una se rellena completa antes de pasar a la otra.

use tlaku_core::Point;

/// Caja envolvente en el marco de barrido.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BBox {
    pub xmin: f64,
    pub xmax: f64,
    pub ymin: f64,
    pub ymax: f64,
}

fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// Tope de pares de aristas que se comparan para decidir si dos formas se cortan; por encima
/// se asume que interactúan (agrupar de más es siempre correcto, solo menos fino).
const MAX_EDGE_PAIRS: usize = 250_000;

fn cross(o: Point, a: Point, b: Point) -> f64 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

fn on_segment(a: Point, b: Point, p: Point) -> bool {
    p.x >= a.x.min(b.x) && p.x <= a.x.max(b.x) && p.y >= a.y.min(b.y) && p.y <= a.y.max(b.y)
}

/// Intersección de segmentos incluyendo contacto en extremos y tramos colineales.
fn segments_meet(a: Point, b: Point, c: Point, d: Point) -> bool {
    let (d1, d2) = (cross(c, d, a), cross(c, d, b));
    let (d3, d4) = (cross(a, b, c), cross(a, b, d));
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }
    (d1 == 0.0 && on_segment(c, d, a))
        || (d2 == 0.0 && on_segment(c, d, b))
        || (d3 == 0.0 && on_segment(a, b, c))
        || (d4 == 0.0 && on_segment(a, b, d))
}

/// Punto dentro de un polígono (par-impar). Solo se usa cuando las aristas no se cortan.
fn point_in(p: Point, poly: &[Point]) -> bool {
    let mut inside = false;
    let mut prev = poly[poly.len() - 1];
    for &q in poly {
        if (q.y > p.y) != (prev.y > p.y)
            && p.x < (prev.x - q.x) * (p.y - q.y) / (prev.y - q.y) + q.x
        {
            inside = !inside;
        }
        prev = q;
    }
    inside
}

/// `true` si las dos formas se tocan, se cruzan o una contiene a la otra.
pub(crate) fn shapes_interact(a: &[Point], b: &[Point]) -> bool {
    if a.len().saturating_mul(b.len()) > MAX_EDGE_PAIRS {
        return true;
    }
    let mut pa = a[a.len() - 1];
    for &qa in a {
        let (ax0, ax1) = (pa.x.min(qa.x), pa.x.max(qa.x));
        let (ay0, ay1) = (pa.y.min(qa.y), pa.y.max(qa.y));
        let mut pb = b[b.len() - 1];
        for &qb in b {
            let separate = pb.x.min(qb.x) > ax1
                || pb.x.max(qb.x) < ax0
                || pb.y.min(qb.y) > ay1
                || pb.y.max(qb.y) < ay0;
            if !separate && segments_meet(pa, qa, pb, qb) {
                return true;
            }
            pb = qb;
        }
        pa = qa;
    }
    // Sin cortes, una está entera dentro de la otra o fuera: basta un vértice.
    point_in(a[0], b) || point_in(b[0], a)
}

/// Une los polígonos cuyas cajas se solapan y que además interactúan según `interact(i, j)`
/// (contornos con sus agujeros, formas superpuestas). Dos grupos así separados nunca se
/// tocan, de modo que rellenarlos por separado cubre lo mismo que juntos. Devuelve índices
/// dentro de `boxes`; los grupos salen ordenados por su primer miembro.
pub(crate) fn components(
    boxes: &[BBox],
    mut interact: impl FnMut(usize, usize) -> bool,
) -> Vec<Vec<usize>> {
    let n = boxes.len();
    let mut parent: Vec<usize> = (0..n).collect();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_unstable_by(|&a, &b| boxes[a].xmin.total_cmp(&boxes[b].xmin));

    let mut active: Vec<usize> = Vec::new();
    for &i in &order {
        let bi = boxes[i];
        active.retain(|&j| boxes[j].xmax >= bi.xmin);
        for &j in &active {
            let bj = boxes[j];
            if bj.ymin <= bi.ymax && bi.ymin <= bj.ymax {
                let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                if ri != rj && interact(i, j) {
                    parent[ri.max(rj)] = ri.min(rj);
                }
            }
        }
        active.push(i);
    }

    // La raíz es siempre el menor índice del grupo.
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut slot = vec![usize::MAX; n];
    for i in 0..n {
        let r = find(&mut parent, i);
        if slot[r] == usize::MAX {
            slot[r] = groups.len();
            groups.push(Vec::new());
        }
        groups[slot[r]].push(i);
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(xmin: f64, xmax: f64, ymin: f64, ymax: f64) -> BBox {
        BBox {
            xmin,
            xmax,
            ymin,
            ymax,
        }
    }

    #[test]
    fn groups_overlapping_and_nested_boxes() {
        let boxes = [
            b(0.0, 10.0, 0.0, 10.0),
            b(20.0, 30.0, 0.0, 10.0),
            b(2.0, 4.0, 2.0, 4.0),
            b(9.0, 12.0, 9.0, 12.0),
            b(0.0, 5.0, 50.0, 60.0),
        ];
        assert_eq!(
            components(&boxes, |_, _| true),
            vec![vec![0, 2, 3], vec![1], vec![4]]
        );
    }

    #[test]
    fn touching_boxes_join_and_empty_is_empty() {
        let boxes = [b(0.0, 1.0, 0.0, 1.0), b(1.0, 2.0, 1.0, 2.0)];
        assert_eq!(components(&boxes, |_, _| true), vec![vec![0, 1]]);
        assert!(components(&[], |_, _| true).is_empty());
    }
}

#[cfg(test)]
mod interact_tests {
    use super::*;

    fn sq(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ]
    }

    #[test]
    fn disjoint_shapes_with_overlapping_boxes_do_not_interact() {
        // Triángulos "A" y "V" encajados: las cajas se montan pero las formas no se tocan.
        let a = vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(0.0, 10.0),
        ];
        let b = vec![
            Point::new(11.0, 1.0),
            Point::new(11.0, 11.0),
            Point::new(1.0, 11.0),
        ];
        assert!(!shapes_interact(&a, &b));
    }

    #[test]
    fn crossing_touching_and_nested_interact() {
        assert!(shapes_interact(
            &sq(0.0, 0.0, 4.0, 4.0),
            &sq(2.0, 2.0, 6.0, 6.0)
        ));
        assert!(shapes_interact(
            &sq(0.0, 0.0, 4.0, 4.0),
            &sq(4.0, 0.0, 8.0, 4.0)
        ));
        assert!(shapes_interact(
            &sq(0.0, 0.0, 10.0, 10.0),
            &sq(3.0, 3.0, 5.0, 5.0)
        ));
        assert!(shapes_interact(
            &sq(3.0, 3.0, 5.0, 5.0),
            &sq(0.0, 0.0, 10.0, 10.0)
        ));
        assert!(!shapes_interact(
            &sq(0.0, 0.0, 1.0, 1.0),
            &sq(2.0, 2.0, 3.0, 3.0)
        ));
    }
}
