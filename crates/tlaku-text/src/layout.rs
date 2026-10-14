//! Del texto a curvas: shaping, disposición en líneas y colocación en trayecto.

use crate::effects::{MAX_BEND_DEG, bend_path, bend_radius, has_closed, weld_paths};
use crate::library::FontLibrary;
use crate::linefont::LineFont;
use crate::path_sampler::PathSampler;
use crate::spec::{Align, TextSpec};
use serde::Serialize;
use std::collections::BTreeSet;
use tlaku_core::bezpath::{BezPath, PathSeg};
use tlaku_core::{Bounds, Point};
use tlaku_i18n::tr;
use ttf_parser::{GlyphId, OutlineBuilder, Tag};

/// Distancias menores que esto (mm) se consideran el mismo punto.
const EPS_MM: f64 = 1e-6;
/// Áreas menores (mm²) se consideran un contorno vacío.
const MIN_AREA_MM2: f64 = 1e-10;
/// Tolerancia de aplanado usada solo para medir (área y límites).
const MEASURE_TOL_MM: f64 = 0.005;
/// Espacios que equivale un tabulador.
const TAB_SPACES: usize = 4;

/// Resultado de convertir un texto en curvas.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TextOutline {
    /// Un `BezPath` por glifo visible, en orden lógico. Un glifo con huecos ("O", "B") trae
    /// varios subtrayectos con sentidos opuestos. Coordenadas en mm, Y hacia arriba.
    pub paths: Vec<BezPath>,
    /// Sustituciones de fuente, caracteres sin glifo, texto que no cabe en la trayectoria, etc.
    pub warnings: Vec<String>,
    /// Ancho de la línea más larga (mm), sin el espaciado tras el último carácter.
    pub advance_mm: f64,
    /// Caja que envuelve todas las curvas; `None` si no hay nada que dibujar.
    pub bounds: Option<Bounds>,
}

/// Un glifo ya situado dentro de su línea.
struct PlacedGlyph {
    id: GlyphId,
    /// Posición de la pluma desde el inicio de la línea (mm).
    pen_x: f64,
    /// Avance propio del glifo (mm), sin el espaciado entre letras.
    advance: f64,
    x_offset: f64,
    y_offset: f64,
}

struct Line {
    glyphs: Vec<PlacedGlyph>,
    width: f64,
}

/// Convierte `spec` en contornos. `on_path` solo se usa si `spec.on_path` es `Some`.
pub fn outline(
    lib: &FontLibrary,
    spec: &TextSpec,
    on_path: Option<&BezPath>,
) -> Result<TextOutline, String> {
    if !(spec.size_mm.is_finite() && spec.size_mm > 0.0) {
        return Err(tr!("text.layout.bad_size", value = spec.size_mm));
    }
    if !spec.letter_spacing_mm.is_finite() {
        return Err(tr!("text.layout.bad_letter_spacing"));
    }
    if !(spec.line_spacing.is_finite() && spec.line_spacing > 0.0) {
        return Err(tr!(
            "text.layout.bad_line_spacing",
            value = spec.line_spacing
        ));
    }

    if !spec.bend_deg.is_finite() {
        return Err(tr!("text.layout.bad_bend"));
    }

    let line_font = lib.line_font(&spec.font_family);
    let (mut warnings, shaped) = match &line_font {
        Some(font) => {
            let mut w = Vec::new();
            if spec.bold || spec.italic {
                w.push(tr!("text.layout.line_font_style", name = font.name));
            }
            (w, shape_line_font(font, spec))
        }
        None => {
            let choice = lib.resolve(&spec.font_family, spec.bold, spec.italic)?;
            let w = choice.warnings.clone();
            let shaped = lib
                .with_face_data(choice.id, |data, index| {
                    shape_and_extract(data, index, spec)
                })
                .ok_or_else(|| tr!("text.layout.font_unreadable", family = choice.family))??;
            (w, shaped)
        }
    };

    let sampler = match (&spec.on_path, on_path) {
        (None, _) => None,
        (Some(_), None) => {
            return Err(tr!("text.layout.missing_path"));
        }
        (Some(op), Some(path)) => {
            let (s, extra) = PathSampler::new(path, op.flip)?;
            if extra {
                warnings.push(tr!("text.layout.many_subpaths"));
            }
            Some(s)
        }
    };
    warnings.extend(shaped.warnings);

    let width = shaped.lines.iter().map(|l| l.width).fold(0.0_f64, f64::max);
    let mut paths = place(
        &shaped.glyph_paths,
        &shaped.lines,
        spec,
        width,
        sampler.as_ref(),
        &mut warnings,
    );

    if spec.welded {
        if line_font.is_some() || !has_closed(&paths) {
            warnings.push(tr!("text.layout.weld_line_font"));
        } else {
            paths = weld_paths(&paths);
        }
    }
    if spec.bend_deg != 0.0 {
        if sampler.is_some() {
            warnings.push(tr!("text.layout.bend_on_path"));
        } else {
            let bend = spec.bend_deg.clamp(-MAX_BEND_DEG, MAX_BEND_DEG);
            if bend != spec.bend_deg {
                warnings.push(tr!("text.layout.bend_clamped", bend = bend));
            }
            if let Some(radius) = bend_radius(width, bend) {
                // Ancla horizontal del arco: el centro del bloque según la alineación.
                let x_mid = match spec.align {
                    Align::Left => width / 2.0,
                    Align::Center => 0.0,
                    Align::Right => -width / 2.0,
                };
                let lowest =
                    -((shaped.lines.len().max(1) - 1) as f64) * spec.line_spacing * spec.size_mm
                        - spec.size_mm * 0.3;
                if radius > 0.0 && radius + lowest <= 0.0 {
                    warnings.push(tr!("text.layout.bend_too_tight"));
                }
                paths = paths.iter().map(|p| bend_path(p, x_mid, radius)).collect();
            }
        }
    }

    let bounds = bounds_of(&paths);
    Ok(TextOutline {
        paths,
        warnings,
        advance_mm: width,
        bounds,
    })
}

/// Datos que salen del paso que necesita los bytes de la fuente.
struct Shaped {
    lines: Vec<Line>,
    /// Contorno de cada glifo usado, en mm con el origen en su punto de pluma.
    glyph_paths: std::collections::HashMap<u16, BezPath>,
    warnings: Vec<String>,
}

fn shape_and_extract(data: &[u8], index: u32, spec: &TextSpec) -> Result<Shaped, String> {
    let face =
        rustybuzz::Face::from_slice(data, index).ok_or_else(|| tr!("text.layout.face_damaged"))?;
    let upem = f64::from(face.units_per_em());
    if upem <= 0.0 {
        return Err(tr!("text.layout.bad_em"));
    }
    let scale = spec.size_mm / upem;

    // Con espaciado entre letras se desactivan las ligaduras: una ligadura funde caracteres
    // y el espaciado no tendría dónde aplicarse.
    let mut features = Vec::new();
    if spec.letter_spacing_mm != 0.0 {
        for tag in [b"liga", b"clig", b"dlig"] {
            features.push(rustybuzz::Feature::new(Tag::from_bytes(tag), 0, ..));
        }
    }

    let normalized = spec
        .text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\t', &" ".repeat(TAB_SPACES));

    let mut missing: BTreeSet<char> = BTreeSet::new();
    let mut lines = Vec::new();
    let mut glyph_paths = std::collections::HashMap::new();

    for text_line in normalized.split('\n') {
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text_line);
        let glyphs = rustybuzz::shape(&face, &features, buffer);
        let infos = glyphs.glyph_infos();
        let positions = glyphs.glyph_positions();

        let mut placed = Vec::with_capacity(infos.len());
        let mut pen = 0.0;
        for (i, (info, pos)) in infos.iter().zip(positions).enumerate() {
            let id = GlyphId(info.glyph_id as u16);
            if i > 0 {
                pen += spec.letter_spacing_mm;
            }
            let advance = f64::from(pos.x_advance) * scale;
            if id.0 == 0 {
                // Carácter sin glifo en esta fuente: no se dibuja el recuadro de "no definido".
                let ch = text_line
                    .get(info.cluster as usize..)
                    .and_then(|s| s.chars().next());
                if let Some(ch) = ch.filter(|c| !c.is_whitespace() && !c.is_control()) {
                    missing.insert(ch);
                }
            } else {
                glyph_paths
                    .entry(id.0)
                    .or_insert_with(|| glyph_outline(&face, id, scale));
            }
            placed.push(PlacedGlyph {
                id,
                pen_x: pen,
                advance,
                x_offset: f64::from(pos.x_offset) * scale,
                y_offset: f64::from(pos.y_offset) * scale,
            });
            pen += advance;
        }
        lines.push(Line {
            glyphs: placed,
            width: pen,
        });
    }

    let mut warnings = Vec::new();
    if !missing.is_empty() {
        let list: String = missing
            .iter()
            .map(|c| format!("'{c}'"))
            .collect::<Vec<_>>()
            .join(" ");
        warnings.push(tr!("text.layout.missing_glyphs", list = list));
    }
    Ok(Shaped {
        lines,
        glyph_paths,
        warnings,
    })
}

/// Relación entre la altura de la mayúscula y el em en las fuentes de una línea. Con ella
/// `size_mm` significa lo mismo (el em) que en las fuentes de contorno y cambiar de una a otra
/// mantiene el tamaño visual parecido.
pub const LINE_FONT_CAP_RATIO: f64 = 0.72;

/// Equivalente de [`shape_and_extract`] para fuentes de un trazo: cada glifo es una sola
/// trayectoria abierta con varios subtrayectos (uno por trazo), sin cerrar.
fn shape_line_font(font: &LineFont, spec: &TextSpec) -> Shaped {
    let unit = spec.size_mm * LINE_FONT_CAP_RATIO / font.cap_units;
    let normalized = spec
        .text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\t', &" ".repeat(TAB_SPACES));

    let mut missing: BTreeSet<char> = BTreeSet::new();
    let mut ids: std::collections::HashMap<char, u16> = std::collections::HashMap::new();
    let mut glyph_paths = std::collections::HashMap::new();
    let mut lines = Vec::new();
    for text_line in normalized.split('\n') {
        let mut placed = Vec::new();
        let mut pen = 0.0;
        for (i, ch) in text_line.chars().enumerate() {
            if i > 0 {
                pen += spec.letter_spacing_mm;
            }
            let Some(glyph) = font.glyph(ch) else {
                if !ch.is_whitespace() && !ch.is_control() {
                    missing.insert(ch);
                }
                continue;
            };
            let next = ids.len() as u16;
            let id = *ids.entry(ch).or_insert_with(|| {
                let segs = glyph
                    .strokes
                    .iter()
                    .flat_map(|stroke| {
                        stroke.iter().enumerate().map(|(k, &(x, y))| {
                            let to = Point::new(x * unit, y * unit);
                            if k == 0 {
                                PathSeg::MoveTo { to }
                            } else {
                                PathSeg::LineTo { to }
                            }
                        })
                    })
                    .collect();
                glyph_paths.insert(next, BezPath(segs));
                next
            });
            let advance = glyph.advance * unit;
            placed.push(PlacedGlyph {
                id: GlyphId(id),
                pen_x: pen,
                advance,
                x_offset: 0.0,
                y_offset: 0.0,
            });
            pen += advance;
        }
        lines.push(Line {
            glyphs: placed,
            width: pen,
        });
    }
    let mut warnings = Vec::new();
    if !missing.is_empty() {
        let list: String = missing
            .iter()
            .map(|c| format!("'{c}'"))
            .collect::<Vec<_>>()
            .join(" ");
        warnings.push(tr!("text.layout.missing_glyphs", list = list));
    }
    Shaped {
        lines,
        glyph_paths,
        warnings,
    }
}

/// Recoge los comandos de ttf-parser sin aplanar, ya escalados a mm.
struct GlyphBuilder {
    scale: f64,
    segs: Vec<PathSeg>,
}

impl GlyphBuilder {
    fn pt(&self, x: f32, y: f32) -> Point {
        Point::new(f64::from(x) * self.scale, f64::from(y) * self.scale)
    }
}

impl OutlineBuilder for GlyphBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        let to = self.pt(x, y);
        self.segs.push(PathSeg::MoveTo { to });
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let to = self.pt(x, y);
        self.segs.push(PathSeg::LineTo { to });
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (ctrl, to) = (self.pt(x1, y1), self.pt(x, y));
        self.segs.push(PathSeg::QuadTo { ctrl, to });
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (ctrl1, ctrl2, to) = (self.pt(x1, y1), self.pt(x2, y2), self.pt(x, y));
        self.segs.push(PathSeg::CubicTo { ctrl1, ctrl2, to });
    }
    fn close(&mut self) {
        self.segs.push(PathSeg::Close);
    }
}

fn glyph_outline(face: &rustybuzz::Face, id: GlyphId, scale: f64) -> BezPath {
    let mut builder = GlyphBuilder {
        scale,
        segs: Vec::new(),
    };
    // Sin contorno (espacios) devuelve None: queda un trayecto vacío.
    let _ = face.outline_glyph(id, &mut builder);
    clean(&BezPath(builder.segs))
}

fn near(a: Point, b: Point) -> bool {
    a.dist(b) < EPS_MM
}

/// Quita segmentos degenerados y contornos sin área; todos los subtrayectos quedan cerrados.
/// No cambia el sentido de giro de los contornos.
fn clean(path: &BezPath) -> BezPath {
    let mut out: Vec<PathSeg> = Vec::new();
    let mut contour: Vec<PathSeg> = Vec::new();
    let mut start = Point::new(0.0, 0.0);
    let mut cur = start;
    let mut open = false;

    let flush = |contour: &mut Vec<PathSeg>, out: &mut Vec<PathSeg>| {
        if contour.len() > 1 {
            // Una línea final que vuelve al inicio es redundante: el cierre ya la dibuja.
            let first = match contour[0] {
                PathSeg::MoveTo { to } => to,
                _ => unreachable!("un contorno siempre empieza con MoveTo"),
            };
            if matches!(contour.last(), Some(PathSeg::LineTo { to }) if near(*to, first)) {
                contour.pop();
            }
            contour.push(PathSeg::Close);
            if contour_area(contour).abs() >= MIN_AREA_MM2 {
                out.append(contour);
            }
        }
        contour.clear();
    };

    for seg in &path.0 {
        match *seg {
            PathSeg::MoveTo { to } => {
                flush(&mut contour, &mut out);
                contour.push(*seg);
                start = to;
                cur = to;
                open = true;
            }
            _ if !open => {}
            PathSeg::LineTo { to } => {
                if !near(to, cur) {
                    contour.push(*seg);
                    cur = to;
                }
            }
            PathSeg::QuadTo { ctrl, to } => {
                if !(near(to, cur) && near(ctrl, cur)) {
                    contour.push(*seg);
                    cur = to;
                }
            }
            PathSeg::CubicTo { ctrl1, ctrl2, to } => {
                if !(near(to, cur) && near(ctrl1, cur) && near(ctrl2, cur)) {
                    contour.push(*seg);
                    cur = to;
                }
            }
            PathSeg::Close => {
                flush(&mut contour, &mut out);
                cur = start;
                open = false;
            }
        }
    }
    flush(&mut contour, &mut out);
    BezPath(out)
}

/// Área con signo (fórmula del zapatero) del contorno aplanado.
fn contour_area(segs: &[PathSeg]) -> f64 {
    BezPath(segs.to_vec())
        .flatten(MEASURE_TOL_MM)
        .iter()
        .map(|p| {
            let n = p.points.len();
            (0..n)
                .map(|i| {
                    let (a, b) = (p.points[i], p.points[(i + 1) % n]);
                    a.x * b.y - b.x * a.y
                })
                .sum::<f64>()
                / 2.0
        })
        .sum()
}

/// Coloca los glifos en recta o sobre la trayectoria.
fn place(
    glyph_paths: &std::collections::HashMap<u16, BezPath>,
    lines: &[Line],
    spec: &TextSpec,
    block_width: f64,
    sampler: Option<&PathSampler>,
    warnings: &mut Vec<String>,
) -> Vec<BezPath> {
    let mut out = Vec::new();
    let mut skipped = 0usize;
    let offset = spec.on_path.map_or(0.0, |o| o.offset_mm);

    for (line_idx, line) in lines.iter().enumerate() {
        let baseline = -(line_idx as f64) * spec.line_spacing * spec.size_mm;
        for g in &line.glyphs {
            let Some(base) = glyph_paths.get(&g.id.0) else {
                continue;
            };
            if base.0.is_empty() {
                continue;
            }
            let placed = match sampler {
                None => {
                    // Ancla en x = 0: izquierda empieza ahí, centro se centra, derecha termina ahí.
                    let x0 = match spec.align {
                        Align::Left => 0.0,
                        Align::Center => -line.width / 2.0,
                        Align::Right => -line.width,
                    };
                    let (dx, dy) = (x0 + g.pen_x + g.x_offset, baseline + g.y_offset);
                    base.map_points(|p| Point::new(p.x + dx, p.y + dy))
                }
                Some(s) => {
                    // El bloque arranca en `offset`; las líneas cortas se alinean dentro del
                    // ancho de la más larga.
                    let slack = block_width - line.width;
                    let x0 = match spec.align {
                        Align::Left => 0.0,
                        Align::Center => slack / 2.0,
                        Align::Right => slack,
                    };
                    let s_mid = offset + x0 + g.pen_x + g.advance / 2.0;
                    if !s.closed && !(0.0..=s.length()).contains(&s_mid) {
                        skipped += 1;
                        continue;
                    }
                    let origin = s.point_at(s_mid);
                    let (tx, ty) = s.direction_at(s_mid, g.advance / 2.0);
                    // Normal a la izquierda del sentido de avance: hacia donde apunta la cabeza.
                    let (nx, ny) = (-ty, tx);
                    let half = g.advance / 2.0;
                    base.map_points(|p| {
                        let u = p.x + g.x_offset - half;
                        let v = p.y + g.y_offset + baseline;
                        Point::new(origin.x + u * tx + v * nx, origin.y + u * ty + v * ny)
                    })
                }
            };
            out.push(placed);
        }
    }

    if let Some(s) = sampler {
        let end = offset + block_width;
        if s.closed {
            if block_width > s.length() + EPS_MM {
                warnings.push(tr!(
                    "text.layout.longer_than_closed",
                    text = format!("{block_width:.2}"),
                    path = format!("{:.2}", s.length())
                ));
            }
        } else if skipped > 0 || end > s.length() + EPS_MM || offset < -EPS_MM {
            warnings.push(tr!(
                "text.layout.does_not_fit",
                text = format!("{block_width:.2}"),
                offset = format!("{offset:.2}"),
                path = format!("{:.2}", s.length()),
                skipped = skipped
            ));
        }
    }
    out
}

fn bounds_of(paths: &[BezPath]) -> Option<Bounds> {
    let mut min = Point::new(f64::INFINITY, f64::INFINITY);
    let mut max = Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    let mut any = false;
    for path in paths {
        for line in path.flatten(MEASURE_TOL_MM) {
            for p in line.points {
                any = true;
                min = Point::new(min.x.min(p.x), min.y.min(p.y));
                max = Point::new(max.x.max(p.x), max.y.max(p.y));
            }
        }
    }
    any.then_some(Bounds { min, max })
}
