//! Comandos del área `camera` (fases 3 y 4): detección de tablero, calibración de lente, marcas
//! grabadas para alinear con la cama, corrección por altura del material y la configuración
//! persistente (calibración + alineación con la cama).
//!
//! El video nunca pasa por aquí: la interfaz corrige cada cuadro con WebGL. Estos comandos
//! reciben fotogramas sueltos (PNG en base64) y guardan lo que no debe perderse al reiniciar.

use crate::engine::write_atomic;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use image::GrayImage;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use tlaku_camera::{
    CalibrationOptions, CalibrationReport, CircleFit, CircleHint, DetectOptions, HeightModel,
    HeightSample, Homography, ImageToBed, LensCalibration, LensModelChoice, MarkLayout,
    MarksReport, MosaicOptions, Tile, detect_circle, detect_marks, find_chessboard,
    fit_height_model,
};
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Point, Polyline, Rgb};
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{
    DOC_VERSION, DesignObject, Document, LayerMode, LayerSettings, ObjectKind, VectorObject,
};
use tlaku_i18n::tr;

/// Archivo de la configuración de cámara dentro de la carpeta de configuración de la app.
const SETUP_FILE: &str = "camera.json";
/// Versión del formato de `camera.json`.
const SETUP_VERSION: u32 = 1;

/// Decodifica un PNG (o JPEG/BMP) en base64, con o sin prefijo `data:...;base64,`, a gris.
fn decode_gray(b64: &str) -> Result<GrayImage, String> {
    let raw = b64.split_once(',').map_or(b64, |(_, rest)| rest).trim();
    let bytes = STANDARD
        .decode(raw)
        .map_err(|e| tr!("desktop.edit.camera.bad_base64", err = e))?;
    let img = image::load_from_memory(&bytes)
        .map_err(|e| tr!("desktop.edit.camera.decode_failed", err = e))?;
    Ok(img.to_luma8())
}

mod bed_image;
mod board;
mod circle;
mod marks;
mod network;
mod normalize;
mod setup;
mod store;

// Las rutas `edit::camera::x` (comandos y `run_*`) se conservan con estos `pub use`; los
// elementos privados de cada submódulo se comparten con sus hermanos y con las pruebas.
pub use bed_image::*;
pub use board::*;
pub use circle::*;
pub use marks::*;
pub use network::*;
pub use normalize::*;
pub use setup::*;
pub use store::*;

#[cfg(test)]
mod tests;
