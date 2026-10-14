//! Importación de archivos de diseño a objetos del documento.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Job, Rgb};
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{DesignObject, ImageObject, ImportResult, LayerSettings, ObjectKind, VectorObject};

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
    /// Trabajos de controladoras Ruida (`.rd`), una capa de Tlaku por capa del archivo.
    Rd,
    /// Trabajos de placas Lihuiyu M2 Nano (`.egv`, LHYMICRO-GL), una capa por velocidad.
    Egv,
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
        "rd" => Ok(InputKind::Rd),
        "egv" => Ok(InputKind::Egv),
        _ => Err(format!(
            "{}: formato de entrada no soportado; usa SVG, DXF, PDF, AI, HPGL, G-code, \
             LightBurn (.lbrn/.lbrn2), Ruida (.rd), K40 (.egv) o una imagen (PNG, JPG, BMP, WebP, TIFF, GIF, TGA)",
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

/// Opciones que elige el usuario en el diálogo de importación (D91). Van aparte de
/// [`ImportOptions`] para no cambiar quien ya construye esa estructura (el CLI).
#[derive(Debug, Clone, PartialEq)]
pub struct ImportExtras {
    /// Factor de escala uniforme que se aplica al final (1 = sin cambio).
    pub scale: f64,
    /// Resolución (ppp) con la que se interpretan un SVG sin unidades físicas y las imágenes.
    /// `None` = 96, que es lo que usa el resto de programas por defecto.
    pub dpi: Option<f64>,
    /// DXF sin unidades declaradas: mm que vale una unidad del dibujo (25.4 = pulgadas).
    /// `None` = se supone milímetros, con un aviso.
    pub dxf_unit_mm: Option<f64>,
    /// Une todo lo importado en un solo grupo.
    pub group: bool,
    /// Pone todos los vectores en la capa de este color en vez de una capa por color.
    pub target_layer: Option<Rgb>,
    /// PDF y AI: página a importar, desde 1.
    pub pdf_page: u32,
    /// G-code: una capa por nivel de potencia (`true`) o una sola capa (`false`).
    pub gcode_by_power: bool,
    /// G-code: valor de `S` (en las unidades del archivo) con el que se toman los avances
    /// `G1`/`G2`/`G3` que llegan con potencia 0 (`M3 S0`, o sin `S`). `None` = no se dibujan
    /// (el archivo sin una potencia mayor que 0 importa vacío y el error lo explica). Debe ser
    /// mayor que 0; solo sirve para que esos cortes cuenten como quemados y para agruparlos en
    /// una capa.
    pub gcode_default_power: Option<f64>,
}

impl Default for ImportExtras {
    fn default() -> Self {
        Self {
            scale: 1.0,
            dpi: None,
            dxf_unit_mm: None,
            group: false,
            target_layer: None,
            pdf_page: 1,
            gcode_by_power: true,
            gcode_default_power: None,
        }
    }
}

/// Comienzo del aviso del importador de DXF cuando el archivo no declara unidades.
const DXF_NO_UNITS: &str = "El archivo no declara unidades";

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
        priority: None,
        power_scale: None,
        shape: None,
        start: None,
        manual_tabs: Vec::new(),
        array: None,
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

/// Ruta que usará el objeto imagen y el aviso correspondiente. La ventana de la app no sabe
/// mostrar TIFF ni TGA, así que se convierten a PNG: junto al original (`foto.tlaku.png`) o, si
/// esa carpeta no se puede escribir, en la carpeta temporal.
fn displayable_source(path: &Path) -> Result<(std::path::PathBuf, Option<String>), String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if !matches!(ext.as_str(), "tif" | "tiff" | "tga") {
        return Ok((path.to_path_buf(), None));
    }
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("imagen");
    let beside = path.with_file_name(format!("{stem}.tlaku.png"));
    if tlaku_raster::gray::convert_to_png(path, &beside).is_ok() {
        return Ok((
            beside.clone(),
            Some(format!(
                "La ventana no muestra {} directamente: se convirtió a PNG y el diseño usa la \
                 copia {}. Conserva esa copia junto al proyecto.",
                ext.to_uppercase(),
                beside.display()
            )),
        ));
    }
    let tmp = std::env::temp_dir().join(format!("tlaku-{}-{stem}.png", new_id()));
    tlaku_raster::gray::convert_to_png(path, &tmp)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((
        tmp.clone(),
        Some(format!(
            "La ventana no muestra {} directamente y no se pudo escribir junto al original: se \
             convirtió a PNG en la carpeta temporal ({}). Si cambias de equipo o reinicias, \
             vuelve a importar la imagen.",
            ext.to_uppercase(),
            tmp.display()
        )),
    ))
}

/// Objeto imagen de `source` a `dpi`. Lee solo la cabecera: no hace falta decodificar la imagen
/// (que en 100 Mpx cuesta más de 2 GB) para saber su tamaño.
fn image_kind(source: &Path, shown_as: &Path, dpi: f64) -> Result<ObjectKind, String> {
    let (w, h) = tlaku_raster::gray::dimensions(source)
        .map_err(|e| format!("{}: {e}", shown_as.display()))?;
    let mm = |px: u32| f64::from(px) * 25.4 / dpi;
    Ok(ObjectKind::Image(ImageObject {
        source_path: source.to_string_lossy().into_owned(),
        width_px: w,
        height_px: h,
        size_mm: [mm(w), mm(h)],
        mask: Vec::new(),
    }))
}

fn import_image(path: &Path, dpi: f64) -> Result<ImportResult, String> {
    let (source, note) = displayable_source(path)?;
    let obj = object(LayerSettings::IMAGE_COLOR, image_kind(&source, path, dpi)?);
    Ok(ImportResult {
        objects: vec![obj],
        new_layers: vec![LayerSettings::for_images()],
        warnings: note.into_iter().collect(),
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
    import_file_extras(path, opts, &ImportExtras::default())
}

/// Como [`import_file_with`], más las opciones del diálogo de importación.
pub fn import_file_extras(
    path: &Path,
    opts: &ImportOptions,
    extras: &ImportExtras,
) -> Result<ImportResult, String> {
    let kind = input_kind(path)?;
    if kind == InputKind::Image {
        let image_dpi = extras.dpi.filter(|d| d.is_finite() && *d > 0.0);
        return import_image(path, image_dpi.unwrap_or(IMAGE_DPI))
            .map(|r| apply_extras(r, extras, 1.0));
    }
    let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    // Formatos que traen sus propias capas o se resuelven en su módulo.
    let own = match kind {
        InputKind::Lbrn => Some(crate::import_lbrn::import(path, &data, opts)),
        InputKind::Pdf => Some(crate::import_pdf::import(path, &data, opts, extras)),
        InputKind::Hpgl => Some(crate::import_hpgl::import(path, &data, opts, extras)),
        InputKind::Gcode => Some(crate::import_gcode::import(path, &data, opts, extras)),
        InputKind::Rd => Some(crate::import_rd::import(path, &data, opts, extras)),
        InputKind::Egv => Some(crate::import_egv::import(path, &data, opts, extras)),
        _ => None,
    };
    if let Some(result) = own {
        // Un proyecto de LightBurn trae sus propias capas: las opciones no lo tocan.
        return if kind == InputKind::Lbrn {
            result
        } else {
            result.map(|r| apply_extras(r, extras, 1.0))
        };
    }
    let mut factor = 1.0;
    let mut svg_images = Vec::new();
    let mut svg_height_mm = 0.0;
    let (layers, mut warnings) = match kind {
        InputKind::Svg => {
            let mut doc = tlaku_svg::import_svg(&data, opts.tolerance_mm)
                .map_err(|e| format!("{}: SVG inválido: {e}", path.display()))?;
            let warnings = doc.warnings.clone();
            svg_images = std::mem::take(&mut doc.images);
            svg_height_mm = doc.height_mm;
            // El DPI solo cuenta cuando el SVG no trae unidades físicas: usvg ya lo pasó a
            // 96 ppp, así que se reescala por 96 / ppp.
            if let Some(dpi) = extras.dpi.filter(|d| d.is_finite() && *d > 0.0)
                && !svg_has_physical_units(&data)
            {
                factor = IMAGE_DPI / dpi;
            }
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
            let mut warnings = doc.warnings.clone();
            if let Some(unit) = extras.dxf_unit_mm.filter(|u| u.is_finite() && *u > 0.0)
                && let Some(i) = warnings.iter().position(|w| w.starts_with(DXF_NO_UNITS))
            {
                factor = unit;
                warnings[i] = format!(
                    "El archivo no declara unidades: se interpretó cada unidad como {unit} mm \
                     por la opción de importación."
                );
            }
            (
                job_layers(doc.into_job(|_| tlaku_core::CutSettings::default())),
                warnings,
            )
        }
        InputKind::Image
        | InputKind::Pdf
        | InputKind::Hpgl
        | InputKind::Gcode
        | InputKind::Rd
        | InputKind::Egv
        | InputKind::Lbrn => unreachable!("se importan arriba"),
    };
    if layers.is_empty() {
        // Un SVG que solo trae imágenes incrustadas se importa como imagen.
        if !svg_images.is_empty() {
            let result =
                svg_images_result(path, &svg_images, svg_height_mm, opts.flip_y, warnings)?;
            return Ok(apply_extras(result, extras, factor));
        }
        return Err(empty_error(path, kind, &warnings));
    }
    let mut result = from_curves(layers, Vec::new());
    result.warnings.append(&mut warnings);
    Ok(apply_extras(result, extras, factor))
}

/// Avisos del importador que explican por qué no salió nada (lo omitido), sin los de escala ni
/// los genéricos que solo repiten que el archivo no tiene geometría.
fn cause_warnings(warnings: &[String]) -> Vec<&str> {
    const NOISE: [&str; 5] = [
        DXF_NO_UNITS,
        "El SVG no define width/height",
        "width/height sin unidades",
        "El documento no contiene trayectorias importables",
        "El archivo no contiene geometría importable",
    ];
    warnings
        .iter()
        .map(String::as_str)
        .filter(|w| !NOISE.iter().any(|n| w.starts_with(n)))
        .collect()
}

/// Error de un archivo sin trayectorias: conserva la causa que el importador ya conoce (textos,
/// rellenos y cotas omitidos, imágenes, elementos ocultos...) y dice qué hacer.
fn empty_error(path: &Path, kind: InputKind, warnings: &[String]) -> String {
    let mut msg = format!("{}: el archivo no contiene trayectorias", path.display());
    let causes = cause_warnings(warnings);
    if causes.is_empty() {
        return msg;
    }
    let body = causes.join(" ");
    msg.push_str(". ");
    msg.push_str(&body);
    let hint = if body.contains("contornos") {
        // Los avisos del propio importador ya dicen cómo arreglarlo.
        ""
    } else if body.contains("imagen") {
        " Exporta la imagen como PNG o JPG e impórtala como imagen."
    } else {
        match kind {
            InputKind::Dxf => {
                " Convierte esos elementos a contornos (líneas, polilíneas y curvas) en tu \
                 programa de CAD y vuelve a exportar."
            }
            InputKind::Svg => {
                " Convierte el texto y los objetos a trayectos (contornos) en tu editor y \
                 vuelve a guardar el SVG."
            }
            _ => "",
        }
    };
    msg.push_str(hint);
    msg
}

/// Escribe `data` junto a `original` (`<nombre>.tlaku-<tag>.<ext>`) o, si esa carpeta no se
/// puede escribir, en la carpeta temporal.
fn write_beside(
    original: &Path,
    tag: &str,
    ext: &str,
    data: &[u8],
) -> Result<std::path::PathBuf, String> {
    let stem = original
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("imagen");
    let beside = original.with_file_name(format!("{stem}.tlaku-{tag}.{ext}"));
    if std::fs::write(&beside, data).is_ok() {
        return Ok(beside);
    }
    let tmp = std::env::temp_dir().join(format!("tlaku-{}-{stem}-{tag}.{ext}", new_id()));
    std::fs::write(&tmp, data).map_err(|e| format!("{}: {e}", tmp.display()))?;
    Ok(tmp)
}

/// Un SVG sin trayectorias pero con imágenes incrustadas: cada imagen se guarda como archivo y
/// se importa como objeto imagen en su posición dentro del SVG.
fn svg_images_result(
    path: &Path,
    images: &[tlaku_svg::EmbeddedImage],
    height_mm: f64,
    flip_y: bool,
    svg_warnings: Vec<String>,
) -> Result<ImportResult, String> {
    let mut objects = Vec::new();
    let mut saved = Vec::new();
    for (n, img) in images.iter().enumerate() {
        let file = write_beside(path, &format!("img{}", n + 1), img.ext, &img.data)?;
        let mut kind = image_kind(&file, path, IMAGE_DPI)?;
        if let ObjectKind::Image(i) = &mut kind {
            i.size_mm = [img.width_mm, img.height_mm];
        }
        let y = if flip_y {
            height_mm - img.y_mm - img.height_mm
        } else {
            img.y_mm
        };
        let mut obj = object(LayerSettings::IMAGE_COLOR, kind);
        obj.transform = tlaku_doc::affine::translate(img.x_mm, y);
        objects.push(obj);
        saved.push(file.display().to_string());
    }
    let mut warnings = vec![format!(
        "El SVG no trae trayectorias, solo {} imagen(es) incrustada(s): se importó como imagen \
         (copia en {}). Para cortar o grabar contornos hace falta un SVG con trayectos.",
        images.len(),
        saved.join(", ")
    )];
    // Los avisos que hablan de imágenes omitidas o de falta de trayectorias ya no aplican.
    warnings.extend(svg_warnings.into_iter().filter(|w| {
        !w.contains("incrustada(s) omitida(s)")
            && !w.starts_with("El documento no contiene trayectorias importables")
    }));
    Ok(ImportResult {
        objects,
        new_layers: vec![LayerSettings::for_images()],
        warnings,
    })
}

/// `true` si el elemento raíz del SVG declara `width` o `height` con una unidad física
/// (mm, cm, in, pt, pc, Q). Sin unidades (o en `px`) el tamaño depende de la resolución.
fn svg_has_physical_units(data: &[u8]) -> bool {
    let head = &data[..data.len().min(8192)];
    let text = String::from_utf8_lossy(head);
    let Some(start) = text.find("<svg") else {
        return false;
    };
    let tag = &text[start..];
    let tag = &tag[..tag.find('>').unwrap_or(tag.len())];
    ["width", "height"].iter().any(|attr| {
        let mut rest = tag;
        while let Some(i) = rest.find(attr) {
            let before = rest[..i].chars().next_back();
            rest = &rest[i + attr.len()..];
            if before.is_some_and(|c| !c.is_whitespace()) {
                continue;
            }
            let after = rest.trim_start();
            let Some(after) = after.strip_prefix('=') else {
                continue;
            };
            let after = after.trim_start();
            let Some(quote) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else {
                continue;
            };
            let value = after[1..].split(quote).next().unwrap_or("").trim();
            return ["mm", "cm", "in", "pt", "pc", "q"]
                .iter()
                .any(|u| value.to_ascii_lowercase().ends_with(u));
        }
        false
    })
}

/// Escala, capa destino y grupo de las opciones del diálogo. `factor` es la escala propia del
/// formato (DPI del SVG, unidad del DXF), que se multiplica por `extras.scale`.
fn apply_extras(mut result: ImportResult, extras: &ImportExtras, factor: f64) -> ImportResult {
    let scale = factor
        * if extras.scale.is_finite() && extras.scale > 0.0 {
            extras.scale
        } else {
            1.0
        };
    if (scale - 1.0).abs() > 1e-12 {
        for obj in &mut result.objects {
            match &mut obj.kind {
                ObjectKind::Vector(v) => {
                    for p in &mut v.paths {
                        *p = p.map_points(|q| tlaku_core::Point::new(q.x * scale, q.y * scale));
                    }
                }
                ObjectKind::Image(i) => {
                    i.size_mm = [i.size_mm[0] * scale, i.size_mm[1] * scale];
                    obj.transform[4] *= scale;
                    obj.transform[5] *= scale;
                }
                ObjectKind::Text(_) => {}
            }
        }
    }
    if let Some(color) = extras.target_layer {
        let mut any = false;
        for obj in &mut result.objects {
            if matches!(obj.kind, ObjectKind::Vector(_)) {
                obj.layer = color;
                any = true;
            }
        }
        if any {
            // Solo queda la capa elegida para los vectores (más la de imágenes si la hay).
            result
                .new_layers
                .retain(|l| l.color == LayerSettings::IMAGE_COLOR);
            result.new_layers.insert(0, LayerSettings::for_color(color));
        }
    }
    if extras.group && result.objects.len() > 1 {
        let group = format!("grp-{}", new_id());
        for obj in &mut result.objects {
            obj.group = Some(group.clone());
        }
    }
    result
}

/// Arco circular de centro `(cx, cy)` y radio `r`, desde el ángulo `a0` con barrido `sweep`
/// (radianes, positivo = antihorario), como cúbicas de a lo sumo 90°. El último punto es
/// exactamente `end`, para que no se acumule error entre arcos consecutivos.
pub(crate) fn arc_to_cubics(
    (cx, cy): (f64, f64),
    r: f64,
    a0: f64,
    sweep: f64,
    end: tlaku_core::Point,
) -> Vec<tlaku_core::bezpath::PathSeg> {
    use tlaku_core::Point;
    use tlaku_core::bezpath::PathSeg;
    let n = ((sweep.abs() / std::f64::consts::FRAC_PI_2).ceil() as usize).clamp(1, 1024);
    let step = sweep / n as f64;
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    let at = |a: f64| Point::new(cx + r * a.cos(), cy + r * a.sin());
    let tangent = |a: f64| Point::new(-r * a.sin(), r * a.cos());
    (0..n)
        .map(|i| {
            let (a, b) = (a0 + step * i as f64, a0 + step * (i + 1) as f64);
            let (p0, p3) = (at(a), at(b));
            let (t0, t3) = (tangent(a), tangent(b));
            PathSeg::CubicTo {
                ctrl1: Point::new(p0.x + k * t0.x, p0.y + k * t0.y),
                ctrl2: Point::new(p3.x - k * t3.x, p3.y - k * t3.y),
                to: if i + 1 == n { end } else { p3 },
            }
        })
        .collect()
}

#[cfg(test)]
mod extras_tests {
    use super::*;
    use std::path::PathBuf;

    fn workdir(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tlaku-import-{}-{test}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    const SVG_PX: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50">
        <rect x="10" y="10" width="20" height="10" fill="none" stroke="#ff0000"/>
        <rect x="50" y="10" width="10" height="10" fill="none" stroke="#0000ff"/></svg>"##;

    #[test]
    fn webp_gif_tiff_y_tga_se_importan_como_imagen() {
        use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
        let dir = workdir("formatos");
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(8, 4, Rgba([10, 20, 30, 255])));
        for (name, format) in [
            ("a.webp", ImageFormat::WebP),
            ("a.gif", ImageFormat::Gif),
            ("a.tif", ImageFormat::Tiff),
            ("a.tiff", ImageFormat::Tiff),
            ("a.tga", ImageFormat::Tga),
        ] {
            let path = dir.join(name);
            img.save_with_format(&path, format).unwrap();
            let r = import_file(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
            let ObjectKind::Image(i) = &r.objects[0].kind else {
                panic!("{name}: debía ser imagen");
            };
            assert_eq!((i.width_px, i.height_px), (8, 4), "{name}");
            // TIFF y TGA se sirven como un PNG que la ventana sí muestra.
            let shown = Path::new(&i.source_path);
            if name.ends_with("tif") || name.ends_with("tiff") || name.ends_with("tga") {
                assert!(
                    i.source_path.ends_with(".tlaku.png"),
                    "{name}: {}",
                    i.source_path
                );
                assert!(shown.exists());
                assert!(r.warnings.iter().any(|w| w.contains("PNG")), "{name}");
            } else {
                assert_eq!(shown, dir.join(name));
                assert!(r.warnings.is_empty());
            }
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn escala_dpi_capa_destino_y_grupo() {
        let dir = workdir("extras");
        let svg = dir.join("a.svg");
        std::fs::write(&svg, SVG_PX).unwrap();
        let base = import_file(&svg).unwrap();
        assert_eq!(base.objects.len(), 2);
        let w0 = base.objects[0].bounds().unwrap().max.x;

        // 72 ppp en vez de 96: todo sale 96/72 más grande.
        let r = import_file_extras(
            &svg,
            &ImportOptions::default(),
            &ImportExtras {
                dpi: Some(72.0),
                ..Default::default()
            },
        )
        .unwrap();
        approx(r.objects[0].bounds().unwrap().max.x, w0 * 96.0 / 72.0);

        // Escala 200 %, todo a una capa y agrupado.
        let r = import_file_extras(
            &svg,
            &ImportOptions::default(),
            &ImportExtras {
                scale: 2.0,
                group: true,
                target_layer: Some(Rgb(1, 2, 3)),
                ..Default::default()
            },
        )
        .unwrap();
        approx(r.objects[0].bounds().unwrap().max.x, w0 * 2.0);
        assert!(r.objects.iter().all(|o| o.layer == Rgb(1, 2, 3)));
        assert_eq!(r.new_layers.len(), 1);
        assert_eq!(r.new_layers[0].color, Rgb(1, 2, 3));
        let g = r.objects[0].group.clone().unwrap();
        assert!(r.objects.iter().all(|o| o.group.as_deref() == Some(&g)));

        // Con unidades físicas el DPI no cuenta.
        let mm = dir.join("b.svg");
        std::fs::write(
            &mm,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="50mm" viewBox="0 0 100 50">
                <rect x="0" y="0" width="100" height="50" stroke="#000"/></svg>"##,
        )
        .unwrap();
        let r = import_file_extras(
            &mm,
            &ImportOptions::default(),
            &ImportExtras {
                dpi: Some(300.0),
                ..Default::default()
            },
        )
        .unwrap();
        approx(r.objects[0].bounds().unwrap().max.x, 100.0);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unidad_del_dxf_sin_unidades() {
        let dir = workdir("dxf");
        let dxf = dir.join("a.dxf");
        std::fs::write(
            &dxf,
            "0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n10\n21\n0\n0\nENDSEC\n0\nEOF\n",
        )
        .unwrap();
        let mm = import_file(&dxf).unwrap();
        approx(mm.objects[0].bounds().unwrap().max.x, 10.0);
        assert!(mm.warnings.iter().any(|w| w.starts_with(DXF_NO_UNITS)));
        let inch = import_file_extras(
            &dxf,
            &ImportOptions::default(),
            &ImportExtras {
                dxf_unit_mm: Some(25.4),
                ..Default::default()
            },
        )
        .unwrap();
        approx(inch.objects[0].bounds().unwrap().max.x, 254.0);
        assert!(inch.warnings.iter().any(|w| w.contains("25.4 mm")));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn archivos_de_otras_fuentes_pasan_por_el_import_general() {
        let dir = workdir("hpgl");
        let plt = dir.join("a.plt");
        std::fs::write(&plt, "IN;SP1;PU0,0;PD400,0,400,400;").unwrap();
        let r = import_file(&plt).unwrap();
        assert_eq!(r.objects.len(), 1);
        let nc = dir.join("a.nc");
        std::fs::write(&nc, "M3 S100\nG0 X0 Y0\nG1 X5 Y0\n").unwrap();
        assert_eq!(import_file(&nc).unwrap().objects.len(), 1);
        let ai = dir.join("a.ai");
        std::fs::write(&ai, "%!PS-Adobe-3.0\n").unwrap();
        assert!(import_file(&ai).unwrap_err().contains("AI antiguo"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
