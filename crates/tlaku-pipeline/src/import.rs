//! Importación de archivos de diseño a objetos del documento.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Job, Rgb};
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{DesignObject, ImageObject, ImportResult, LayerSettings, ObjectKind, VectorObject};
use tlaku_raster::Luma;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    Svg,
    Dxf,
    Image,
    /// PDF y AI (un AI moderno es un PDF).
    Pdf,
    /// HPGL de plotters y cortadoras (`.plt`, `.hpgl`, `.hpg`).
    Hpgl,
    /// G-code de otros programas (`.nc`, `.gcode`, `.gc`, `.ngc`), como trayectorias.
    Gcode,
    /// Proyectos de LightBurn (`.lbrn`, `.lbrn2`), con sus capas y ajustes.
    Lbrn,
}

pub fn input_kind(path: &Path) -> Result<InputKind, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "svg" => Ok(InputKind::Svg),
        "dxf" => Ok(InputKind::Dxf),
        "png" | "jpg" | "jpeg" | "bmp" | "webp" | "tif" | "tiff" | "gif" | "tga" => {
            Ok(InputKind::Image)
        }
        "pdf" | "ai" => Ok(InputKind::Pdf),
        "plt" | "hpgl" | "hpg" => Ok(InputKind::Hpgl),
        "nc" | "gcode" | "gc" | "ngc" => Ok(InputKind::Gcode),
        "lbrn" | "lbrn2" => Ok(InputKind::Lbrn),
        _ => Err(format!(
            "{}: formato de entrada no soportado; usa SVG, DXF, PDF, AI, HPGL, G-code, \
             LightBurn (.lbrn/.lbrn2) o una imagen (PNG, JPG, BMP, WebP, TIFF, GIF, TGA)",
            path.display()
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImportOptions {
    /// Error máximo al convertir curvas en rectas, en mm. Solo cuenta con
    /// `keep_curves = false`.
    pub tolerance_mm: f64,
    /// SVG: pasa a Y hacia arriba (el DXF ya lo está).
    pub flip_y: bool,
    /// Conserva las curvas Bézier del SVG (lo que usa el editor). Con `false` las aplana en
    /// el import con `tolerance_mm`: así se comporta el CLI con `--tolerance`.
    pub keep_curves: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            tolerance_mm: 0.02,
            flip_y: true,
            keep_curves: true,
        }
    }
}

/// Resolución con la que se interpreta una imagen sin tamaño físico conocido.
const IMAGE_DPI: f64 = 96.0;

/// Id único aunque se importe en sesiones distintas del mismo proyecto.
pub(crate) fn new_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    static START_MS: std::sync::OnceLock<u128> = std::sync::OnceLock::new();
    let start = START_MS.get_or_init(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis())
    });
    format!("obj-{start:x}-{}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

pub(crate) fn object(layer: Rgb, kind: ObjectKind) -> DesignObject {
    DesignObject {
        id: new_id(),
        layer,
        transform: IDENTITY,
        kind,
        locked: false,
        hidden: false,
        group: None,
    }
}

/// Las capas de un `Job` como trayectorias sin curvas.
fn job_layers(job: Job) -> Vec<(Rgb, Vec<BezPath>)> {
    job.layers
        .into_iter()
        .map(|l| (l.color, l.paths.iter().map(BezPath::from).collect()))
        .collect()
}

/// Un objeto vectorial y una capa `line` nueva por color.
pub(crate) fn from_curves(layers: Vec<(Rgb, Vec<BezPath>)>, warnings: Vec<String>) -> ImportResult {
    let mut objects = Vec::new();
    let mut new_layers = Vec::new();
    for (color, paths) in layers {
        objects.push(object(color, ObjectKind::Vector(VectorObject { paths })));
        new_layers.push(LayerSettings::for_color(color));
    }
    ImportResult {
        objects,
        new_layers,
        warnings,
    }
}

fn import_image(path: &Path) -> Result<ImportResult, String> {
    let gray = tlaku_raster::gray::load(path, Luma::default())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let mm = |px: usize| px as f64 * 25.4 / IMAGE_DPI;
    let obj = object(
        LayerSettings::IMAGE_COLOR,
        ObjectKind::Image(ImageObject {
            source_path: path.to_string_lossy().into_owned(),
            width_px: gray.width as u32,
            height_px: gray.height as u32,
            size_mm: [mm(gray.width), mm(gray.height)],
        }),
    );
    Ok(ImportResult {
        objects: vec![obj],
        new_layers: vec![LayerSettings::for_images()],
        warnings: Vec::new(),
    })
}

/// Importa un SVG, DXF o imagen con las opciones por defecto.
///
/// Los vectores salen agrupados por color (un objeto y una capa nueva `line` por color) con
/// transform identidad (el SVG conserva sus curvas Bézier; el DXF da rectas); una imagen sale como un `ImageObject` de 96 ppp en la capa gris de
/// imágenes. Un archivo sin trayectorias es un error.
pub fn import_file(path: &Path) -> Result<ImportResult, String> {
    import_file_with(path, &ImportOptions::default())
}

pub fn import_file_with(path: &Path, opts: &ImportOptions) -> Result<ImportResult, String> {
    let kind = input_kind(path)?;
    if kind == InputKind::Image {
        return import_image(path);
    }
    let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    // Formatos que traen sus propias capas o se resuelven en su módulo.
    let own = match kind {
        InputKind::Lbrn => Some(crate::import_lbrn::import(path, &data, opts)),
        InputKind::Pdf => Some(crate::import_pdf::import(path, &data, opts)),
        InputKind::Hpgl => Some(crate::import_hpgl::import(path, &data, opts)),
        InputKind::Gcode => Some(crate::import_gcode::import(path, &data, opts)),
        _ => None,
    };
    if let Some(result) = own {
        return result;
    }
    let (layers, warnings) = match kind {
        InputKind::Svg => {
            let doc = tlaku_svg::import_svg(&data, opts.tolerance_mm)
                .map_err(|e| format!("{}: SVG inválido: {e}", path.display()))?;
            let warnings = doc.warnings.clone();
            let layers = if opts.keep_curves {
                doc.into_curves(opts.flip_y)
            } else {
                job_layers(doc.into_job(|_| tlaku_core::CutSettings::default(), opts.flip_y))
            };
            (layers, warnings)
        }
        InputKind::Dxf => {
            let doc = tlaku_dxf::import_dxf(&data, opts.tolerance_mm)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            let warnings = doc.warnings.clone();
            (
                job_layers(doc.into_job(|_| tlaku_core::CutSettings::default())),
                warnings,
            )
        }
        InputKind::Image
        | InputKind::Pdf
        | InputKind::Hpgl
        | InputKind::Gcode
        | InputKind::Lbrn => unreachable!("se importan arriba"),
    };
    if layers.is_empty() {
        return Err(format!(
            "{}: el archivo no contiene trayectorias",
            path.display()
        ));
    }
    Ok(from_curves(layers, warnings))
}
