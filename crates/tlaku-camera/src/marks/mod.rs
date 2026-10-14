//! Marcas grabadas para alinear la cámara con la cama.
//!
//! El usuario graba sobre un cartón 4 o 9 cruces en posiciones conocidas de la cama (la
//! primera, la esquina inferior izquierda, lleva además un círculo concéntrico que fija la
//! orientación). Se fotografía sin mover el material, se corrige la lente y este módulo:
//!
//! 1. Busca cruces en la imagen: puntos donde el tensor de estructura (gradientes
//!    integrados en una ventana pequeña) tiene dos direcciones fuertes, con la condición de
//!    simetría de punto de una cruz (descarta esquinas del cartón, vetas y ruido).
//! 2. Las asigna a sus posiciones teóricas. Con una alineación provisional (la manual de 4
//!    puntos) se usa el candidato más cercano a cada posición prevista; sin ella, una
//!    búsqueda por hipótesis (4 candidatos contra 4 marcas, descartando espejos) puntuada por
//!    cuántas otras marcas explica y por el círculo de la marca de origen. Funciona aunque
//!    falte alguna marca o sobren candidatos.
//! 3. Refina el centro de cada cruz con el método de Förstner (mínimos cuadrados de las
//!    rectas normales al gradiente), con una ventana ligada al tamaño de la cruz en píxeles.
//! 4. Ajusta la homografía con RANSAC y mide la precisión real con validación cruzada
//!    dejando una marca fuera, en mm de cama.
//!
//! Convención: el centro del píxel `(i, j)` es `(i, j)`; `DetectOptions::pixel_offset`
//! permite entregar y recibir coordenadas con el origen en el borde del píxel (0.5).
//! Las marcas se colocan en coordenadas de cama (mm, Y hacia arriba).

mod assign;
mod candidates;
mod detect;
mod layout;
mod refine;
mod report;
#[cfg(test)]
mod tests;

pub use detect::detect_marks;
pub use layout::{MarkLayout, MarkStroke};
pub use report::{DetectOptions, MarkResult, MarksReport};

/// Fracción del ancho (y del alto) de la cama que se deja libre a cada lado: las marcas
/// cubren el 85 % de cada dimensión.
const INSET: f64 = 0.075;
/// Medida mínima de la cama para generar marcas (mm).
const MIN_BED_MM: f64 = 60.0;
/// Un residuo mayor a este valor (mm) se considera dudoso.
pub const RESIDUAL_WARN_MM: f64 = 0.3;
