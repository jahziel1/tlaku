//! Generadores de piezas para corte láser (cajas con dientes, bisagras flexibles, pieza de prueba
//! de encaje).
//!
//! Cada generador es una función pura `parámetros -> Result<Generated, PartsError>`: sin disco,
//! sin estado ni hilos. Las medidas están en mm, con el origen abajo a la izquierda y Y hacia
//! arriba, y la geometría es **nominal**: el kerf lo compensa la capa del documento
//! (`tlaku_geom::kerf_compensate`) y el ajuste de encaje (`fit_mm`) es un parámetro aparte.
//!
//! Para añadir un generador (engranajes, bandejas, cola de milano...) basta un módulo con su
//! struct de parámetros (`Serialize + Deserialize + Default`), su función, su `schema()` y una
//! entrada en [`registry::GENERATORS`]; el comando de Tauri, el diálogo y el mock no cambian. La
//! guía está en `docs/mapa.md`, sección «Generadores de piezas».

pub mod boxes;
pub mod error;
pub mod fingers;
pub mod fit_test;
pub mod font;
pub mod hinge;
pub mod joints;
pub mod layout;
pub mod part;
pub mod rectilinear;
pub mod registry;
pub mod rounded_box;
#[cfg(test)]
mod testutil;

pub use error::PartsError;
pub use part::{Generated, Part};
pub use registry::{GENERATORS, GenerateResult, GeneratorInfo, PlacedPart, generate, list};
