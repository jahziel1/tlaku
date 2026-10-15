//! Divisores de la caja: paredes interiores con pestañas que entran en ranuras de las paredes y
//! de la base (o sueltos), y ranuras de media madera donde se cruzan.
//!
//! # Diseño
//!
//! `dividers_x` divisores son láminas perpendiculares a X (corren a lo largo de Y) y `dividers_y`
//! lo son a Y (corren a lo largo de X). Reparten la cavidad `[T, W-T] x [T, D-T]` en compartimentos
//! iguales o de los anchos pedidos; cada divisor ocupa un grosor `T` entre dos compartimentos.
//!
//! Cada divisor se dibuja en coordenadas de cuerpo `(s, z)`: `s` va a lo largo de su recorrido
//! desde la cara interior de la primera pared y `z` desde la cara interior de la base. El cuerpo
//! es `[0, len] x [0, alto]` y las pestañas sobresalen `T` por los cuatro bordes: de un grosor
//! de profundidad, justo lo que mide la pared o la base que las recibe, así que quedan a ras.
//!
//! - **Pestañas.** Cada borde se divide con el reparto de dientes de la caja ([`Cells`]): las
//!   celdas impares llevan pestaña y las pares (las de los extremos) quedan lisas, de modo que
//!   entre una ranura y el borde de la pieza que la recibe siempre queda una celda de material.
//! - **Cruces.** Donde un divisor de X cruza uno de Y, el de X pierde la mitad superior y el de Y
//!   la inferior (ranuras de ancho `T + holgura`, profundidad exacta de media altura): las dos
//!   mitades se complementan. En los cruces no hay pestañas.
//! - **Holgura.** Una ranura mide la pestaña (o el divisor) más `fit_mm`, repartida a ambos lados.
//! - **Sueltos.** Con `divider_joint = loose` no hay pestañas ni ranuras en las paredes; el
//!   divisor se acorta `divider_clearance_mm` en largo y en alto.

use super::panels::{Id, Panel, V3};
use super::{BoxKind, BoxParams, MIN_CELL_MM, Sizes};
use crate::error::PartsError;
use crate::fingers::{Cells, MIN_CELLS};
use crate::part::Part;
use crate::rectilinear::{Rect, rect_minus_rects};
use serde::{Deserialize, Serialize};
use tlaku_core::{Point, Polyline};
use tlaku_i18n::tr;

/// Cómo se sujetan los divisores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DividerJoint {
    /// Pestañas que entran en ranuras de las paredes y de la base (y de la tapa si la hay).
    Tabs,
    /// Sin pestañas: láminas que se meten entre las paredes con un pequeño juego.
    Loose,
}

/// Dirección en que se reparten: `X` = láminas perpendiculares al eje X.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Axis {
    X,
    Y,
}

impl Axis {
    /// Eje (0 = X, 1 = Y) a lo largo del cual corre el cuerpo del divisor.
    pub fn run(self) -> usize {
        match self {
            Axis::X => 1,
            Axis::Y => 0,
        }
    }

    /// Eje del grosor.
    pub fn thick(self) -> usize {
        1 - self.run()
    }
}

/// Un divisor ya dibujado.
#[derive(Debug, Clone)]
pub(crate) struct Divider {
    pub axis: Axis,
    /// Número del divisor dentro de su eje (desde 1).
    pub index: usize,
    /// Coordenada donde empieza su grosor (X para `Axis::X`, Y para `Axis::Y`).
    pub pos: f64,
    /// Punto 3D del origen de las coordenadas de cuerpo `(0, 0)` (lo usan las pruebas 3D).
    #[cfg_attr(not(test), allow(dead_code))]
    pub o: V3,
    /// Contorno en coordenadas de cuerpo (pestañas fuera de `[0, len] x [0, alto]`).
    pub cuts: Vec<Polyline>,
    /// Intervalos `z` de las pestañas de los extremos (iguales en los dos).
    side_tabs: Vec<(f64, f64)>,
    /// Intervalos `s` de las pestañas de la base (y de la tapa).
    edge_tabs: Vec<(f64, f64)>,
    has_top: bool,
}

impl Divider {
    pub fn key(&self) -> String {
        match self.axis {
            Axis::X => format!("divider_x{}", self.index),
            Axis::Y => format!("divider_y{}", self.index),
        }
    }

    pub fn name(&self) -> String {
        match self.axis {
            Axis::X => tr!("parts.box.div.name_x", n = self.index),
            Axis::Y => tr!("parts.box.div.name_y", n = self.index),
        }
    }

    /// Ancho más angosto entre sus pestañas.
    fn narrowest_tab(&self) -> Option<f64> {
        self.side_tabs
            .iter()
            .chain(self.edge_tabs.iter())
            .map(|(a, b)| b - a)
            .min_by(f64::total_cmp)
    }

    /// Ranuras que este divisor pide a las paredes: panel y caja 3D `(min, max)`.
    fn wall_slots(&self, p: &BoxParams, s: &Sizes) -> Vec<(Id, V3, V3)> {
        let (t, half) = (p.thickness_mm, p.fit_mm / 2.0);
        let mut out = Vec::new();
        let (thick, run) = (self.axis.thick(), self.axis.run());
        let (t0, t1) = (self.pos - half, self.pos + t + half);
        // Caja 3D con el grosor del divisor en `thick`, el recorrido en `run` y `z`.
        let make = |r: (f64, f64), z: (f64, f64)| -> (V3, V3) {
            let (mut lo, mut hi) = ([0.0; 3], [0.0; 3]);
            lo[thick] = t0;
            hi[thick] = t1;
            lo[run] = r.0;
            hi[run] = r.1;
            lo[2] = z.0;
            hi[2] = z.1;
            (lo, hi)
        };
        let (run_len, walls) = match self.axis {
            Axis::X => (s.d, (Id::Front, Id::Back)),
            Axis::Y => (s.w, (Id::Left, Id::Right)),
        };
        for &(a, b) in &self.side_tabs {
            let z = (t + a - half, t + b + half);
            let (lo, hi) = make((0.0, t), z);
            out.push((walls.0, lo, hi));
            let (lo, hi) = make((run_len - t, run_len), z);
            out.push((walls.1, lo, hi));
        }
        for &(a, b) in &self.edge_tabs {
            let r = (t + a - half, t + b + half);
            let (lo, hi) = make(r, (0.0, t));
            out.push((Id::Bottom, lo, hi));
            if self.has_top {
                let (lo, hi) = make(r, (s.h - t, s.h));
                out.push((Id::Top, lo, hi));
            }
        }
        out
    }
}

/// Huecos de `[lo, hi]` que no cubren los intervalos `tabs` (ordenados).
fn gaps(lo: f64, hi: f64, tabs: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let mut at = lo;
    for &(a, b) in tabs {
        if a > at {
            out.push((at, a));
        }
        at = at.max(b);
    }
    if hi > at {
        out.push((at, hi));
    }
    out
}

/// Celdas impares del reparto de dientes de `[a, b]` (vacío si no caben tres celdas).
fn odd_cells(a: f64, b: f64, finger: f64) -> Vec<(f64, f64)> {
    if b - a < MIN_CELLS as f64 * MIN_CELL_MM {
        return Vec::new();
    }
    let cells = Cells::plan(a, b - a, finger);
    (0..cells.n)
        .filter(|k| k % 2 == 1)
        .map(|k| cells.cell(k))
        .collect()
}

/// Anchos de los `n + 1` compartimentos de una longitud libre `len` con `n` divisores de grosor `t`.
///
/// `text` lista los anchos de los primeros compartimentos (separados por coma, punto y coma o
/// espacio); los que falten se reparten por igual lo que sobre. Vacío = todos iguales. Si se dan
/// todos, deben sumar lo que mide la cavidad menos los divisores.
fn compartments(
    len: f64,
    n: usize,
    t: f64,
    text: &str,
    field: &str,
) -> Result<Vec<f64>, PartsError> {
    let total = len - n as f64 * t;
    let count = n + 1;
    let min = (2.0 * t).max(3.0);
    let mut widths = Vec::new();
    for tok in text
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .filter(|s| !s.is_empty())
    {
        match tok.parse::<f64>() {
            Ok(v) if v.is_finite() => widths.push(v),
            _ => {
                return Err(PartsError::field(
                    field,
                    tr!("parts.box.div.bad_width", value = tok),
                ));
            }
        }
    }
    if widths.len() > count {
        return Err(PartsError::field(
            field,
            tr!("parts.box.div.too_many_widths", n = count),
        ));
    }
    let sum: f64 = widths.iter().sum();
    let rest = count - widths.len();
    if rest == 0 {
        if (sum - total).abs() > 0.05 {
            return Err(PartsError::field(
                field,
                tr!(
                    "parts.box.div.sum",
                    sum = format!("{sum:.2}"),
                    total = format!("{total:.2}")
                ),
            ));
        }
        // La diferencia (menos de 0,05 mm) se la lleva el último para que todo sume exacto.
        if let Some(last) = widths.last_mut() {
            *last += total - sum;
        }
    } else {
        let each = (total - sum) / rest as f64;
        widths.extend(std::iter::repeat_n(each, rest));
    }
    if let Some(&w) = widths.iter().min_by(|a, b| a.total_cmp(b))
        && w < min
    {
        return Err(PartsError::field(
            field,
            tr!(
                "parts.box.div.narrow",
                width = format!("{w:.1}"),
                min = format!("{min:.1}")
            ),
        ));
    }
    Ok(widths)
}

/// Dónde empieza el grosor de cada divisor a partir de los anchos de compartimento.
fn positions(first: f64, widths: &[f64], t: f64) -> Vec<f64> {
    let mut out = Vec::new();
    let mut at = first;
    for w in &widths[..widths.len() - 1] {
        at += w;
        out.push(at);
        at += t;
    }
    out
}

/// Medidas de la cavidad que recorre un divisor.
struct Cavity {
    /// Largo a lo largo del recorrido del divisor.
    len: f64,
    height: f64,
}

/// Arma un divisor. `cross` son las posiciones `s` (desde el inicio de la cavidad) donde otros
/// divisores lo cruzan.
fn build(
    axis: Axis,
    index: usize,
    pos: f64,
    cav: &Cavity,
    cross: &[f64],
    p: &BoxParams,
) -> Divider {
    let (t, fit) = (p.thickness_mm, p.fit_mm);
    let (len, height) = (cav.len, cav.height);
    let loose = p.divider_joint == DividerJoint::Loose;
    let has_top = !loose && p.kind == BoxKind::Closed;
    let o = match axis {
        Axis::X => [pos, t, t],
        Axis::Y => [t, pos, t],
    };
    // Ranuras de media madera: de arriba hacia abajo en X, de abajo hacia arriba en Y.
    let mid = height / 2.0;
    let lap: Vec<(f64, f64)> = cross
        .iter()
        .map(|&c| (c - fit / 2.0, c + t + fit / 2.0))
        .collect();
    let (side_tabs, edge_tabs) = if loose {
        (Vec::new(), Vec::new())
    } else {
        let side = odd_cells(0.0, height, p.finger_mm);
        let edge = gaps(0.0, len, &lap)
            .into_iter()
            .flat_map(|(a, b)| odd_cells(a, b, p.finger_mm))
            .collect();
        (side, edge)
    };
    // Cuerpo: completo con pestañas; acortado y sin pestañas si es suelto.
    let c = if loose {
        p.divider_clearance_mm / 2.0
    } else {
        0.0
    };
    let (b0, b1) = (c, len - c);
    let z1 = if loose { height - 2.0 * c } else { height };
    // Todo lo que está dentro de `[-t, len+t] x [-t, alto+t]` y no es cuerpo ni pestaña es hueco.
    let mut holes: Vec<Rect> = Vec::new();
    let mut add = |s0: f64, z0: f64, s1: f64, z1: f64| {
        if s1 > s0 && z1 > z0 {
            holes.push(Rect::new(s0 + t, z0 + t, s1 + t, z1 + t));
        }
    };
    for (a, b) in gaps(-t, height + t, &side_tabs) {
        add(-t, a, b0, b);
        add(b1, a, len + t, b);
    }
    for (a, b) in gaps(b0, b1, &edge_tabs) {
        add(a, -t, b, 0.0);
    }
    let top_tabs: &[(f64, f64)] = if has_top { &edge_tabs } else { &[] };
    for (a, b) in gaps(b0, b1, top_tabs) {
        add(a, z1, b, height + t);
    }
    for &(a, b) in &lap {
        match axis {
            Axis::X => add(a, mid, b, height + t),
            Axis::Y => add(a, -t, b, mid),
        }
    }
    let mut cuts = rect_minus_rects(len + 2.0 * t, height + 2.0 * t, &holes);
    for poly in &mut cuts {
        for q in &mut poly.points {
            *q = Point::new(q.x - t, q.y - t);
        }
    }
    Divider {
        axis,
        index,
        pos,
        o,
        cuts,
        side_tabs,
        edge_tabs,
        has_top,
    }
}

/// Divisores de la caja, listos para cortar; además abre en los paneles las ranuras de sus
/// pestañas.
pub(crate) fn build_dividers(
    p: &BoxParams,
    sizes: &Sizes,
    panels: &mut [Panel],
    warnings: &mut Vec<String>,
) -> Result<Vec<Divider>, PartsError> {
    let (nx, ny) = (p.dividers_x.max(0) as usize, p.dividers_y.max(0) as usize);
    if nx + ny == 0 {
        return Ok(Vec::new());
    }
    let t = p.thickness_mm;
    let top = match p.kind {
        BoxKind::Closed => sizes.h - t,
        BoxKind::Open => sizes.h,
        BoxKind::SlidingLid => sizes.back_h,
    };
    let height = top - t;
    let (lx, ly) = (sizes.w - 2.0 * t, sizes.d - 2.0 * t);
    let xs = positions(
        t,
        &compartments(lx, nx, t, &p.divider_widths_x, "divider_widths_x")?,
        t,
    );
    let ys = positions(
        t,
        &compartments(ly, ny, t, &p.divider_widths_y, "divider_widths_y")?,
        t,
    );
    // `s` de los cruces: posición menos el inicio de la cavidad.
    let cross_y: Vec<f64> = ys.iter().map(|y| y - t).collect();
    let cross_x: Vec<f64> = xs.iter().map(|x| x - t).collect();
    let mut out = Vec::new();
    let along_y = Cavity { len: ly, height };
    let along_x = Cavity { len: lx, height };
    for (i, &x) in xs.iter().enumerate() {
        out.push(build(Axis::X, i + 1, x, &along_y, &cross_y, p));
    }
    for (i, &y) in ys.iter().enumerate() {
        out.push(build(Axis::Y, i + 1, y, &along_x, &cross_x, p));
    }
    for d in &out {
        for (id, lo, hi) in d.wall_slots(p, sizes) {
            if let Some(panel) = panels.iter_mut().find(|pn| pn.id == id) {
                let (a, b) = (panel.local(lo), panel.local(hi));
                panel.slots.push(Rect::new(a.0, a.1, b.0, b.1));
            }
        }
    }
    for panel in panels.iter_mut().filter(|pn| !pn.slots.is_empty()) {
        panel.rebuild();
    }
    if p.divider_joint == DividerJoint::Loose {
        warnings.push(tr!(
            "parts.box.div.loose_note",
            clearance = format!("{:.2}", p.divider_clearance_mm)
        ));
    } else if let Some(w) = out
        .iter()
        .filter_map(Divider::narrowest_tab)
        .min_by(f64::total_cmp)
        && w < t
    {
        warnings.push(tr!(
            "parts.box.div.thin_tabs",
            width = format!("{w:.1}"),
            thickness = format!("{t:.1}")
        ));
    }
    Ok(out)
}

/// Piezas de los divisores.
pub(crate) fn divider_parts(dividers: Vec<Divider>) -> Vec<Part> {
    dividers
        .into_iter()
        .map(|d| Part::new(&d.key(), d.name(), d.cuts, Vec::new()))
        .collect()
}
