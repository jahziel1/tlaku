//! Engranajes rectos de evolvente: uno solo, un par (engranaje + engranaje o piñón + corona
//! interior), una cremallera o un piñón con su cremallera, colocados engranando.
//!
//! # Diseño
//!
//! - **Tamaño:** módulo `m` (mm) o paso diametral `DP` (`m = 25,4/DP`). Radio primitivo
//!   `r = m·z/2`, exterior `m·(z+2)/2`, fondo `r − m·(1 + c)` con `c` el coeficiente de holgura
//!   de cabeza (0,25 por omisión). La geometría del perfil está en [`involute`].
//! - **Juego (backlash):** adelgaza cada diente `b/2` sobre el arco del círculo primitivo, de modo
//!   que el par tiene un juego circunferencial `b` con la distancia entre centros **exacta**
//!   `m·(z1+z2)/2` (`m·(z2−z1)/2` en la corona). Con `b = 0` los flancos se tocan.
//! - **Engrane:** el engranaje 1 tiene un diente centrado hacia el 2 (a lo largo de +X) y el 2 un
//!   hueco hacia el 1; en la corona el piñón y el hueco de la corona coinciden en +X; el piñón
//!   con cremallera tiene la cremallera debajo (su línea primitiva tangente) con un diente de
//!   la cremallera bajo el centro y un hueco del piñón mirando hacia abajo.
//! - **Corona (engranaje interior):** el hueco de la corona es el perfil de un engranaje exterior
//!   con cabeza y pie intercambiados (cabeza `m(1+c)` hacia afuera, pie `m` hacia adentro) y el
//!   espesor del diente `p/2 + b/2`: el piñón cabe en él con juego `b`. Sin filete.
//! - **Socavado:** no se simula el tallado; por debajo del círculo base el flanco sigue radial.
//!   Con menos de `2/sin²α` dientes (17 a 20°) se avisa y se sugiere el mínimo.
//! - **Agujero y aligeramiento:** agujero redondo, con chavetero o en D; agujeros redondos o
//!   radios (aberturas entre rayos de ancho constante) entre el cubo y el pie, ajustados solos.

mod involute;
#[cfg(test)]
mod tests;

use crate::error::PartsError;
use crate::part::{Generated, Part};
use crate::registry::{ParamKind, ParamSpec};
use involute::{Profile, ProfileError, arc, circle, rack_outline};
use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};
use tlaku_core::{Point, Polyline};
use tlaku_i18n::tr;

/// Qué se genera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Un engranaje.
    Single,
    /// Dos engranajes exteriores que engranan.
    Pair,
    /// Piñón y corona (engranaje interior).
    Ring,
    /// Una cremallera.
    Rack,
    /// Piñón y cremallera.
    RackPinion,
}

/// Cómo se da el tamaño de los dientes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SizeBy {
    Module,
    DiametralPitch,
}

/// Ángulo de presión.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PressureAngle {
    #[serde(rename = "deg14_5")]
    Deg14_5,
    #[serde(rename = "deg20")]
    Deg20,
    #[serde(rename = "deg25")]
    Deg25,
}

impl PressureAngle {
    pub fn degrees(self) -> f64 {
        match self {
            Self::Deg14_5 => 14.5,
            Self::Deg20 => 20.0,
            Self::Deg25 => 25.0,
        }
    }
}

/// Pie del diente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Root {
    /// Con filete tangente (menos concentración de esfuerzos).
    Fillet,
    /// Flanco y círculo de pie en esquina viva.
    Simple,
}

/// Forma del agujero central.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoreType {
    Round,
    /// Con chavetero rectangular.
    Keyway,
    /// Eje en D (un plano).
    DFlat,
}

/// Aligeramiento del disco.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lightening {
    None,
    /// Agujeros redondos.
    Holes,
    /// Radios: aberturas entre rayos de ancho constante.
    Spokes,
}

/// Cómo se entregan las piezas de un conjunto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    /// Un solo objeto con las piezas engranando.
    Meshed,
    /// Una pieza por engranaje, separadas en filas.
    Separate,
}

/// Parámetros del generador de engranajes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GearParams {
    pub kind: Kind,
    pub size_by: SizeBy,
    pub module_mm: f64,
    /// Dientes por pulgada de diámetro primitivo (solo con `size_by = diametral_pitch`).
    pub diametral_pitch: f64,
    /// Dientes del engranaje (el piñón en pares, coronas y cremallera con piñón).
    pub teeth: i32,
    /// Dientes del segundo engranaje o de la corona.
    pub teeth2: i32,
    pub rack_teeth: i32,
    pub pressure_angle: PressureAngle,
    /// Holgura de cabeza como fracción del módulo (la cabeza del conjugado no toca el fondo).
    pub clearance: f64,
    /// Juego circunferencial entre dientes (mm), repartido entre los dos engranajes.
    pub backlash_mm: f64,
    pub root: Root,
    /// Flecha máxima de la aproximación de las curvas (mm).
    pub tolerance_mm: f64,
    /// Diámetro del agujero central (0 = sin agujero).
    pub bore_mm: f64,
    /// Agujero del segundo engranaje de un par.
    pub bore2_mm: f64,
    pub bore_type: BoreType,
    pub key_width_mm: f64,
    /// Cuánto sobresale el chavetero del radio del agujero.
    pub key_depth_mm: f64,
    /// Profundidad del plano del eje en D.
    pub flat_depth_mm: f64,
    pub lightening: Lightening,
    pub lightening_count: i32,
    /// Material mínimo: pared del cubo y del aro, y ancho de los rayos.
    pub web_mm: f64,
    /// Espesor del aro exterior de la corona bajo el fondo de sus dientes.
    pub ring_rim_mm: f64,
    /// Cuerpo de la cremallera bajo el fondo de los dientes.
    pub rack_base_mm: f64,
    pub layout: Layout,
}

impl Default for GearParams {
    fn default() -> Self {
        Self {
            kind: Kind::Single,
            size_by: SizeBy::Module,
            module_mm: 2.0,
            diametral_pitch: 12.0,
            teeth: 20,
            teeth2: 40,
            rack_teeth: 10,
            pressure_angle: PressureAngle::Deg20,
            clearance: 0.25,
            backlash_mm: 0.1,
            root: Root::Fillet,
            tolerance_mm: 0.01,
            bore_mm: 5.0,
            bore2_mm: 5.0,
            bore_type: BoreType::Round,
            key_width_mm: 2.0,
            key_depth_mm: 1.0,
            flat_depth_mm: 0.5,
            lightening: Lightening::None,
            lightening_count: 5,
            web_mm: 4.0,
            ring_rim_mm: 6.0,
            rack_base_mm: 8.0,
            layout: Layout::Meshed,
        }
    }
}

fn number(key: &str, section: &str, min: f64, max: f64, step: f64, default: f64) -> ParamSpec {
    ParamSpec {
        key: key.to_string(),
        kind: ParamKind::Number {
            min,
            max,
            step,
            default,
        },
        section: section.to_string(),
        when: None,
    }
}

/// Campos del formulario de los engranajes.
pub fn schema() -> Vec<ParamSpec> {
    let d = GearParams::default();
    let with_partner = ["pair", "ring"];
    let with_rack = ["rack", "rack_pinion"];
    let with_gear = ["single", "pair", "ring", "rack_pinion"];
    let assembled = ["pair", "ring", "rack_pinion"];
    let keyed = ["keyway"];
    let lit = ["holes", "spokes"];
    vec![
        ParamSpec::choice(
            "kind",
            "gear_main",
            &["single", "pair", "ring", "rack", "rack_pinion"],
            "single",
        ),
        ParamSpec::choice(
            "size_by",
            "gear_main",
            &["module", "diametral_pitch"],
            "module",
        ),
        ParamSpec::length("module_mm", "gear_main", 0.2, 25.0, 0.1, d.module_mm)
            .when("size_by", &["module"]),
        number(
            "diametral_pitch",
            "gear_main",
            1.0,
            127.0,
            0.5,
            d.diametral_pitch,
        )
        .when("size_by", &["diametral_pitch"]),
        ParamSpec::int("teeth", "gear_main", 6, 200, d.teeth).when("kind", &with_gear),
        ParamSpec::int("teeth2", "gear_main", 6, 400, d.teeth2).when("kind", &with_partner),
        ParamSpec::int("rack_teeth", "gear_main", 2, 200, d.rack_teeth).when("kind", &with_rack),
        ParamSpec::choice(
            "pressure_angle",
            "gear_main",
            &["deg14_5", "deg20", "deg25"],
            "deg20",
        ),
        number("clearance", "gear_tooth", 0.05, 0.5, 0.01, d.clearance),
        ParamSpec::length("backlash_mm", "gear_tooth", 0.0, 2.0, 0.01, d.backlash_mm),
        ParamSpec::choice("root", "gear_tooth", &["fillet", "simple"], "fillet"),
        ParamSpec::length(
            "tolerance_mm",
            "gear_tooth",
            0.002,
            0.1,
            0.002,
            d.tolerance_mm,
        ),
        ParamSpec::length("bore_mm", "gear_bore", 0.0, 500.0, 0.5, d.bore_mm)
            .when("kind", &with_gear),
        ParamSpec::length("bore2_mm", "gear_bore", 0.0, 500.0, 0.5, d.bore2_mm)
            .when("kind", &["pair"]),
        ParamSpec::choice(
            "bore_type",
            "gear_bore",
            &["round", "keyway", "d_flat"],
            "round",
        )
        .when("kind", &with_gear),
        ParamSpec::length("key_width_mm", "gear_bore", 0.5, 50.0, 0.5, d.key_width_mm)
            .when("bore_type", &keyed),
        ParamSpec::length("key_depth_mm", "gear_bore", 0.2, 25.0, 0.1, d.key_depth_mm)
            .when("bore_type", &keyed),
        ParamSpec::length(
            "flat_depth_mm",
            "gear_bore",
            0.1,
            100.0,
            0.1,
            d.flat_depth_mm,
        )
        .when("bore_type", &["d_flat"]),
        ParamSpec::choice(
            "lightening",
            "gear_light",
            &["none", "holes", "spokes"],
            "none",
        )
        .when("kind", &with_gear),
        ParamSpec::int("lightening_count", "gear_light", 3, 24, d.lightening_count)
            .when("lightening", &lit),
        ParamSpec::length("web_mm", "gear_light", 1.0, 50.0, 0.5, d.web_mm)
            .when("lightening", &lit),
        ParamSpec::length("ring_rim_mm", "gear_layout", 2.0, 100.0, 0.5, d.ring_rim_mm)
            .when("kind", &["ring"]),
        ParamSpec::length(
            "rack_base_mm",
            "gear_layout",
            2.0,
            200.0,
            0.5,
            d.rack_base_mm,
        )
        .when("kind", &with_rack),
        ParamSpec::choice("layout", "gear_layout", &["meshed", "separate"], "meshed")
            .when("kind", &assembled),
    ]
}

/// Diámetro exterior máximo de un engranaje y largo máximo de una cremallera (mm).
const MAX_SIZE_MM: f64 = 2000.0;
/// Pared mínima (mm) entre el agujero y el fondo de los dientes.
const MIN_WALL_MM: f64 = 1.0;
/// Hueco mínimo (mm) de una abertura de aligeramiento.
const MIN_OPENING_MM: f64 = 2.0;
/// Si la corona tiene menos dientes de diferencia con el piñón, los dientes pueden interferir.
const MIN_RING_DIFF: i32 = 8;

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn range(field: &str, v: f64, lo: f64, hi: f64) -> Result<(), PartsError> {
    if v.is_nan() || !(lo..=hi).contains(&v) {
        return Err(PartsError::field(
            field,
            tr!("parts.gears.range", min = lo, max = hi),
        ));
    }
    Ok(())
}

/// Dientes mínimos sin socavado: `2/sin²α`, redondeado (17 a 20°, 32 a 14,5°, 11 a 25°).
pub fn min_teeth(alpha: f64) -> u32 {
    (2.0 / alpha.sin().powi(2)).round() as u32
}

/// Datos comunes de todos los engranajes del conjunto.
struct Ctx {
    module: f64,
    alpha: f64,
    /// Holgura de cabeza absoluta (mm).
    clearance: f64,
    backlash: f64,
    fillet: bool,
    tol: f64,
}

impl Ctx {
    fn pitch(&self) -> f64 {
        PI * self.module
    }

    /// Espesor del diente exterior sobre el círculo primitivo.
    fn thickness(&self) -> f64 {
        self.pitch() / 2.0 - self.backlash / 2.0
    }

    fn external(&self, teeth: u32) -> Profile {
        Profile {
            teeth,
            module: self.module,
            alpha: self.alpha,
            tip_radius: self.module * (f64::from(teeth) / 2.0 + 1.0),
            root_radius: self.module * (f64::from(teeth) / 2.0 - 1.0) - self.clearance,
            thickness: self.thickness(),
            fillet_max: if self.fillet { 0.3 * self.module } else { 0.0 },
            clearance: self.clearance,
            tolerance: self.tol,
        }
    }

    /// Hueco de la corona: engranaje exterior con cabeza y pie intercambiados.
    fn ring_void(&self, teeth: u32) -> Profile {
        let r = self.module * f64::from(teeth) / 2.0;
        Profile {
            teeth,
            module: self.module,
            alpha: self.alpha,
            tip_radius: r + self.module + self.clearance,
            root_radius: r - self.module,
            thickness: self.pitch() / 2.0 + self.backlash / 2.0,
            fillet_max: 0.0,
            clearance: 0.0,
            tolerance: self.tol,
        }
    }
}

fn profile_error(e: ProfileError, ctx: &Ctx, field: &str, teeth: u32) -> PartsError {
    match e {
        ProfileError::PointedTip => PartsError::field(
            field,
            tr!(
                "parts.gears.pointed",
                teeth = teeth,
                angle = ctx.alpha.to_degrees().round()
            ),
        ),
        ProfileError::Overlap => PartsError::general(tr!("parts.gears.overlap")),
        ProfileError::Invalid => PartsError::general(tr!("parts.gears.invalid")),
    }
}

fn poly(points: Vec<Point>, center: Point) -> Polyline {
    Polyline {
        points: points
            .into_iter()
            .map(|p| Point::new(p.x + center.x, p.y + center.y))
            .collect(),
        closed: true,
    }
}

/// Agujero central centrado en el origen.
fn bore_shape(p: &GearParams, diameter: f64, tol: f64) -> Vec<Point> {
    let r = diameter / 2.0;
    let o = Point::new(0.0, 0.0);
    match p.bore_type {
        BoreType::Round => circle(o, r, tol),
        BoreType::Keyway => {
            // Ranura hacia +Y: se construye hacia +X y se gira 90°.
            let half = p.key_width_mm / 2.0;
            let beta = (half / r).asin();
            let mut pts = arc(o, r, beta, TAU - beta, tol);
            let x = r + p.key_depth_mm;
            pts.push(Point::new(x, -half));
            pts.push(Point::new(x, half));
            pts.into_iter().map(|q| Point::new(-q.y, q.x)).collect()
        }
        BoreType::DFlat => {
            let x = r - p.flat_depth_mm;
            let gamma = (x / r).acos();
            arc(o, r, gamma, TAU - gamma, tol)
        }
    }
}

/// Cuánto sobresale el agujero (con su chavetero) del radio nominal.
fn bore_extent(p: &GearParams, diameter: f64) -> f64 {
    if diameter <= 0.0 {
        return 0.0;
    }
    let r = diameter / 2.0;
    match p.bore_type {
        BoreType::Keyway => r + p.key_depth_mm,
        _ => r,
    }
}

/// Aligeramiento entre el cubo (`hub`) y el aro (`rim`): contornos cerrados, o `None` si no cabe.
fn lightening_shapes(
    p: &GearParams,
    hub: f64,
    rim: f64,
    center: Point,
    tol: f64,
) -> Option<Vec<Polyline>> {
    let n = p.lightening_count.max(3) as usize;
    let width = rim - hub;
    if width < MIN_OPENING_MM {
        return None;
    }
    let step = TAU / n as f64;
    let mut out = Vec::new();
    match p.lightening {
        Lightening::None => return Some(out),
        Lightening::Holes => {
            let rc = (hub + rim) / 2.0;
            let d = width.min(2.0 * rc * (step / 2.0).sin() - p.web_mm);
            if d < MIN_OPENING_MM {
                return None;
            }
            for i in 0..n {
                let a = step * i as f64 + step / 2.0;
                let c = Point::new(center.x + rc * a.cos(), center.y + rc * a.sin());
                out.push(Polyline {
                    points: circle(c, d / 2.0, tol),
                    closed: true,
                });
            }
        }
        Lightening::Spokes => {
            let half = p.web_mm / 2.0;
            let (s_in, s_out) = ((half / hub).asin(), (half / rim).asin());
            if hub * (step - 2.0 * s_in) < MIN_OPENING_MM {
                return None;
            }
            for i in 0..n {
                let a0 = step * i as f64;
                let a1 = a0 + step;
                let o = Point::new(0.0, 0.0);
                let mut pts = arc(o, hub, a0 + s_in, a1 - s_in, tol);
                let mut outer = arc(o, rim, a0 + s_out, a1 - s_out, tol);
                outer.reverse();
                pts.extend(outer);
                out.push(poly(pts, center));
            }
        }
    }
    Some(out)
}

/// Un engranaje exterior terminado: contorno, agujero y aligeramiento.
struct Built {
    contours: Vec<Polyline>,
}

/// Dónde y cómo va un engranaje exterior.
struct Slot {
    teeth: u32,
    /// Diámetro del agujero central (0 = sin agujero).
    bore: f64,
    /// Campos del formulario que se marcan si falla el diente o el agujero.
    tooth_field: &'static str,
    bore_field: &'static str,
    center: Point,
    /// Ángulo del centro del diente 0 (radianes).
    rotation: f64,
}

fn external_gear(
    ctx: &Ctx,
    p: &GearParams,
    slot: Slot,
    warnings: &mut Vec<String>,
) -> Result<Built, PartsError> {
    let Slot {
        teeth,
        bore,
        tooth_field,
        bore_field,
        center,
        rotation,
    } = slot;
    let prof = ctx.external(teeth);
    if 2.0 * prof.tip_radius > MAX_SIZE_MM {
        return Err(PartsError::general(tr!(
            "parts.gears.too_big",
            max = MAX_SIZE_MM
        )));
    }
    let min = min_teeth(ctx.alpha);
    if teeth < min {
        warnings.push(tr!(
            "parts.gears.undercut",
            teeth = teeth,
            angle = ctx.alpha.to_degrees(),
            min = min
        ));
    }
    let tip_width = 2.0 * prof.tip_radius * prof.half_angle(prof.tip_radius);
    if tip_width > 0.0 && tip_width < 0.2 * ctx.module {
        warnings.push(tr!("parts.gears.thin_tip", width = round2(tip_width)));
    }
    let outline = prof
        .outline(rotation)
        .map_err(|e| profile_error(e, ctx, tooth_field, teeth))?;
    let mut contours = vec![poly(outline, center)];

    let root = prof.root_radius;
    if bore > 0.0 {
        if bore < 0.5 {
            return Err(PartsError::field(
                bore_field,
                tr!("parts.gears.bore_min", min = 0.5),
            ));
        }
        let r = bore / 2.0;
        match p.bore_type {
            BoreType::Keyway => {
                if p.key_width_mm >= bore * 0.9 {
                    return Err(PartsError::field(
                        "key_width_mm",
                        tr!("parts.gears.key_wide", max = round2(bore * 0.9)),
                    ));
                }
            }
            BoreType::DFlat => {
                if p.flat_depth_mm >= r * 0.9 {
                    return Err(PartsError::field(
                        "flat_depth_mm",
                        tr!("parts.gears.flat_deep", max = round2(r * 0.9)),
                    ));
                }
            }
            BoreType::Round => {}
        }
        let extent = bore_extent(p, bore);
        let wall = root - extent;
        if wall < MIN_WALL_MM {
            return Err(PartsError::field(
                bore_field,
                tr!(
                    "parts.gears.bore_big",
                    wall = round2(wall),
                    min = MIN_WALL_MM
                ),
            ));
        }
        contours.push(poly(bore_shape(p, bore, ctx.tol), center));
    }

    if p.lightening != Lightening::None {
        let hub = bore_extent(p, bore) + p.web_mm;
        let rim = root - p.web_mm;
        match lightening_shapes(p, hub, rim, center, ctx.tol) {
            Some(shapes) => contours.extend(shapes),
            None => warnings.push(tr!("parts.gears.no_room", teeth = teeth)),
        }
    }
    Ok(Built { contours })
}

/// Corona: aro circular con el hueco dentado.
fn ring_gear(
    ctx: &Ctx,
    p: &GearParams,
    teeth: u32,
    center: Point,
) -> Result<(Built, Profile), PartsError> {
    let prof = ctx.ring_void(teeth);
    let outer = prof.tip_radius + p.ring_rim_mm;
    if 2.0 * outer > MAX_SIZE_MM {
        return Err(PartsError::general(tr!(
            "parts.gears.too_big",
            max = MAX_SIZE_MM
        )));
    }
    let hole = prof
        .outline(0.0)
        .map_err(|e| profile_error(e, ctx, "teeth2", teeth))?;
    let contours = vec![
        Polyline {
            points: circle(center, outer, ctx.tol),
            closed: true,
        },
        poly(hole, center),
    ];
    Ok((Built { contours }, prof))
}

fn path_of_contact(tip: f64, base: f64) -> f64 {
    (tip * tip - base * base).max(0.0).sqrt()
}

/// Avisa si la relación de contacto transversal (largo del camino de contacto entre el paso
/// base) es baja: con menos de 1,2 el par engrana a tirones.
fn warn_contact(ctx: &Ctx, line: f64, warnings: &mut Vec<String>) {
    let eps = line / (PI * ctx.module * ctx.alpha.cos());
    if eps < 1.2 {
        warnings.push(tr!("parts.gears.contact", ratio = round2(eps)));
    }
}

fn to_u32(field: &str, v: i32, lo: i32, hi: i32) -> Result<u32, PartsError> {
    range(field, f64::from(v), f64::from(lo), f64::from(hi))?;
    Ok(v as u32)
}

/// Genera la pieza (o las piezas) de engranajes.
pub fn gears_parts(p: &GearParams) -> Result<Generated, PartsError> {
    let module = match p.size_by {
        SizeBy::Module => {
            range("module_mm", p.module_mm, 0.2, 25.0)?;
            p.module_mm
        }
        SizeBy::DiametralPitch => {
            range("diametral_pitch", p.diametral_pitch, 1.0, 127.0)?;
            25.4 / p.diametral_pitch
        }
    };
    range("clearance", p.clearance, 0.05, 0.5)?;
    range("backlash_mm", p.backlash_mm, 0.0, 2.0)?;
    range("tolerance_mm", p.tolerance_mm, 0.002, 0.1)?;
    range("bore_mm", p.bore_mm, 0.0, 500.0)?;
    range("bore2_mm", p.bore2_mm, 0.0, 500.0)?;
    range("key_width_mm", p.key_width_mm, 0.5, 50.0)?;
    range("key_depth_mm", p.key_depth_mm, 0.2, 25.0)?;
    range("flat_depth_mm", p.flat_depth_mm, 0.1, 100.0)?;
    range("web_mm", p.web_mm, 1.0, 50.0)?;
    range("ring_rim_mm", p.ring_rim_mm, 2.0, 100.0)?;
    range("rack_base_mm", p.rack_base_mm, 2.0, 200.0)?;
    range("lightening_count", f64::from(p.lightening_count), 3.0, 24.0)?;

    let ctx = Ctx {
        module,
        alpha: p.pressure_angle.degrees().to_radians(),
        clearance: p.clearance * module,
        backlash: p.backlash_mm,
        fillet: p.root == Root::Fillet,
        tol: p.tolerance_mm,
    };
    // Con tanto juego el diente casi desaparece.
    if ctx.thickness() < 0.25 * ctx.pitch() / 2.0 {
        return Err(PartsError::field(
            "backlash_mm",
            tr!("parts.gears.backlash_big", max = round2(ctx.pitch() * 0.75)),
        ));
    }

    let mut warnings = Vec::new();
    let origin = Point::new(0.0, 0.0);
    let mut sets: Vec<(&str, String, Vec<Polyline>)> = Vec::new();

    match p.kind {
        Kind::Single => {
            let z = to_u32("teeth", p.teeth, 6, 200)?;
            let g = external_gear(
                &ctx,
                p,
                Slot {
                    teeth: z,
                    bore: p.bore_mm,
                    tooth_field: "teeth",
                    bore_field: "bore_mm",
                    center: origin,
                    rotation: 0.0,
                },
                &mut warnings,
            )?;
            sets.push(("gear", tr!("parts.gears.name.gear", teeth = z), g.contours));
        }
        Kind::Pair => {
            let z1 = to_u32("teeth", p.teeth, 6, 200)?;
            let z2 = to_u32("teeth2", p.teeth2, 6, 400)?;
            let (r1, r2) = (module * f64::from(z1) / 2.0, module * f64::from(z2) / 2.0);
            let c = r1 + r2;
            let (second_at, second_rot) = match p.layout {
                Layout::Meshed => (Point::new(c, 0.0), PI - PI / f64::from(z2)),
                Layout::Separate => (origin, PI - PI / f64::from(z2)),
            };
            let g1 = external_gear(
                &ctx,
                p,
                Slot {
                    teeth: z1,
                    bore: p.bore_mm,
                    tooth_field: "teeth",
                    bore_field: "bore_mm",
                    center: origin,
                    rotation: 0.0,
                },
                &mut warnings,
            )?;
            let g2 = external_gear(
                &ctx,
                p,
                Slot {
                    teeth: z2,
                    bore: p.bore2_mm,
                    tooth_field: "teeth2",
                    bore_field: "bore2_mm",
                    center: second_at,
                    rotation: second_rot,
                },
                &mut warnings,
            )?;
            let (p1, p2) = (ctx.external(z1), ctx.external(z2));
            let line = path_of_contact(p1.tip_radius, p1.base_radius())
                + path_of_contact(p2.tip_radius, p2.base_radius())
                - c * ctx.alpha.sin();
            warn_contact(&ctx, line, &mut warnings);
            sets.push((
                "gear_1",
                tr!("parts.gears.name.gear", teeth = z1),
                g1.contours,
            ));
            sets.push((
                "gear_2",
                tr!("parts.gears.name.gear", teeth = z2),
                g2.contours,
            ));
        }
        Kind::Ring => {
            let z1 = to_u32("teeth", p.teeth, 6, 200)?;
            let z2 = to_u32("teeth2", p.teeth2, 6, 400)?;
            if z2 <= z1 {
                return Err(PartsError::field(
                    "teeth2",
                    tr!("parts.gears.ring_more", teeth = z1),
                ));
            }
            if ((z2 - z1) as i32) < MIN_RING_DIFF {
                warnings.push(tr!(
                    "parts.gears.ring_diff",
                    diff = z2 - z1,
                    min = MIN_RING_DIFF
                ));
            }
            let c = module * f64::from(z2 - z1) / 2.0;
            let (ring_at, pinion_at) = match p.layout {
                Layout::Meshed => (origin, Point::new(c, 0.0)),
                Layout::Separate => (origin, origin),
            };
            let (ring, void) = ring_gear(&ctx, p, z2, ring_at)?;
            let pinion = external_gear(
                &ctx,
                p,
                Slot {
                    teeth: z1,
                    bore: p.bore_mm,
                    tooth_field: "teeth",
                    bore_field: "bore_mm",
                    center: pinion_at,
                    rotation: 0.0,
                },
                &mut warnings,
            )?;
            let p1 = ctx.external(z1);
            let ring_tip = void.root_radius;
            let rb2 = void.base_radius();
            let line = path_of_contact(p1.tip_radius, p1.base_radius())
                - path_of_contact(ring_tip, rb2)
                + c * ctx.alpha.sin();
            warn_contact(&ctx, line, &mut warnings);
            sets.push((
                "pinion",
                tr!("parts.gears.name.pinion", teeth = z1),
                pinion.contours,
            ));
            sets.push((
                "ring",
                tr!("parts.gears.name.ring", teeth = z2),
                ring.contours,
            ));
        }
        Kind::Rack | Kind::RackPinion => {
            let zr = to_u32("rack_teeth", p.rack_teeth, 2, 200)?;
            if f64::from(zr) * ctx.pitch() > MAX_SIZE_MM {
                return Err(PartsError::field(
                    "rack_teeth",
                    tr!("parts.gears.too_big", max = MAX_SIZE_MM),
                ));
            }
            let outline = rack_outline(
                zr,
                module,
                ctx.alpha,
                ctx.clearance,
                ctx.thickness(),
                p.rack_base_mm,
            )
            .map_err(|e| profile_error(e, &ctx, "rack_teeth", zr))?;
            let rack_name = tr!("parts.gears.name.rack", teeth = zr);
            if p.kind == Kind::Rack {
                sets.push(("rack", rack_name, vec![poly(outline, origin)]));
            } else {
                let z1 = to_u32("teeth", p.teeth, 6, 200)?;
                let r1 = module * f64::from(z1) / 2.0;
                let rot = -PI / 2.0 - PI / f64::from(z1);
                let (pinion_at, dx, dy) = match p.layout {
                    // El diente central de la cremallera queda bajo el centro del piñón.
                    Layout::Meshed => (origin, -f64::from(zr / 2) * ctx.pitch(), -r1),
                    Layout::Separate => (origin, 0.0, 0.0),
                };
                let pinion = external_gear(
                    &ctx,
                    p,
                    Slot {
                        teeth: z1,
                        bore: p.bore_mm,
                        tooth_field: "teeth",
                        bore_field: "bore_mm",
                        center: pinion_at,
                        rotation: rot,
                    },
                    &mut warnings,
                )?;
                let moved: Vec<Point> = outline
                    .into_iter()
                    .map(|q| Point::new(q.x + dx, q.y + dy))
                    .collect();
                let p1 = ctx.external(z1);
                let line = path_of_contact(p1.tip_radius, p1.base_radius()) - r1 * ctx.alpha.sin()
                    + ctx.module / ctx.alpha.sin();
                warn_contact(&ctx, line, &mut warnings);
                sets.push((
                    "pinion",
                    tr!("parts.gears.name.pinion", teeth = z1),
                    pinion.contours,
                ));
                sets.push((
                    "rack",
                    rack_name,
                    vec![Polyline {
                        points: moved,
                        closed: true,
                    }],
                ));
            }
        }
    }

    let meshed = sets.len() > 1 && p.layout == Layout::Meshed;
    let parts = if meshed {
        let key = match p.kind {
            Kind::Pair => "pair",
            Kind::Ring => "ring_pair",
            _ => "rack_pair",
        };
        let name = match p.kind {
            Kind::Pair => tr!("parts.gears.name.pair", z1 = p.teeth, z2 = p.teeth2),
            Kind::Ring => tr!("parts.gears.name.ring_pair", z1 = p.teeth, z2 = p.teeth2),
            _ => tr!(
                "parts.gears.name.rack_pair",
                z1 = p.teeth,
                z2 = p.rack_teeth
            ),
        };
        let cuts = sets.into_iter().flat_map(|s| s.2).collect();
        vec![Part::new(key, name, cuts, Vec::new())]
    } else {
        sets.into_iter()
            .map(|(key, name, cuts)| Part::new(key, name, cuts, Vec::new()))
            .collect()
    };
    // Dos engranajes iguales darían el mismo aviso dos veces.
    let mut seen = std::collections::HashSet::new();
    warnings.retain(|w| seen.insert(w.clone()));
    Ok(Generated { parts, warnings })
}
