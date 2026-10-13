//! Importación de proyectos de LightBurn (`.lbrn`, `.lbrn2`, XML) con capas y ajustes.
//!
//! El XML se lee con `tlaku-lbrn` (modelo neutro); aquí se traduce a objetos y capas de Tlaku:
//!
//! - **Formas**: `Rect` (con radio de esquina), `Ellipse`, `Path` (vértices y primitivas con Bézier),
//!   `Group` (con su `XForm`; los grupos anidados se aplanan a un grupo de Tlaku), `Text` (como
//!   `TextObject`, con el contorno que traiga el archivo; ver [`text`]) y `Bitmap` (la imagen
//!   incrustada se extrae a un archivo; ver [`bitmap`]). Lo demás se omite con un aviso que dice
//!   cuántas formas y de qué tipo.
//! - **Capas** (`CutSetting`): ajustes, color y orden; ver [`layers`] y la tabla de ajustes de
//!   `docs/spec/01-diseno-edicion.md` (D97).
//! - **Coordenadas**: LightBurn guarda milímetros con Y hacia arriba, como Tlaku, y las formas se
//!   mantienen en la misma posición de la cama: no se invierte ni se desplaza nada. La posición de
//!   cada forma está en su `XForm` (`x' = a·x + c·y + e`, `y' = b·x + d·y + f`, el mismo convenio
//!   que `DesignObject.transform`); `Rect`, `Ellipse` y `Bitmap` están centrados en su origen local.
//!   `MirrorX`/`MirrorY` del archivo (origen elegido al crearlo) no cambian las coordenadas
//!   guardadas y se ignoran (inferido).
//! - **Versiones**: `FormatVersion` 1 es `.lbrn2`; 0 o ausente, `.lbrn`. Una versión mayor se intenta
//!   leer igual con un aviso.

mod bitmap;
mod layers;
#[cfg(test)]
mod tests;
mod text;

use crate::import::{ImportOptions, new_id, object};
use std::collections::BTreeMap;
use std::path::Path;
use tlaku_core::Point;
use tlaku_core::bezpath::BezPath;
use tlaku_doc::affine::{self, Transform};
use tlaku_doc::{DesignObject, ImportResult, ObjectKind, VectorObject};
use tlaku_lbrn::{Shape, ShapeKind};

/// Versión de formato más alta que se conoce (`.lbrn2`).
const KNOWN_FORMAT_VERSION: u32 = 1;

/// Objetos y capas (con sus ajustes traducidos) del proyecto.
pub(crate) fn import(
    path: &Path,
    data: &[u8],
    opts: &ImportOptions,
) -> Result<ImportResult, String> {
    let project = tlaku_lbrn::parse(data).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut warnings = project.warnings.clone();
    if let Some(v) = project.format_version.filter(|v| *v > KNOWN_FORMAT_VERSION) {
        warnings.push(format!(
            "Versión de formato de LightBurn desconocida (FormatVersion {v}, guardado con {}): \
             se intenta leer, pero puede faltar información",
            if project.app_version.is_empty() {
                "una versión sin identificar"
            } else {
                &project.app_version
            }
        ));
    }

    let usage = layers::Usage::scan(&project.shapes);
    let mut conv = Converter {
        opts: *opts,
        layers: layers::Layers::new(&project, usage),
        images: bitmap::Extractor::new(path),
        objects: Vec::new(),
        omitted: BTreeMap::new(),
        warnings: Vec::new(),
        text_notes: BTreeMap::new(),
    };
    for shape in &project.shapes {
        conv.walk(shape, &affine::IDENTITY, None, false);
    }
    let Converter {
        layers,
        images,
        objects,
        omitted,
        warnings: conv_warnings,
        text_notes,
        ..
    } = conv;
    let (new_layers, layer_warnings) = layers.finish();
    warnings.extend(layer_warnings);
    warnings.extend(images.warnings);
    warnings.extend(conv_warnings);
    for (note, n) in text_notes {
        warnings.push(format!(
            "Texto: {note} (en {n} texto{})",
            if n == 1 { "" } else { "s" }
        ));
    }
    for (ty, n) in omitted {
        warnings.push(format!(
            "Se omitieron {n} forma{} de tipo «{ty}» (no soportada{})",
            if n == 1 { "" } else { "s" },
            if n == 1 { "" } else { "s" }
        ));
    }

    if objects.is_empty() {
        if new_layers.is_empty() {
            return Err(format!(
                "{}: el proyecto de LightBurn no contiene formas ni capas",
                path.display()
            ));
        }
        warnings.push(
            "El proyecto no contiene formas importables: solo se importaron las capas".into(),
        );
    }
    Ok(ImportResult {
        objects,
        new_layers,
        warnings,
    })
}

struct Converter {
    opts: ImportOptions,
    layers: layers::Layers,
    images: bitmap::Extractor,
    objects: Vec<DesignObject>,
    /// Tipo de forma omitida -> cuántas.
    omitted: BTreeMap<String, usize>,
    warnings: Vec<String>,
    /// Avisos de texto -> en cuántos textos.
    text_notes: BTreeMap<String, usize>,
}

impl Converter {
    /// Recorre una forma con la transformación `parent` ya compuesta de sus antecesores.
    /// `group` es el grupo de Tlaku al que pertenece (el más externo de LightBurn).
    fn walk(&mut self, shape: &Shape, parent: &Transform, group: Option<&str>, locked: bool) {
        let world = affine::compose(parent, &shape.xform);
        let locked = locked || shape.locked;
        if !affine::is_finite(&world) {
            *self
                .omitted
                .entry("con transformación inválida".into())
                .or_default() += 1;
            return;
        }
        let layer_index = shape.cut_index.unwrap_or(0);
        match &shape.kind {
            ShapeKind::Group(kids) => {
                let before = self.objects.len();
                let own = group.is_none().then(|| format!("grp-{}", new_id()));
                let id = group.map(str::to_string).or_else(|| own.clone());
                for k in kids {
                    self.walk(k, &world, id.as_deref(), locked);
                }
                // Un grupo de un solo objeto no agrupa nada.
                if own.is_some() && self.objects.len() - before < 2 {
                    for o in &mut self.objects[before..] {
                        o.group = None;
                    }
                }
            }
            ShapeKind::Rect {
                w,
                h,
                corner_radius,
            } => {
                let path = tlaku_lbrn::rect_path(*w, *h, *corner_radius);
                self.push_vector(path, layer_index, &world, group, locked);
            }
            ShapeKind::Ellipse { rx, ry } => {
                let path = tlaku_lbrn::ellipse_path(*rx, *ry);
                self.push_vector(path, layer_index, &world, group, locked);
            }
            ShapeKind::Path(path) => {
                self.push_vector(path.clone(), layer_index, &world, group, locked);
            }
            ShapeKind::Text(t) => {
                let color = self.layers.ensure(layer_index);
                let converted = text::convert(t, &world);
                for w in converted.warnings {
                    *self.text_notes.entry(w).or_default() += 1;
                }
                let mut obj = object(color, converted.kind);
                obj.transform = converted.transform;
                self.finish(obj, group, locked);
            }
            ShapeKind::Bitmap(b) => match self.images.extract(&b.data, b.w, b.h) {
                Ok(img) => {
                    let color = self.layers.image_layer(layer_index);
                    let mut obj = object(color, ObjectKind::Image(img));
                    // LightBurn centra la imagen en su origen; la de Tlaku va de (0,0) a (w,h).
                    obj.transform =
                        affine::compose(&world, &affine::translate(-b.w / 2.0, -b.h / 2.0));
                    self.finish(obj, group, locked);
                }
                Err(e) => {
                    self.warnings.push(format!(
                        "Se omitió una imagen incrustada de {:.1} x {:.1} mm: {e}",
                        b.w, b.h
                    ));
                }
            },
            ShapeKind::Unknown(ty) => {
                *self.omitted.entry(ty.clone()).or_default() += 1;
            }
        }
    }

    fn push_vector(
        &mut self,
        path: BezPath,
        layer_index: i32,
        world: &Transform,
        group: Option<&str>,
        locked: bool,
    ) {
        let color = self.layers.ensure(layer_index);
        let (paths, transform) = if self.opts.keep_curves {
            (vec![path], *world)
        } else {
            // Sin curvas: la geometría ya va en coordenadas de la cama y aplanada.
            let placed = path.map_points(|p| {
                let [x, y] = affine::apply(world, [p.x, p.y]);
                Point::new(x, y)
            });
            let flat = placed
                .flatten(self.opts.tolerance_mm)
                .iter()
                .map(BezPath::from)
                .collect();
            (flat, affine::IDENTITY)
        };
        let mut obj = object(color, ObjectKind::Vector(VectorObject { paths }));
        obj.transform = transform;
        self.finish(obj, group, locked);
    }

    fn finish(&mut self, mut obj: DesignObject, group: Option<&str>, locked: bool) {
        obj.group = group.map(str::to_string);
        obj.locked = locked;
        self.objects.push(obj);
    }
}
