//! Print & Cut: alinear el diseño con marcas de registro físicas.
//!
//! Dos piezas:
//!
//! - [`fit_registration`] calcula la transformación del diseño a la cama a partir de pares
//!   `(punto del diseño, punto físico)`: una rígida o de similitud (traslación, rotación y
//!   escala uniforme opcional) por mínimos cuadrados, o una afín completa con 3 o más puntos.
//!   Devuelve el error residual y avisos (escala lejos de 1, impresión deformada o espejada).
//! - [`apply`] aplica `JobOptions::transform` a todo el trabajo ya colocado. Como actúa sobre las
//!   trayectorias, el encuadre y la verificación de límites (que salen del trabajo calculado) lo
//!   ven sin código extra.

use crate::place::{ImageWarp, PlacedImage, Placement};
use serde::{Deserialize, Serialize};
use std::fmt;
use tlaku_core::{Point, Polyline};
use tlaku_doc::affine::{self, Transform};
use tlaku_i18n::tr;

/// Desviación de la escala respecto de 1 (fracción) a partir de la cual se avisa: 0,5 %.
pub const SCALE_WARN: f64 = 0.005;

/// Distancia mínima (mm) entre dos marcas: más cerca, la rotación no se puede medir.
pub const MIN_SEPARATION_MM: f64 = 1.0;

/// Una marca: dónde está en el diseño y dónde se midió en la cama, en mm.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MarkPair {
    pub design: [f64; 2],
    pub physical: [f64; 2],
}

/// Qué tipo de transformación se busca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FitMode {
    /// Solo traslación y rotación (escala 1). Con 2 puntos la diferencia de distancias queda
    /// repartida como residual.
    Rigid,
    /// Traslación, rotación y escala uniforme.
    Similarity,
    /// Afín completa (escala distinta en cada eje y sesgo): pide 3 puntos o más, no alineados.
    Affine,
}

impl FitMode {
    /// El modo por omisión según cuántas marcas hay: 2 dan similitud, 3 o más afín.
    pub fn auto(marks: usize) -> FitMode {
        if marks >= 3 {
            FitMode::Affine
        } else {
            FitMode::Similarity
        }
    }
}

/// Lo que se pide para el ajuste: un modo concreto o «según cuántas marcas haya».
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FitChoice {
    #[default]
    Auto,
    Rigid,
    Similarity,
    Affine,
}

impl FitChoice {
    /// El modo que se usa de verdad. `Auto` da similitud con 2 marcas y afín con 3 o más. La afín
    /// no se puede resolver con menos de 3 marcas ni con marcas en línea recta: en esos casos se
    /// cae a la similitud.
    pub fn resolve(self, design: &[[f64; 2]]) -> FitMode {
        let wanted = match self {
            FitChoice::Auto => FitMode::auto(design.len()),
            FitChoice::Rigid => FitMode::Rigid,
            FitChoice::Similarity => FitMode::Similarity,
            FitChoice::Affine => FitMode::Affine,
        };
        if wanted == FitMode::Affine && (design.len() < 3 || are_collinear(design)) {
            FitMode::Similarity
        } else {
            wanted
        }
    }
}

/// ¿Los puntos están (casi) en línea recta? Compara el área que abarcan con su largo. Con menos
/// de 3 puntos no.
pub fn are_collinear(points: &[[f64; 2]]) -> bool {
    if points.len() < 3 {
        return false;
    }
    let c = centroid(points);
    let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
    for p in points {
        let (dx, dy) = (p[0] - c[0], p[1] - c[1]);
        sxx += dx * dx;
        sxy += dx * dy;
        syy += dy * dy;
    }
    let det = sxx * syy - sxy * sxy;
    let energy = sxx + syy;
    energy < 1e-12 || det <= 1e-9 * energy * energy
}

/// Por qué no se pudo calcular el ajuste. `Display` da el mensaje en español; la interfaz usa el
/// tipo para mostrar el suyo en el idioma elegido.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum FitError {
    /// Faltan marcas: `need` como mínimo, hay `have`.
    Few { need: usize, have: usize },
    /// Una marca tiene coordenadas que no son números.
    Nan,
    /// Las marcas `a` y `b` (desde 1) están a menos de [`MIN_SEPARATION_MM`].
    Close { a: usize, b: usize },
    /// Las marcas están en línea recta y la afín necesita que no lo estén.
    Collinear,
}

impl fmt::Display for FitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FitError::Few { need: 3, have } => {
                write!(
                    f,
                    "{}",
                    tr!("pipeline.registration.few_affine", have = have)
                )
            }
            FitError::Few { have, .. } => {
                write!(f, "{}", tr!("pipeline.registration.few", have = have))
            }
            FitError::Nan => write!(f, "{}", tr!("pipeline.registration.nan")),
            FitError::Close { a, b } => write!(
                f,
                "{}",
                tr!(
                    "pipeline.registration.close",
                    a = a,
                    b = b,
                    min = MIN_SEPARATION_MM
                )
            ),
            FitError::Collinear => write!(f, "{}", tr!("pipeline.registration.collinear")),
        }
    }
}

impl std::error::Error for FitError {}

/// Aviso sobre el ajuste. `Display` da el texto en el idioma activo del motor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum FitWarning {
    /// La transformación invierte el diseño (impresión espejada o marcas en otro orden).
    Mirrored,
    /// La escala medida (en %) está a más de [`SCALE_WARN`] de 100 %.
    Scale { pct: f64 },
    /// Estiramiento distinto en cada eje (en %).
    Anisotropy {
        #[serde(rename = "xPct")]
        x_pct: f64,
        #[serde(rename = "yPct")]
        y_pct: f64,
    },
}

impl fmt::Display for FitWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FitWarning::Mirrored => write!(f, "{}", tr!("pipeline.registration.mirrored")),
            FitWarning::Scale { pct } => write!(
                f,
                "{}",
                tr!(
                    "pipeline.registration.scale",
                    pct = format!("{pct:.2}"),
                    tolerance = format!("{:.1}", SCALE_WARN * 100.0)
                )
            ),
            FitWarning::Anisotropy { x_pct, y_pct } => write!(
                f,
                "{}",
                tr!(
                    "pipeline.registration.anisotropy",
                    x = format!("{x_pct:.2}"),
                    y = format!("{y_pct:.2}")
                )
            ),
        }
    }
}

/// Resultado del ajuste.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrationFit {
    /// Del documento a la cama; es lo que va en `JobOptions::transform`.
    pub transform: Transform,
    pub mode: FitMode,
    /// Giro en grados (antihorario) que aplica la transformación.
    pub rotation_deg: f64,
    /// Escala medida: la uniforme de la similitud, o `sqrt(|det|)` en la afín. En el modo
    /// rígido es la que habría hecho falta (la transformación usa 1), para poder avisar.
    pub scale: f64,
    /// Escala en X y en Y (longitud de las columnas de la matriz).
    #[serde(rename = "scaleXY")]
    pub scale_xy: [f64; 2],
    /// Error de cada marca tras transformar, en mm (distancia al punto físico).
    pub residuals: Vec<f64>,
    /// Raíz del error cuadrático medio de `residuals`, en mm.
    pub residual_rms: f64,
    /// El peor de `residuals`, en mm.
    pub residual_max: f64,
    /// Avisos para mostrar al usuario (escala, deformación, espejo).
    pub warnings: Vec<FitWarning>,
}

fn finite2(p: [f64; 2]) -> bool {
    p[0].is_finite() && p[1].is_finite()
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn centroid(points: &[[f64; 2]]) -> [f64; 2] {
    let n = points.len() as f64;
    [
        points.iter().map(|p| p[0]).sum::<f64>() / n,
        points.iter().map(|p| p[1]).sum::<f64>() / n,
    ]
}

/// Calcula la transformación que lleva los puntos del diseño a los físicos.
///
/// Con `Rigid` y `Similarity` sirven 2 o más pares (mínimos cuadrados de Procrustes en 2D);
/// con `Affine`, 3 o más no alineados. Falla si faltan marcas, no son números, dos marcas
/// coinciden o (en la afín) están en línea recta.
pub fn fit_registration(pairs: &[MarkPair], mode: FitMode) -> Result<RegistrationFit, FitError> {
    let need = if mode == FitMode::Affine { 3 } else { 2 };
    if pairs.len() < need {
        return Err(FitError::Few {
            need,
            have: pairs.len(),
        });
    }
    if pairs
        .iter()
        .any(|p| !finite2(p.design) || !finite2(p.physical))
    {
        return Err(FitError::Nan);
    }
    for (i, a) in pairs.iter().enumerate() {
        for (j, b) in pairs.iter().enumerate().skip(i + 1) {
            if dist(a.design, b.design) < MIN_SEPARATION_MM
                || dist(a.physical, b.physical) < MIN_SEPARATION_MM
            {
                return Err(FitError::Close { a: i + 1, b: j + 1 });
            }
        }
    }

    let design: Vec<[f64; 2]> = pairs.iter().map(|p| p.design).collect();
    let physical: Vec<[f64; 2]> = pairs.iter().map(|p| p.physical).collect();
    let (cd, cp) = (centroid(&design), centroid(&physical));
    let a: Vec<[f64; 2]> = design
        .iter()
        .map(|p| [p[0] - cd[0], p[1] - cd[1]])
        .collect();
    let b: Vec<[f64; 2]> = physical
        .iter()
        .map(|p| [p[0] - cp[0], p[1] - cp[1]])
        .collect();

    // Procrustes con números complejos: z = Σ conj(a)·b da el giro (su ángulo) y la escala
    // (su módulo entre Σ|a|²).
    let dot: f64 = a
        .iter()
        .zip(&b)
        .map(|(p, q)| p[0] * q[0] + p[1] * q[1])
        .sum();
    let cross: f64 = a
        .iter()
        .zip(&b)
        .map(|(p, q)| p[0] * q[1] - p[1] * q[0])
        .sum();
    let energy: f64 = a.iter().map(|p| p[0] * p[0] + p[1] * p[1]).sum();
    if energy < 1e-12 {
        return Err(FitError::Close { a: 1, b: 2 });
    }
    let theta = cross.atan2(dot);
    let sim_scale = dot.hypot(cross) / energy;

    // Matriz [m00 m01; m10 m11] con x' = m00·x + m01·y.
    let m = match mode {
        FitMode::Rigid | FitMode::Similarity => {
            let s = if mode == FitMode::Rigid {
                1.0
            } else {
                sim_scale
            };
            let (sin, cos) = theta.sin_cos();
            [[s * cos, -s * sin], [s * sin, s * cos]]
        }
        FitMode::Affine => {
            // M = (Σ b·aᵀ)(Σ a·aᵀ)⁻¹
            let (mut saa, mut sba) = ([[0.0; 2]; 2], [[0.0; 2]; 2]);
            for (p, q) in a.iter().zip(&b) {
                for i in 0..2 {
                    for j in 0..2 {
                        saa[i][j] += p[i] * p[j];
                        sba[i][j] += q[i] * p[j];
                    }
                }
            }
            let det = saa[0][0] * saa[1][1] - saa[0][1] * saa[1][0];
            // Cuando las marcas están en línea, el menor valor propio de `saa` es ~0.
            if det.abs() <= 1e-9 * energy * energy {
                return Err(FitError::Collinear);
            }
            let inv = [
                [saa[1][1] / det, -saa[0][1] / det],
                [-saa[1][0] / det, saa[0][0] / det],
            ];
            let mut m = [[0.0; 2]; 2];
            for i in 0..2 {
                for j in 0..2 {
                    m[i][j] = sba[i][0] * inv[0][j] + sba[i][1] * inv[1][j];
                }
            }
            m
        }
    };
    let t: Transform = [
        m[0][0],
        m[1][0],
        m[0][1],
        m[1][1],
        cp[0] - (m[0][0] * cd[0] + m[0][1] * cd[1]),
        cp[1] - (m[1][0] * cd[0] + m[1][1] * cd[1]),
    ];

    let residuals: Vec<f64> = pairs
        .iter()
        .map(|p| dist(affine::apply(&t, p.design), p.physical))
        .collect();
    let residual_rms =
        (residuals.iter().map(|r| r * r).sum::<f64>() / residuals.len() as f64).sqrt();
    let residual_max = residuals.iter().copied().fold(0.0, f64::max);

    let det = t[0] * t[3] - t[1] * t[2];
    let scale_xy = [t[0].hypot(t[1]), t[2].hypot(t[3])];
    let (scale, rotation) = match mode {
        FitMode::Affine => (det.abs().sqrt(), (t[1] - t[2]).atan2(t[0] + t[3])),
        _ => (sim_scale, theta),
    };

    let mut warnings = Vec::new();
    if det < 0.0 {
        warnings.push(FitWarning::Mirrored);
    }
    if (scale - 1.0).abs() > SCALE_WARN {
        warnings.push(FitWarning::Scale { pct: scale * 100.0 });
    }
    if mode == FitMode::Affine
        && det > 0.0
        && (scale_xy[0] - scale_xy[1]).abs() / scale_xy[0].max(scale_xy[1]) > SCALE_WARN
    {
        warnings.push(FitWarning::Anisotropy {
            x_pct: scale_xy[0] * 100.0,
            y_pct: scale_xy[1] * 100.0,
        });
    }

    Ok(RegistrationFit {
        transform: t,
        mode,
        rotation_deg: rotation.to_degrees(),
        scale,
        scale_xy,
        residuals,
        residual_rms,
        residual_max,
        warnings,
    })
}

/// Ajuste según lo que pide la persona: resuelve el modo con [`FitChoice::resolve`] (según las
/// marcas del diseño) y calcula. Es lo que llama la interfaz.
pub fn fit_with_choice(pairs: &[MarkPair], choice: FitChoice) -> Result<RegistrationFit, FitError> {
    let design: Vec<[f64; 2]> = pairs.iter().map(|p| p.design).collect();
    fit_registration(pairs, choice.resolve(&design))
}

fn map_point(t: &Transform, p: Point) -> Point {
    let [x, y] = affine::apply(t, [p.x, p.y]);
    Point::new(x, y)
}

/// Giro (en radianes, como `sin`) bajo el cual una imagen se deja alineada a los ejes: 0,006° mueve
/// el borde de una imagen de 400 mm unos 0,04 mm, menos que un punto de grabado.
const IMAGE_ROTATION_EPS: f64 = 1e-4;

/// Aplica la transformación de Print & Cut a un trabajo ya colocado: trayectorias, puentes
/// manuales, imágenes y punto de inicio. Una imagen que solo se mueve, escala o refleja sigue
/// alineada a los ejes; si la transformación la gira o la inclina, guarda el giro en
/// [`ImageWarp`] y se vuelve a muestrear al trazarla (su caja pasa a ser la de la imagen girada).
pub(crate) fn apply(
    placement: &mut Placement,
    transform: Option<&Transform>,
) -> Result<(), String> {
    let Some(t) = transform else {
        return Ok(());
    };
    if !affine::is_finite(t) {
        return Err(tr!("pipeline.registration.transform_invalid"));
    }
    let det = t[0] * t[3] - t[1] * t[2];
    if det.abs() < 1e-12 {
        return Err(tr!("pipeline.registration.transform_collapses"));
    }
    if affine::is_identity(t) {
        return Ok(());
    }
    placement.start = map_point(t, placement.start);
    for layer in &mut placement.layers {
        for p in layer.paths.iter_mut().flat_map(|p| &mut p.points) {
            *p = map_point(t, *p);
        }
        for p in layer.manual_tabs.values_mut().flatten() {
            *p = map_point(t, *p);
        }
        for img in &mut layer.images {
            move_image(img, t);
        }
    }
    Ok(())
}

fn move_image(img: &mut PlacedImage, t: &Transform) {
    for p in img.mask.iter_mut().flat_map(|m| &mut m.points) {
        *p = map_point(t, *p);
    }
    if t[1].abs() > IMAGE_ROTATION_EPS || t[2].abs() > IMAGE_ROTATION_EPS {
        let warp = ImageWarp {
            src_origin: img.origin,
            src_size: img.size_mm,
            transform: *t,
        };
        let corners = warp.corners();
        let (min_x, max_x) = corners.iter().fold((f64::MAX, f64::MIN), |(lo, hi), c| {
            (lo.min(c.x), hi.max(c.x))
        });
        let (min_y, max_y) = corners.iter().fold((f64::MAX, f64::MIN), |(lo, hi), c| {
            (lo.min(c.y), hi.max(c.y))
        });
        img.origin = Point::new(min_x, min_y);
        img.size_mm = (max_x - min_x, max_y - min_y);
        img.warp = Some(warp);
        return;
    }
    // Sin giro: se reubica y escala; un reflejo en un eje invierte la imagen en ese eje.
    let center = map_point(
        t,
        Point::new(
            img.origin.x + img.size_mm.0 / 2.0,
            img.origin.y + img.size_mm.1 / 2.0,
        ),
    );
    img.size_mm = (img.size_mm.0 * t[0].abs(), img.size_mm.1 * t[3].abs());
    img.origin = Point::new(
        center.x - img.size_mm.0 / 2.0,
        center.y - img.size_mm.1 / 2.0,
    );
    if t[0] < 0.0 {
        img.mirror_x = !img.mirror_x;
    }
    if t[3] < 0.0 {
        img.mirror_y = !img.mirror_y;
    }
}

/// El contorno de la imagen ya transformada (un cuadrilátero), para recortar lo que cae fuera
/// de ella al volver a muestrearla.
pub(crate) fn warp_outline(warp: &ImageWarp) -> Polyline {
    Polyline {
        points: warp.corners().to_vec(),
        closed: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::place::PlacedImage;
    use crate::{build, check_bounds, frame_gcode};
    use tlaku_core::bezpath::BezPath;
    use tlaku_core::{Polyline, Rgb};
    use tlaku_doc::{
        DesignObject, DeviceProfile, Document, JobOptions, LayerSettings, ObjectKind, VectorObject,
    };

    const RED: Rgb = Rgb(255, 0, 0);

    fn pair(d: [f64; 2], p: [f64; 2]) -> MarkPair {
        MarkPair {
            design: d,
            physical: p,
        }
    }

    /// Pares generados con la transformación `t`.
    fn pairs_of(t: &Transform, pts: &[[f64; 2]]) -> Vec<MarkPair> {
        pts.iter().map(|&d| pair(d, affine::apply(t, d))).collect()
    }

    fn near(a: f64, b: f64, tol: f64) {
        assert!((a - b).abs() <= tol, "{a} != {b} (tol {tol})");
    }

    fn near_t(a: &Transform, b: &Transform, tol: f64) {
        for (x, y) in a.iter().zip(b) {
            near(*x, *y, tol);
        }
    }

    /// Generador pseudoaleatorio determinista (xorshift) en [-1, 1].
    fn noise(seed: &mut u64) -> f64 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        (*seed % 20001) as f64 / 10000.0 - 1.0
    }

    #[test]
    fn two_points_pure_translation() {
        let t = affine::translate(3.0, -2.0);
        let fit = fit_registration(
            &pairs_of(&t, &[[0.0, 0.0], [100.0, 0.0]]),
            FitMode::Similarity,
        )
        .unwrap();
        near_t(&fit.transform, &t, 1e-9);
        near(fit.rotation_deg, 0.0, 1e-9);
        near(fit.scale, 1.0, 1e-12);
        assert!(fit.residual_max < 1e-9);
        assert!(fit.warnings.is_empty(), "{:?}", fit.warnings);
    }

    #[test]
    fn two_points_rotation_and_scale_are_recovered() {
        let t = affine::compose(
            &affine::translate(10.0, 5.0),
            &affine::compose(&affine::rotate(0.02), &affine::scale(1.003, 1.003)),
        );
        let fit = fit_registration(
            &pairs_of(&t, &[[5.0, 5.0], [155.0, 105.0]]),
            FitMode::Similarity,
        )
        .unwrap();
        near_t(&fit.transform, &t, 1e-9);
        near(fit.rotation_deg, 0.02_f64.to_degrees(), 1e-9);
        near(fit.scale, 1.003, 1e-9);
        assert!(fit.residual_rms < 1e-9);
        // 0,3 % no llega al umbral de aviso.
        assert!(fit.warnings.is_empty(), "{:?}", fit.warnings);
    }

    #[test]
    fn rigid_with_two_points_spreads_the_length_mismatch() {
        // Las marcas físicas están 0,4 mm más separadas que las del diseño.
        let fit = fit_registration(
            &[
                pair([0.0, 0.0], [0.0, 0.0]),
                pair([100.0, 0.0], [100.4, 0.0]),
            ],
            FitMode::Rigid,
        )
        .unwrap();
        near(fit.scale, 1.004, 1e-9);
        near(fit.scale_xy[0], 1.0, 1e-12);
        near(fit.residuals[0], 0.2, 1e-9);
        near(fit.residuals[1], 0.2, 1e-9);
        near(fit.residual_max, 0.2, 1e-9);
    }

    #[test]
    fn scale_warning_triggers_above_half_a_percent() {
        let pts = [[0.0, 0.0], [200.0, 0.0]];
        let ok = fit_registration(
            &pairs_of(&affine::scale(1.004, 1.004), &pts),
            FitMode::Similarity,
        )
        .unwrap();
        assert!(ok.warnings.is_empty());
        let bad = fit_registration(
            &pairs_of(&affine::scale(1.01, 1.01), &pts),
            FitMode::Similarity,
        )
        .unwrap();
        assert!(
            bad.warnings
                .iter()
                .any(|w| matches!(w, FitWarning::Scale { .. })),
            "{:?}",
            bad.warnings
        );
        // También por debajo de 1 y en modo rígido (avisa con la escala que haría falta).
        let small =
            fit_registration(&pairs_of(&affine::scale(0.99, 0.99), &pts), FitMode::Rigid).unwrap();
        assert!(
            small
                .warnings
                .iter()
                .any(|w| matches!(w, FitWarning::Scale { .. }))
        );
        // En modo rígido la transformación no escala.
        near(small.transform[0].hypot(small.transform[1]), 1.0, 1e-9);
    }

    #[test]
    fn three_points_affine_recovers_shear_and_anisotropy() {
        let t: Transform = [1.002, 0.01, -0.004, 0.998, 7.0, -3.0];
        let fit = fit_registration(
            &pairs_of(&t, &[[0.0, 0.0], [200.0, 0.0], [0.0, 150.0]]),
            FitMode::Affine,
        )
        .unwrap();
        near_t(&fit.transform, &t, 1e-9);
        assert!(fit.residual_max < 1e-9);
    }

    #[test]
    fn affine_flags_a_stretched_print() {
        let t = affine::scale(1.012, 1.0);
        let fit = fit_registration(
            &pairs_of(&t, &[[0.0, 0.0], [200.0, 0.0], [0.0, 150.0]]),
            FitMode::Affine,
        )
        .unwrap();
        assert!(
            fit.warnings
                .iter()
                .any(|w| matches!(w, FitWarning::Anisotropy { .. })),
            "{:?}",
            fit.warnings
        );
    }

    #[test]
    fn four_points_with_noise_average_out() {
        let truth = affine::compose(&affine::translate(12.0, -4.0), &affine::rotate(-0.015));
        let mut seed = 0x1234_5678_9abc_def1_u64;
        let pts = [[0.0, 0.0], [300.0, 0.0], [300.0, 200.0], [0.0, 200.0]];
        let noisy: Vec<MarkPair> = pts
            .iter()
            .map(|&d| {
                let [x, y] = affine::apply(&truth, d);
                pair(
                    d,
                    [x + 0.05 * noise(&mut seed), y + 0.05 * noise(&mut seed)],
                )
            })
            .collect();
        for mode in [FitMode::Rigid, FitMode::Similarity, FitMode::Affine] {
            let fit = fit_registration(&noisy, mode).unwrap();
            // El ruido es de ±0,05 mm: el residual lo refleja y la transformación sigue cerca.
            assert!(
                fit.residual_rms > 0.0 && fit.residual_rms < 0.08,
                "{mode:?} {}",
                fit.residual_rms
            );
            assert!(fit.residual_max < 0.12, "{mode:?} {}", fit.residual_max);
            assert_eq!(fit.residuals.len(), 4);
            for p in &pts {
                let got = affine::apply(&fit.transform, *p);
                let want = affine::apply(&truth, *p);
                assert!(dist(got, want) < 0.1, "{mode:?}: {got:?} vs {want:?}");
            }
            near(fit.rotation_deg, (-0.015_f64).to_degrees(), 0.05);
        }
    }

    #[test]
    fn mirrored_print_is_reported() {
        let t: Transform = [-1.0, 0.0, 0.0, 1.0, 200.0, 0.0];
        let fit = fit_registration(
            &pairs_of(&t, &[[0.0, 0.0], [200.0, 0.0], [0.0, 150.0]]),
            FitMode::Affine,
        )
        .unwrap();
        assert!(fit.warnings.contains(&FitWarning::Mirrored));
    }

    #[test]
    fn rejects_bad_input() {
        let one = [pair([0.0, 0.0], [1.0, 1.0])];
        assert!(fit_registration(&one, FitMode::Similarity).is_err());
        let two = [pair([0.0, 0.0], [0.0, 0.0]), pair([50.0, 0.0], [50.0, 0.0])];
        assert!(fit_registration(&two, FitMode::Affine).is_err());
        // Marcas que coinciden en el diseño o en la cama.
        let same_d = [pair([0.0, 0.0], [0.0, 0.0]), pair([0.2, 0.0], [50.0, 0.0])];
        assert_eq!(
            fit_registration(&same_d, FitMode::Similarity).unwrap_err(),
            FitError::Close { a: 1, b: 2 }
        );
        let same_p = [pair([0.0, 0.0], [0.0, 0.0]), pair([50.0, 0.0], [0.5, 0.0])];
        assert!(fit_registration(&same_p, FitMode::Similarity).is_err());
        // Tres marcas en línea recta no sirven para una afín, pero sí para la similitud.
        let line = pairs_of(&affine::IDENTITY, &[[0.0, 0.0], [50.0, 0.0], [100.0, 0.0]]);
        assert_eq!(
            fit_registration(&line, FitMode::Affine).unwrap_err(),
            FitError::Collinear
        );
        assert!(fit_registration(&line, FitMode::Similarity).is_ok());
        // NaN.
        let nan = [
            pair([0.0, 0.0], [0.0, 0.0]),
            pair([50.0, 0.0], [f64::NAN, 0.0]),
        ];
        assert!(fit_registration(&nan, FitMode::Similarity).is_err());
    }

    #[test]
    fn auto_mode_follows_the_mark_count() {
        assert_eq!(FitMode::auto(2), FitMode::Similarity);
        assert_eq!(FitMode::auto(3), FitMode::Affine);
        assert_eq!(FitMode::auto(5), FitMode::Affine);
    }

    // ---- aplicación al trabajo -------------------------------------------------------

    fn square_doc() -> Document {
        let obj = DesignObject {
            id: "a".into(),
            layer: RED,
            transform: affine::IDENTITY,
            kind: ObjectKind::Vector(VectorObject {
                paths: vec![BezPath::from(&Polyline {
                    points: vec![
                        Point::new(10.0, 10.0),
                        Point::new(30.0, 10.0),
                        Point::new(30.0, 30.0),
                        Point::new(10.0, 30.0),
                    ],
                    closed: true,
                })],
            }),
            locked: false,
            hidden: false,
            group: None,
            priority: None,
            power_scale: None,
            shape: None,
            start: None,
            manual_tabs: Vec::new(),
            array: None,
        };
        Document {
            objects: vec![obj],
            layers: vec![LayerSettings::for_color(RED)],
            ..Document::default()
        }
    }

    fn with_transform(t: Option<Transform>) -> JobOptions {
        JobOptions {
            transform: t,
            ..JobOptions::default()
        }
    }

    #[test]
    fn the_transform_moves_the_whole_job() {
        let doc = square_doc();
        let dev = DeviceProfile::default();
        let plain = build(&doc, &dev, &with_transform(None), None).unwrap();
        let moved = build(
            &doc,
            &dev,
            &with_transform(Some(affine::translate(5.0, 7.0))),
            None,
        )
        .unwrap();
        let (a, b) = (plain.bounds().unwrap(), moved.bounds().unwrap());
        near(b.min.x - a.min.x, 5.0, 1e-6);
        near(b.min.y - a.min.y, 7.0, 1e-6);
        near(b.max.x - a.max.x, 5.0, 1e-6);
        // Con la identidad nada cambia.
        let same = build(&doc, &dev, &with_transform(Some(affine::IDENTITY)), None).unwrap();
        assert_eq!(same.bounds(), plain.bounds());
    }

    #[test]
    fn a_rotation_is_applied_around_the_bed_origin() {
        let doc = square_doc();
        let dev = DeviceProfile::default();
        // 90° antihorario: (x, y) -> (-y, x); el cuadrado queda con X negativa.
        let job = build(
            &doc,
            &dev,
            &with_transform(Some(affine::rotate(std::f64::consts::FRAC_PI_2))),
            None,
        )
        .unwrap();
        let b = job.bounds().unwrap();
        near(b.min.x, -30.0, 1e-6);
        near(b.max.x, -10.0, 1e-6);
        near(b.min.y, 10.0, 1e-6);
        near(b.max.y, 30.0, 1e-6);
        assert!(check_bounds(&job, dev.bed).is_err());
    }

    #[test]
    fn bounds_check_and_frame_see_the_transform() {
        let doc = square_doc();
        let dev = DeviceProfile::default();
        let ok = build(&doc, &dev, &with_transform(None), None).unwrap();
        assert!(check_bounds(&ok, dev.bed).is_ok());
        let far = affine::translate(dev.bed[0], 0.0);
        let out = build(&doc, &dev, &with_transform(Some(far)), None).unwrap();
        let err = check_bounds(&out, dev.bed).unwrap_err();
        assert!(err.contains("se sale del área"), "{err}");
        // El encuadre sigue al trabajo desplazado: es otro que el sin transformar.
        assert_ne!(
            frame_gcode(&ok, &dev, 1000.0),
            frame_gcode(&out, &dev, 1000.0)
        );
    }

    #[test]
    fn invalid_transforms_are_rejected() {
        let doc = square_doc();
        let dev = DeviceProfile::default();
        let nan = with_transform(Some([f64::NAN, 0.0, 0.0, 1.0, 0.0, 0.0]));
        assert!(build(&doc, &dev, &nan, None).is_err());
        let flat = with_transform(Some([0.0, 0.0, 0.0, 0.0, 1.0, 1.0]));
        assert!(
            build(&doc, &dev, &flat, None)
                .unwrap_err()
                .contains("colapsa")
        );
    }

    fn image_work(settings: &LayerSettings) -> crate::place::LayerWork<'_> {
        crate::place::LayerWork {
            settings,
            color: RED,
            paths: Vec::new(),
            meta: Vec::new(),
            manual_tabs: std::collections::HashMap::new(),
            images: vec![PlacedImage {
                source_path: "x.png".into(),
                origin: Point::new(10.0, 20.0),
                size_mm: (40.0, 20.0),
                mirror_x: false,
                mirror_y: false,
                mask: vec![Polyline {
                    points: vec![
                        Point::new(10.0, 20.0),
                        Point::new(20.0, 20.0),
                        Point::new(20.0, 30.0),
                    ],
                    closed: true,
                }],
                warp: None,
            }],
            parent: 0,
            sub: None,
            sub_total: 0,
        }
    }

    fn moved_by(t: &Transform) -> Placement<'static> {
        // `settings` vive lo que dura el programa: así el Placement se devuelve sin préstamos.
        let settings: &'static LayerSettings = Box::leak(Box::new(LayerSettings::for_color(RED)));
        let mut p = Placement {
            layers: vec![image_work(settings)],
            start: Point::new(0.0, 0.0),
        };
        apply(&mut p, Some(t)).unwrap();
        p
    }

    #[test]
    fn images_are_moved_and_scaled_and_their_mask_follows() {
        let p = moved_by(&affine::compose(
            &affine::translate(2.0, -3.0),
            &affine::scale(2.0, 1.5),
        ));
        let img = &p.layers[0].images[0];
        // Centro (30, 30) -> (62, 42); tamaño (80, 30).
        near(img.size_mm.0, 80.0, 1e-9);
        near(img.size_mm.1, 30.0, 1e-9);
        near(img.origin.x, 22.0, 1e-9);
        near(img.origin.y, 27.0, 1e-9);
        assert!(img.warp.is_none());
        // La máscara también se mueve (antes se quedaba donde estaba).
        near(img.mask[0].points[0].x, 22.0, 1e-9);
        near(img.mask[0].points[2].y, 42.0, 1e-9);
    }

    #[test]
    fn a_mirroring_transform_flips_the_matching_axis_only() {
        let img = &moved_by(&affine::scale(-1.0, 1.0)).layers[0].images[0];
        assert!(img.mirror_x && !img.mirror_y && img.warp.is_none());
        let img = &moved_by(&affine::scale(1.0, -1.0)).layers[0].images[0];
        assert!(!img.mirror_x && img.mirror_y && img.warp.is_none());
    }

    #[test]
    fn a_rotation_turns_the_image_and_keeps_its_axis_aligned_box() {
        // 30 grados alrededor del origen: la caja es la envolvente de las cuatro esquinas.
        let t = affine::rotate(30.0_f64.to_radians());
        let p = moved_by(&t);
        let img = &p.layers[0].images[0];
        let warp = img.warp.as_ref().expect("la imagen gira");
        assert_eq!(warp.src_size, (40.0, 20.0));
        near(warp.src_origin.x, 10.0, 1e-12);
        let corners = warp.corners();
        let min_x = corners.iter().map(|c| c.x).fold(f64::MAX, f64::min);
        let max_y = corners.iter().map(|c| c.y).fold(f64::MIN, f64::max);
        near(img.origin.x, min_x, 1e-9);
        near(img.origin.y + img.size_mm.1, max_y, 1e-9);
        // Una caja de 40 x 20 girada 30 grados mide 40·cos + 20·sin de ancho.
        let (c, s) = (30.0_f64.to_radians().cos(), 30.0_f64.to_radians().sin());
        near(img.size_mm.0, 40.0 * c + 20.0 * s, 1e-9);
        near(img.size_mm.1, 40.0 * s + 20.0 * c, 1e-9);
        // Un giro mínimo (por debajo del umbral) deja la imagen alineada.
        let tiny = moved_by(&affine::rotate(5e-5));
        assert!(tiny.layers[0].images[0].warp.is_none());
    }

    #[test]
    fn fit_choice_resolves_like_the_wizard() {
        let two = [[0.0, 0.0], [50.0, 0.0]];
        let tri = [[0.0, 0.0], [50.0, 0.0], [0.0, 80.0]];
        let line = [[0.0, 0.0], [50.0, 0.0], [100.0, 0.0]];
        assert_eq!(FitChoice::Auto.resolve(&two), FitMode::Similarity);
        assert_eq!(FitChoice::Auto.resolve(&tri), FitMode::Affine);
        // Tres marcas en línea: la afín no se puede resolver y cae a la similitud.
        assert_eq!(FitChoice::Auto.resolve(&line), FitMode::Similarity);
        assert_eq!(FitChoice::Affine.resolve(&two), FitMode::Similarity);
        assert_eq!(FitChoice::Affine.resolve(&tri), FitMode::Affine);
        assert_eq!(FitChoice::Rigid.resolve(&tri), FitMode::Rigid);
        assert_eq!(FitChoice::Similarity.resolve(&line), FitMode::Similarity);
    }

    #[test]
    fn collinearity_check() {
        assert!(are_collinear(&[[0.0, 0.0], [50.0, 0.0], [100.0, 0.0]]));
        assert!(!are_collinear(&[[0.0, 0.0], [50.0, 0.0], [0.0, 100.0]]));
        assert!(!are_collinear(&[[0.0, 0.0], [50.0, 0.0]]));
        assert!(are_collinear(&[[1.0, 1.0], [1.0, 1.0], [1.0, 1.0]]));
    }

    #[test]
    fn wire_format_matches_what_the_interface_reads() {
        // Los nombres de campos son el contrato con la interfaz (TypeScript).
        let t = affine::compose(&affine::translate(1.0, 2.0), &affine::scale(1.02, 1.0));
        let fit = fit_registration(
            &pairs_of(&t, &[[0.0, 0.0], [200.0, 0.0], [0.0, 150.0]]),
            FitMode::Affine,
        )
        .unwrap();
        let v = serde_json::to_value(&fit).unwrap();
        for key in [
            "transform",
            "mode",
            "rotationDeg",
            "scale",
            "scaleXY",
            "residuals",
            "residualRms",
            "residualMax",
            "warnings",
        ] {
            assert!(v.get(key).is_some(), "falta {key}: {v}");
        }
        assert_eq!(v["mode"], "affine");
        assert!(
            v["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w["kind"] == "anisotropy" && w["xPct"].is_number()),
            "{v}"
        );
        let e = serde_json::to_value(FitError::Few { need: 3, have: 2 }).unwrap();
        assert_eq!(e, serde_json::json!({"kind": "few", "need": 3, "have": 2}));
        let e = serde_json::to_value(FitError::Close { a: 1, b: 2 }).unwrap();
        assert_eq!(e, serde_json::json!({"kind": "close", "a": 1, "b": 2}));
        assert_eq!(
            serde_json::to_value(FitError::Collinear).unwrap(),
            serde_json::json!({"kind": "collinear"})
        );
        assert_eq!(
            serde_json::to_value(FitWarning::Mirrored).unwrap(),
            serde_json::json!({"kind": "mirrored"})
        );
        // Y la entrada: el modo en minúsculas y `auto`.
        let c: FitChoice = serde_json::from_str("\"auto\"").unwrap();
        assert_eq!(c, FitChoice::Auto);
        let m: FitMode = serde_json::from_str("\"rigid\"").unwrap();
        assert_eq!(m, FitMode::Rigid);
    }

    /// Imagen real que Print & Cut gira: el trabajo se calcula, su caja es la de la imagen girada
    /// y lo que se graba cambia respecto de la imagen sin girar.
    #[test]
    fn a_rotated_image_is_engraved_turned() {
        use tlaku_doc::{ImageObject, JobOptions as Jo};
        let dir = std::env::temp_dir().join(format!("tlaku-reg-img-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("barra.png");
        // 40 x 20 px: la mitad izquierda negra, la derecha blanca.
        image::GrayImage::from_fn(40, 20, |x, _| image::Luma([if x < 20 { 0 } else { 255 }]))
            .save(&src)
            .unwrap();
        let mut layer = LayerSettings::for_images();
        layer.interval = 0.5;
        let color = layer.color;
        let doc = Document {
            layers: vec![layer],
            objects: vec![DesignObject {
                id: "foto".into(),
                layer: color,
                transform: affine::translate(20.0, 20.0),
                kind: ObjectKind::Image(ImageObject {
                    source_path: src.to_string_lossy().into_owned(),
                    width_px: 40,
                    height_px: 20,
                    size_mm: [20.0, 10.0],
                    mask: Vec::new(),
                }),
                locked: false,
                hidden: false,
                group: None,
                priority: None,
                power_scale: None,
                shape: None,
                start: None,
                manual_tabs: Vec::new(),
                array: None,
            }],
            ..Document::default()
        };
        let dev = DeviceProfile::default();
        let plain = build(&doc, &dev, &Jo::default(), None).unwrap();
        // Giro de 90 grados alrededor del centro de la imagen (30, 25): la caja pasa a 10 x 20.
        let t = affine::compose(
            &affine::translate(30.0, 25.0),
            &affine::compose(
                &affine::rotate(std::f64::consts::FRAC_PI_2),
                &affine::translate(-30.0, -25.0),
            ),
        );
        let turned = build(
            &doc,
            &dev,
            &Jo {
                transform: Some(t),
                ..Jo::default()
            },
            None,
        )
        .unwrap();
        // Lo grabado es la mitad izquierda negra: sin girar es x 20..30, y 20..30; girada 90
        // grados alrededor de (30, 25) queda abajo, en x 25..35, y 15..25 (el centro baja a y 20).
        let lit = |job: &crate::ComputedJob| {
            let mut b = job.bounds().unwrap();
            // El overscan ensancha el barrido a los lados: se compara solo el centro.
            b.min.x += 0.0;
            b.max.x += 0.0;
            Point::new((b.min.x + b.max.x) / 2.0, (b.min.y + b.max.y) / 2.0)
        };
        let c = lit(&turned);
        near(c.x, 30.0, 0.8);
        near(c.y, 20.0, 0.8);
        let c = lit(&plain);
        near(c.x, 25.0, 0.8);
        near(c.y, 25.0, 0.8);
        // Es la mitad de la imagen en ambos casos: 10 mm de alto.
        let h = |job: &crate::ComputedJob| {
            let b = job.bounds().unwrap();
            b.max.y - b.min.y
        };
        near(h(&plain), 10.0, 0.6);
        near(h(&turned), 10.0, 0.6);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn fit_then_apply_lands_the_marks_on_the_physical_points() {
        // El flujo completo: se mide el desfase y el trabajo cae sobre las marcas físicas.
        let truth = affine::compose(&affine::translate(3.2, 1.5), &affine::rotate(0.014));
        let pairs = pairs_of(&truth, &[[10.0, 10.0], [30.0, 30.0]]);
        let fit = fit_registration(&pairs, FitMode::Similarity).unwrap();
        let doc = square_doc();
        let dev = DeviceProfile::default();
        let job = build(&doc, &dev, &with_transform(Some(fit.transform)), None).unwrap();
        let expected = build(&doc, &dev, &with_transform(Some(truth)), None).unwrap();
        let (a, b) = (job.bounds().unwrap(), expected.bounds().unwrap());
        near(a.min.x, b.min.x, 1e-6);
        near(a.max.y, b.max.y, 1e-6);
    }
}
