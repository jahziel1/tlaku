//! Zócalo o patas para elevar la caja.
//!
//! Se hacen apilando capas planas del grosor del material (`capas = altura / grosor`, redondeado)
//! que se pegan entre sí y debajo de la base: no llevan uniones, así que no cambian los paneles de
//! la caja. El zócalo es un marco rectangular retranqueado un grosor del borde de la base; las
//! patas son cuatro cuadrados por capa, que van en las esquinas.

use super::{BoxParams, Sizes};
use crate::error::PartsError;
use crate::part::Part;
use crate::rectilinear::{Rect, rect_minus_rects};
use serde::{Deserialize, Serialize};
use tlaku_core::{Point, Polyline};
use tlaku_i18n::tr;

/// Qué eleva la caja.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feet {
    None,
    /// Marco rectangular bajo la base.
    Plinth,
    /// Cuatro patas en las esquinas.
    Legs,
}

/// Capas máximas (más de eso es una torre difícil de pegar derecha).
pub(crate) const MAX_LAYERS: usize = 10;

/// Separación entre las cuatro patas de una capa.
const LEG_GAP_MM: f64 = 5.0;

/// Número de capas que da la altura pedida.
pub(crate) fn layers(p: &BoxParams) -> usize {
    ((p.foot_height_mm / p.thickness_mm).round() as usize).max(1)
}

/// Piezas del zócalo o las patas (vacío si no hay) y sus avisos.
pub(crate) fn feet_parts(
    p: &BoxParams,
    sizes: &Sizes,
) -> Result<(Vec<Part>, Vec<String>), PartsError> {
    if p.feet == Feet::None {
        return Ok((Vec::new(), Vec::new()));
    }
    let t = p.thickness_mm;
    let n = layers(p);
    if n > MAX_LAYERS {
        return Err(PartsError::field(
            "foot_height_mm",
            tr!("parts.box.feet.too_tall", max = MAX_LAYERS),
        ));
    }
    // Huella del zócalo: la base retranqueada un grosor por lado.
    let (w, d) = (sizes.w - 2.0 * t, sizes.d - 2.0 * t);
    if w.min(d) < 6.0 * t.min(2.0) {
        return Err(PartsError::field(
            "foot_height_mm",
            tr!("parts.box.feet.too_small"),
        ));
    }
    let rim = (4.0 * t).min(w / 3.0).min(d / 3.0);
    let mut parts = Vec::new();
    for layer in 1..=n {
        let part = match p.feet {
            Feet::Plinth => {
                let cuts = rect_minus_rects(w, d, &[Rect::new(rim, rim, w - rim, d - rim)]);
                Part::new(
                    &format!("plinth{layer}"),
                    tr!("parts.box.feet.plinth", n = layer),
                    cuts,
                    Vec::new(),
                )
            }
            _ => {
                let mut cuts: Vec<Polyline> = Vec::new();
                for (i, j) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                    let (dx, dy) = (i * (rim + LEG_GAP_MM), j * (rim + LEG_GAP_MM));
                    for mut sq in rect_minus_rects(rim, rim, &[]) {
                        for q in &mut sq.points {
                            *q = Point::new(q.x + dx, q.y + dy);
                        }
                        cuts.push(sq);
                    }
                }
                Part::new(
                    &format!("legs{layer}"),
                    tr!("parts.box.feet.legs", n = layer),
                    cuts,
                    Vec::new(),
                )
            }
        };
        parts.push(part);
    }
    let height = format!("{:.1}", n as f64 * t);
    let warnings = vec![if p.feet == Feet::Plinth {
        tr!("parts.box.feet.note_plinth", layers = n, height = height, inset = format!("{t:.1}"))
    } else {
        tr!("parts.box.feet.note_legs", layers = n, height = height)
    }];
    Ok((parts, warnings))
}
