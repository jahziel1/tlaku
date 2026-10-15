//! Del documento (objetos + ajustes por capa + máquina) a `Toolpath`, estimaciones y G-code.
//!
//! Es la lógica compartida por el CLI y la app: no guarda estado, recibe el documento
//! completo y devuelve resultados.
//!
//! - [`import_file`]: SVG, DXF o imagen a objetos y capas nuevas.
//! - [`compute`] / [`build`]: documento a [`ComputedJob`] (trayectoria, vista previa, avisos).
//! - [`gcode`]: [`ComputedJob`] a G-code para la máquina.
//! - [`mark_sequence`]: piezas y movimientos de mesa o rotativo del marcado dividido y repetido
//!   (galvo); ver también `GalvoDevice` en `tlaku_doc`.
//!
//! Las curvas del documento se aplanan al construir el trabajo con [`FLATTEN_TOLERANCE_MM`]
//! (0.01 mm); un texto sin contorno generado se omite con una advertencia.

mod build;
mod cylinder;
mod galvo;
mod gcode;
mod import;
mod import_egv;
mod import_gcode;
mod import_hpgl;
mod import_lbrn;
mod import_pdf;
mod import_rd;
mod job;
mod layers;
mod material_test;
mod place;
mod preview;
mod processed;
mod registration;
mod rotary;
mod ruida;
mod scan_offset;
mod slice3d;
mod split;
mod split_shapes;
mod start;
mod stroke;
mod tabs;
mod tile;
mod tonal;
mod zaxis;

pub use build::{build, compute, compute_at};
pub use galvo::field_rect;
pub use gcode::{
    doc_to_machine, frame_gcode, frame_gcode_pointer, gcode, link_load, machine_to_doc,
    pointer_shift, swap_active,
};
pub use import::{
    ImportExtras, ImportOptions, InputKind, import_file, import_file_extras, import_file_with,
    input_kind,
};
pub use job::{
    ComputedJob, LayerReport, block_bounds, check_bounds, check_device_bounds, is_nothing_to_send,
};
pub use layers::{
    LayerRole, MAX_PASSES, ResolvedImage, ResolvedLayer, classify, layer_sequence, resolve_image,
    resolve_layer,
};
pub use material_test::material_test;
pub use processed::{ProcessedPreview, processed_png, processed_preview};
pub use registration::{
    FitChoice, FitError, FitMode, FitWarning, MIN_SEPARATION_MM, MarkPair, RegistrationFit,
    SCALE_WARN, are_collinear, fit_registration, fit_with_choice,
};
pub use rotary::rotary_test;
pub use ruida::{export_rd, export_rd_for_device, home_for_origin, limits_for_device};
pub use split::{
    GalvoRotarySetup, MAX_PIECES, MarkPlan, MarkStep, StageMove, galvo_rotary, job_mark_sequence,
    mark_sequence, mark_sequence_shapes,
};
pub use split_shapes::{ShapeScan, ShapeUnit, doc_mark_sequence, scan_shapes};
pub use tile::{
    MARK_ID_PREFIX, MAX_TILES, PlannedTile, TileError, TileOrder, TilePlan, add_alignment_marks,
    plan_grid, plan_tiles, shift_tiles, tile_marks,
};
pub use tonal::{
    MAX_BOOST, MAX_SIDE_PX, MIN_PX_MM, SHEET_MARGIN_MM, TonalImage, TonalOptions, render_tonal,
};

/// Los tipos del documento, para quien solo depende de este crate.
pub use tlaku_doc as doc;
pub use tlaku_doc::FLATTEN_TOLERANCE_MM;

#[cfg(test)]
mod tests;
