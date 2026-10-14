//! Exportación del diseño a SVG, DXF, PNG, PDF y AI.
//!
//! Cada función recibe el documento, una selección opcional (ids de objeto; `None` es todo el
//! documento) y las opciones del formato, y devuelve los bytes del archivo y los avisos de lo que
//! no se pudo representar. Las coordenadas del documento son mm con Y hacia arriba; cada formato
//! hace su propia conversión (SVG e imagen con Y hacia abajo, DXF y PDF con Y hacia arriba).
//!
//! - [`export_svg`]: curvas Bézier, mm reales, una capa por color de capa y grupos.
//! - [`export_dxf`]: AC1018 (o AC1015) con LWPOLYLINE y SPLINE (ver [`DxfOptions`]).
//! - [`export_png`]: imagen a una resolución en ppp elegida.
//! - [`export_pdf`]: PDF vectorial a escala 1:1.
//! - [`export_ai`]: el mismo dibujo como PDF 1.5 con una capa por color (el `.ai` moderno es un PDF).
//!
//! Los objetos ocultos, y los de capas ocultas, no se exportan. Un texto sin contorno generado
//! se omite con un aviso; con contorno sale como trayectos.

mod dxf_out;
mod pdf;
mod png_out;
mod scene;
mod svg;

pub use dxf_out::export_dxf;
pub use pdf::{export_ai, export_pdf};
pub use png_out::export_png;
pub use svg::export_svg;

use serde::{Deserialize, Serialize};

/// Resultado de una exportación.
#[derive(Debug, Clone, PartialEq)]
pub struct Export {
    /// El archivo.
    pub bytes: Vec<u8>,
    /// Tamaño de la página o lienzo en mm, `[ancho, alto]`.
    pub size_mm: [f64; 2],
    /// Lo que no se pudo representar (texto sin contorno, imágenes que faltan...).
    pub warnings: Vec<String>,
}

/// Qué hacer con las imágenes del diseño al exportar a SVG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageHandling {
    /// No se exportan.
    Skip,
    /// Referencia al archivo por su ruta (el SVG queda chico pero depende del archivo).
    Link,
    /// Incrustadas en el SVG (autónomo).
    Embed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SvgOptions {
    pub images: ImageHandling,
    /// Grosor del trazo en mm.
    pub stroke_mm: f64,
    /// Margen alrededor del diseño, en mm.
    pub margin_mm: f64,
    /// Conserva las coordenadas del documento (el origen de la cama es la esquina del SVG) en
    /// vez de ajustar la página al diseño.
    pub keep_position: bool,
    /// Rellena además las formas de las capas en modo relleno.
    pub fill_layers: bool,
}

impl Default for SvgOptions {
    fn default() -> Self {
        Self {
            images: ImageHandling::Embed,
            stroke_mm: 0.1,
            margin_mm: 0.0,
            keep_position: false,
            fill_layers: false,
        }
    }
}

/// Cómo se escriben las curvas Bézier en el DXF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DxfCurves {
    /// Un subtrayecto que es todo curvas sale como un SPLINE cúbico exacto; el que mezcla rectas
    /// y curvas, aplanado en una sola LWPOLYLINE.
    Splines,
    /// Todo aplanado a LWPOLYLINE con `tolerance_mm`.
    Flatten,
}

/// # Por qué AC1015/AC1018 y LWPOLYLINE más SPLINE
///
/// - AC1015 (AutoCAD 2000) es la versión más antigua con LWPOLYLINE y la que leen casi todos los
///   programas de CAD, CAM y láser; R12 (AC1009) no tiene ni LWPOLYLINE ni SPLINE y obligaría a
///   POLYLINE/VERTEX, más pesado, sin ninguna ventaja. Con `true_color` se sube a AC1018
///   (AutoCAD 2004), la primera con color verdadero, que también leen todos ellos.
/// - Una cúbica de Bézier es exactamente una B-spline de grado 3 con nudos repetidos, así que un
///   SPLINE conserva la curva sin pérdida. Pero un subtrayecto con rectas y curvas mezcladas
///   necesitaría varias entidades que el programa receptor tendría que volver a unir antes de
///   cortar; por eso ese caso se aplana (con `tolerance_mm`, 0,005 mm por defecto) en una sola
///   polilínea, que siempre es una forma cerrada si el original lo era.
/// - Quien no soporte SPLINE puede pedir `Flatten`.
///
/// Los colores van por capa: una capa por color de capa, con el ACI más cercano en la capa y en
/// la entidad, y además el color verdadero (código 420, AC1018) si `true_color`, para que el color
/// exacto sobreviva en los programas que lo leen; los que no, usan el ACI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DxfOptions {
    pub curves: DxfCurves,
    /// Error máximo (mm) al aplanar.
    pub tolerance_mm: f64,
    pub true_color: bool,
}

impl Default for DxfOptions {
    fn default() -> Self {
        Self {
            curves: DxfCurves::Splines,
            tolerance_mm: 0.005,
            true_color: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PngOptions {
    /// Puntos por pulgada.
    pub dpi: f64,
    pub margin_mm: f64,
    /// Fondo transparente en vez de blanco.
    pub transparent: bool,
    /// Grosor del trazo en mm (al menos un píxel).
    pub stroke_mm: f64,
    /// Rellena las formas cerradas de las capas en modo relleno.
    pub fill_layers: bool,
}

impl Default for PngOptions {
    fn default() -> Self {
        Self {
            dpi: 300.0,
            margin_mm: 2.0,
            transparent: false,
            stroke_mm: 0.1,
            fill_layers: true,
        }
    }
}

/// Tamaño de la hoja del PDF.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageSize {
    /// La hoja mide lo que el diseño más el margen.
    Fit,
    A4,
    A3,
    Letter,
    Legal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PdfOptions {
    pub page: PageSize,
    /// Margen en mm (alrededor del diseño con `Fit`, o del borde de la hoja).
    pub margin_mm: f64,
    pub stroke_mm: f64,
    /// Incrusta las imágenes del diseño (en gris o color, sobre fondo blanco).
    pub images: bool,
    pub fill_layers: bool,
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            page: PageSize::Fit,
            margin_mm: 5.0,
            stroke_mm: 0.1,
            images: true,
            fill_layers: false,
        }
    }
}

#[cfg(test)]
mod tests;
