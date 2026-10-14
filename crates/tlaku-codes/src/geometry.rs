//! De módulos a formas cerradas: barras o cuadros unidos, texto legible e inversión.

use crate::encode::Encoded;
use crate::{CodeOutput, CodeSpec};
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Point, Polyline};
use tlaku_geom::{BoolOp, boolean, signed_area, weld};
use tlaku_i18n::tr;
use tlaku_text::{Align, FontLibrary, TextSpec, outline};

/// Tolerancia al aplanar los glifos del texto legible (mm).
const TEXT_TOL_MM: f64 = 0.005;
/// Alto del em del texto legible respecto al alto de las barras.
const TEXT_EM_RATIO: f64 = 0.3;
/// Separación entre barras y texto, en múltiplos del em.
const TEXT_GAP_EM: f64 = 0.25;
/// El texto no ocupa más que esta fracción del ancho de las barras.
const TEXT_MAX_FILL: f64 = 1.0;

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Polyline {
    Polyline {
        points: vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ],
        closed: true,
    }
}

/// Tramos consecutivos de `true` en `row`: `(inicio, fin_exclusivo)`.
fn runs(row: impl Iterator<Item = bool>) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut n = 0;
    for (i, on) in row.enumerate() {
        n = i + 1;
        match (on, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                out.push((s, i));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, n));
    }
    out
}

/// Código 2D: `bits` por filas de arriba abajo, `side` módulos por lado.
pub(crate) fn matrix(spec: &CodeSpec, bits: &[bool], side: usize) -> Result<CodeOutput, String> {
    let m = spec.size_mm / side as f64;
    let mut dark = Vec::new();
    for row in 0..side {
        let y1 = spec.size_mm - row as f64 * m;
        let y0 = y1 - m;
        for (a, b) in runs((0..side).map(|c| bits[row * side + c])) {
            dark.push(rect(a as f64 * m, y0, b as f64 * m, y1));
        }
    }
    finish(spec, dark, side)
}

/// Código 1D: barras de `spec.height_mm` y, si se pide, el texto legible debajo.
pub(crate) fn linear(
    lib: &FontLibrary,
    spec: &CodeSpec,
    encoded: &Encoded,
) -> Result<CodeOutput, String> {
    debug_assert!(spec.kind.is_linear());
    let n = encoded.bits.len();
    let m = spec.size_mm / n as f64;
    let mut dark = Vec::new();
    for (a, b) in runs(encoded.bits.iter().map(|&b| b == 1)) {
        dark.push(rect(a as f64 * m, 0.0, b as f64 * m, spec.height_mm));
    }
    if spec.human_readable {
        dark.extend(readable_text(lib, spec, &encoded.text)?);
    }
    finish(spec, dark, n)
}

/// Contornos cerrados del texto, centrado bajo las barras.
fn readable_text(lib: &FontLibrary, spec: &CodeSpec, text: &str) -> Result<Vec<Polyline>, String> {
    if spec.font_family.trim().is_empty() {
        return Err(tr!("codes.geometry.pick_font"));
    }
    if lib.line_font(&spec.font_family).is_some() {
        return Err(tr!("codes.geometry.outline_font"));
    }
    let mut em = (spec.height_mm * TEXT_EM_RATIO).max(0.5);
    let mut ts = TextSpec {
        text: text.to_string(),
        font_family: spec.font_family.clone(),
        size_mm: em,
        align: Align::Left,
        ..TextSpec::default()
    };
    let mut out = outline(lib, &ts, None)?;
    let width = |o: &tlaku_text::TextOutline| o.bounds.map_or(0.0, |b| b.max.x - b.min.x);
    if width(&out) > spec.size_mm * TEXT_MAX_FILL && width(&out) > 0.0 {
        em *= spec.size_mm * TEXT_MAX_FILL / width(&out);
        ts.size_mm = em;
        out = outline(lib, &ts, None)?;
    }
    let Some(b) = out.bounds else {
        return Ok(Vec::new());
    };
    // Centrado en x; la parte alta del texto queda `gap` bajo las barras.
    let dx = spec.size_mm / 2.0 - (b.min.x + b.max.x) / 2.0;
    let dy = -em * TEXT_GAP_EM - b.max.y;
    let mut polys = Vec::new();
    for path in &out.paths {
        for mut line in path.flatten(TEXT_TOL_MM) {
            if !line.closed {
                continue;
            }
            for p in &mut line.points {
                *p = Point::new(p.x + dx, p.y + dy);
            }
            polys.push(line);
        }
    }
    Ok(polys)
}

/// Une las piezas oscuras (o las resta del fondo si `invert`) y lo lleva al origen.
fn finish(spec: &CodeSpec, dark: Vec<Polyline>, modules: usize) -> Result<CodeOutput, String> {
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for p in dark.iter().flat_map(|l| &l.points) {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    if !min_x.is_finite() {
        return Err(tr!("codes.geometry.no_shapes"));
    }
    let (mut pieces, origin_x, origin_y, width, height);
    if spec.invert {
        let g = spec.margin_mm;
        let (x0, y0, x1, y1) = (min_x - g, min_y - g, max_x + g, max_y + g);
        pieces = boolean(&[rect(x0, y0, x1, y1)], &dark, BoolOp::Difference);
        (origin_x, origin_y) = (x0, y0);
        (width, height) = (x1 - x0, y1 - y0);
    } else {
        pieces = weld(&dark);
        (origin_x, origin_y) = (min_x, min_y);
        (width, height) = (max_x - min_x, max_y - min_y);
    }
    if pieces.is_empty() {
        return Err(tr!("codes.geometry.empty_result"));
    }
    let mut paths: Vec<BezPath> = Vec::new();
    for poly in &mut pieces {
        for p in &mut poly.points {
            *p = Point::new(p.x - origin_x, p.y - origin_y);
        }
        let path = BezPath::from(&*poly);
        // Salida de `weld`/`boolean`: cada exterior (antihorario) va seguido de sus agujeros.
        match paths.last_mut() {
            Some(last) if signed_area(&poly.points) <= 0.0 => last.0.extend(path.0),
            _ => paths.push(path),
        }
    }
    Ok(CodeOutput {
        paths,
        width_mm: width,
        height_mm: height,
        modules,
        warnings: Vec::new(),
    })
}
