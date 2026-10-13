//! Texto a contornos vectoriales para grabar y cortar.
//!
//! - [`FontLibrary`] carga las fuentes (sistema o archivos) y elige la cara por familia,
//!   negrita y cursiva, informando cuando hay que usar un sustituto.
//! - [`outline`] convierte un [`TextSpec`] en curvas [`BezPath`](tlaku_core::bezpath::BezPath)
//!   en milímetros, con kerning y ligaduras (rustybuzz), varias líneas y texto en trayecto.
//!
//! Convenciones: Y hacia arriba, la línea base de la primera línea en `y = 0`; `size_mm` es el
//! tamaño del em (no la altura de la mayúscula). Los contornos conservan el sentido que traen
//! las fuentes, así que tanto la regla par-impar como la non-zero rellenan bien los huecos.

mod effects;
mod layout;
mod library;
mod linefont;
mod path_sampler;
mod shx;
mod spec;

pub use effects::MAX_BEND_DEG;
pub use layout::{LINE_FONT_CAP_RATIO, TextOutline, outline};
pub use library::{FaceChoice, FontFamily, FontLibrary};
pub use linefont::{LineFont, LineFontSource, LineGlyph, builtin_line_fonts};
pub use spec::{Align, OnPathSpec, TextSpec};
