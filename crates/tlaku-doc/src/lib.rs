//! Tipos del contrato entre la app y el motor (ver `docs/app-api.md`).
//!
//! Todo se serializa con `serde` en `snake_case` y se exporta a TypeScript con `ts-rs`
//! (`cargo test -p tlaku-doc` escribe los `.ts` en `apps/desktop/src/bindings/`).
//! Unidades: mm, mm/min y %. Origen abajo a la izquierda, Y hacia arriba.

pub mod affine;
// Re-exporta las curvas para quien solo depende de este crate.
pub use tlaku_core::bezpath::{BezPath, PathSeg};
mod device;
mod document;
mod job;
mod layer;
mod machine;
mod text;

pub use device::{DeviceKind, DeviceProfile, Origin, PowerMode};
pub use document::{
    DOC_VERSION, DesignObject, Document, FLATTEN_TOLERANCE_MM, ImageObject, ObjectKind,
    VectorObject, bounds_of, union,
};
pub use job::{ImportResult, JobOptions, JobPreview, LayerOrder, PreviewLayer, StartFrom};
pub use layer::{ImageMode, ImageSettings, LayerMode, LayerSettings, Overscan};
pub use machine::{
    BufferStatus, ConnectResult, ConnectionEvent, ExportResult, JobEvent, JobState, LogDir,
    LogEvent, MachineState, ProgressEvent, RealtimeCommand, StatusReport,
};
pub use text::{TextAlign, TextObject, TextOnPath};

#[cfg(test)]
mod tests;
