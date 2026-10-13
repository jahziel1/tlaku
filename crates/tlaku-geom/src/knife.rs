//! Cortar formas (Cut Shapes): una forma cerrada hace de cuchilla y parte otra en el trozo que
//! queda dentro de ella y el que queda fuera (F65 de `docs/spec/01-diseno-edicion.md`).
//!
//! Dos modos para una forma cerrada, según se vaya a cortar o a rellenar:
//! - [`cut_closed`]: las piezas son regiones cerradas (la línea de corte pasa a ser parte del
//!   borde de ambas). Es lo que se quiere con rellenos.
//! - [`cut_open`]: el contorno se parte en tramos abiertos, dentro y fuera de la cuchilla, sin
//!   añadir la línea de corte. Es lo que se quiere con capas de línea, donde no debe cortarse
//!   dos veces por el borde de la cuchilla. También sirve para trayectorias abiertas.
//!
//! La cuchilla es una región con la regla par-impar: sus trayectorias anidadas forman huecos.

use crate::boolean::{BoolOp, boolean};
use crate::trim::{cut_params, point_at, segment_count, sub_path};
use crate::util::{PointLocation, locate_point};
use tlaku_core::{Point, Polyline};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct KnifeCut {
    /// Lo que queda dentro de la cuchilla.
    pub inside: Vec<Polyline>,
    /// Lo que queda fuera.
    pub outside: Vec<Polyline>,
}

/// ¿El punto está dentro de la región de la cuchilla (regla par-impar)? El borde cuenta como dentro.
fn in_knife(p: Point, knife: &[Polyline]) -> bool {
    let mut depth = 0usize;
    for k in knife.iter().filter(|k| k.closed) {
        match locate_point(p, &k.points) {
            PointLocation::Boundary => return true,
            PointLocation::Inside => depth += 1,
            PointLocation::Outside => {}
        }
    }
    depth % 2 == 1
}

/// Parte las trayectorias cerradas de `target` con la cuchilla en regiones: dentro (intersección)
/// y fuera (diferencia). Las salidas siguen la convención del crate (exterior antihorario
/// seguido de sus agujeros). Las abiertas de `target` se ignoran: usa [`cut_open`] para ellas.
pub fn cut_closed(target: &[Polyline], knife: &[Polyline]) -> KnifeCut {
    KnifeCut {
        inside: boolean(target, knife, BoolOp::Intersection),
        outside: boolean(target, knife, BoolOp::Difference),
    }
}

/// Parte cada trayectoria de `target` (abierta o cerrada) donde cruza la cuchilla y reparte los
/// tramos abiertos según el punto medio de cada uno. Una cerrada que no cruza nada se reparte
/// entera, sin abrirla.
pub fn cut_open(target: &[Polyline], knife: &[Polyline]) -> KnifeCut {
    let knife_closed: Vec<Polyline> = knife.iter().filter(|k| k.closed).cloned().collect();
    let mut out = KnifeCut::default();
    for path in target {
        let segs = segment_count(path);
        if segs == 0 {
            continue;
        }
        let cuts = cut_params(path, None, &knife_closed);
        let n = segs as f64;
        let mut push = |piece: Polyline| {
            let mid = point_at(&piece, segment_count(&piece) as f64 / 2.0);
            // El punto medio por parámetro puede caer en un vértice; vale igual (no es borde de la cuchilla).
            if in_knife(mid, &knife_closed) {
                out.inside.push(piece);
            } else {
                out.outside.push(piece);
            }
        };
        if cuts.is_empty() {
            // Sin cortes todo el trazo está de un lado: se mira un punto que no sea vértice.
            if in_knife(point_at(path, 0.5), &knife_closed) {
                out.inside.push(path.clone());
            } else {
                out.outside.push(path.clone());
            }
            continue;
        }
        if path.closed {
            for k in 0..cuts.len() {
                let from = cuts[k];
                let to = if k + 1 < cuts.len() {
                    cuts[k + 1]
                } else {
                    cuts[0] + n
                };
                if let Some(piece) = sub_path(path, from, to - from) {
                    push(piece);
                }
            }
        } else {
            let mut bounds = vec![0.0];
            bounds.extend(cuts.iter().copied().filter(|&c| c > 1e-9 && c < n - 1e-9));
            bounds.push(n);
            for w in bounds.windows(2) {
                if let Some(piece) = sub_path(path, w[0], w[1] - w[0]) {
                    push(piece);
                }
            }
        }
    }
    out
}
