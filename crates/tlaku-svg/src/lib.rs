//! Importación de SVG en milímetros, con las trayectorias agrupadas por color para formar
//! capas. Cada trayectoria sale de dos formas: aplanada a polilíneas con una tolerancia
//! ([`SvgDocument::paths`], para el CLI y [`SvgDocument::into_job`]) y como curvas Bézier
//! exactas ([`SvgDocument::curves`], para el editor). Ambas pasan por la misma transformación
//! (`abs_transform`), los mismos colores y generan las mismas advertencias.
//!
//! # `clip-path` y `mask`
//!
//! El recorte se aplica de verdad a la geometría, sobre curvas ya aplanadas con la tolerancia de
//! la importación (`import_svg`, normalmente 0.01 mm). Lo que el recorte toca queda en segmentos
//! rectos; lo que queda entero dentro conserva sus curvas.
//!
//! - Una forma con solo relleno (sin trazo) se intersecta con la región recortada como figura:
//!   su contorno resultante incluye el borde del recorte, igual que al verla dibujada.
//! - Una trayectoria con trazo (o sin pintado) se parte en las entradas y salidas de la región;
//!   el borde del recorte no se agrega. Los subtrayectos abiertos se tratan siempre así.
//! - Se soportan `clipPathUnits` en `userSpaceOnUse` y `objectBoundingBox` (usvg ya lo convierte
//!   a unidades de usuario), transformaciones anidadas y recortes encadenados (un `clipPath` o un
//!   hijo suyo con `clip-path`). Varios grupos anidados con recorte se intersectan.
//! - Una `mask` se aproxima como recorte binario: la unión de la geometría con relleno de color
//!   cuya opacidad (por luminancia en máscaras de luminancia) llega al 50 %. Siempre se avisa que
//!   es aproximado. Si la máscara usa degradados, patrones, imágenes, trazos sin relleno o recortes
//!   internos no se puede evaluar: se ignora y se avisa.

mod clip;

use clip::{LineClip, Region};
use tlaku_core::bezpath::{BezPath, PathSeg};
use tlaku_core::{CutSettings, Job, Layer, Point, Polyline, Rgb};
use usvg::tiny_skia_path::{self, PathSegment};
use usvg::{ClipPath, Group, Mask, MaskType, Node, Paint, Transform};

/// usvg normaliza todas las unidades a píxeles CSS (96 por pulgada).
const PX_TO_MM: f64 = 25.4 / 96.0;

pub struct ImportedPath {
    pub color: Rgb,
    pub polyline: Polyline,
}

/// Un subtrayecto con sus curvas intactas (rectas, cuadráticas y cúbicas), en mm.
pub struct ImportedCurve {
    pub color: Rgb,
    pub path: BezPath,
}

/// Imagen raster incrustada en el SVG (`<image>` con datos `data:`), con su caja en mm.
#[derive(Debug, Clone)]
pub struct EmbeddedImage {
    /// Bytes del archivo de imagen tal como vienen en el SVG.
    pub data: std::sync::Arc<Vec<u8>>,
    /// Extensión del formato: `png`, `jpg`, `gif` o `webp`.
    pub ext: &'static str,
    /// Esquina superior izquierda de la caja (Y hacia abajo, como el resto del documento).
    pub x_mm: f64,
    pub y_mm: f64,
    pub width_mm: f64,
    pub height_mm: f64,
}

/// Documento importado, en coordenadas SVG (origen arriba a la izquierda, Y hacia abajo).
pub struct SvgDocument {
    pub width_mm: f64,
    pub height_mm: f64,
    /// Subtrayectos aplanados con la tolerancia pedida.
    pub paths: Vec<ImportedPath>,
    /// Los mismos subtrayectos sin aplanar, en el mismo orden de aparición.
    pub curves: Vec<ImportedCurve>,
    /// Imágenes raster incrustadas (no forman parte de `paths` ni de `curves`).
    pub images: Vec<EmbeddedImage>,
    /// Avisos de importación (escala asumida, elementos omitidos o aproximados).
    pub warnings: Vec<String>,
}

/// `tolerance_mm` es el error máximo permitido al aproximar curvas con rectas.
pub fn import_svg(data: &[u8], tolerance_mm: f64) -> Result<SvgDocument, usvg::Error> {
    let mut opt = usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    let tree = usvg::Tree::from_data(data, &opt)?;
    let mut paths = Vec::new();
    let mut curves = Vec::new();
    let mut stats = Stats::default();
    let mut out = Out {
        polylines: &mut paths,
        curves: &mut curves,
    };
    collect(tree.root(), tolerance_mm, None, &mut out, &mut stats);
    let width_mm = tree.size().width() as f64 * PX_TO_MM;
    let height_mm = tree.size().height() as f64 * PX_TO_MM;
    let warnings = build_warnings(data, width_mm, height_mm, paths.is_empty(), &stats);
    Ok(SvgDocument {
        width_mm,
        height_mm,
        paths,
        curves,
        images: stats.embedded,
        warnings,
    })
}

/// Conteos de lo que el recorrido del árbol de usvg omite o aproxima.
#[derive(Default)]
struct Stats {
    images: usize,
    /// Las imágenes raster que se pudieron extraer (no SVG anidados).
    embedded: Vec<EmbeddedImage>,
    /// Máscaras aproximadas como recorte binario.
    mask_approx: usize,
    /// Máscaras que no se pudieron evaluar y se ignoraron.
    mask_unsupported: usize,
    /// Trayectorias cuyo color de capa se tomó de un degradado o patrón.
    non_color_paints: usize,
    /// Trayectorias con `visibility` oculta.
    invisible_paths: usize,
    /// Nodos de texto que produjeron al menos una trayectoria.
    text_with_paths: usize,
    /// Nodos de texto que no produjeron ninguna.
    text_empty: usize,
}

/// Información del XML original que usvg normaliza o descarta.
#[derive(Default)]
struct XmlInfo {
    width: Option<String>,
    height: Option<String>,
    has_viewbox: bool,
    illustrator: bool,
    /// Elementos con `display:none` (sin contar los anidados en otro oculto).
    hidden_elements: usize,
    /// Elementos `<text>` visibles fuera de definiciones.
    text_elements: usize,
}

fn decode_xml(data: &[u8]) -> Option<String> {
    let raw = if data.starts_with(&[0x1f, 0x8b]) {
        usvg::decompress_svgz(data).ok()?
    } else {
        data.to_vec()
    };
    String::from_utf8(raw).ok()
}

fn is_display_none(node: usvg::roxmltree::Node) -> bool {
    node.attribute("display").map(str::trim) == Some("none")
        || node.attribute("style").is_some_and(|s| {
            s.split(';').any(|d| {
                let mut kv = d.splitn(2, ':');
                kv.next().map(str::trim) == Some("display")
                    && kv.next().map(str::trim) == Some("none")
            })
        })
}

fn scan_xml(data: &[u8]) -> XmlInfo {
    let mut info = XmlInfo::default();
    let Some(text) = decode_xml(data) else {
        return info;
    };
    info.illustrator = text.contains("Adobe Illustrator") || text.contains("AdobeIllustrator");
    let opts = usvg::roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let Ok(doc) = usvg::roxmltree::Document::parse_with_options(&text, opts) else {
        return info;
    };
    let root = doc.root_element();
    info.width = root.attribute("width").map(str::to_owned);
    info.height = root.attribute("height").map(str::to_owned);
    info.has_viewbox = root.has_attribute("viewBox");
    for node in root.descendants().filter(|n| n.is_element()) {
        let hidden_self = is_display_none(node);
        let hidden_above = node.ancestors().skip(1).any(is_display_none);
        if hidden_self && !hidden_above {
            info.hidden_elements += 1;
        }
        let in_defs = node.ancestors().skip(1).any(|a| {
            matches!(
                a.tag_name().name(),
                "defs" | "clipPath" | "mask" | "pattern" | "symbol" | "marker"
            )
        });
        if node.tag_name().name() == "text" && !hidden_self && !hidden_above && !in_defs {
            info.text_elements += 1;
        }
    }
    info
}

#[derive(PartialEq)]
enum Dim {
    Missing,
    /// Sin unidad o en px: usvg asume 96 DPI.
    Pixels,
    /// Unidad física absoluta (mm, cm, in, pt, pc).
    Physical,
    /// Porcentaje o unidad relativa a la fuente: no define un tamaño absoluto.
    Relative,
}

fn classify_dim(value: Option<&str>) -> Dim {
    let Some(value) = value.map(str::trim).filter(|v| !v.is_empty()) else {
        return Dim::Missing;
    };
    if value.parse::<f64>().is_ok() {
        return Dim::Pixels;
    }
    let unit = value.trim_start_matches(|c: char| c.is_ascii_digit() || "+-.".contains(c));
    match unit.trim().to_ascii_lowercase().as_str() {
        "" | "px" => Dim::Pixels,
        "mm" | "cm" | "in" | "pt" | "pc" | "q" => Dim::Physical,
        _ => Dim::Relative,
    }
}

fn build_warnings(
    data: &[u8],
    width_mm: f64,
    height_mm: f64,
    no_paths: bool,
    stats: &Stats,
) -> Vec<String> {
    let xml = scan_xml(data);
    let mut w = Vec::new();
    let size = format!("{width_mm:.1} x {height_mm:.1} mm");

    // Escala: width/height originales (usvg ya los convirtió a px).
    let dims = [
        classify_dim(xml.width.as_deref()),
        classify_dim(xml.height.as_deref()),
    ];
    if dims
        .iter()
        .any(|d| matches!(d, Dim::Missing | Dim::Relative))
    {
        let origin = if xml.has_viewbox {
            "del viewBox"
        } else {
            "del contenido"
        };
        w.push(format!(
            "El SVG no define width/height absolutos (faltan o están en %, em o ex): \
             el tamaño se tomó {origin} a 96 DPI, resultado {size}. Verifica la escala."
        ));
    }
    if dims.contains(&Dim::Pixels) {
        let true_mm = (width_mm * 4.0 / 3.0, height_mm * 4.0 / 3.0);
        let extra = if xml.illustrator {
            format!(
                "Detectado Adobe Illustrator: suele exportar a 72 DPI, así que el tamaño real \
                 sería {:.1} x {:.1} mm (133 % del mostrado); escala al 133 % para corregirlo.",
                true_mm.0, true_mm.1
            )
        } else {
            format!(
                "Si viene de Illustrator (72 DPI) el tamaño real sería {:.1} x {:.1} mm \
                 (133 % del mostrado); si en cambio se ve a 133 % en otro programa, escala al 75 %.",
                true_mm.0, true_mm.1
            )
        };
        w.push(format!(
            "width/height sin unidades o en px: se interpretó a 96 DPI, tamaño resultante {size}. {extra}"
        ));
    }

    if stats.images > 0 {
        w.push(format!(
            "{} imagen(es) incrustada(s) omitida(s): el importador no mezcla imágenes con \
             trayectorias; importa la imagen como archivo aparte (PNG o JPG) para grabarla.",
            stats.images
        ));
    }
    if stats.mask_approx > 0 {
        w.push(format!(
            "{} grupo(s) con máscara (mask): se aplicó como recorte binario de su geometría \
             opaca (aproximación; se ignora la transparencia gradual).",
            stats.mask_approx
        ));
    }
    if stats.mask_unsupported > 0 {
        w.push(format!(
            "{} grupo(s) con máscara que no se pudo evaluar (degradados, patrones, imágenes, \
             trazos o recortes dentro de la máscara): se ignoró la máscara y las trayectorias \
             se importan completas.",
            stats.mask_unsupported
        ));
    }
    if stats.non_color_paints > 0 {
        w.push(format!(
            "{} trayectoria(s) con degradado o patrón en relleno o trazo: se usó negro como color de capa.",
            stats.non_color_paints
        ));
    }
    let missing_text = xml.text_elements.saturating_sub(stats.text_with_paths);
    if missing_text > 0 || stats.text_empty > 0 {
        w.push(format!(
            "{} texto(s) sin trayectorias: no se encontró una fuente disponible, convierte el \
             texto a curvas antes de exportar.",
            missing_text.max(stats.text_empty)
        ));
    }
    if stats.invisible_paths > 0 {
        w.push(format!(
            "{} trayectoria(s) con visibility oculta omitida(s).",
            stats.invisible_paths
        ));
    }
    if xml.hidden_elements > 0 {
        w.push(format!(
            "{} elemento(s) con display:none omitido(s).",
            xml.hidden_elements
        ));
    }
    if no_paths {
        w.push(
            "El documento no contiene trayectorias importables: revisa si el contenido es \
             solo texto, imágenes o elementos ocultos."
                .into(),
        );
    }
    w
}

impl SvgDocument {
    /// Las curvas agrupadas por color (en el orden en que aparece cada uno). Con `flip_y` se
    /// pasa a coordenadas de máquina (Y hacia arriba), igual que [`into_job`](Self::into_job).
    pub fn into_curves(self, flip_y: bool) -> Vec<(Rgb, Vec<BezPath>)> {
        let height = self.height_mm;
        let mut out: Vec<(Rgb, Vec<BezPath>)> = Vec::new();
        for ImportedCurve { color, path } in self.curves {
            let path = if flip_y {
                path.map_points(|p| Point::new(p.x, height - p.y))
            } else {
                path
            };
            match out.iter_mut().find(|(c, _)| *c == color) {
                Some((_, paths)) => paths.push(path),
                None => out.push((color, vec![path])),
            }
        }
        out
    }

    /// Crea un trabajo con una capa por color, en el orden en que aparecen.
    /// Con `flip_y` se pasa a coordenadas de máquina (Y hacia arriba).
    pub fn into_job(self, settings_for: impl Fn(Rgb) -> CutSettings, flip_y: bool) -> Job {
        let height = self.height_mm;
        let mut job = Job::default();
        for ImportedPath {
            color,
            mut polyline,
        } in self.paths
        {
            if flip_y {
                for p in &mut polyline.points {
                    p.y = height - p.y;
                }
            }
            match job.layers.iter_mut().find(|l| l.color == color) {
                Some(layer) => layer.paths.push(polyline),
                None => job.layers.push(Layer {
                    color,
                    settings: settings_for(color),
                    paths: vec![polyline],
                }),
            }
        }
        job
    }
}

/// Destinos del recorrido del árbol.
struct Out<'a> {
    polylines: &'a mut Vec<ImportedPath>,
    curves: &'a mut Vec<ImportedCurve>,
}

/// La imagen raster de un nodo `<image>` con su caja en mm; `None` si es un SVG anidado.
fn embedded_image(img: &usvg::Image) -> Option<EmbeddedImage> {
    let (data, ext) = match img.kind() {
        usvg::ImageKind::PNG(d) => (d, "png"),
        usvg::ImageKind::JPEG(d) => (d, "jpg"),
        usvg::ImageKind::GIF(d) => (d, "gif"),
        usvg::ImageKind::WEBP(d) => (d, "webp"),
        usvg::ImageKind::SVG(_) => return None,
    };
    // `abs_bounding_box` de un `<image>` no incluye bien el escalado de su caja; se calcula a
    // partir de la transformación absoluta, que sí lo trae (ya con el `preserveAspectRatio`).
    let b = img.bounding_box().transform(img.abs_transform())?;
    let (w, h) = (
        f64::from(b.width()) * PX_TO_MM,
        f64::from(b.height()) * PX_TO_MM,
    );
    (w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0).then(|| EmbeddedImage {
        data: data.clone(),
        ext,
        x_mm: f64::from(b.x()) * PX_TO_MM,
        y_mm: f64::from(b.y()) * PX_TO_MM,
        width_mm: w,
        height_mm: h,
    })
}

fn collect(group: &Group, tolerance: f64, clip: Option<&Region>, out: &mut Out, stats: &mut Stats) {
    for node in group.children() {
        match node {
            Node::Group(g) => {
                let local = group_region(g, tolerance, clip, stats);
                collect(g, tolerance, local.as_ref().or(clip), out, stats);
            }
            Node::Text(t) => {
                let before = out.polylines.len();
                collect(t.flattened(), tolerance, clip, out, stats);
                if out.polylines.len() > before {
                    stats.text_with_paths += 1;
                } else {
                    stats.text_empty += 1;
                }
            }
            Node::Image(img) => {
                stats.images += 1;
                if let Some(e) = embedded_image(img) {
                    stats.embedded.push(e);
                }
            }
            Node::Path(p) if !p.is_visible() => stats.invisible_paths += 1,
            Node::Path(p) => {
                let Some(data) = p.data().clone().transform(p.abs_transform()) else {
                    continue;
                };
                let color = path_color(p);
                if !has_solid_paint(p) {
                    stats.non_color_paints += 1;
                }
                match clip {
                    None => {
                        for polyline in flatten(&data, tolerance) {
                            out.polylines.push(ImportedPath { color, polyline });
                        }
                        for path in curves(&data) {
                            out.curves.push(ImportedCurve { color, path });
                        }
                    }
                    Some(region) => clip_path(p, &data, color, tolerance, region, out),
                }
            }
        }
    }
}

/// La región activa dentro de `g`: la heredada intersectada con su `clip-path` y su `mask`.
/// `None` si el grupo no agrega ninguna.
fn group_region(
    g: &Group,
    tolerance: f64,
    inherited: Option<&Region>,
    stats: &mut Stats,
) -> Option<Region> {
    let mut region: Option<Region> = None;
    let add = |region: &mut Option<Region>, r: Region| {
        *region = Some(match region.take().or_else(|| inherited.cloned()) {
            Some(prev) => clip::intersect(&prev, &r),
            None => r,
        });
    };
    if let Some(c) = g.clip_path() {
        add(&mut region, clip_region(c, g.abs_transform(), tolerance));
    }
    if let Some(m) = g.mask() {
        match mask_region(m, g.abs_transform(), tolerance) {
            Some(r) => {
                stats.mask_approx += 1;
                add(&mut region, r);
            }
            None => stats.mask_unsupported += 1,
        }
    }
    region
}

/// Los anillos de un camino (ya transformado a px) en mm, como región.
fn path_region(data: &tiny_skia_path::Path, ts: Transform, tolerance: f64) -> Region {
    match data.clone().transform(ts) {
        Some(d) => clip::region_of(&flatten(&d, tolerance)),
        None => Vec::new(),
    }
}

/// Región de un `clipPath`. `group_abs` es la transformación absoluta del grupo recortado; el
/// recorte se define en su sistema de usuario más la transformación propia del `clipPath`.
fn clip_region(clip: &ClipPath, group_abs: Transform, tolerance: f64) -> Region {
    let base = group_abs.pre_concat(clip.transform());
    let mut region = clip_children(clip.root(), base, tolerance);
    if let Some(next) = clip.clip_path() {
        region = clip::intersect(&region, &clip_region(next, group_abs, tolerance));
    }
    region
}

/// Unión de los hijos de un `clipPath` (cada hijo recortado por su propio `clip-path`).
fn clip_children(group: &Group, base: Transform, tolerance: f64) -> Region {
    let mut acc: Region = Vec::new();
    for node in group.children() {
        let part = match node {
            Node::Path(p) => path_region(p.data(), base.pre_concat(p.abs_transform()), tolerance),
            Node::Group(g) => {
                let mut sub = clip_children(g, base, tolerance);
                if let Some(c) = g.clip_path() {
                    let abs = base.pre_concat(g.abs_transform());
                    sub = clip::intersect(&sub, &clip_region(c, abs, tolerance));
                }
                sub
            }
            Node::Text(t) => clip_children(t.flattened(), base, tolerance),
            Node::Image(_) => continue,
        };
        acc = clip::union(&acc, &part);
    }
    acc
}

/// Región de una `mask` como recorte binario, o `None` si no se puede evaluar.
fn mask_region(mask: &Mask, group_abs: Transform, tolerance: f64) -> Option<Region> {
    let mut supported = true;
    let mut region: Region = Vec::new();
    mask_children(
        mask.root(),
        group_abs,
        1.0,
        mask.kind(),
        tolerance,
        &mut region,
        &mut supported,
    );
    if !supported {
        return None;
    }
    // La máscara solo actúa dentro de su rectángulo.
    let r = mask.rect();
    let corners = [
        (r.left(), r.top()),
        (r.right(), r.top()),
        (r.right(), r.bottom()),
        (r.left(), r.bottom()),
    ];
    let rect = Polyline {
        points: corners
            .iter()
            .map(|&(x, y)| {
                let mut p = tiny_skia_path::Point::from_xy(x, y);
                group_abs.map_point(&mut p);
                mm(p)
            })
            .collect(),
        closed: true,
    };
    region = clip::intersect(&region, &vec![rect]);
    if let Some(next) = mask.mask() {
        region = clip::intersect(&region, &mask_region(next, group_abs, tolerance)?);
    }
    Some(region)
}

/// Acumula la geometría opaca de la máscara en el orden de pintado: lo claro suma y, en máscaras
/// de luminancia, lo oscuro opaco resta (queda pintado encima).
fn mask_children(
    group: &Group,
    base: Transform,
    opacity: f32,
    kind: MaskType,
    tolerance: f64,
    region: &mut Region,
    supported: &mut bool,
) {
    for node in group.children() {
        match node {
            Node::Path(p) => {
                let Some(fill) = p.fill() else {
                    // Un trazo sin relleno no se evalúa (su contorno no se calcula).
                    *supported &= p.stroke().is_none();
                    continue;
                };
                let Paint::Color(c) = fill.paint() else {
                    *supported = false;
                    continue;
                };
                let alpha = opacity * fill.opacity().get();
                let lum =
                    (0.2125 * c.red as f32 + 0.7154 * c.green as f32 + 0.0721 * c.blue as f32)
                        / 255.0;
                let coverage = match kind {
                    MaskType::Luminance => lum * alpha,
                    MaskType::Alpha => alpha,
                };
                let shape = path_region(p.data(), base.pre_concat(p.abs_transform()), tolerance);
                if coverage >= 0.5 {
                    *region = clip::union(region, &shape);
                } else if kind == MaskType::Luminance && alpha >= 0.5 {
                    *region = clip::subtract(region, &shape);
                }
            }
            Node::Group(g) => {
                if g.clip_path().is_some() || g.mask().is_some() {
                    *supported = false;
                    continue;
                }
                let o = opacity * g.opacity().get();
                mask_children(g, base, o, kind, tolerance, region, supported);
            }
            Node::Text(t) => mask_children(
                t.flattened(),
                base,
                opacity,
                kind,
                tolerance,
                region,
                supported,
            ),
            Node::Image(_) => *supported = false,
        }
    }
}

fn push_clipped(out: &mut Out, color: Rgb, polyline: Polyline, curve: BezPath) {
    out.polylines.push(ImportedPath { color, polyline });
    out.curves.push(ImportedCurve { color, path: curve });
}

/// Recorta una trayectoria contra la región activa y agrega lo que queda a la salida. Ver la
/// documentación del módulo para la regla de relleno frente a trazo.
fn clip_path(
    p: &usvg::Path,
    data: &tiny_skia_path::Path,
    color: Rgb,
    tolerance: f64,
    region: &Region,
    out: &mut Out,
) {
    let flat = flatten(data, tolerance);
    let beziers = curves(data);
    // Si aplanado y curvas coinciden uno a uno, lo que no se toca conserva sus curvas.
    let curve_of = |i: usize, poly: &Polyline| match beziers.get(i) {
        Some(c) if flat.len() == beziers.len() => c.clone(),
        _ => BezPath::from(poly),
    };
    let as_region = p.stroke().is_none() && p.fill().is_some();
    let mut line_idx: Vec<usize> = (0..flat.len()).collect();

    if as_region {
        line_idx.retain(|&i| !flat[i].closed);
        let closed: Vec<usize> = (0..flat.len()).filter(|&i| flat[i].closed).collect();
        if !closed.is_empty() {
            let polys: Vec<Polyline> = closed.iter().map(|&i| flat[i].clone()).collect();
            let whole = !clip::has_holes(region)
                && polys
                    .iter()
                    .all(|l| matches!(clip::clip_polyline(l, region), LineClip::Untouched));
            if whole {
                for &i in &closed {
                    push_clipped(out, color, flat[i].clone(), curve_of(i, &flat[i]));
                }
            } else {
                for r in tlaku_geom::boolean(&polys, region, tlaku_geom::BoolOp::Intersection) {
                    let curve = BezPath::from(&r);
                    push_clipped(out, color, r, curve);
                }
            }
        }
    }

    for i in line_idx {
        match clip::clip_polyline(&flat[i], region) {
            LineClip::Untouched => {
                push_clipped(out, color, flat[i].clone(), curve_of(i, &flat[i]));
            }
            LineClip::Pieces(pieces) => {
                for piece in pieces {
                    let curve = BezPath::from(&piece);
                    push_clipped(out, color, piece, curve);
                }
            }
        }
    }
}

/// Indica si el pintado que define la capa es un color sólido (o no hay pintado).
fn has_solid_paint(p: &usvg::Path) -> bool {
    let paint = p
        .stroke()
        .map(|s| s.paint())
        .or_else(|| p.fill().map(|f| f.paint()));
    matches!(paint, Some(Paint::Color(_)) | None)
}

/// El color del trazo define la capa; si no hay trazo se usa el relleno.
fn path_color(p: &usvg::Path) -> Rgb {
    let paint = p
        .stroke()
        .map(|s| s.paint())
        .or_else(|| p.fill().map(|f| f.paint()));
    match paint {
        Some(Paint::Color(c)) => Rgb(c.red, c.green, c.blue),
        _ => Rgb(0, 0, 0),
    }
}

fn mm(p: tiny_skia_path::Point) -> Point {
    Point::new(p.x as f64 * PX_TO_MM, p.y as f64 * PX_TO_MM)
}

/// Los subtrayectos del camino como curvas (mm). Un subtrayecto sin trazo se descarta; tras
/// un `Close`, lo que siga empieza otro subtrayecto en el punto inicial.
fn curves(path: &tiny_skia_path::Path) -> Vec<BezPath> {
    let mut result = Vec::new();
    let mut segs: Vec<PathSeg> = Vec::new();
    let mut start: Option<Point> = None;
    let mut drawn = false;
    let flush = |segs: &mut Vec<PathSeg>, drawn: &mut bool, result: &mut Vec<BezPath>| {
        let segs = std::mem::take(segs);
        if std::mem::take(drawn) {
            result.push(BezPath(segs));
        }
    };
    for seg in path.segments() {
        if !matches!(seg, PathSegment::MoveTo(_)) && segs.is_empty() {
            segs.extend(start.map(|to| PathSeg::MoveTo { to }));
        }
        match seg {
            PathSegment::MoveTo(p) => {
                flush(&mut segs, &mut drawn, &mut result);
                start = Some(mm(p));
                segs.push(PathSeg::MoveTo { to: mm(p) });
            }
            PathSegment::LineTo(p) => {
                drawn = true;
                segs.push(PathSeg::LineTo { to: mm(p) });
            }
            PathSegment::QuadTo(c, p) => {
                drawn = true;
                segs.push(PathSeg::QuadTo {
                    ctrl: mm(c),
                    to: mm(p),
                });
            }
            PathSegment::CubicTo(c1, c2, p) => {
                drawn = true;
                segs.push(PathSeg::CubicTo {
                    ctrl1: mm(c1),
                    ctrl2: mm(c2),
                    to: mm(p),
                });
            }
            PathSegment::Close => {
                segs.push(PathSeg::Close);
                flush(&mut segs, &mut drawn, &mut result);
            }
        }
    }
    flush(&mut segs, &mut drawn, &mut result);
    result
}

/// Número de segmentos según la fórmula de Wang para no exceder la tolerancia.
fn segments_for(max_second_diff: f64, degree_factor: f64, tolerance: f64) -> usize {
    ((degree_factor * max_second_diff / tolerance).sqrt().ceil() as usize).clamp(1, 1000)
}

fn flatten(path: &tiny_skia_path::Path, tolerance: f64) -> Vec<Polyline> {
    let mut result = Vec::new();
    let mut current: Vec<Point> = Vec::new();

    let mut finish = |points: &mut Vec<Point>, closed: bool| {
        let mut pts = std::mem::take(points);
        pts.dedup_by(|a, b| a.dist(*b) < 1e-6);
        let mut closed = closed;
        if pts.len() > 2 && pts[0].dist(pts[pts.len() - 1]) < 1e-6 {
            pts.pop();
            closed = true;
        }
        if pts.len() >= 2 {
            result.push(Polyline {
                points: pts,
                closed,
            });
        }
    };

    for seg in path.segments() {
        match seg {
            PathSegment::MoveTo(p) => {
                finish(&mut current, false);
                current.push(mm(p));
            }
            PathSegment::LineTo(p) => current.push(mm(p)),
            PathSegment::QuadTo(c, p) => {
                let (p0, c, p1) = (*current.last().unwrap(), mm(c), mm(p));
                let dd = Point::new(p0.x - 2.0 * c.x + p1.x, p0.y - 2.0 * c.y + p1.y);
                let n = segments_for(dd.x.hypot(dd.y), 0.25, tolerance);
                for i in 1..=n {
                    let t = i as f64 / n as f64;
                    let u = 1.0 - t;
                    current.push(Point::new(
                        u * u * p0.x + 2.0 * u * t * c.x + t * t * p1.x,
                        u * u * p0.y + 2.0 * u * t * c.y + t * t * p1.y,
                    ));
                }
            }
            PathSegment::CubicTo(c1, c2, p) => {
                let (p0, c1, c2, p1) = (*current.last().unwrap(), mm(c1), mm(c2), mm(p));
                let d1 = (p0.x - 2.0 * c1.x + c2.x).hypot(p0.y - 2.0 * c1.y + c2.y);
                let d2 = (c1.x - 2.0 * c2.x + p1.x).hypot(c1.y - 2.0 * c2.y + p1.y);
                let n = segments_for(d1.max(d2), 0.75, tolerance);
                for i in 1..=n {
                    let t = i as f64 / n as f64;
                    let u = 1.0 - t;
                    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    current.push(Point::new(
                        a * p0.x + b * c1.x + c * c2.x + d * p1.x,
                        a * p0.y + b * c1.y + c * c2.y + d * p1.y,
                    ));
                }
            }
            PathSegment::Close => {
                let start = current.first().copied();
                finish(&mut current, true);
                // Tras cerrar, el siguiente segmento continúa desde el inicio del subtrayecto.
                current.extend(start);
            }
        }
    }
    finish(&mut current, false);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_keep_beziers_and_match_the_flattened_paths() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="50mm" viewBox="0 0 100 50">
            <path d="M10 10 C 10 30 40 30 40 10 Z M60 10 L 80 10 Q 90 20 80 30" fill="none" stroke="#ff0000"/>
            <rect x="5" y="5" width="4" height="4" fill="none" stroke="#0000ff"/></svg>"##;
        let doc = import_svg(svg, 0.01).unwrap();
        assert_eq!(doc.paths.len(), 3);
        assert_eq!(doc.curves.len(), 3);
        // La cúbica sigue siendo cúbica y la cuadrática también.
        assert!(
            doc.curves[0]
                .path
                .0
                .iter()
                .any(|s| matches!(s, PathSeg::CubicTo { .. }))
        );
        assert!(matches!(doc.curves[0].path.0.last(), Some(PathSeg::Close)));
        assert!(
            doc.curves[1]
                .path
                .0
                .iter()
                .any(|s| matches!(s, PathSeg::QuadTo { .. }))
        );
        let flat = doc.curves[0].path.flatten(0.01);
        assert_eq!(flat.len(), 1);
        assert_eq!(flat[0].closed, doc.paths[0].polyline.closed);
        // Misma transformación, color y volteo que el camino aplanado.
        let grouped = doc.into_curves(true);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].0, Rgb(255, 0, 0));
        assert_eq!(grouped[0].1.len(), 2);
        let b = grouped[0].1[0].bounds().unwrap();
        assert!(
            b.min.y > 50.0 - 30.0 - 1e-6 && b.max.y < 50.0 - 10.0 + 1e-6,
            "{b:?}"
        );
    }

    #[test]
    fn rect_in_mm_is_closed() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="50mm" viewBox="0 0 100 50">
            <rect x="10" y="10" width="20" height="5" fill="none" stroke="#ff0000"/></svg>"##;
        let doc = import_svg(svg, 0.01).unwrap();
        assert!((doc.width_mm - 100.0).abs() < 1e-3);
        assert_eq!(doc.paths.len(), 1);
        let p = &doc.paths[0];
        assert_eq!(p.color, Rgb(255, 0, 0));
        assert!(p.polyline.closed);
        assert_eq!(p.polyline.points.len(), 4);
        assert!((p.polyline.points[0].x - 10.0).abs() < 1e-3);
    }

    #[test]
    fn circle_respects_tolerance() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100">
            <circle cx="50" cy="50" r="40" fill="none" stroke="black"/></svg>"##;
        let doc = import_svg(svg, 0.05).unwrap();
        let poly = &doc.paths[0].polyline;
        assert!(poly.closed);
        for p in &poly.points {
            let r = (p.x - 50.0).hypot(p.y - 50.0);
            assert!((r - 40.0).abs() < 0.05, "radio {r}");
        }
    }

    #[test]
    fn flip_and_group_by_color() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100">
            <line x1="0" y1="10" x2="10" y2="10" stroke="#0000ff"/>
            <line x1="0" y1="20" x2="10" y2="20" stroke="#ff0000"/>
            <line x1="0" y1="30" x2="10" y2="30" stroke="#0000ff"/></svg>"##;
        let job = import_svg(svg, 0.05)
            .unwrap()
            .into_job(|_| CutSettings::default(), true);
        assert_eq!(job.layers.len(), 2);
        assert_eq!(job.layers[0].color, Rgb(0, 0, 255));
        assert_eq!(job.layers[0].paths.len(), 2);
        assert!((job.layers[0].paths[0].points[0].y - 90.0).abs() < 1e-3);
    }

    fn warnings_of(svg: &str) -> Vec<String> {
        import_svg(svg.as_bytes(), 0.05).unwrap().warnings
    }

    fn any_has(w: &[String], needle: &str) -> bool {
        w.iter().any(|m| m.contains(needle))
    }

    const MM_HEAD: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100">"##;
    const LINE: &str = r##"<line x1="0" y1="10" x2="50" y2="10" stroke="#ff0000"/>"##;

    #[test]
    fn clean_mm_svg_has_no_warnings() {
        let w = warnings_of(&format!("{MM_HEAD}{LINE}</svg>"));
        assert!(w.is_empty(), "{w:?}");
    }

    #[test]
    fn unitless_size_warns_96_dpi() {
        let w = warnings_of(&format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="288" height="144">{LINE}</svg>"##
        ));
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(
            w[0].contains("96 DPI") && w[0].contains("76.2 x 38.1 mm"),
            "{}",
            w[0]
        );
        assert!(w[0].contains("133 %") && w[0].contains("75 %"));
        assert!(!w[0].contains("Detectado"));
    }

    #[test]
    fn px_size_warns() {
        let w = warnings_of(&format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="96px" height="96px">{LINE}</svg>"##
        ));
        assert!(
            any_has(&w, "96 DPI") && any_has(&w, "25.4 x 25.4 mm"),
            "{w:?}"
        );
    }

    #[test]
    fn illustrator_warning_is_explicit() {
        let w = warnings_of(&format!(
            r##"<?xml version="1.0"?><!-- Generator: Adobe Illustrator 27.0, SVG Export Plug-In -->
            <svg xmlns="http://www.w3.org/2000/svg" width="72px" height="72px" viewBox="0 0 72 72">{LINE}</svg>"##
        ));
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].contains("Detectado Adobe Illustrator") && w[0].contains("133 %"));
        assert!(w[0].contains("19.1 x 19.1 mm"), "{}", w[0]);
    }

    #[test]
    fn physical_units_do_not_warn() {
        for (w, h) in [("2in", "1in"), ("10cm", "5cm"), ("72pt", "72pt")] {
            let svg = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 10 10">{LINE}</svg>"##
            );
            assert!(warnings_of(&svg).is_empty(), "{w} x {h}");
        }
    }

    #[test]
    fn viewbox_only_warns() {
        let w = warnings_of(&format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50">{LINE}</svg>"##
        ));
        assert!(
            any_has(&w, "no define width/height") && any_has(&w, "viewBox"),
            "{w:?}"
        );
        assert!(any_has(&w, "26.5 x 13.2 mm"), "{w:?}");
    }

    #[test]
    fn embedded_image_warns() {
        // PNG de 1x1 píxel.
        let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
        let w = warnings_of(&format!(
            r##"{MM_HEAD}{LINE}<image x="0" y="0" width="10" height="10" href="data:image/png;base64,{png}"/></svg>"##
        ));
        assert!(any_has(&w, "1 imagen"), "{w:?}");
    }

    #[test]
    fn embedded_image_is_extracted_with_its_box() {
        let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="40mm" height="30mm" viewBox="0 0 40 30"><image x="5" y="3" width="10" height="8" preserveAspectRatio="none" href="data:image/png;base64,{png}"/></svg>"##
        );
        let doc = import_svg(svg.as_bytes(), 0.01).unwrap();
        assert!(doc.paths.is_empty());
        assert_eq!(doc.images.len(), 1);
        let img = &doc.images[0];
        assert_eq!(img.ext, "png");
        assert!(img.data.starts_with(&[0x89, b'P', b'N', b'G']));
        for (got, want) in [
            (img.x_mm, 5.0),
            (img.y_mm, 3.0),
            (img.width_mm, 10.0),
            (img.height_mm, 8.0),
        ] {
            assert!((got - want).abs() < 1e-3, "{got} != {want}");
        }
    }

    fn area(p: &Polyline) -> f64 {
        let n = p.points.len();
        (0..n)
            .map(|i| {
                let (a, b) = (p.points[i], p.points[(i + 1) % n]);
                a.x * b.y - b.x * a.y
            })
            .sum::<f64>()
            / 2.0
    }

    /// Suma de áreas con signo (los agujeros salen de las orientaciones opuestas).
    fn net_area(doc: &SvgDocument) -> f64 {
        doc.paths
            .iter()
            .filter(|p| p.polyline.closed)
            .map(|p| area(&p.polyline))
            .sum()
    }

    fn clipped(body: &str) -> SvgDocument {
        import_svg(format!("{MM_HEAD}{body}</svg>").as_bytes(), 0.01).unwrap()
    }

    fn no_clip_warning(doc: &SvgDocument) {
        assert!(
            !any_has(&doc.warnings, "clip-path") && !any_has(&doc.warnings, "máscara"),
            "{:?}",
            doc.warnings
        );
    }

    fn x_range(doc: &SvgDocument) -> (f64, f64) {
        let xs = doc
            .paths
            .iter()
            .flat_map(|p| p.polyline.points.iter().map(|q| q.x));
        (
            xs.clone().fold(f64::MAX, f64::min),
            xs.fold(f64::MIN, f64::max),
        )
    }

    #[test]
    fn clip_path_cuts_a_filled_rect_with_a_circle() {
        let doc = clipped(
            r##"<defs><clipPath id="c"><circle cx="20" cy="20" r="10"/></clipPath></defs>
            <rect x="0" y="0" width="40" height="40" fill="#ff0000" clip-path="url(#c)"/>"##,
        );
        no_clip_warning(&doc);
        assert_eq!(doc.paths.len(), 1);
        assert_eq!(doc.curves.len(), 1);
        let p = &doc.paths[0].polyline;
        assert!(p.closed);
        // Área del círculo aplanado con 0.01 mm de tolerancia.
        assert!(
            (area(p).abs() - std::f64::consts::PI * 100.0).abs() < 0.5,
            "{}",
            area(p)
        );
        for q in &p.points {
            assert!((q.x - 20.0).hypot(q.y - 20.0) <= 10.0 + 1e-2);
        }
    }

    #[test]
    fn clip_path_keeps_the_part_of_a_filled_shape_inside() {
        // Cuadrado 0..20 recortado por 10..30: queda el cuadrado 10..20.
        let doc = clipped(
            r##"<defs><clipPath id="c"><rect x="10" y="10" width="20" height="20"/></clipPath></defs>
            <g clip-path="url(#c)"><rect width="20" height="20" fill="black"/></g>"##,
        );
        assert_eq!(doc.paths.len(), 1);
        assert!((area(&doc.paths[0].polyline).abs() - 100.0).abs() < 1e-3);
        no_clip_warning(&doc);
    }

    #[test]
    fn clip_path_on_converted_text_keeps_the_holes() {
        // "O" convertida a curvas: anillo (círculo exterior y agujero en la misma trayectoria),
        // recortada por la mitad izquierda.
        let ring = "M30 20 A10 10 0 1 0 10 20 A10 10 0 1 0 30 20 Z \
                    M25 20 A5 5 0 1 0 15 20 A5 5 0 1 0 25 20 Z";
        let doc = clipped(&format!(
            r##"<defs><clipPath id="c"><rect x="0" y="0" width="20" height="40"/></clipPath></defs>
            <path d="{ring}" fill="black" clip-path="url(#c)"/>"##
        ));
        no_clip_warning(&doc);
        // Medio anillo: un exterior y un agujero recortados, con el área de media corona.
        let expected = std::f64::consts::PI * (100.0 - 25.0) / 2.0;
        assert!(
            (net_area(&doc).abs() - expected).abs() < 0.5,
            "{}",
            net_area(&doc)
        );
        let (_, max_x) = x_range(&doc);
        assert!(max_x <= 20.0 + 1e-6, "{max_x}");
        // Sigue habiendo agujero: el anillo no se convirtió en un disco.
        assert!(net_area(&doc).abs() < std::f64::consts::PI * 100.0 / 2.0 - 10.0);
    }

    #[test]
    fn real_text_is_clipped_when_a_font_is_available() {
        let doc = clipped(
            r##"<defs><clipPath id="c"><rect x="0" y="0" width="12" height="100"/></clipPath></defs>
            <g clip-path="url(#c)"><text x="2" y="50" font-size="30">HHHH</text></g>"##,
        );
        // Sin fuentes en el sistema no hay trayectorias (y se avisa); con ellas, todo cabe en el recorte.
        if !doc.paths.is_empty() {
            let (_, max_x) = x_range(&doc);
            assert!(max_x <= 12.0 + 1e-6, "{max_x}");
        } else {
            assert!(any_has(&doc.warnings, "sin trayectorias"));
        }
    }

    #[test]
    fn clip_path_trims_an_open_line() {
        let doc = clipped(
            r##"<defs><clipPath id="c"><rect x="10" y="0" width="20" height="50"/></clipPath></defs>
            <line x1="0" y1="10" x2="50" y2="10" stroke="#ff0000" clip-path="url(#c)"/>"##,
        );
        no_clip_warning(&doc);
        assert_eq!(doc.paths.len(), 1);
        let p = &doc.paths[0].polyline;
        assert!(!p.closed);
        assert_eq!(p.points.len(), 2);
        assert!((p.points[0].x - 10.0).abs() < 1e-4 && (p.points[1].x - 30.0).abs() < 1e-4);
        assert_eq!(doc.paths[0].color, Rgb(255, 0, 0));
    }

    #[test]
    fn clip_path_splits_a_polyline_in_several_pieces() {
        // Zigzag que entra y sale de una franja horizontal dos veces.
        let doc = clipped(
            r##"<defs><clipPath id="c"><rect x="0" y="10" width="100" height="10"/></clipPath></defs>
            <polyline points="5,0 15,30 25,0 35,30 45,0" fill="none" stroke="black" clip-path="url(#c)"/>"##,
        );
        assert_eq!(doc.paths.len(), 4, "{:?}", doc.warnings);
        assert!(doc.paths.iter().all(|p| !p.polyline.closed));
        for p in &doc.paths {
            for q in &p.polyline.points {
                assert!(q.y >= 10.0 - 1e-6 && q.y <= 20.0 + 1e-6);
            }
        }
    }

    #[test]
    fn clip_path_on_a_stroked_closed_shape_gives_open_arcs() {
        // Contorno cerrado sin relleno: el borde del recorte no se agrega y las dos mitades
        // del tramo que da la vuelta por el inicio se unen en una sola trayectoria.
        let doc = clipped(
            r##"<defs><clipPath id="c"><rect x="0" y="0" width="10" height="50"/></clipPath></defs>
            <rect x="0" y="5" width="20" height="20" fill="none" stroke="black" clip-path="url(#c)"/>"##,
        );
        assert_eq!(doc.paths.len(), 1, "{:?}", doc.paths.len());
        let p = &doc.paths[0].polyline;
        assert!(!p.closed);
        // Entra por el borde izquierdo: (10,5) -> (0,5) -> (0,25) -> (10,25).
        assert_eq!(p.points.len(), 4);
        let (min_x, max_x) = x_range(&doc);
        assert!(min_x.abs() < 1e-6 && (max_x - 10.0).abs() < 1e-6);
    }

    #[test]
    fn edges_on_the_clip_border_are_kept() {
        // El contorno coincide con el borde del recorte: no debe perderse ningún lado.
        let doc = clipped(
            r##"<defs><clipPath id="c"><rect x="10" y="10" width="20" height="20"/></clipPath></defs>
            <rect x="10" y="10" width="20" height="20" fill="none" stroke="black" clip-path="url(#c)"/>"##,
        );
        assert_eq!(doc.paths.len(), 1);
        assert!(doc.paths[0].polyline.closed);
        assert_eq!(doc.paths[0].polyline.points.len(), 4);
    }

    #[test]
    fn untouched_shapes_keep_their_curves() {
        let doc = clipped(
            r##"<defs><clipPath id="c"><rect x="0" y="0" width="100" height="100"/></clipPath></defs>
            <circle cx="50" cy="50" r="10" fill="none" stroke="black" clip-path="url(#c)"/>"##,
        );
        assert_eq!(doc.curves.len(), 1);
        assert!(
            doc.curves[0]
                .path
                .0
                .iter()
                .any(|s| matches!(s, PathSeg::CubicTo { .. }))
        );
        assert!(doc.paths[0].polyline.closed);
    }

    #[test]
    fn clip_path_object_bounding_box_units() {
        // Círculo de radio 0.5 en unidades del cuadro del rectángulo: inscribe el rectángulo.
        let doc = clipped(
            r##"<defs><clipPath id="c" clipPathUnits="objectBoundingBox"><circle cx="0.5" cy="0.5" r="0.5"/></clipPath></defs>
            <rect x="10" y="30" width="20" height="20" fill="black" clip-path="url(#c)"/>"##,
        );
        no_clip_warning(&doc);
        assert_eq!(doc.paths.len(), 1);
        assert!((area(&doc.paths[0].polyline).abs() - std::f64::consts::PI * 100.0).abs() < 0.5);
        let b = doc.paths[0]
            .polyline
            .points
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), q| {
                (lo.min(q.y), hi.max(q.y))
            });
        assert!(
            (b.0 - 30.0).abs() < 0.01 && (b.1 - 50.0).abs() < 0.01,
            "{b:?}"
        );
    }

    #[test]
    fn clip_path_follows_group_and_clip_transforms() {
        // El recorte vive en el sistema del grupo (desplazado 10 y escalado 2) y además tiene
        // su propia transformación (desplaza 5 en x): x absoluta = 10 + 2 * (5 + 0..10) = 20..40.
        let doc = clipped(
            r##"<defs><clipPath id="c" transform="translate(5 0)"><rect width="10" height="100"/></clipPath></defs>
            <g transform="translate(10 0) scale(2)" clip-path="url(#c)">
              <line x1="0" y1="5" x2="50" y2="5" stroke="black"/></g>"##,
        );
        no_clip_warning(&doc);
        assert_eq!(doc.paths.len(), 1);
        let p = &doc.paths[0].polyline;
        assert!(
            (p.points[0].x - 20.0).abs() < 1e-4 && (p.points[1].x - 40.0).abs() < 1e-4,
            "{p:?}"
        );
        assert!((p.points[0].y - 10.0).abs() < 1e-4);
    }

    #[test]
    fn nested_clip_paths_intersect() {
        // clipPath con clip-path (0..30 y 20..50) y un grupo exterior más (0..100 en y).
        let doc = clipped(
            r##"<defs>
              <clipPath id="b"><rect x="20" y="0" width="30" height="100"/></clipPath>
              <clipPath id="a" clip-path="url(#b)"><rect x="0" y="0" width="30" height="100"/></clipPath>
              <clipPath id="outer"><rect x="0" y="0" width="100" height="100"/></clipPath>
            </defs>
            <g clip-path="url(#outer)"><g clip-path="url(#a)">
              <line x1="0" y1="10" x2="60" y2="10" stroke="black"/></g></g>"##,
        );
        no_clip_warning(&doc);
        assert_eq!(doc.paths.len(), 1);
        let p = &doc.paths[0].polyline;
        assert!(
            (p.points[0].x - 20.0).abs() < 1e-4 && (p.points[1].x - 30.0).abs() < 1e-4,
            "{p:?}"
        );
    }

    #[test]
    fn nested_groups_with_clips_intersect() {
        let doc = clipped(
            r##"<defs>
              <clipPath id="l"><rect x="0" y="0" width="30" height="100"/></clipPath>
              <clipPath id="r"><rect x="10" y="0" width="90" height="100"/></clipPath>
            </defs>
            <g clip-path="url(#l)"><g clip-path="url(#r)">
              <rect x="0" y="0" width="50" height="10" fill="black"/></g></g>"##,
        );
        assert_eq!(doc.paths.len(), 1);
        assert!((area(&doc.paths[0].polyline).abs() - 200.0).abs() < 1e-3);
    }

    #[test]
    fn shapes_fully_outside_the_clip_disappear() {
        let doc = clipped(
            r##"<defs><clipPath id="c"><rect x="0" y="0" width="5" height="5"/></clipPath></defs>
            <g clip-path="url(#c)"><rect x="20" y="20" width="10" height="10" fill="black"/>
            <line x1="20" y1="20" x2="40" y2="20" stroke="black"/></g>"##,
        );
        assert!(doc.paths.is_empty() && doc.curves.is_empty());
    }

    #[test]
    fn mask_is_applied_as_a_binary_clip_and_warns() {
        let doc = clipped(
            r##"<defs><mask id="m" maskUnits="userSpaceOnUse" x="0" y="0" width="100" height="100">
              <rect x="0" y="0" width="20" height="100" fill="white"/>
              <rect x="10" y="0" width="10" height="100" fill="black"/></mask></defs>
            <g mask="url(#m)"><line x1="0" y1="10" x2="50" y2="10" stroke="black"/></g>"##,
        );
        // Lo blanco suma y lo negro opaco encima resta: queda 0..10.
        assert_eq!(doc.paths.len(), 1);
        let p = &doc.paths[0].polyline;
        assert!(
            p.points[0].x.abs() < 1e-4 && (p.points[1].x - 10.0).abs() < 1e-4,
            "{p:?}"
        );
        assert!(
            any_has(&doc.warnings, "1 grupo(s) con máscara"),
            "{:?}",
            doc.warnings
        );
        assert!(any_has(&doc.warnings, "aproximación"));
        assert!(!any_has(&doc.warnings, "no se pudo evaluar"));
    }

    #[test]
    fn gradient_mask_is_ignored_with_a_warning() {
        let doc = clipped(
            r##"<defs>
              <linearGradient id="g"><stop offset="0" stop-color="white"/><stop offset="1" stop-color="black"/></linearGradient>
              <mask id="m" maskUnits="userSpaceOnUse" x="0" y="0" width="100" height="100">
                <rect width="100" height="100" fill="url(#g)"/></mask></defs>
            <g mask="url(#m)"><line x1="0" y1="10" x2="50" y2="10" stroke="black"/></g>"##,
        );
        assert_eq!(doc.paths.len(), 1);
        let p = &doc.paths[0].polyline;
        assert!((p.points[1].x - 50.0).abs() < 1e-4);
        assert!(
            any_has(&doc.warnings, "no se pudo evaluar"),
            "{:?}",
            doc.warnings
        );
        assert!(!any_has(&doc.warnings, "aproximación"));
    }

    #[test]
    fn gradient_and_pattern_paint_warn() {
        let w = warnings_of(&format!(
            r##"{MM_HEAD}<defs>
              <linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient>
              <pattern id="p" width="5" height="5" patternUnits="userSpaceOnUse"><rect width="2" height="2"/></pattern>
            </defs>
            <rect width="10" height="10" fill="url(#g)"/>
            <rect x="20" width="10" height="10" fill="url(#p)"/>{LINE}</svg>"##
        ));
        assert!(
            any_has(&w, "2 trayectoria(s) con degradado o patrón"),
            "{w:?}"
        );
        assert!(any_has(&w, "negro"));
    }

    #[test]
    fn text_without_font_warns() {
        let w = warnings_of(&format!(
            r##"{MM_HEAD}{LINE}<text x="5" y="50" font-family="FuenteQueNoExiste123" font-size="8">Hola</text></svg>"##
        ));
        // Con o sin fuente de respaldo del sistema, no debe haber aviso espurio si hay trayectorias.
        let doc = import_svg(
            format!(
                r##"{MM_HEAD}<text x="5" y="50" font-family="FuenteQueNoExiste123" font-size="8">Hola</text></svg>"##
            )
            .as_bytes(),
            0.05,
        )
        .unwrap();
        if doc.paths.is_empty() {
            assert!(
                any_has(&doc.warnings, "sin trayectorias"),
                "{:?}",
                doc.warnings
            );
            assert!(any_has(&doc.warnings, "fuente"));
        } else {
            assert!(!any_has(&w, "fuente"), "{w:?}");
        }
    }

    #[test]
    fn hidden_elements_warn() {
        let w = warnings_of(&format!(
            r##"{MM_HEAD}{LINE}
            <line x1="0" y1="20" x2="5" y2="20" stroke="red" display="none"/>
            <g style="fill:none; display:none"><line x1="0" y1="30" x2="5" y2="30" stroke="red"/></g>
            <line x1="0" y1="40" x2="5" y2="40" stroke="red" visibility="hidden"/></svg>"##
        ));
        assert!(any_has(&w, "2 elemento(s) con display:none"), "{w:?}");
        assert!(
            any_has(&w, "1 trayectoria(s) con visibility oculta"),
            "{w:?}"
        );
    }

    #[test]
    fn empty_document_warns() {
        let w = warnings_of(&format!("{MM_HEAD}</svg>"));
        assert!(any_has(&w, "no contiene trayectorias"), "{w:?}");
    }
}
