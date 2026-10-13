//! Importación de SVG: convierte curvas en polilíneas en milímetros y agrupa
//! las trayectorias por color para formar capas.

use laser_core::{CutSettings, Job, Layer, Point, Polyline, Rgb};
use usvg::tiny_skia_path::{self, PathSegment};
use usvg::{Group, Node, Paint};

/// usvg normaliza todas las unidades a píxeles CSS (96 por pulgada).
const PX_TO_MM: f64 = 25.4 / 96.0;

pub struct ImportedPath {
    pub color: Rgb,
    pub polyline: Polyline,
}

/// Documento importado, en coordenadas SVG (origen arriba a la izquierda, Y hacia abajo).
pub struct SvgDocument {
    pub width_mm: f64,
    pub height_mm: f64,
    pub paths: Vec<ImportedPath>,
    /// Avisos de importación (escala asumida, elementos omitidos o aproximados).
    pub warnings: Vec<String>,
}

/// `tolerance_mm` es el error máximo permitido al aproximar curvas con rectas.
pub fn import_svg(data: &[u8], tolerance_mm: f64) -> Result<SvgDocument, usvg::Error> {
    let mut opt = usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    let tree = usvg::Tree::from_data(data, &opt)?;
    let mut paths = Vec::new();
    let mut stats = Stats::default();
    collect(tree.root(), tolerance_mm, &mut paths, &mut stats);
    let width_mm = tree.size().width() as f64 * PX_TO_MM;
    let height_mm = tree.size().height() as f64 * PX_TO_MM;
    let warnings = build_warnings(data, width_mm, height_mm, paths.is_empty(), &stats);
    Ok(SvgDocument {
        width_mm,
        height_mm,
        paths,
        warnings,
    })
}

/// Conteos de lo que el recorrido del árbol de usvg omite o aproxima.
#[derive(Default)]
struct Stats {
    images: usize,
    clip_groups: usize,
    mask_groups: usize,
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
            "{} imagen(es) incrustada(s) omitida(s): las imágenes raster aún no se importan.",
            stats.images
        ));
    }
    if stats.clip_groups > 0 {
        w.push(format!(
            "{} grupo(s) con clip-path: usvg los reconoce pero el importador no recorta la \
             geometría, así que las trayectorias se importan completas.",
            stats.clip_groups
        ));
    }
    if stats.mask_groups > 0 {
        w.push(format!(
            "{} grupo(s) con máscara (mask): se ignoró la máscara y las trayectorias se \
             importan completas.",
            stats.mask_groups
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

fn collect(group: &Group, tolerance: f64, out: &mut Vec<ImportedPath>, stats: &mut Stats) {
    for node in group.children() {
        match node {
            Node::Group(g) => {
                stats.clip_groups += g.clip_path().is_some() as usize;
                stats.mask_groups += g.mask().is_some() as usize;
                collect(g, tolerance, out, stats);
            }
            Node::Text(t) => {
                let before = out.len();
                collect(t.flattened(), tolerance, out, stats);
                if out.len() > before {
                    stats.text_with_paths += 1;
                } else {
                    stats.text_empty += 1;
                }
            }
            Node::Image(_) => stats.images += 1,
            Node::Path(p) if !p.is_visible() => stats.invisible_paths += 1,
            Node::Path(p) => {
                let Some(data) = p.data().clone().transform(p.abs_transform()) else {
                    continue;
                };
                let color = path_color(p);
                if !has_solid_paint(p) {
                    stats.non_color_paints += 1;
                }
                for polyline in flatten(&data, tolerance) {
                    out.push(ImportedPath { color, polyline });
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
    fn clip_path_warns_and_keeps_geometry() {
        let doc = import_svg(
            format!(
                r##"{MM_HEAD}<defs><clipPath id="c"><rect width="10" height="10"/></clipPath></defs>
                <g clip-path="url(#c)">{LINE}</g></svg>"##
            )
            .as_bytes(),
            0.05,
        )
        .unwrap();
        assert_eq!(doc.paths.len(), 1);
        assert!(
            any_has(&doc.warnings, "1 grupo(s) con clip-path"),
            "{:?}",
            doc.warnings
        );
    }

    #[test]
    fn mask_warns() {
        let w = warnings_of(&format!(
            r##"{MM_HEAD}<defs><mask id="m"><rect width="10" height="10" fill="white"/></mask></defs>
            <g mask="url(#m)">{LINE}</g></svg>"##
        ));
        assert!(any_has(&w, "máscara"), "{w:?}");
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
