//! Qué objetos entran al trabajo y dónde se colocan (`start_from` y `job_origin`).

use laser_core::{Bounds, Point, Polyline};
use laser_doc::affine::{self, Transform};
use laser_doc::{
    DesignObject, Document, ImageObject, JobOptions, LayerMode, LayerSettings, ObjectKind,
    StartFrom, bounds_of, union,
};
use std::collections::HashSet;

/// Imagen alineada a los ejes lista para rasterizar.
#[derive(Debug, Clone)]
pub(crate) struct PlacedImage {
    pub source_path: String,
    /// Esquina inferior izquierda, en mm.
    pub origin: Point,
    pub size_mm: (f64, f64),
    pub mirror_x: bool,
    pub mirror_y: bool,
}

/// Contenido de una capa del documento que entra al trabajo.
pub(crate) struct LayerWork<'a> {
    pub settings: &'a LayerSettings,
    pub paths: Vec<Polyline>,
    pub images: Vec<PlacedImage>,
}

impl LayerWork<'_> {
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty() && self.images.is_empty()
    }
}

pub(crate) struct Placement<'a> {
    pub layers: Vec<LayerWork<'a>>,
    /// Posición inicial de la cabeza para planificar y estimar.
    pub start: Point,
}

/// Punto del rectángulo `b` que elige `job_origin` (0..8, fila por fila desde arriba).
pub(crate) fn origin_point(b: Bounds, job_origin: u32) -> Point {
    let (col, row) = (f64::from(job_origin % 3), f64::from(job_origin / 3));
    Point::new(
        b.min.x + (b.max.x - b.min.x) * col / 2.0,
        b.max.y - (b.max.y - b.min.y) * row / 2.0,
    )
}

fn placed_image(
    obj: &DesignObject,
    img: &ImageObject,
    t: &Transform,
) -> Result<PlacedImage, String> {
    let [w, h] = img.size_mm;
    if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
        return Err(format!("imagen '{}': tamaño inválido {w}x{h} mm", obj.id));
    }
    if t[1] != 0.0 || t[2] != 0.0 {
        return Err(format!(
            "imagen '{}': solo se admiten escala, reflejo y traslado (sin rotación ni sesgo)",
            obj.id
        ));
    }
    let (x1, y1) = (t[4] + t[0] * w, t[5] + t[3] * h);
    let (sw, sh) = ((t[0] * w).abs(), (t[3] * h).abs());
    if !(sw > 0.0 && sh > 0.0) {
        return Err(format!(
            "imagen '{}': la escala deja la imagen sin área",
            obj.id
        ));
    }
    Ok(PlacedImage {
        source_path: img.source_path.clone(),
        origin: Point::new(t[4].min(x1), t[5].min(y1)),
        size_mm: (sw, sh),
        mirror_x: t[0] < 0.0,
        mirror_y: t[3] < 0.0,
    })
}

impl PlacedImage {
    fn rect(&self) -> Bounds {
        Bounds {
            min: self.origin,
            max: Point::new(
                self.origin.x + self.size_mm.0,
                self.origin.y + self.size_mm.1,
            ),
        }
    }
}

/// Elige los objetos que salen a la máquina, los agrupa por capa (en el orden del documento)
/// y los coloca según `options.start_from`. `current` es la posición de trabajo de la cabeza.
pub(crate) fn place<'a>(
    doc: &'a Document,
    options: &JobOptions,
    current: Option<Point>,
    warnings: &mut Vec<String>,
) -> Result<Placement<'a>, String> {
    if options.job_origin > 8 {
        return Err(format!("job_origin fuera de 0-8: {}", options.job_origin));
    }
    let mut seen = HashSet::new();
    if let Some(dup) = doc.objects.iter().find(|o| !seen.insert(o.id.as_str())) {
        return Err(format!("el documento repite el id de objeto '{}'", dup.id));
    }
    if let Some(sel) = &options.selection {
        for id in sel.iter().filter(|id| doc.object(id).is_none()) {
            warnings.push(format!(
                "la selección incluye '{id}', que no existe en el documento"
            ));
        }
    }

    let mut work: Vec<LayerWork<'a>> = doc
        .layers
        .iter()
        .map(|settings| LayerWork {
            settings,
            paths: Vec::new(),
            images: Vec::new(),
        })
        .collect();
    let selected: Option<HashSet<&str>> = options
        .selection
        .as_ref()
        .map(|sel| sel.iter().map(String::as_str).collect());
    for obj in &doc.objects {
        let wanted = selected
            .as_ref()
            .is_none_or(|sel| sel.contains(obj.id.as_str()));
        if !wanted || obj.hidden {
            continue;
        }
        let Some(idx) = doc.layers.iter().position(|l| l.color == obj.layer) else {
            return Err(format!(
                "el objeto '{}' usa la capa {}, que no existe en el documento",
                obj.id, obj.layer
            ));
        };
        let layer = &mut work[idx];
        if !layer.settings.output {
            continue;
        }
        if !affine::is_finite(&obj.transform) {
            return Err(format!(
                "el objeto '{}' tiene una transformación inválida",
                obj.id
            ));
        }
        match (&obj.kind, layer.settings.mode) {
            (ObjectKind::Vector(_), LayerMode::Line | LayerMode::Fill) => {
                layer
                    .paths
                    .extend(obj.polylines().into_iter().filter(|p| !p.points.is_empty()));
            }
            (ObjectKind::Image(img), LayerMode::Image) => {
                match placed_image(obj, img, &obj.transform) {
                    Ok(p) => layer.images.push(p),
                    Err(e) => warnings.push(format!("{e}; se ignora")),
                }
            }
            (kind, mode) => {
                let what = match kind {
                    ObjectKind::Vector(_) => "vectorial",
                    ObjectKind::Image(_) => "de imagen",
                };
                warnings.push(format!(
                    "objeto '{}' {what} ignorado: la capa {} es de modo {}",
                    obj.id,
                    layer.settings.color,
                    mode.name()
                ));
            }
        }
    }
    work.retain(|l| !l.is_empty());

    let target = match options.start_from {
        StartFrom::Absolute => None,
        StartFrom::CurrentPosition => Some(current.ok_or(
            "falta la posición actual de la máquina (conéctala para trabajar desde la posición actual)",
        )?),
        StartFrom::UserOrigin => {
            let [x, y] = options
                .user_origin
                .ok_or("falta el origen de usuario (user_origin)")?;
            Some(Point::new(x, y))
        }
    };
    let Some(target) = target else {
        return Ok(Placement {
            layers: work,
            start: Point::new(0.0, 0.0),
        });
    };
    if !(target.x.is_finite() && target.y.is_finite()) {
        return Err("el punto de inicio del trabajo no es un número".into());
    }

    let rects = work.iter().flat_map(|l| {
        let paths = bounds_of(&l.paths);
        let images = l.images.iter().map(PlacedImage::rect);
        paths.into_iter().chain(images)
    });
    let Some(all) = rects.reduce(union) else {
        return Ok(Placement {
            layers: work,
            start: target,
        });
    };
    let anchor = origin_point(all, options.job_origin);
    let (dx, dy) = (target.x - anchor.x, target.y - anchor.y);
    for layer in &mut work {
        for p in layer.paths.iter_mut().flat_map(|p| &mut p.points) {
            p.x += dx;
            p.y += dy;
        }
        for img in &mut layer.images {
            img.origin.x += dx;
            img.origin.y += dy;
        }
    }
    Ok(Placement {
        layers: work,
        start: target,
    })
}
