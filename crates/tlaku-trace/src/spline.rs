//! Modo spline: vectoriza con vtracer (binario) y convierte su salida a cúbicas.
//!
//! vtracer no expone sus trayectorias como tipos propios (el crate `visioncortex` no se
//! reexporta), así que se lee la salida SVG con precisión completa de `f64`. Cada `<path>`
//! es una forma con su exterior y sus huecos como subtrayectos `M ... C ... Z`.

use crate::binarize::Bitmap;
use tlaku_i18n::tr;
use vtracer::{ColorImage, ColorMode, Config, Hierarchical};

/// Una forma: cada lazo es una lista de nodos cúbicos `(inicio, [(c1, c2, fin), ...])`.
pub(crate) type CubicLoop = ((f64, f64), Vec<[(f64, f64); 3]>);
pub(crate) type CubicShape = Vec<CubicLoop>;

pub(crate) struct SplineParams {
    pub corner_threshold_deg: i32,
    pub length_threshold: f64,
}

fn trace_whole(bm: &Bitmap, params: &SplineParams) -> Result<Vec<CubicShape>, String> {
    let mut pixels = Vec::with_capacity(bm.w * bm.h * 4);
    for &ink in &bm.ink {
        let v = if ink { 0 } else { 255 };
        pixels.extend_from_slice(&[v, v, v, 255]);
    }
    let img = ColorImage {
        pixels,
        width: bm.w,
        height: bm.h,
    };
    let config = Config {
        color_mode: ColorMode::Binary,
        hierarchical: Hierarchical::Stacked,
        // Las manchas ya se filtraron antes (incluye huecos pequeños).
        filter_speckle: 0,
        corner_threshold: params.corner_threshold_deg,
        length_threshold: params.length_threshold,
        path_precision: None,
        ..Config::default()
    };
    // vtracer puede entrar en pánico con formas degeneradas: se convierte en error.
    let svg = std::panic::catch_unwind(|| vtracer::convert(img, config))
        .map_err(|_| tr!("trace.spline.failed"))??;
    let text = svg.to_string();
    Ok(text.lines().filter_map(parse_path_line).collect())
}

/// Lee una línea `<path d="..." ... transform="translate(x,y)"/>`.
fn parse_path_line(line: &str) -> Option<CubicShape> {
    let d = attr(line, "d=\"")?;
    let (ox, oy) = attr(line, "translate(")
        .and_then(|t| t.split_once(','))
        .and_then(|(a, b)| Some((a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?)))
        .unwrap_or((0.0, 0.0));
    let shape = parse_d(d, (ox, oy));
    (!shape.is_empty()).then_some(shape)
}

/// Texto de un atributo hasta su cierre (`"` o `)`).
fn attr<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let start = line.find(key)? + key.len();
    let rest = &line[start..];
    let end = rest.find(['"', ')'])?;
    Some(&rest[..end])
}

fn parse_d(d: &str, off: (f64, f64)) -> CubicShape {
    let mut shape: CubicShape = Vec::new();
    let mut cmd = ' ';
    let mut nums: Vec<f64> = Vec::new();
    let flush = |cmd: char, nums: &mut Vec<f64>, shape: &mut CubicShape| {
        let p = |i: usize| (nums[i] + off.0, nums[i + 1] + off.1);
        match cmd {
            'M' if nums.len() >= 2 => shape.push((p(0), Vec::new())),
            'C' if nums.len() >= 6 => {
                if let Some(last) = shape.last_mut() {
                    last.1.push([p(0), p(2), p(4)]);
                }
            }
            _ => {}
        }
        nums.clear();
    };
    for token in d.split_whitespace() {
        let (head, tail) = match token.chars().next() {
            Some(c) if c.is_ascii_alphabetic() => (Some(c), &token[1..]),
            _ => (None, token),
        };
        if let Some(c) = head {
            flush(cmd, &mut nums, &mut shape);
            cmd = c;
        }
        if !tail.is_empty() {
            nums.extend(tail.parse::<f64>());
        }
    }
    flush(cmd, &mut nums, &mut shape);
    shape.retain(|l| !l.1.is_empty());
    shape
}

/// Por encima de este número de formas se salta el intento sobre la imagen completa: vtracer
/// (visioncortex) numera los grupos de píxeles con 16 bits y falla con decenas de miles.
const MANY_SHAPES: usize = 20_000;

/// Vectoriza el mapa. Si tiene muchísimas formas (o vtracer falla), traza cada componente
/// por separado sobre su recuadro.
pub(crate) fn trace_splines(
    bm: &Bitmap,
    params: &SplineParams,
    warnings: &mut Vec<String>,
) -> Result<Vec<CubicShape>, String> {
    let (labels, count) = bm.label_components();
    if count <= MANY_SHAPES
        && let Ok(shapes) = trace_whole(bm, params)
    {
        return Ok(shapes);
    }
    // Recuadro de cada componente.
    let mut boxes = vec![(usize::MAX, usize::MAX, 0usize, 0usize); count];
    for (i, &l) in labels.iter().enumerate() {
        if l == u32::MAX {
            continue;
        }
        let (x, y) = (i % bm.w, i / bm.w);
        let b = &mut boxes[l as usize];
        *b = (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y));
    }
    let total: usize = boxes
        .iter()
        .map(|b| (b.2 - b.0 + 3) * (b.3 - b.1 + 3))
        .sum();
    if total > 400_000_000 {
        return Err(tr!("trace.spline.too_many_shapes"));
    }
    let mut out = Vec::new();
    let mut failed = 0;
    for (label, &(x0, y0, x1, y1)) in boxes.iter().enumerate() {
        // Un píxel de margen blanco alrededor para que el borde se suavice como el resto.
        let (w, h) = (x1 - x0 + 3, y1 - y0 + 3);
        let mut crop = Bitmap {
            w,
            h,
            ink: vec![false; w * h],
        };
        for y in y0..=y1 {
            for x in x0..=x1 {
                if labels[y * bm.w + x] as usize == label {
                    crop.ink[(y - y0 + 1) * w + (x - x0 + 1)] = true;
                }
            }
        }
        match trace_whole(&crop, params) {
            Ok(shapes) => {
                let (ox, oy) = (x0 as f64 - 1.0, y0 as f64 - 1.0);
                for mut shape in shapes {
                    for (start, curves) in &mut shape {
                        start.0 += ox;
                        start.1 += oy;
                        for c in curves.iter_mut().flatten() {
                            c.0 += ox;
                            c.1 += oy;
                        }
                    }
                    out.push(shape);
                }
            }
            Err(_) => failed += 1,
        }
    }
    if failed > 0 {
        warnings.push(tr!("trace.spline.shapes_failed", n = failed));
    }
    Ok(out)
}
