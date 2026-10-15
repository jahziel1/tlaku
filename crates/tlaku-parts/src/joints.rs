//! Uniones sueltas entre dos placas por una arista: cola de milano plana y pestaña-ranura.
//!
//! Son piezas de prueba o de ejemplo para usar en diseños propios (las cajas ya traen sus dientes
//! en `boxes`). La geometría es nominal: el kerf lo compensa la capa y `fit_mm` es el juego que
//! se le deja a la unión (positivo = más flojo, negativo = más apretado).
//!
//! # Cola de milano plana
//!
//! Las dos placas están en el **mismo plano** y se juntan por una arista de largo `length_mm`,
//! como las piezas de un rompecabezas. La placa A lleva `count` colas trapezoidales repartidas a
//! intervalos iguales: el cuello (en la arista) mide `neck_mm` y la cola se ensancha según
//! `angle_deg` (medido respecto a la perpendicular de la arista) hasta su punta, a `depth_mm`. La
//! placa B lleva las ranuras del mismo trapecio. La ranura sale de desplazar los lados de la cola
//! `fit_mm / 2` hacia afuera (en perpendicular a cada lado), así que entre la cola y la ranura
//! queda `fit_mm / 2` de aire por cada lado y `fit_mm` de diferencia de ancho. Con `fit_mm = 0`
//! las dos placas juntas rellenan el rectángulo completo sin huecos ni solapes. Se ensamblan
//! deslizando una placa sobre la otra a lo largo de la arista; la forma impide separarlas tirando
//! en perpendicular.
//!
//! # Pestaña-ranura
//!
//! La placa A es perpendicular a la B y se apoya en su cara: A lleva `count` pestañas de
//! `tab_mm` de ancho y `thickness_mm` de largo (el grosor de B, para que queden a ras) y B lleva
//! las ranuras pasantes de `(tab_mm + fit_mm) x (thickness_mm + fit_mm)`, a `offset_mm` de su
//! borde. Las pestañas y las ranuras se reparten a intervalos iguales y van centradas.

use crate::error::PartsError;
use crate::part::{Generated, Part};
use crate::registry::{ParamKind, ParamSpec};
use serde::{Deserialize, Serialize};
use tlaku_core::{Point, Polyline};
use tlaku_i18n::tr;

/// Tipo de unión.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JointKind {
    /// Cola de milano plana entre dos placas del mismo plano.
    Dovetail,
    /// Pestañas en una placa y ranuras pasantes en otra perpendicular.
    TabSlot,
}

/// Parámetros de las uniones.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JointsParams {
    pub kind: JointKind,
    /// Largo de la arista que se une.
    pub length_mm: f64,
    /// Fondo de cada placa (perpendicular a la arista).
    pub plate_mm: f64,
    /// Número de colas o de pestañas.
    pub count: i32,
    /// Cola de milano: ancho del cuello, en la arista.
    pub neck_mm: f64,
    /// Cola de milano: cuánto entra la cola en la otra placa.
    pub depth_mm: f64,
    /// Cola de milano: ángulo de cada lado respecto a la perpendicular de la arista.
    pub angle_deg: f64,
    /// Pestaña-ranura: ancho de cada pestaña.
    pub tab_mm: f64,
    /// Pestaña-ranura: grosor del material (largo de la pestaña y ancho corto de la ranura).
    pub thickness_mm: f64,
    /// Pestaña-ranura: distancia del eje de las ranuras al borde de la placa B.
    pub offset_mm: f64,
    /// Ranura menos pestaña (o cola), en mm: positivo = más flojo.
    pub fit_mm: f64,
}

impl Default for JointsParams {
    fn default() -> Self {
        Self {
            kind: JointKind::Dovetail,
            length_mm: 60.0,
            plate_mm: 40.0,
            count: 3,
            neck_mm: 8.0,
            depth_mm: 8.0,
            angle_deg: 12.0,
            tab_mm: 10.0,
            thickness_mm: 3.0,
            offset_mm: 20.0,
            fit_mm: 0.0,
        }
    }
}

/// Campos del formulario de las uniones.
pub fn schema() -> Vec<ParamSpec> {
    let d = JointsParams::default();
    let dove = ["dovetail"];
    let tabs = ["tab_slot"];
    vec![
        ParamSpec::choice("kind", "joint", &["dovetail", "tab_slot"], "dovetail"),
        ParamSpec::length("length_mm", "size", 10.0, 1000.0, 1.0, d.length_mm),
        ParamSpec::length("plate_mm", "size", 5.0, 1000.0, 1.0, d.plate_mm),
        ParamSpec::int("count", "size", 1, 30, d.count),
        ParamSpec::length("neck_mm", "dovetail", 2.0, 200.0, 0.5, d.neck_mm).when("kind", &dove),
        ParamSpec::length("depth_mm", "dovetail", 2.0, 100.0, 0.5, d.depth_mm).when("kind", &dove),
        angle_spec(d.angle_deg).when("kind", &dove),
        ParamSpec::length("tab_mm", "tabs", 2.0, 200.0, 0.5, d.tab_mm).when("kind", &tabs),
        ParamSpec::length("thickness_mm", "tabs", 0.5, 20.0, 0.1, d.thickness_mm)
            .when("kind", &tabs),
        ParamSpec::length("offset_mm", "tabs", 2.0, 500.0, 0.5, d.offset_mm).when("kind", &tabs),
        ParamSpec::length("fit_mm", "joints", -0.5, 1.0, 0.01, d.fit_mm),
    ]
}

/// Campo del ángulo: un número sin unidad de longitud (grados).
fn angle_spec(default: f64) -> ParamSpec {
    ParamSpec {
        key: "angle_deg".to_string(),
        kind: ParamKind::Number {
            min: 1.0,
            max: 45.0,
            step: 0.5,
            default,
        },
        section: "dovetail".to_string(),
        when: None,
    }
}

/// Material mínimo (mm) que debe quedar entre dos ranuras y entre una ranura y el borde.
const MIN_LAND_MM: f64 = 1.5;

fn range(field: &str, v: f64, lo: f64, hi: f64) -> Result<(), PartsError> {
    if v.is_nan() || !(lo..=hi).contains(&v) {
        return Err(PartsError::field(
            field,
            tr!("parts.joints.range", min = lo, max = hi),
        ));
    }
    Ok(())
}

impl JointsParams {
    fn validate(&self) -> Result<(), PartsError> {
        range("length_mm", self.length_mm, 10.0, 1000.0)?;
        range("plate_mm", self.plate_mm, 5.0, 1000.0)?;
        range("count", f64::from(self.count), 1.0, 30.0)?;
        range("fit_mm", self.fit_mm, -0.5, 1.0)?;
        match self.kind {
            JointKind::Dovetail => {
                range("neck_mm", self.neck_mm, 2.0, 200.0)?;
                range("depth_mm", self.depth_mm, 2.0, 100.0)?;
                range("angle_deg", self.angle_deg, 1.0, 45.0)?;
            }
            JointKind::TabSlot => {
                range("tab_mm", self.tab_mm, 2.0, 200.0)?;
                range("thickness_mm", self.thickness_mm, 0.5, 20.0)?;
                range("offset_mm", self.offset_mm, 2.0, 500.0)?;
            }
        }
        Ok(())
    }

    /// Distancia entre ejes de dos colas o pestañas vecinas.
    pub(crate) fn pitch(&self) -> f64 {
        self.length_mm / f64::from(self.count)
    }

    /// Centro (a lo largo de la arista) de la cola o pestaña `i`.
    pub(crate) fn center(&self, i: i32) -> f64 {
        (f64::from(i) + 0.5) * self.pitch()
    }
}

/// Medidas del trapecio de la cola y de su ranura (semi-anchos y fondo).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DovetailShape {
    /// Semiancho del cuello de la cola.
    pub neck_half: f64,
    /// Semiancho de la punta de la cola.
    pub tip_half: f64,
    pub depth: f64,
    /// Semiancho de la boca de la ranura (en la arista).
    pub slot_neck_half: f64,
    /// Semiancho del fondo de la ranura.
    pub slot_tip_half: f64,
    pub slot_depth: f64,
}

impl JointsParams {
    /// Trapecio de la cola y de la ranura que le corresponde.
    pub fn dovetail_shape(&self) -> DovetailShape {
        let alpha = self.angle_deg.to_radians();
        let e = self.fit_mm / 2.0;
        let neck_half = self.neck_mm / 2.0;
        let tip_half = neck_half + self.depth_mm * alpha.tan();
        // Desplazar cada lado `e` en perpendicular mueve el lado `e / cos(alpha)` en horizontal.
        let slot_neck_half = neck_half + e / alpha.cos();
        let slot_depth = self.depth_mm + e;
        DovetailShape {
            neck_half,
            tip_half,
            depth: self.depth_mm,
            slot_neck_half,
            slot_tip_half: slot_neck_half + slot_depth * alpha.tan(),
            slot_depth,
        }
    }
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn closed(points: Vec<Point>) -> Polyline {
    Polyline {
        points,
        closed: true,
    }
}

/// Las dos placas de la cola de milano **ensambladas**: la arista común es `x = 0`, la placa A
/// ocupa `x <= 0`, la B `x >= 0`, y las colas de A entran en B. Contornos antihorarios.
pub fn dovetail_assembled(p: &JointsParams) -> Result<(Polyline, Polyline), PartsError> {
    p.validate()?;
    if p.kind != JointKind::Dovetail {
        return Err(PartsError::general(tr!("parts.joints.not_dovetail")));
    }
    let s = p.dovetail_shape();
    let (l, plate) = (p.length_mm, p.plate_mm);
    // Las ranuras vecinas, y la primera y la última contra el borde, dejan material.
    if 2.0 * s.slot_tip_half > p.pitch() - MIN_LAND_MM {
        return Err(PartsError::field(
            "count",
            tr!(
                "parts.joints.too_wide",
                max = format!("{:.1}", (p.pitch() - MIN_LAND_MM).max(0.0))
            ),
        ));
    }
    if s.slot_depth + MIN_LAND_MM + 0.5 > plate {
        return Err(PartsError::field(
            "depth_mm",
            tr!("parts.joints.too_deep", max = format!("{:.1}", plate - 2.0)),
        ));
    }
    let mut a = vec![pt(-plate, 0.0), pt(0.0, 0.0)];
    for i in 0..p.count {
        let c = p.center(i);
        a.extend([
            pt(0.0, c - s.neck_half),
            pt(s.depth, c - s.tip_half),
            pt(s.depth, c + s.tip_half),
            pt(0.0, c + s.neck_half),
        ]);
    }
    a.extend([pt(0.0, l), pt(-plate, l)]);
    let mut b = vec![pt(0.0, 0.0), pt(plate, 0.0), pt(plate, l), pt(0.0, l)];
    for i in (0..p.count).rev() {
        let c = p.center(i);
        b.extend([
            pt(0.0, c + s.slot_neck_half),
            pt(s.slot_depth, c + s.slot_tip_half),
            pt(s.slot_depth, c - s.slot_tip_half),
            pt(0.0, c - s.slot_neck_half),
        ]);
    }
    Ok((closed(a), closed(b)))
}

fn dovetail_parts(p: &JointsParams) -> Result<Generated, PartsError> {
    let (a, b) = dovetail_assembled(p)?;
    let mut warnings = vec![tr!("parts.joints.dovetail_note")];
    if p.neck_mm < 4.0 {
        warnings.push(tr!("parts.joints.thin_neck", neck = p.neck_mm));
    }
    if p.fit_mm != 0.0 {
        warnings.push(tr!("parts.joints.fit_vs_kerf"));
    }
    Ok(Generated {
        parts: vec![
            Part::new(
                "dovetail_tails",
                tr!("parts.joints.name.tails"),
                vec![a],
                Vec::new(),
            ),
            Part::new(
                "dovetail_sockets",
                tr!("parts.joints.name.sockets"),
                vec![b],
                Vec::new(),
            ),
        ],
        warnings,
    })
}

/// La placa con pestañas y los contornos de la placa con ranuras de la unión pestaña-ranura, cada
/// una en sus coordenadas (sin normalizar): la de pestañas tiene su arista en `y = 0` y las
/// pestañas hacia abajo (`y < 0`); la de ranuras va de `(0, 0)` a `(length, plate)` (primer
/// contorno) y sus ranuras son contornos horarios. Los centros de pestañas y ranuras coinciden
/// en `x`.
pub fn tab_slot_plates(p: &JointsParams) -> Result<(Polyline, Vec<Polyline>), PartsError> {
    p.validate()?;
    if p.kind != JointKind::TabSlot {
        return Err(PartsError::general(tr!("parts.joints.not_tab_slot")));
    }
    let (l, plate, t) = (p.length_mm, p.plate_mm, p.thickness_mm);
    let slot_w = p.tab_mm + p.fit_mm;
    let slot_h = t + p.fit_mm;
    if slot_w > p.pitch() - MIN_LAND_MM {
        return Err(PartsError::field(
            "tab_mm",
            tr!(
                "parts.joints.tab_too_wide",
                max = format!("{:.1}", (p.pitch() - MIN_LAND_MM - p.fit_mm).max(0.0))
            ),
        ));
    }
    if p.offset_mm - slot_h / 2.0 < MIN_LAND_MM || p.offset_mm + slot_h / 2.0 > plate - MIN_LAND_MM
    {
        return Err(PartsError::field(
            "offset_mm",
            tr!(
                "parts.joints.offset_range",
                min = format!("{:.1}", MIN_LAND_MM + slot_h / 2.0),
                max = format!("{:.1}", plate - MIN_LAND_MM - slot_h / 2.0)
            ),
        ));
    }
    let mut tabs = vec![pt(0.0, 0.0)];
    for i in 0..p.count {
        let (x0, x1) = (p.center(i) - p.tab_mm / 2.0, p.center(i) + p.tab_mm / 2.0);
        tabs.extend([pt(x0, 0.0), pt(x0, -t), pt(x1, -t), pt(x1, 0.0)]);
    }
    tabs.extend([pt(l, 0.0), pt(l, plate), pt(0.0, plate)]);
    Ok((closed(tabs), slot_plate(p)))
}

fn tab_slot_parts(p: &JointsParams) -> Result<Generated, PartsError> {
    let (tabs, slots) = tab_slot_plates(p)?;
    let mut warnings = Vec::new();
    if p.tab_mm < p.thickness_mm {
        warnings.push(tr!(
            "parts.joints.thin_tab",
            tab = p.tab_mm,
            thickness = p.thickness_mm
        ));
    }
    if p.fit_mm != 0.0 {
        warnings.push(tr!("parts.joints.fit_vs_kerf"));
    }
    Ok(Generated {
        parts: vec![
            Part::new(
                "tab_plate",
                tr!("parts.joints.name.tabs"),
                vec![tabs],
                Vec::new(),
            ),
            Part::new(
                "slot_plate",
                tr!("parts.joints.name.slots"),
                slots,
                Vec::new(),
            ),
        ],
        warnings,
    })
}

/// La placa de ranuras: el rectángulo y sus ranuras pasantes (contornos horarios).
fn slot_plate(p: &JointsParams) -> Vec<Polyline> {
    let (l, plate) = (p.length_mm, p.plate_mm);
    let (w, h) = (p.tab_mm + p.fit_mm, p.thickness_mm + p.fit_mm);
    let mut out = vec![closed(vec![
        pt(0.0, 0.0),
        pt(l, 0.0),
        pt(l, plate),
        pt(0.0, plate),
    ])];
    for i in 0..p.count {
        let c = p.center(i);
        let (x0, x1) = (c - w / 2.0, c + w / 2.0);
        let (y0, y1) = (p.offset_mm - h / 2.0, p.offset_mm + h / 2.0);
        out.push(closed(vec![pt(x0, y0), pt(x0, y1), pt(x1, y1), pt(x1, y0)]));
    }
    out
}

/// Genera las uniones.
pub fn joints_parts(p: &JointsParams) -> Result<Generated, PartsError> {
    match p.kind {
        JointKind::Dovetail => dovetail_parts(p),
        JointKind::TabSlot => tab_slot_parts(p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rectilinear::signed_area;
    use crate::testutil::{crossings, inside};

    fn dove(fit: f64) -> JointsParams {
        JointsParams {
            kind: JointKind::Dovetail,
            fit_mm: fit,
            ..JointsParams::default()
        }
    }

    fn point_seg(p: Point, a: Point, b: Point) -> f64 {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
        (p.x - (a.x + t * dx)).hypot(p.y - (a.y + t * dy))
    }

    fn tabs(fit: f64) -> JointsParams {
        JointsParams {
            kind: JointKind::TabSlot,
            fit_mm: fit,
            ..JointsParams::default()
        }
    }

    #[test]
    fn dovetail_plates_are_complementary_without_clearance() {
        for p in [
            dove(0.0),
            JointsParams {
                count: 1,
                angle_deg: 20.0,
                length_mm: 37.0,
                ..dove(0.0)
            },
            JointsParams {
                count: 5,
                neck_mm: 5.0,
                depth_mm: 6.0,
                length_mm: 100.0,
                plate_mm: 25.0,
                ..dove(0.0)
            },
        ] {
            let (a, b) = dovetail_assembled(&p).unwrap();
            // Las áreas suman exactamente el rectángulo completo.
            let total = 2.0 * p.plate_mm * p.length_mm;
            assert!(
                (signed_area(&a) + signed_area(&b) - total).abs() < 1e-6,
                "{p:?}"
            );
            // Cada punto del rectángulo pertenece a una placa y solo a una.
            let polys = [a.clone(), b.clone()];
            let n = 120;
            for i in 0..n {
                for j in 0..n {
                    let x = -p.plate_mm + (i as f64 + 0.37) * 2.0 * p.plate_mm / n as f64;
                    let y = (j as f64 + 0.61) * p.length_mm / n as f64;
                    let in_a = inside(&polys[..1], x, y);
                    let in_b = inside(&polys[1..], x, y);
                    assert!(in_a ^ in_b, "{p:?} ({x}, {y}): a={in_a} b={in_b}");
                }
            }
        }
    }

    #[test]
    fn dovetail_clearance_leaves_the_requested_gap() {
        for fit in [0.1, 0.25, -0.1] {
            let p = dove(fit);
            let (a, b) = dovetail_assembled(&p).unwrap();
            let s = p.dovetail_shape();
            // Ningún punto está en las dos placas y ninguna se cruza.
            let n = 140;
            for i in 0..n {
                for j in 0..n {
                    let x = -p.plate_mm + (i as f64 + 0.37) * 2.0 * p.plate_mm / n as f64;
                    let y = (j as f64 + 0.61) * p.length_mm / n as f64;
                    let both = inside(std::slice::from_ref(&a), x, y)
                        && inside(std::slice::from_ref(&b), x, y);
                    if fit >= 0.0 {
                        assert!(!both, "fit {fit}: solape en ({x}, {y})");
                    }
                }
            }
            // Distancia mínima entre los lados de la cola y los de la ranura (sin contar la
            // arista común `x = 0`): `fit / 2` por cada lado.
            if fit > 0.0 {
                let sides = |p: &Polyline, max_x: f64| -> Vec<(Point, Point)> {
                    crate::testutil::segments(p)
                        .into_iter()
                        .filter(|(u, v)| {
                            (u.x.abs() > 1e-9 || v.x.abs() > 1e-9)
                                && u.x <= max_x + 1e-9
                                && v.x <= max_x + 1e-9
                        })
                        .collect()
                };
                let mut gap = f64::MAX;
                for (u, v) in sides(&a, s.depth) {
                    for (r, w) in sides(&b, s.slot_depth) {
                        for d in [
                            point_seg(u, r, w),
                            point_seg(v, r, w),
                            point_seg(r, u, v),
                            point_seg(w, u, v),
                        ] {
                            gap = gap.min(d);
                        }
                    }
                }
                assert!((gap - fit / 2.0).abs() < 1e-9, "fit {fit}: aire {gap}");
            }
            // Ancho de ranura menos ancho de cola = fit, en el cuello y en la punta del trapecio.
            assert!(
                (2.0 * (s.slot_neck_half - s.neck_half) / (1.0 / p.angle_deg.to_radians().cos())
                    - fit)
                    .abs()
                    < 1e-9
            );
            // Distancia perpendicular entre el lado de la cola y el de la ranura = fit / 2.
            let alpha = p.angle_deg.to_radians();
            let lateral = s.slot_neck_half - s.neck_half; // desplazamiento horizontal
            assert!((lateral * alpha.cos() - fit / 2.0).abs() < 1e-9);
            // El fondo de la ranura queda `fit / 2` más lejos que la punta de la cola.
            assert!((s.slot_depth - s.depth - fit / 2.0).abs() < 1e-9);
        }
    }

    #[test]
    fn dovetail_tails_flare_by_the_requested_angle_and_cannot_pull_out() {
        let p = dove(0.0);
        let s = p.dovetail_shape();
        let flare = ((s.tip_half - s.neck_half) / s.depth).atan().to_degrees();
        assert!((flare - p.angle_deg).abs() < 1e-9);
        // La punta es más ancha que el cuello: no sale tirando en perpendicular.
        assert!(s.tip_half > s.neck_half);
        let (a, _) = dovetail_assembled(&p).unwrap();
        // Hay `count` colas: cuenta los puntos de la punta (x = profundidad).
        let tips = a
            .points
            .iter()
            .filter(|q| (q.x - s.depth).abs() < 1e-9)
            .count();
        assert_eq!(tips, 2 * p.count as usize);
    }

    #[test]
    fn dovetail_parts_have_their_own_origin_and_the_right_size() {
        let p = dove(0.0);
        let g = joints_parts(&p).unwrap();
        assert_eq!(g.parts.len(), 2);
        let s = p.dovetail_shape();
        let tails = &g.parts[0];
        let sockets = &g.parts[1];
        assert!((tails.width - (p.plate_mm + s.depth)).abs() < 1e-9);
        assert!((tails.height - p.length_mm).abs() < 1e-9);
        assert!((sockets.width - p.plate_mm).abs() < 1e-9);
        assert!((sockets.height - p.length_mm).abs() < 1e-9);
        assert!(!g.warnings.is_empty());
        // Contornos cerrados sin autointersecciones.
        for part in &g.parts {
            assert_eq!(crossings(&part.cuts), 0, "{}", part.key);
        }
    }

    #[test]
    fn tab_slot_slots_are_wider_than_the_tabs_by_the_clearance() {
        for fit in [0.0, 0.1, 0.2] {
            let p = tabs(fit);
            let (plate_tabs, slots) = tab_slot_plates(&p).unwrap();
            assert_eq!(slots.len(), 1 + p.count as usize);
            // Anchos y largos de las pestañas, desde los vértices del contorno.
            let tab_tips: Vec<(f64, f64)> = plate_tabs
                .points
                .windows(2)
                .filter(|w| {
                    (w[0].y + p.thickness_mm).abs() < 1e-9 && (w[1].y + p.thickness_mm).abs() < 1e-9
                })
                .map(|w| (w[0].x, w[1].x))
                .collect();
            assert_eq!(tab_tips.len(), p.count as usize);
            for (i, (x0, x1)) in tab_tips.iter().enumerate() {
                assert!((x1 - x0 - p.tab_mm).abs() < 1e-9);
                let hole = &slots[1 + i];
                let xs: Vec<f64> = hole.points.iter().map(|q| q.x).collect();
                let ys: Vec<f64> = hole.points.iter().map(|q| q.y).collect();
                let (hx0, hx1) = (
                    xs.iter().cloned().fold(f64::MAX, f64::min),
                    xs.iter().cloned().fold(f64::MIN, f64::max),
                );
                let (hy0, hy1) = (
                    ys.iter().cloned().fold(f64::MAX, f64::min),
                    ys.iter().cloned().fold(f64::MIN, f64::max),
                );
                // Ancho de ranura − pestaña = fit, y la pestaña queda centrada.
                assert!(((hx1 - hx0) - (x1 - x0) - fit).abs() < 1e-9);
                assert!(((hx0 + hx1) / 2.0 - (x0 + x1) / 2.0).abs() < 1e-9);
                // Alto de ranura − grosor = fit.
                assert!(((hy1 - hy0) - p.thickness_mm - fit).abs() < 1e-9);
                // Contorno horario (agujero) dentro de la placa, sin tocar el borde.
                assert!(signed_area(hole) < 0.0);
                assert!(
                    hx0 > 1.0 && hx1 < p.length_mm - 1.0 && hy0 > 1.0 && hy1 < p.plate_mm - 1.0
                );
            }
            // Las pestañas tienen el grosor como largo.
            let low = plate_tabs
                .points
                .iter()
                .map(|q| q.y)
                .fold(f64::MAX, f64::min);
            assert!((low + p.thickness_mm).abs() < 1e-9);
        }
    }

    #[test]
    fn tab_slot_parts_are_valid_contours() {
        let g = joints_parts(&tabs(0.1)).unwrap();
        assert_eq!(g.parts.len(), 2);
        assert_eq!(g.parts[0].key, "tab_plate");
        assert_eq!(g.parts[1].key, "slot_plate");
        for part in &g.parts {
            assert_eq!(crossings(&part.cuts), 0, "{}", part.key);
        }
        // La placa con ranuras mide el rectángulo; la de pestañas suma el largo de la pestaña.
        let p = tabs(0.1);
        assert!((g.parts[1].width - p.length_mm).abs() < 1e-9);
        assert!((g.parts[0].height - (p.plate_mm + p.thickness_mm)).abs() < 1e-9);
        assert!(!g.warnings.is_empty());
    }

    #[test]
    fn invalid_values_name_the_field() {
        let bad = |p: JointsParams| joints_parts(&p).unwrap_err().field;
        assert_eq!(
            bad(JointsParams {
                length_mm: 1.0,
                ..dove(0.0)
            })
            .as_deref(),
            Some("length_mm")
        );
        assert_eq!(
            bad(JointsParams {
                count: 0,
                ..dove(0.0)
            })
            .as_deref(),
            Some("count")
        );
        // Colas tan anchas que no queda material entre ranuras.
        assert_eq!(
            bad(JointsParams {
                count: 6,
                neck_mm: 12.0,
                ..dove(0.0)
            })
            .as_deref(),
            Some("count")
        );
        // Ranura más profunda que la placa.
        assert_eq!(
            bad(JointsParams {
                depth_mm: 39.0,
                angle_deg: 1.0,
                ..dove(0.0)
            })
            .as_deref(),
            Some("depth_mm")
        );
        assert_eq!(
            bad(JointsParams {
                tab_mm: 30.0,
                ..tabs(0.0)
            })
            .as_deref(),
            Some("tab_mm")
        );
        assert_eq!(
            bad(JointsParams {
                offset_mm: 39.5,
                ..tabs(0.0)
            })
            .as_deref(),
            Some("offset_mm")
        );
        assert_eq!(
            bad(JointsParams {
                angle_deg: 60.0,
                ..dove(0.0)
            })
            .as_deref(),
            Some("angle_deg")
        );
    }

    #[test]
    fn wrong_kind_is_reported_instead_of_panicking() {
        assert!(dovetail_assembled(&tabs(0.0)).is_err());
        assert!(tab_slot_plates(&dove(0.0)).is_err());
    }

    #[test]
    fn thin_tabs_and_necks_warn() {
        let g = joints_parts(&JointsParams {
            tab_mm: 2.0,
            ..tabs(0.0)
        })
        .unwrap();
        assert!(g.warnings.iter().any(|w| w.contains('2')));
        let g = joints_parts(&JointsParams {
            neck_mm: 3.0,
            ..dove(0.0)
        })
        .unwrap();
        assert!(g.warnings.len() >= 2);
    }
}
