//! Caja de esquinas redondeadas: base y tapa con esquinas de radio `R` y un solo lateral con
//! bisagra flexible en las curvas, unido con dientes a la base y a la tapa.
//!
//! # Diseño
//!
//! El contorno es un rectángulo `W x D` (medidas **exteriores**) de esquinas redondeadas de radio
//! `R`. El lateral es una sola tira que da la vuelta completa; las rectas del contorno quedan
//! planas y las cuatro esquinas se curvan, por lo que solo ahí lleva la bisagra (cortes paralelos
//! al eje vertical, el de flexión).
//!
//! **Longitud del lateral.** Es el perímetro de la *línea media* del grosor del lateral (el
//! contorno desplazado `T / 2` hacia adentro, de radio `R - T/2`): al curvar la tira esa línea no
//! se estira ni se comprime, así que es la que conserva su largo. La tira mide exactamente
//! [`mid_perimeter`] entre los dos extremos del cierre; el contorno exterior es más largo
//! (`2 pi T / 2` más) y el interior más corto.
//!
//! **Dientes.** Se reparten `n` celdas iguales (siempre par) a lo largo de esa línea media, con el
//! cierre del lateral en un límite de celda. Las celdas pares son dientes del lateral (entran en
//! muescas de la base y de la tapa) y las impares, lengüetas de la base y de la tapa (el lateral
//! queda con su canto a `T` del borde). Las muescas de la placa son sectores de corona de ancho
//! `T` (rectángulos en las rectas, arcos en las curvas) cuya área coincide con la del diente
//! plano: `T x ancho de celda`, porque la línea media es la que no se estira. La holgura de
//! encaje ensancha cada muesca `fit_mm / 2` por lado.
//!
//! **Cierre.** El lateral empieza y termina en el centro del lado frontal. Los dos extremos se
//! unen con cola de milano plana (módulo [`crate::joints`]) o, con `seam = butt`, a tope (se pega).
//!
//! **Lo que no cubre.** La bisagra flexible no se ha probado en material real con esta geometría:
//! el radio mínimo que aguanta cada material depende de la bisagra elegida; empieza con la pieza
//! de prueba de la bisagra.

use crate::error::PartsError;
use crate::hinge::{HingeParams, HingePattern, Orientation, zone_cuts};
use crate::joints::{JointKind, JointsParams};
use crate::part::{Generated, Part};
use crate::registry::ParamSpec;
use serde::{Deserialize, Serialize};
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use tlaku_core::{Point, Polyline};
use tlaku_i18n::tr;

/// Cierre del lateral.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Seam {
    /// Cola(s) de milano plana(s) en los dos extremos.
    Dovetail,
    /// Extremos rectos, a tope: se pegan.
    Butt,
}

/// Parámetros de la caja redondeada. Los campos que falten en un guardado viejo toman el valor
/// de fábrica.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoundedBoxParams {
    pub width_mm: f64,
    pub depth_mm: f64,
    /// Alto exterior total (con la base y la tapa).
    pub height_mm: f64,
    /// Radio exterior de las esquinas.
    pub radius_mm: f64,
    pub thickness_mm: f64,
    /// Ancho de diente pedido; el real reparte el perímetro en un número par de celdas.
    pub finger_mm: f64,
    /// Muesca menos diente, en mm (positivo = más flojo).
    pub fit_mm: f64,
    /// Con tapa (si no, solo base y lateral).
    pub lid: bool,
    pub seam: Seam,
    pub pattern: HingePattern,
    pub cut_length_mm: f64,
    pub bridge_mm: f64,
    pub row_gap_mm: f64,
    pub margin_mm: f64,
    pub amplitude_mm: f64,
    pub wavelength_mm: f64,
}

impl Default for RoundedBoxParams {
    fn default() -> Self {
        Self {
            width_mm: 100.0,
            depth_mm: 70.0,
            height_mm: 40.0,
            radius_mm: 15.0,
            thickness_mm: 3.0,
            finger_mm: 8.0,
            fit_mm: 0.0,
            lid: true,
            seam: Seam::Dovetail,
            pattern: HingePattern::Straight,
            cut_length_mm: 15.0,
            bridge_mm: 3.0,
            row_gap_mm: 2.5,
            margin_mm: 3.0,
            amplitude_mm: 1.5,
            wavelength_mm: 10.0,
        }
    }
}

/// Campos del formulario de la caja redondeada.
pub fn schema() -> Vec<ParamSpec> {
    let d = RoundedBoxParams::default();
    let wavy = ["wave", "zigzag"];
    vec![
        ParamSpec::boolean("lid", "box", d.lid),
        ParamSpec::length("width_mm", "size", 20.0, 2000.0, 1.0, d.width_mm),
        ParamSpec::length("depth_mm", "size", 20.0, 2000.0, 1.0, d.depth_mm),
        ParamSpec::length("height_mm", "size", 15.0, 500.0, 1.0, d.height_mm),
        ParamSpec::length("radius_mm", "size", 5.0, 500.0, 0.5, d.radius_mm),
        ParamSpec::length("thickness_mm", "size", 0.5, 20.0, 0.1, d.thickness_mm),
        ParamSpec::length("finger_mm", "joints", 3.0, 200.0, 0.5, d.finger_mm),
        ParamSpec::length("fit_mm", "joints", -0.5, 1.0, 0.01, d.fit_mm),
        ParamSpec::choice("seam", "joints", &["dovetail", "butt"], "dovetail"),
        ParamSpec::choice(
            "pattern",
            "pattern",
            &["straight", "wave", "zigzag", "brick", "diamond"],
            "straight",
        ),
        ParamSpec::length("cut_length_mm", "cuts", 1.0, 500.0, 0.5, d.cut_length_mm),
        ParamSpec::length("bridge_mm", "cuts", 0.2, 100.0, 0.1, d.bridge_mm),
        ParamSpec::length("row_gap_mm", "cuts", 0.4, 50.0, 0.1, d.row_gap_mm),
        ParamSpec::length("margin_mm", "cuts", 0.0, 200.0, 0.5, d.margin_mm),
        ParamSpec::length("amplitude_mm", "wave", 0.0, 50.0, 0.1, d.amplitude_mm)
            .when("pattern", &wavy),
        ParamSpec::length("wavelength_mm", "wave", 1.0, 500.0, 0.5, d.wavelength_mm)
            .when("pattern", &wavy),
    ]
}

/// Mínimo de ancho de celda (mm) por debajo del cual el diente no se puede cortar.
const MIN_CELL_MM: f64 = 1.5;
/// Tope de celdas: la vista previa se recalcula mientras se teclea.
const MAX_CELLS: usize = 400;
/// Aire (mm) entre la zona de dientes o de la tapa y los cortes/cierre del lateral.
const SEAM_GAP_MM: f64 = 2.0;
/// Error máximo (mm) al aproximar un arco con segmentos.
const ARC_TOL_MM: f64 = 0.01;

fn range(field: &str, v: f64, lo: f64, hi: f64) -> Result<(), PartsError> {
    if v.is_nan() || !(lo..=hi).contains(&v) {
        return Err(PartsError::field(
            field,
            tr!("parts.rounded.range", min = lo, max = hi),
        ));
    }
    Ok(())
}

// ---- la línea media del lateral ----------------------------------------------------------------

/// Un tramo de la línea media: recta o cuarto de círculo, ambos en sentido antihorario.
#[derive(Debug, Clone, Copy)]
enum Piece {
    Line {
        from: (f64, f64),
        dir: (f64, f64),
    },
    Arc {
        center: (f64, f64),
        radius: f64,
        start_angle: f64,
    },
}

#[derive(Debug, Clone, Copy)]
struct Seg {
    piece: Piece,
    /// Posición del inicio del tramo a lo largo de la línea media.
    start: f64,
    len: f64,
}

/// El contorno medio recorrido de `s = 0` (centro del lado frontal) en sentido antihorario.
#[derive(Debug, Clone)]
pub struct Contour {
    segs: Vec<Seg>,
    total: f64,
}

/// Tipo de una zona del lateral.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneKind {
    Straight,
    /// Esquina curva: lleva la bisagra.
    Arc,
}

/// Un tramo del lateral, de `from` a `to` medidos a lo largo de la línea media (coordenada X de
/// la tira plana).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Zone {
    pub kind: ZoneKind,
    pub from: f64,
    pub to: f64,
}

impl Contour {
    /// Línea media de un rectángulo `w x d` con esquinas de radio `r`, centrado en el origen.
    fn new(w: f64, d: f64, r: f64) -> Self {
        let (hx, hy) = (w / 2.0, d / 2.0);
        let mut segs: Vec<Seg> = Vec::new();
        let mut at = 0.0;
        let mut push = |piece: Piece, len: f64| {
            if len > 1e-12 {
                segs.push(Seg {
                    piece,
                    start: at,
                    len,
                });
                at += len;
            }
        };
        let (sx, sy) = (hx - r, hy - r);
        // Mitad derecha del lado frontal.
        push(
            Piece::Line {
                from: (0.0, -hy),
                dir: (1.0, 0.0),
            },
            sx,
        );
        let quarter = r * FRAC_PI_2;
        push(
            Piece::Arc {
                center: (sx, -sy),
                radius: r,
                start_angle: -FRAC_PI_2,
            },
            quarter,
        );
        push(
            Piece::Line {
                from: (hx, -sy),
                dir: (0.0, 1.0),
            },
            2.0 * sy,
        );
        push(
            Piece::Arc {
                center: (sx, sy),
                radius: r,
                start_angle: 0.0,
            },
            quarter,
        );
        push(
            Piece::Line {
                from: (sx, hy),
                dir: (-1.0, 0.0),
            },
            2.0 * sx,
        );
        push(
            Piece::Arc {
                center: (-sx, sy),
                radius: r,
                start_angle: FRAC_PI_2,
            },
            quarter,
        );
        push(
            Piece::Line {
                from: (-hx, sy),
                dir: (0.0, -1.0),
            },
            2.0 * sy,
        );
        push(
            Piece::Arc {
                center: (-sx, -sy),
                radius: r,
                start_angle: PI,
            },
            quarter,
        );
        // Mitad izquierda del lado frontal.
        push(
            Piece::Line {
                from: (-sx, -hy),
                dir: (1.0, 0.0),
            },
            sx,
        );
        Self { segs, total: at }
    }

    /// Longitud de la línea media.
    pub fn total(&self) -> f64 {
        self.total
    }

    /// Las zonas rectas y curvas en orden, de `0` a `total`.
    pub fn zones(&self) -> Vec<Zone> {
        self.segs
            .iter()
            .map(|s| Zone {
                kind: match s.piece {
                    Piece::Line { .. } => ZoneKind::Straight,
                    Piece::Arc { .. } => ZoneKind::Arc,
                },
                from: s.start,
                to: s.start + s.len,
            })
            .collect()
    }

    fn seg_at(&self, s: f64) -> &Seg {
        let s = s.rem_euclid(self.total);
        self.segs
            .iter()
            .rev()
            .find(|g| s >= g.start - 1e-12)
            .unwrap_or(&self.segs[0])
    }

    /// Punto a `s` a lo largo de la línea media, desplazado `off` hacia afuera (negativo =
    /// hacia adentro). `s` es cíclico.
    fn at(&self, s: f64, off: f64) -> Point {
        let g = self.seg_at(s);
        let u = s.rem_euclid(self.total) - g.start;
        match g.piece {
            Piece::Line { from, dir } => {
                // Hacia afuera es la derecha del sentido de avance (recorrido antihorario).
                let n = (dir.1, -dir.0);
                Point::new(
                    from.0 + dir.0 * u + n.0 * off,
                    from.1 + dir.1 * u + n.1 * off,
                )
            }
            Piece::Arc {
                center,
                radius,
                start_angle,
            } => {
                let a = start_angle + u / radius;
                Point::new(
                    center.0 + (radius + off) * a.cos(),
                    center.1 + (radius + off) * a.sin(),
                )
            }
        }
    }

    /// Recorre de `s0` a `s1` (`s1 >= s0`, puede pasar del cierre) el contorno desplazado `off`,
    /// con vértices en los cambios de tramo y puntos intermedios en los arcos.
    fn walk(&self, s0: f64, s1: f64, off: f64) -> Vec<Point> {
        let mut knots = vec![s0];
        let laps = (s0 / self.total).floor() as i64..=(s1 / self.total).ceil() as i64;
        for lap in laps {
            for g in &self.segs {
                for b in [g.start, g.start + g.len] {
                    let k = b + lap as f64 * self.total;
                    if k > s0 + 1e-12 && k < s1 - 1e-12 {
                        knots.push(k);
                    }
                }
            }
        }
        knots.push(s1);
        knots.sort_by(f64::total_cmp);
        knots.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        let mut out = vec![self.at(knots[0], off)];
        for w in knots.windows(2) {
            let (u, v) = (w[0], w[1]);
            if let Piece::Arc { radius, .. } = self.seg_at((u + v) / 2.0).piece {
                let big = radius + off.abs();
                let step = 2.0 * (1.0 - ARC_TOL_MM / big).clamp(-1.0, 1.0).acos().max(0.02);
                let n = (((v - u) / radius) / step).ceil().max(1.0) as usize;
                for i in 1..n {
                    out.push(self.at(u + (v - u) * i as f64 / n as f64, off));
                }
            }
            out.push(self.at(v, off));
        }
        out
    }
}

/// Perímetro de la línea media del lateral de estos parámetros (el largo de la tira, sin las
/// colas del cierre).
pub fn mid_perimeter(p: &RoundedBoxParams) -> f64 {
    let t = p.thickness_mm;
    let r = p.radius_mm - t / 2.0;
    2.0 * ((p.width_mm - t) + (p.depth_mm - t)) - 8.0 * r + TAU * r
}

/// Medidas ya validadas.
struct Layout {
    contour: Contour,
    /// Número de celdas (par).
    cells: usize,
    /// Alto de la zona del lateral entre la muesca inferior y la superior.
    zone_y: (f64, f64),
    joints: Option<JointsParams>,
}

impl Layout {
    fn cell_width(&self) -> f64 {
        self.contour.total / self.cells as f64
    }
}

impl RoundedBoxParams {
    fn hinge(&self, arc_len: f64, height: f64) -> HingeParams {
        HingeParams {
            width_mm: arc_len,
            height_mm: height,
            pattern: self.pattern,
            orientation: Orientation::Vertical,
            cut_length_mm: self.cut_length_mm,
            bridge_mm: self.bridge_mm,
            row_gap_mm: self.row_gap_mm,
            margin_mm: self.margin_mm,
            amplitude_mm: self.amplitude_mm,
            wavelength_mm: self.wavelength_mm,
            corner_radius_mm: 0.0,
            outline: false,
        }
    }

    fn layout(&self) -> Result<Layout, PartsError> {
        let t = self.thickness_mm;
        range("thickness_mm", t, 0.5, 20.0)?;
        range("width_mm", self.width_mm, 20.0, 2000.0)?;
        range("depth_mm", self.depth_mm, 20.0, 2000.0)?;
        range("height_mm", self.height_mm, 15.0, 500.0)?;
        range("radius_mm", self.radius_mm, 5.0, 500.0)?;
        range("finger_mm", self.finger_mm, 3.0, 200.0)?;
        range("fit_mm", self.fit_mm, -0.5, 1.0)?;
        self.hinge(10.0, 10.0).validate_cuts()?;
        // El radio interior (R - T) debe quedar con curva, y las curvas caber en el rectángulo.
        if self.radius_mm < t + 2.0 {
            return Err(PartsError::field(
                "radius_mm",
                tr!(
                    "parts.rounded.radius_small",
                    min = format!("{:.1}", t + 2.0)
                ),
            ));
        }
        if self.radius_mm > self.width_mm.min(self.depth_mm) / 2.0 {
            return Err(PartsError::field(
                "radius_mm",
                tr!("parts.rounded.radius_big"),
            ));
        }
        let r_mid = self.radius_mm - t / 2.0;
        let contour = Contour::new(self.width_mm - t, self.depth_mm - t, r_mid);
        let per = contour.total;
        let cells = ((per / self.finger_mm / 2.0).round() as usize * 2).max(6);
        if cells > MAX_CELLS {
            return Err(PartsError::field(
                "finger_mm",
                tr!("parts.rounded.too_many_fingers", max = MAX_CELLS),
            ));
        }
        if per / cells as f64 <= MIN_CELL_MM {
            return Err(PartsError::field(
                "finger_mm",
                tr!("parts.rounded.cells_too_narrow"),
            ));
        }
        // Alto libre del lateral entre las muescas de la base y de la tapa.
        let y0 = t;
        let y1 = self.height_mm - if self.lid { t } else { 0.0 };
        if y1 - y0 < 8.0 + 2.0 * SEAM_GAP_MM {
            return Err(PartsError::field(
                "height_mm",
                tr!(
                    "parts.rounded.too_short",
                    min = format!(
                        "{:.1}",
                        8.0 + 2.0 * SEAM_GAP_MM + y0 + (self.height_mm - y1)
                    )
                ),
            ));
        }
        // La bisagra de cada esquina: debe caber el patrón.
        let arc_len = r_mid * FRAC_PI_2;
        if arc_len < 5.0 {
            return Err(PartsError::field(
                "radius_mm",
                tr!(
                    "parts.rounded.radius_small",
                    min = format!("{:.1}", t + 2.0)
                ),
            ));
        }
        let joints = match self.seam {
            Seam::Butt => None,
            Seam::Dovetail => {
                let span = y1 - y0 - 2.0 * SEAM_GAP_MM;
                let count: i32 = if span >= 30.0 { 2 } else { 1 };
                let pitch = span / f64::from(count);
                let neck = (0.3 * pitch).clamp(4.0, 10.0);
                let j = JointsParams {
                    kind: JointKind::Dovetail,
                    length_mm: span,
                    plate_mm: 1000.0,
                    count,
                    neck_mm: neck,
                    depth_mm: neck,
                    angle_deg: 15.0,
                    fit_mm: self.fit_mm,
                    ..JointsParams::default()
                };
                let s = j.dovetail_shape();
                if 2.0 * s.slot_tip_half > pitch - 1.5 {
                    return Err(PartsError::field(
                        "height_mm",
                        tr!(
                            "parts.rounded.too_short",
                            min = format!(
                                "{:.1}",
                                8.0 + 2.0 * SEAM_GAP_MM + y0 + (self.height_mm - y1) + 6.0
                            )
                        ),
                    ));
                }
                // Las colas y las ranuras del cierre no deben pisar la bisagra de la esquina.
                let straight = (self.width_mm - t) / 2.0 - r_mid;
                if straight < s.slot_depth + SEAM_GAP_MM {
                    return Err(PartsError::field(
                        "width_mm",
                        tr!(
                            "parts.rounded.seam_no_room",
                            need = format!("{:.1}", 2.0 * (s.slot_depth + SEAM_GAP_MM))
                        ),
                    ));
                }
                Some(j)
            }
        };
        Ok(Layout {
            contour,
            cells,
            zone_y: (y0, y1),
            joints,
        })
    }
}

// ---- piezas -------------------------------------------------------------------------------------

/// Quita puntos repetidos y colineales de un contorno cerrado.
fn tidy(points: Vec<Point>) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(points.len());
    for p in points {
        if out
            .last()
            .is_none_or(|q| (q.x - p.x).hypot(q.y - p.y) > 1e-9)
        {
            out.push(p);
        }
    }
    while out.len() > 1 && {
        let (a, b) = (out[0], out[out.len() - 1]);
        (a.x - b.x).hypot(a.y - b.y) <= 1e-9
    } {
        out.pop();
    }
    // Colineales: el producto vectorial casi nulo con la misma dirección.
    let n = out.len();
    let keep: Vec<Point> = (0..n)
        .filter(|&i| {
            let (a, b, c) = (out[(i + n - 1) % n], out[i], out[(i + 1) % n]);
            let cross = (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x);
            let dot = (b.x - a.x) * (c.x - b.x) + (b.y - a.y) * (c.y - b.y);
            !(cross.abs() < 1e-9 && dot > 0.0)
        })
        .map(|i| out[i])
        .collect();
    keep
}

/// Base (o tapa): el contorno exterior con una muesca de ancho `T` por cada diente del lateral.
fn plate_outline(p: &RoundedBoxParams, lay: &Layout) -> Polyline {
    let t = p.thickness_mm;
    let e = p.fit_mm / 2.0;
    let cw = lay.cell_width();
    let c = &lay.contour;
    let mut pts: Vec<Point> = Vec::new();
    for k in 0..lay.cells {
        let (s0, s1) = (k as f64 * cw, (k + 1) as f64 * cw);
        if k % 2 == 1 {
            // Lengüeta de la placa: sigue el contorno exterior.
            pts.extend(c.walk(s0 + e, s1 - e, t / 2.0));
        } else {
            // Muesca para un diente del lateral: hacia adentro `T`, a lo largo, y de vuelta.
            pts.push(c.at(s0 - e, t / 2.0));
            pts.extend(c.walk(s0 - e, s1 + e, -t / 2.0));
            pts.push(c.at(s1 + e, t / 2.0));
        }
    }
    Polyline {
        points: tidy(pts),
        closed: true,
    }
}

/// El lateral plano: contorno con dientes arriba y abajo y el cierre, más los cortes de la
/// bisagra en cada esquina. Coordenadas: X a lo largo de la línea media (cierre en `x = 0` y
/// `x = L`), Y de abajo hacia arriba.
fn wall_paths(
    p: &RoundedBoxParams,
    lay: &Layout,
) -> Result<(Vec<Polyline>, Vec<String>), PartsError> {
    let (t, h) = (p.thickness_mm, p.height_mm);
    let cw = lay.cell_width();
    let l = lay.contour.total;
    let (y0, y1) = lay.zone_y;
    let top = |tooth: bool| {
        if !p.lid || tooth { h } else { h - t }
    };
    let mut pts: Vec<Point> = Vec::new();
    // Borde inferior, de izquierda a derecha: dientes (pares) llegan a y = 0.
    for k in 0..lay.cells {
        let y = if k % 2 == 0 { 0.0 } else { t };
        pts.push(Point::new(k as f64 * cw, y));
        pts.push(Point::new((k + 1) as f64 * cw, y));
    }
    // Borde derecho, hacia arriba, con las ranuras del cierre.
    let seam = lay.joints.as_ref().map(|j| (j, j.dovetail_shape()));
    if let Some((j, s)) = &seam {
        for i in 0..j.count {
            let c = y0 + SEAM_GAP_MM + j.center(i);
            pts.extend([
                Point::new(l, c - s.slot_neck_half),
                Point::new(l - s.slot_depth, c - s.slot_tip_half),
                Point::new(l - s.slot_depth, c + s.slot_tip_half),
                Point::new(l, c + s.slot_neck_half),
            ]);
        }
    }
    // Borde superior, de derecha a izquierda.
    for k in (0..lay.cells).rev() {
        let y = top(k % 2 == 0);
        pts.push(Point::new((k + 1) as f64 * cw, y));
        pts.push(Point::new(k as f64 * cw, y));
    }
    // Borde izquierdo, hacia abajo, con las colas del cierre.
    if let Some((j, s)) = &seam {
        for i in (0..j.count).rev() {
            let c = y0 + SEAM_GAP_MM + j.center(i);
            pts.extend([
                Point::new(0.0, c + s.neck_half),
                Point::new(-s.depth, c + s.tip_half),
                Point::new(-s.depth, c - s.tip_half),
                Point::new(0.0, c - s.neck_half),
            ]);
        }
    }
    let mut out = vec![Polyline {
        points: tidy(pts),
        closed: true,
    }];
    // Bisagra: una zona por cada arco, con los mismos cortes.
    let arcs: Vec<Zone> = lay
        .contour
        .zones()
        .into_iter()
        .filter(|z| z.kind == ZoneKind::Arc)
        .collect();
    let first = arcs.first().copied().unwrap_or(Zone {
        kind: ZoneKind::Arc,
        from: 0.0,
        to: 0.0,
    });
    let (cuts, warnings) = zone_cuts(&p.hinge(first.to - first.from, y1 - y0))?;
    for z in &arcs {
        for c in &cuts {
            out.push(Polyline {
                points: c
                    .points
                    .iter()
                    .map(|q| Point::new(q.x + z.from, q.y + y0))
                    .collect(),
                closed: false,
            });
        }
    }
    Ok((out, warnings))
}

/// Genera la caja redondeada: base, tapa (si hay) y el lateral con bisagra.
pub fn rounded_box_parts(p: &RoundedBoxParams) -> Result<Generated, PartsError> {
    let lay = p.layout()?;
    let (wall_cuts, mut warnings) = wall_paths(p, &lay)?;
    warnings.push(tr!(
        "parts.rounded.strip_note",
        len = format!("{:.1}", lay.contour.total),
        outer = format!("{:.1}", lay.contour.total + TAU * p.thickness_mm / 2.0)
    ));
    warnings.push(tr!("parts.rounded.hinge_note"));
    if p.fit_mm != 0.0 {
        warnings.push(tr!("parts.rounded.fit_vs_kerf"));
    }
    if lay.cell_width() < p.thickness_mm {
        warnings.push(tr!(
            "parts.rounded.thin_fingers",
            width = format!("{:.1}", lay.cell_width()),
            thickness = format!("{:.1}", p.thickness_mm)
        ));
    }
    let plate = plate_outline(p, &lay);
    let mut parts = vec![Part::new(
        "base",
        tr!("parts.rounded.name.base"),
        vec![plate.clone()],
        Vec::new(),
    )];
    if p.lid {
        parts.push(Part::new(
            "lid",
            tr!("parts.rounded.name.lid"),
            vec![plate],
            Vec::new(),
        ));
    }
    parts.push(Part::new(
        "wall",
        tr!("parts.rounded.name.wall"),
        wall_cuts,
        Vec::new(),
    ));
    Ok(Generated { parts, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rectilinear::signed_area;
    use crate::testutil::{crossings, inside};

    fn base() -> RoundedBoxParams {
        RoundedBoxParams::default()
    }

    fn wall(g: &Generated) -> &Part {
        g.parts.iter().find(|p| p.key == "wall").unwrap()
    }

    fn each_variant() -> Vec<RoundedBoxParams> {
        let mut v = vec![base()];
        for pattern in [
            HingePattern::Wave,
            HingePattern::Zigzag,
            HingePattern::Brick,
            HingePattern::Diamond,
        ] {
            v.push(RoundedBoxParams {
                pattern,
                row_gap_mm: 3.0,
                cut_length_mm: 12.0,
                ..base()
            });
        }
        v.push(RoundedBoxParams {
            width_mm: 83.3,
            depth_mm: 61.7,
            height_mm: 33.0,
            radius_mm: 11.0,
            thickness_mm: 2.5,
            finger_mm: 6.5,
            fit_mm: 0.1,
            lid: false,
            ..base()
        });
        v.push(RoundedBoxParams {
            seam: Seam::Butt,
            ..base()
        });
        v.push(RoundedBoxParams {
            width_mm: 60.0,
            depth_mm: 60.0,
            radius_mm: 30.0,
            seam: Seam::Butt,
            ..base()
        });
        v
    }

    /// Perímetro de la polilínea cerrada de la línea media, medido a partir de su geometría.
    fn sampled_perimeter(c: &Contour) -> f64 {
        let pts = c.walk(0.0, c.total, 0.0);
        pts.windows(2)
            .map(|w| (w[1].x - w[0].x).hypot(w[1].y - w[0].y))
            .sum()
    }

    #[test]
    fn strip_length_equals_the_perimeter_of_the_contour() {
        for p in each_variant() {
            let g = rounded_box_parts(&p).unwrap();
            let w = wall(&g);
            // Fórmula cerrada: 2(W' + D') - 8r + 2 pi r con las medidas de la línea media.
            let t = p.thickness_mm;
            let r = p.radius_mm - t / 2.0;
            let want = 2.0 * ((p.width_mm - t) + (p.depth_mm - t)) - 8.0 * r + TAU * r;
            assert!((mid_perimeter(&p) - want).abs() < 1e-9);
            // La tira mide eso entre los extremos del cierre (más las colas si las hay).
            let tails = match (p.seam, p.lid) {
                (Seam::Dovetail, _) => p.layout().unwrap().joints.unwrap().dovetail_shape().depth,
                (Seam::Butt, _) => 0.0,
            };
            assert!(
                (w.width - tails - want).abs() < 1e-6,
                "{p:?}: {} vs {want}",
                w.width - tails
            );
            // Y coincide con el perímetro medido sobre el contorno muestreado.
            let lay = p.layout().unwrap();
            assert!((lay.contour.total - want).abs() < 1e-9);
            assert!(
                (sampled_perimeter(&lay.contour) - want).abs() < want * 1e-3,
                "{p:?}"
            );
            // Alto de la tira = alto de la caja.
            assert!((w.height - p.height_mm).abs() < 1e-9);
        }
    }

    #[test]
    fn hinge_zones_coincide_with_the_arcs() {
        for p in each_variant() {
            let lay = p.layout().unwrap();
            let arcs: Vec<Zone> = lay
                .contour
                .zones()
                .into_iter()
                .filter(|z| z.kind == ZoneKind::Arc)
                .collect();
            assert_eq!(arcs.len(), 4);
            let r = p.radius_mm - p.thickness_mm / 2.0;
            let quarter = r * FRAC_PI_2;
            let straight_x = (p.width_mm - p.thickness_mm) / 2.0 - r;
            let side_y = (p.depth_mm - p.thickness_mm) - 2.0 * r;
            // Posiciones esperadas, calculadas con la geometría a mano.
            let a1 = straight_x;
            let a2 = a1 + quarter + side_y;
            let a3 = a2 + quarter + 2.0 * straight_x;
            let a4 = a3 + quarter + side_y;
            for (z, from) in arcs.iter().zip([a1, a2, a3, a4]) {
                assert!((z.from - from).abs() < 1e-9, "{p:?}: {z:?} vs {from}");
                assert!((z.to - z.from - quarter).abs() < 1e-9);
            }
            // Los cortes abiertos de la bisagra caen solo dentro de los arcos.
            let g = rounded_box_parts(&p).unwrap();
            let slits: Vec<&Polyline> = wall(&g).cuts.iter().filter(|c| !c.closed).collect();
            assert!(slits.len() > 8, "{p:?}");
            // La pieza se normaliza al origen: el cierre con colas desplaza `x` en `depth`.
            let shift = lay
                .joints
                .as_ref()
                .map_or(0.0, |j| j.dovetail_shape().depth);
            for s in &slits {
                let xs: Vec<f64> = s.points.iter().map(|q| q.x - shift).collect();
                let (lo, hi) = (
                    xs.iter().cloned().fold(f64::MAX, f64::min),
                    xs.iter().cloned().fold(f64::MIN, f64::max),
                );
                assert!(
                    arcs.iter()
                        .any(|z| lo >= z.from - 1e-9 && hi <= z.to + 1e-9),
                    "{p:?}: corte {lo}..{hi} fuera de los arcos"
                );
            }
            // Cada arco recibe cortes.
            for z in &arcs {
                assert!(
                    slits.iter().any(|s| {
                        let x = s.points[0].x - shift;
                        x >= z.from && x <= z.to
                    }),
                    "{p:?}: arco sin bisagra {z:?}"
                );
            }
        }
    }

    #[test]
    fn the_straight_zones_have_no_cuts_and_the_walls_do_not_self_cross() {
        for p in each_variant() {
            let g = rounded_box_parts(&p).unwrap();
            for part in &g.parts {
                assert_eq!(crossings(&part.cuts), 0, "{p:?}: {}", part.key);
            }
        }
    }

    #[test]
    fn plate_notches_match_the_wall_teeth_in_area() {
        for fit in [0.0, 0.1] {
            let p = RoundedBoxParams {
                fit_mm: fit,
                ..base()
            };
            let lay = p.layout().unwrap();
            let outline = plate_outline(&p, &lay);
            // Área de la placa llena (sin muescas), con el mismo muestreo del contorno exterior.
            let full = Polyline {
                points: lay
                    .contour
                    .walk(0.0, lay.contour.total, p.thickness_mm / 2.0),
                closed: true,
            };
            let removed = signed_area(&full) - signed_area(&outline);
            let n_teeth = lay.cells / 2;
            // Área de una muesca = T x (ancho de celda + fit): la línea media no se estira.
            let want = n_teeth as f64 * p.thickness_mm * (lay.cell_width() + fit);
            assert!(
                (removed - want).abs() < want * 0.01,
                "fit {fit}: quitado {removed} esperado {want}"
            );
            // Y el diente plano del lateral mide justo T x ancho de celda.
            let g = rounded_box_parts(&p).unwrap();
            let w = wall(&g);
            let outline_area = signed_area(&w.cuts[0]);
            let full_rect = lay.contour.total * p.height_mm;
            // Una muesca de T x ancho de celda por cada celda impar, arriba y abajo; las colas del
            // cierre suman y sus ranuras restan lo mismo (salvo la holgura).
            let missing = 2.0 * (lay.cells / 2) as f64 * p.thickness_mm * lay.cell_width();
            assert!(
                (outline_area - (full_rect - missing)).abs() < 12.0,
                "{} vs {}",
                outline_area,
                full_rect - missing
            );
        }
    }

    #[test]
    fn plate_stays_inside_the_outer_contour_and_notches_are_a_thickness_deep() {
        let p = base();
        let lay = p.layout().unwrap();
        let outline = plate_outline(&p, &lay);
        let (hx, hy) = (p.width_mm / 2.0, p.depth_mm / 2.0);
        // Distancia al centro del rectángulo con la esquina redondeada.
        let sdf = |q: &Point| {
            let (cx, cy) = (hx - p.radius_mm, hy - p.radius_mm);
            let (dx, dy) = ((q.x.abs() - cx).max(0.0), (q.y.abs() - cy).max(0.0));
            // distancia al borde exterior: positivo dentro.
            let outer_r = p.radius_mm;
            if q.x.abs() > cx && q.y.abs() > cy {
                outer_r - dx.hypot(dy)
            } else {
                (hx - q.x.abs()).min(hy - q.y.abs())
            }
        };
        let mut deepest: f64 = 0.0;
        for q in &outline.points {
            let d = sdf(q);
            assert!(d > -0.02, "fuera del contorno: {q:?} {d}");
            deepest = deepest.max(d);
        }
        assert!((deepest - p.thickness_mm).abs() < 0.02, "{deepest}");
        // Sale antihorario y las muescas dejan `n/2` entrantes.
        assert!(signed_area(&outline) > 0.0);
        let on_edge = outline.points.iter().filter(|q| sdf(q) < 0.02).count();
        assert!(on_edge > lay.cells);
    }

    #[test]
    fn closing_tails_fit_their_sockets() {
        // Sin holgura, desplazar la tira `L` hace que cada punto de la franja del cierre esté
        // en la cola o en la ranura, nunca en ambas ni en ninguna.
        let p = base();
        let lay = p.layout().unwrap();
        let g = rounded_box_parts(&p).unwrap();
        let w = wall(&g);
        let outline = std::slice::from_ref(&w.cuts[0]);
        let s = lay.joints.as_ref().unwrap().dovetail_shape();
        let l = lay.contour.total;
        let (y0, y1) = lay.zone_y;
        let n = 80;
        for i in 0..n {
            for j in 0..n {
                let x = -s.depth + (i as f64 + 0.37) * s.depth / n as f64;
                let y =
                    y0 + SEAM_GAP_MM + (j as f64 + 0.61) * (y1 - y0 - 2.0 * SEAM_GAP_MM) / n as f64;
                // La pieza ya está normalizada: su origen está en x = -depth.
                let (px, py) = (x + s.depth, y);
                let here = inside(outline, px, py);
                let there = inside(outline, px + l, py);
                assert!(here ^ there, "({x}, {y}): cola={here} ranura-libre={there}");
            }
        }
    }

    #[test]
    fn tooth_cells_alternate_and_are_symmetric_around_the_seam() {
        let p = RoundedBoxParams {
            seam: Seam::Butt,
            ..base()
        };
        let lay = p.layout().unwrap();
        assert_eq!(lay.cells % 2, 0);
        let g = rounded_box_parts(&p).unwrap();
        let w = wall(&g);
        // Segmentos horizontales del borde inferior: y = 0 (dientes) y y = T (entre dientes).
        let outline = &w.cuts[0];
        let t = p.thickness_mm;
        let teeth: Vec<(f64, f64)> = crate::testutil::segments(outline)
            .into_iter()
            .filter(|(a, b)| a.y.abs() < 1e-9 && b.y.abs() < 1e-9)
            .map(|(a, b)| (a.x.min(b.x), a.x.max(b.x)))
            .collect();
        assert_eq!(teeth.len(), lay.cells / 2);
        for (a, b) in &teeth {
            assert!((b - a - lay.cell_width()).abs() < 1e-9);
        }
        let notches = crate::testutil::segments(outline)
            .into_iter()
            .filter(|(a, b)| (a.y - t).abs() < 1e-9 && (b.y - t).abs() < 1e-9)
            .count();
        assert_eq!(notches, lay.cells / 2);
        // El primer diente empieza en el cierre.
        assert!(teeth.iter().any(|(a, _)| a.abs() < 1e-9));
    }

    #[test]
    fn open_box_has_no_lid_and_a_straight_top() {
        let p = RoundedBoxParams {
            lid: false,
            seam: Seam::Butt,
            ..base()
        };
        let g = rounded_box_parts(&p).unwrap();
        assert_eq!(g.parts.len(), 2);
        assert!(g.parts.iter().all(|x| x.key != "lid"));
        let w = wall(&g);
        let top = w.cuts[0]
            .points
            .iter()
            .filter(|q| (q.y - p.height_mm).abs() < 1e-9)
            .count();
        assert_eq!(top, 2);
        // Con tapa hay una muesca en cada celda impar y diente en cada par.
        let with_lid = rounded_box_parts(&RoundedBoxParams {
            seam: Seam::Butt,
            ..base()
        })
        .unwrap();
        assert_eq!(with_lid.parts.len(), 3);
    }

    #[test]
    fn invalid_values_name_the_field() {
        let bad = |p: RoundedBoxParams| rounded_box_parts(&p).unwrap_err().field;
        assert_eq!(
            bad(RoundedBoxParams {
                radius_mm: 2.0,
                ..base()
            })
            .as_deref(),
            Some("radius_mm")
        );
        assert_eq!(
            bad(RoundedBoxParams {
                radius_mm: 40.0,
                depth_mm: 60.0,
                ..base()
            })
            .as_deref(),
            Some("radius_mm")
        );
        assert_eq!(
            bad(RoundedBoxParams {
                radius_mm: 4.0,
                thickness_mm: 3.0,
                ..base()
            })
            .as_deref(),
            Some("radius_mm")
        );
        assert_eq!(
            bad(RoundedBoxParams {
                height_mm: 16.0,
                ..base()
            })
            .as_deref(),
            Some("height_mm")
        );
        assert_eq!(
            bad(RoundedBoxParams {
                thickness_mm: -1.0,
                ..base()
            })
            .as_deref(),
            Some("thickness_mm")
        );
        assert_eq!(
            bad(RoundedBoxParams {
                width_mm: 2000.0,
                depth_mm: 2000.0,
                finger_mm: 3.0,
                ..base()
            })
            .as_deref(),
            Some("finger_mm")
        );
        // La bisagra elegida no cabe: el error nombra el campo de la bisagra.
        assert_eq!(
            bad(RoundedBoxParams {
                pattern: HingePattern::Diamond,
                cut_length_mm: 100.0,
                row_gap_mm: 0.5,
                ..base()
            })
            .as_deref(),
            Some("bridge_mm")
        );
        // Un lado recto muy corto no deja sitio al cierre en cola de milano.
        assert!(
            bad(RoundedBoxParams {
                width_mm: 40.0,
                radius_mm: 19.0,
                ..base()
            })
            .is_some()
        );
    }

    #[test]
    fn warnings_explain_the_strip_length() {
        let g = rounded_box_parts(&base()).unwrap();
        let len = format!("{:.1}", mid_perimeter(&base()));
        assert!(
            g.warnings.iter().any(|w| w.contains(&len)),
            "{:?}",
            g.warnings
        );
    }
}
