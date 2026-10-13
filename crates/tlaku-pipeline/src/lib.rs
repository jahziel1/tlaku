//! Del documento (objetos + ajustes por capa + máquina) a `Toolpath`, estimaciones y G-code.
//!
//! Es la lógica compartida por el CLI y la app: no guarda estado, recibe el documento
//! completo y devuelve resultados.
//!
//! - [`import_file`]: SVG, DXF o imagen a objetos y capas nuevas.
//! - [`compute`] / [`build`]: documento a [`ComputedJob`] (trayectoria, vista previa, avisos).
//! - [`gcode`]: [`ComputedJob`] a G-code para la máquina.
//!
//! Las curvas del documento se aplanan al construir el trabajo con [`FLATTEN_TOLERANCE_MM`]
//! (0.01 mm); un texto sin contorno generado se omite con una advertencia.

mod build;
mod gcode;
mod import;
mod job;
mod layers;
mod material_test;
mod place;
mod preview;

pub use build::{build, compute, compute_at};
pub use gcode::{frame_gcode, gcode};
pub use import::{ImportOptions, InputKind, import_file, import_file_with, input_kind};
pub use job::{ComputedJob, LayerReport, block_bounds, check_bounds};
pub use material_test::material_test;
pub use layers::{
    LayerRole, MAX_PASSES, ResolvedImage, ResolvedLayer, classify, layer_sequence, resolve_image,
    resolve_layer,
};

/// Los tipos del documento, para quien solo depende de este crate.
pub use tlaku_doc as doc;
pub use tlaku_doc::FLATTEN_TOLERANCE_MM;

#[cfg(test)]
mod tests;
