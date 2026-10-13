//! Exportación a PNG: el diseño dibujado a la resolución pedida (ppp), con el color de cada
//! capa. Las curvas se dibujan exactas (no aplanadas) con `tiny-skia`.

use crate::scene::{Item, Page, Scene, decode_image, fit_page};
use crate::{Export, PngOptions};
use tiny_skia::{
    Color, FillRule, FilterQuality, LineCap, LineJoin, Paint, PathBuilder, Pattern, Pixmap, Rect,
    SpreadMode, Stroke, Transform,
};
use tlaku_core::Rgb;
use tlaku_core::bezpath::{BezPath, PathSeg};
use tlaku_doc::Document;

/// Lado máximo y área máxima del PNG, para no agotar la memoria con un ppp absurdo.
const MAX_SIDE_PX: f64 = 30_000.0;
const MAX_PIXELS: f64 = 100_000_000.0;

pub fn export_png(
    doc: &Document,
    selection: Option<&[String]>,
    opts: &PngOptions,
) -> Result<Export, String> {
    if !opts.dpi.is_finite() || opts.dpi < 1.0 {
        return Err(format!("Resolución inválida: {} ppp.", opts.dpi));
    }
    let mut scene = Scene::collect(doc, selection)?;
    let page = fit_page(scene.bounds, opts.margin_mm, false);
    let scale = opts.dpi / 25.4;
    let (wf, hf) = ((page.w * scale).ceil(), (page.h * scale).ceil());
    if wf > MAX_SIDE_PX || hf > MAX_SIDE_PX || wf * hf > MAX_PIXELS {
        return Err(format!(
            "La imagen sería de {wf} x {hf} píxeles ({:.0} x {:.0} mm a {} ppp): es demasiado \
             grande. Baja la resolución.",
            page.w, page.h, opts.dpi
        ));
    }
    let mut pixmap = Pixmap::new(wf as u32, hf as u32)
        .ok_or_else(|| "No se pudo crear la imagen (tamaño inválido).".to_string())?;
    pixmap.fill(if opts.transparent {
        Color::TRANSPARENT
    } else {
        Color::WHITE
    });
    // Del documento (mm, Y hacia arriba) a píxeles (Y hacia abajo).
    let to_px = Transform::from_row(
        scale as f32,
        0.0,
        0.0,
        -scale as f32,
        (-page.x0 * scale) as f32,
        (page.y1() * scale) as f32,
    );
    // Al menos un píxel de grosor, para que un trazo fino no desaparezca.
    let width_mm = opts.stroke_mm.max(1.0 / scale);
    let mut warnings = std::mem::take(&mut scene.warnings);
    for color in scene.layer_colors.clone() {
        let fills = opts.fill_layers && scene.fills(color);
        for it in scene.in_layer(color) {
            if let Some(img) = it.image {
                if let Err(e) = draw_image(&mut pixmap, it, &page, scale, img.size_mm) {
                    warnings.push(format!("No se pudo dibujar la imagen: {e}"));
                }
                continue;
            }
            for path in &it.curves {
                draw_path(&mut pixmap, path, color, fills, width_mm, to_px);
            }
        }
    }
    let png = pixmap
        .encode_png()
        .map_err(|e| format!("No se pudo codificar el PNG: {e}"))?;
    Ok(Export {
        bytes: with_dpi(png, opts.dpi),
        size_mm: [page.w, page.h],
        warnings,
    })
}

fn paint_of(c: Rgb) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c.0, c.1, c.2, 255);
    p.anti_alias = true;
    p
}

fn draw_path(
    pixmap: &mut Pixmap,
    path: &BezPath,
    color: Rgb,
    fill: bool,
    width_mm: f64,
    to_px: Transform,
) {
    let mut pb = PathBuilder::new();
    for seg in &path.0 {
        match *seg {
            PathSeg::MoveTo { to } => pb.move_to(to.x as f32, to.y as f32),
            PathSeg::LineTo { to } => pb.line_to(to.x as f32, to.y as f32),
            PathSeg::QuadTo { ctrl, to } => {
                pb.quad_to(ctrl.x as f32, ctrl.y as f32, to.x as f32, to.y as f32);
            }
            PathSeg::CubicTo { ctrl1, ctrl2, to } => pb.cubic_to(
                ctrl1.x as f32,
                ctrl1.y as f32,
                ctrl2.x as f32,
                ctrl2.y as f32,
                to.x as f32,
                to.y as f32,
            ),
            PathSeg::Close => pb.close(),
        }
    }
    let Some(p) = pb.finish() else { return };
    let paint = paint_of(color);
    if fill {
        pixmap.fill_path(&p, &paint, FillRule::EvenOdd, to_px, None);
    }
    let stroke = Stroke {
        width: width_mm as f32,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    };
    pixmap.stroke_path(&p, &paint, &stroke, to_px, None);
}

fn draw_image(
    pixmap: &mut Pixmap,
    it: &Item,
    page: &Page,
    scale: f64,
    size_mm: [f64; 2],
) -> Result<(), String> {
    let img = it.image.ok_or("no es una imagen")?;
    let raw = std::fs::read(&img.source_path).map_err(|e| format!("{}: {e}", img.source_path))?;
    let rgba = decode_image(&img.source_path, &raw)?.to_rgba8();
    let (pw, ph) = (rgba.width(), rgba.height());
    let mut tex = Pixmap::new(pw, ph).ok_or("imagen vacía")?;
    // tiny-skia usa alfa premultiplicado.
    for (dst, src) in tex.pixels_mut().iter_mut().zip(rgba.pixels()) {
        let [r, g, b, a] = src.0;
        *dst = tiny_skia::ColorU8::from_rgba(r, g, b, a).premultiply();
    }
    // Píxel de la imagen (Y hacia abajo) -> mm locales (Y hacia arriba) -> documento -> píxeles
    // de salida.
    let [w, h] = size_mm;
    let local_from_px: tlaku_doc::affine::Transform =
        [w / f64::from(pw), 0.0, 0.0, -h / f64::from(ph), 0.0, h];
    let doc_from_px = tlaku_doc::affine::compose(&it.obj.transform, &local_from_px);
    let out_from_doc: tlaku_doc::affine::Transform =
        [scale, 0.0, 0.0, -scale, -page.x0 * scale, page.y1() * scale];
    let m = tlaku_doc::affine::compose(&out_from_doc, &doc_from_px);
    let transform = Transform::from_row(
        m[0] as f32,
        m[1] as f32,
        m[2] as f32,
        m[3] as f32,
        m[4] as f32,
        m[5] as f32,
    );
    let paint = Paint {
        shader: Pattern::new(
            tex.as_ref(),
            SpreadMode::Pad,
            FilterQuality::Bilinear,
            1.0,
            Transform::identity(),
        ),
        ..Paint::default()
    };
    let rect = Rect::from_xywh(0.0, 0.0, pw as f32, ph as f32).ok_or("imagen vacía")?;
    let path = PathBuilder::from_rect(rect);
    pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
    Ok(())
}

/// Inserta el trozo `pHYs` (resolución física) tras la cabecera, para que los programas abran
/// el PNG con su tamaño real.
fn with_dpi(png: Vec<u8>, dpi: f64) -> Vec<u8> {
    // Firma (8) + IHDR (4 longitud + 4 tipo + 13 datos + 4 CRC).
    const AFTER_IHDR: usize = 8 + 25;
    if png.len() < AFTER_IHDR {
        return png;
    }
    let ppm = (dpi / 0.0254).round() as u32;
    let mut data = Vec::with_capacity(9);
    data.extend(ppm.to_be_bytes());
    data.extend(ppm.to_be_bytes());
    data.push(1); // metros
    let mut chunk = Vec::with_capacity(21);
    chunk.extend(9u32.to_be_bytes());
    chunk.extend(b"pHYs");
    chunk.extend(&data);
    chunk.extend(crc32(&chunk[4..]).to_be_bytes());
    let mut out = Vec::with_capacity(png.len() + chunk.len());
    out.extend(&png[..AFTER_IHDR]);
    out.extend(chunk);
    out.extend(&png[AFTER_IHDR..]);
    out
}

/// CRC-32 de PNG (polinomio 0xEDB88320).
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}
