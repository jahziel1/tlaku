//! Códigos QR y de barras como formas vectoriales cerradas, listas para rellenar o grabar.
//!
//! - [`generate`] convierte un [`CodeSpec`] en un [`CodeOutput`]: contornos cerrados (barras o
//!   cuadros ya unidos, sin costuras entre módulos vecinos) en milímetros, con la esquina
//!   inferior izquierda del conjunto en `(0, 0)` y Y hacia arriba.
//! - Simbologías: QR, Data Matrix, Code 128, Code 39, EAN-13, EAN-8, UPC-A, ITF (2 de 5
//!   intercalado) y Codabar.
//! - Con `invert` se entrega el fondo (un recuadro con los módulos como huecos), para grabar el
//!   fondo y dejar el código en el color del material.
//!
//! Dependencias (todas permisivas): `qrcode` (MIT/Apache-2.0), `barcoders` (MIT/Apache-2.0) y
//! `datamatrix` (MIT/Apache-2.0). `rqrr` (MIT/Apache-2.0 con ISC) solo en las pruebas.

mod encode;
mod geometry;

use serde::{Deserialize, Serialize};
use tlaku_core::bezpath::BezPath;
use tlaku_i18n::tr;
use tlaku_text::FontLibrary;

pub use encode::{Encoded, check_digit, encode_linear};

/// Simbología.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeKind {
    #[default]
    Qr,
    DataMatrix,
    Code128,
    Code39,
    Ean13,
    Ean8,
    UpcA,
    Itf,
    Codabar,
}

impl CodeKind {
    /// Los códigos de barras de una dimensión (con alto propio y texto legible).
    pub fn is_linear(self) -> bool {
        !matches!(self, CodeKind::Qr | CodeKind::DataMatrix)
    }
}

/// Nivel de corrección de errores del QR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QrEcc {
    /// ~7 %
    L,
    /// ~15 %
    #[default]
    M,
    /// ~25 %
    Q,
    /// ~30 %
    H,
}

/// Qué código generar y cómo dibujarlo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CodeSpec {
    pub kind: CodeKind,
    pub data: String,
    /// Corrección de errores (solo QR).
    pub ecc: QrEcc,
    /// 2D: lado del área de módulos (mm). 1D: ancho de las barras, sin zona en blanco (mm).
    pub size_mm: f64,
    /// 1D: alto de las barras (mm), sin el texto. Se ignora en los códigos 2D.
    pub height_mm: f64,
    /// Zona en blanco alrededor (mm). Solo se dibuja al invertir, como borde del fondo.
    pub margin_mm: f64,
    /// 1D: escribe los datos bajo las barras.
    pub human_readable: bool,
    /// Entrega el fondo con los módulos como huecos (para grabar el fondo).
    pub invert: bool,
    /// Fuente de contorno para el texto legible (no una de una línea).
    pub font_family: String,
}

impl Default for CodeSpec {
    fn default() -> Self {
        Self {
            kind: CodeKind::Qr,
            data: String::new(),
            ecc: QrEcc::M,
            size_mm: 30.0,
            height_mm: 15.0,
            margin_mm: 2.0,
            human_readable: false,
            invert: false,
            font_family: String::new(),
        }
    }
}

/// Resultado: formas cerradas (cada `BezPath` es un exterior seguido de sus agujeros).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CodeOutput {
    pub paths: Vec<BezPath>,
    pub width_mm: f64,
    pub height_mm: f64,
    /// Módulos por lado (2D) o módulos de ancho (1D).
    pub modules: usize,
    pub warnings: Vec<String>,
}

/// Módulos más pequeños que esto (mm) suelen quedar por debajo de lo que graba una máquina.
const MIN_MODULE_MM: f64 = 0.25;

/// Genera el código. `lib` solo se usa para el texto legible.
pub fn generate(lib: &FontLibrary, spec: &CodeSpec) -> Result<CodeOutput, String> {
    if spec.data.is_empty() {
        return Err(tr!("codes.lib.empty_content"));
    }
    if !(spec.size_mm.is_finite() && spec.size_mm > 0.0) {
        return Err(tr!("codes.lib.bad_size", value = spec.size_mm));
    }
    if !(spec.margin_mm.is_finite() && spec.margin_mm >= 0.0) {
        return Err(tr!("codes.lib.negative_margin"));
    }
    if spec.kind.is_linear() && !(spec.height_mm.is_finite() && spec.height_mm > 0.0) {
        return Err(tr!("codes.lib.bad_height", value = spec.height_mm));
    }

    let mut warnings = Vec::new();
    let out = if spec.kind.is_linear() {
        let encoded = encode_linear(spec.kind, &spec.data)?;
        warnings.extend(encoded.warnings.iter().cloned());
        geometry::linear(lib, spec, &encoded)?
    } else {
        let (bits, side) = encode::matrix(spec, &mut warnings)?;
        geometry::matrix(spec, &bits, side)?
    };
    let module_mm = spec.size_mm / out.modules as f64;
    if module_mm < MIN_MODULE_MM {
        warnings.push(tr!(
            "codes.lib.small_module",
            module = format!("{module_mm:.2}")
        ));
    }
    Ok(CodeOutput { warnings, ..out })
}

#[cfg(test)]
mod tests;
