//! Bandejas y cajones.
//!
//! # Diseño
//!
//! El cuerpo es la caja abierta de [`crate::boxes`] (cinco paneles con dientes, la misma regla de
//! aristas y de holgura), de modo que el encaje de los dientes es el ya probado en 3D allí. Este
//! módulo añade encima:
//!
//! - **Asas.** En la bandeja, dos recortes redondeados en los laterales izquierdo y derecho: una
//!   muesca abierta en el borde superior (`notch`) o un ojal en forma de píldora (`hole`). Se
//!   centran en el lateral, que en un tramo central del borde superior es recto, así que no tocan
//!   los dientes de las esquinas.
//! - **Cajón.** El cuerpo más un frente liso más grande (`front_overlap_mm` de más por cada lado y
//!   por arriba y abajo), que se pega por fuera del frente del cuerpo y tapa el hueco. El asa va
//!   en el frente y, para que se pueda meter el dedo, el mismo recorte atraviesa la pared frontal
//!   del cuerpo que queda detrás.
//! - **Caja exterior (`drawer_case`).** Una caja abierta por el frente, con el interior del
//!   cajón más `drawer_clearance_mm` por cada lado y de fondo. Se arma con la misma caja abierta
//!   de dientes, con los ejes cambiados (su «tapa abierta» es el hueco del cajón), así que sus
//!   uniones son las mismas que las de cualquier caja.
//!
//! Las medidas son del cuerpo (el cajón sin su frente). La geometría es nominal: el kerf lo
//! compensa la capa.

use crate::boxes::{BoxParams, Dims, build_panels};
use crate::error::PartsError;
use crate::part::{Generated, Part};
use crate::rectilinear::{rect_minus_rects, signed_area};
use crate::registry::ParamSpec;
use serde::{Deserialize, Serialize};
use tlaku_core::{Point, Polyline};
use tlaku_i18n::tr;

/// Qué se genera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrayKind {
    /// Bandeja abierta.
    Tray,
    /// Cajón: cuerpo y frente grande.
    Drawer,
    /// Cajón y la caja exterior que lo contiene.
    DrawerCase,
}

/// Forma del asa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Handle {
    None,
    /// Muesca redondeada abierta en el borde superior.
    Notch,
    /// Ojal en forma de píldora.
    Hole,
}

/// Parámetros de la bandeja o el cajón.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrayParams {
    pub kind: TrayKind,
    pub dims: Dims,
    pub width_mm: f64,
    pub depth_mm: f64,
    pub height_mm: f64,
    pub thickness_mm: f64,
    /// Ancho de diente pedido (se ajusta a un número impar de celdas iguales).
    pub finger_mm: f64,
    /// Ranura menos diente.
    pub fit_mm: f64,
    pub handle: Handle,
    /// Ancho del asa.
    pub handle_width_mm: f64,
    /// Hondo de la muesca o alto del ojal.
    pub handle_depth_mm: f64,
    /// Cuánto sobresale el frente del cajón del cuerpo por cada lado.
    pub front_overlap_mm: f64,
    /// Juego entre el cajón y la caja exterior, por cada lado y de fondo.
    pub drawer_clearance_mm: f64,
}

impl Default for TrayParams {
    fn default() -> Self {
        Self {
            kind: TrayKind::Tray,
            dims: Dims::Outer,
            width_mm: 120.0,
            depth_mm: 90.0,
            height_mm: 30.0,
            thickness_mm: 3.0,
            finger_mm: 10.0,
            fit_mm: 0.0,
            handle: Handle::None,
            handle_width_mm: 30.0,
            handle_depth_mm: 12.0,
            front_overlap_mm: 6.0,
            drawer_clearance_mm: 0.5,
        }
    }
}

/// Material mínimo (mm) que debe quedar entre un recorte y los dientes o el borde.
const MIN_WEB_MM: f64 = 3.0;

fn range(field: &str, v: f64, lo: f64, hi: f64) -> Option<PartsError> {
    (!(lo..=hi).contains(&v) || v.is_nan())
        .then(|| PartsError::field(field, tr!("parts.tray.range", min = lo, max = hi)))
}

impl TrayParams {
    /// La caja abierta de dientes que es el cuerpo.
    fn body(&self) -> BoxParams {
        BoxParams {
            kind: crate::boxes::BoxKind::Open,
            dims: self.dims,
            width_mm: self.width_mm,
            depth_mm: self.depth_mm,
            height_mm: self.height_mm,
            thickness_mm: self.thickness_mm,
            finger_mm: self.finger_mm,
            fit_mm: self.fit_mm,
            ..BoxParams::default()
        }
    }

    fn validate(&self) -> Result<(), PartsError> {
        let checks = [
            range("handle_width_mm", self.handle_width_mm, 5.0, 300.0),
            range("handle_depth_mm", self.handle_depth_mm, 3.0, 100.0),
            range("front_overlap_mm", self.front_overlap_mm, 0.0, 30.0),
            range("drawer_clearance_mm", self.drawer_clearance_mm, 0.0, 5.0),
        ];
        if let Some(e) = checks.into_iter().flatten().next() {
            return Err(e);
        }
        Ok(())
    }
}

/// Píldora de `w x h` centrada en `(cx, cy)`, en sentido horario (un agujero).
fn pill(cx: f64, cy: f64, w: f64, h: f64) -> Polyline {
    let r = w.min(h) / 2.0;
    let (ax, ay) = ((w / 2.0 - r).max(0.0), (h / 2.0 - r).max(0.0));
    let steps = 16;
    let mut pts = Vec::new();
    // Cuatro esquinas redondas, en sentido antihorario, empezando por arriba a la derecha.
    for (qx, qy, start) in [
        (ax, ay, 0.0_f64),
        (-ax, ay, 90.0),
        (-ax, -ay, 180.0),
        (ax, -ay, 270.0),
    ] {
        for k in 0..=steps {
            let a = (start + 90.0 * f64::from(k) / f64::from(steps)).to_radians();
            pts.push(Point::new(cx + qx + r * a.cos(), cy + qy + r * a.sin()));
        }
    }
    pts.reverse();
    // Si un lado recto mide cero, el último punto repite al primero.
    pts.dedup_by(|a, b| (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9);
    if let (Some(f), Some(l)) = (pts.first().copied(), pts.last().copied())
        && (f.x - l.x).abs() < 1e-9
        && (f.y - l.y).abs() < 1e-9
    {
        pts.pop();
    }
    Polyline {
        points: pts,
        closed: true,
    }
}

/// Recorte en U con el fondo redondeado, de arriba hacia abajo y de derecha a izquierda (el
/// sentido del borde superior de un contorno antihorario). `top` es la altura del borde.
fn notch_points(cx: f64, top: f64, hw: f64, hd: f64) -> Vec<Point> {
    let r = (hw / 2.0).min(hd);
    let steps = 12;
    let mut pts = vec![Point::new(cx + hw / 2.0, top)];
    // Esquina inferior derecha (de 0 a -90 grados) y luego la izquierda (de -90 a -180).
    for (ccx, from) in [(cx + hw / 2.0 - r, 0.0_f64), (cx - hw / 2.0 + r, -90.0)] {
        for k in 0..=steps {
            let a = (from - 90.0 * f64::from(k) / f64::from(steps)).to_radians();
            pts.push(Point::new(ccx + r * a.cos(), top - hd + r + r * a.sin()));
        }
    }
    pts.push(Point::new(cx - hw / 2.0, top));
    pts
}

/// Abre una muesca en el borde superior recto (`y == top`) del contorno exterior. `false` si el
/// borde no tiene un tramo recto que la contenga.
fn cut_notch(cuts: &mut [Polyline], top: f64, cx: f64, hw: f64, hd: f64) -> bool {
    let (x_hi, x_lo) = (cx + hw / 2.0, cx - hw / 2.0);
    let Some(outer) = cuts
        .iter_mut()
        .filter(|p| p.closed)
        .max_by(|a, b| signed_area(a).total_cmp(&signed_area(b)))
    else {
        return false;
    };
    let n = outer.points.len();
    let found = (0..n).find(|&i| {
        let (a, b) = (outer.points[i], outer.points[(i + 1) % n]);
        (a.y - top).abs() < 1e-9
            && (b.y - top).abs() < 1e-9
            && a.x >= x_hi - 1e-9
            && b.x <= x_lo + 1e-9
    });
    let Some(i) = found else { return false };
    let notch = notch_points(cx, top, hw, hd);
    let tail = outer.points.split_off(i + 1);
    outer.points.extend(notch);
    outer.points.extend(tail);
    outer
        .points
        .dedup_by(|a, b| (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9);
    if outer.points.len() > 1 {
        let (f, l) = (outer.points[0], outer.points[outer.points.len() - 1]);
        if (f.x - l.x).abs() < 1e-9 && (f.y - l.y).abs() < 1e-9 {
            outer.points.pop();
        }
    }
    true
}

/// Forma de un asa: centro, altura del borde superior de la forma completa y tamaño.
struct Cutout {
    cx: f64,
    top: f64,
    hw: f64,
    hd: f64,
    hole: bool,
}

/// Recorta la forma en la pieza (`edge` es la altura de su borde superior). Para una muesca más
/// alta que el borde solo se corta la parte que cae dentro de la pieza.
fn apply_cutout(cuts: &mut Vec<Polyline>, edge: f64, c: &Cutout) -> bool {
    if c.hole {
        let cy = c.top - c.hd / 2.0;
        if cy + c.hd / 2.0 > edge {
            return false;
        }
        cuts.push(pill(c.cx, cy, c.hw, c.hd));
        true
    } else {
        let depth = c.top - c.hd;
        let inside = edge - depth;
        if inside <= 0.0 {
            return true; // la muesca no llega a la pieza
        }
        cut_notch(cuts, edge, c.cx, c.hw, inside)
    }
}

/// Genera las piezas de la bandeja o del cajón.
pub fn tray_parts(p: &TrayParams) -> Result<Generated, PartsError> {
    p.validate()?;
    let t = p.thickness_mm;
    let body = p.body();
    let (sizes, mut panels, mut warnings) = build_panels(&body)?;
    let drawer = p.kind != TrayKind::Tray;
    let ov = p.front_overlap_mm;
    let handle_on = p.handle != Handle::None;
    let hole = p.handle == Handle::Hole;
    let (hw, hd) = (p.handle_width_mm, p.handle_depth_mm);
    // Las paredes y el frente donde cabe el asa: dientes de las esquinas, fondo y bordes.
    let mut skipped = false;
    let mut extra: Vec<Part> = Vec::new();
    let mut plate_cuts = if drawer {
        rect_minus_rects(sizes.w + 2.0 * ov, sizes.h + 2.0 * ov, &[])
    } else {
        Vec::new()
    };
    if handle_on {
        if drawer {
            let (pw, ph) = (sizes.w + 2.0 * ov, sizes.h + 2.0 * ov);
            if hw > pw - 2.0 * MIN_WEB_MM * 2.0 {
                return Err(PartsError::field(
                    "handle_width_mm",
                    tr!("parts.tray.handle_wide", max = format!("{:.1}", pw - 12.0)),
                ));
            }
            if hd > ph - 2.0 * MIN_WEB_MM * 2.0 {
                return Err(PartsError::field(
                    "handle_depth_mm",
                    tr!("parts.tray.handle_deep", max = format!("{:.1}", ph - 12.0)),
                ));
            }
            // Ojal: a una distancia del borde superior del frente; muesca: desde el borde.
            let top = if hole {
                ph - ov - (2.0 * t).max(5.0)
            } else {
                ph
            };
            let front_cut = Cutout {
                cx: pw / 2.0,
                top,
                hw,
                hd,
                hole,
            };
            apply_cutout(&mut plate_cuts, ph, &front_cut);
            // Misma abertura en la pared frontal del cuerpo, que queda detrás (coordenadas del
            // cuerpo = las del frente menos el sobrante).
            let back_cut = Cutout {
                cx: sizes.w / 2.0,
                top: top - ov,
                ..front_cut
            };
            let min_below = t + MIN_WEB_MM;
            let low = if hole {
                back_cut.top - hd
            } else {
                sizes.h - (hd - ov).max(0.0)
            };
            if low < min_below {
                return Err(PartsError::field(
                    "handle_depth_mm",
                    tr!(
                        "parts.tray.handle_deep",
                        max = format!("{:.1}", sizes.h - min_below + ov)
                    ),
                ));
            }
            if hw > sizes.w - 2.0 * (t + MIN_WEB_MM) {
                return Err(PartsError::field(
                    "handle_width_mm",
                    tr!(
                        "parts.tray.handle_wide",
                        max = format!("{:.1}", sizes.w - 2.0 * (t + MIN_WEB_MM))
                    ),
                ));
            }
            if let Some(front) = panels.iter_mut().find(|pn| pn.key() == "front") {
                skipped |= !apply_cutout(&mut front.cuts, sizes.h, &back_cut);
            }
        } else {
            // Bandeja: laterales izquierdo y derecho, cuyo borde superior es recto entre los
            // dientes de las esquinas.
            let side_len = sizes.d;
            let max_w = side_len - 2.0 * (t + MIN_WEB_MM);
            if hw > max_w {
                return Err(PartsError::field(
                    "handle_width_mm",
                    tr!("parts.tray.handle_wide", max = format!("{max_w:.1}")),
                ));
            }
            let top = if hole {
                sizes.h - (2.0 * t).max(5.0)
            } else {
                sizes.h
            };
            let max_d = if hole {
                top - t - MIN_WEB_MM
            } else {
                sizes.h - t - MIN_WEB_MM
            };
            if hd > max_d {
                return Err(PartsError::field(
                    "handle_depth_mm",
                    tr!("parts.tray.handle_deep", max = format!("{max_d:.1}")),
                ));
            }
            let cut = Cutout {
                cx: side_len / 2.0,
                top,
                hw,
                hd,
                hole,
            };
            for pn in panels
                .iter_mut()
                .filter(|pn| pn.key() == "left" || pn.key() == "right")
            {
                skipped |= !apply_cutout(&mut pn.cuts, sizes.h, &cut);
            }
        }
        if skipped {
            warnings.push(tr!("parts.tray.handle_skipped"));
        }
    }

    if drawer {
        extra.push(Part::new(
            "drawer_front",
            tr!("parts.tray.name.front"),
            plate_cuts,
            Vec::new(),
        ));
    }
    let mut parts: Vec<Part> = panels
        .into_iter()
        .map(|panel| {
            let name = panel.name();
            Part::new(panel.key(), name, panel.cuts, panel.engraves)
        })
        .collect();
    parts.extend(extra);

    if p.fit_mm != 0.0 {
        warnings.push(tr!("parts.box.fit_vs_kerf"));
    }
    if drawer {
        warnings.push(tr!(
            "parts.tray.front_glue",
            w = format!("{:.1}", sizes.w + 2.0 * ov),
            h = format!("{:.1}", sizes.h + 2.0 * ov)
        ));
    }
    if p.kind == TrayKind::DrawerCase {
        let c = p.drawer_clearance_mm;
        if ov - c < 2.0 {
            warnings.push(tr!(
                "parts.tray.front_short",
                overlap = format!("{ov:.1}"),
                clearance = format!("{c:.1}")
            ));
        }
        let case = BoxParams {
            kind: crate::boxes::BoxKind::Open,
            dims: Dims::Inner,
            width_mm: sizes.w + 2.0 * c,
            depth_mm: sizes.h + 2.0 * c,
            height_mm: sizes.d + c,
            thickness_mm: t,
            finger_mm: p.finger_mm,
            fit_mm: p.fit_mm,
            ..BoxParams::default()
        };
        let (_, case_panels, _) =
            build_panels(&case).map_err(|e| PartsError::general(e.message))?;
        for panel in case_panels {
            // Ejes de la caja abierta -> caja exterior: su «frente» y «atrás» son la base y la
            // tapa de la caja, los laterales siguen siendo laterales y su fondo es el trasero.
            let (key, name) = match panel.key() {
                "front" => ("case_bottom", tr!("parts.tray.case.bottom")),
                "back" => ("case_top", tr!("parts.tray.case.top")),
                "left" => ("case_left", tr!("parts.tray.case.left")),
                "right" => ("case_right", tr!("parts.tray.case.right")),
                _ => ("case_back", tr!("parts.tray.case.back")),
            };
            parts.push(Part::new(key, name, panel.cuts, Vec::new()));
        }
        warnings.push(tr!(
            "parts.tray.case_note",
            w = format!("{:.1}", sizes.w + 2.0 * c),
            h = format!("{:.1}", sizes.h + 2.0 * c),
            d = format!("{:.1}", sizes.d + c),
            c = format!("{c:.1}")
        ));
    }
    Ok(Generated { parts, warnings })
}

/// Campos del formulario.
pub fn schema() -> Vec<ParamSpec> {
    let d = TrayParams::default();
    vec![
        ParamSpec::choice("kind", "tray", &["tray", "drawer", "drawer_case"], "tray"),
        ParamSpec::choice("dims", "size", &["outer", "inner"], "outer"),
        ParamSpec::length("width_mm", "size", 1.0, 5000.0, 1.0, d.width_mm),
        ParamSpec::length("depth_mm", "size", 1.0, 5000.0, 1.0, d.depth_mm),
        ParamSpec::length("height_mm", "size", 1.0, 5000.0, 1.0, d.height_mm),
        ParamSpec::length("thickness_mm", "size", 0.5, 20.0, 0.1, d.thickness_mm),
        ParamSpec::length("finger_mm", "joints", 1.5, 200.0, 0.5, d.finger_mm),
        ParamSpec::length("fit_mm", "joints", -0.5, 1.0, 0.01, d.fit_mm),
        ParamSpec::choice("handle", "handle", &["none", "notch", "hole"], "none"),
        ParamSpec::length(
            "handle_width_mm",
            "handle",
            5.0,
            300.0,
            1.0,
            d.handle_width_mm,
        )
        .when("handle", &["notch", "hole"]),
        ParamSpec::length(
            "handle_depth_mm",
            "handle",
            3.0,
            100.0,
            1.0,
            d.handle_depth_mm,
        )
        .when("handle", &["notch", "hole"]),
        ParamSpec::length(
            "front_overlap_mm",
            "drawer",
            0.0,
            30.0,
            0.5,
            d.front_overlap_mm,
        )
        .when("kind", &["drawer", "drawer_case"]),
        ParamSpec::length(
            "drawer_clearance_mm",
            "drawer",
            0.0,
            5.0,
            0.1,
            d.drawer_clearance_mm,
        )
        .when("kind", &["drawer_case"]),
    ]
}

#[cfg(test)]
mod tests;
