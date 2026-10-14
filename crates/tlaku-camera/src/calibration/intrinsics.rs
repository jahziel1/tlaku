//! Solución cerrada de Zhang, poses desde homografías y residuos de reproyección.

use super::LensModel;
use super::model::distort_with;
use crate::homography::Homography;
use nalgebra::{DMatrix, DVector, Matrix3, Rotation3, Vector3};
use tlaku_core::Point;
use tlaku_i18n::tr;

/// Solución cerrada de Zhang con sesgo nulo, trabajando en píxeles centrados y
/// escalados para mejorar el condicionamiento. Si falla, reintenta con el punto
/// principal en el centro de la imagen.
pub(super) fn initial_intrinsics(
    hs: &[Homography],
    w: f64,
    h: f64,
) -> Result<(f64, f64, f64, f64), String> {
    let s = w.max(h);
    let t = Matrix3::new(
        1.0 / s,
        0.0,
        -w / (2.0 * s),
        0.0,
        1.0 / s,
        -h / (2.0 * s),
        0.0,
        0.0,
        1.0,
    );
    let hn: Vec<Matrix3<f64>> = hs
        .iter()
        .map(|hm| {
            let m = t * Matrix3::from_fn(|r, c| hm.m[r][c]);
            m / m.norm()
        })
        .collect();
    let back = |fxn: f64, fyn: f64, cxn: f64, cyn: f64| {
        (fxn * s, fyn * s, cxn * s + w / 2.0, cyn * s + h / 2.0)
    };

    let v_ij = |m: &Matrix3<f64>, i: usize, j: usize| -> [f64; 6] {
        let a = |k: usize, c: usize| m[(k, c)];
        [
            a(0, i) * a(0, j),
            a(0, i) * a(1, j) + a(1, i) * a(0, j),
            a(1, i) * a(1, j),
            a(2, i) * a(0, j) + a(0, i) * a(2, j),
            a(2, i) * a(1, j) + a(1, i) * a(2, j),
            a(2, i) * a(2, j),
        ]
    };
    let mut rows: Vec<[f64; 6]> = Vec::new();
    for m in &hn {
        rows.push(v_ij(m, 0, 1));
        let (v11, v22) = (v_ij(m, 0, 0), v_ij(m, 1, 1));
        let mut d = [0.0; 6];
        for k in 0..6 {
            d[k] = v11[k] - v22[k];
        }
        rows.push(d);
    }
    rows.push([0.0, 1.0, 0.0, 0.0, 0.0, 0.0]);
    let nrows = rows.len().max(6);
    let mut a = DMatrix::<f64>::zeros(nrows, 6);
    for (i, r) in rows.iter().enumerate() {
        for k in 0..6 {
            a[(i, k)] = r[k];
        }
    }
    let svd = a.svd(false, true);
    if let Some(v_t) = svd.v_t {
        let sv = &svd.singular_values;
        let idx = (0..sv.len())
            .min_by(|&i, &j| sv[i].total_cmp(&sv[j]))
            .unwrap_or(0);
        let mut b: Vec<f64> = (0..6).map(|k| v_t[(idx, k)]).collect();
        if b[0] < 0.0 {
            b.iter_mut().for_each(|v| *v = -*v);
        }
        let (b11, b22, b13, b23, b33) = (b[0], b[2], b[3], b[4], b[5]);
        if b11 > 0.0 && b22 > 0.0 {
            let lambda = b33 - b13 * b13 / b11 - b23 * b23 / b22;
            if lambda > 0.0 {
                let (cxn, cyn) = (-b13 / b11, -b23 / b22);
                let (fxn, fyn) = ((lambda / b11).sqrt(), (lambda / b22).sqrt());
                let r = back(fxn, fyn, cxn, cyn);
                let plausible = r.0.is_finite()
                    && r.1.is_finite()
                    && r.2 > 0.0
                    && r.2 < w
                    && r.3 > 0.0
                    && r.3 < h
                    && r.0 > 0.1 * s
                    && r.1 > 0.1 * s;
                if plausible {
                    return Ok(r);
                }
            }
        }
    }
    // Respaldo: punto principal en el centro; incógnitas u = 1/fx^2 y v = 1/fy^2.
    let mut rows2: Vec<[f64; 3]> = Vec::new();
    for m in &hn {
        let c1 = [m[(0, 0)], m[(1, 0)], m[(2, 0)]];
        let c2 = [m[(0, 1)], m[(1, 1)], m[(2, 1)]];
        rows2.push([c1[0] * c2[0], c1[1] * c2[1], -c1[2] * c2[2]]);
        rows2.push([
            c1[0] * c1[0] - c2[0] * c2[0],
            c1[1] * c1[1] - c2[1] * c2[1],
            -(c1[2] * c1[2] - c2[2] * c2[2]),
        ]);
    }
    let a2 = DMatrix::from_fn(rows2.len(), 2, |i, j| rows2[i][j]);
    let b2 = DVector::from_fn(rows2.len(), |i, _| rows2[i][2]);
    if let Ok(sol) = a2.svd(true, true).solve(&b2, 1e-12) {
        let (u, v) = (sol[0], sol[1]);
        if u > 0.0 && v > 0.0 {
            return Ok(back((1.0 / u).sqrt(), (1.0 / v).sqrt(), 0.0, 0.0));
        }
    }
    Err(tr!("camera.intrinsics.focal_unknown"))
}

pub(super) fn pose_from_homography(hm: &Homography, k_inv: &Matrix3<f64>) -> [f64; 6] {
    let col = |c: usize| Vector3::new(hm.m[0][c], hm.m[1][c], hm.m[2][c]);
    let (a1, a2, a3) = (k_inv * col(0), k_inv * col(1), k_inv * col(2));
    let lambda = 2.0 / (a1.norm() + a2.norm());
    let (mut r1, mut r2, mut t) = (a1 * lambda, a2 * lambda, a3 * lambda);
    if t.z < 0.0 {
        r1 = -r1;
        r2 = -r2;
        t = -t;
    }
    let r3 = r1.cross(&r2);
    let rough = Matrix3::from_columns(&[r1, r2, r3]);
    let svd = rough.svd(true, true);
    let r = match (svd.u, svd.v_t) {
        (Some(u), Some(vt)) => {
            let mut r = u * vt;
            if r.determinant() < 0.0 {
                let mut u2 = u;
                u2.column_mut(2).neg_mut();
                r = u2 * vt;
            }
            r
        }
        _ => rough,
    };
    let rv = Rotation3::from_matrix_unchecked(r).scaled_axis();
    [rv.x, rv.y, rv.z, t.x, t.y, t.z]
}

/// Residuos (u - u_obs, v - v_obs) de una vista; 2 por esquina.
pub(super) fn residuals_view(
    model: LensModel,
    intr: &[f64; 10],
    pose: &[f64; 6],
    obj: &[[f64; 2]],
    obs: &[Point],
    out: &mut [f64],
) {
    let rot = Rotation3::from_scaled_axis(Vector3::new(pose[0], pose[1], pose[2]));
    let m = rot.matrix();
    let (r1, r2) = (m.column(0), m.column(1));
    let (fx, fy, cx, cy) = (intr[0], intr[1], intr[2], intr[3]);
    let k = [intr[4], intr[5], intr[8], intr[9]];
    let p = [intr[6], intr[7]];
    for (i, (o, ob)) in obj.iter().zip(obs).enumerate() {
        let px = o[0] * r1[0] + o[1] * r2[0] + pose[3];
        let py = o[0] * r1[1] + o[1] * r2[1] + pose[4];
        let pz = o[0] * r1[2] + o[1] * r2[2] + pose[5];
        if pz < 1e-9 {
            out[2 * i] = 1e4;
            out[2 * i + 1] = 1e4;
            continue;
        }
        let (x, y) = (px / pz, py / pz);
        let (xd, yd) = distort_with(model, k, p, x, y);
        out[2 * i] = fx * xd + cx - ob.x;
        out[2 * i + 1] = fy * yd + cy - ob.y;
    }
}
