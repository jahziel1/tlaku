//! Alinear una cámara FIJA sobre la cama con el cabezal de la máquina, sin grabar ni gastar material.
//!
//! Con el láser apagado, el cabezal visita una rejilla de puntos conocidos de la cama y la cámara
//! toma una foto en cada uno (más una de referencia con el cabezal en otro lugar). El motor:
//!
//! 1. [`plan_grid`] reparte los puntos dentro del área segura y los ordena para recorrerlos sin
//!    saltos largos.
//! 2. [`detect_heads`] localiza el cabezal en cada foto: por su LED verde si lo tiene, o **por
//!    diferencia de imágenes** (el fondo de cada píxel es la mediana de las fotos de la misma fila
//!    y lo que cambia respecto de ese fondo es el cabezal; el pórtico queda en el fondo). Sale un
//!    punto característico estable en píxeles con precisión de subpíxel. Una cámara que cubre
//!    solo parte de la cama se maneja con [`plan_refine`]: nueva rejilla donde ya se vio el cabezal.
//! 3. [`fit_head_alignment`] con los pares (posición de la máquina en mm, píxel) ajusta una
//!    homografía píxel a cama. Si la imagen ya viene corregida de lente basta eso; si no hay
//!    calibración de lente, estima una distorsión aproximada (Brown-Conrady o ojo de pez) con el
//!    mismo ajuste. Descarta atípicos y mide el error por punto con validación cruzada (mm).
//!
//! Paralaje: el punto detectado está unos mm por encima del material y la cámara lo ve desplazado
//! hacia fuera de su vertical. Se corrige con la altura de la cámara y la del punto ([`Parallax`])
//! y un desfase fijo en mm ([`HeadAlignOptions::offset_mm`]), que se calcula con un único punto de
//! referencia conocido ([`reference_offset`]).
//!
//! Convención de píxeles: el centro del píxel `(i, j)` es `(i, j)` en la detección; el ajuste
//! entrega la alineación con el origen en el borde del píxel (suma 0.5), como el resto de la
//! alineación de la cámara.

mod detect;
mod fit;
mod lensfit;
mod plan;
pub mod sim;
#[cfg(test)]
mod tests;

pub use detect::{
    DetectFailure, DetectOutcome, HeadDetectOptions, HeadDetection, HeadFeature, HeadMethod,
    detect_heads,
};
pub use fit::{
    HeadAlignFit, HeadAlignIssue, HeadAlignOptions, HeadObservation, Parallax, PointFit, Quality,
    fit_head_alignment, reference_offset,
};
pub use plan::{HeadAlignPlan, MAX_GRID, MIN_GRID, PlanOptions, plan_grid, plan_refine};
