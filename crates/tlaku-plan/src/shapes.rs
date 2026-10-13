//! Estado de entrada/salida de cada figura durante la planificación.

use tlaku_core::Point;

use crate::{PlanItem, PlanOptions};

/// Orientación elegida para cada figura. En las abiertas `a`/`b` son los extremos originales
/// y `flip` indica si se recorre al revés; en las cerradas `a == b` es el vértice de inicio.
pub(crate) struct Shapes {
    pub a: Vec<Point>,
    pub b: Vec<Point>,
    pub flip: Vec<bool>,
    pub vertex: Vec<usize>,
}

impl Shapes {
    /// Las figuras inválidas (sin puntos) quedan en el origen y no se usan.
    pub fn new(items: &[PlanItem]) -> Shapes {
        let n = items.len();
        let mut s = Shapes {
            a: vec![Point::new(0.0, 0.0); n],
            b: vec![Point::new(0.0, 0.0); n],
            flip: vec![false; n],
            vertex: vec![0; n],
        };
        for (i, it) in items.iter().enumerate() {
            let pts = &it.path.points;
            if pts.is_empty() {
                continue;
            }
            s.a[i] = pts[0];
            s.b[i] = if it.path.closed {
                pts[0]
            } else {
                pts[pts.len() - 1]
            };
        }
        s
    }

    pub fn entry(&self, i: usize) -> Point {
        if self.flip[i] { self.b[i] } else { self.a[i] }
    }

    pub fn exit(&self, i: usize) -> Point {
        if self.flip[i] { self.a[i] } else { self.b[i] }
    }

    /// Aplica el punto de entrada `pt` devuelto por [`push_entries`].
    pub fn apply_entry(&mut self, items: &[PlanItem], i: usize, pt: usize) {
        if items[i].path.closed {
            self.vertex[i] = pt;
            self.a[i] = items[i].path.points[pt];
            self.b[i] = self.a[i];
        } else {
            self.flip[i] = pt == 1;
        }
    }
}

/// Puntos por los que se puede empezar la figura `i`: (elemento, índice de punto, coordenada).
/// Cerradas: todos los vértices (o solo el primero); abiertas: los dos extremos (o solo el
/// inicio) con índice 0 = inicio, 1 = final. Una figura fijada por el usuario (`locked`) solo
/// ofrece la entrada que ya trae.
pub(crate) fn push_entries(
    items: &[PlanItem],
    opts: &PlanOptions,
    i: usize,
    out: &mut Vec<(usize, usize, Point)>,
) {
    let path = &items[i].path;
    if path.closed {
        if opts.choose_start_point && !items[i].locked {
            out.extend(path.points.iter().enumerate().map(|(v, &p)| (i, v, p)));
        } else {
            out.push((i, 0, path.points[0]));
        }
    } else {
        out.push((i, 0, path.points[0]));
        if opts.choose_direction && !items[i].locked {
            out.push((i, 1, path.points[path.points.len() - 1]));
        }
    }
}
