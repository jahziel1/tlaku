//! Qué objetos entran al trabajo y dónde se colocan (`start_from` y `job_origin`).

use std::collections::{HashMap, HashSet};
use tlaku_core::{Bounds, Point, Polyline, Rgb};
use tlaku_doc::affine::{self, Transform};
use tlaku_doc::{
    DesignObject, Document, ImageObject, JobOptions, LayerMode, LayerSettings, ObjectKind,
    StartFrom, bounds_of, union,
};

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
}

impl PathMeta {
    /// Sin origen conocido: sin grupo, prioridad 0 y potencia completa.
    pub const NONE: PathMeta = PathMeta {
        object: usize::MAX,
        group: None,
        priority: 0,
        fill_group: 0,
        power_scale: 100.0,
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

    /// Un id de grupo por trayectoria, para `tlaku_fill::fill_with_groups`.
    pub fn fill_group_ids(&self) -> Vec<u32> {
        self.meta.iter().map(|m| m.fill_group).collect()
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
        for path in obj.polylines().into_iter().filter(|p| !p.points.is_empty()) {
            unit.paths.push(path);
            unit.meta.push(meta);
        }
    };
    match (&obj.kind, mode) {
        (ObjectKind::Vector(_), LayerMode::Line | LayerMode::Fill) => push_paths(unit),
        (ObjectKind::Image(img), LayerMode::Image) => {
            match placed_image(obj, img, &obj.transform) {
                Ok(p) => unit.images.push(p),
                Err(e) if report => warnings.push(format!("{e}; se ignora")),
                Err(_) => {}
            }
        }
        (ObjectKind::Text(t), LayerMode::Line | LayerMode::Fill) => {
            // El pipeline solo corta el contorno ya generado; no interpreta fuentes.
            if t.outline.is_empty() {
                if report {
                    warnings.push(format!(
                        "objeto '{}' omitido: el texto no tiene contorno generado",
                        obj.id
                    ));
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

fn kind_name(kind: &ObjectKind) -> &'static str {
    match kind {
        ObjectKind::Vector(_) => "vectorial",
        ObjectKind::Image(_) => "de imagen",
        ObjectKind::Text(_) => "de texto",
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
            return Err(format!(
                "capa {}: demasiadas sub-capas ({}); el máximo es {MAX_SUB_LAYERS}",
                layer.color,
                layer.sub_layers.len()
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
            return Err(format!(
                "el objeto '{}' usa la capa {}, que no existe en el documento",
                obj.id, obj.layer
            ));
        };
        // La salida de la capa manda sobre la de sus sub-capas.
        if !doc.layers[parent].output {
            continue;
        }
        if !affine::is_finite(&obj.transform) {
            return Err(format!(
                "el objeto '{}' tiene una transformación inválida",
                obj.id
            ));
        }
        let power_scale = obj.power_scale.unwrap_or(100.0);
        if !(power_scale.is_finite() && power_scale >= 0.0) {
            return Err(format!(
                "el objeto '{}': power_scale inválida ({power_scale}); debe ser un % mayor o igual que 0",
                obj.id
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
        };
        let ops = first[parent]..first[parent] + 1 + doc.layers[parent].sub_layers.len();
        for u in ops {
            let unit = &mut work[u];
            if u != first[parent] && !unit.settings.output {
                continue;
            }
            let main = u == first[parent];
            if !add_object(unit, obj, meta, main, warnings) {
                if main {
                    warnings.push(format!(
                        "objeto '{}' {} ignorado: la capa {} es de modo {}",
                        obj.id,
                        kind_name(&obj.kind),
                        unit.settings.color,
                        unit.settings.mode.name()
                    ));
                } else {
                    skipped[u] += 1;
                }
            }
        }
    }
    for (unit, n) in work.iter().zip(&skipped) {
        if *n > 0 {
            warnings.push(format!(
                "capa {}, sub-capa {}: {n} objeto(s) ignorado(s) porque no admite el modo {}",
                unit.settings.color,
                unit.sub.unwrap_or(0),
                unit.settings.mode.name()
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
    if options.use_selection_origin {
        if options.selection.is_none() {
            warnings.push(
                "use_selection_origin se ignora: no hay selección (usa `selection` para cortar solo algunos objetos)"
                    .into(),
            );
        } else if options.start_from == StartFrom::Absolute {
            warnings.push(
                "use_selection_origin se ignora: el modo absoluto usa las coordenadas del documento"
                    .into(),
            );
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
    if work.is_empty() && selected.is_some() {
        warnings.push(
            "la selección no incluye ningún objeto con salida (oculto, de una capa sin salida o de un tipo que su capa no admite)"
                .into(),
        );
    }

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
