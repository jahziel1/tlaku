//! Cajas con uniones de dientes (finger joints).
//!
//! # Diseño
//!
//! La caja se describe con sus medidas **exteriores** `W x D x H` (ancho en X, fondo en Y, alto en
//! Z) y el grosor `T`. Cada panel es un rectángulo del tamaño exterior de su cara al que se le
//! restan muescas de profundidad `T` en los bordes que se unen con otro panel; no hay dientes que
//! "sobresalgan" del rectángulo. Así las medidas de la caja salen exactas por construcción.
//!
//! Las 12 aristas se reparten con una regla fija que evita conflictos en los 8 vértices (el cubo
//! de `T x T x T` de cada vértice pertenece al panel frontal o al trasero):
//!
//! - frontal/trasero con superior/inferior (aristas en X) y con izquierdo/derecho (aristas en Z):
//!   el frontal o el trasero posee las celdas pares, incluidas las dos de los extremos;
//! - izquierdo/derecho con superior/inferior (aristas en Y): la arista se acorta `T` en cada
//!   extremo (esos cubos son del frontal y del trasero) y el lateral posee las celdas pares.
//!
//! Los dos paneles de una arista usan **el mismo reparto de celdas** ([`Cells`]); en cada celda uno
//! tiene diente y el otro muesca. La holgura de encaje ensancha las muescas (la mitad por cada
//! pieza), de modo que ranura menos diente es `fit_mm`. La compensación del kerf no se hace aquí.

mod dividers;
mod feet;
mod panels;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_dividers;

use crate::error::PartsError;
use crate::fingers::MIN_CELLS;
use crate::font;
use crate::part::{Generated, Part};
use crate::registry::ParamSpec;
use panels::{Id, Panel, build_panel};
use serde::{Deserialize, Serialize};
use tlaku_i18n::tr;

pub use dividers::DividerJoint;
pub use feet::Feet;

/// Si las medidas pedidas son exteriores o interiores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dims {
    Outer,
    Inner,
}

/// Tipo de caja.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoxKind {
    /// Seis paneles unidos con dientes.
    Closed,
    /// Sin tapa: cinco paneles.
    Open,
    /// Tapa que se desliza por ranuras de los laterales; el trasero es más bajo.
    SlidingLid,
}

/// Parámetros de la caja. Los campos que falten en un guardado viejo toman el valor de fábrica.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BoxParams {
    pub width_mm: f64,
    pub depth_mm: f64,
    pub height_mm: f64,
    pub dims: Dims,
    pub kind: BoxKind,
    pub thickness_mm: f64,
    /// Ancho de diente pedido; el real se ajusta para que la arista tenga un número impar de
    /// celdas iguales.
    pub finger_mm: f64,
    /// Ranura menos diente, en mm (positivo = más flojo, negativo = más apretado).
    pub fit_mm: f64,
    /// Juego de la tapa deslizante en su ranura.
    pub lid_clearance_mm: f64,
    /// Grabar el nombre de cada panel en su cara exterior.
    pub labels: bool,
    /// Divisores perpendiculares a X (reparten el ancho en `n + 1` compartimentos).
    pub dividers_x: i32,
    /// Divisores perpendiculares a Y (reparten el fondo).
    pub dividers_y: i32,
    /// Anchos de los primeros compartimentos en X, separados por comas; los que falten se reparten
    /// por igual. Vacío = todos iguales.
    pub divider_widths_x: String,
    /// Lo mismo para Y.
    pub divider_widths_y: String,
    pub divider_joint: DividerJoint,
    /// Juego de los divisores sueltos (solo `divider_joint = loose`): se acortan esto en largo y alto.
    pub divider_clearance_mm: f64,
    pub feet: Feet,
    /// Altura del zócalo o de las patas (se redondea a capas del grosor del material).
    pub foot_height_mm: f64,
}

impl Default for BoxParams {
    fn default() -> Self {
        Self {
            width_mm: 100.0,
            depth_mm: 80.0,
            height_mm: 60.0,
            dims: Dims::Outer,
            kind: BoxKind::Closed,
            thickness_mm: 3.0,
            finger_mm: 10.0,
            fit_mm: 0.0,
            lid_clearance_mm: 0.2,
            labels: false,
            dividers_x: 0,
            dividers_y: 0,
            divider_widths_x: String::new(),
            divider_widths_y: String::new(),
            divider_joint: DividerJoint::Tabs,
            divider_clearance_mm: 0.1,
            feet: Feet::None,
            foot_height_mm: 12.0,
        }
    }
}

/// Medidas exteriores ya resueltas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Sizes {
    pub w: f64,
    pub d: f64,
    pub h: f64,
    /// Alto del panel trasero (menor que `h` solo con tapa deslizante).
    pub back_h: f64,
    /// Alto de la ranura de la tapa (0 si no hay).
    pub slot_h: f64,
}

/// Tope de dientes por arista (la interfaz recalcula en vivo mientras se teclea).
const MAX_CELLS_PER_EDGE: f64 = 150.0;

/// Mínimo de ancho de celda (mm) por debajo del cual el diente no se puede cortar.
const MIN_CELL_MM: f64 = 1.5;

fn out_of(field: &str, v: f64, lo: f64, hi: f64) -> Option<PartsError> {
    (!(lo..=hi).contains(&v) || v.is_nan())
        .then(|| PartsError::field(field, tr!("parts.box.range", min = lo, max = hi)))
}

impl BoxParams {
    /// Valida y resuelve las medidas exteriores.
    pub(crate) fn sizes(&self) -> Result<Sizes, PartsError> {
        let t = self.thickness_mm;
        if let Some(e) = [
            out_of("thickness_mm", t, 0.5, 20.0),
            out_of("finger_mm", self.finger_mm, 1.5, 200.0),
            out_of("fit_mm", self.fit_mm, -0.5, 1.0),
            out_of("lid_clearance_mm", self.lid_clearance_mm, 0.0, 2.0),
            out_of("width_mm", self.width_mm, 1.0, 5000.0),
            out_of("depth_mm", self.depth_mm, 1.0, 5000.0),
            out_of("height_mm", self.height_mm, 1.0, 5000.0),
            out_of("dividers_x", f64::from(self.dividers_x), 0.0, 10.0),
            out_of("dividers_y", f64::from(self.dividers_y), 0.0, 10.0),
            out_of("divider_clearance_mm", self.divider_clearance_mm, -0.5, 1.0),
            out_of("foot_height_mm", self.foot_height_mm, 1.0, 60.0),
        ]
        .into_iter()
        .flatten()
        .next()
        {
            return Err(e);
        }
        let sliding = self.kind == BoxKind::SlidingLid;
        let slot_h = if sliding {
            t + self.lid_clearance_mm
        } else {
            0.0
        };
        let rim = if sliding { 2.0 * t } else { 0.0 };
        let (w, d, h) = match self.dims {
            Dims::Outer => (self.width_mm, self.depth_mm, self.height_mm),
            Dims::Inner => (
                self.width_mm + 2.0 * t,
                self.depth_mm + 2.0 * t,
                match self.kind {
                    BoxKind::Closed => self.height_mm + 2.0 * t,
                    BoxKind::Open => self.height_mm + t,
                    BoxKind::SlidingLid => self.height_mm + t + slot_h + rim,
                },
            ),
        };
        let back_h = h - slot_h - rim;
        // Cada arista necesita tres celdas de al menos `MIN_CELL_MM`, y los dientes de lados
        // opuestos de un panel no pueden cruzarse.
        let min_dim = 2.0 * t + MIN_CELLS as f64 * MIN_CELL_MM;
        for (field, v) in [("width_mm", w), ("depth_mm", d), ("height_mm", back_h)] {
            if v < min_dim {
                return Err(PartsError::field(
                    field,
                    tr!("parts.box.too_small", min = format!("{min_dim:.1}")),
                ));
            }
        }
        // Un reparto absurdo (miles de dientes) solo haría lenta la vista previa.
        if w.max(d).max(h) / self.finger_mm > MAX_CELLS_PER_EDGE {
            return Err(PartsError::field(
                "finger_mm",
                tr!("parts.box.too_many_fingers", max = MAX_CELLS_PER_EDGE),
            ));
        }
        Ok(Sizes {
            w,
            d,
            h,
            back_h,
            slot_h,
        })
    }
}

/// Qué paneles lleva cada tipo de caja.
fn ids(kind: BoxKind) -> &'static [Id] {
    match kind {
        BoxKind::Closed => &[
            Id::Front,
            Id::Back,
            Id::Left,
            Id::Right,
            Id::Bottom,
            Id::Top,
        ],
        BoxKind::Open => &[Id::Front, Id::Back, Id::Left, Id::Right, Id::Bottom],
        BoxKind::SlidingLid => &[
            Id::Front,
            Id::Back,
            Id::Left,
            Id::Right,
            Id::Bottom,
            Id::Lid,
        ],
    }
}

/// Todo lo que sale de la caja: medidas, paneles con su geometría 3D, divisores y avisos.
pub(crate) struct Built {
    pub sizes: Sizes,
    pub panels: Vec<Panel>,
    pub dividers: Vec<dividers::Divider>,
    pub warnings: Vec<String>,
}

pub(crate) fn build_all(p: &BoxParams) -> Result<Built, PartsError> {
    let sizes = p.sizes()?;
    let list: Vec<Id> = ids(p.kind).to_vec();
    let mut warnings = Vec::new();
    let mut panels: Vec<Panel> = list.iter().map(|&id| Panel::new(id, p, &sizes)).collect();
    let snapshot: Vec<Panel> = panels.clone();
    for panel in &mut panels {
        build_panel(panel, &snapshot, p, &mut warnings)?;
    }
    let dividers = dividers::build_dividers(p, &sizes, &mut panels, &mut warnings)?;
    if p.labels {
        for panel in &mut panels {
            label_panel(panel, p, &mut warnings);
        }
    }
    warnings.dedup();
    Ok(Built {
        sizes,
        panels,
        dividers,
        warnings,
    })
}

/// Paneles de la caja con su geometría 3D y sus contornos (sin el origen normalizado).
pub(crate) fn build_panels(p: &BoxParams) -> Result<(Sizes, Vec<Panel>, Vec<String>), PartsError> {
    let b = build_all(p)?;
    Ok((b.sizes, b.panels, b.warnings))
}

/// Rótulo grabado en el centro de la cara exterior, si cabe entre los dientes y la ranura.
fn label_panel(panel: &mut Panel, p: &BoxParams, warnings: &mut Vec<String>) {
    let text = match panel.id {
        Id::Front => tr!("parts.box.label.front"),
        Id::Back => tr!("parts.box.label.back"),
        Id::Left => tr!("parts.box.label.left"),
        Id::Right => tr!("parts.box.label.right"),
        Id::Bottom => tr!("parts.box.label.bottom"),
        Id::Top => tr!("parts.box.label.top"),
        Id::Lid => tr!("parts.box.label.lid"),
    };
    let t = p.thickness_mm;
    let (w, h) = (panel.w, panel.h);
    let free_w = w - 2.0 * (t + 1.0);
    let free_h = h - 2.0 * (t + 1.0);
    let mut height = (w.min(h) * 0.1).clamp(2.5, 6.0).min(free_h);
    while height > 2.0 && font::text_width(&text, height) > free_w {
        height -= 0.5;
    }
    let (cx, cy) = (w / 2.0, h / 2.0);
    let (tw, th) = (font::text_width(&text, height), height);
    let area = crate::rectilinear::Rect::new(
        cx - tw / 2.0 - 1.0,
        cy - th / 2.0 - 1.0,
        cx + tw / 2.0 + 1.0,
        cy + th / 2.0 + 1.0,
    );
    let clear = height >= 2.0
        && tw <= free_w
        && !panel.cut_rects.iter().any(|r| r.overlaps(&area))
        && !panel.voids.iter().any(|r| r.overlaps(&area))
        && !panel.slots.iter().any(|r| r.overlaps(&area));
    if clear {
        panel.engraves = font::text_centered(&text, cx, cy, height);
    } else {
        warnings.push(tr!("parts.box.label_skipped", name = panel.name()));
    }
}

/// Genera las piezas de la caja.
pub fn box_parts(p: &BoxParams) -> Result<Generated, PartsError> {
    let Built {
        sizes,
        panels,
        dividers,
        mut warnings,
    } = build_all(p)?;
    if p.fit_mm != 0.0 {
        warnings.push(tr!("parts.box.fit_vs_kerf"));
    }
    // Un diente más estrecho que el material se parte con facilidad.
    let narrowest = panels::narrowest_cell(&panels, p);
    if let Some(c) = narrowest
        && c < p.thickness_mm
    {
        warnings.push(tr!(
            "parts.box.thin_fingers",
            width = format!("{c:.1}"),
            thickness = format!("{:.1}", p.thickness_mm)
        ));
    }
    if p.kind == BoxKind::SlidingLid {
        warnings.push(tr!(
            "parts.box.sliding_note",
            back = format!("{:.1}", sizes.back_h)
        ));
    }
    let mut parts: Vec<Part> = panels
        .into_iter()
        .map(|panel| {
            let name = panel.name();
            Part::new(panel.key(), name, panel.cuts, panel.engraves)
        })
        .collect();
    parts.extend(dividers::divider_parts(dividers));
    let (feet, feet_warnings) = feet::feet_parts(p, &sizes)?;
    parts.extend(feet);
    warnings.extend(feet_warnings);
    Ok(Generated { parts, warnings })
}

/// Campos del formulario de la caja.
pub fn schema() -> Vec<ParamSpec> {
    let d = BoxParams::default();
    vec![
        ParamSpec::choice("kind", "box", &["closed", "open", "sliding_lid"], "closed"),
        ParamSpec::choice("dims", "size", &["outer", "inner"], "outer"),
        ParamSpec::length("width_mm", "size", 1.0, 5000.0, 1.0, d.width_mm),
        ParamSpec::length("depth_mm", "size", 1.0, 5000.0, 1.0, d.depth_mm),
        ParamSpec::length("height_mm", "size", 1.0, 5000.0, 1.0, d.height_mm),
        ParamSpec::length("thickness_mm", "size", 0.5, 20.0, 0.1, d.thickness_mm),
        ParamSpec::length("finger_mm", "joints", 1.5, 200.0, 0.5, d.finger_mm),
        ParamSpec::length("fit_mm", "joints", -0.5, 1.0, 0.01, d.fit_mm),
        ParamSpec::length(
            "lid_clearance_mm",
            "joints",
            0.0,
            2.0,
            0.05,
            d.lid_clearance_mm,
        )
        .when("kind", &["sliding_lid"]),
        ParamSpec::boolean("labels", "options", d.labels),
        ParamSpec::int("dividers_x", "dividers", 0, 10, d.dividers_x),
        ParamSpec::int("dividers_y", "dividers", 0, 10, d.dividers_y),
        ParamSpec::text("divider_widths_x", "dividers", &d.divider_widths_x),
        ParamSpec::text("divider_widths_y", "dividers", &d.divider_widths_y),
        ParamSpec::choice("divider_joint", "dividers", &["tabs", "loose"], "tabs"),
        ParamSpec::length(
            "divider_clearance_mm",
            "dividers",
            -0.5,
            1.0,
            0.05,
            d.divider_clearance_mm,
        )
        .when("divider_joint", &["loose"]),
        ParamSpec::choice("feet", "feet", &["none", "plinth", "legs"], "none"),
        ParamSpec::length("foot_height_mm", "feet", 1.0, 60.0, 1.0, d.foot_height_mm)
            .when("feet", &["plinth", "legs"]),
    ]
}
