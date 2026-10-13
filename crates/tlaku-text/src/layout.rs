//! Del texto a curvas: shaping, disposición en líneas y colocación en trayecto.

use crate::library::FontLibrary;
use crate::path_sampler::PathSampler;
use crate::spec::{Align, TextSpec};
use serde::Serialize;
use std::collections::BTreeSet;
use tlaku_core::bezpath::{BezPath, PathSeg};
use tlaku_core::{Bounds, Point};
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
        return Err(format!(
            "El tamaño del texto debe ser mayor que 0 mm (recibido {})",
            spec.size_mm
        ));
    }
    if !spec.letter_spacing_mm.is_finite() {
        return Err("El espaciado entre letras no es un número válido".into());
    }
    if !(spec.line_spacing.is_finite() && spec.line_spacing > 0.0) {
        return Err(format!(
            "El interlineado debe ser mayor que 0 (recibido {})",
            spec.line_spacing
        ));
    }

    let choice = lib.resolve(&spec.font_family, spec.bold, spec.italic)?;
    let mut warnings = choice.warnings.clone();

    let sampler = match (&spec.on_path, on_path) {
        (None, _) => None,
        (Some(_), None) => {
            return Err("El texto está en trayecto pero no se recibió la trayectoria".into());
        }
        (Some(op), Some(path)) => {
            let (s, extra) = PathSampler::new(path, op.flip)?;
            if extra {
                warnings.push(
                    "La trayectoria tiene varios subtrayectos; solo se usa el primero".into(),
                );
            }
            Some(s)
        }
    };

    let shaped = lib
        .with_face_data(choice.id, |data, index| {
            shape_and_extract(data, index, spec)
        })
        .ok_or_else(|| format!("No se pudo leer la fuente '{}'", choice.family))??;
    warnings.extend(shaped.warnings);

    let width = shaped.lines.iter().map(|l| l.width).fold(0.0_f64, f64::max);
    let paths = place(
        &shaped.glyph_paths,
        &shaped.lines,
        spec,
        width,
        sampler.as_ref(),
        &mut warnings,
    );

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
    let face = rustybuzz::Face::from_slice(data, index)
        .ok_or("La fuente está dañada o su formato no es compatible")?;
    let upem = f64::from(face.units_per_em());
    if upem <= 0.0 {
        return Err("La fuente declara un tamaño de em inválido".into());
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
        warnings.push(format!(
            "La fuente no tiene glifo para: {list}; esos caracteres se omiten"
        ));
    }
    Ok(Shaped {
        lines,
        glyph_paths,
        warnings,
    })
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
                warnings.push(format!(
                    "El texto ({block_width:.2} mm) es más largo que la trayectoria cerrada ({:.2} mm); se solapará",
                    s.length()
                ));
            }
        } else if skipped > 0 || end > s.length() + EPS_MM || offset < -EPS_MM {
            warnings.push(format!(
                "El texto no cabe en la trayectoria ({block_width:.2} mm de texto desde {offset:.2} mm sobre {:.2} mm de largo); se omiten {skipped} glifos",
                s.length()
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
