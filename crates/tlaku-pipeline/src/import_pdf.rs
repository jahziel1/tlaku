//! Importación de PDF y AI (vectores del contenido de página).
//!
//! Un AI moderno (Illustrator CS en adelante, con "Crear archivo compatible con PDF") es un PDF
//! con datos propios añadidos, así que se lee igual. Un AI antiguo es PostScript: se rechaza con
//! un mensaje que dice cómo convertirlo.
//!
//! Se usa `lopdf` solo para el análisis sintáctico (objetos, flujos y decodificación de filtros);
//! la interpretación del contenido es propia. Se leen los operadores de trayectoria (`m l c v y
//! h re`), las matrices (`cm`, `q`/`Q`), los colores de trazo y relleno (espacios Device*, ICC,
//! Separation e Indexed) y los formularios (`Do`). Cada trayectoria se pasa a mm con Y hacia
//! arriba (el PDF ya lo usa) y se agrupa en una capa por color; si solo hay relleno, el color es
//! el del relleno.
//!
//! Lo que no se importa avisa y no se pierde en silencio: texto con fuentes (no son contornos),
//! imágenes incrustadas, degradados y patrones, y recortes (`W`), que no se aplican.

use crate::import::{ImportExtras, ImportOptions, from_curves};
use lopdf::content::Content;
use lopdf::{Dictionary, Document, LoadOptions, Object, ObjectId};
use std::path::Path;
use tlaku_core::bezpath::{BezPath, PathSeg};
use tlaku_core::{Point, Rgb};
use tlaku_doc::ImportResult;
use tlaku_doc::affine::{self, IDENTITY, Transform};
use tlaku_i18n::tr;

/// 1 punto PDF en mm.
const PT_TO_MM: f64 = 25.4 / 72.0;
/// Tope de lo que se descomprime de un flujo (contra bombas de descompresión).
const MAX_STREAM_BYTES: usize = 512 * 1024 * 1024;
/// Profundidad máxima de formularios anidados.
const MAX_DEPTH: usize = 16;
/// Tope de operadores interpretados en total.
const MAX_OPERATIONS: usize = 20_000_000;

/// Trayectorias vectoriales del PDF, una capa por color.
pub(crate) fn import(
    path: &Path,
    data: &[u8],
    _opts: &ImportOptions,
    extras: &ImportExtras,
) -> Result<ImportResult, String> {
    let name = path.display();
    if data.starts_with(b"%!PS-Adobe") || data.starts_with(b"%!") {
        return Err(tr!("pipeline.import_pdf.legacy_ai", name = name));
    }
    // Algunos generadores anteponen unos bytes al `%PDF-`; el lector los tolera.
    if !data[..data.len().min(1024)]
        .windows(5)
        .any(|w| w == b"%PDF-")
    {
        return Err(tr!("pipeline.import_pdf.not_pdf", name = name));
    }
    let options = LoadOptions {
        password: Some(String::new()),
        ..LoadOptions::default()
    };
    let doc = Document::load_mem_with_options(data, options).map_err(|e| {
        let text = e.to_string();
        if text.to_lowercase().contains("password") {
            tr!("pipeline.import_pdf.password", name = name)
        } else {
            tr!("pipeline.import_pdf.invalid", name = name, error = text)
        }
    })?;
    import_document(path, &doc, extras.pdf_page)
}

fn import_document(path: &Path, doc: &Document, page_number: u32) -> Result<ImportResult, String> {
    let name = path.display();
    let pages = doc.get_pages();
    if pages.is_empty() {
        return Err(tr!("pipeline.import_pdf.no_pages", name = name));
    }
    let wanted = page_number.max(1);
    let (&number, &page_id) = pages.get_key_value(&wanted).ok_or_else(|| {
        tr!(
            "pipeline.import_pdf.no_such_page",
            name = name,
            n = pages.len(),
            page = wanted
        )
    })?;
    let mut warnings = Vec::new();
    if pages.len() > 1 {
        warnings.push(tr!(
            "pipeline.import_pdf.multi_page",
            n = pages.len(),
            page = number
        ));
    }

    let (media, rotate, user_unit) = page_geometry(doc, page_id);
    let (x0, y0, x1, y1) = media;
    let (w, h) = (x1 - x0, y1 - y0);
    let base = page_matrix(x0, y0, w, h, rotate, user_unit);
    let page_mm = (
        if rotate % 180 == 0 { w } else { h } * user_unit * PT_TO_MM,
        if rotate % 180 == 0 { h } else { w } * user_unit * PT_TO_MM,
    );

    let content = doc
        .get_page_content_with_limit(page_id, MAX_STREAM_BYTES)
        .map_err(|e| {
            tr!(
                "pipeline.import_pdf.page_unreadable",
                name = name,
                error = e
            )
        })?;
    let ops = Content::decode(&content)
        .map_err(|e| tr!("pipeline.import_pdf.page_garbled", name = name, error = e))?
        .operations;

    let resources = inherited(doc, page_id, b"Resources")
        .and_then(|o| resolve_dict(doc, &o).cloned())
        .unwrap_or_default();
    let mut interp = Interpreter::new(doc, base, page_mm);
    interp.run(&ops, &resources, 0);
    interp.finish(&mut warnings);

    let layers = interp.layers;
    if layers.is_empty() {
        let mut why = vec![tr!(
            "pipeline.import_pdf.no_vectors",
            name = name,
            page = number
        )];
        why.extend(warnings);
        return Err(why.join(". "));
    }
    Ok(from_curves(layers, warnings))
}

// ---------------------------------------------------------------------------
// Página
// ---------------------------------------------------------------------------

/// Valor de `key` en la página o en un ancestro del árbol de páginas.
fn inherited(doc: &Document, page: ObjectId, key: &[u8]) -> Option<Object> {
    let mut id = page;
    for _ in 0..64 {
        let dict = doc.get_dictionary(id).ok()?;
        if let Ok(v) = dict.get(key) {
            return Some(v.clone());
        }
        id = dict.get(b"Parent").ok()?.as_reference().ok()?;
    }
    None
}

fn resolve_dict<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a Dictionary> {
    match doc.dereference(obj).ok()?.1 {
        Object::Dictionary(d) => Some(d),
        Object::Stream(s) => Some(&s.dict),
        _ => None,
    }
}

fn number(doc: &Document, obj: &Object) -> Option<f64> {
    doc.dereference(obj)
        .ok()
        .and_then(|(_, o)| o.as_float().ok())
        .map(f64::from)
}

fn number_array(doc: &Document, obj: &Object) -> Option<Vec<f64>> {
    let (_, arr) = doc.dereference(obj).ok()?;
    arr.as_array()
        .ok()?
        .iter()
        .map(|o| number(doc, o))
        .collect()
}

/// Caja del contenido `(x0, y0, x1, y1)` en puntos, rotación (0, 90, 180, 270) y `UserUnit`.
fn page_geometry(doc: &Document, page: ObjectId) -> ((f64, f64, f64, f64), i64, f64) {
    let mut media = (0.0, 0.0, 612.0, 792.0);
    if let Some(b) = inherited(doc, page, b"MediaBox").and_then(|o| number_array(doc, &o))
        && b.len() == 4
    {
        let (xa, xb) = (b[0].min(b[2]), b[0].max(b[2]));
        let (ya, yb) = (b[1].min(b[3]), b[1].max(b[3]));
        if xb > xa && yb > ya {
            media = (xa, ya, xb, yb);
        }
    }
    let rotate = inherited(doc, page, b"Rotate")
        .and_then(|o| number(doc, &o))
        .map_or(0, |r| (r as i64).rem_euclid(360) / 90 * 90);
    let user_unit = doc
        .get_dictionary(page)
        .ok()
        .and_then(|d| d.get(b"UserUnit").ok())
        .and_then(|o| number(doc, o))
        .filter(|u| *u > 0.0)
        .unwrap_or(1.0);
    (media, rotate, user_unit)
}

/// Del espacio de la página (pt) al de la importación (mm, origen en la esquina inferior
/// izquierda de la página ya girada).
fn page_matrix(x0: f64, y0: f64, w: f64, h: f64, rotate: i64, user_unit: f64) -> Transform {
    let origin = affine::translate(-x0, -y0);
    let rot: Transform = match rotate {
        90 => [0.0, -1.0, 1.0, 0.0, 0.0, w],
        180 => [-1.0, 0.0, 0.0, -1.0, w, h],
        270 => [0.0, 1.0, -1.0, 0.0, h, 0.0],
        _ => IDENTITY,
    };
    let k = user_unit * PT_TO_MM;
    affine::compose(&affine::scale(k, k), &affine::compose(&rot, &origin))
}

// ---------------------------------------------------------------------------
// Colores
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum ColorSpace {
    Gray,
    Rgb,
    Cmyk,
    /// Un solo componente (tinta) que una función exponencial lleva al espacio alterno.
    Separation {
        alt: Box<ColorSpace>,
        c0: Vec<f64>,
        c1: Vec<f64>,
        n: f64,
    },
    Indexed {
        base: Box<ColorSpace>,
        table: Vec<u8>,
        hival: usize,
    },
    /// Patrón, Lab u otro que no se evalúa.
    Unsupported,
}

impl ColorSpace {
    /// Número de componentes que espera `sc`/`scn`.
    fn components(&self) -> usize {
        match self {
            ColorSpace::Gray | ColorSpace::Separation { .. } | ColorSpace::Indexed { .. } => 1,
            ColorSpace::Rgb => 3,
            ColorSpace::Cmyk => 4,
            ColorSpace::Unsupported => 0,
        }
    }

    /// Color inicial al elegir el espacio (negro; una tinta al máximo).
    fn initial(&self) -> Option<Rgb> {
        match self {
            ColorSpace::Separation { .. } => self.to_rgb(&[1.0]),
            ColorSpace::Unsupported => None,
            _ => self.to_rgb(&vec![0.0; self.components()]),
        }
    }

    fn to_rgb(&self, c: &[f64]) -> Option<Rgb> {
        let q = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        match self {
            ColorSpace::Gray => {
                let g = q(*c.first()?);
                Some(Rgb(g, g, g))
            }
            ColorSpace::Rgb => Some(Rgb(q(*c.first()?), q(*c.get(1)?), q(*c.get(2)?))),
            ColorSpace::Cmyk => {
                let (cc, m, y, k) = (*c.first()?, *c.get(1)?, *c.get(2)?, *c.get(3)?);
                let f = |v: f64| q((1.0 - v.clamp(0.0, 1.0)) * (1.0 - k.clamp(0.0, 1.0)));
                Some(Rgb(f(cc), f(m), f(y)))
            }
            ColorSpace::Separation { alt, c0, c1, n } => {
                let t = c.first()?.clamp(0.0, 1.0).powf(*n);
                let out: Vec<f64> = c0.iter().zip(c1).map(|(a, b)| a + t * (b - a)).collect();
                alt.to_rgb(&out)
            }
            ColorSpace::Indexed { base, table, hival } => {
                let i = (c.first()?.round().max(0.0) as usize).min(*hival);
                let n = base.components();
                let raw = table.get(i * n..(i + 1) * n)?;
                let comps: Vec<f64> = raw.iter().map(|b| f64::from(*b) / 255.0).collect();
                base.to_rgb(&comps)
            }
            ColorSpace::Unsupported => None,
        }
    }
}

fn device_space(name: &[u8]) -> Option<ColorSpace> {
    match name {
        b"DeviceGray" | b"G" | b"CalGray" => Some(ColorSpace::Gray),
        b"DeviceRGB" | b"RGB" | b"CalRGB" => Some(ColorSpace::Rgb),
        b"DeviceCMYK" | b"CMYK" => Some(ColorSpace::Cmyk),
        _ => None,
    }
}

/// Interpreta un objeto de espacio de color (nombre o arreglo).
fn parse_space(doc: &Document, obj: &Object, resources: &Dictionary, depth: usize) -> ColorSpace {
    if depth > 6 {
        return ColorSpace::Unsupported;
    }
    let Ok((_, obj)) = doc.dereference(obj) else {
        return ColorSpace::Unsupported;
    };
    match obj {
        Object::Name(n) => {
            if let Some(s) = device_space(n) {
                return s;
            }
            // Un nombre de los recursos de la página.
            let named = resources
                .get(b"ColorSpace")
                .ok()
                .and_then(|o| resolve_dict(doc, o))
                .and_then(|d| d.get(n).ok());
            match named {
                Some(o) => parse_space(doc, o, resources, depth + 1),
                None => ColorSpace::Unsupported,
            }
        }
        Object::Array(a) => {
            let Some(kind) = a.first().and_then(|o| o.as_name().ok()) else {
                return ColorSpace::Unsupported;
            };
            match kind {
                b"ICCBased" => {
                    let n = a
                        .get(1)
                        .and_then(|o| resolve_dict(doc, o))
                        .and_then(|d| d.get(b"N").ok())
                        .and_then(|o| number(doc, o))
                        .unwrap_or(3.0);
                    match n as i64 {
                        1 => ColorSpace::Gray,
                        4 => ColorSpace::Cmyk,
                        _ => ColorSpace::Rgb,
                    }
                }
                b"CalGray" => ColorSpace::Gray,
                b"CalRGB" => ColorSpace::Rgb,
                b"Separation" => parse_separation(doc, a, resources, depth),
                b"Indexed" | b"I" => parse_indexed(doc, a, resources, depth),
                b"DeviceN" => ColorSpace::Unsupported,
                _ => device_space(kind).unwrap_or(ColorSpace::Unsupported),
            }
        }
        _ => ColorSpace::Unsupported,
    }
}

fn parse_separation(
    doc: &Document,
    a: &[Object],
    resources: &Dictionary,
    depth: usize,
) -> ColorSpace {
    let (Some(alt), Some(func)) = (a.get(2), a.get(3)) else {
        return ColorSpace::Unsupported;
    };
    let alt = parse_space(doc, alt, resources, depth + 1);
    let Some(f) = resolve_dict(doc, func) else {
        return ColorSpace::Unsupported;
    };
    // Solo funciones de tipo 2 (interpolación exponencial), que son las que escriben los
    // programas de dibujo para una tinta plana.
    if f.get(b"FunctionType").ok().and_then(|o| number(doc, o)) != Some(2.0) {
        // Sin función evaluable: la tinta se pinta como gris (tinta máxima = negro).
        return ColorSpace::Separation {
            alt: Box::new(ColorSpace::Gray),
            c0: vec![1.0],
            c1: vec![0.0],
            n: 1.0,
        };
    }
    let n_out = alt.components().max(1);
    let c0 = f
        .get(b"C0")
        .ok()
        .and_then(|o| number_array(doc, o))
        .unwrap_or_else(|| vec![0.0; n_out]);
    let c1 = f
        .get(b"C1")
        .ok()
        .and_then(|o| number_array(doc, o))
        .unwrap_or_else(|| vec![1.0; n_out]);
    let n = f.get(b"N").ok().and_then(|o| number(doc, o)).unwrap_or(1.0);
    if c0.len() != c1.len() || c0.len() < alt.components() {
        return ColorSpace::Unsupported;
    }
    ColorSpace::Separation {
        alt: Box::new(alt),
        c0,
        c1,
        n,
    }
}

fn parse_indexed(doc: &Document, a: &[Object], resources: &Dictionary, depth: usize) -> ColorSpace {
    let (Some(base), Some(hival), Some(lookup)) = (a.get(1), a.get(2), a.get(3)) else {
        return ColorSpace::Unsupported;
    };
    let base = parse_space(doc, base, resources, depth + 1);
    let Some(hival) = number(doc, hival).map(|h| h.max(0.0) as usize) else {
        return ColorSpace::Unsupported;
    };
    let table = match doc.dereference(lookup).map(|(_, o)| o) {
        Ok(Object::String(s, _)) => s.clone(),
        Ok(Object::Stream(s)) => s.decompressed_content().unwrap_or_default(),
        _ => return ColorSpace::Unsupported,
    };
    if base.components() == 0 {
        return ColorSpace::Unsupported;
    }
    ColorSpace::Indexed {
        base: Box::new(base),
        table,
        hival,
    }
}

// ---------------------------------------------------------------------------
// Intérprete
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct GState {
    ctm: Transform,
    stroke_space: ColorSpace,
    fill_space: ColorSpace,
    stroke: Option<Rgb>,
    fill: Option<Rgb>,
}

#[derive(Clone, Copy, PartialEq)]
enum Paint {
    None,
    Stroke,
    Fill,
    Both,
}

struct Interpreter<'a> {
    doc: &'a Document,
    gs: GState,
    stack: Vec<GState>,
    /// Trayectoria en construcción, ya en mm.
    path: Vec<PathSeg>,
    start: Option<Point>,
    current: Option<Point>,
    pending_clip: bool,
    page_mm: (f64, f64),
    layers: Vec<(Rgb, Vec<BezPath>)>,
    ops_left: usize,
    // Contadores para los avisos.
    text_ops: usize,
    images: usize,
    shadings: usize,
    patterns: usize,
    clips: usize,
    unsupported_colors: usize,
    skipped_backgrounds: usize,
    too_deep: bool,
    budget_exhausted: bool,
}

impl<'a> Interpreter<'a> {
    fn new(doc: &'a Document, base: Transform, page_mm: (f64, f64)) -> Self {
        let black = Some(Rgb(0, 0, 0));
        Self {
            doc,
            gs: GState {
                ctm: base,
                stroke_space: ColorSpace::Gray,
                fill_space: ColorSpace::Gray,
                stroke: black,
                fill: black,
            },
            stack: Vec::new(),
            path: Vec::new(),
            start: None,
            current: None,
            pending_clip: false,
            page_mm,
            layers: Vec::new(),
            ops_left: MAX_OPERATIONS,
            text_ops: 0,
            images: 0,
            shadings: 0,
            patterns: 0,
            clips: 0,
            unsupported_colors: 0,
            skipped_backgrounds: 0,
            too_deep: false,
            budget_exhausted: false,
        }
    }

    fn run(&mut self, ops: &[lopdf::content::Operation], resources: &Dictionary, depth: usize) {
        if depth > MAX_DEPTH {
            self.too_deep = true;
            return;
        }
        let stack_base = self.stack.len();
        for op in ops {
            if self.ops_left == 0 {
                self.budget_exhausted = true;
                break;
            }
            self.ops_left -= 1;
            self.operator(&op.operator, &op.operands, resources, depth);
        }
        // Un `q` sin su `Q` dentro de un formulario no debe filtrarse al exterior.
        if let Some(saved) = self.stack.get(stack_base).cloned() {
            self.gs = saved;
        }
        self.stack.truncate(stack_base);
    }

    fn point(&self, x: f64, y: f64) -> Point {
        let [px, py] = affine::apply(&self.gs.ctm, [x, y]);
        Point::new(px, py)
    }

    fn operator(&mut self, op: &str, args: &[Object], resources: &Dictionary, depth: usize) {
        let n: Vec<f64> = args
            .iter()
            .filter_map(|o| o.as_float().ok().map(f64::from))
            .collect();
        match op {
            "q" => self.stack.push(self.gs.clone()),
            "Q" => {
                if let Some(g) = self.stack.pop() {
                    self.gs = g;
                }
            }
            "cm" if n.len() == 6 => {
                let m: Transform = [n[0], n[1], n[2], n[3], n[4], n[5]];
                self.gs.ctm = affine::compose(&self.gs.ctm, &m);
            }
            "m" if n.len() >= 2 => {
                let p = self.point(n[0], n[1]);
                self.path.push(PathSeg::MoveTo { to: p });
                self.start = Some(p);
                self.current = Some(p);
            }
            "l" if n.len() >= 2 => {
                let p = self.point(n[0], n[1]);
                self.line_to(p);
            }
            "c" if n.len() >= 6 => {
                let (c1, c2, p) = (
                    self.point(n[0], n[1]),
                    self.point(n[2], n[3]),
                    self.point(n[4], n[5]),
                );
                self.curve_to(c1, c2, p);
            }
            "v" if n.len() >= 4 => {
                let (c2, p) = (self.point(n[0], n[1]), self.point(n[2], n[3]));
                let c1 = self.current.unwrap_or(p);
                self.curve_to(c1, c2, p);
            }
            "y" if n.len() >= 4 => {
                let (c1, p) = (self.point(n[0], n[1]), self.point(n[2], n[3]));
                self.curve_to(c1, p, p);
            }
            "h" => self.close(),
            "re" if n.len() >= 4 => {
                let (x, y, w, h) = (n[0], n[1], n[2], n[3]);
                let a = self.point(x, y);
                self.path.push(PathSeg::MoveTo { to: a });
                for (px, py) in [(x + w, y), (x + w, y + h), (x, y + h)] {
                    let p = self.point(px, py);
                    self.path.push(PathSeg::LineTo { to: p });
                }
                self.path.push(PathSeg::Close);
                self.start = Some(a);
                self.current = Some(a);
            }
            "W" | "W*" => self.pending_clip = true,
            "n" => self.paint(Paint::None),
            "S" => self.paint(Paint::Stroke),
            "s" => {
                self.close();
                self.paint(Paint::Stroke);
            }
            "f" | "F" | "f*" => self.paint(Paint::Fill),
            "B" | "B*" => self.paint(Paint::Both),
            "b" | "b*" => {
                self.close();
                self.paint(Paint::Both);
            }
            "G" | "g" | "RG" | "rg" | "K" | "k" => self.device_color(op, &n),
            "CS" | "cs" => {
                let space = args.first().map_or(ColorSpace::Unsupported, |o| {
                    parse_space(self.doc, o, resources, 0)
                });
                let color = space.initial();
                if op == "CS" {
                    self.gs.stroke_space = space;
                    self.gs.stroke = color;
                } else {
                    self.gs.fill_space = space;
                    self.gs.fill = color;
                }
            }
            "SC" | "SCN" | "sc" | "scn" => {
                let stroke = op.starts_with('S');
                // El último operando de `scn` puede ser el nombre de un patrón.
                if matches!(args.last(), Some(Object::Name(_))) {
                    self.patterns += 1;
                    if stroke {
                        self.gs.stroke = None;
                    } else {
                        self.gs.fill = None;
                    }
                    return;
                }
                let space = if stroke {
                    &self.gs.stroke_space
                } else {
                    &self.gs.fill_space
                };
                let color = space.to_rgb(&n);
                if color.is_none() {
                    self.unsupported_colors += 1;
                }
                if stroke {
                    self.gs.stroke = color;
                } else {
                    self.gs.fill = color;
                }
            }
            "sh" => self.shadings += 1,
            "BT" | "Tj" | "TJ" | "'" | "\"" => {
                if op == "BT" {
                    self.text_ops += 1;
                }
            }
            "Do" => {
                if let Some(Object::Name(name)) = args.first() {
                    self.do_xobject(name, resources, depth);
                }
            }
            _ => {}
        }
    }

    fn line_to(&mut self, p: Point) {
        if self.current.is_none() {
            self.path.push(PathSeg::MoveTo { to: p });
            self.start = Some(p);
        } else {
            self.path.push(PathSeg::LineTo { to: p });
        }
        self.current = Some(p);
    }

    fn curve_to(&mut self, ctrl1: Point, ctrl2: Point, to: Point) {
        if self.current.is_none() {
            self.path.push(PathSeg::MoveTo { to: ctrl1 });
            self.start = Some(ctrl1);
        }
        self.path.push(PathSeg::CubicTo { ctrl1, ctrl2, to });
        self.current = Some(to);
    }

    fn close(&mut self) {
        if self.current.is_some() {
            self.path.push(PathSeg::Close);
            self.current = self.start;
        }
    }

    fn device_color(&mut self, op: &str, n: &[f64]) {
        let (space, comps) = match op {
            "G" | "g" => (ColorSpace::Gray, 1),
            "RG" | "rg" => (ColorSpace::Rgb, 3),
            _ => (ColorSpace::Cmyk, 4),
        };
        if n.len() < comps {
            return;
        }
        let color = space.to_rgb(n);
        if op.chars().next().is_some_and(char::is_uppercase) {
            self.gs.stroke_space = space;
            self.gs.stroke = color;
        } else {
            self.gs.fill_space = space;
            self.gs.fill = color;
        }
    }

    /// Termina la trayectoria en construcción: la pinta (o solo la descarta, con `n`).
    fn paint(&mut self, how: Paint) {
        let segs = std::mem::take(&mut self.path);
        self.start = None;
        self.current = None;
        let clip = std::mem::take(&mut self.pending_clip);
        if segs.is_empty() {
            return;
        }
        let mut path = BezPath(segs);
        if clip {
            // Un recorte del tamaño de la página es el habitual de Illustrator y no cambia nada.
            let covers_page = path
                .bounds()
                .is_some_and(|b| self.covers_page(b.max.x - b.min.x, b.max.y - b.min.y));
            if !covers_page {
                self.clips += 1;
            }
        }
        if how == Paint::None {
            return;
        }
        let color = match how {
            Paint::Stroke | Paint::Both => self.gs.stroke,
            _ => self.gs.fill,
        };
        let Some(color) = color else {
            self.unsupported_colors += 1;
            return;
        };
        if how == Paint::Fill {
            // Un relleno cierra solo los subtrayectos abiertos.
            close_open_subpaths(&mut path);
            // El fondo blanco de página completa no es parte del diseño.
            if color == Rgb(255, 255, 255)
                && path
                    .bounds()
                    .is_some_and(|b| self.covers_page(b.max.x - b.min.x, b.max.y - b.min.y))
            {
                self.skipped_backgrounds += 1;
                return;
            }
        }
        match self.layers.iter_mut().find(|(c, _)| *c == color) {
            Some((_, paths)) => paths.push(path),
            None => self.layers.push((color, vec![path])),
        }
    }

    fn covers_page(&self, w: f64, h: f64) -> bool {
        w >= self.page_mm.0 * 0.98 && h >= self.page_mm.1 * 0.98
    }

    fn do_xobject(&mut self, name: &[u8], resources: &Dictionary, depth: usize) {
        let doc = self.doc;
        let Some(obj) = resources
            .get(b"XObject")
            .ok()
            .and_then(|o| resolve_dict(doc, o))
            .and_then(|d| d.get(name).ok())
        else {
            return;
        };
        let Ok((_, Object::Stream(stream))) = doc.dereference(obj) else {
            return;
        };
        match stream.dict.get(b"Subtype").and_then(Object::as_name) {
            Ok(b"Image") => self.images += 1,
            Ok(b"Form") => {
                let Ok(data) = stream.decompressed_content_with_limit(MAX_STREAM_BYTES) else {
                    return;
                };
                let Ok(content) = Content::decode(&data) else {
                    return;
                };
                let matrix = stream
                    .dict
                    .get(b"Matrix")
                    .ok()
                    .and_then(|o| number_array(doc, o))
                    .filter(|m| m.len() == 6)
                    .map_or(IDENTITY, |m| [m[0], m[1], m[2], m[3], m[4], m[5]]);
                let res = stream
                    .dict
                    .get(b"Resources")
                    .ok()
                    .and_then(|o| resolve_dict(doc, o))
                    .unwrap_or(resources);
                let saved = self.gs.clone();
                // La trayectoria en construcción no cruza el borde del formulario.
                let outer_path = std::mem::take(&mut self.path);
                self.gs.ctm = affine::compose(&self.gs.ctm, &matrix);
                self.run(&content.operations, res, depth + 1);
                self.gs = saved;
                self.path = outer_path;
            }
            _ => {}
        }
    }

    fn finish(&self, warnings: &mut Vec<String>) {
        if self.text_ops > 0 {
            warnings.push(tr!("pipeline.import_pdf.text_blocks", n = self.text_ops));
        }
        if self.images > 0 {
            warnings.push(tr!("pipeline.import_pdf.images_skipped", n = self.images));
        }
        if self.shadings + self.patterns > 0 {
            warnings.push(tr!(
                "pipeline.import_pdf.shadings_skipped",
                n = self.shadings + self.patterns
            ));
        }
        if self.clips > 0 {
            warnings.push(tr!("pipeline.import_pdf.clips_ignored", n = self.clips));
        }
        if self.unsupported_colors > 0 {
            warnings.push(tr!(
                "pipeline.import_pdf.unsupported_colors",
                n = self.unsupported_colors
            ));
        }
        if self.skipped_backgrounds > 0 {
            warnings.push(tr!("pipeline.import_pdf.white_background"));
        }
        if self.too_deep {
            warnings.push(tr!("pipeline.import_pdf.too_deep"));
        }
        if self.budget_exhausted {
            warnings.push(tr!("pipeline.import_pdf.too_big"));
        }
    }
}

/// Agrega `Close` a los subtrayectos que no lo traen (el relleno los cierra implícitamente).
fn close_open_subpaths(path: &mut BezPath) {
    let mut out = Vec::with_capacity(path.0.len() + 1);
    let mut open = false;
    for seg in path.0.drain(..) {
        match seg {
            PathSeg::MoveTo { .. } => {
                if open {
                    out.push(PathSeg::Close);
                }
                open = false;
                out.push(seg);
            }
            PathSeg::Close => {
                open = false;
                out.push(seg);
            }
            _ => {
                open = true;
                out.push(seg);
            }
        }
    }
    if open {
        out.push(PathSeg::Close);
    }
    path.0 = out;
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::Operation;
    use lopdf::{Stream, dictionary};
    use tlaku_doc::ObjectKind;

    /// PDF de una página con el contenido dado (operadores) y los recursos opcionales.
    fn pdf(media: [i64; 4], ops: Vec<Operation>, resources: Dictionary) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages = doc.new_object_id();
        let content = Content { operations: ops };
        let content_id = doc.add_object(Stream::new(Dictionary::new(), content.encode().unwrap()));
        let page = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages,
            "MediaBox" => media.iter().map(|v| Object::Integer(*v)).collect::<Vec<_>>(),
            "Contents" => content_id,
            "Resources" => resources,
        });
        doc.objects.insert(
            pages,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page.into()],
                "Count" => 1,
            }),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
        doc.trailer.set("Root", catalog);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    fn op(name: &str, args: &[f64]) -> Operation {
        Operation::new(name, args.iter().map(|v| Object::Real(*v as f32)).collect())
    }

    fn run(data: &[u8]) -> Result<ImportResult, String> {
        import(
            Path::new("a.pdf"),
            data,
            &ImportOptions::default(),
            &ImportExtras::default(),
        )
    }

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-3, "{a} != {b}");
    }

    #[test]
    fn rectangulo_con_trazo_rojo_en_mm() {
        // 100 x 50 pt; un rectángulo de 20 x 10 pt en (10, 5) trazado en rojo.
        let data = pdf(
            [0, 0, 100, 50],
            vec![
                op("RG", &[1.0, 0.0, 0.0]),
                op("re", &[10.0, 5.0, 20.0, 10.0]),
                op("S", &[]),
            ],
            Dictionary::new(),
        );
        let r = run(&data).unwrap();
        assert_eq!(r.objects.len(), 1);
        assert_eq!(r.objects[0].layer, Rgb(255, 0, 0));
        let b = r.objects[0].bounds().unwrap();
        approx(b.min.x, 10.0 * PT_TO_MM);
        approx(b.min.y, 5.0 * PT_TO_MM);
        approx(b.max.x, 30.0 * PT_TO_MM);
        approx(b.max.y, 15.0 * PT_TO_MM);
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    }

    #[test]
    fn curvas_matrices_y_pila_de_estados() {
        // Dentro de q/Q una escala x2 y un desplazamiento; fuera, sin transformar.
        let data = pdf(
            [0, 0, 200, 200],
            vec![
                op("q", &[]),
                op("cm", &[2.0, 0.0, 0.0, 2.0, 10.0, 0.0]),
                op("m", &[0.0, 0.0]),
                op("c", &[0.0, 10.0, 10.0, 20.0, 20.0, 20.0]),
                op("S", &[]),
                op("Q", &[]),
                op("m", &[0.0, 0.0]),
                op("l", &[50.0, 0.0]),
                op("S", &[]),
            ],
            Dictionary::new(),
        );
        let r = run(&data).unwrap();
        let ObjectKind::Vector(v) = &r.objects[0].kind else {
            panic!()
        };
        assert_eq!(v.paths.len(), 2);
        let PathSeg::CubicTo { to, .. } = v.paths[0].0[1] else {
            panic!("debía conservar la curva")
        };
        approx(to.x, (2.0 * 20.0 + 10.0) * PT_TO_MM);
        approx(to.y, 40.0 * PT_TO_MM);
        let PathSeg::LineTo { to } = v.paths[1].0[1] else {
            panic!()
        };
        approx(to.x, 50.0 * PT_TO_MM);
    }

    #[test]
    fn relleno_cmyk_cierra_y_agrupa_por_color() {
        let data = pdf(
            [0, 0, 100, 100],
            vec![
                op("k", &[0.0, 1.0, 1.0, 0.0]),
                op("m", &[10.0, 10.0]),
                op("l", &[30.0, 10.0]),
                op("l", &[30.0, 30.0]),
                op("f", &[]),
                op("m", &[50.0, 50.0]),
                op("l", &[60.0, 50.0]),
                op("l", &[60.0, 60.0]),
                op("f", &[]),
            ],
            Dictionary::new(),
        );
        let r = run(&data).unwrap();
        // Los dos triángulos comparten color: un solo objeto con dos trayectorias cerradas.
        assert_eq!(r.objects.len(), 1);
        assert_eq!(r.objects[0].layer, Rgb(255, 0, 0));
        let ObjectKind::Vector(v) = &r.objects[0].kind else {
            panic!()
        };
        assert_eq!(v.paths.len(), 2);
        assert!(v.paths.iter().all(|p| p.0.last() == Some(&PathSeg::Close)));
    }

    #[test]
    fn recortes_texto_y_fondo_avisan() {
        let data = pdf(
            [0, 0, 100, 100],
            vec![
                // Fondo blanco de toda la página: se omite.
                op("g", &[1.0]),
                op("re", &[0.0, 0.0, 100.0, 100.0]),
                op("f", &[]),
                // Recorte parcial.
                op("re", &[0.0, 0.0, 50.0, 50.0]),
                op("W", &[]),
                op("n", &[]),
                op("G", &[0.0]),
                op("re", &[10.0, 10.0, 10.0, 10.0]),
                op("S", &[]),
                Operation::new("BT", vec![]),
                Operation::new("ET", vec![]),
            ],
            Dictionary::new(),
        );
        let r = run(&data).unwrap();
        assert_eq!(r.objects.len(), 1);
        let all = r.warnings.join(" | ");
        assert!(all.contains("fondo blanco"), "{all}");
        assert!(all.contains("recorte"), "{all}");
        assert!(all.contains("texto"), "{all}");
    }

    #[test]
    fn formulario_con_matriz_y_tinta_plana() {
        // Separation con función exponencial: tinta 1 = magenta (1 0 1 en RGB).
        let func = dictionary! {
            "FunctionType" => 2,
            "Domain" => vec![0.into(), 1.into()],
            "C0" => vec![1.0.into(), 1.0.into(), 1.0.into()],
            "C1" => vec![1.0.into(), 0.0.into(), 1.0.into()],
            "N" => 1,
        };
        let sep: Vec<Object> = vec![
            "Separation".into(),
            "CutContour".into(),
            "DeviceRGB".into(),
            func.into(),
        ];
        let mut doc = Document::with_version("1.5");
        let pages = doc.new_object_id();
        let form = doc.add_object(Stream::new(
            dictionary! {
                "Subtype" => "Form",
                "Matrix" => vec![1.into(), 0.into(), 0.into(), 1.into(), 20.into(), 30.into()],
            },
            b"0 0 m 10 0 l S".to_vec(),
        ));
        let res = dictionary! {
            "ColorSpace" => dictionary! { "Cs1" => sep },
            "XObject" => dictionary! { "Fm1" => form },
        };
        let content = doc.add_object(Stream::new(
            Dictionary::new(),
            b"/Cs1 CS 1 SCN /Fm1 Do".to_vec(),
        ));
        let page = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages,
            "MediaBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
            "Contents" => content, "Resources" => res,
        });
        doc.objects.insert(
            pages,
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1,
            }),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
        doc.trailer.set("Root", catalog);
        let mut data = Vec::new();
        doc.save_to(&mut data).unwrap();

        let r = run(&data).unwrap();
        assert_eq!(r.objects[0].layer, Rgb(255, 0, 255));
        let b = r.objects[0].bounds().unwrap();
        approx(b.min.x, 20.0 * PT_TO_MM);
        approx(b.min.y, 30.0 * PT_TO_MM);
        approx(b.max.x, 30.0 * PT_TO_MM);
    }

    #[test]
    fn rotacion_de_pagina() {
        // Página de 100 x 50 pt girada 90°: queda de 50 x 100 pt.
        let data = pdf(
            [0, 0, 100, 50],
            vec![op("m", &[0.0, 0.0]), op("l", &[100.0, 0.0]), op("S", &[])],
            Dictionary::new(),
        );
        // Se agrega /Rotate a la página.
        let mut doc = Document::load_mem(&data).unwrap();
        let page = *doc.get_pages().values().next().unwrap();
        doc.get_object_mut(page)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("Rotate", 90);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        let r = run(&out).unwrap();
        let b = r.objects[0].bounds().unwrap();
        // El borde inferior (y = 0) pasa al lado izquierdo (x = 0) y se alarga en Y.
        approx(b.min.x, 0.0);
        approx(b.max.x, 0.0);
        approx(b.max.y, 100.0 * PT_TO_MM);
    }

    #[test]
    fn errores_claros() {
        let e = run(b"%!PS-Adobe-3.0 EPSF-3.0\n%%Creator: Illustrator\n").unwrap_err();
        assert!(e.contains("AI antiguo"), "{e}");
        assert!(run(b"hola").unwrap_err().contains("no parece un PDF"));
        // Página sin trayectorias.
        let vacio = pdf([0, 0, 10, 10], vec![], Dictionary::new());
        assert!(
            run(&vacio)
                .unwrap_err()
                .contains("no contiene trayectorias")
        );
        // Página inexistente.
        let data = pdf(
            [0, 0, 10, 10],
            vec![op("re", &[0.0, 0.0, 5.0, 5.0]), op("S", &[])],
            Dictionary::new(),
        );
        let e = import(
            Path::new("a.pdf"),
            &data,
            &ImportOptions::default(),
            &ImportExtras {
                pdf_page: 3,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(e.contains("no existe la 3"), "{e}");
    }
}
