//! Tipos del contrato entre la app y el motor (ver `docs/app-api.md`).
//!
//! Todo se serializa con `serde` en `snake_case` y se exporta a TypeScript con `ts-rs`
//! (`cargo test -p tlaku-doc` escribe los `.ts` en `apps/desktop/src/bindings/`).
//! Unidades: mm, mm/min y %. Origen abajo a la izquierda, Y hacia arriba.

pub mod affine;
mod device;
mod document;
mod job;
mod layer;
mod machine;

pub use device::{DeviceKind, DeviceProfile, Origin, PowerMode};
pub use document::{
    DesignObject, Document, FORMAT_VERSION, ImageObject, ObjectKind, PathData, VectorObject,
    bounds_of, union,
};
pub use job::{ImportResult, JobOptions, JobPreview, LayerOrder, PreviewLayer, StartFrom};
pub use layer::{ImageMode, ImageSettings, LayerMode, LayerSettings, Overscan};
pub use machine::{
    BufferStatus, ConnectResult, ConnectionEvent, ExportResult, JobEvent, JobState, LogDir,
    LogEvent, MachineState, ProgressEvent, RealtimeCommand, StatusReport,
};

#[cfg(test)]
mod tests;
