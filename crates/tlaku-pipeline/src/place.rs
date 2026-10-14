//! Qué objetos entran al trabajo y dónde se colocan (`start_from` y `job_origin`).

use crate::start;
use std::collections::{HashMap, HashSet};
use tlaku_core::{Bounds, Point, Polyline, Rgb};
use tlaku_doc::affine::{self, Transform};
use tlaku_doc::{
    DesignObject, Document, ImageObject, JobOptions, LayerMode, LayerSettings, ObjectKind,
    StartFrom, bounds_of, union,
};
use tlaku_i18n::tr;

/// Imagen alineada a los ejes lista para rasterizar.
#[derive(Debug, Clone)]
pub(crate) struct PlacedImage {
    pub source_path: String,
    /// Esquina inferior izquierda, en mm.
    pub origin: Point,
    pub size_mm: (f64, f64),
    pub mirror_x: bool,
    pub mirror_y: bool,
    /// Máscara ya en coordenadas del documento (mm); vacía = la imagen completa.
    pub mask: Vec<Polyline>,
    /// Giro o deformación que la imagen tiene además de su colocación (Print & Cut con giro). Con
    /// `Some`, `origin` y `size_mm` son la caja que ocupa ya transformada, no la imagen original.
    pub warp: Option<ImageWarp>,
}

/// Cómo se lleva una imagen alineada a los ejes a donde la deja la transformación de Print &
/// Cut. La imagen se vuelve a muestrear sobre la caja `origin`/`size_mm` de [`PlacedImage`].
#[derive(Debug, Clone)]
pub(crate) struct ImageWarp {
    /// Esquina inferior izquierda de la imagen sin transformar, en mm.
    pub src_origin: Point,
    /// Tamaño de la imagen sin transformar, en mm.
    pub src_size: (f64, f64),
    /// De las coordenadas de la imagen sin transformar a las finales.
    pub transform: Transform,
}

impl ImageWarp {
    /// Las cuatro esquinas de la imagen ya transformada, en orden antihorario desde la inferior
    /// izquierda.
    pub fn corners(&self) -> [Point; 4] {
        let (o, (w, h)) = (self.src_origin, self.src_size);
        [
            Point::new(o.x, o.y),
            Point::new(o.x + w, o.y),
            Point::new(o.x + w, o.y + h),
            Point::new(o.x, o.y + h),
        ]
        .map(|c| {
            let [x, y] = affine::apply(&self.transform, [c.x, c.y]);
            Point::new(x, y)
        })
    }
}

/// Máximo de sub-capas de una capa (el de la ficha F30: hasta 11).
pub(crate) const MAX_SUB_LAYERS: usize = 11;

/// De qué objeto sale una trayectoria de una capa y qué claves de orden y potencia lleva.
/// `LayerWork::meta[i]` describe a `LayerWork::paths[i]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PathMeta {
    /// Posición del objeto en `Document::objects`.
    pub object: usize,
    /// Grupo del objeto, numerado por orden de aparición en el documento (global, no por capa).
    /// Con grupos anidados (`"g2/g1"`, de afuera hacia adentro) cuenta el de más afuera, igual
    /// que al seleccionar: un grupo es una sola unidad.
    pub group: Option<usize>,
    /// Id de grupo para el relleno por grupos (`tlaku_fill::fill_with_groups`): el de `group`, o
    /// uno propio del objeto si no tiene grupo. Igual para todas las trayectorias de un objeto.
    pub fill_group: u32,
    /// `DesignObject::priority`; sin valor = 0 (automática).
    pub priority: i32,
    /// `DesignObject::power_scale` en % de la potencia de la capa; sin valor = 100.
    pub power_scale: f64,
    /// El usuario fijó el punto de inicio o el sentido de esta trayectoria (`DesignObject::start`):
    /// el planificador no los cambia. Ver `start.rs`.
    pub locked: bool,
}

impl PathMeta {
    /// Sin origen conocido: sin grupo, prioridad 0 y potencia completa.
    pub const NONE: PathMeta = PathMeta {
        object: usize::MAX,
        group: None,
        priority: 0,
        fill_group: 0,
        power_scale: 100.0,
        locked: false,
    };
}

/// Contenido de una operación del trabajo: una capa del documento o una de sus sub-capas.
pub(crate) struct LayerWork<'a> {
    pub settings: &'a LayerSettings,
    /// Color de la capa del documento (el de una sub-capa se ignora: usa el de su capa).
    pub color: Rgb,
    pub paths: Vec<Polyline>,
    /// Origen de cada trayectoria de `paths`, en el mismo orden.
    pub meta: Vec<PathMeta>,
    /// Puentes manuales de los objetos de esta operación (`DesignObject::manual_tabs`), por
    /// posición del objeto en `Document::objects`, ya en el mismo sistema que `paths`.
    pub manual_tabs: HashMap<usize, Vec<Point>>,
    pub images: Vec<PlacedImage>,
    /// Capa del documento de la que sale (posición en `Document::layers`).
    pub parent: usize,
    /// `None` en la capa principal; `Some(k)` (desde 1) en su k-ésima sub-capa.
    pub sub: Option<usize>,
    /// Cuántas sub-capas tiene la capa del documento (0 = una sola operación).
    pub sub_total: usize,
}

impl LayerWork<'_> {
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty() && self.images.is_empty()
    }

    /// `(posición, total)` entre las operaciones de su capa, si ésta tiene sub-capas
    /// (la principal es `1/total`).
    pub fn position(&self) -> Option<(u32, u32)> {
        (self.sub_total > 0).then(|| (self.sub.unwrap_or(0) as u32 + 1, self.sub_total as u32 + 1))
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
        return Err(tr!(
            "pipeline.place.image_bad_size",
            id = obj.id,
            w = w,
            h = h
        ));
    }
    if t[1] != 0.0 || t[2] != 0.0 {
        return Err(tr!("pipeline.place.image_transform", id = obj.id));
    }
    let (x1, y1) = (t[4] + t[0] * w, t[5] + t[3] * h);
    let (sw, sh) = ((t[0] * w).abs(), (t[3] * h).abs());
    if !(sw > 0.0 && sh > 0.0) {
        return Err(tr!("pipeline.place.image_no_area", id = obj.id));
    }
    Ok(PlacedImage {
        source_path: img.source_path.clone(),
        origin: Point::new(t[4].min(x1), t[5].min(y1)),
        size_mm: (sw, sh),
        mirror_x: t[0] < 0.0,
        mirror_y: t[3] < 0.0,
        mask: img
            .mask
            .iter()
            .flat_map(|path| {
                path.map_points(|p| {
                    let [x, y] = affine::apply(t, [p.x, p.y]);
                    Point::new(x, y)
                })
                .flatten(0.01)
            })
            .collect(),
        warp: None,
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

/// Agrega el objeto a una operación según su tipo y el modo de ésta. `report` es `true` solo
/// en la capa principal: los avisos propios del objeto (texto sin contorno, imagen inválida) no
/// se repiten en cada sub-capa. Devuelve `false` si el objeto no es de un tipo que el modo
/// admita (el aviso lo da quien llama).
fn add_object(
    unit: &mut LayerWork,
    obj: &DesignObject,
    meta: PathMeta,
    report: bool,
    warnings: &mut Vec<String>,
) -> bool {
    let mode = unit.settings.mode;
    let push_paths = |unit: &mut LayerWork| {
        let mut paths: Vec<Polyline> = obj
            .polylines()
            .into_iter()
            .filter(|p| !p.points.is_empty())
            .collect();
        // Punto de inicio y sentido elegidos a mano: se aplican aquí y el planificador los respeta.
        let locked = start::lock_paths(&mut paths, obj.start.as_ref());
        for (path, locked) in paths.into_iter().zip(locked) {
            unit.paths.push(path);
            unit.meta.push(PathMeta { locked, ..meta });
        }
    };
    match (&obj.kind, mode) {
        (ObjectKind::Vector(_), LayerMode::Line | LayerMode::Fill) => push_paths(unit),
        (ObjectKind::Image(img), LayerMode::Image) => {
            match placed_image(obj, img, &obj.transform) {
                Ok(p) => unit.images.push(p),
                Err(e) if report => warnings.push(tr!("pipeline.place.ignored", error = e)),
                Err(_) => {}
            }
        }
        (ObjectKind::Text(t), LayerMode::Line | LayerMode::Fill) => {
            // El pipeline solo corta el contorno ya generado; no interpreta fuentes.
            if t.outline.is_empty() {
                if report {
                    warnings.push(tr!("pipeline.place.text_no_outline", id = obj.id));
                }
            } else {
                push_paths(unit);
            }
        }
        _ => return false,
    }
    true
}

/// Grupo de más afuera de una cadena `"g2/g1"` (de afuera hacia adentro); `None` si está vacía.
fn outer_group(group: &str) -> Option<&str> {
    group.split('/').find(|g| !g.is_empty())
}

/// Aviso de un objeto que su capa no admite por el modo (vectorial, de imagen o de texto).
fn kind_ignored(obj: &DesignObject, color: Rgb, mode: &str) -> String {
    match obj.kind {
        ObjectKind::Vector(_) => tr!(
            "pipeline.place.vector_ignored",
            id = obj.id,
            layer = color,
            mode = mode
        ),
        ObjectKind::Image(_) => tr!(
            "pipeline.place.image_ignored",
            id = obj.id,
            layer = color,
            mode = mode
        ),
        ObjectKind::Text(_) => tr!(
            "pipeline.place.text_ignored",
            id = obj.id,
            layer = color,
            mode = mode
        ),
    }
}

/// Reparte los objetos (todos, o solo los de `selected`) entre las operaciones de cada capa del
/// documento: la principal y luego sus sub-capas, que trabajan sobre los mismos objetos.
fn collect<'a>(
    doc: &'a Document,
    selected: Option<&HashSet<&str>>,
    groups: &HashMap<&str, usize>,
    warnings: &mut Vec<String>,
) -> Result<Vec<LayerWork<'a>>, String> {
    // Posición de la primera operación de cada capa del documento.
    let mut first = Vec::with_capacity(doc.layers.len());
    let mut work: Vec<LayerWork<'a>> = Vec::new();
    for (parent, layer) in doc.layers.iter().enumerate() {
        if layer.sub_layers.len() > MAX_SUB_LAYERS {
            return Err(tr!(
                "pipeline.place.too_many_sublayers",
                layer = layer.color,
                n = layer.sub_layers.len(),
                max = MAX_SUB_LAYERS
            ));
        }
        first.push(work.len());
        let total = layer.sub_layers.len();
        let ops = std::iter::once((layer, None)).chain(
            layer
                .sub_layers
                .iter()
                .enumerate()
                .map(|(k, sub)| (sub, Some(k + 1))),
        );
        for (settings, sub) in ops {
            work.push(LayerWork {
                settings,
                color: layer.color,
                paths: Vec::new(),
                meta: Vec::new(),
                manual_tabs: HashMap::new(),
                images: Vec::new(),
                parent,
                sub,
                sub_total: total,
            });
        }
    }
    // Objetos que una sub-capa no puede tomar (por su modo), contados por operación.
    let mut skipped = vec![0usize; work.len()];
    for (idx, obj) in doc.objects.iter().enumerate() {
        let wanted = selected.is_none_or(|sel| sel.contains(obj.id.as_str()));
        if !wanted || obj.hidden {
            continue;
        }
        let Some(parent) = doc.layers.iter().position(|l| l.color == obj.layer) else {
            return Err(tr!(
                "pipeline.place.missing_layer",
                id = obj.id,
                layer = obj.layer
            ));
        };
        // La salida de la capa manda sobre la de sus sub-capas.
        if !doc.layers[parent].output {
            continue;
        }
        if !affine::is_finite(&obj.transform) {
            return Err(tr!("pipeline.place.bad_transform", id = obj.id));
        }
        let power_scale = obj.power_scale.unwrap_or(100.0);
        if !(power_scale.is_finite() && power_scale >= 0.0) {
            return Err(tr!(
                "pipeline.place.bad_power_scale",
                id = obj.id,
                value = power_scale
            ));
        }
        let group = obj
            .group
            .as_deref()
            .and_then(outer_group)
            .and_then(|g| groups.get(g).copied());
        let meta = PathMeta {
            object: idx,
            group,
            // Los objetos sin grupo numeran después de los grupos, sin chocar con ellos.
            fill_group: group.unwrap_or(groups.len() + idx) as u32,
            priority: obj.priority.unwrap_or(0),
            power_scale,
            locked: false,
        };
        let ops = first[parent]..first[parent] + 1 + doc.layers[parent].sub_layers.len();
        // Copias del arreglo virtual: el mismo objeto con otra transformación, sin existir en el documento.
        let copies = match obj.virtual_copies() {
            Ok(list) => list,
            Err(e) => {
                warnings.push(tr!("pipeline.place.copies_ignored", error = e));
                Vec::new()
            }
        };
        let mut copy_obj = (!copies.is_empty()).then(|| obj.clone());
        for u in ops {
            let unit = &mut work[u];
            if !obj.manual_tabs.is_empty() {
                let points = obj.manual_tabs.iter().map(|p| Point::new(p[0], p[1]));
                unit.manual_tabs.insert(idx, points.collect());
            }
            if u != first[parent] && !unit.settings.output {
                continue;
            }
            let main = u == first[parent];
            let added = add_object(unit, obj, meta, main, warnings);
            if added && let Some(copy) = copy_obj.as_mut() {
                // Los avisos de las copias son los del original: se da uno solo.
                let mut sink = Vec::new();
                for t in &copies {
                    copy.transform = affine::compose(t, &obj.transform);
                    add_object(unit, copy, meta, main, &mut sink);
                }
                if let Some(first_warning) = sink.first() {
                    warnings.push(tr!("pipeline.place.in_copies", warning = first_warning));
                }
            }
            if !added {
                if main {
                    warnings.push(kind_ignored(
                        obj,
                        unit.settings.color,
                        unit.settings.mode.name(),
                    ));
                } else {
                    skipped[u] += 1;
                }
            }
        }
    }
    for (unit, n) in work.iter().zip(&skipped) {
        if *n > 0 {
            warnings.push(tr!(
                "pipeline.place.sublayer_skipped",
                layer = unit.settings.color,
                sub = unit.sub.unwrap_or(0),
                n = n,
                mode = unit.settings.mode.name()
            ));
        }
    }
    work.retain(|l| !l.is_empty());
    Ok(work)
}

fn union_all(work: &[LayerWork]) -> Option<Bounds> {
    work.iter()
        .flat_map(|l| {
            let paths = bounds_of(&l.paths);
            let images = l.images.iter().map(PlacedImage::rect);
            paths.into_iter().chain(images)
        })
        .reduce(union)
}

/// Elige los objetos que salen a la máquina, los agrupa por capa (en el orden del documento,
/// cada capa seguida de sus sub-capas) y los coloca según `options.start_from`. `current` es la
/// posición de trabajo de la cabeza.
///
/// Con `selection`, el origen del trabajo (`job_origin`) se calcula con la caja de **todo el
/// diseño con salida** y la selección se corta en el lugar que le toca respecto a él; con
/// `use_selection_origin` se calcula con la caja de la selección. En modo absoluto no hay
/// origen que calcular.
pub(crate) fn place<'a>(
    doc: &'a Document,
    options: &JobOptions,
    current: Option<Point>,
    warnings: &mut Vec<String>,
) -> Result<Placement<'a>, String> {
    if options.job_origin > 8 {
        return Err(tr!(
            "pipeline.place.bad_job_origin",
            value = options.job_origin
        ));
    }
    let mut seen = HashSet::new();
    if let Some(dup) = doc.objects.iter().find(|o| !seen.insert(o.id.as_str())) {
        return Err(tr!("pipeline.place.duplicate_id", id = dup.id));
    }
    if let Some(sel) = &options.selection {
        for id in sel.iter().filter(|id| doc.object(id).is_none()) {
            warnings.push(tr!("pipeline.place.selection_missing", id = id));
        }
    }
    if options.use_selection_origin {
        if options.selection.is_none() {
            warnings.push(tr!("pipeline.place.selection_origin_no_selection"));
        } else if options.start_from == StartFrom::Absolute {
            warnings.push(tr!("pipeline.place.selection_origin_absolute"));
        }
    }

    // Número de cada grupo por orden de aparición en el documento.
    let mut groups: HashMap<&str, usize> = HashMap::new();
    for g in doc
        .objects
        .iter()
        .filter_map(|o| o.group.as_deref().and_then(outer_group))
    {
        let next = groups.len();
        groups.entry(g).or_insert(next);
    }
    let selected: Option<HashSet<&str>> = options
        .selection
        .as_ref()
        .map(|sel| sel.iter().map(String::as_str).collect());
    let mut work = collect(doc, selected.as_ref(), &groups, warnings)?;
    // Tiling: solo el tramo pedido, recortado y desplazado (ver `tile.rs`).
    if let Some(tile) = &options.tile {
        crate::tile::apply(&mut work, tile, warnings)?;
    }
    if work.is_empty() && selected.is_some() {
        warnings.push(tr!("pipeline.place.selection_empty"));
    }

    let target = match options.start_from {
        StartFrom::Absolute => None,
        StartFrom::CurrentPosition => {
            Some(current.ok_or_else(|| tr!("pipeline.place.no_current_position"))?)
        }
        StartFrom::UserOrigin => {
            let [x, y] = options
                .user_origin
                .ok_or_else(|| tr!("pipeline.place.no_user_origin"))?;
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
        return Err(tr!("pipeline.place.bad_start_point"));
    }

    // Caja que ancla el origen: la de la selección, o la de todo el diseño con salida.
    let anchor_box = if selected.is_some() && !options.use_selection_origin {
        let mut scratch = Vec::new();
        union_all(&collect(doc, None, &groups, &mut scratch)?)
    } else {
        union_all(&work)
    };
    let Some(all) = anchor_box else {
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
        for p in layer.manual_tabs.values_mut().flatten() {
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

#[cfg(test)]
mod virtual_array_tests {
    use super::*;
    use tlaku_core::bezpath::BezPath;
    use tlaku_doc::VectorObject;
    use tlaku_doc::VirtualArray;

    const RED: Rgb = Rgb(255, 0, 0);

    fn square(id: &str, x: f64, y: f64, s: f64) -> DesignObject {
        DesignObject {
            id: id.into(),
            layer: RED,
            transform: affine::IDENTITY,
            kind: ObjectKind::Vector(VectorObject::new(vec![BezPath::from(&Polyline {
                points: vec![
                    Point::new(x, y),
                    Point::new(x + s, y),
                    Point::new(x + s, y + s),
                    Point::new(x, y + s),
                ],
                closed: true,
            })])),
            locked: false,
            hidden: false,
            group: None,
            priority: None,
            power_scale: None,
            shape: None,
            start: None,
            manual_tabs: Vec::new(),
            array: None,
        }
    }

    fn doc_of(objects: Vec<DesignObject>) -> Document {
        Document {
            objects,
            layers: vec![LayerSettings::for_color(RED)],
            ..Document::default()
        }
    }

    #[test]
    fn place_expands_the_copies_into_the_layer() {
        let mut o = square("a", 0.0, 0.0, 10.0);
        o.array = Some(VirtualArray::Grid {
            cols: 2,
            rows: 2,
            dx: 5.0,
            dy: 5.0,
        });
        let doc = doc_of(vec![o, square("b", 100.0, 0.0, 10.0)]);
        let mut warnings = Vec::new();
        let placed = place(&doc, &JobOptions::default(), None, &mut warnings).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        let work = &placed.layers[0];
        // 4 piezas del arreglo + el objeto suelto.
        assert_eq!(work.paths.len(), 5);
        assert_eq!(work.meta.iter().filter(|m| m.object == 0).count(), 4);
        let b = bounds_of(&work.paths).unwrap();
        assert!((b.max.x - 110.0).abs() < 1e-9 && (b.max.y - 25.0).abs() < 1e-9);
    }

    #[test]
    fn copies_follow_the_object_transform_and_the_selection() {
        let mut o = square("a", 0.0, 0.0, 10.0);
        o.transform = affine::translate(50.0, 0.0);
        o.array = Some(VirtualArray::Grid {
            cols: 3,
            rows: 1,
            dx: 0.0,
            dy: 0.0,
        });
        let doc = doc_of(vec![o, square("b", 0.0, 100.0, 10.0)]);
        let options = JobOptions {
            selection: Some(vec!["a".into()]),
            ..JobOptions::default()
        };
        let mut warnings = Vec::new();
        let placed = place(&doc, &options, None, &mut warnings).unwrap();
        let b = bounds_of(&placed.layers[0].paths).unwrap();
        assert_eq!(placed.layers[0].paths.len(), 3);
        assert!((b.min.x - 50.0).abs() < 1e-9 && (b.max.x - 80.0).abs() < 1e-9);
    }

    #[test]
    fn a_broken_array_warns_and_keeps_the_original() {
        let mut o = square("a", 0.0, 0.0, 10.0);
        o.array = Some(VirtualArray::Grid {
            cols: 0,
            rows: 0,
            dx: 0.0,
            dy: 0.0,
        });
        let doc = doc_of(vec![o]);
        let mut warnings = Vec::new();
        let placed = place(&doc, &JobOptions::default(), None, &mut warnings).unwrap();
        assert_eq!(placed.layers[0].paths.len(), 1);
        assert!(warnings.iter().any(|w| w.contains("arreglo virtual")));
    }
}
