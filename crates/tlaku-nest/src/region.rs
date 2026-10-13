//! Regiones planas a partir de contornos cerrados: soldar con la regla par-impar de
//! anidamiento (un contorno dentro de otro es un hueco) y desplazar el borde (offset).

use crate::geom::{Pt, Ring, bounds, clean_ring, make_ccw, make_cw, signed_area};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::overlay::FloatOverlay;
use i_overlay::mesh::float::outline::offset::OutlineOffset;
use i_overlay::mesh::float::style::{LineJoin, OutlineStyle};
use std::f64::consts::PI;

/// Áreas (mm²) por debajo de esto se consideran cero.
const MIN_AREA: f64 = 1e-9;

fn point_in_ring(p: Pt, ring: &[Pt]) -> bool {
    let n = ring.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0])
        {
            inside = !inside;
        }
    }
    inside
}

type Shapes = Vec<Vec<Vec<[f64; 2]>>>;

fn to_rings(shapes: Shapes) -> Vec<Ring> {
    let mut out = Vec::new();
    for shape in shapes {
        for (i, c) in shape.into_iter().enumerate() {
            if c.len() < 3 || signed_area(&c).abs() < MIN_AREA {
                continue;
            }
            out.push(if i == 0 { make_ccw(c) } else { make_cw(c) });
        }
    }
    out
}

/// Une los contornos en una región: exteriores antihorarios, cada uno seguido de sus huecos
/// (horarios). Los contornos que se solapan sin contenerse se unen; los contenidos en otro
/// cuentan como hueco (regla par-impar), y uno dentro de un hueco es de nuevo material.
fn weld_shapes(rings: &[Ring]) -> Shapes {
    let clean: Vec<Ring> = rings
        .iter()
        .map(|r| clean_ring(r, 1e-9))
        .filter(|r| r.len() >= 3 && signed_area(r).abs() > MIN_AREA)
        .collect();
    if clean.is_empty() {
        return Vec::new();
    }
    let oriented: Vec<Ring> = clean
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let depth = clean
                .iter()
                .enumerate()
                .filter(|&(j, o)| j != i && point_in_ring(r[0], o))
                .count();
            if depth % 2 == 0 {
                make_ccw(r.clone())
            } else {
                make_cw(r.clone())
            }
        })
        .collect();
    FloatOverlay::<[f64; 2]>::with_subj(&oriented).overlay(OverlayRule::Subject, FillRule::NonZero)
}

/// Región (exteriores antihorarios y huecos horarios) de un conjunto de contornos cerrados.
pub fn weld(rings: &[Ring]) -> Vec<Ring> {
    to_rings(weld_shapes(rings))
}

/// Desplaza el borde de la región `d` mm (positivo = hacia afuera de la pieza; los huecos se
/// encogen). Las esquinas son en punta, con recorte a `miter_limit` veces la distancia: la
/// punta contiene al arco redondo (al crecer) y queda dentro de él (al encoger), que es el
/// lado conservador en ambos casos.
pub fn offset(rings: &[Ring], d: f64, miter_limit: f64) -> Vec<Ring> {
    let region = weld_shapes(rings);
    if region.is_empty() || !d.is_finite() {
        return Vec::new();
    }
    if d.abs() < 1e-9 {
        return to_rings(region);
    }
    if d < 0.0 {
        let outers: Vec<Ring> = region.iter().filter_map(|s| s.first().cloned()).collect();
        if let Some((lo, hi)) = bounds(&outers)
            && -d * 2.0 >= (hi[0] - lo[0]).min(hi[1] - lo[1])
        {
            return Vec::new();
        }
    }
    let limit = miter_limit.max(1.0);
    let style =
        OutlineStyle::new(d).line_join(LineJoin::Miter(2.0 * (1.0 / limit).asin().clamp(1e-3, PI)));
    to_rings(region.outline(&style))
}
