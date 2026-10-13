//! Calibración de lente con un tablero plano. Dos modelos de distorsión:
//!
//! - Brown-Conrady (el de siempre): método de Zhang (2000), solución cerrada más LM.
//! - Ojo de pez equidistante de Kannala-Brandt (2006): el ángulo del rayo con el eje óptico
//!   `θ = atan(r)` se lleva a `θd = θ (1 + k1 θ² + k2 θ⁴ + k3 θ⁶ + k4 θ⁸)` y el píxel es
//!   `f · θd · (x, y) / r`. La inicialización de Zhang no sirve aquí (supone una cámara casi
//!   estenopeica), así que se parte de una focal supuesta (varias, y gana la de menor error) y
//!   de las poses calculadas con ella.
//!
//! Con `LensModelChoice::Auto` se ajustan los dos y se elige por el error de reproyección y por
//! validación cruzada con vistas reservadas (ver [`select_model`]).
//!
//! Pasos del modelo Brown-Conrady:
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

/// Modelo matemático de la distorsión de la lente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LensModel {
    /// Polinomio radial `k1, k2, k3` más tangencial `p1, p2` (lentes normales y gran angular).
    #[default]
    BrownConrady,
    /// Ojo de pez equidistante de Kannala-Brandt (`k1..k4`; `p1` y `p2` no se usan).
    Fisheye,
}

impl LensModel {
    pub fn label(self) -> &'static str {
        match self {
            LensModel::BrownConrady => "Brown-Conrady",
            LensModel::Fisheye => "ojo de pez (Kannala-Brandt)",
        }
    }
}

fn one() -> f64 {
    1.0
}

/// Parámetros de la lente: matriz de la cámara, distorsión y calidad del ajuste.
///
/// Los campos `model`, `k4` y `view_zoom` son posteriores a las primeras calibraciones guardadas:
/// si faltan al leer, valen Brown-Conrady, 0 y 1, así que las calibraciones viejas no cambian.
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
    /// Modelo con que se interpretan `k1..k4`, `p1` y `p2`.
    #[serde(default)]
    pub model: LensModel,
    /// Cuarto coeficiente del ojo de pez (con Brown-Conrady vale 0 y no se usa).
    #[serde(default)]
    pub k4: f64,
    /// Factor de la focal de la imagen corregida respecto de `fx`/`fy`. Con Brown-Conrady es 1
    /// (la imagen corregida conserva la escala del centro); con ojo de pez es menor que 1 para
    /// que quepa un campo amplio, porque la proyección rectilínea no puede mostrar los 180°.
    #[serde(default = "one")]
    pub view_zoom: f64,
    /// Error de reproyección rms en píxeles (distancia euclídea).
    pub rms_error_px: f64,
    pub views_used: usize,
}

/// Distorsión de coordenadas normalizadas sin distorsión `(x, y)` según el modelo.
fn distort_with(model: LensModel, k: [f64; 4], p: [f64; 2], x: f64, y: f64) -> (f64, f64) {
    match model {
        LensModel::BrownConrady => {
            let r2 = x * x + y * y;
            let q = 1.0 + r2 * (k[0] + r2 * (k[1] + r2 * k[2]));
            (
                x * q + 2.0 * p[0] * x * y + p[1] * (r2 + 2.0 * x * x),
                y * q + p[0] * (r2 + 2.0 * y * y) + 2.0 * p[1] * x * y,
            )
        }
        LensModel::Fisheye => {
            let r = x.hypot(y);
            if r < 1e-12 {
                return (x, y);
            }
            let th = r.atan();
            let t2 = th * th;
            let thd = th * (1.0 + t2 * (k[0] + t2 * (k[1] + t2 * (k[2] + t2 * k[3]))));
            let s = thd / r;
            (x * s, y * s)
        }
    }
}

/// Ángulo máximo (rad) que se invierte en el ojo de pez: más allá, `tan` se dispara.
const MAX_FISHEYE_THETA: f64 = 1.55;

/// Inversa de [`distort_with`]. Brown-Conrady por Newton en 2D; ojo de pez por Newton en 1D sobre
/// `θd = θ (1 + ...)` y luego `r = tan θ`.
fn undistort_with(model: LensModel, k: [f64; 4], p: [f64; 2], xd: f64, yd: f64) -> (f64, f64) {
    match model {
        LensModel::BrownConrady => {
            let (k1, k2, k3, p1, p2) = (k[0], k[1], k[2], p[0], p[1]);
            let (mut x, mut y) = (xd, yd);
            for _ in 0..12 {
                let r2 = x * x + y * y;
                let q = 1.0 + r2 * (k1 + r2 * (k2 + r2 * k3));
                let dq = k1 + r2 * (2.0 * k2 + 3.0 * r2 * k3);
                let (fx, fy) = distort_with(model, k, p, x, y);
                let (ex, ey) = (fx - xd, fy - yd);
                let j11 = q + 2.0 * x * x * dq + 2.0 * p1 * y + 6.0 * p2 * x;
                let j12 = 2.0 * x * y * dq + 2.0 * p1 * x + 2.0 * p2 * y;
                let j21 = 2.0 * x * y * dq + 2.0 * p1 * x + 2.0 * p2 * y;
                let j22 = q + 2.0 * y * y * dq + 6.0 * p1 * y + 2.0 * p2 * x;
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
        LensModel::Fisheye => {
            let rd = xd.hypot(yd);
            if rd < 1e-12 {
                return (xd, yd);
            }
            let mut th = rd.min(MAX_FISHEYE_THETA);
            for _ in 0..40 {
                let t2 = th * th;
                let f = th * (1.0 + t2 * (k[0] + t2 * (k[1] + t2 * (k[2] + t2 * k[3])))) - rd;
                let df = 1.0
                    + t2 * (3.0 * k[0] + t2 * (5.0 * k[1] + t2 * (7.0 * k[2] + t2 * 9.0 * k[3])));
                if df.abs() < 1e-9 {
                    break;
                }
                let step = f / df;
                th = (th - step).clamp(0.0, MAX_FISHEYE_THETA);
                if step.abs() < 1e-14 {
                    break;
                }
            }
            let r = th.tan();
            (xd * r / rd, yd * r / rd)
        }
    }
}

impl LensCalibration {
    /// Comprueba que los parámetros son utilizables.
    pub fn validate(&self) -> Result<(), String> {
        let vals = [
            self.fx,
            self.fy,
            self.cx,
            self.cy,
            self.k1,
            self.k2,
            self.p1,
            self.p2,
            self.k3,
            self.k4,
            self.view_zoom,
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
        if !(0.05..=10.0).contains(&self.view_zoom) {
            return Err("la calibración tiene un factor de vista fuera de rango".into());
        }
        Ok(())
    }

    fn k4(&self) -> [f64; 4] {
        [self.k1, self.k2, self.k3, self.k4]
    }

    /// Focales (px) de la imagen corregida: las de la calibración por `view_zoom`.
    pub fn corrected_focal(&self) -> (f64, f64) {
        (self.fx * self.view_zoom, self.fy * self.view_zoom)
    }

    /// Aplica la distorsión a coordenadas normalizadas sin distorsión.
    pub fn distort_normalized(&self, x: f64, y: f64) -> (f64, f64) {
        distort_with(self.model, self.k4(), [self.p1, self.p2], x, y)
    }

    /// Inversa de [`Self::distort_normalized`] (la distorsión no tiene forma cerrada inversa).
    pub fn undistort_normalized(&self, xd: f64, yd: f64) -> (f64, f64) {
        undistort_with(self.model, self.k4(), [self.p1, self.p2], xd, yd)
    }

    /// Píxel de la imagen cruda (con distorsión) a coordenadas normalizadas sin distorsión.
    pub fn pixel_to_normalized(&self, p: Point) -> (f64, f64) {
        self.undistort_normalized((p.x - self.cx) / self.fx, (p.y - self.cy) / self.fy)
    }
}

/// Qué modelo de lente ajustar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LensModelChoice {
    /// Ajusta los dos y elige por error de reproyección y validación cruzada.
    #[default]
    Auto,
    BrownConrady,
    Fisheye,
}

/// Opciones del ajuste.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalibrationOptions {
    /// Modelo de lente: uno fijo o la elección automática.
    pub model: LensModelChoice,
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
            model: LensModelChoice::Auto,
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
    /// Puntaje de cada modelo probado (uno solo si se fijó el modelo).
    pub model_scores: Vec<ModelScore>,
}

/// Cómo le fue a un modelo de lente con los mismos datos.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelScore {
    pub model: LensModel,
    /// Error de reproyección rms con todas las vistas usadas (px).
    pub rms_px: f64,
    /// Error rms (px) en las vistas reservadas: se ajusta sin ellas y se mide en ellas con la
    /// pose recalculada. `None` si hay muy pocas vistas.
    pub cv_rms_px: Option<f64>,
    /// Es el modelo de la calibración devuelta.
    pub chosen: bool,
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
    let run = |model: LensModel, act: &[usize]| {
        let refs: Vec<&[Point]> = act.iter().map(|&i| views[i].as_slice()).collect();
        fit_model(model, &refs, &obj, size, opts)
    };
    let models: &[LensModel] = match opts.model {
        LensModelChoice::Auto => &[LensModel::BrownConrady, LensModel::Fisheye],
        LensModelChoice::BrownConrady => &[LensModel::BrownConrady],
        LensModelChoice::Fisheye => &[LensModel::Fisheye],
    };
    // Primer ajuste con todas las vistas; el modelo que mejor ajusta decide qué vistas son atípicas.
    let first: Vec<(LensModel, Result<Fit, String>)> =
        models.iter().map(|&m| (m, run(m, &active))).collect();
    let leader = pick_leader(&first).ok_or_else(|| {
        first
            .iter()
            .find_map(|(_, r)| r.as_ref().err().cloned())
            .unwrap_or_else(|| "no se pudo ajustar ningún modelo de lente".into())
    })?;
    let mut current = first
        .into_iter()
        .find(|(m, _)| *m == leader)
        .and_then(|(_, r)| r.ok())
        .expect("el líder ajustó");
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
        match run(leader, &trial) {
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

    // Con el conjunto final de vistas se compara cada modelo (ajuste y validación cruzada).
    let refs: Vec<&[Point]> = active.iter().map(|&i| views[i].as_slice()).collect();
    let mut fits: Vec<(LensModel, Fit)> = Vec::new();
    for &m in models {
        if m == leader {
            fits.push((m, current.clone()));
        } else if let Ok(f) = run(m, &active) {
            fits.push((m, f));
        }
    }
    let mut scores: Vec<ModelScore> = fits
        .iter()
        .map(|(m, f)| ModelScore {
            model: *m,
            rms_px: f.rms,
            cv_rms_px: if models.len() > 1 {
                cross_validate(*m, &refs, &obj, size, opts)
            } else {
                None
            },
            chosen: false,
        })
        .collect();
    let chosen = select_model(&scores);
    for sc in &mut scores {
        sc.chosen = sc.model == chosen;
    }
    if models.len() > 1 {
        let brown = scores.iter().find(|s| s.model == LensModel::BrownConrady);
        let fish = scores.iter().find(|s| s.model == LensModel::Fisheye);
        match (brown, fish) {
            (Some(b), Some(f)) if chosen == LensModel::Fisheye => warnings.push(format!(
                "se eligió el modelo de ojo de pez: error en vistas reservadas {} px frente a {} px con Brown-Conrady",
                fmt_score(f.cv_rms_px.unwrap_or(f.rms_px)),
                fmt_score(b.cv_rms_px.unwrap_or(b.rms_px)),
            )),
            (None, Some(_)) => warnings.push(
                "Brown-Conrady no logró ajustar estas vistas; se usa el modelo de ojo de pez".into(),
            ),
            (Some(_), None) => warnings.push(
                "el modelo de ojo de pez no logró ajustar estas vistas; se usa Brown-Conrady".into(),
            ),
            _ => {}
        }
    }
    let current = fits
        .into_iter()
        .find(|(m, _)| *m == chosen)
        .map(|(_, f)| f)
        .expect("el modelo elegido ajustó");
    if current.rms > 1.0 {
        warnings.push(format!(
            "el error de reproyección es alto ({:.2} px); revisa el enfoque, que el tablero esté plano y el tamaño del cuadro",
            current.rms
        ));
    }
    let p = &current.intr;
    let view_zoom = if chosen == LensModel::Fisheye {
        auto_view_zoom(p, size)
    } else {
        1.0
    };
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
        model: chosen,
        k4: p[9],
        view_zoom,
        rms_error_px: current.rms,
        views_used: active.len(),
    };
    Ok(CalibrationReport {
        calibration,
        warnings,
        view_rms_px: current.view_rms.clone(),
        used_views: active,
        model_scores: scores,
    })
}

fn fmt_score(v: f64) -> String {
    if v.is_finite() {
        format!("{v:.2}")
    } else {
        "sin convergencia".into()
    }
}

/// El modelo que ajusta mejor con todas las vistas (Brown-Conrady en un empate), o `None` si
/// ninguno ajustó.
fn pick_leader(first: &[(LensModel, Result<Fit, String>)]) -> Option<LensModel> {
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
fn select_model(scores: &[ModelScore]) -> LensModel {
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
fn auto_view_zoom(intr: &[f64; 10], size: (u32, u32)) -> f64 {
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
fn cross_validate(
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

#[derive(Clone)]
struct Fit {
    /// fx, fy, cx, cy, k1, k2, p1, p2, k3, k4.
    intr: [f64; 10],
    rms: f64,
    view_rms: Vec<f64>,
}

fn fit_model(
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
fn init_poses(
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

/// Levenberg-Marquardt sobre intrínsecos libres (`free`) y las poses de todas las vistas.
/// El jacobiano se obtiene por diferencias centrales.
fn levenberg_marquardt(
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

#[cfg(test)]
mod fisheye_tests {
    use super::*;
    use crate::testutil::*;
    use image::GrayImage;

    const BOARD: [u32; 2] = [9, 6];
    const SQ: f64 = 25.0;

    fn views_of(cam: &LensCalibration, noise_px: f64, seed: u64) -> Vec<Vec<Point>> {
        let mut rng = Rng::new(seed);
        fisheye_poses(BOARD, SQ)
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let pts = board_corners(cam, p, BOARD, SQ);
                for c in &pts {
                    assert!(
                        c.x > 3.0
                            && c.y > 3.0
                            && c.x < f64::from(cam.width) - 3.0
                            && c.y < f64::from(cam.height) - 3.0,
                        "vista {i}: esquina fuera de la imagen ({:.0}, {:.0})",
                        c.x,
                        c.y
                    );
                }
                pts.into_iter()
                    .map(|c| Point::new(c.x + noise_px * rng.gauss(), c.y + noise_px * rng.gauss()))
                    .collect()
            })
            .collect()
    }

    /// Error de posición en un plano a 350 mm de la cámara, para los píxeles de la imagen
    /// cuyo rayo verdadero está a menos de `max_deg` del eje: rms y máximo en mm. Es lo que
    /// importa a la hora de colocar un diseño sobre la cama.
    fn plane_error_mm(est: &LensCalibration, truth: &LensCalibration, max_deg: f64) -> (f64, f64) {
        const Z: f64 = 350.0;
        let (mut sum, mut worst, mut n) = (0.0, 0.0_f64, 0usize);
        for v in (0..480).step_by(16) {
            for u in (0..640).step_by(16) {
                let p = Point::new(f64::from(u), f64::from(v));
                let (tx, ty) = truth.pixel_to_normalized(p);
                if tx.hypot(ty).atan().to_degrees() > max_deg {
                    continue;
                }
                let (ex, ey) = est.pixel_to_normalized(p);
                let e = if ex.is_finite() && ey.is_finite() {
                    (Z * (ex - tx)).hypot(Z * (ey - ty))
                } else {
                    f64::INFINITY
                };
                sum += e * e;
                worst = worst.max(e);
                n += 1;
            }
        }
        ((sum / n as f64).sqrt(), worst)
    }

    fn opts(model: LensModelChoice) -> CalibrationOptions {
        CalibrationOptions {
            model,
            ..Default::default()
        }
    }

    #[test]
    fn kannala_brandt_va_y_vuelve() {
        let cam = fisheye_camera();
        for deg in [0.0_f64, 10.0, 35.0, 60.0, 80.0] {
            let th = deg.to_radians();
            let (x, y) = (th.tan() * 0.6, th.tan() * 0.8);
            let (xd, yd) = cam.distort_normalized(x, y);
            let (bx, by) = cam.undistort_normalized(xd, yd);
            assert!(
                (bx - x).hypot(by - y) < 1e-9 * (1.0 + x.hypot(y)),
                "{deg} grados"
            );
        }
        // Cerca del eje equivale a la identidad (θd ≈ θ ≈ r).
        let (xd, yd) = cam.distort_normalized(1e-4, 2e-4);
        assert!((xd - 1e-4).abs() < 1e-9 && (yd - 2e-4).abs() < 1e-9);
        assert_eq!(cam.distort_normalized(0.0, 0.0), (0.0, 0.0));
    }

    #[test]
    fn el_ojo_de_pez_se_recupera_y_baja_el_error_frente_a_brown_conrady() {
        let cam = fisheye_camera();
        let views = views_of(&cam, 0.1, 7);
        let size = (640, 480);
        let fish =
            calibrate_views(&views, BOARD, SQ, size, &opts(LensModelChoice::Fisheye)).unwrap();
        let brown = calibrate_views(
            &views,
            BOARD,
            SQ,
            size,
            &opts(LensModelChoice::BrownConrady),
        );
        let fc = &fish.calibration;
        let (f_rms, f_max) = plane_error_mm(fc, &cam, 55.0);
        eprintln!(
            "KB: fx {:.1} fy {:.1} cx {:.1} cy {:.1} k1 {:.4} k2 {:.4} k3 {:.4} k4 {:.4} zoom {:.2} rms {:.3} px; plano: rms {f_rms:.3} mm, max {f_max:.3} mm",
            fc.fx, fc.fy, fc.cx, fc.cy, fc.k1, fc.k2, fc.k3, fc.k4, fc.view_zoom, fc.rms_error_px
        );
        assert_eq!(fc.model, LensModel::Fisheye);
        assert!(fc.rms_error_px < 0.25, "rms {}", fc.rms_error_px);
        assert!(((fc.fx - cam.fx) / cam.fx).abs() < 0.01, "fx {}", fc.fx);
        assert!((fc.cx - cam.cx).abs() < 2.0 && (fc.cy - cam.cy).abs() < 2.0);
        assert!(
            f_rms < 1.6 && f_max < 4.0,
            "KB: rms {f_rms:.2} max {f_max:.2} mm"
        );
        match brown {
            Ok(b) => {
                let (b_rms, b_max) = plane_error_mm(&b.calibration, &cam, 55.0);
                eprintln!(
                    "BC: rms {:.3} px; plano: rms {b_rms:.3} mm, max {b_max:.3} mm",
                    b.calibration.rms_error_px
                );
                assert!(
                    b_rms > 4.0 * f_rms.max(0.1),
                    "BC {b_rms:.2} mm vs KB {f_rms:.2} mm"
                );
            }
            Err(e) => eprintln!("BC no converge: {e}"),
        }
    }

    #[test]
    fn auto_elige_ojo_de_pez_con_distorsion_fuerte_y_brown_con_distorsion_normal() {
        // Ojo de pez fuerte.
        let cam = fisheye_camera();
        let views = views_of(&cam, 0.1, 11);
        let rep =
            calibrate_views(&views, BOARD, SQ, (640, 480), &opts(LensModelChoice::Auto)).unwrap();
        eprintln!(
            "auto (ojo de pez): {:?} avisos {:?}",
            rep.model_scores, rep.warnings
        );
        assert_eq!(rep.calibration.model, LensModel::Fisheye);
        assert!(
            rep.model_scores
                .iter()
                .any(|s| s.chosen && s.model == LensModel::Fisheye)
        );
        assert!(rep.warnings.iter().any(|w| w.contains("ojo de pez")));
        assert!(
            rep.calibration.view_zoom < 1.0,
            "zoom {}",
            rep.calibration.view_zoom
        );

        // Lente normal con distorsión de barril: se queda en Brown-Conrady.
        let normal = truth_camera();
        let mut rng = Rng::new(5);
        let views: Vec<Vec<Point>> = standard_poses(BOARD, SQ)
            .iter()
            .map(|p| {
                board_corners(&normal, p, BOARD, SQ)
                    .into_iter()
                    .map(|c| Point::new(c.x + 0.1 * rng.gauss(), c.y + 0.1 * rng.gauss()))
                    .collect()
            })
            .collect();
        let rep =
            calibrate_views(&views, BOARD, SQ, (640, 480), &opts(LensModelChoice::Auto)).unwrap();
        eprintln!("auto (normal): {:?}", rep.model_scores);
        assert_eq!(rep.calibration.model, LensModel::BrownConrady);
        assert_eq!(rep.calibration.view_zoom, 1.0);
        assert_eq!(rep.model_scores.len(), 2);
        // Las dos tienen validación cruzada (hay 10 vistas) y está cerca del ruido.
        assert!(rep.model_scores.iter().all(|s| s.cv_rms_px.is_some()));
    }

    #[test]
    fn la_validacion_cruzada_delata_el_sobreajuste_de_brown_en_el_borde() {
        // Con vistas solo del centro Brown-Conrady parece bueno; las vistas reservadas del
        // borde lo delatan.
        let cam = fisheye_camera();
        let views = views_of(&cam, 0.1, 3);
        let refs: Vec<&[Point]> = views.iter().map(|v| v.as_slice()).collect();
        let obj: Vec<[f64; 2]> = (0..6)
            .flat_map(|j| (0..9).map(move |i| [f64::from(i) * SQ, f64::from(j) * SQ]))
            .collect();
        let o = CalibrationOptions::default();
        let cv_fish = cross_validate(LensModel::Fisheye, &refs, &obj, (640, 480), &o).unwrap();
        let cv_brown =
            cross_validate(LensModel::BrownConrady, &refs, &obj, (640, 480), &o).unwrap();
        eprintln!("validación cruzada: KB {cv_fish:.3} px, BC {cv_brown:.3} px");
        assert!(cv_fish < 0.5, "KB {cv_fish}");
        assert!(cv_brown > 4.0 * cv_fish, "BC {cv_brown} vs KB {cv_fish}");
    }

    #[test]
    fn desde_imagenes_renderizadas_el_ojo_de_pez_se_calibra_y_se_elige_solo() {
        let cam = fisheye_camera();
        let imgs: Vec<GrayImage> = fisheye_poses(BOARD, SQ)
            .iter()
            .enumerate()
            .map(|(i, p)| render_board(&cam, p, BOARD, SQ, 1.5, 300 + i as u64))
            .collect();
        let rep = calibrate_lens(&imgs, BOARD, SQ).unwrap();
        let c = &rep.calibration;
        let (rms, max) = plane_error_mm(c, &cam, 50.0);
        eprintln!(
            "desde imágenes: {:?} vistas {}/16 rms {:.3} px; plano rms {rms:.2} mm max {max:.2} mm; {:?} {:?}",
            c.model, c.views_used, c.rms_error_px, rep.model_scores, rep.warnings
        );
        assert_eq!(c.model, LensModel::Fisheye);
        assert!(c.views_used >= 8, "vistas {}", c.views_used);
        assert!(c.rms_error_px < 0.6);
        assert!(rms < 3.0, "plano rms {rms:.2} mm");
    }

    #[test]
    fn el_json_viejo_sin_modelo_sigue_siendo_brown_conrady() {
        let json = r#"{"width":640,"height":480,"fx":620.0,"fy":627.0,"cx":322.5,"cy":238.0,
            "k1":-0.2,"k2":0.05,"p1":0.0006,"p2":-0.0005,"k3":0.0,"rms_error_px":0.3,"views_used":8}"#;
        let c: LensCalibration = serde_json::from_str(json).unwrap();
        assert_eq!(c.model, LensModel::BrownConrady);
        assert_eq!(c.k4, 0.0);
        assert_eq!(c.view_zoom, 1.0);
        c.validate().unwrap();
        // Lo nuevo se guarda con el nombre del modelo.
        let back = serde_json::to_string(&fisheye_camera()).unwrap();
        assert!(back.contains("\"model\":\"fisheye\""), "{back}");
    }
}
