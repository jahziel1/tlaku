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

use crate::place::Placement;
use tlaku_core::Point;
use tlaku_doc::affine::{self, Transform};

/// Desviación de la escala respecto de 1 (fracción) a partir de la cual se avisa: 0,5 %.
pub const SCALE_WARN: f64 = 0.005;

/// Distancia mínima (mm) entre dos marcas: más cerca, la rotación no se puede medir.
pub const MIN_SEPARATION_MM: f64 = 1.0;

/// Una marca: dónde está en el diseño y dónde se midió en la cama, en mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkPair {
    pub design: [f64; 2],
    pub physical: [f64; 2],
}

/// Qué tipo de transformación se busca.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Resultado del ajuste.
#[derive(Debug, Clone, PartialEq)]
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
    pub scale_xy: [f64; 2],
    /// Error de cada marca tras transformar, en mm (distancia al punto físico).
    pub residuals: Vec<f64>,
    /// Raíz del error cuadrático medio de `residuals`, en mm.
    pub residual_rms: f64,
    /// El peor de `residuals`, en mm.
    pub residual_max: f64,
    /// Avisos para mostrar al usuario (escala, deformación, espejo).
    pub warnings: Vec<String>,
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
pub fn fit_registration(pairs: &[MarkPair], mode: FitMode) -> Result<RegistrationFit, String> {
    let need = if mode == FitMode::Affine { 3 } else { 2 };
    if pairs.len() < need {
        return Err(match mode {
            FitMode::Affine => format!(
                "la transformación afín pide 3 marcas o más (hay {})",
                pairs.len()
            ),
            _ => format!("hacen falta 2 marcas como mínimo (hay {})", pairs.len()),
        });
    }
    if pairs
        .iter()
        .any(|p| !finite2(p.design) || !finite2(p.physical))
    {
        return Err("una marca tiene coordenadas que no son números".into());
    }
    for (i, a) in pairs.iter().enumerate() {
        for (j, b) in pairs.iter().enumerate().skip(i + 1) {
            if dist(a.design, b.design) < MIN_SEPARATION_MM
                || dist(a.physical, b.physical) < MIN_SEPARATION_MM
            {
                return Err(format!(
                    "las marcas {} y {} están demasiado cerca (menos de {MIN_SEPARATION_MM} mm): \
                     no se puede medir el giro",
                    i + 1,
                    j + 1
                ));
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
        return Err("las marcas del diseño coinciden".into());
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
                return Err(
                    "las marcas están en línea recta: la transformación afín necesita \
                     que no estén alineadas"
                        .into(),
                );
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
        warnings.push(
            "la impresión parece espejada (la transformación invierte el diseño): revisa que \
             las marcas físicas estén en el mismo orden que las del diseño"
                .to_string(),
        );
    }
    if (scale - 1.0).abs() > SCALE_WARN {
        warnings.push(format!(
            "la escala medida es {:.2} % (más de {:.1} % lejos de 100 %): la impresión puede \
             estar deformada o la referencia es mala; revisa la escala de impresión y las posiciones",
            scale * 100.0,
            SCALE_WARN * 100.0
        ));
    }
    if mode == FitMode::Affine
        && det > 0.0
        && (scale_xy[0] - scale_xy[1]).abs() / scale_xy[0].max(scale_xy[1]) > SCALE_WARN
    {
        warnings.push(format!(
            "la impresión está estirada de forma distinta en cada eje (X {:.2} %, Y {:.2} %)",
            scale_xy[0] * 100.0,
            scale_xy[1] * 100.0
        ));
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

fn map_point(t: &Transform, p: Point) -> Point {
    let [x, y] = affine::apply(t, [p.x, p.y]);
    Point::new(x, y)
}

/// Aplica la transformación de Print & Cut a un trabajo ya colocado: trayectorias, puentes
/// manuales, imágenes y punto de inicio. Las imágenes se reubican y escalan pero no se giran
/// (se rasterizan alineadas a los ejes): si la transformación gira, se avisa.
pub(crate) fn apply(
    placement: &mut Placement,
    transform: Option<&Transform>,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let Some(t) = transform else {
        return Ok(());
    };
    if !affine::is_finite(t) {
        return Err(
            "la transformación de Print & Cut no es válida (tiene valores que no son números)"
                .into(),
        );
    }
    let det = t[0] * t[3] - t[1] * t[2];
    if det.abs() < 1e-12 {
        return Err(
            "la transformación de Print & Cut colapsa el diseño (determinante cero)".into(),
        );
    }
    if affine::is_identity(t) {
        return Ok(());
    }
    placement.start = map_point(t, placement.start);
    let mut rotated_images = 0usize;
    for layer in &mut placement.layers {
        for p in layer.paths.iter_mut().flat_map(|p| &mut p.points) {
            *p = map_point(t, *p);
        }
        for p in layer.manual_tabs.values_mut().flatten() {
            *p = map_point(t, *p);
        }
        for img in &mut layer.images {
            let center = map_point(
                t,
                Point::new(
                    img.origin.x + img.size_mm.0 / 2.0,
                    img.origin.y + img.size_mm.1 / 2.0,
                ),
            );
            let (sx, sy) = (t[0].hypot(t[1]), t[2].hypot(t[3]));
            img.size_mm = (img.size_mm.0 * sx, img.size_mm.1 * sy);
            img.origin = Point::new(
                center.x - img.size_mm.0 / 2.0,
                center.y - img.size_mm.1 / 2.0,
            );
            if det < 0.0 {
                img.mirror_x = !img.mirror_x;
            }
            // Más de 0,05° de giro ya mueve el borde de una imagen de 100 mm unos 0,09 mm.
            if t[1].atan2(t[0]).to_degrees().abs() > 0.05 {
                rotated_images += 1;
            }
        }
    }
    if rotated_images > 0 {
        warnings.push(format!(
            "Print & Cut: {rotated_images} imagen(es) se reubican y escalan pero no se giran \
             (se graban alineadas a los ejes); el giro del registro solo se aplica a los vectores"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
            bad.warnings.iter().any(|w| w.contains("escala")),
            "{:?}",
            bad.warnings
        );
        // También por debajo de 1 y en modo rígido (avisa con la escala que haría falta).
        let small =
            fit_registration(&pairs_of(&affine::scale(0.99, 0.99), &pts), FitMode::Rigid).unwrap();
        assert!(small.warnings.iter().any(|w| w.contains("escala")));
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
            fit.warnings.iter().any(|w| w.contains("cada eje")),
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
        assert!(fit.warnings.iter().any(|w| w.contains("espejada")));
    }

    #[test]
    fn rejects_bad_input() {
        let one = [pair([0.0, 0.0], [1.0, 1.0])];
        assert!(fit_registration(&one, FitMode::Similarity).is_err());
        let two = [pair([0.0, 0.0], [0.0, 0.0]), pair([50.0, 0.0], [50.0, 0.0])];
        assert!(fit_registration(&two, FitMode::Affine).is_err());
        // Marcas que coinciden en el diseño o en la cama.
        let same_d = [pair([0.0, 0.0], [0.0, 0.0]), pair([0.2, 0.0], [50.0, 0.0])];
        assert!(
            fit_registration(&same_d, FitMode::Similarity)
                .unwrap_err()
                .contains("cerca")
        );
        let same_p = [pair([0.0, 0.0], [0.0, 0.0]), pair([50.0, 0.0], [0.5, 0.0])];
        assert!(fit_registration(&same_p, FitMode::Similarity).is_err());
        // Tres marcas en línea recta no sirven para una afín, pero sí para la similitud.
        let line = pairs_of(&affine::IDENTITY, &[[0.0, 0.0], [50.0, 0.0], [100.0, 0.0]]);
        assert!(
            fit_registration(&line, FitMode::Affine)
                .unwrap_err()
                .contains("línea")
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

    #[test]
    fn images_are_moved_and_scaled_but_a_rotation_is_reported() {
        use crate::place::{LayerWork, PlacedImage};
        use std::collections::HashMap;
        let settings = LayerSettings::for_color(RED);
        let work = |settings| LayerWork {
            settings,
            color: RED,
            paths: Vec::new(),
            meta: Vec::new(),
            manual_tabs: HashMap::new(),
            images: vec![PlacedImage {
                source_path: "x.png".into(),
                origin: Point::new(10.0, 20.0),
                size_mm: (40.0, 20.0),
                mirror_x: false,
                mirror_y: false,
                mask: Vec::new(),
            }],
            parent: 0,
            sub: None,
            sub_total: 0,
        };
        let mut moved = Placement {
            layers: vec![work(&settings)],
            start: Point::new(0.0, 0.0),
        };
        let mut warnings = Vec::new();
        apply(
            &mut moved,
            Some(&affine::translate(2.0, -3.0)),
            &mut warnings,
        )
        .unwrap();
        let img = &moved.layers[0].images[0];
        near(img.origin.x, 12.0, 1e-9);
        near(img.origin.y, 17.0, 1e-9);
        assert!(warnings.is_empty());

        let mut turned = Placement {
            layers: vec![work(&settings)],
            start: Point::new(0.0, 0.0),
        };
        apply(&mut turned, Some(&affine::rotate(0.02)), &mut warnings).unwrap();
        assert!(
            warnings.iter().any(|w| w.contains("no se giran")),
            "{warnings:?}"
        );
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
