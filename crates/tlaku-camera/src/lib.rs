//! Cámara: calibración de lente, corrección de distorsión y homografía imagen-cama.
//!
//! Todo en Rust puro, sin OpenCV. Algoritmos de la literatura pública:
//! - Detección de tablero de ajedrez: puntos de silla (Hessiano) + agrupación en
//!   rejilla + refinamiento subpíxel tipo Förstner (ver [`chessboard`]).
//! - Calibración: Zhang (2000), solución cerrada más Levenberg-Marquardt, con el
//!   modelo de distorsión de Brown-Conrady (k1, k2, p1, p2, k3) (ver [`calibration`]).
//! - Homografía: DLT normalizada de Hartley y Zisserman, con RANSAC opcional
//!   (ver [`homography`]).
//! - Marcas grabadas: detección de cruces con tensor de estructura y refinamiento de
//!   Förstner, asignación robusta y validación cruzada en mm (ver [`marks`]).
//! - Ojo de pez: modelo equidistante de Kannala-Brandt como alternativa a Brown-Conrady, con
//!   elección automática por validación cruzada (ver [`calibration`]).
//! - Mosaico: fotos con su posición en la cama (una captura de la cama o la cámara del cabezal)
//!   compuestas con mezcla en los bordes (ver [`mosaic`]).
//! - Buscador de círculos: Hough con gradiente y ajuste por mínimos cuadrados (ver [`circle`]).
//! - Alinear con el cabezal (cámara fija, sin grabar): el cabezal visita puntos conocidos, se lo
//!   localiza por su LED verde o por diferencia de imágenes y se ajusta una homografía píxel a cama
//!   (ver [`head_align`]).
//! - Altura del material: `H(h) = [H1 H2 (H3 + h·G)]` ajustado con marcas a dos alturas
//!   (ver [`height`]).
//!
//! Convención de píxeles: el centro del píxel `(i, j)` tiene coordenadas `(i, j)`;
//! el eje Y de la imagen crece hacia abajo.

pub mod calibration;
pub mod chessboard;
pub mod circle;
pub mod head_align;
pub mod height;
pub mod homography;
mod imgproc;
pub mod marks;
pub mod mosaic;
pub mod undistort;

#[cfg(test)]
mod testutil;

pub use calibration::{
    CalibrationOptions, CalibrationReport, LensCalibration, LensModel, LensModelChoice, ModelScore,
    calibrate_lens, calibrate_lens_files, calibrate_lens_with, calibrate_views,
};
pub use chessboard::find_chessboard;
pub use circle::{CircleFit, CircleHint, circle_from_three, detect_circle, fit_circle_points};
pub use head_align::{
    DetectFailure, DetectOutcome, HeadAlignFit, HeadAlignIssue, HeadAlignOptions, HeadAlignPlan,
    HeadDetectOptions, HeadDetection, HeadFeature, HeadMethod, HeadObservation, Parallax,
    PlanOptions, PointFit, Quality, detect_heads, fit_head_alignment, plan_grid, plan_refine,
    reference_offset,
};
pub use height::{
    HeightFit, HeightModel, HeightSample, HeightSource, MIN_HEIGHT_SPAN_MM, fit_height_model,
};
pub use homography::{
    AlignmentFit, Homography, ImageToBed, RansacOptions, RansacResult, estimate_homography,
    estimate_homography_ransac,
};
pub use marks::{
    DetectOptions, MarkLayout, MarkResult, MarkStroke, MarksReport, RESIDUAL_WARN_MM, detect_marks,
};
pub use mosaic::{
    Mosaic, MosaicOptions, Tile, compose, covered_rect, tile_scale_px_per_mm, to_gray,
};
pub use undistort::{UndistortMap, undistort_file, undistorted_path};
