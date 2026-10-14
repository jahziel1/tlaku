//! Tiling (M39): dividir un trabajo más grande que la cama en tramos.
//!
//! - [`plan_tiles`] reparte la caja del diseño en rectángulos (los tramos) con solape y los
//!   ordena; cada [`TileSpec`] trae el desplazamiento que lleva su esquina inferior izquierda al
//!   origen de la cama.
//! - [`apply`] (usado por `place`) recorta la geometría de un tramo a su rectángulo y la desplaza:
//!   los trazos se parten en el borde, los rellenos se recortan como región (con booleanas) y las
//!   imágenes se enmascaran, así no queda ninguna línea a medias.
//! - [`tile_marks`] y [`add_alignment_marks`] dan pequeñas cruces en las zonas de solape. Como
//!   caen dentro de dos o más tramos, se graban en todos ellos y sirven para alinear el material
//!   a mano al moverlo.
//!
//! Fuente del comportamiento: el foro público pide justo esto (partir vectores, registrar y
//! solapar unos 2 mm); ver `docs/spec/03-maquinas-avanzadas.md`, sección 3.16. No existe una
//! función equivalente en LightBurn, así que no hay nada que copiar.

use crate::place::{LayerWork, PathMeta, PlacedImage};
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Bounds, Point, Polyline};
use tlaku_doc::affine;
use tlaku_doc::{
    DesignObject, Document, LayerMode, LayerSettings, ObjectKind, TileSpec, VectorObject,
};
use tlaku_geom::{BoolOp, boolean, break_groups};
use tlaku_i18n::tr;

use serde::{Deserialize, Serialize};
use std::fmt;

/// Tolerancia geométrica para decidir si un trazo recortado quedó vacío, en mm.
const EPS: f64 = 1e-9;
/// Máximo de tramos de un plan: más de esto es casi seguro un tamaño de tramo mal escrito.
pub const MAX_TILES: usize = 400;
/// Tamaño máximo de una cruz de alineación, mm.
const MAX_MARK_MM: f64 = 6.0;
/// Prefijo del id de los objetos de las marcas de alineación.
pub const MARK_ID_PREFIX: &str = "tile-mark-";

/// Orden en que se recorren los tramos. Las filas van de arriba hacia abajo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TileOrder {
    /// Todas las filas de izquierda a derecha.
    #[default]
    Rows,
    /// La primera fila de izquierda a derecha y la siguiente al revés, y así: la cabeza (o el
    /// material) recorre menos distancia entre tramos.
    Serpentine,
}

/// Por qué no se pudo repartir el diseño en tramos. `Display` da el mensaje en el idioma
/// activo del motor; la interfaz usa el tipo para mostrar el suyo en el idioma elegido.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TileError {
    /// Algún valor no es un número.
    Numbers,
    /// El diseño no tiene área.
    Empty,
    /// El tamaño del tramo no es mayor que 0.
    Size,
    /// El solape es negativo.
    OverlapNegative,
    /// El solape (mm) no es menor que el lado menor del tramo.
    #[serde(rename_all = "camelCase")]
    Overlap { overlap: f64, tile: [f64; 2] },
    /// Salen más de [`MAX_TILES`] tramos.
    #[serde(rename_all = "camelCase")]
    TooMany { count: usize },
}

impl fmt::Display for TileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TileError::Numbers => write!(f, "{}", tr!("pipeline.tile.numbers")),
            TileError::Empty => write!(f, "{}", tr!("pipeline.tile.empty")),
            TileError::Size => write!(f, "{}", tr!("pipeline.tile.size")),
            TileError::OverlapNegative => write!(f, "{}", tr!("pipeline.tile.overlap_negative")),
            TileError::Overlap { overlap, tile } => write!(
                f,
                "{}",
                tr!(
                    "pipeline.tile.overlap",
                    overlap = overlap,
                    w = tile[0],
                    h = tile[1]
                )
            ),
            TileError::TooMany { count } => write!(
                f,
                "{}",
                tr!("pipeline.tile.too_many", count = count, max = MAX_TILES)
            ),
        }
    }
}

impl std::error::Error for TileError {}

/// Cuadrícula de tramos: rangos `(min, max)` de cada columna (X creciente) y de cada fila (de
/// arriba hacia abajo, es decir Y decreciente).
struct Grid {
    cols: Vec<(f64, f64)>,
    rows: Vec<(f64, f64)>,
}

/// Reparte `[lo, hi]` en tramos de a lo sumo `size` mm que se solapan `overlap` mm. El último
/// tramo llega justo hasta `hi`.
fn split_axis(lo: f64, hi: f64, size: f64, overlap: f64) -> Vec<(f64, f64)> {
    let len = hi - lo;
    if len <= size + EPS {
        return vec![(lo, hi)];
    }
    let step = size - overlap;
    let n = ((len - size) / step - EPS).ceil().max(0.0) as usize + 1;
    (0..n)
        .map(|k| {
            let start = lo + k as f64 * step;
            (start, (start + size).min(hi))
        })
        .collect()
}

fn grid(design: Bounds, tile_size: [f64; 2], overlap: f64) -> Result<Grid, TileError> {
    let finite = [
        design.min.x,
        design.min.y,
        design.max.x,
        design.max.y,
        tile_size[0],
        tile_size[1],
        overlap,
    ]
    .iter()
    .all(|v| v.is_finite());
    if !finite {
        return Err(TileError::Numbers);
    }
    if design.max.x <= design.min.x || design.max.y <= design.min.y {
        return Err(TileError::Empty);
    }
    if tile_size[0] <= 0.0 || tile_size[1] <= 0.0 {
        return Err(TileError::Size);
    }
    if overlap < 0.0 {
        return Err(TileError::OverlapNegative);
    }
    if overlap >= tile_size[0].min(tile_size[1]) {
        return Err(TileError::Overlap {
            overlap,
            tile: tile_size,
        });
    }
    let cols = split_axis(design.min.x, design.max.x, tile_size[0], overlap);
    // Las filas se numeran de arriba hacia abajo y se parten desde arriba: el tramo sobrante
    // (si lo hay) queda abajo. Se parte el eje negado y se vuelve a invertir.
    let rows = split_axis(-design.max.y, -design.min.y, tile_size[1], overlap)
        .into_iter()
        .map(|(a, b)| (-b, -a))
        .collect::<Vec<_>>();
    if cols.len() * rows.len() > MAX_TILES {
        return Err(TileError::TooMany {
            count: cols.len() * rows.len(),
        });
    }
    Ok(Grid { cols, rows })
}

/// Reparte la caja `design` en tramos de `tile_size` (ancho, alto) mm con `overlap` mm de solape.
///
/// Los tramos salen en el orden de `order` (filas de arriba hacia abajo). Cada `TileSpec` lleva
/// `rect` en coordenadas del documento y `offset` = `-rect.min`: al aplicarlo, la esquina
/// inferior izquierda del tramo queda en el origen de la cama. Para dejar un margen en la cama
/// usa [`shift_tiles`]. Si el diseño cabe en un solo tramo, sale ese único tramo.
pub fn plan_tiles(
    design: Bounds,
    tile_size: [f64; 2],
    overlap: f64,
    order: TileOrder,
) -> Result<Vec<TileSpec>, String> {
    let g = grid(design, tile_size, overlap).map_err(|e| e.to_string())?;
    Ok(tiles_of(&g, order).into_iter().map(|t| t.spec).collect())
}

/// Un tramo del plan con su posición en la cuadrícula.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlannedTile {
    pub spec: TileSpec,
    /// Fila (0 = arriba) y columna (0 = izquierda) en la cuadrícula.
    pub row: usize,
    pub col: usize,
}

/// Plan completo de tiling: los tramos en orden, las dimensiones de la cuadrícula y las marcas de
/// alineación. Es lo que usa la interfaz para dibujar el plan sobre el lienzo y enviar los tramos.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TilePlan {
    pub tiles: Vec<PlannedTile>,
    pub cols: usize,
    pub rows: usize,
    /// Rangos `[min, max]` de cada columna (X creciente) y de cada fila (de arriba hacia abajo).
    pub col_ranges: Vec<[f64; 2]>,
    pub row_ranges: Vec<[f64; 2]>,
    /// Segmentos de las cruces de alineación (dos por cruz); vacío sin solape o con un tramo.
    pub marks: Vec<[[f64; 2]; 2]>,
}

/// Reparte `design` como [`plan_tiles`] pero devuelve el plan completo, con `margin` mm sumados
/// al desplazamiento de todos los tramos (el margen de la cama) y las marcas de alineación. Los
/// errores son estructurados para poder traducirlos.
pub fn plan_grid(
    design: Bounds,
    tile_size: [f64; 2],
    overlap: f64,
    order: TileOrder,
    margin: f64,
) -> Result<TilePlan, TileError> {
    let g = grid(design, tile_size, overlap)?;
    if !margin.is_finite() {
        return Err(TileError::Numbers);
    }
    let mut tiles = tiles_of(&g, order);
    for t in &mut tiles {
        t.spec.offset[0] += margin;
        t.spec.offset[1] += margin;
    }
    let marks = marks_of(&g, design, overlap)
        .iter()
        .map(|seg| {
            [
                [seg.points[0].x, seg.points[0].y],
                [seg.points[1].x, seg.points[1].y],
            ]
        })
        .collect();
    Ok(TilePlan {
        tiles,
        cols: g.cols.len(),
        rows: g.rows.len(),
        col_ranges: g.cols.iter().map(|&(a, b)| [a, b]).collect(),
        row_ranges: g.rows.iter().map(|&(a, b)| [a, b]).collect(),
        marks,
    })
}

fn tiles_of(g: &Grid, order: TileOrder) -> Vec<PlannedTile> {
    let mut out = Vec::with_capacity(g.cols.len() * g.rows.len());
    for (r, &(y0, y1)) in g.rows.iter().enumerate() {
        let mut row: Vec<PlannedTile> = g
            .cols
            .iter()
            .enumerate()
            .map(|(c, &(x0, x1))| PlannedTile {
                spec: TileSpec {
                    rect: [[x0, y0], [x1, y1]],
                    offset: [-x0, -y0],
                },
                row: r,
                col: c,
            })
            .collect();
        if order == TileOrder::Serpentine && r % 2 == 1 {
            row.reverse();
        }
        out.extend(row);
    }
    out
}

/// Suma `by` al desplazamiento de todos los tramos (por ejemplo, el margen de la cama).
pub fn shift_tiles(specs: &mut [TileSpec], by: [f64; 2]) {
    for s in specs {
        s.offset[0] += by[0];
        s.offset[1] += by[1];
    }
}

/// Cruces de alineación en las zonas de solape, como trazos abiertos (dos por cruz) en
/// coordenadas del documento. Cada cruz cae dentro de al menos dos tramos, así que se graba en
/// todos los que la contienen. Sin solape (o con un solo tramo) no hay zona donde ponerlas y
/// sale una lista vacía. Mide `min(0.8 * solape, 6)` mm.
pub fn tile_marks(
    design: Bounds,
    tile_size: [f64; 2],
    overlap: f64,
) -> Result<Vec<Polyline>, String> {
    let g = grid(design, tile_size, overlap).map_err(|e| e.to_string())?;
    Ok(marks_of(&g, design, overlap))
}

fn marks_of(g: &Grid, design: Bounds, overlap: f64) -> Vec<Polyline> {
    if overlap <= 0.0 || (g.cols.len() == 1 && g.rows.len() == 1) {
        return Vec::new();
    }
    let size = (overlap * 0.8).min(MAX_MARK_MM);
    let half = size / 2.0;
    let m = overlap / 2.0;
    // Centro de cada franja de solape y puntos de los extremos del diseño.
    let x_bands: Vec<f64> = g.cols.windows(2).map(|w| w[1].0 + m).collect();
    let y_bands: Vec<f64> = g.rows.windows(2).map(|w| w[0].0 + m).collect();
    let mut xs = vec![design.min.x + m];
    xs.extend(&x_bands);
    xs.push(design.max.x - m);
    let mut ys = vec![design.min.y + m];
    ys.extend(&y_bands);
    ys.push(design.max.y - m);
    let mut centers: Vec<(f64, f64)> = Vec::new();
    let mut add = |x: f64, y: f64| {
        if !centers
            .iter()
            .any(|&(cx, cy)| (cx - x).abs() < 1e-6 && (cy - y).abs() < 1e-6)
        {
            centers.push((x, y));
        }
    };
    for &x in &x_bands {
        for &y in &ys {
            add(x, y);
        }
    }
    for &y in &y_bands {
        for &x in &xs {
            add(x, y);
        }
    }
    centers
        .into_iter()
        .flat_map(|(x, y)| {
            [
                Polyline {
                    points: vec![Point::new(x - half, y), Point::new(x + half, y)],
                    closed: false,
                },
                Polyline {
                    points: vec![Point::new(x, y - half), Point::new(x, y + half)],
                    closed: false,
                },
            ]
        })
        .collect()
}

/// Agrega las marcas de alineación al documento como objetos vectoriales de la capa `layer`
/// (se crea si no existe, en modo línea). Devuelve cuántas cruces se agregaron. Los objetos
/// llevan ids `tile-mark-N`; quita los anteriores si ya había.
pub fn add_alignment_marks(
    doc: &mut Document,
    design: Bounds,
    tile_size: [f64; 2],
    overlap: f64,
    layer: tlaku_core::Rgb,
) -> Result<usize, String> {
    let marks = tile_marks(design, tile_size, overlap)?;
    if marks.is_empty() {
        return Ok(0);
    }
    if !doc.layers.iter().any(|l| l.color == layer) {
        doc.layers.push(LayerSettings::for_color(layer));
    }
    doc.objects.retain(|o| !o.id.starts_with(MARK_ID_PREFIX));
    // Cada cruz son dos trazos consecutivos: un objeto por cruz.
    for (i, cross) in marks.chunks(2).enumerate() {
        doc.objects.push(DesignObject {
            id: format!("{MARK_ID_PREFIX}{}", i + 1),
            layer,
            transform: affine::IDENTITY,
            kind: ObjectKind::Vector(VectorObject::new(cross.iter().map(BezPath::from).collect())),
            locked: false,
            hidden: false,
            group: None,
            priority: None,
            power_scale: None,
            shape: None,
            start: None,
            manual_tabs: Vec::new(),
            array: None,
        });
    }
    Ok(marks.len() / 2)
}

// ---------------------------------------------------------------------------
// Recorte
// ---------------------------------------------------------------------------

fn inside(r: Bounds, p: Point) -> bool {
    p.x >= r.min.x && p.x <= r.max.x && p.y >= r.min.y && p.y <= r.max.y
}

fn rect_polyline(r: Bounds) -> Polyline {
    Polyline {
        points: vec![
            Point::new(r.min.x, r.min.y),
            Point::new(r.max.x, r.min.y),
            Point::new(r.max.x, r.max.y),
            Point::new(r.min.x, r.max.y),
        ],
        closed: true,
    }
}

fn overlaps(a: Bounds, b: Bounds) -> bool {
    a.min.x <= b.max.x && a.max.x >= b.min.x && a.min.y <= b.max.y && a.max.y >= b.min.y
}

fn contains(outer: Bounds, inner: Bounds) -> bool {
    inside(outer, inner.min) && inside(outer, inner.max)
}

/// Parte del segmento `a`-`b` dentro de `r` (Liang-Barsky): parámetros `(t0, t1)` en `[0, 1]`.
fn clip_segment(r: Bounds, a: Point, b: Point) -> Option<(f64, f64)> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for (p, q) in [
        (-dx, a.x - r.min.x),
        (dx, r.max.x - a.x),
        (-dy, a.y - r.min.y),
        (dy, r.max.y - a.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    (t0 <= t1).then_some((t0, t1))
}

fn path_length(points: &[Point]) -> f64 {
    points
        .windows(2)
        .map(|w| (w[1].x - w[0].x).hypot(w[1].y - w[0].y))
        .sum()
}

/// Parte un trazo en los bordes de `r`: devuelve los tramos que quedan dentro, como trazos
/// abiertos. Un trazo cerrado que cruza el borde queda abierto, y el tramo que pasa por su
/// punto de partida se une en uno solo. Un trazo que ya cae entero dentro se devuelve igual.
fn clip_stroke(path: &Polyline, r: Bounds) -> Vec<Polyline> {
    if path.points.iter().all(|&p| inside(r, p)) {
        return vec![path.clone()];
    }
    let pts = &path.points;
    let n = pts.len();
    if n < 2 {
        return Vec::new();
    }
    let segs = if path.closed { n } else { n - 1 };
    let mut pieces: Vec<Vec<Point>> = Vec::new();
    let mut current: Vec<Point> = Vec::new();
    // El primer tramo empieza justo en el primer punto del trazo.
    let mut first_at_start = false;
    for i in 0..segs {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        let Some((t0, t1)) = clip_segment(r, a, b) else {
            if !current.is_empty() {
                pieces.push(std::mem::take(&mut current));
            }
            continue;
        };
        let at = |t: f64| {
            if t == 0.0 {
                a
            } else if t == 1.0 {
                b
            } else {
                Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
            }
        };
        if t0 > 0.0 && !current.is_empty() {
            pieces.push(std::mem::take(&mut current));
        }
        if current.is_empty() {
            if pieces.is_empty() && i == 0 && t0 == 0.0 {
                first_at_start = true;
            }
            current.push(at(t0));
        }
        current.push(at(t1));
        if t1 < 1.0 {
            pieces.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        // Sigue abierto al terminar el recorrido: llega al punto de partida.
        if path.closed && first_at_start && !pieces.is_empty() {
            let head = pieces.remove(0);
            current.extend(head.into_iter().skip(1));
            pieces.insert(0, current);
        } else {
            pieces.push(current);
        }
    }
    pieces
        .into_iter()
        .filter(|p| p.len() >= 2 && path_length(p) > EPS)
        .map(|points| Polyline {
            points,
            closed: false,
        })
        .collect()
}

/// Recorta las trayectorias de una capa de línea.
fn clip_line_layer(layer: &mut LayerWork, r: Bounds) {
    let paths = std::mem::take(&mut layer.paths);
    let metas = std::mem::take(&mut layer.meta);
    for (path, meta) in paths.into_iter().zip(metas) {
        let Some(b) = tlaku_doc::bounds_of(std::slice::from_ref(&path)) else {
            continue;
        };
        if !overlaps(r, b) {
            continue;
        }
        if contains(r, b) {
            layer.paths.push(path);
            layer.meta.push(meta);
            continue;
        }
        for piece in clip_stroke(&path, r) {
            layer.paths.push(piece);
            // El punto de inicio y el sentido fijados a mano ya no valen en el trozo recortado.
            layer.meta.push(PathMeta {
                locked: false,
                ..meta
            });
        }
    }
}

/// Recorta las trayectorias de una capa de relleno como región: cada pieza con sus huecos se
/// intersecta con el rectángulo, así el relleno llega justo hasta el borde del tramo.
fn clip_fill_layer(layer: &mut LayerWork, r: Bounds) {
    let paths = std::mem::take(&mut layer.paths);
    let metas = std::mem::take(&mut layer.meta);
    let region = [rect_polyline(r)];
    for group in break_groups(&paths, true) {
        let meta = metas[group[0]];
        let members: Vec<&Polyline> = group.iter().map(|&i| &paths[i]).collect();
        let closed = members.iter().all(|p| p.closed);
        let Some(b) =
            tlaku_doc::bounds_of(&group.iter().map(|&i| paths[i].clone()).collect::<Vec<_>>())
        else {
            continue;
        };
        if !overlaps(r, b) {
            continue;
        }
        if contains(r, b) {
            for &i in &group {
                layer.paths.push(paths[i].clone());
                layer.meta.push(metas[i]);
            }
            continue;
        }
        if closed {
            let pieces: Vec<Polyline> = group.iter().map(|&i| paths[i].clone()).collect();
            for p in boolean(&pieces, &region, BoolOp::Intersection) {
                layer.paths.push(p);
                layer.meta.push(PathMeta {
                    locked: false,
                    ..meta
                });
            }
        } else {
            // Un trazo abierto en una capa de relleno no es una región: se parte como trazo.
            for piece in clip_stroke(&paths[group[0]], r) {
                layer.paths.push(piece);
                layer.meta.push(PathMeta {
                    locked: false,
                    ..meta
                });
            }
        }
    }
}

/// Recorta una imagen al rectángulo con una máscara. `None` si no queda nada.
fn clip_image(img: &PlacedImage, r: Bounds) -> Option<PlacedImage> {
    let rect = Bounds {
        min: img.origin,
        max: Point::new(img.origin.x + img.size_mm.0, img.origin.y + img.size_mm.1),
    };
    if !overlaps(r, rect) {
        return None;
    }
    let mask_bounds = if img.mask.is_empty() {
        rect
    } else {
        tlaku_doc::bounds_of(&img.mask)?
    };
    if contains(r, rect) && contains(r, mask_bounds) {
        return Some(img.clone());
    }
    let cut = [rect_polyline(r)];
    let base = if img.mask.is_empty() {
        vec![rect_polyline(rect)]
    } else {
        img.mask.clone()
    };
    let mask = boolean(&base, &cut, BoolOp::Intersection);
    if mask.is_empty() {
        return None;
    }
    Some(PlacedImage {
        mask,
        ..img.clone()
    })
}

fn validate(tile: &TileSpec) -> Result<Bounds, String> {
    let [[x0, y0], [x1, y1]] = tile.rect;
    let all = [x0, y0, x1, y1, tile.offset[0], tile.offset[1]];
    if !all.iter().all(|v| v.is_finite()) {
        return Err(tr!("pipeline.tile.rect_not_numbers"));
    }
    if x1 <= x0 || y1 <= y0 {
        return Err(tr!(
            "pipeline.tile.rect_no_area",
            x0 = x0,
            y0 = y0,
            x1 = x1,
            y1 = y1
        ));
    }
    Ok(Bounds {
        min: Point::new(x0, y0),
        max: Point::new(x1, y1),
    })
}

/// Deja en `work` solo lo que cae dentro del rectángulo del tramo y lo desplaza `tile.offset`.
/// Las capas que quedan vacías se quitan. Si no queda nada se avisa (y `build` lo rechaza como
/// "nada que enviar").
pub(crate) fn apply(
    work: &mut Vec<LayerWork<'_>>,
    tile: &TileSpec,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let r = validate(tile)?;
    let [dx, dy] = tile.offset;
    for layer in work.iter_mut() {
        match layer.settings.mode {
            LayerMode::Fill => clip_fill_layer(layer, r),
            LayerMode::Line => clip_line_layer(layer, r),
            LayerMode::Image => {}
        }
        let images = std::mem::take(&mut layer.images);
        layer.images = images.iter().filter_map(|img| clip_image(img, r)).collect();
        // Puentes manuales: solo los que caen dentro del tramo.
        for pts in layer.manual_tabs.values_mut() {
            pts.retain(|&p| inside(r, p));
        }
        layer.manual_tabs.retain(|_, pts| !pts.is_empty());

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
            for p in img.mask.iter_mut().flat_map(|m| &mut m.points) {
                p.x += dx;
                p.y += dy;
            }
        }
    }
    work.retain(|l| !l.is_empty());
    if work.is_empty() {
        warnings.push(tr!("pipeline.tile.empty_tile"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::place::place;
    use tlaku_core::Rgb;
    use tlaku_doc::{DeviceProfile, ImageObject, JobOptions};

    const RED: Rgb = Rgb(255, 0, 0);

    fn bounds(x0: f64, y0: f64, x1: f64, y1: f64) -> Bounds {
        Bounds {
            min: Point::new(x0, y0),
            max: Point::new(x1, y1),
        }
    }

    fn object(id: &str, polys: &[Polyline]) -> DesignObject {
        DesignObject {
            id: id.into(),
            layer: RED,
            transform: affine::IDENTITY,
            kind: ObjectKind::Vector(VectorObject::new(polys.iter().map(BezPath::from).collect())),
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

    fn rect_poly(x: f64, y: f64, w: f64, h: f64) -> Polyline {
        rect_polyline(bounds(x, y, x + w, y + h))
    }

    fn doc_with(mode: LayerMode, polys: &[Polyline]) -> Document {
        let mut layer = LayerSettings::for_color(RED);
        layer.mode = mode;
        Document {
            objects: vec![object("a", polys)],
            layers: vec![layer],
            ..Document::default()
        }
    }

    fn placed(doc: &Document, tile: &TileSpec) -> (Vec<Polyline>, Vec<String>) {
        let options = JobOptions {
            tile: Some(tile.clone()),
            ..JobOptions::default()
        };
        let mut warnings = Vec::new();
        let p = place(doc, &options, None, &mut warnings).unwrap();
        (
            p.layers.iter().flat_map(|l| l.paths.clone()).collect(),
            warnings,
        )
    }

    fn perimeter(p: &Polyline) -> f64 {
        let mut pts = p.points.clone();
        if p.closed {
            pts.push(pts[0]);
        }
        path_length(&pts)
    }

    fn area(p: &Polyline) -> f64 {
        tlaku_geom::signed_area(&p.points).abs()
    }

    // ---- planificación -----------------------------------------------------

    #[test]
    fn a_design_that_fits_is_one_tile_at_the_origin() {
        let specs = plan_tiles(
            bounds(10.0, 20.0, 60.0, 50.0),
            [100.0, 100.0],
            5.0,
            TileOrder::Rows,
        )
        .unwrap();
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].rect, [[10.0, 20.0], [60.0, 50.0]]);
        assert_eq!(specs[0].offset, [-10.0, -20.0]);
    }

    #[test]
    fn a_wide_design_splits_in_four_with_the_requested_overlap() {
        let specs = plan_tiles(
            bounds(0.0, 0.0, 100.0, 60.0),
            [55.0, 35.0],
            10.0,
            TileOrder::Rows,
        )
        .unwrap();
        assert_eq!(specs.len(), 4);
        // Filas de arriba hacia abajo; la primera es la completa.
        assert_eq!(specs[0].rect, [[0.0, 25.0], [55.0, 60.0]]);
        assert_eq!(specs[1].rect, [[45.0, 25.0], [100.0, 60.0]]);
        assert_eq!(specs[2].rect, [[0.0, 0.0], [55.0, 35.0]]);
        for s in &specs {
            assert_eq!(s.offset, [-s.rect[0][0], -s.rect[0][1]]);
            assert!(s.rect[1][0] - s.rect[0][0] <= 55.0 + 1e-9);
            assert!(s.rect[1][1] - s.rect[0][1] <= 35.0 + 1e-9);
        }
        // El solape entre columnas es el pedido.
        assert!((specs[0].rect[1][0] - specs[1].rect[0][0] - 10.0).abs() < 1e-9);
    }

    #[test]
    fn the_union_of_the_tiles_covers_the_design() {
        let d = bounds(3.0, -7.0, 321.0, 190.0);
        let specs = plan_tiles(d, [120.0, 80.0], 4.0, TileOrder::Rows).unwrap();
        // Rejilla de puntos de prueba: cada uno cae en algún tramo.
        let mut x = d.min.x;
        while x <= d.max.x {
            let mut y = d.min.y;
            while y <= d.max.y {
                let p = Point::new(x, y);
                assert!(
                    specs.iter().any(|s| inside(
                        bounds(s.rect[0][0], s.rect[0][1], s.rect[1][0], s.rect[1][1]),
                        p
                    )),
                    "({x}, {y}) sin cubrir"
                );
                y += 3.7;
            }
            x += 3.7;
        }
    }

    #[test]
    fn serpentine_reverses_every_other_row() {
        let d = bounds(0.0, 0.0, 100.0, 60.0);
        let rows = plan_tiles(d, [55.0, 35.0], 10.0, TileOrder::Rows).unwrap();
        let snake = plan_tiles(d, [55.0, 35.0], 10.0, TileOrder::Serpentine).unwrap();
        assert_eq!(snake[0], rows[0]);
        assert_eq!(snake[1], rows[1]);
        assert_eq!(snake[2], rows[3]);
        assert_eq!(snake[3], rows[2]);
    }

    #[test]
    fn bad_parameters_are_rejected() {
        let d = bounds(0.0, 0.0, 100.0, 100.0);
        assert!(plan_tiles(d, [0.0, 50.0], 0.0, TileOrder::Rows).is_err());
        assert!(plan_tiles(d, [50.0, 50.0], 50.0, TileOrder::Rows).is_err());
        assert!(plan_tiles(d, [50.0, 50.0], -1.0, TileOrder::Rows).is_err());
        assert!(plan_tiles(d, [f64::NAN, 50.0], 0.0, TileOrder::Rows).is_err());
        assert!(
            plan_tiles(
                bounds(0.0, 0.0, 0.0, 5.0),
                [50.0, 50.0],
                0.0,
                TileOrder::Rows
            )
            .is_err()
        );
        assert!(plan_tiles(d, [0.5, 0.5], 0.1, TileOrder::Rows).is_err());
    }

    #[test]
    fn shift_adds_a_margin_to_every_offset() {
        let mut specs = plan_tiles(
            bounds(0.0, 0.0, 100.0, 60.0),
            [55.0, 35.0],
            10.0,
            TileOrder::Rows,
        )
        .unwrap();
        shift_tiles(&mut specs, [5.0, 5.0]);
        assert_eq!(specs[1].offset, [-45.0 + 5.0, -25.0 + 5.0]);
    }

    // ---- recorte -----------------------------------------------------------

    /// Rectángulo de 100 x 60 en capa de línea, 4 tramos de 55 x 35 con 10 mm de solape.
    fn stroke_rect_tiles() -> (Document, Vec<TileSpec>) {
        let doc = doc_with(LayerMode::Line, &[rect_poly(0.0, 0.0, 100.0, 60.0)]);
        let specs = plan_tiles(
            bounds(0.0, 0.0, 100.0, 60.0),
            [55.0, 35.0],
            10.0,
            TileOrder::Rows,
        )
        .unwrap();
        (doc, specs)
    }

    #[test]
    fn the_stroke_lengths_add_up_to_the_original_plus_the_overlaps() {
        let (doc, specs) = stroke_rect_tiles();
        let mut total = 0.0;
        for s in &specs {
            let (paths, _) = placed(&doc, s);
            assert!(!paths.is_empty());
            // Todo trazo es abierto (el rectángulo se partió) y cae en la cama del tramo.
            for p in &paths {
                assert!(!p.closed, "quedó un trazo cerrado");
                for q in &p.points {
                    assert!(q.x >= -1e-9 && q.x <= 55.0 + 1e-9, "x fuera: {}", q.x);
                    assert!(q.y >= -1e-9 && q.y <= 35.0 + 1e-9, "y fuera: {}", q.y);
                }
            }
            total += paths.iter().map(perimeter).sum::<f64>();
        }
        // Perímetro 320 + los tramos de borde que caen en una franja de solape se graban dos
        // veces: 2 bordes horizontales x 10 mm + 2 verticales x 10 mm.
        assert!((total - (320.0 + 40.0)).abs() < 1e-6, "total = {total}");
    }

    #[test]
    fn a_tile_moves_its_part_to_the_bed_origin() {
        let (doc, specs) = stroke_rect_tiles();
        // Tramo 4 (abajo a la derecha): x 45-100, y 0-35.
        let (paths, _) = placed(&doc, &specs[3]);
        let b = tlaku_doc::bounds_of(&paths).unwrap();
        assert!(b.min.x.abs() < 1e-9 && b.min.y.abs() < 1e-9);
        assert!((b.max.x - 55.0).abs() < 1e-9 && (b.max.y - 35.0).abs() < 1e-9);
    }

    #[test]
    fn a_stroke_fully_inside_is_left_closed_and_untouched() {
        let doc = doc_with(LayerMode::Line, &[rect_poly(10.0, 10.0, 20.0, 20.0)]);
        let tile = TileSpec {
            rect: [[0.0, 0.0], [50.0, 50.0]],
            offset: [1.0, 2.0],
        };
        let (paths, _) = placed(&doc, &tile);
        assert_eq!(paths.len(), 1);
        assert!(paths[0].closed);
        assert_eq!(paths[0].points[0], Point::new(11.0, 12.0));
    }

    #[test]
    fn a_closed_stroke_that_crosses_joins_the_piece_through_its_start() {
        // El círculo (cuadrado) empieza dentro del tramo y sale por la derecha: dos tramos
        // abiertos que se unen en uno solo al pasar por el punto de partida.
        let poly = rect_poly(0.0, 0.0, 20.0, 10.0);
        let doc = doc_with(LayerMode::Line, &[poly]);
        let tile = TileSpec {
            rect: [[0.0, 0.0], [12.0, 10.0]],
            offset: [0.0, 0.0],
        };
        let (paths, _) = placed(&doc, &tile);
        // Dentro quedan: el borde inferior hasta x=12, el izquierdo y el superior hasta x=12.
        assert_eq!(paths.len(), 1, "{paths:?}");
        assert!((perimeter(&paths[0]) - (12.0 + 10.0 + 12.0)).abs() < 1e-9);
    }

    #[test]
    fn a_stroke_outside_the_tile_disappears_and_an_empty_tile_warns() {
        let doc = doc_with(LayerMode::Line, &[rect_poly(100.0, 100.0, 10.0, 10.0)]);
        let tile = TileSpec {
            rect: [[0.0, 0.0], [50.0, 50.0]],
            offset: [0.0, 0.0],
        };
        let (paths, warnings) = placed(&doc, &tile);
        assert!(paths.is_empty());
        assert!(warnings.iter().any(|w| w.contains("ninguna parte")));
    }

    #[test]
    fn a_fill_is_clipped_as_a_region_with_its_holes() {
        // Cuadrado de 100 con un hueco de 20 en el centro; tramo: la mitad izquierda.
        let doc = doc_with(
            LayerMode::Fill,
            &[
                rect_poly(0.0, 0.0, 100.0, 100.0),
                rect_poly(40.0, 40.0, 20.0, 20.0),
            ],
        );
        let tile = TileSpec {
            rect: [[0.0, 0.0], [50.0, 100.0]],
            offset: [0.0, 0.0],
        };
        let (paths, _) = placed(&doc, &tile);
        let solid: f64 = paths
            .iter()
            .map(|p| {
                if tlaku_geom::signed_area(&p.points) > 0.0 {
                    area(p)
                } else {
                    -area(p)
                }
            })
            .sum();
        // 50 x 100 menos la mitad del hueco (10 x 20).
        assert!((solid - (5000.0 - 200.0)).abs() < 1e-6, "{solid}");
        assert!(paths.iter().all(|p| p.closed));
    }

    #[test]
    fn the_fill_areas_add_up_to_the_original_plus_the_overlaps() {
        let doc = doc_with(LayerMode::Fill, &[rect_poly(0.0, 0.0, 100.0, 60.0)]);
        let specs = plan_tiles(
            bounds(0.0, 0.0, 100.0, 60.0),
            [55.0, 35.0],
            10.0,
            TileOrder::Rows,
        )
        .unwrap();
        let total: f64 = specs
            .iter()
            .map(|s| placed(&doc, s).0.iter().map(area).sum::<f64>())
            .sum();
        // 6000 + franja vertical (10 x 60) + franja horizontal (100 x 10) - cruce contado de
        // más (10 x 10): cada punto vale tantas veces como tramos lo contienen.
        let expected = 6000.0 + 10.0 * 60.0 + 100.0 * 10.0 + 10.0 * 10.0;
        assert!((total - expected).abs() < 1e-6, "total = {total}");
    }

    #[test]
    fn a_tile_may_not_have_an_invalid_rect() {
        let doc = doc_with(LayerMode::Line, &[rect_poly(0.0, 0.0, 10.0, 10.0)]);
        let options = JobOptions {
            tile: Some(TileSpec {
                rect: [[5.0, 5.0], [5.0, 9.0]],
                offset: [0.0, 0.0],
            }),
            ..JobOptions::default()
        };
        let err = place(&doc, &options, None, &mut Vec::new()).err().unwrap();
        assert!(err.contains("no tiene área"), "{err}");
    }

    #[test]
    fn an_image_is_masked_to_the_tile_and_moved() {
        let img = PlacedImage {
            source_path: "x.png".into(),
            origin: Point::new(0.0, 0.0),
            size_mm: (100.0, 50.0),
            mirror_x: false,
            mirror_y: false,
            mask: Vec::new(),
            warp: None,
        };
        let r = bounds(60.0, 10.0, 160.0, 40.0);
        let clipped = clip_image(&img, r).unwrap();
        let b = tlaku_doc::bounds_of(&clipped.mask).unwrap();
        assert!((b.min.x - 60.0).abs() < 1e-9 && (b.max.x - 100.0).abs() < 1e-9);
        assert!((b.min.y - 10.0).abs() < 1e-9 && (b.max.y - 40.0).abs() < 1e-9);
        // Disjunta: se descarta. Entera dentro: no se toca.
        assert!(clip_image(&img, bounds(200.0, 0.0, 300.0, 50.0)).is_none());
        assert!(
            clip_image(&img, bounds(-1.0, -1.0, 101.0, 51.0))
                .unwrap()
                .mask
                .is_empty()
        );
        // Con máscara previa se intersecta con ella.
        let masked = PlacedImage {
            mask: vec![rect_poly(0.0, 0.0, 70.0, 50.0)],
            ..img
        };
        let c = clip_image(&masked, r).unwrap();
        let b = tlaku_doc::bounds_of(&c.mask).unwrap();
        assert!((b.min.x - 60.0).abs() < 1e-9 && (b.max.x - 70.0).abs() < 1e-9);
    }

    fn gray_png(dir: &std::path::Path) -> std::path::PathBuf {
        let path = dir.join("tile_test.png");
        let img = image::GrayImage::from_fn(40, 20, |x, _| {
            image::Luma([if x % 2 == 0 { 0 } else { 255 }])
        });
        img.save(&path).unwrap();
        path
    }

    #[test]
    fn a_tiled_image_job_stays_inside_the_bed() {
        let dir = std::env::temp_dir().join(format!("tlaku_tile_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = gray_png(&dir);
        let mut layer = LayerSettings::for_color(LayerSettings::IMAGE_COLOR);
        layer.mode = LayerMode::Image;
        layer.overscan = tlaku_doc::Overscan::Mm(0.0);
        let doc = Document {
            objects: vec![DesignObject {
                id: "img".into(),
                layer: LayerSettings::IMAGE_COLOR,
                transform: affine::IDENTITY,
                kind: ObjectKind::Image(ImageObject {
                    source_path: png.to_str().unwrap().into(),
                    width_px: 40,
                    height_px: 20,
                    size_mm: [80.0, 40.0],
                    mask: Vec::new(),
                }),
                locked: false,
                hidden: false,
                group: None,
                priority: None,
                power_scale: None,
                shape: None,
                start: None,
                manual_tabs: Vec::new(),
                array: None,
            }],
            layers: vec![layer],
            ..Document::default()
        };
        let device = DeviceProfile {
            bed: [50.0, 50.0],
            ..DeviceProfile::default()
        };
        let specs = plan_tiles(
            bounds(0.0, 0.0, 80.0, 40.0),
            [45.0, 45.0],
            5.0,
            TileOrder::Rows,
        )
        .unwrap();
        assert_eq!(specs.len(), 2);
        for s in specs {
            let options = JobOptions {
                tile: Some(s),
                ..JobOptions::default()
            };
            let job = crate::compute(&doc, &device, &options).unwrap();
            crate::check_device_bounds(&job, &device).unwrap();
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_tiled_vector_job_never_leaves_the_bed() {
        let (doc, specs) = stroke_rect_tiles();
        let device = DeviceProfile {
            bed: [55.0, 35.0],
            ..DeviceProfile::default()
        };
        for s in specs {
            let options = JobOptions {
                tile: Some(s),
                ..JobOptions::default()
            };
            let job = crate::compute(&doc, &device, &options).unwrap();
            crate::check_device_bounds(&job, &device).unwrap();
            let g = crate::gcode(&job, &device);
            assert!(!g.is_empty());
        }
    }

    // ---- marcas ------------------------------------------------------------

    #[test]
    fn marks_sit_in_the_overlap_zones_and_inside_two_or_more_tiles() {
        let d = bounds(0.0, 0.0, 100.0, 60.0);
        let specs = plan_tiles(d, [55.0, 35.0], 10.0, TileOrder::Rows).unwrap();
        let marks = tile_marks(d, [55.0, 35.0], 10.0).unwrap();
        assert!(!marks.is_empty());
        assert_eq!(marks.len() % 2, 0);
        for cross in marks.chunks(2) {
            let b = tlaku_doc::bounds_of(cross).unwrap();
            let n = specs
                .iter()
                .filter(|s| {
                    contains(
                        bounds(s.rect[0][0], s.rect[0][1], s.rect[1][0], s.rect[1][1]),
                        b,
                    )
                })
                .count();
            assert!(n >= 2, "la cruz {b:?} cae solo en {n} tramo(s)");
        }
    }

    #[test]
    fn a_mark_is_engraved_whole_in_every_tile_that_contains_it() {
        let d = bounds(0.0, 0.0, 100.0, 60.0);
        let specs = plan_tiles(d, [55.0, 35.0], 10.0, TileOrder::Rows).unwrap();
        let mut doc = doc_with(LayerMode::Line, &[rect_poly(0.0, 0.0, 100.0, 60.0)]);
        let n = add_alignment_marks(&mut doc, d, [55.0, 35.0], 10.0, Rgb(0, 128, 255)).unwrap();
        assert!(n >= 2);
        assert_eq!(doc.objects.len(), 1 + n);
        let marks = tile_marks(d, [55.0, 35.0], 10.0).unwrap();
        let mut engraved = 0usize;
        for s in &specs {
            let rect = bounds(s.rect[0][0], s.rect[0][1], s.rect[1][0], s.rect[1][1]);
            let whole = marks
                .chunks(2)
                .filter(|c| contains(rect, tlaku_doc::bounds_of(c).unwrap()))
                .count();
            let options = JobOptions {
                tile: Some(s.clone()),
                selection: Some(
                    doc.objects
                        .iter()
                        .filter(|o| o.id.starts_with(MARK_ID_PREFIX))
                        .map(|o| o.id.clone())
                        .collect(),
                ),
                ..JobOptions::default()
            };
            let p = place(&doc, &options, None, &mut Vec::new()).unwrap();
            let paths: Vec<Polyline> = p.layers.iter().flat_map(|l| l.paths.clone()).collect();
            // Cada cruz entera del tramo sale con sus dos trazos completos (6 mm cada uno).
            assert!(paths.len() >= 2 * whole);
            assert!(
                paths
                    .iter()
                    .filter(|p| (perimeter(p) - 6.0).abs() < 1e-9)
                    .count()
                    >= 2 * whole
            );
            engraved += whole;
        }
        assert!(engraved >= 2 * n, "{engraved} grabadas de {n} marcas");
        // Reaplicar no duplica las marcas.
        add_alignment_marks(&mut doc, d, [55.0, 35.0], 10.0, Rgb(0, 128, 255)).unwrap();
        assert_eq!(doc.objects.len(), 1 + n);
    }

    #[test]
    fn no_overlap_or_one_tile_means_no_marks() {
        let d = bounds(0.0, 0.0, 100.0, 60.0);
        assert!(tile_marks(d, [55.0, 35.0], 0.0).unwrap().is_empty());
        assert!(tile_marks(d, [200.0, 200.0], 5.0).unwrap().is_empty());
    }

    #[test]
    fn plan_grid_agrees_with_plan_tiles_and_tile_marks() {
        let d = bounds(0.0, 0.0, 100.0, 60.0);
        for order in [TileOrder::Rows, TileOrder::Serpentine] {
            let plan = plan_grid(d, [55.0, 35.0], 10.0, order, 0.0).unwrap();
            let specs: Vec<TileSpec> = plan.tiles.iter().map(|t| t.spec.clone()).collect();
            assert_eq!(specs, plan_tiles(d, [55.0, 35.0], 10.0, order).unwrap());
            let marks = tile_marks(d, [55.0, 35.0], 10.0).unwrap();
            assert_eq!(plan.marks.len(), marks.len());
            for (m, p) in plan.marks.iter().zip(&marks) {
                assert_eq!(m[0], [p.points[0].x, p.points[0].y]);
                assert_eq!(m[1], [p.points[1].x, p.points[1].y]);
            }
        }
        // El margen se suma igual que `shift_tiles`.
        let plan = plan_grid(d, [55.0, 35.0], 10.0, TileOrder::Rows, 5.0).unwrap();
        let mut shifted = plan_tiles(d, [55.0, 35.0], 10.0, TileOrder::Rows).unwrap();
        shift_tiles(&mut shifted, [5.0, 5.0]);
        let got: Vec<TileSpec> = plan.tiles.iter().map(|t| t.spec.clone()).collect();
        assert_eq!(got, shifted);
        assert!(plan_grid(d, [55.0, 35.0], 10.0, TileOrder::Rows, f64::NAN).is_err());
    }

    #[test]
    fn plan_grid_wire_format_and_error_messages() {
        let d = bounds(0.0, 0.0, 100.0, 60.0);
        let v =
            serde_json::to_value(plan_grid(d, [55.0, 35.0], 10.0, TileOrder::Rows, 0.0).unwrap())
                .unwrap();
        for key in ["tiles", "cols", "rows", "colRanges", "rowRanges", "marks"] {
            assert!(v.get(key).is_some(), "falta {key}");
        }
        let first = &v["tiles"][0];
        assert_eq!(
            first["spec"]["rect"],
            serde_json::json!([[0.0, 25.0], [55.0, 60.0]])
        );
        assert_eq!(first["row"], 0);
        let kind = |e: TileError| serde_json::to_value(e).unwrap();
        assert_eq!(
            kind(TileError::Numbers),
            serde_json::json!({"kind": "numbers"})
        );
        assert_eq!(
            kind(TileError::OverlapNegative),
            serde_json::json!({"kind": "overlapNegative"})
        );
        let e = plan_grid(d, [50.0, 50.0], 50.0, TileOrder::Rows, 0.0).unwrap_err();
        assert_eq!(
            e.to_string(),
            "tiling: el solape (50 mm) debe ser menor que el tramo (50 x 50 mm)"
        );
        let order: TileOrder = serde_json::from_str("\"serpentine\"").unwrap();
        assert_eq!(order, TileOrder::Serpentine);
    }
}
