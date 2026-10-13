//! Lo que comparten todos los exportadores: qué objetos entran y cuánto mide el dibujo.

use std::path::Path;
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Bounds, Rgb};
use tlaku_doc::affine::Transform;
use tlaku_doc::{DesignObject, Document, ImageObject, LayerMode, ObjectKind, union};

/// Un objeto a exportar.
pub(crate) struct Item<'a> {
    pub obj: &'a DesignObject,
    /// Curvas en coordenadas del documento. Vacío para una imagen.
    pub curves: Vec<BezPath>,
    pub image: Option<&'a ImageObject>,
}

pub(crate) struct Scene<'a> {
    pub items: Vec<Item<'a>>,
    pub bounds: Bounds,
    pub warnings: Vec<String>,
    /// Colores de capa con objetos, en el orden de la lista de capas del documento (los que no
    /// están en ella, al final por aparición).
    pub layer_colors: Vec<Rgb>,
    doc: &'a Document,
}

impl<'a> Scene<'a> {
    /// Objetos visibles de `selection` (o de todo el documento si es `None`).
    pub fn collect(doc: &'a Document, selection: Option<&[String]>) -> Result<Self, String> {
        let mut items = Vec::new();
        let mut warnings = Vec::new();
        let mut bounds: Option<Bounds> = None;
        let mut hidden = 0usize;
        for obj in &doc.objects {
            if let Some(sel) = selection
                && !sel.contains(&obj.id)
            {
                continue;
            }
            let layer_hidden = doc.layer(obj.layer).is_some_and(|l| !l.visible);
            if obj.hidden || layer_hidden {
                hidden += 1;
                continue;
            }
            let item = match &obj.kind {
                ObjectKind::Vector(_) => Item {
                    obj,
                    curves: obj.curves(),
                    image: None,
                },
                ObjectKind::Text(t) => {
                    if t.outline.is_empty() {
                        warnings.push(format!(
                            "El texto «{}» no tiene contorno generado y se omitió.",
                            t.text.lines().next().unwrap_or("").trim()
                        ));
                        continue;
                    }
                    Item {
                        obj,
                        curves: obj.curves(),
                        image: None,
                    }
                }
                ObjectKind::Image(img) => Item {
                    obj,
                    curves: Vec::new(),
                    image: Some(img),
                },
            };
            let Some(b) = obj.bounds() else { continue };
            bounds = Some(bounds.map_or(b, |a| union(a, b)));
            items.push(item);
        }
        if hidden > 0 {
            warnings.push(format!(
                "{hidden} objeto(s) oculto(s) o de capas ocultas no se exportaron."
            ));
        }
        let Some(bounds) = bounds else {
            return Err(if selection.is_some() {
                "La selección no tiene nada que exportar.".into()
            } else {
                "El diseño no tiene nada que exportar.".into()
            });
        };
        let mut layer_colors: Vec<Rgb> = doc
            .layers
            .iter()
            .map(|l| l.color)
            .filter(|c| items.iter().any(|i| i.obj.layer == *c))
            .collect();
        for i in &items {
            if !layer_colors.contains(&i.obj.layer) {
                layer_colors.push(i.obj.layer);
            }
        }
        Ok(Self {
            items,
            bounds,
            warnings,
            layer_colors,
            doc,
        })
    }

    /// `true` si la capa de ese color rellena sus formas (modo relleno).
    pub fn fills(&self, color: Rgb) -> bool {
        self.doc
            .layer(color)
            .is_some_and(|l| l.mode == LayerMode::Fill)
    }

    pub fn layer_name(&self, color: Rgb) -> Option<&str> {
        self.doc
            .layer(color)
            .map(|l| l.name.as_str())
            .filter(|n| !n.trim().is_empty())
    }

    /// Objetos de una capa, en el orden del documento.
    pub fn in_layer(&self, color: Rgb) -> impl Iterator<Item = &Item<'a>> {
        self.items.iter().filter(move |i| i.obj.layer == color)
    }
}

/// Rectángulo de la página en coordenadas del documento (esquina inferior izquierda y tamaño).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Page {
    pub x0: f64,
    pub y0: f64,
    pub w: f64,
    pub h: f64,
}

impl Page {
    /// Esquina superior (en el documento, Y hacia arriba).
    pub fn y1(&self) -> f64 {
        self.y0 + self.h
    }

    /// Del documento al lienzo con Y hacia abajo (SVG, imagen), mm desde la esquina superior
    /// izquierda de la página: `x' = x - x0`, `y' = y1 - y`.
    pub fn down_matrix(&self) -> Transform {
        [1.0, 0.0, 0.0, -1.0, -self.x0, self.y1()]
    }
}

/// Página ajustada al diseño más `margin_mm`; con `keep_position` incluye también el origen del
/// documento (las coordenadas se conservan). Mide al menos 1 mm por lado.
pub(crate) fn fit_page(bounds: Bounds, margin_mm: f64, keep_position: bool) -> Page {
    let m = if margin_mm.is_finite() {
        margin_mm.max(0.0)
    } else {
        0.0
    };
    let (mut x0, mut y0) = (bounds.min.x - m, bounds.min.y - m);
    let (x1, y1) = (bounds.max.x + m, bounds.max.y + m);
    if keep_position {
        x0 = x0.min(0.0);
        y0 = y0.min(0.0);
    }
    Page {
        x0,
        y0,
        w: (x1 - x0).max(1.0),
        h: (y1 - y0).max(1.0),
    }
}

/// Número para un archivo de texto: hasta 4 decimales, sin ceros sobrantes ni `-0`.
pub(crate) fn num(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s.into()
    }
}

/// Subtrayectos de un `BezPath` como trayectos independientes (cada uno empieza con `MoveTo`).
pub(crate) fn subpaths(path: &BezPath) -> Vec<BezPath> {
    use tlaku_core::bezpath::PathSeg;
    let mut out: Vec<BezPath> = Vec::new();
    for seg in &path.0 {
        match seg {
            PathSeg::MoveTo { .. } => out.push(BezPath(vec![*seg])),
            _ => match out.last_mut() {
                Some(p) => p.0.push(*seg),
                None => {
                    // Un trazo que no empieza con MoveTo no se puede ubicar.
                }
            },
        }
    }
    out.retain(|p| p.0.len() > 1);
    out
}

/// Bytes de una imagen y su tipo MIME si ya está en un formato que los visores abren
/// directamente (PNG, JPEG); si no, la decodifica y la vuelve a escribir como PNG.
pub(crate) fn image_bytes(path: &str) -> Result<(Vec<u8>, &'static str), String> {
    let p = Path::new(path);
    let raw = std::fs::read(p).map_err(|e| format!("{path}: {e}"))?;
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "png" => return Ok((raw, "image/png")),
        "jpg" | "jpeg" => return Ok((raw, "image/jpeg")),
        _ => {}
    }
    let img = decode_image(path, &raw)?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| format!("{path}: {e}"))?;
    Ok((out.into_inner(), "image/png"))
}

/// Decodifica una imagen con el formato que sugiere la extensión (TGA no tiene firma).
pub(crate) fn decode_image(path: &str, raw: &[u8]) -> Result<image::DynamicImage, String> {
    let hint = image::ImageFormat::from_path(path).ok();
    hint.and_then(|f| image::load_from_memory_with_format(raw, f).ok())
        .map_or_else(|| image::load_from_memory(raw), Ok)
        .map_err(|e| format!("{path}: {e}"))
}
