//! Ajuste de un modelo: poses iniciales, Brown-Conrady y ojo de pez.

use super::intrinsics::{initial_intrinsics, pose_from_homography, residuals_view};
use super::lm::levenberg_marquardt;
use super::model::undistort_with;
use super::{CalibrationOptions, LensModel};
use crate::homography::estimate_homography;
use nalgebra::Matrix3;
use tlaku_core::Point;

#[derive(Clone)]
pub(super) struct Fit {
    /// fx, fy, cx, cy, k1, k2, p1, p2, k3, k4.
    pub(super) intr: [f64; 10],
    pub(super) rms: f64,
    pub(super) view_rms: Vec<f64>,
}

pub(super) fn fit_model(
    model: LensModel,
    views: &[&[Point]],
    obj: &[[f64; 2]],
    size: (u32, u32),
    opts: &CalibrationOptions,
) -> Result<Fit, String> {
    match model {
        LensModel::BrownConrady => fit(views, obj, size, opts),
        LensModel::Fisheye => fit_fisheye(views, obj, size, opts),
    }
}

fn fit(
    views: &[&[Point]],
    obj: &[[f64; 2]],
    size: (u32, u32),
    opts: &CalibrationOptions,
) -> Result<Fit, String> {
    let (w, h) = (f64::from(size.0), f64::from(size.1));
    let obj_pts: Vec<Point> = obj.iter().map(|o| Point::new(o[0], o[1])).collect();
    let mut hs = Vec::with_capacity(views.len());
    for v in views {
        let pairs: Vec<(Point, Point)> = obj_pts.iter().copied().zip(v.iter().copied()).collect();
        let hm = estimate_homography(&pairs)
            .map_err(|e| format!("no se pudo calcular la homografía de una vista: {e}"))?;
        hs.push(hm);
    }
    let (fx, fy, cx, cy) = initial_intrinsics(&hs, w, h)?;
    let k = Matrix3::new(fx, 0.0, cx, 0.0, fy, cy, 0.0, 0.0, 1.0);
    let k_inv = k
        .try_inverse()
        .ok_or("matriz de cámara inicial no invertible")?;
    let mut poses: Vec<[f64; 6]> = hs
        .iter()
        .map(|hm| pose_from_homography(hm, &k_inv))
        .collect();
    let mut intr = [fx, fy, cx, cy, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

    let mut free: Vec<usize> = vec![0, 1, 2, 3, 4, 5];
    if opts.fit_tangential {
        free.extend([6, 7]);
    }
    if opts.fit_k3 {
        free.push(8);
    }
    let model = LensModel::BrownConrady;
    levenberg_marquardt(
        model,
        &mut intr,
        &mut poses,
        views,
        obj,
        &free,
        opts.max_iterations,
    );
    finish_fit(model, intr, &poses, views, obj)
}

/// Ojo de pez: no hay solución cerrada, así que se prueban varias focales supuestas (según
/// cuántos radianes llegue la esquina de la imagen) y gana la de menor error. Cada una se
/// refina en dos etapas: primero solo focal y `k1` con el centro fijo (así las poses iniciales,
/// que salen de la focal supuesta, se corrigen sin que el centro se vaya) y luego todo.
fn fit_fisheye(
    views: &[&[Point]],
    obj: &[[f64; 2]],
    size: (u32, u32),
    opts: &CalibrationOptions,
) -> Result<Fit, String> {
    let (w, h) = (f64::from(size.0), f64::from(size.1));
    let half_diag = w.hypot(h) / 2.0;
    let model = LensModel::Fisheye;
    let mut best: Option<Fit> = None;
    for corner_theta in [1.0_f64, 1.4, 1.8] {
        let f0 = half_diag / corner_theta;
        let mut intr = [f0, f0, w / 2.0, h / 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let mut poses = init_poses(model, &intr, views, obj);
        levenberg_marquardt(model, &mut intr, &mut poses, views, obj, &[0, 1, 4], 80);
        levenberg_marquardt(
            model,
            &mut intr,
            &mut poses,
            views,
            obj,
            &[0, 1, 2, 3, 4, 5],
            opts.max_iterations,
        );
        // k3 y k4 solo si hacen falta: con poco campo cubierto se compensan entre sí y
        // extrapolan mal fuera de donde se vio el tablero.
        let rms_now = finish_fit(model, intr, &poses, views, obj).map_or(f64::INFINITY, |f| f.rms);
        if opts.fit_k3 || rms_now > 0.4 {
            levenberg_marquardt(
                model,
                &mut intr,
                &mut poses,
                views,
                obj,
                &[0, 1, 2, 3, 4, 5, 8, 9],
                opts.max_iterations,
            );
        }
        if let Ok(f) = finish_fit(model, intr, &poses, views, obj)
            && best.as_ref().is_none_or(|b| f.rms < b.rms)
        {
            best = Some(f);
        }
    }
    best.ok_or_else(|| {
        "el modelo de ojo de pez no convergió; toma más vistas, con el tablero inclinado y cerca de las esquinas"
            .into()
    })
}

/// Poses iniciales de cada vista con una lente supuesta: los píxeles se llevan a rayos con el
/// modelo, se ajusta la homografía tablero a plano normalizado y se descompone.
pub(super) fn init_poses(
    model: LensModel,
    intr: &[f64; 10],
    views: &[&[Point]],
    obj: &[[f64; 2]],
) -> Vec<[f64; 6]> {
    let k = [intr[4], intr[5], intr[8], intr[9]];
    let p = [intr[6], intr[7]];
    views
        .iter()
        .map(|v| {
            let pairs: Vec<(Point, Point)> = obj
                .iter()
                .zip(v.iter())
                .filter_map(|(o, px)| {
                    let (x, y) = undistort_with(
                        model,
                        k,
                        p,
                        (px.x - intr[2]) / intr[0],
                        (px.y - intr[3]) / intr[1],
                    );
                    // Rayos casi horizontales dan coordenadas enormes que arruinan la homografía.
                    (x.is_finite() && y.is_finite() && x.abs() < 6.0 && y.abs() < 6.0)
                        .then(|| (Point::new(o[0], o[1]), Point::new(x, y)))
                })
                .collect();
            match estimate_homography(&pairs) {
                Ok(hm) if hm.m.iter().flatten().all(|v| v.is_finite()) => {
                    pose_from_homography(&hm, &Matrix3::identity())
                }
                _ => [0.0, 0.0, 0.0, 0.0, 0.0, 500.0],
            }
        })
        .collect()
}

/// Valida el resultado del LM y calcula el error global y por vista.
fn finish_fit(
    model: LensModel,
    intr: [f64; 10],
    poses: &[[f64; 6]],
    views: &[&[Point]],
    obj: &[[f64; 2]],
) -> Result<Fit, String> {
    if !intr.iter().all(|v| v.is_finite()) || intr[0] <= 0.0 || intr[1] <= 0.0 {
        return Err("la calibración no convergió; toma más vistas con el tablero inclinado".into());
    }
    let mut view_rms = Vec::with_capacity(views.len());
    let mut total = 0.0;
    let mut count = 0usize;
    let mut buf = vec![0.0; 2 * obj.len()];
    for (v, pose) in views.iter().zip(poses) {
        residuals_view(model, &intr, pose, obj, v, &mut buf);
        let s: f64 = buf.iter().map(|r| r * r).sum();
        total += s;
        count += obj.len();
        view_rms.push((s / obj.len() as f64).sqrt());
    }
    let rms = (total / count as f64).sqrt();
    if !rms.is_finite() {
        return Err("la calibración no convergió; toma más vistas con el tablero inclinado".into());
    }
    Ok(Fit {
        intr,
        rms,
        view_rms,
    })
}
