//! Cámara: calibración de lente, corrección de distorsión y homografía imagen-cama.
//!
//! Todo en Rust puro, sin OpenCV. Algoritmos de la literatura pública:
//! - Detección de tablero de ajedrez: puntos de silla (Hessiano) + agrupación en
//!   rejilla + refinamiento subpíxel tipo Förstner (ver [`chessboard`]).
//! - Calibración: Zhang (2000), solución cerrada más Levenberg-Marquardt, con el
//!   modelo de distorsión de Brown-Conrady (k1, k2, p1, p2, k3) (ver [`calibration`]).
//! - Homografía: DLT normalizada de Hartley y Zisserman, con RANSAC opcional
//!   (ver [`homography`]).
//!
//! Convención de píxeles: el centro del píxel `(i, j)` tiene coordenadas `(i, j)`;
//! el eje Y de la imagen crece hacia abajo.

pub mod calibration;
pub mod chessboard;
pub mod homography;
mod imgproc;
pub mod undistort;

#[cfg(test)]
mod testutil;

pub use calibration::{
    CalibrationOptions, CalibrationReport, LensCalibration, calibrate_lens, calibrate_lens_files,
    calibrate_lens_with, calibrate_views,
};
pub use chessboard::find_chessboard;
pub use homography::{
    AlignmentFit, Homography, ImageToBed, RansacOptions, RansacResult, estimate_homography,
    estimate_homography_ransac,
};
pub use undistort::{UndistortMap, undistort_file, undistorted_path};
