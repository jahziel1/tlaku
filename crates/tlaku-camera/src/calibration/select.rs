//! Elección del modelo (Brown-Conrady u ojo de pez) y validación cruzada.

use super::fit::{Fit, fit_model, init_poses};
use super::intrinsics::residuals_view;
use super::lm::levenberg_marquardt;
use super::model::undistort_with;
use super::{CalibrationOptions, LensModel, ModelScore};
use tlaku_core::Point;
use tlaku_i18n::tr;

pub(super) fn fmt_score(v: f64) -> String {
    if v.is_finite() {
        format!("{v:.2}")
    } else {
        tr!("camera.select.no_converge")
    }
}

/// El modelo que ajusta mejor con todas las vistas (Brown-Conrady en un empate), o `None` si
/// ninguno ajustó.
pub(super) fn pick_leader(first: &[(LensModel, Result<Fit, String>)]) -> Option<LensModel> {
    let mut best: Option<(LensModel, f64)> = None;
    for (m, r) in first {
        let Ok(f) = r else { continue };
        let better = match best {
            None => true,
            // El ojo de pez tiene más grados de libertad: tiene que ganar con claridad.
            Some((_, rms)) => *m == LensModel::Fisheye && f.rms < 0.8 * rms,
        };
        if better {
            best = Some((*m, f.rms));
        }
    }
    best.map(|(m, _)| m)
}

/// Un solo modelo posible o dos: el ojo de pez solo se elige si mejora con claridad el error en
/// las vistas reservadas (o, sin validación cruzada, el error de reproyección): al menos un 15 %
/// y 0.1 px. Así una lente normal no cambia de modelo por ruido.
pub(super) fn select_model(scores: &[ModelScore]) -> LensModel {
    let key = |s: &ModelScore| {
        let v = s.cv_rms_px.unwrap_or(s.rms_px);
        if v.is_finite() { v } else { f64::INFINITY }
    };
    let brown = scores.iter().find(|s| s.model == LensModel::BrownConrady);
    let fish = scores.iter().find(|s| s.model == LensModel::Fisheye);
    match (brown, fish) {
        (Some(b), Some(f)) => {
            let (kb, kf) = (key(b), key(f));
            if kf < 0.85 * kb && kb - kf > 0.1 {
                LensModel::Fisheye
            } else {
                LensModel::BrownConrady
            }
        }
        (Some(_), None) => LensModel::BrownConrady,
        (None, Some(_)) => LensModel::Fisheye,
        (None, None) => LensModel::BrownConrady,
    }
}

/// Factor de la focal de la imagen corregida del ojo de pez: que el borde horizontal de la
/// imagen cruda (hasta 60° del eje) quepa en la imagen corregida. Nunca mayor que 1.
pub(super) fn auto_view_zoom(intr: &[f64; 10], size: (u32, u32)) -> f64 {
    let (w, cx) = (f64::from(size.0), intr[2]);
    let half_w = cx.max(w - cx);
    let k = [intr[4], intr[5], intr[8], intr[9]];
    let (x, _) = undistort_with(LensModel::Fisheye, k, [0.0, 0.0], half_w / intr[0], 0.0);
    let theta = x.abs().atan().min(60f64.to_radians());
    if theta < 1e-3 {
        return 1.0;
    }
    (half_w / theta.tan() / intr[0]).clamp(0.25, 1.0)
}

/// Validación cruzada con vistas reservadas: se reparten las vistas en pliegues, se ajusta sin
/// un pliegue y se mide el error en él con la pose recalculada (los intrínsecos no se tocan).
/// `Some(INFINITY)` si el modelo no logra ajustar un pliegue; `None` con menos de 4 vistas.
pub(super) fn cross_validate(
    model: LensModel,
    views: &[&[Point]],
    obj: &[[f64; 2]],
    size: (u32, u32),
    opts: &CalibrationOptions,
) -> Option<f64> {
    let n = views.len();
    if n < 4 {
        return None;
    }
    let folds = if n >= 6 { 3 } else { n };
    let (mut sum, mut count) = (0.0, 0usize);
    for f in 0..folds {
        let train: Vec<&[Point]> = (0..n)
            .filter(|i| i % folds != f)
            .map(|i| views[i])
            .collect();
        let Ok(fit) = fit_model(model, &train, obj, size, opts) else {
            return Some(f64::INFINITY);
        };
        for held in (0..n).filter(|i| i % folds == f).map(|i| views[i]) {
            sum += holdout_sq_error(model, &fit.intr, obj, held);
            count += obj.len();
        }
    }
    let rms = (sum / count as f64).sqrt();
    Some(if rms.is_finite() { rms } else { f64::INFINITY })
}

/// Suma de cuadrados del error de una vista no usada en el ajuste, con la pose recalculada.
fn holdout_sq_error(model: LensModel, intr: &[f64; 10], obj: &[[f64; 2]], view: &[Point]) -> f64 {
    let mut poses = init_poses(model, intr, &[view], obj);
    let mut local = *intr;
    levenberg_marquardt(model, &mut local, &mut poses, &[view], obj, &[], 60);
    let mut buf = vec![0.0; 2 * obj.len()];
    residuals_view(model, intr, &poses[0], obj, view, &mut buf);
    buf.iter().map(|r| r * r).sum()
}
