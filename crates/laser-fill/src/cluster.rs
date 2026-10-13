//! Agrupación de polígonos en componentes independientes para el modo `PerShape`.

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

/// Une los polígonos cuyas cajas se tocan o se solapan (contornos con sus agujeros, formas
/// superpuestas). Dos grupos con cajas disjuntas nunca comparten una línea de barrido, así
/// que rellenarlos por separado da el mismo resultado que juntos. Devuelve índices dentro de
/// `boxes`; los grupos salen ordenados por su primer miembro.
pub(crate) fn components(boxes: &[BBox]) -> Vec<Vec<usize>> {
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
                if ri != rj {
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
        assert_eq!(components(&boxes), vec![vec![0, 2, 3], vec![1], vec![4]]);
    }

    #[test]
    fn touching_boxes_join_and_empty_is_empty() {
        let boxes = [b(0.0, 1.0, 0.0, 1.0), b(1.0, 2.0, 1.0, 2.0)];
        assert_eq!(components(&boxes), vec![vec![0, 1]]);
        assert!(components(&[]).is_empty());
    }
}
