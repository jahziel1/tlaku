//! Importación de archivos de diseño a objetos del documento.

use laser_core::{Job, Rgb};
use laser_doc::affine::IDENTITY;
use laser_doc::{
    DesignObject, ImageObject, ImportResult, LayerSettings, ObjectKind, PathData, VectorObject,
};
use laser_raster::Luma;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    Svg,
    Dxf,
    Image,
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
        "png" | "jpg" | "jpeg" | "bmp" => Ok(InputKind::Image),
        _ => Err(format!(
            "{}: formato de entrada no soportado; usa .svg, .dxf, .png, .jpg, .jpeg o .bmp",
            path.display()
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImportOptions {
    /// Error máximo al convertir curvas en rectas, en mm.
    pub tolerance_mm: f64,
    /// SVG: pasa a Y hacia arriba (el DXF ya lo está).
    pub flip_y: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            tolerance_mm: 0.02,
            flip_y: true,
        }
    }
}

/// Resolución con la que se interpreta una imagen sin tamaño físico conocido.
const IMAGE_DPI: f64 = 96.0;

/// Id único aunque se importe en sesiones distintas del mismo proyecto.
fn new_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    static START_MS: std::sync::OnceLock<u128> = std::sync::OnceLock::new();
    let start = START_MS.get_or_init(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis())
    });
    format!("obj-{start:x}-{}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

fn object(layer: Rgb, kind: ObjectKind) -> DesignObject {
    DesignObject {
        id: new_id(),
        layer,
        transform: IDENTITY,
        kind,
        locked: false,
        hidden: false,
    }
}

fn from_job(job: Job, warnings: Vec<String>) -> ImportResult {
    let mut objects = Vec::new();
    let mut new_layers = Vec::new();
    for layer in job.layers {
        let paths = layer
            .paths
            .iter()
            .map(|p| PathData {
                points: p.points.iter().map(|q| [q.x, q.y]).collect(),
                closed: p.closed,
            })
            .collect();
        objects.push(object(
            layer.color,
            ObjectKind::Vector(VectorObject { paths }),
        ));
        new_layers.push(LayerSettings::for_color(layer.color));
    }
    ImportResult {
        objects,
        new_layers,
        warnings,
    }
}

fn import_image(path: &Path) -> Result<ImportResult, String> {
    let gray = laser_raster::gray::load(path, Luma::default())
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
/// transform identidad; una imagen sale como un `ImageObject` de 96 ppp en la capa gris de
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
    let (job, warnings) = match kind {
        InputKind::Svg => {
            let doc = laser_svg::import_svg(&data, opts.tolerance_mm)
                .map_err(|e| format!("{}: SVG inválido: {e}", path.display()))?;
            let warnings = doc.warnings.clone();
            (
                doc.into_job(|_| laser_core::CutSettings::default(), opts.flip_y),
                warnings,
            )
        }
        InputKind::Dxf => {
            let doc = laser_dxf::import_dxf(&data, opts.tolerance_mm)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            let warnings = doc.warnings.clone();
            (
                doc.into_job(|_| laser_core::CutSettings::default()),
                warnings,
            )
        }
        InputKind::Image => unreachable!("las imágenes se importan arriba"),
    };
    if job.layers.is_empty() {
        return Err(format!(
            "{}: el archivo no contiene trayectorias",
            path.display()
        ));
    }
    Ok(from_job(job, warnings))
}
