//! Exportación a PDF vectorial a escala 1:1 (1 mm del diseño = 1 mm en el papel).
//!
//! El PDF ya tiene Y hacia arriba, como el documento, así que no se invierte nada: solo se pasa
//! de mm a puntos (72 por pulgada). Se escribe a mano (cabecera, objetos, tabla de referencias):
//! solo hacen falta trayectos con trazo, un relleno opcional e imágenes, y así no se agrega una
//! dependencia.

use crate::scene::{Page, Scene, decode_image, fit_page, num};
use crate::{Export, PageSize, PdfOptions};
use flate2::Compression;
use flate2::write::ZlibEncoder;
use std::io::Write as _;
use tlaku_core::Point;
use tlaku_core::bezpath::{BezPath, PathSeg};
use tlaku_doc::Document;
use tlaku_doc::affine::{self, Transform};
use tlaku_i18n::tr;

/// Puntos PDF por mm.
const K: f64 = 72.0 / 25.4;

pub fn export_pdf(
    doc: &Document,
    selection: Option<&[String]>,
    opts: &PdfOptions,
) -> Result<Export, String> {
    render(doc, selection, opts, false)
}

/// Exporta un `.ai`: el mismo dibujo vectorial a escala 1:1, pero como PDF 1.5 con una capa
/// (grupo de contenido opcional) por color de capa de Tlaku, que es como Illustrator guarda sus
/// capas dentro de un PDF. Illustrator abre cualquier PDF aunque lleve la extensión `.ai`; este
/// archivo **no** trae los datos privados de Illustrator (`PieceInfo`), así que lo abre como un
/// PDF con sus capas y no como un documento nativo: no conserva ajustes propios de Illustrator
/// (mesas de trabajo múltiples, símbolos, efectos...). Para un `.ai` nativo hay que guardar el
/// resultado desde Illustrator.
pub fn export_ai(
    doc: &Document,
    selection: Option<&[String]>,
    opts: &PdfOptions,
) -> Result<Export, String> {
    let mut out = render(doc, selection, opts, true)?;
    out.warnings.push(tr!("export.pdf.ai_is_pdf"));
    Ok(out)
}

fn render(
    doc: &Document,
    selection: Option<&[String]>,
    opts: &PdfOptions,
    ai: bool,
) -> Result<Export, String> {
    let mut scene = Scene::collect(doc, selection)?;
    let mut warnings = std::mem::take(&mut scene.warnings);
    let page = page_for(&scene, opts, &mut warnings);
    let stroke = if opts.stroke_mm.is_finite() && opts.stroke_mm > 0.0 {
        opts.stroke_mm
    } else {
        0.1
    };

    let mut content = String::new();
    content.push_str("1 J 1 j\n");
    content.push_str(&format!("{} w\n", num(stroke * K)));
    let mut images: Vec<Vec<u8>> = Vec::new(); // objetos de imagen ya escritos
    let mut layer_names: Vec<String> = Vec::new(); // capas del .ai (grupos de contenido opcional)
    let to_pt = |p: Point| format!("{} {}", num((p.x - page.x0) * K), num((p.y - page.y0) * K));
    for color in scene.layer_colors.clone() {
        let fills = opts.fill_layers && scene.fills(color);
        let (r, g, b) = (
            f64::from(color.0) / 255.0,
            f64::from(color.1) / 255.0,
            f64::from(color.2) / 255.0,
        );
        let mut colored = false;
        if ai {
            content.push_str(&format!("/OC /Lyr{} BDC\n", layer_names.len()));
            layer_names.push(
                scene
                    .layer_name(color)
                    .map_or_else(|| color.to_string(), str::to_string),
            );
        }
        for it in scene.in_layer(color) {
            if let Some(img) = it.image {
                if !opts.images {
                    continue;
                }
                match image_object(&img.source_path) {
                    Ok(obj) => {
                        let [w, h] = img.size_mm;
                        let pdf_from_doc: Transform = [K, 0.0, 0.0, K, -page.x0 * K, -page.y0 * K];
                        let m = affine::compose(
                            &pdf_from_doc,
                            &affine::compose(&it.obj.transform, &affine::scale(w, h)),
                        );
                        content.push_str(&format!(
                            "q {} cm /Im{} Do Q\n",
                            m.iter().map(|v| num(*v)).collect::<Vec<_>>().join(" "),
                            images.len()
                        ));
                        images.push(obj);
                    }
                    Err(e) => warnings.push(tr!("export.pdf.embed_failed", error = e)),
                }
                continue;
            }
            if !colored {
                colored = true;
                content.push_str(&format!(
                    "{} {} {} RG {} {} {} rg\n",
                    num(r),
                    num(g),
                    num(b),
                    num(r),
                    num(g),
                    num(b)
                ));
            }
            for path in &it.curves {
                path_ops(&mut content, path, &to_pt);
                // B* = rellena (par-impar) y traza; S = solo traza.
                content.push_str(if fills { "B*\n" } else { "S\n" });
            }
        }
        if ai {
            content.push_str("EMC\n");
        }
    }
    if !opts.images && scene.items.iter().any(|i| i.image.is_some()) {
        warnings.push(tr!("export.pdf.images_off"));
    }

    let bytes = write_pdf(&page, &content, &images, &layer_names, ai);
    Ok(Export {
        bytes,
        size_mm: [page.w, page.h],
        warnings,
    })
}

/// Página del PDF según la hoja pedida. Si el diseño no cabe en la hoja se usa una del tamaño
/// del diseño y se avisa.
fn page_for(scene: &Scene, opts: &PdfOptions, warnings: &mut Vec<String>) -> Page {
    let b = scene.bounds;
    let m = if opts.margin_mm.is_finite() {
        opts.margin_mm.max(0.0)
    } else {
        0.0
    };
    let (cw, ch) = (b.max.x - b.min.x, b.max.y - b.min.y);
    let sheet = match opts.page {
        PageSize::Fit => None,
        PageSize::A4 => Some((210.0, 297.0)),
        PageSize::A3 => Some((297.0, 420.0)),
        PageSize::Letter => Some((215.9, 279.4)),
        PageSize::Legal => Some((215.9, 355.6)),
    };
    let Some((sw, sh)) = sheet else {
        return fit_page(b, m, false);
    };
    // Se elige la orientación en la que cabe el diseño; con ambas, la que sigue su forma.
    let fits = |w: f64, h: f64| cw <= w - 2.0 * m && ch <= h - 2.0 * m;
    let (w, h) = if cw > ch { (sh, sw) } else { (sw, sh) };
    let (w, h) = if fits(w, h) {
        (w, h)
    } else if fits(h, w) {
        (h, w)
    } else {
        warnings.push(tr!(
            "export.pdf.page_too_small",
            w = format!("{cw:.0}"),
            h = format!("{ch:.0}")
        ));
        return fit_page(b, m, false);
    };
    // El diseño queda en la esquina superior izquierda, dentro del margen.
    let y1 = b.max.y + m;
    Page {
        x0: b.min.x - m,
        y0: y1 - h,
        w,
        h,
    }
}

fn path_ops(out: &mut String, path: &BezPath, to_pt: &impl Fn(Point) -> String) {
    let mut cur = Point::new(0.0, 0.0);
    for seg in &path.0 {
        match *seg {
            PathSeg::MoveTo { to } => {
                out.push_str(&format!("{} m\n", to_pt(to)));
                cur = to;
            }
            PathSeg::LineTo { to } => {
                out.push_str(&format!("{} l\n", to_pt(to)));
                cur = to;
            }
            PathSeg::QuadTo { ctrl, to } => {
                // Cuadrática elevada a cúbica.
                let third = |a: Point, b: Point| {
                    Point::new(a.x + (b.x - a.x) * 2.0 / 3.0, a.y + (b.y - a.y) * 2.0 / 3.0)
                };
                out.push_str(&format!(
                    "{} {} {} c\n",
                    to_pt(third(cur, ctrl)),
                    to_pt(third(to, ctrl)),
                    to_pt(to)
                ));
                cur = to;
            }
            PathSeg::CubicTo { ctrl1, ctrl2, to } => {
                out.push_str(&format!(
                    "{} {} {} c\n",
                    to_pt(ctrl1),
                    to_pt(ctrl2),
                    to_pt(to)
                ));
                cur = to;
            }
            PathSeg::Close => out.push_str("h\n"),
        }
    }
}

fn deflate(data: &[u8]) -> Vec<u8> {
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    // Escribir en un Vec no falla.
    let _ = enc.write_all(data);
    enc.finish().unwrap_or_default()
}

/// Objeto XObject de imagen (sin número): diccionario más datos Flate, gris o RGB, con la
/// transparencia compuesta sobre blanco.
fn image_object(path: &str) -> Result<Vec<u8>, String> {
    let raw = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let img = decode_image(path, &raw)?;
    let color = img.color().has_color();
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let over_white = |c: u8, a: u8| -> u8 {
        let (c, a) = (u32::from(c), u32::from(a));
        ((c * a + 255 * (255 - a) + 127) / 255) as u8
    };
    let mut samples = Vec::with_capacity((w * h) as usize * if color { 3 } else { 1 });
    for p in rgba.pixels() {
        let [r, g, b, a] = p.0;
        if color {
            samples.extend([over_white(r, a), over_white(g, a), over_white(b, a)]);
        } else {
            samples.push(over_white(r, a));
        }
    }
    let data = deflate(&samples);
    let mut obj = format!(
        "<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /{} \
         /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>\nstream\n",
        if color { "DeviceRGB" } else { "DeviceGray" },
        data.len()
    )
    .into_bytes();
    obj.extend(data);
    obj.extend(b"\nendstream");
    Ok(obj)
}

/// Texto PDF en UTF-16BE con marca de orden (`<FEFF...>`), que admite cualquier carácter.
fn pdf_text(s: &str) -> String {
    let mut out = String::from("<FEFF");
    for u in s.encode_utf16() {
        out.push_str(&format!("{u:04X}"));
    }
    out.push('>');
    out
}

fn write_pdf(
    page: &Page,
    content: &str,
    images: &[Vec<u8>],
    layers: &[String],
    ai: bool,
) -> Vec<u8> {
    // Los grupos de contenido opcional (capas) existen desde PDF 1.5.
    let mut out: Vec<u8> = if ai {
        b"%PDF-1.5\n%\xE2\xE3\xCF\xD3\n".to_vec()
    } else {
        b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec()
    };
    let mut offsets: Vec<usize> = Vec::new();
    let mut add = |out: &mut Vec<u8>, body: &[u8]| {
        offsets.push(out.len());
        let n = offsets.len();
        out.extend(format!("{n} 0 obj\n").as_bytes());
        out.extend(body);
        out.extend(b"\nendobj\n");
    };
    let first_image = 6;
    let first_layer = first_image + images.len();
    let xobjects: String = (0..images.len())
        .map(|i| format!("/Im{i} {} 0 R ", first_image + i))
        .collect();
    let refs: String = (0..layers.len())
        .map(|i| format!("{} 0 R ", first_layer + i))
        .collect();
    let properties: String = (0..layers.len())
        .map(|i| format!("/Lyr{i} {} 0 R ", first_layer + i))
        .collect();
    let catalog = if layers.is_empty() {
        "<< /Type /Catalog /Pages 2 0 R >>".to_string()
    } else {
        format!(
            "<< /Type /Catalog /Pages 2 0 R /OCProperties << /OCGs [{refs}] \
             /D << /Order [{refs}] /ON [{refs}] >> >> >>"
        )
    };
    add(&mut out, catalog.as_bytes());
    add(&mut out, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    let props = if layers.is_empty() {
        String::new()
    } else {
        format!("/Properties << {properties}>> ")
    };
    add(
        &mut out,
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents 4 0 R \
             /Resources << {props}/XObject << {xobjects}>> >> >>",
            num(page.w * K),
            num(page.h * K)
        )
        .as_bytes(),
    );
    let data = deflate(content.as_bytes());
    let mut stream = format!(
        "<< /Filter /FlateDecode /Length {} >>\nstream\n",
        data.len()
    )
    .into_bytes();
    stream.extend(data);
    stream.extend(b"\nendstream");
    add(&mut out, &stream);
    add(&mut out, b"<< /Producer (Tlaku) /Creator (Tlaku) >>");
    for img in images {
        add(&mut out, img);
    }
    for name in layers {
        add(
            &mut out,
            format!("<< /Type /OCG /Name {} >>", pdf_text(name)).as_bytes(),
        );
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes());
    for o in &offsets {
        out.extend(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len() + 1
        )
        .as_bytes(),
    );
    out
}
