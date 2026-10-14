//! Tipos del contrato entre la app y el motor (ver `docs/app-api.md`).
//!
//! Todo se serializa con `serde` en `snake_case` y se exporta a TypeScript con `ts-rs`
//! (`cargo test -p tlaku-doc` escribe los `.ts` en `apps/desktop/src/bindings/`).
//! Unidades: mm, mm/min y %. Origen abajo a la izquierda, Y hacia arriba.

pub mod affine;
// Re-exporta las curvas para quien solo depende de este crate.
pub use tlaku_core::bezpath::{BezPath, PathSeg};
mod cut;
mod device;
mod document;
mod galvo;
mod job;
mod layer;
mod machine;
mod materials;
mod ruida;
mod text;
mod virtual_array;

pub use cut::{
    CutThrough, DotMode, FillGrouping, Guide, GuideAxis, Lead, LeadKind, Optimization, Perforation,
    Ramp, ShapeParams, StartSpec, VariableText, VirtualArray,
};
pub use device::{
    CommentStyle, Connection, DeviceKind, DeviceProfile, DwellFormat, GcodeDialect,
    LhyMicroSettings, Macro, Origin, PowerMode, RotaryKind, RotarySettings, RuidaSettings,
    SavedPosition, ScanOffset,
};
pub use document::{
    DOC_VERSION, DesignObject, Document, FLATTEN_TOLERANCE_MM, ImageObject, ObjectKind,
    VectorObject, bounds_of, union,
};
pub use galvo::{
    CylinderAxis, CylinderCorrection, FeederSettings, FieldCorrection, GalvoDevice, GalvoJump,
    GalvoLayer, GalvoTiming, LaserSource, RepeatMarking, Slice3d, SplitAxes, SplitMarking,
    SplitOrder, TaperWarp, TileSpec, Wobble,
};
pub use job::{ImportResult, JobOptions, JobPreview, LayerOrder, PreviewLayer, StartFrom};
pub use layer::{
    FillStyle, ImageMode, ImageSettings, LayerMode, LayerSettings, Overscan, TabSettings,
};
pub use machine::{
    BufferStatus, ConnectResult, ConnectionEvent, ExportResult, JobEvent, JobState, LogDir,
    LogEvent, MachineState, OverrideKind, Overrides, ProgressEvent, RealtimeCommand,
    SimulatedFault, StatusReport,
};
pub use materials::{
    MATERIAL_LIBRARY_VERSION, MaterialLibrary, MaterialPreset, MaterialTestParams, TestAxis,
    TestParam,
};
pub use ruida::{RuidaFile, RuidaInfo};
pub use text::{TextAlign, TextObject, TextOnPath};
pub use virtual_array::MAX_VIRTUAL_COPIES;

#[cfg(test)]
mod tests;
