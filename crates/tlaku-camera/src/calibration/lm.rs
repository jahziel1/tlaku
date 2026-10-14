//! Levenberg-Marquardt sobre intrínsecos y poses.

use super::LensModel;
use super::intrinsics::residuals_view;
use nalgebra::{DMatrix, DVector};
use tlaku_core::Point;

/// Levenberg-Marquardt sobre intrínsecos libres (`free`) y las poses de todas las vistas.
/// El jacobiano se obtiene por diferencias centrales.
pub(super) fn levenberg_marquardt(
    model: LensModel,
    intr: &mut [f64; 10],
    poses: &mut [[f64; 6]],
    views: &[&[Point]],
    obj: &[[f64; 2]],
    free: &[usize],
    max_iter: usize,
) {
    let nv = views.len();
    let np = obj.len();
    let nf = free.len();
    let ncols = nf + 6 * nv;
    let nrows = 2 * np * nv;

    let full_residuals = |intr: &[f64; 10], poses: &[[f64; 6]], out: &mut [f64]| {
        for (vi, v) in views.iter().enumerate() {
            residuals_view(
                model,
                intr,
                &poses[vi],
                obj,
                v,
                &mut out[vi * 2 * np..(vi + 1) * 2 * np],
            );
        }
    };

    let mut r = vec![0.0; nrows];
    full_residuals(intr, poses, &mut r);
    let mut cost: f64 = r.iter().map(|v| v * v).sum::<f64>() * 0.5;
    let mut lambda = 1e-3;

    for _ in 0..max_iter {
        let mut jac = DMatrix::<f64>::zeros(nrows, ncols);
        let (mut rp, mut rm) = (vec![0.0; nrows], vec![0.0; nrows]);
        for (c, &fi) in free.iter().enumerate() {
            let h = 1e-6 * intr[fi].abs().max(1.0);
            let mut ip = *intr;
            ip[fi] += h;
            let mut im = *intr;
            im[fi] -= h;
            full_residuals(&ip, poses, &mut rp);
            full_residuals(&im, poses, &mut rm);
            for i in 0..nrows {
                jac[(i, c)] = (rp[i] - rm[i]) / (2.0 * h);
            }
        }
        let (mut vp, mut vm) = (vec![0.0; 2 * np], vec![0.0; 2 * np]);
        for vi in 0..nv {
            for k in 0..6 {
                let h = 1e-6 * poses[vi][k].abs().max(1.0);
                let mut pp = poses[vi];
                pp[k] += h;
                let mut pm = poses[vi];
                pm[k] -= h;
                residuals_view(model, intr, &pp, obj, views[vi], &mut vp);
                residuals_view(model, intr, &pm, obj, views[vi], &mut vm);
                let col = nf + 6 * vi + k;
                for i in 0..2 * np {
                    jac[(vi * 2 * np + i, col)] = (vp[i] - vm[i]) / (2.0 * h);
                }
            }
        }
        let rv = DVector::from_row_slice(&r);
        let jt = jac.transpose();
        let jtj = &jt * &jac;
        let g = &jt * &rv;
        if g.amax() < 1e-9 {
            break;
        }
        let mut improved = false;
        for _ in 0..12 {
            let mut a = jtj.clone();
            for i in 0..ncols {
                a[(i, i)] += lambda * jtj[(i, i)].max(1e-9);
            }
            let Some(dx) = a.lu().solve(&(-&g)) else {
                lambda *= 10.0;
                continue;
            };
            let mut ni = *intr;
            for (c, &fi) in free.iter().enumerate() {
                ni[fi] += dx[c];
            }
            let mut np_ = poses.to_vec();
            for (vi, p) in np_.iter_mut().enumerate() {
                for (k, pk) in p.iter_mut().enumerate() {
                    *pk += dx[nf + 6 * vi + k];
                }
            }
            let mut rn = vec![0.0; nrows];
            full_residuals(&ni, &np_, &mut rn);
            let ncost: f64 = rn.iter().map(|v| v * v).sum::<f64>() * 0.5;
            if ncost.is_finite() && ncost < cost {
                let rel = (cost - ncost) / cost.max(1e-300);
                *intr = ni;
                poses.copy_from_slice(&np_);
                r = rn;
                cost = ncost;
                lambda = (lambda / 3.0).max(1e-12);
                improved = true;
                if rel < 1e-12 || dx.amax() < 1e-10 {
                    return;
                }
                break;
            }
            lambda *= 4.0;
        }
        if !improved {
            break;
        }
    }
}
