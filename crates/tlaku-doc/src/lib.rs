//! Tipos del contrato entre la app y el motor (ver `docs/app-api.md`).
//!
//! Todo se serializa con `serde` en `snake_case` y se exporta a TypeScript con `ts-rs`
//! (`cargo test -p tlaku-doc` escribe los `.ts` en `apps/desktop/src/bindings/`).
//! Unidades: mm, mm/min y %. Origen abajo a la izquierda, Y hacia arriba.

pub mod affine;
// Re-exporta las curvas para quien solo depende de este crate.
pub use tlaku_core::bezpath::{BezPath, PathSeg};
mod community;
mod cut;
mod device;
mod document;
mod factory;
mod galvo;
mod job;
mod layer;
mod lbdev;
mod machine;
mod materials;
mod resume;
mod ruida;
mod text;
mod variables;
mod virtual_array;

pub use community::{
    COMMUNITY_ID_PREFIX, COMMUNITY_ISSUES_URL, COMMUNITY_LICENSE, CommunityEntry, issue_number,
};
pub use cut::{
    CutThrough, DotMode, FillGrouping, Guide, GuideAxis, Lead, LeadKind, Optimization, Perforation,
    Ramp, ShapeParams, StartSpec, VariableText, VirtualArray,
};
pub use device::{
    CommentStyle, Connection, DeviceKind, DeviceProfile, DwellFormat, GcodeDialect,
    LhyMicroSettings, Macro, Origin, PowerMode, RotaryKind, RotarySettings, RuidaSettings,
    SavedPosition, ScanOffset, SpeedUnit,
};
pub use document::{
    CornerMark, DOC_VERSION, DesignObject, Document, FLATTEN_TOLERANCE_MM, ImageObject, ObjectKind,
    VectorObject, bounds_of, union,
};
pub use factory::{FACTORY_ID_PREFIX, check_factory, factory_library, parse_factory};
pub use galvo::{
    CylinderAxis, CylinderCorrection, FeederSettings, FieldCorrection, GalvoDevice, GalvoJump,
    GalvoLayer, GalvoRotary, GalvoTiming, InputRequirement, LaserSource, RepeatMarking, Slice3d,
    SplitAxes, SplitMarking, SplitOrder, TaperWarp, TileSpec, Wobble,
};
pub use job::{ImportResult, JobOptions, JobPreview, LayerOrder, PreviewLayer, StartFrom};
pub use layer::{
    FillStyle, ImageMode, ImageSettings, LayerMode, LayerSettings, Overscan, TabSettings,
};
pub use lbdev::{
    LbdevImport, LbdevNotice, LbdevNoticeKind, kind_from_name, origin_from_mirror, parse_lbdev,
};
pub use machine::{
    BufferStatus, ConnectResult, ConnectionEvent, ExportResult, JobEvent, JobState, LogDir,
    LogEvent, MachineState, OverrideKind, Overrides, ProgressEvent, RealtimeCommand,
    SimulatedFault, StatusReport,
};
pub use materials::{
    Confidence, LaserKind, MATERIAL_LIBRARY_VERSION, MachineClass, MaterialLibrary, MaterialPreset,
    MaterialRestriction, MaterialRole, MaterialTestParams, MaterialWarning, TestAxis, TestParam,
};
pub use resume::{
    InterruptedJob, ResumeAir, ResumeKind, ResumeMachine, ResumeModal, ResumePoint,
    ResumeReadiness, ResumeSpindle, ResumeStart, ResumeView,
};
pub use ruida::{
    RuidaApplyReport, RuidaBackup, RuidaCalibration, RuidaControllerSettings, RuidaDocResult,
    RuidaFile, RuidaInfo, RuidaPlannedChange, RuidaSetting, RuidaSettingChange, RuidaSettingsPlan,
    RuidaVerified,
};
pub use text::{TextAlign, TextAxis, TextFeature, TextObject, TextOnPath, TextVAlign};
pub use variables::{
    DataSource, ImageFit, ShortRows, VariableCode, VariableImage, absolutize, clean_source_name,
    relativize,
};
pub use virtual_array::MAX_VIRTUAL_COPIES;

#[cfg(test)]
mod tests;
