//! Comando `shape_op`: operaciones de edición de formas de la ola 3 (recortar, separar,
//! optimizar, redondear esquinas, cortar formas y redimensionar ranuras).
//!
//! Los objetos llegan en coordenadas de documento (con su `transform`) y los resultados vuelven
//! igual, con transform identidad. Solo se aceptan objetos vectoriales: las imágenes no tienen
//! trazos y un texto que cambiara dejaría de ser texto (conviene convertirlo antes).
//!
//! Protocolo de resultado (el mismo que `join`/`close`/`dedupe` de `geometry_op`): vuelve la
//! lista completa de objetos de entrada, en su orden. Cada objeto sin cambios aparece idéntico
//! (mismo id); cada objeto que cambia se sustituye por uno o más objetos nuevos con id
//! `{op}-N`, con la capa, el grupo, la prioridad y la potencia de su origen. Quien llama: los ids
//! de entrada que ya no están en el resultado se eliminan, los demás se dejan como están y los
//! ids nuevos se insertan donde estaba el primero eliminado.
//!
//! Operaciones:
//! - `trim`: borra el tramo de trazo más cercano a `point` entre dos intersecciones. Las demás
//!   subtrayectorias del objeto conservan sus curvas; solo la recortada pasa a rectas. Con
//!   `keep_removed` el tramo borrado queda como objeto aparte.
//! - `break_apart`: un objeto por subtrayecto (las curvas se conservan). Con `keep_holes` cada
//!   contorno exterior se queda con los huecos que contiene.
//! - `optimize`: quita nodos redundantes, simplifica con `tolerance` y, con `smooth`, ajusta
//!   curvas. Solo se sustituye un subtrayecto si queda con menos nodos que antes. Las
//!   estadísticas dan los nodos antes y después (sirve de vista previa sin aplicar el resultado).
//! - `fillet`: redondea (`round`), achaflana (`chamfer`) o muerde (`bite`) las esquinas de recta
//!   con recta; con `nodes` solo las esquinas en esos puntos. Las curvas se conservan.
//! - `cut_shapes`: una forma (`knife_id`; por defecto el último objeto de la lista) parte a las
//!   demás en lo que queda dentro y fuera. Con `close_pieces` (por defecto) las piezas cerradas
//!   son regiones; si no, el contorno se parte en tramos abiertos sin la línea de corte. Un
//!   objeto que la cuchilla no atraviesa se deja intacto. La cuchilla se consume salvo
//!   `keep_knife`.
//! - `resize_slots`: cambia las ranuras de `from_width ± tolerance` a `to_width`. Calcula la
//!   detección con todas las formas a la vez (un agujero puede estar en otro objeto).

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tlaku_core::{Point, Polyline};
use tlaku_doc::{BezPath, DesignObject, ObjectKind, VectorObject, affine};
use tlaku_geom::{
    FilletOptions, FilletStyle, OptimizeOptions, SlotAnchor, SlotOptions, break_groups, cut_closed,
    cut_open, fillet_path, node_count, optimize_polyline, resize_slots, signed_area, slot_widths,
    split_subpaths, trim_at,
};

/// Error máximo (mm) al aplanar las curvas para calcular intersecciones y contornos.
const FLATTEN_MM: f64 = 0.01;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeOp {
    Trim,
    BreakApart,
    Optimize,
    Fillet,
    CutShapes,
    ResizeSlots,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilletKind {
    Round,
    Chamfer,
    Bite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorKind {
    Center,
    Start,
    End,
}

/// Parámetros de todas las operaciones; cada una lee solo los suyos.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ShapeParams {
    /// `trim`: punto del clic (mm, documento).
    pub point: Option<[f64; 2]>,
    /// `trim`: distancia máxima (mm) del clic al trazo.
    pub max_dist: Option<f64>,
    /// `trim`: conservar el tramo borrado como objeto aparte.
    pub keep_removed: bool,
    /// `break_apart`: cada exterior conserva sus huecos.
    pub keep_holes: bool,
    /// `optimize` y `resize_slots`: tolerancia en mm.
    pub tolerance: Option<f64>,
    /// `optimize`: ajustar curvas cúbicas.
    pub smooth: bool,
    /// `optimize`: giro (grados) desde el cual un vértice es esquina.
    pub corner_angle: Option<f64>,
    /// `fillet`: radio (o distancia) en mm.
    pub radius: Option<f64>,
    pub style: Option<FilletKind>,
    /// `fillet`: solo las esquinas en estos puntos (mm, documento).
    pub nodes: Option<Vec<[f64; 2]>>,
    /// `cut_shapes`: id de la cuchilla.
    pub knife_id: Option<String>,
    /// `cut_shapes`: piezas cerradas (regiones) en vez de tramos abiertos. Por defecto sí.
    pub close_pieces: Option<bool>,
    /// `cut_shapes`: conservar la cuchilla.
    pub keep_knife: bool,
    /// `resize_slots`: ancho a buscar y ancho nuevo (mm).
    pub from_width: Option<f64>,
    pub to_width: Option<f64>,
    pub anchor: Option<AnchorKind>,
    /// `resize_slots`: qué patrones buscar. Por defecto muescas y agujeros.
    pub notches: Option<bool>,
    pub holes: Option<bool>,
    pub tabs: Option<bool>,
}

/// Números del resultado, para avisos y vistas previas. Los campos que una operación no usa van en 0.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ShapeStats {
    /// Cosas hechas: tramos recortados, objetos separados o cortados, esquinas redondeadas,
    /// subtrayectos optimizados o ranuras cambiadas.
    pub count: usize,
    /// Cosas que se dejaron igual por no caber o no poder (esquinas sin espacio, ranuras bloqueadas).
    pub skipped: usize,
    /// Esquinas que se redondearon con un radio menor al pedido.
    pub clamped: usize,
    /// Objetos nuevos que produce la operación.
    pub produced: usize,
    /// `resize_slots`: ranuras que coinciden con el ancho buscado (aunque no se pudieran cambiar).
    pub found: usize,
    /// `optimize`: nodos de los subtrayectos revisados, antes y después.
    pub nodes_before: usize,
    pub nodes_after: usize,
    /// `resize_slots`: anchos detectados `[ancho, cuántas]`, de menor a mayor.
    pub widths: Vec<[f64; 2]>,
    /// `trim`: puntos del tramo que se borra, para resaltarlo.
    pub removed: Vec<[f64; 2]>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShapeOpResult {
    pub objects: Vec<DesignObject>,
    pub stats: ShapeStats,
}

fn pt(p: [f64; 2]) -> Point {
    Point::new(p[0], p[1])
}

fn xy(p: Point) -> [f64; 2] {
    [p.x, p.y]
}

/// Objeto vectorial nuevo con los datos de `source` y estos trazos.
fn vector_object(source: &DesignObject, id: String, paths: Vec<BezPath>) -> DesignObject {
    DesignObject {
        id,
        transform: affine::IDENTITY,
        kind: ObjectKind::Vector(VectorObject { paths }),
        locked: false,
        // La forma paramétrica, el punto de inicio, los puentes y las copias virtuales dejan de valer.
        shape: None,
        start: None,
        manual_tabs: Vec::new(),
        array: None,
        ..source.clone()
    }
}

/// Generador de ids `{op}-N` que no chocan con los de entrada.
struct Ids<'a> {
    prefix: &'a str,
    taken: HashSet<&'a str>,
    counter: usize,
}

impl<'a> Ids<'a> {
    fn new(prefix: &'a str, objects: &'a [DesignObject]) -> Self {
        Self {
            prefix,
            taken: objects.iter().map(|o| o.id.as_str()).collect(),
            counter: 0,
        }
    }

    fn next(&mut self) -> String {
        loop {
            self.counter += 1;
            let id = format!("{}-{}", self.prefix, self.counter);
            if !self.taken.contains(id.as_str()) {
                return id;
            }
        }
    }
}

/// Qué pasa con cada objeto de entrada.
enum Slot {
    Keep,
    Gone,
    /// Lo sustituyen estos objetos.
    New(Vec<DesignObject>),
}

fn finish(objects: &[DesignObject], slots: Vec<Slot>, mut stats: ShapeStats) -> ShapeOpResult {
    let mut out = Vec::new();
    for (o, slot) in objects.iter().zip(slots) {
        match slot {
            Slot::Keep => out.push(o.clone()),
            Slot::Gone => {}
            Slot::New(list) => {
                stats.produced += list.len();
                out.extend(list);
            }
        }
    }
    ShapeOpResult {
        objects: out,
        stats,
    }
}

fn require_vectors(objects: &[DesignObject]) -> Result<(), String> {
    if objects.is_empty() {
        return Err("No hay objetos sobre los que operar.".into());
    }
    let bad: Vec<String> = objects
        .iter()
        .filter(|o| !matches!(o.kind, ObjectKind::Vector(_)))
        .map(|o| format!("«{}»", o.id))
        .collect();
    if bad.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Esta operación solo trabaja con trazos vectoriales; convierte los textos a trayecto y quita las imágenes: {}.",
            bad.join(", ")
        ))
    }
}

/// Subtrayectos (con sus curvas) de un objeto, en coordenadas de documento.
fn subpaths(o: &DesignObject) -> Vec<BezPath> {
    o.curves().iter().flat_map(split_subpaths).collect()
}

/// Aplanado de un subtrayecto (uno solo por construcción); un punto suelto si no dibuja nada.
fn flat_of(sub: &BezPath) -> Polyline {
    sub.flatten(FLATTEN_MM)
        .into_iter()
        .next()
        .unwrap_or(Polyline {
            points: Vec::new(),
            closed: false,
        })
}

/// Núcleo puro de `shape_op` (sin Tauri), para poder probarlo.
pub fn run(
    op: ShapeOp,
    objects: &[DesignObject],
    p: &ShapeParams,
) -> Result<ShapeOpResult, String> {
    require_vectors(objects)?;
    match op {
        ShapeOp::Trim => trim(objects, p),
        ShapeOp::BreakApart => break_apart(objects, p),
        ShapeOp::Optimize => optimize(objects, p),
        ShapeOp::Fillet => fillet(objects, p),
        ShapeOp::CutShapes => cut_shapes(objects, p),
        ShapeOp::ResizeSlots => resize(objects, p),
    }
}

// --- recortar ---

fn trim(objects: &[DesignObject], p: &ShapeParams) -> Result<ShapeOpResult, String> {
    let click = pt(p.point.ok_or("Falta el punto donde recortar.")?);
    // Todos los subtrayectos de la selección, con su objeto.
    let mut subs: Vec<(usize, BezPath)> = Vec::new();
    for (i, o) in objects.iter().enumerate() {
        subs.extend(subpaths(o).into_iter().map(|s| (i, s)));
    }
    let flat: Vec<Polyline> = subs.iter().map(|(_, s)| flat_of(s)).collect();
    let usable: Vec<usize> = (0..flat.len())
        .filter(|&k| flat[k].points.len() >= 2)
        .collect();
    let usable_paths: Vec<Polyline> = usable.iter().map(|&k| flat[k].clone()).collect();
    let hit = trim_at(&usable_paths, click, p.max_dist)
        .ok_or("No hay ningún trazo cerca del punto indicado.")?;
    let target = usable[hit.path_index];
    let owner = subs[target].0;

    let mut ids = Ids::new("trim", objects);
    let mut slots: Vec<Slot> = objects.iter().map(|_| Slot::Keep).collect();
    let mut paths: Vec<BezPath> = Vec::new();
    for (k, (i, sub)) in subs.iter().enumerate() {
        if *i != owner {
            continue;
        }
        if k == target {
            paths.extend(hit.remaining.iter().map(BezPath::from));
        } else {
            paths.push(sub.clone());
        }
    }
    let source = &objects[owner];
    let mut fresh = Vec::new();
    if !paths.is_empty() {
        fresh.push(vector_object(source, ids.next(), paths));
    }
    if p.keep_removed {
        fresh.push(vector_object(
            source,
            ids.next(),
            vec![BezPath::from(&hit.removed)],
        ));
    }
    slots[owner] = if fresh.is_empty() {
        Slot::Gone
    } else {
        Slot::New(fresh)
    };
    let stats = ShapeStats {
        count: 1,
        removed: hit.removed.points.iter().map(|&q| xy(q)).collect(),
        ..ShapeStats::default()
    };
    Ok(finish(objects, slots, stats))
}

// --- separar ---

fn break_apart(objects: &[DesignObject], p: &ShapeParams) -> Result<ShapeOpResult, String> {
    let mut ids = Ids::new("break", objects);
    let mut slots: Vec<Slot> = objects.iter().map(|_| Slot::Keep).collect();
    let mut stats = ShapeStats::default();
    for (i, o) in objects.iter().enumerate() {
        let subs = subpaths(o);
        if subs.len() < 2 {
            continue;
        }
        let flat: Vec<Polyline> = subs.iter().map(flat_of).collect();
        let groups = break_groups(&flat, p.keep_holes);
        if groups.len() < 2 {
            continue;
        }
        stats.count += 1;
        slots[i] = Slot::New(
            groups
                .iter()
                .map(|g| vector_object(o, ids.next(), g.iter().map(|&k| subs[k].clone()).collect()))
                .collect(),
        );
    }
    Ok(finish(objects, slots, stats))
}

// --- optimizar ---

fn optimize(objects: &[DesignObject], p: &ShapeParams) -> Result<ShapeOpResult, String> {
    let tolerance = p.tolerance.unwrap_or(0.02);
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err("La tolerancia no es un número válido (debe ser 0 o más).".into());
    }
    let opts = OptimizeOptions {
        tolerance,
        smooth: p.smooth,
        corner_angle_deg: p.corner_angle.unwrap_or(35.0),
    };
    let mut ids = Ids::new("opt", objects);
    let mut slots: Vec<Slot> = objects.iter().map(|_| Slot::Keep).collect();
    let mut stats = ShapeStats::default();
    for (i, o) in objects.iter().enumerate() {
        let mut changed = false;
        let mut paths = Vec::new();
        for sub in subpaths(o) {
            let before = node_count(&sub);
            let optimized = optimize_polyline(&flat_of(&sub), &opts);
            let after = node_count(&optimized);
            stats.nodes_before += before;
            if after < before {
                stats.nodes_after += after;
                stats.count += 1;
                changed = true;
                paths.push(optimized);
            } else {
                stats.nodes_after += before;
                paths.push(sub);
            }
        }
        if changed {
            slots[i] = Slot::New(vec![vector_object(o, ids.next(), paths)]);
        }
    }
    Ok(finish(objects, slots, stats))
}

// --- redondear esquinas ---

fn fillet(objects: &[DesignObject], p: &ShapeParams) -> Result<ShapeOpResult, String> {
    let radius = p.radius.ok_or("Falta el radio.")?;
    if !radius.is_finite() || radius <= 0.0 {
        return Err("El radio debe ser un número mayor que 0.".into());
    }
    let style = match p.style.unwrap_or(FilletKind::Round) {
        FilletKind::Round => FilletStyle::Round,
        FilletKind::Chamfer => FilletStyle::Chamfer,
        FilletKind::Bite => FilletStyle::Bite,
    };
    let opts = FilletOptions { radius, style };
    let only: Option<Vec<Point>> = p
        .nodes
        .as_ref()
        .map(|v| v.iter().copied().map(pt).collect());
    let mut ids = Ids::new("fillet", objects);
    let mut slots: Vec<Slot> = objects.iter().map(|_| Slot::Keep).collect();
    let mut stats = ShapeStats::default();
    for (i, o) in objects.iter().enumerate() {
        let mut rounded = 0;
        let paths: Vec<BezPath> = o
            .curves()
            .iter()
            .map(|c| {
                let r = fillet_path(c, &opts, only.as_deref());
                rounded += r.rounded;
                stats.clamped += r.clamped;
                stats.skipped += r.no_space;
                r.path
            })
            .collect();
        if rounded > 0 {
            stats.count += rounded;
            slots[i] = Slot::New(vec![vector_object(o, ids.next(), paths)]);
        }
    }
    Ok(finish(objects, slots, stats))
}

// --- cortar formas ---

/// Parte una región (cada exterior seguido de sus agujeros) en piezas: una por exterior.
fn pieces(region: Vec<Polyline>) -> Vec<Vec<Polyline>> {
    let mut out: Vec<Vec<Polyline>> = Vec::new();
    for p in region {
        if signed_area(&p.points) > 0.0 || out.is_empty() {
            out.push(vec![p]);
        } else if let Some(last) = out.last_mut() {
            last.push(p);
        }
    }
    out
}

fn cut_shapes(objects: &[DesignObject], p: &ShapeParams) -> Result<ShapeOpResult, String> {
    if objects.len() < 2 {
        return Err("Cortar formas necesita la cuchilla y al menos una forma más.".into());
    }
    let knife_index = match &p.knife_id {
        Some(id) => objects
            .iter()
            .position(|o| &o.id == id)
            .ok_or_else(|| format!("La cuchilla «{id}» no está entre los objetos."))?,
        None => objects.len() - 1,
    };
    let knife_paths = objects[knife_index].polylines_with(FLATTEN_MM);
    if knife_paths.is_empty() || knife_paths.iter().any(|k| !k.closed) {
        return Err(format!(
            "La cuchilla «{}» debe ser una forma cerrada. Ciérrala antes (por ejemplo con «Cerrar polilínea»).",
            objects[knife_index].id
        ));
    }
    let close_pieces = p.close_pieces.unwrap_or(true);
    let mut ids = Ids::new("cut", objects);
    let mut slots: Vec<Slot> = objects.iter().map(|_| Slot::Keep).collect();
    let mut stats = ShapeStats::default();
    for (i, o) in objects.iter().enumerate() {
        if i == knife_index {
            continue;
        }
        let flat = o.polylines_with(FLATTEN_MM);
        let (closed, open): (Vec<Polyline>, Vec<Polyline>) =
            flat.into_iter().partition(|f| f.closed);
        // Regiones (cerradas) y tramos (abiertos) de cada lado.
        let mut regions: [Vec<Vec<Polyline>>; 2] = [Vec::new(), Vec::new()];
        let mut spans: [Vec<Polyline>; 2] = [Vec::new(), Vec::new()];
        if close_pieces {
            let cut = cut_closed(&closed, &knife_paths);
            regions[0] = pieces(cut.inside);
            regions[1] = pieces(cut.outside);
        } else {
            let cut = cut_open(&closed, &knife_paths);
            spans[0].extend(cut.inside);
            spans[1].extend(cut.outside);
        }
        let cut = cut_open(&open, &knife_paths);
        spans[0].extend(cut.inside);
        spans[1].extend(cut.outside);

        let inside = regions[0].len() + usize::from(!spans[0].is_empty());
        let outside = regions[1].len() + usize::from(!spans[1].is_empty());
        if inside == 0 || outside == 0 {
            // La cuchilla no atraviesa el objeto: no hay nada que partir.
            stats.skipped += 1;
            continue;
        }
        let mut fresh = Vec::new();
        for side in 0..2 {
            for piece in &regions[side] {
                fresh.push(vector_object(
                    o,
                    ids.next(),
                    piece.iter().map(BezPath::from).collect(),
                ));
            }
            if !spans[side].is_empty() {
                fresh.push(vector_object(
                    o,
                    ids.next(),
                    spans[side].iter().map(BezPath::from).collect(),
                ));
            }
        }
        stats.count += 1;
        slots[i] = Slot::New(fresh);
    }
    if stats.count > 0 && !p.keep_knife {
        slots[knife_index] = Slot::Gone;
    }
    Ok(finish(objects, slots, stats))
}

// --- redimensionar ranuras ---

fn resize(objects: &[DesignObject], p: &ShapeParams) -> Result<ShapeOpResult, String> {
    let from_width = p
        .from_width
        .ok_or("Falta el ancho actual de las ranuras.")?;
    let to_width = p.to_width.ok_or("Falta el ancho nuevo de las ranuras.")?;
    let tolerance = p.tolerance.unwrap_or(0.1);
    for (v, name) in [(from_width, "actual"), (to_width, "nuevo")] {
        if !v.is_finite() || v <= 0.0 {
            return Err(format!("El ancho {name} debe ser un número mayor que 0."));
        }
    }
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err("La tolerancia no es un número válido (debe ser 0 o más).".into());
    }
    let opts = SlotOptions {
        from_width,
        tolerance,
        to_width,
        anchor: match p.anchor.unwrap_or(AnchorKind::Center) {
            AnchorKind::Center => SlotAnchor::Center,
            AnchorKind::Start => SlotAnchor::Start,
            AnchorKind::End => SlotAnchor::End,
        },
        notches: p.notches.unwrap_or(true),
        holes: p.holes.unwrap_or(true),
        tabs: p.tabs.unwrap_or(false),
    };
    // Todas las formas juntas: un agujero puede estar dentro del contorno de otro objeto.
    let mut all: Vec<Polyline> = Vec::new();
    let mut owner: Vec<usize> = Vec::new();
    for (i, o) in objects.iter().enumerate() {
        for f in o.polylines_with(FLATTEN_MM) {
            all.push(f);
            owner.push(i);
        }
    }
    let (resized, report) = resize_slots(&all, &opts);
    let mut ids = Ids::new("slot", objects);
    let mut slots: Vec<Slot> = objects.iter().map(|_| Slot::Keep).collect();
    for i in 0..objects.len() {
        let mine: Vec<usize> = (0..all.len()).filter(|&k| owner[k] == i).collect();
        if mine.iter().any(|&k| resized[k] != all[k]) {
            let paths = mine.iter().map(|&k| BezPath::from(&resized[k])).collect();
            slots[i] = Slot::New(vec![vector_object(&objects[i], ids.next(), paths)]);
        }
    }
    let stats = ShapeStats {
        count: report.resized,
        skipped: report.blocked,
        found: report.found,
        widths: slot_widths(&all, opts.notches, opts.holes, opts.tabs)
            .into_iter()
            .map(|(w, n)| [w, n as f64])
            .collect(),
        ..ShapeStats::default()
    };
    Ok(finish(objects, slots, stats))
}

/// Recortar, separar, optimizar, redondear esquinas, cortar formas y redimensionar ranuras (ver
/// la nota del módulo). Corre fuera del hilo principal.
#[tauri::command]
pub async fn shape_op(
    op: ShapeOp,
    objects: Vec<DesignObject>,
    params: Option<ShapeParams>,
) -> Result<ShapeOpResult, String> {
    tauri::async_runtime::spawn_blocking(move || run(op, &objects, &params.unwrap_or_default()))
        .await
        .map_err(|e| format!("La tarea se interrumpió: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_core::Rgb;

    fn object(id: &str, polys: &[Polyline]) -> DesignObject {
        DesignObject {
            id: id.into(),
            layer: Rgb(255, 0, 0),
            transform: affine::IDENTITY,
            kind: ObjectKind::Vector(VectorObject {
                paths: polys.iter().map(BezPath::from).collect(),
            }),
            locked: false,
            hidden: false,
            group: Some("g".into()),
            priority: None,
            power_scale: None,
            shape: None,
            start: None,
            manual_tabs: Vec::new(),
            array: None,
        }
    }

    fn poly(pts: &[(f64, f64)], closed: bool) -> Polyline {
        Polyline {
            points: pts.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            closed,
        }
    }

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Polyline {
        poly(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], true)
    }

    fn area(objects: &[DesignObject]) -> f64 {
        objects
            .iter()
            .flat_map(|o| o.polylines_with(0.001))
            .map(|p| signed_area(&p.points))
            .sum()
    }

    fn params() -> ShapeParams {
        ShapeParams::default()
    }

    #[test]
    fn rechaza_imagenes_y_listas_vacias() {
        assert!(run(ShapeOp::BreakApart, &[], &params()).is_err());
        let mut img = object("i", &[rect(0.0, 0.0, 1.0, 1.0)]);
        img.kind = ObjectKind::Image(tlaku_doc::ImageObject {
            source_path: "x.png".into(),
            width_px: 1,
            height_px: 1,
            size_mm: [1.0, 1.0],
            mask: Vec::new(),
        });
        let e = run(ShapeOp::BreakApart, &[img], &params()).unwrap_err();
        assert!(e.contains("«i»"), "{e}");
    }

    #[test]
    fn recortar_sustituye_el_objeto_y_conserva_el_resto() {
        let a = object("a", &[poly(&[(0.0, 0.0), (10.0, 0.0)], false)]);
        let b = object("b", &[poly(&[(5.0, -5.0), (5.0, 5.0)], false)]);
        let p = ShapeParams {
            point: Some([2.0, 0.0]),
            ..params()
        };
        let r = run(ShapeOp::Trim, &[a, b.clone()], &p).unwrap();
        assert_eq!(r.objects.len(), 2);
        assert_eq!(r.objects[1], b, "el otro objeto vuelve idéntico");
        assert_eq!(r.objects[0].id, "trim-1");
        assert_eq!(r.objects[0].group.as_deref(), Some("g"));
        assert_eq!(r.stats.count, 1);
        assert_eq!(r.stats.removed.len(), 2);
        // Con keep_removed aparece el tramo borrado como objeto aparte.
        let p2 = ShapeParams {
            keep_removed: true,
            ..p
        };
        let a = object("a", &[poly(&[(0.0, 0.0), (10.0, 0.0)], false)]);
        let b = object("b", &[poly(&[(5.0, -5.0), (5.0, 5.0)], false)]);
        let r = run(ShapeOp::Trim, &[a, b], &p2).unwrap();
        assert_eq!(r.objects.len(), 3);
    }

    #[test]
    fn recortar_una_linea_aislada_la_elimina() {
        let a = object("a", &[poly(&[(0.0, 0.0), (10.0, 0.0)], false)]);
        let p = ShapeParams {
            point: Some([3.0, 0.1]),
            max_dist: Some(1.0),
            ..params()
        };
        let r = run(ShapeOp::Trim, std::slice::from_ref(&a), &p).unwrap();
        assert!(r.objects.is_empty());
        let lejos = ShapeParams {
            point: Some([3.0, 9.0]),
            max_dist: Some(1.0),
            ..params()
        };
        assert!(run(ShapeOp::Trim, &[a], &lejos).is_err());
        assert!(run(ShapeOp::Trim, &[], &params()).is_err());
    }

    #[test]
    fn separar_da_un_objeto_por_subtrayecto() {
        let marco = object(
            "m",
            &[
                rect(0.0, 0.0, 20.0, 20.0),
                rect(5.0, 5.0, 15.0, 15.0),
                rect(30.0, 0.0, 40.0, 10.0),
            ],
        );
        let solo = object("s", &[rect(0.0, 0.0, 1.0, 1.0)]);
        let r = run(
            ShapeOp::BreakApart,
            &[marco.clone(), solo.clone()],
            &params(),
        )
        .unwrap();
        assert_eq!(r.objects.len(), 4);
        assert_eq!(r.objects[3], solo);
        assert_eq!(r.stats.count, 1);
        assert_eq!(r.stats.produced, 3);
        let con_huecos = ShapeParams {
            keep_holes: true,
            ..params()
        };
        let r = run(ShapeOp::BreakApart, &[marco], &con_huecos).unwrap();
        assert_eq!(
            r.objects.len(),
            2,
            "el marco con su hueco y el cuadrado suelto"
        );
        let ObjectKind::Vector(v) = &r.objects[0].kind else {
            panic!("debe ser vectorial");
        };
        assert_eq!(v.paths.len(), 2, "exterior y hueco");
    }

    #[test]
    fn separar_sin_nada_que_separar_no_cambia_nada() {
        let solo = object("s", &[rect(0.0, 0.0, 1.0, 1.0)]);
        let r = run(ShapeOp::BreakApart, std::slice::from_ref(&solo), &params()).unwrap();
        assert_eq!(r.objects, vec![solo]);
        assert_eq!(r.stats.count, 0);
    }

    #[test]
    fn optimizar_cuenta_nodos_antes_y_despues() {
        let mut pts = Vec::new();
        for i in 0..50 {
            pts.push((i as f64 / 5.0, 0.0));
        }
        pts.extend([(10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
        let o = object("o", &[poly(&pts, true)]);
        let r = run(ShapeOp::Optimize, &[o], &params()).unwrap();
        assert_eq!(r.stats.nodes_before, 53);
        assert_eq!(r.stats.nodes_after, 4);
        assert_eq!(r.objects.len(), 1);
        assert_eq!(r.objects[0].id, "opt-1");
        // Un cuadrado limpio ya está optimizado: no se toca.
        let limpio = object("c", &[rect(0.0, 0.0, 5.0, 5.0)]);
        let r = run(ShapeOp::Optimize, std::slice::from_ref(&limpio), &params()).unwrap();
        assert_eq!(r.objects, vec![limpio]);
        assert_eq!(r.stats.count, 0);
        let mala = ShapeParams {
            tolerance: Some(-1.0),
            ..params()
        };
        assert!(
            run(
                ShapeOp::Optimize,
                &[object("c", &[rect(0.0, 0.0, 1.0, 1.0)])],
                &mala
            )
            .is_err()
        );
    }

    #[test]
    fn redondear_reduce_el_area_y_valida_el_radio() {
        let o = object("o", &[rect(0.0, 0.0, 10.0, 10.0)]);
        let p = ShapeParams {
            radius: Some(2.0),
            ..params()
        };
        let r = run(ShapeOp::Fillet, std::slice::from_ref(&o), &p).unwrap();
        assert_eq!(r.stats.count, 4);
        assert!((area(&r.objects) - (100.0 - (4.0 - std::f64::consts::PI) * 4.0)).abs() < 0.01);
        assert!(run(ShapeOp::Fillet, std::slice::from_ref(&o), &params()).is_err());
        let neg = ShapeParams {
            radius: Some(-1.0),
            ..params()
        };
        assert!(run(ShapeOp::Fillet, std::slice::from_ref(&o), &neg).is_err());
        let uno = ShapeParams {
            radius: Some(2.0),
            nodes: Some(vec![[0.0, 0.0]]),
            ..params()
        };
        assert_eq!(run(ShapeOp::Fillet, &[o], &uno).unwrap().stats.count, 1);
    }

    #[test]
    fn cortar_formas_parte_y_consume_la_cuchilla() {
        let pieza = object("p", &[rect(0.0, 0.0, 10.0, 10.0)]);
        let cuchilla = object("k", &[rect(5.0, -5.0, 15.0, 15.0)]);
        let lejos = object("l", &[rect(100.0, 0.0, 110.0, 10.0)]);
        let r = run(
            ShapeOp::CutShapes,
            &[pieza.clone(), lejos.clone(), cuchilla.clone()],
            &params(),
        )
        .unwrap();
        assert_eq!(
            r.objects.len(),
            3,
            "dos mitades y el objeto lejano; sin cuchilla"
        );
        assert_eq!(r.stats.count, 1);
        assert_eq!(r.stats.skipped, 1);
        assert!(r.objects.iter().any(|o| o == &lejos));
        assert!((area(&r.objects[..2]) - 100.0).abs() < 1e-6);
        let conserva = ShapeParams {
            keep_knife: true,
            ..params()
        };
        let r = run(
            ShapeOp::CutShapes,
            &[pieza.clone(), cuchilla.clone()],
            &conserva,
        )
        .unwrap();
        assert!(r.objects.iter().any(|o| o == &cuchilla));
        // Tramos abiertos: no se añade la línea de corte.
        let abiertas = ShapeParams {
            close_pieces: Some(false),
            ..params()
        };
        let r = run(ShapeOp::CutShapes, &[pieza, cuchilla], &abiertas).unwrap();
        assert_eq!(r.objects.len(), 2);
        assert!(
            r.objects
                .iter()
                .all(|o| o.polylines().iter().all(|p| !p.closed))
        );
    }

    #[test]
    fn cortar_formas_exige_cuchilla_cerrada_y_nada_cortado_deja_todo() {
        let linea = object("k", &[poly(&[(0.0, 0.0), (1.0, 1.0)], false)]);
        let pieza = object("p", &[rect(0.0, 0.0, 10.0, 10.0)]);
        assert!(run(ShapeOp::CutShapes, &[pieza.clone(), linea], &params()).is_err());
        assert!(run(ShapeOp::CutShapes, std::slice::from_ref(&pieza), &params()).is_err());
        let aparte = object("k", &[rect(50.0, 50.0, 60.0, 60.0)]);
        let r = run(
            ShapeOp::CutShapes,
            &[pieza.clone(), aparte.clone()],
            &params(),
        )
        .unwrap();
        assert_eq!(r.objects, vec![pieza, aparte]);
        assert_eq!(r.stats.count, 0);
    }

    #[test]
    fn ranuras_cambia_de_ancho_y_lista_los_anchos() {
        let pieza = poly(
            &[
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 5.0),
                (13.0, 5.0),
                (13.0, 0.0),
                (40.0, 0.0),
                (40.0, 20.0),
                (0.0, 20.0),
            ],
            true,
        );
        let o = object("o", &[pieza]);
        let p = ShapeParams {
            from_width: Some(3.0),
            to_width: Some(3.5),
            ..params()
        };
        let r = run(ShapeOp::ResizeSlots, std::slice::from_ref(&o), &p).unwrap();
        assert_eq!(r.stats.count, 1);
        assert_eq!(r.stats.found, 1);
        assert_eq!(r.stats.widths, vec![[3.0, 1.0]]);
        assert!((area(&r.objects) - 782.5).abs() < 1e-6);
        let nada = ShapeParams {
            from_width: Some(7.0),
            to_width: Some(8.0),
            ..params()
        };
        let r = run(ShapeOp::ResizeSlots, std::slice::from_ref(&o), &nada).unwrap();
        assert_eq!(r.objects, vec![o.clone()]);
        let sin_ancho = ShapeParams {
            from_width: Some(3.0),
            ..params()
        };
        assert!(run(ShapeOp::ResizeSlots, &[o], &sin_ancho).is_err());
    }

    #[test]
    fn ranuras_encuentra_el_agujero_en_otro_objeto() {
        let marco = object("marco", &[rect(0.0, 0.0, 50.0, 50.0)]);
        let hueco = object("hueco", &[rect(10.0, 10.0, 30.0, 13.0)]);
        let p = ShapeParams {
            from_width: Some(3.0),
            to_width: Some(4.0),
            ..params()
        };
        let r = run(ShapeOp::ResizeSlots, &[marco.clone(), hueco], &p).unwrap();
        assert_eq!(r.objects[0], marco);
        assert_eq!(r.objects.len(), 2);
        assert!((area(&r.objects[1..]) - 80.0).abs() < 1e-6);
    }
}
