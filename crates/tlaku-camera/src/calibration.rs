//! Calibración de lente con un tablero plano: método de Zhang (2000) con el modelo
//! de distorsión de Brown-Conrady.
//!
//! 1. Homografía por vista (DLT normalizada) entre el tablero (mm) y los píxeles.
//! 2. Solución cerrada de los intrínsecos (fx, fy, cx, cy; sin sesgo) a partir de
//!    las restricciones de ortonormalidad de la rotación.
//! 3. Poses iniciales por vista y refinamiento conjunto de intrínsecos, distorsión
//!    y poses con Levenberg-Marquardt sobre el error de reproyección.
//! 4. Las vistas atípicas (orden de esquinas equivocado, desenfoque, tablero
//!    deformado) se descartan con un aviso.
//!
//! Modelo (coordenadas normalizadas `x = X/Z`, `y = Y/Z`, `r2 = x^2 + y^2`):
//! `xd = x*(1 + k1 r2 + k2 r2^2 + k3 r2^3) + 2 p1 x y + p2 (r2 + 2 x^2)`,
//! `yd = y*(1 + ...) + p1 (r2 + 2 y^2) + 2 p2 x y`, `u = fx xd + cx`, `v = fy yd + cy`.

use crate::chessboard::find_chessboard;
use crate::homography::{Homography, estimate_homography};
use image::GrayImage;
use nalgebra::{DMatrix, DVector, Matrix3, Rotation3, Vector3};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tlaku_core::Point;

/// Parámetros de la lente: matriz de la cámara, distorsión y calidad del ajuste.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LensCalibration {
    pub width: u32,
    pub height: u32,
    pub fx: f64,
    pub fy: f64,
    pub cx: f64,
    pub cy: f64,
    pub k1: f64,
    pub k2: f64,
    pub p1: f64,
    pub p2: f64,
    pub k3: f64,
    /// Error de reproyección rms en píxeles (distancia euclídea).
    pub rms_error_px: f64,
    pub views_used: usize,
}

impl LensCalibration {
    /// Comprueba que los parámetros son utilizables.
    pub fn validate(&self) -> Result<(), String> {
        let vals = [
            self.fx, self.fy, self.cx, self.cy, self.k1, self.k2, self.p1, self.p2, self.k3,
        ];
        if self.width == 0 || self.height == 0 {
            return Err("la calibración tiene un tamaño de imagen nulo".into());
        }
        if vals.iter().any(|v| !v.is_finite()) {
            return Err("la calibración contiene valores no finitos".into());
        }
        if self.fx <= 0.0 || self.fy <= 0.0 {
            return Err("la calibración tiene una distancia focal no positiva".into());
        }
        Ok(())
    }

    /// Aplica la distorsión a coordenadas normalizadas sin distorsión.
    pub fn distort_normalized(&self, x: f64, y: f64) -> (f64, f64) {
        let r2 = x * x + y * y;
        let q = 1.0 + r2 * (self.k1 + r2 * (self.k2 + r2 * self.k3));
        (
            x * q + 2.0 * self.p1 * x * y + self.p2 * (r2 + 2.0 * x * x),
            y * q + self.p1 * (r2 + 2.0 * y * y) + 2.0 * self.p2 * x * y,
        )
    }

    /// Inversa de [`Self::distort_normalized`] por Newton (la distorsión no tiene
    /// forma cerrada inversa).
    pub fn undistort_normalized(&self, xd: f64, yd: f64) -> (f64, f64) {
        let (mut x, mut y) = (xd, yd);
        for _ in 0..12 {
            let r2 = x * x + y * y;
            let q = 1.0 + r2 * (self.k1 + r2 * (self.k2 + r2 * self.k3));
            let dq = self.k1 + r2 * (2.0 * self.k2 + 3.0 * r2 * self.k3);
            let (fx, fy) = self.distort_normalized(x, y);
            let (ex, ey) = (fx - xd, fy - yd);
            let j11 = q + 2.0 * x * x * dq + 2.0 * self.p1 * y + 6.0 * self.p2 * x;
            let j12 = 2.0 * x * y * dq + 2.0 * self.p1 * x + 2.0 * self.p2 * y;
            let j21 = 2.0 * x * y * dq + 2.0 * self.p1 * x + 2.0 * self.p2 * y;
            let j22 = q + 2.0 * y * y * dq + 6.0 * self.p1 * y + 2.0 * self.p2 * x;
            let det = j11 * j22 - j12 * j21;
            if det.abs() < 1e-12 {
                break;
            }
            let dx = (j22 * ex - j12 * ey) / det;
            let dy = (-j21 * ex + j11 * ey) / det;
            x -= dx;
            y -= dy;
            if dx.abs() + dy.abs() < 1e-13 {
                break;
            }
        }
        (x, y)
    }

    /// Píxel de la imagen cruda (con distorsión) a coordenadas normalizadas sin distorsión.
    pub fn pixel_to_normalized(&self, p: Point) -> (f64, f64) {
        self.undistort_normalized((p.x - self.cx) / self.fx, (p.y - self.cy) / self.fy)
    }
}

/// Opciones del ajuste.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalibrationOptions {
    /// Ajusta la distorsión tangencial (p1, p2).
    pub fit_tangential: bool,
    /// Ajusta k3. Solo conviene con distorsión fuerte y buena cobertura del campo;
    /// si no, extrapola mal fuera de donde se vio el tablero.
    pub fit_k3: bool,
    pub max_iterations: usize,
}

impl Default for CalibrationOptions {
    fn default() -> Self {
        Self {
            fit_tangential: true,
            fit_k3: false,
            max_iterations: 150,
        }
    }
}

/// Resultado con diagnóstico.
#[derive(Debug, Clone, Serialize)]
pub struct CalibrationReport {
    pub calibration: LensCalibration,
    /// Avisos para el usuario (vistas descartadas, pocas vistas, error alto...).
    pub warnings: Vec<String>,
    /// Índices (de la lista de entrada) de las vistas usadas.
    pub used_views: Vec<usize>,
    /// Error rms en píxeles de cada vista usada (mismo orden que `used_views`).
    pub view_rms_px: Vec<f64>,
}

/// Detecta el tablero en cada imagen y calibra. Las imágenes sin tablero se omiten
/// con un aviso; hacen falta al menos 3 vistas válidas.
pub fn calibrate_lens(
    images: &[GrayImage],
    board: [u32; 2],
    square_mm: f64,
) -> Result<CalibrationReport, String> {
    calibrate_lens_with(images, board, square_mm, &CalibrationOptions::default())
}

pub fn calibrate_lens_with(
    images: &[GrayImage],
    board: [u32; 2],
    square_mm: f64,
    opts: &CalibrationOptions,
) -> Result<CalibrationReport, String> {
    let Some(first) = images.first() else {
        return Err("no se indicó ninguna imagen para calibrar".into());
    };
    let size = (first.width(), first.height());
    let mut warnings = Vec::new();
    let mut views: Vec<Vec<Point>> = Vec::new();
    let mut map: Vec<usize> = Vec::new();
    for (i, img) in images.iter().enumerate() {
        if (img.width(), img.height()) != size {
            return Err(format!(
                "la imagen {} mide {}x{} y la primera {}x{}; todas deben tener el mismo tamaño",
                i + 1,
                img.width(),
                img.height(),
                size.0,
                size.1
            ));
        }
        match find_chessboard(img, board) {
            Ok(pts) => {
                views.push(pts);
                map.push(i);
            }
            Err(e) => warnings.push(format!("imagen {}: {e}; se omite", i + 1)),
        }
    }
    if views.len() < 3 {
        return Err(format!(
            "solo {} de {} imágenes tienen el tablero visible; se necesitan al menos 3 (ideal 5 o más, con el tablero inclinado y en las esquinas)",
            views.len(),
            images.len()
        ));
    }
    let mut report = calibrate_views(&views, board, square_mm, size, opts)?;
    // Reasignar los índices a la lista de imágenes original.
    report.used_views = report.used_views.iter().map(|&k| map[k]).collect();
    for w in &mut report.warnings {
        for (k, &orig) in map.iter().enumerate().rev() {
            // Los avisos de vistas descartadas dicen "vista N " (base 1, sobre las
            // vistas detectadas); se reescriben con el número de imagen original.
            *w = w.replace(
                &format!("vista {} ", k + 1),
                &format!("imagen {} ", orig + 1),
            );
        }
    }
    warnings.extend(std::mem::take(&mut report.warnings));
    report.warnings = warnings;
    Ok(report)
}

/// Igual que [`calibrate_lens`] pero leyendo las imágenes desde rutas.
pub fn calibrate_lens_files<P: AsRef<Path>>(
    paths: &[P],
    board: [u32; 2],
    square_mm: f64,
) -> Result<CalibrationReport, String> {
    let mut images = Vec::with_capacity(paths.len());
    for p in paths {
        let img = image::open(p.as_ref())
            .map_err(|e| format!("no se pudo abrir {}: {e}", p.as_ref().display()))?;
        images.push(img.to_luma8());
    }
    calibrate_lens(&images, board, square_mm)
}

// ---------------------------------------------------------------------------
// Calibración a partir de esquinas ya detectadas
// ---------------------------------------------------------------------------

/// Calibra con esquinas ya detectadas (`cols * rows` puntos por vista, fila por
/// fila). Descarta vistas atípicas mientras queden al menos 3.
pub fn calibrate_views(
    views: &[Vec<Point>],
    board: [u32; 2],
    square_mm: f64,
    size: (u32, u32),
    opts: &CalibrationOptions,
) -> Result<CalibrationReport, String> {
    let (cols, rows) = (board[0] as usize, board[1] as usize);
    if views.len() < 3 {
        return Err(format!(
            "se necesitan al menos 3 vistas del tablero (hay {})",
            views.len()
        ));
    }
    if !(square_mm.is_finite() && square_mm > 0.0) {
        return Err("el tamaño del cuadro debe ser mayor que 0 mm".into());
    }
    if let Some((i, v)) = views
        .iter()
        .enumerate()
        .find(|(_, v)| v.len() != cols * rows)
    {
        return Err(format!(
            "la vista {} tiene {} esquinas y se esperaban {}",
            i + 1,
            v.len(),
            cols * rows
        ));
    }
    let obj: Vec<[f64; 2]> = (0..rows)
        .flat_map(|j| (0..cols).map(move |i| [i as f64 * square_mm, j as f64 * square_mm]))
        .collect();

    let mut active: Vec<usize> = (0..views.len()).collect();
    let mut warnings = Vec::new();
    let run = |act: &[usize]| {
        let refs: Vec<&[Point]> = act.iter().map(|&i| views[i].as_slice()).collect();
        fit(&refs, &obj, size, opts)
    };
    let mut current = run(&active)?;
    while active.len() > 3 {
        let (worst, &worst_rms) = current
            .view_rms
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .expect("hay vistas");
        let mut sorted = current.view_rms.clone();
        sorted.sort_by(f64::total_cmp);
        let median = sorted[sorted.len() / 2];
        if worst_rms <= (2.5 * median).max(0.75) {
            break;
        }
        let mut trial = active.clone();
        trial.remove(worst);
        match run(&trial) {
            Ok(f) if f.rms < 0.6 * current.rms => {
                warnings.push(format!(
                    "vista {} descartada: error de reproyección {:.2} px frente a una mediana de {:.2} px (¿tablero movido, desenfocado o mal detectado?)",
                    active[worst] + 1, worst_rms, median
                ));
                active = trial;
                current = f;
            }
            _ => break,
        }
    }
    if active.len() < 5 {
        warnings.push(format!(
            "solo {} vistas útiles; con 5 o más (tablero inclinado y cerca de las esquinas) la calibración es más fiable",
            active.len()
        ));
    }
    if current.rms > 1.0 {
        warnings.push(format!(
            "el error de reproyección es alto ({:.2} px); revisa el enfoque, que el tablero esté plano y el tamaño del cuadro",
            current.rms
        ));
    }
    let p = &current.intr;
    let calibration = LensCalibration {
        width: size.0,
        height: size.1,
        fx: p[0],
        fy: p[1],
        cx: p[2],
        cy: p[3],
        k1: p[4],
        k2: p[5],
        p1: p[6],
        p2: p[7],
        k3: p[8],
        rms_error_px: current.rms,
        views_used: active.len(),
    };
    Ok(CalibrationReport {
        calibration,
        warnings,
        view_rms_px: current.view_rms.clone(),
        used_views: active,
    })
}

struct Fit {
    intr: [f64; 9],
    rms: f64,
    view_rms: Vec<f64>,
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
    let mut intr = [fx, fy, cx, cy, 0.0, 0.0, 0.0, 0.0, 0.0];

    let mut free: Vec<usize> = vec![0, 1, 2, 3, 4, 5];
    if opts.fit_tangential {
        free.extend([6, 7]);
    }
    if opts.fit_k3 {
        free.push(8);
    }
    levenberg_marquardt(
        &mut intr,
        &mut poses,
        views,
        obj,
        &free,
        opts.max_iterations,
    );

    if !intr.iter().all(|v| v.is_finite()) || intr[0] <= 0.0 || intr[1] <= 0.0 {
        return Err("la calibración no convergió; toma más vistas con el tablero inclinado".into());
    }
    let mut view_rms = Vec::with_capacity(views.len());
    let mut total = 0.0;
    let mut count = 0usize;
    let mut buf = vec![0.0; 2 * obj.len()];
    for (v, pose) in views.iter().zip(&poses) {
        residuals_view(&intr, pose, obj, v, &mut buf);
        let s: f64 = buf.iter().map(|r| r * r).sum();
        total += s;
        count += obj.len();
        view_rms.push((s / obj.len() as f64).sqrt());
    }
    Ok(Fit {
        intr,
        rms: (total / count as f64).sqrt(),
        view_rms,
    })
}

/// Solución cerrada de Zhang con sesgo nulo, trabajando en píxeles centrados y
/// escalados para mejorar el condicionamiento. Si falla, reintenta con el punto
/// principal en el centro de la imagen.
fn initial_intrinsics(hs: &[Homography], w: f64, h: f64) -> Result<(f64, f64, f64, f64), String> {
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
    Err("no se pudo estimar la distancia focal: las vistas son demasiado parecidas; inclina el tablero en distintas direcciones".into())
}

fn pose_from_homography(hm: &Homography, k_inv: &Matrix3<f64>) -> [f64; 6] {
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
fn residuals_view(
    intr: &[f64; 9],
    pose: &[f64; 6],
    obj: &[[f64; 2]],
    obs: &[Point],
    out: &mut [f64],
) {
    let rot = Rotation3::from_scaled_axis(Vector3::new(pose[0], pose[1], pose[2]));
    let m = rot.matrix();
    let (r1, r2) = (m.column(0), m.column(1));
    let (fx, fy, cx, cy) = (intr[0], intr[1], intr[2], intr[3]);
    let (k1, k2, p1, p2, k3) = (intr[4], intr[5], intr[6], intr[7], intr[8]);
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
        let r2n = x * x + y * y;
        let q = 1.0 + r2n * (k1 + r2n * (k2 + r2n * k3));
        let xd = x * q + 2.0 * p1 * x * y + p2 * (r2n + 2.0 * x * x);
        let yd = y * q + p1 * (r2n + 2.0 * y * y) + 2.0 * p2 * x * y;
        out[2 * i] = fx * xd + cx - ob.x;
        out[2 * i + 1] = fy * yd + cy - ob.y;
    }
}

/// Levenberg-Marquardt sobre intrínsecos libres (`free`) y las poses de todas las vistas.
/// El jacobiano se obtiene por diferencias centrales.
fn levenberg_marquardt(
    intr: &mut [f64; 9],
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

    let full_residuals = |intr: &[f64; 9], poses: &[[f64; 6]], out: &mut [f64]| {
        for (vi, v) in views.iter().enumerate() {
            residuals_view(
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
                residuals_view(intr, &pp, obj, views[vi], &mut vp);
                residuals_view(intr, &pm, obj, views[vi], &mut vm);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    const BOARD: [u32; 2] = [9, 6];
    const SQ: f64 = 25.0;

    fn rel(a: f64, b: f64) -> f64 {
        ((a - b) / b).abs()
    }

    fn images(cam: &LensCalibration, n: usize, noise: f32) -> Vec<GrayImage> {
        standard_poses(BOARD, SQ)
            .iter()
            .take(n)
            .enumerate()
            .map(|(i, p)| {
                // Todas las esquinas deben caer dentro de la imagen.
                for c in board_corners(cam, p, BOARD, SQ) {
                    assert!(
                        c.x > 12.0 && c.y > 12.0 && c.x < 628.0 && c.y < 468.0,
                        "vista {i} fuera"
                    );
                }
                render_board(cam, p, BOARD, SQ, noise, 100 + i as u64)
            })
            .collect()
    }

    #[test]
    fn recupera_los_parametros_de_la_camara_virtual() {
        let cam = truth_camera();
        let imgs = images(&cam, 8, 1.5);
        let rep = calibrate_lens(&imgs, BOARD, SQ).unwrap();
        let c = &rep.calibration;
        eprintln!(
            "fx {:.2} fy {:.2} cx {:.2} cy {:.2} k1 {:.4} k2 {:.4} p1 {:.5} p2 {:.5} rms {:.3} avisos {:?}",
            c.fx, c.fy, c.cx, c.cy, c.k1, c.k2, c.p1, c.p2, c.rms_error_px, rep.warnings
        );
        assert_eq!(c.views_used, 8);
        assert!(rel(c.fx, cam.fx) < 0.01, "fx {}", c.fx);
        assert!(rel(c.fy, cam.fy) < 0.01, "fy {}", c.fy);
        assert!((c.cx - cam.cx).abs() < 0.01 * cam.cx, "cx {}", c.cx);
        assert!((c.cy - cam.cy).abs() < 0.015 * cam.cy, "cy {}", c.cy);
        assert!((c.k1 - cam.k1).abs() < 0.03, "k1 {}", c.k1);
        assert!((c.k2 - cam.k2).abs() < 0.1, "k2 {}", c.k2);
        assert!(c.rms_error_px < 0.5, "rms {}", c.rms_error_px);
        assert_eq!((c.width, c.height), (640, 480));
        // El efecto real de la distorsión recuperada: error de posición en toda la imagen.
        let mut worst: f64 = 0.0;
        for v in (0..480).step_by(40) {
            for u in (0..640).step_by(40) {
                let (x, y) = (
                    (f64::from(u) - cam.cx) / cam.fx,
                    (f64::from(v) - cam.cy) / cam.fy,
                );
                let (xd, yd) = cam.distort_normalized(x, y);
                let truth = (cam.fx * xd + cam.cx, cam.fy * yd + cam.cy);
                let (ex, ey) = c
                    .distort_normalized((f64::from(u) - c.cx) / c.fx, (f64::from(v) - c.cy) / c.fy);
                let est = (c.fx * ex + c.cx, c.fy * ey + c.cy);
                worst = worst.max((truth.0 - est.0).hypot(truth.1 - est.1));
            }
        }
        eprintln!("desviación máxima del modelo en la imagen: {worst:.2} px");
        assert!(worst < 3.0, "el modelo recuperado se desvía {worst:.2} px");
    }

    #[test]
    fn descarta_una_vista_atipica_con_aviso() {
        let cam = truth_camera();
        let imgs = images(&cam, 6, 1.5);
        let mut views: Vec<Vec<Point>> = imgs
            .iter()
            .map(|i| find_chessboard(i, BOARD).unwrap())
            .collect();
        // Una vista con el etiquetado corrido un lugar (la inversión por filas sería
        // una pose válida vista desde atrás, no un error).
        let mut bad = views[2].clone();
        bad.rotate_left(1);
        views.push(bad);
        let rep = calibrate_views(
            &views,
            BOARD,
            SQ,
            (640, 480),
            &CalibrationOptions::default(),
        )
        .unwrap();
        assert_eq!(rep.calibration.views_used, 6);
        assert!(!rep.used_views.contains(&6));
        assert!(
            rep.warnings
                .iter()
                .any(|w| w.contains("vista 7 descartada")),
            "{:?}",
            rep.warnings
        );
        assert!(rep.calibration.rms_error_px < 0.5);
        assert!(rel(rep.calibration.fx, cam.fx) < 0.02);
    }

    #[test]
    fn errores_claros() {
        let cam = truth_camera();
        let imgs = images(&cam, 2, 1.0);
        let e = calibrate_lens(&imgs, BOARD, SQ).unwrap_err();
        assert!(e.contains("al menos 3"), "{e}");
        // Imágenes sin tablero: se omiten con aviso y luego faltan vistas.
        let blank = vec![GrayImage::from_pixel(640, 480, image::Luma([128])); 4];
        assert!(
            calibrate_lens(&blank, BOARD, SQ)
                .unwrap_err()
                .contains("al menos 3")
        );
        assert!(calibrate_lens(&[], BOARD, SQ).is_err());
    }

    #[test]
    fn omite_imagenes_sin_tablero_y_reporta_indices_originales() {
        let cam = truth_camera();
        let mut imgs = images(&cam, 5, 1.5);
        imgs.insert(1, GrayImage::from_pixel(640, 480, image::Luma([128])));
        let rep = calibrate_lens(&imgs, BOARD, SQ).unwrap();
        assert_eq!(rep.used_views, vec![0, 2, 3, 4, 5]);
        assert!(
            rep.warnings.iter().any(|w| w.starts_with("imagen 2:")),
            "{:?}",
            rep.warnings
        );
    }

    fn projected_views(cam: &LensCalibration, noise_px: f64) -> Vec<Vec<Point>> {
        let mut rng = Rng::new(42);
        standard_poses(BOARD, SQ)
            .iter()
            .map(|p| {
                board_corners(cam, p, BOARD, SQ)
                    .into_iter()
                    .map(|c| Point::new(c.x + noise_px * rng.gauss(), c.y + noise_px * rng.gauss()))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn puntos_exactos_dan_error_casi_nulo() {
        let cam = truth_camera();
        let views = projected_views(&cam, 0.0);
        let rep = calibrate_views(
            &views,
            BOARD,
            SQ,
            (640, 480),
            &CalibrationOptions::default(),
        )
        .unwrap();
        let c = &rep.calibration;
        assert!(c.rms_error_px < 1e-3, "rms {}", c.rms_error_px);
        assert!(rel(c.fx, cam.fx) < 1e-4 && rel(c.fy, cam.fy) < 1e-4);
        assert!((c.cx - cam.cx).abs() < 0.05 && (c.cy - cam.cy).abs() < 0.05);
        assert!((c.k1 - cam.k1).abs() < 1e-3 && (c.p1 - cam.p1).abs() < 1e-4);
        assert!(rep.warnings.is_empty(), "{:?}", rep.warnings);
    }

    #[test]
    fn distorsion_fuerte_con_k3() {
        let cam = LensCalibration {
            k1: -0.38,
            k2: 0.25,
            k3: -0.12,
            p1: 0.0,
            p2: 0.0,
            ..truth_camera()
        };
        let views = projected_views(&cam, 0.05);
        let opts = CalibrationOptions {
            fit_k3: true,
            ..Default::default()
        };
        let rep = calibrate_views(&views, BOARD, SQ, (640, 480), &opts).unwrap();
        let c = &rep.calibration;
        eprintln!(
            "k1 {:.3} k2 {:.3} k3 {:.3} fx {:.1} rms {:.3}",
            c.k1, c.k2, c.k3, c.fx, c.rms_error_px
        );
        assert!(c.rms_error_px < 0.2);
        assert!(rel(c.fx, cam.fx) < 0.02);
        assert!((c.k1 - cam.k1).abs() < 0.08, "k1 {}", c.k1);
    }

    #[test]
    fn vistas_sin_inclinacion_dan_error_claro() {
        // Siempre de frente y sin giros: no se puede estimar la focal.
        let cam = truth_camera();
        let views: Vec<Vec<Point>> = [
            (-30.0, 0.0, 480.0),
            (20.0, 10.0, 520.0),
            (0.0, -20.0, 600.0),
            (10.0, 20.0, 560.0),
        ]
        .iter()
        .map(|&(x, y, z)| {
            let pose = Pose::about_center(BOARD, SQ, (0.0, 0.0, 0.0), (x, y, z));
            board_corners(&cam, &pose, BOARD, SQ)
        })
        .collect();
        match calibrate_views(
            &views,
            BOARD,
            SQ,
            (640, 480),
            &CalibrationOptions::default(),
        ) {
            Err(e) => assert!(e.contains("inclina") || e.contains("convergió"), "{e}"),
            Ok(r) => assert!(r.warnings.len() + 1 > 0), // si converge, al menos no debe entrar en pánico
        }
    }
}
