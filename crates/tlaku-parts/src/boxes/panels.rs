//! Paneles de la caja: marco 3D, aristas que comparten y muescas de los dientes.

use super::{BoxKind, BoxParams, Sizes};
use crate::error::PartsError;
use crate::fingers::Cells;
use crate::rectilinear::{Rect, rect_minus_rects};
use tlaku_core::Polyline;
use tlaku_i18n::tr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Id {
    Front,
    Back,
    Left,
    Right,
    Bottom,
    Top,
    Lid,
}

pub(crate) type V3 = [f64; 3];

/// Un panel con su marco 3D. El marco `(o, u, v)` lleva las coordenadas locales (u, v) del panel
/// a la caja: `o` es la esquina de la cara **exterior** y la normal exterior es `u x v`, de modo
/// que visto desde fuera (con `u` a la derecha y `v` arriba) los rótulos se leen bien. El panel
/// ocupa un espesor `thickness` hacia el interior.
#[derive(Debug, Clone)]
pub(crate) struct Panel {
    pub id: Id,
    pub w: f64,
    pub h: f64,
    pub o: V3,
    pub u: V3,
    pub v: V3,
    #[cfg(test)]
    pub thickness: f64,
    /// Ranuras que no son dientes (la de la tapa deslizante).
    pub voids: Vec<Rect>,
    /// Muescas de los dientes (ya con la holgura).
    pub cut_rects: Vec<Rect>,
    /// Ranuras interiores (agujeros) de las pestañas de los divisores.
    pub slots: Vec<Rect>,
    pub cuts: Vec<Polyline>,
    pub engraves: Vec<Polyline>,
    /// Ancho de celda más angosto de las aristas de este panel.
    pub min_cell: Option<f64>,
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[cfg(test)]
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

impl Panel {
    pub fn new(id: Id, p: &BoxParams, s: &Sizes) -> Self {
        let t = p.thickness_mm;
        let lc = p.lid_clearance_mm;
        let (w, h, o, u, v) = match id {
            Id::Front => (s.w, s.h, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            Id::Back => (
                s.w,
                s.back_h,
                [s.w, s.d, 0.0],
                [-1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
            ),
            Id::Left => (s.d, s.h, [0.0, s.d, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]),
            Id::Right => (s.d, s.h, [s.w, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
            Id::Bottom => (s.w, s.d, [0.0, s.d, 0.0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0]),
            Id::Top => (s.w, s.d, [0.0, 0.0, s.h], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            Id::Lid => (
                s.w - lc,
                s.d - t - t - lc / 2.0,
                [lc / 2.0, t + t + lc / 2.0, s.back_h + t],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
            ),
        };
        let mut voids = Vec::new();
        if p.kind == BoxKind::SlidingLid {
            // Ranura pasante abierta hacia atrás. Se detiene un grosor antes del frontal: ese
            // bloque une la franja de arriba con el resto del lateral (sin él quedaría suelta).
            let (v0, v1) = (s.back_h, s.back_h + s.slot_h);
            match id {
                Id::Left => voids.push(Rect::new(0.0, v0, s.d - 2.0 * t, v1)),
                Id::Right => voids.push(Rect::new(2.0 * t, v0, s.d, v1)),
                _ => {}
            }
        }
        Self {
            id,
            w,
            h,
            o,
            u,
            v,
            #[cfg(test)]
            thickness: t,
            voids,
            cut_rects: Vec::new(),
            slots: Vec::new(),
            cuts: Vec::new(),
            engraves: Vec::new(),
            min_cell: None,
        }
    }

    pub fn key(&self) -> &'static str {
        match self.id {
            Id::Front => "front",
            Id::Back => "back",
            Id::Left => "left",
            Id::Right => "right",
            Id::Bottom => "bottom",
            Id::Top => "top",
            Id::Lid => "lid",
        }
    }

    pub fn name(&self) -> String {
        match self.id {
            Id::Front => tr!("parts.box.name.front"),
            Id::Back => tr!("parts.box.name.back"),
            Id::Left => tr!("parts.box.name.left"),
            Id::Right => tr!("parts.box.name.right"),
            Id::Bottom => tr!("parts.box.name.bottom"),
            Id::Top => tr!("parts.box.name.top"),
            Id::Lid => tr!("parts.box.name.lid"),
        }
    }

    /// Recalcula el contorno con las muescas, las ranuras de la tapa y las de los divisores.
    pub fn rebuild(&mut self) {
        let mut holes = self.cut_rects.clone();
        holes.extend(self.voids.iter().copied());
        holes.extend(self.slots.iter().copied());
        self.cuts = rect_minus_rects(self.w, self.h, &holes);
    }

    /// Punto 3D de la cara exterior en coordenadas locales.
    pub fn to3(&self, u: f64, v: f64) -> V3 {
        [
            self.o[0] + u * self.u[0] + v * self.v[0],
            self.o[1] + u * self.u[1] + v * self.v[1],
            self.o[2] + u * self.u[2] + v * self.v[2],
        ]
    }

    /// Coordenadas locales de un punto 3D (se ignora su distancia al plano).
    pub fn local(&self, p: V3) -> (f64, f64) {
        let d = [p[0] - self.o[0], p[1] - self.o[1], p[2] - self.o[2]];
        (dot(d, self.u), dot(d, self.v))
    }

    /// Normal exterior.
    #[cfg(test)]
    pub fn normal(&self) -> V3 {
        cross(self.u, self.v)
    }

    /// Distancia hacia el interior desde la cara exterior de un punto 3D (positiva = dentro).
    #[cfg(test)]
    pub fn depth(&self, p: V3) -> f64 {
        let n = self.normal();
        let d = [p[0] - self.o[0], p[1] - self.o[1], p[2] - self.o[2]];
        -dot(d, n)
    }

    /// Extremos 3D del lado `s` (0 abajo, 1 derecha, 2 arriba, 3 izquierda), en orden antihorario.
    fn side_ends(&self, s: usize) -> (V3, V3) {
        let (w, h) = (self.w, self.h);
        let (a, b) = match s {
            0 => ((0.0, 0.0), (w, 0.0)),
            1 => ((w, 0.0), (w, h)),
            2 => ((w, h), (0.0, h)),
            _ => ((0.0, h), (0.0, 0.0)),
        };
        (self.to3(a.0, a.1), self.to3(b.0, b.1))
    }
}

/// Arista: eje en que corre y las otras dos coordenadas (en micras, para comparar).
type EdgeKey = (usize, i64, i64);

fn edge_of(a: V3, b: V3) -> Option<(EdgeKey, f64, f64)> {
    let axis = (0..3).find(|&k| (a[k] - b[k]).abs() > 1e-9)?;
    let others: Vec<i64> = (0..3)
        .filter(|&k| k != axis)
        .map(|k| (a[k] * 1e6).round() as i64)
        .collect();
    Some((
        (axis, others[0], others[1]),
        a[axis].min(b[axis]),
        a[axis].max(b[axis]),
    ))
}

fn is_front_back(id: Id) -> bool {
    matches!(id, Id::Front | Id::Back)
}

/// ¿`me` posee las celdas pares (las de los extremos) de la arista que comparte con `other`?
fn owns_even(me: Id, other: Id) -> bool {
    if is_front_back(me) {
        true
    } else if is_front_back(other) {
        false
    } else {
        // Lateral contra superior/inferior.
        matches!(me, Id::Left | Id::Right)
    }
}

/// Calcula las muescas del panel y su contorno. `all` son todos los paneles de la caja.
pub(crate) fn build_panel(
    panel: &mut Panel,
    all: &[Panel],
    p: &BoxParams,
    _warnings: &mut Vec<String>,
) -> Result<(), PartsError> {
    let t = p.thickness_mm;
    let fit = p.fit_mm;
    for s in 0..4 {
        let (a3, b3) = panel.side_ends(s);
        let Some((key, lo, hi)) = edge_of(a3, b3) else {
            continue;
        };
        for other in all.iter().filter(|o| o.id != panel.id) {
            if other.id == Id::Lid || panel.id == Id::Lid {
                continue;
            }
            let found = (0..4).find_map(|s2| {
                let (c, d) = other.side_ends(s2);
                edge_of(c, d).filter(|(k, ..)| *k == key)
            });
            let Some((_, olo, ohi)) = found else { continue };
            let (mut from, mut to) = (lo.max(olo), hi.min(ohi));
            if key.0 == 1 {
                // Aristas en Y: los cubos de los vértices son del frontal y del trasero.
                from += t;
                to -= t;
            }
            if to - from <= 0.0 {
                continue;
            }
            let cells = Cells::plan(from, to - from, p.finger_mm);
            panel.min_cell = Some(
                panel
                    .min_cell
                    .map_or(cells.width(), |m| m.min(cells.width())),
            );
            let me_even = owns_even(panel.id, other.id);
            for k in 0..cells.n {
                // Muesca donde la celda es de la otra pieza.
                if (k % 2 == 0) == me_even {
                    continue;
                }
                let (ca, cb) = cells.cell(k);
                let along = |c: f64| {
                    let mut pt = a3;
                    pt[key.0] = c;
                    let (lu, lv) = panel.local(pt);
                    if s % 2 == 0 { lu } else { lv }
                };
                let (x, y) = (along(ca).min(along(cb)), along(ca).max(along(cb)));
                let side_len = if s % 2 == 0 { panel.w } else { panel.h };
                let (x, y) = ((x - fit / 4.0).max(0.0), (y + fit / 4.0).min(side_len));
                let (w, h) = (panel.w, panel.h);
                let rect = match s {
                    0 => Rect::new(x, 0.0, y, t),
                    1 => Rect::new(w - t, x, w, y),
                    2 => Rect::new(x, h - t, y, h),
                    _ => Rect::new(0.0, x, t, y),
                };
                panel.cut_rects.push(rect);
            }
            break;
        }
    }
    panel.rebuild();
    Ok(())
}

/// Ancho de celda más angosto entre todos los paneles.
pub(crate) fn narrowest_cell(panels: &[Panel], _p: &BoxParams) -> Option<f64> {
    panels
        .iter()
        .filter_map(|p| p.min_cell)
        .min_by(f64::total_cmp)
}
