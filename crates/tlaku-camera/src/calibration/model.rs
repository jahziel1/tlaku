//! Modelos de distorsión de la lente y la calibración resultante.

use serde::{Deserialize, Serialize};
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
pub(super) fn distort_with(
    model: LensModel,
    k: [f64; 4],
    p: [f64; 2],
    x: f64,
    y: f64,
) -> (f64, f64) {
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
pub(super) fn undistort_with(
    model: LensModel,
    k: [f64; 4],
    p: [f64; 2],
    xd: f64,
    yd: f64,
) -> (f64, f64) {
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
