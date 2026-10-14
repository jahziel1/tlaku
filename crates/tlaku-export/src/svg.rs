//! Exportación a SVG.
//!
//! La página mide lo que el diseño (mm reales: `width`/`height` en mm y `viewBox` en las mismas
//! unidades, así 1 unidad de usuario es 1 mm). La Y se invierte: el documento tiene Y hacia
//! arriba y el SVG hacia abajo. Cada capa del documento es un `<g>` con el color como trazo (y
//! como relleno en las capas de relleno si se pide); los objetos de un grupo van en un `<g>`
//! anidado. Las curvas Bézier se escriben tal cual (`C`/`Q`).

use crate::scene::{Item, Page, Scene, fit_page, image_bytes, num};
use crate::{Export, ImageHandling, SvgOptions};
use base64::Engine as _;
use tlaku_core::Rgb;
use tlaku_core::bezpath::{BezPath, PathSeg};
use tlaku_doc::Document;
use tlaku_doc::affine::{self, Transform};
use tlaku_i18n::tr;

pub fn export_svg(
    doc: &Document,
    selection: Option<&[String]>,
    opts: &SvgOptions,
) -> Result<Export, String> {
    let mut scene = Scene::collect(doc, selection)?;
    let page = fit_page(scene.bounds, opts.margin_mm, opts.keep_position);
    let stroke = if opts.stroke_mm.is_finite() && opts.stroke_mm > 0.0 {
        opts.stroke_mm
    } else {
        0.1
    };

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" \
         xmlns:xlink=\"http://www.w3.org/1999/xlink\" \
         xmlns:inkscape=\"http://www.inkscape.org/namespaces/inkscape\" \
         width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\">\n",
        w = num(page.w),
        h = num(page.h)
    ));
    let mut warnings = std::mem::take(&mut scene.warnings);
    for color in scene.layer_colors.clone() {
        let fills = opts.fill_layers && scene.fills(color);
        let label = scene
            .layer_name(color)
            .map_or_else(|| color.to_string(), str::to_string);
        out.push_str(&format!(
            "  <g id=\"capa-{hex}\" inkscape:groupmode=\"layer\" inkscape:label=\"{label}\" \
             fill=\"none\" stroke=\"{color}\" stroke-width=\"{sw}\" stroke-linecap=\"round\" \
             stroke-linejoin=\"round\">\n",
            hex = hex(color),
            label = escape(&label),
            sw = num(stroke)
        ));
        // Los objetos de un mismo grupo van juntos, donde aparece el primero.
        let items: Vec<&Item> = scene.in_layer(color).collect();
        let mut done = vec![false; items.len()];
        for i in 0..items.len() {
            if done[i] {
                continue;
            }
            match &items[i].obj.group {
                Some(g) => {
                    out.push_str(&format!("    <g id=\"grupo-{}\">\n", ident(g)));
                    for j in i..items.len() {
                        if !done[j] && items[j].obj.group.as_ref() == Some(g) {
                            done[j] = true;
                            item(
                                &mut out,
                                items[j],
                                &page,
                                fills,
                                opts,
                                "      ",
                                &mut warnings,
                            );
                        }
                    }
                    out.push_str("    </g>\n");
                }
                None => {
                    done[i] = true;
                    item(
                        &mut out,
                        items[i],
                        &page,
                        fills,
                        opts,
                        "    ",
                        &mut warnings,
                    );
                }
            }
        }
        out.push_str("  </g>\n");
    }
    out.push_str("</svg>\n");
    Ok(Export {
        bytes: out.into_bytes(),
        size_mm: [page.w, page.h],
        warnings,
    })
}

fn item(
    out: &mut String,
    it: &Item,
    page: &Page,
    fills: bool,
    opts: &SvgOptions,
    indent: &str,
    warnings: &mut Vec<String>,
) {
    if let Some(img) = it.image {
        match opts.images {
            ImageHandling::Skip => {
                warnings.push(tr!("export.svg.images_off"));
            }
            ImageHandling::Link | ImageHandling::Embed => {
                let href = if opts.images == ImageHandling::Embed {
                    match image_bytes(&img.source_path) {
                        Ok((bytes, mime)) => format!(
                            "data:{mime};base64,{}",
                            base64::engine::general_purpose::STANDARD.encode(bytes)
                        ),
                        Err(e) => {
                            warnings.push(tr!("export.svg.embed_failed", error = e));
                            return;
                        }
                    }
                } else {
                    img.source_path.clone()
                };
                // Del elemento (mm, Y hacia abajo desde su esquina superior) al documento y de ahí
                // a la página: la imagen local tiene Y hacia arriba.
                let [w, h] = img.size_mm;
                let to_local: Transform = [1.0, 0.0, 0.0, -1.0, 0.0, h];
                let m = affine::compose(
                    &page.down_matrix(),
                    &affine::compose(&it.obj.transform, &to_local),
                );
                out.push_str(&format!(
                    "{indent}<image id=\"{id}\" x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" \
                     preserveAspectRatio=\"none\" transform=\"matrix({m})\" xlink:href=\"{href}\"/>\n",
                    id = ident(&it.obj.id),
                    w = num(w),
                    h = num(h),
                    m = m.iter().map(|v| num(*v)).collect::<Vec<_>>().join(" "),
                    href = escape(&href),
                ));
            }
        }
        return;
    }
    let d: String = it
        .curves
        .iter()
        .map(|p| path_data(p, page))
        .collect::<Vec<_>>()
        .join(" ");
    if d.is_empty() {
        return;
    }
    let fill = if fills {
        format!(" fill=\"{}\" fill-rule=\"evenodd\"", it.obj.layer)
    } else {
        String::new()
    };
    out.push_str(&format!(
        "{indent}<path id=\"{}\"{fill} d=\"{d}\"/>\n",
        ident(&it.obj.id)
    ));
}

/// Datos de trayecto SVG (`d`) con las coordenadas ya en la página.
fn path_data(path: &BezPath, page: &Page) -> String {
    let p = |q: tlaku_core::Point| format!("{} {}", num(q.x - page.x0), num(page.y1() - q.y));
    let mut d = String::new();
    for seg in &path.0 {
        if !d.is_empty() {
            d.push(' ');
        }
        match *seg {
            PathSeg::MoveTo { to } => d.push_str(&format!("M{}", p(to))),
            PathSeg::LineTo { to } => d.push_str(&format!("L{}", p(to))),
            PathSeg::QuadTo { ctrl, to } => d.push_str(&format!("Q{} {}", p(ctrl), p(to))),
            PathSeg::CubicTo { ctrl1, ctrl2, to } => {
                d.push_str(&format!("C{} {} {}", p(ctrl1), p(ctrl2), p(to)));
            }
            PathSeg::Close => d.push('Z'),
        }
    }
    d
}

fn hex(c: Rgb) -> String {
    format!("{:02x}{:02x}{:02x}", c.0, c.1, c.2)
}

/// Identificador XML válido a partir de un texto cualquiera.
fn ident(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if !out
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
    {
        out.insert_str(0, "id-");
    }
    out
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
