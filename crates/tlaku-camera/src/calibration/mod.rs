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
//! validación cruzada con vistas reservadas (ver `select_model`).
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

mod calibrate;
#[cfg(test)]
mod fisheye_tests;
mod fit;
mod intrinsics;
mod lm;
mod model;
mod options;
mod select;
#[cfg(test)]
mod tests;

pub use calibrate::{calibrate_lens, calibrate_lens_files, calibrate_lens_with, calibrate_views};
pub use model::{LensCalibration, LensModel};
pub use options::{CalibrationOptions, CalibrationReport, LensModelChoice, ModelScore};
