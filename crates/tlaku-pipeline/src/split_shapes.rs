//! Marcado dividido por formas enteras (M40, «Run Whole Shapes»).
//!
//! El marcado dividido normal parte el diseño en piezas del tamaño del campo y **recorta** lo que
//! cruza el borde de una pieza: la forma se marca en dos partes, con una costura. Con
//! `SplitMarking::whole_shapes` las piezas se **cortan por la frontera de las formas**: cada forma
//! se marca completa en una sola pieza y no hay costuras.
//!
//! Una *forma* es un objeto del documento, o todo un grupo (un grupo es una sola unidad, igual
//! que al seleccionar). Su caja es la de sus trazos ya colocados.
//!
//! # Cómo se reparten
//!
//! Se ordenan las formas por el centro de su caja y se agrupan de manera voraz a lo largo de cada
//! eje que se puede partir: un grupo crece mientras la caja que cubre quepa en el lado de la pieza
//! (`side`, el campo o `piece_mm`). Primero se reparten en filas (de arriba abajo) y dentro de cada
//! fila en columnas (de izquierda a derecha, en zigzag con `SplitOrder::Snake`). El rectángulo de
//! cada pieza es la caja de sus formas (con un margen mínimo), así que el centro de la pieza, que
//! es lo que se lleva al centro del campo, es el centro de lo que se marca. El solape no se usa.
//!
//! Cada paso lleva los ids de sus objetos ([`MarkStep::objects`](crate::MarkStep)) y quien lo
//! ejecuta los pasa como `JobOptions::selection`: no se recorta nada. Una forma queda en una sola
//! pieza aunque la caja de otra pieza la cubra en parte.
//!
//! # Cuándo no se puede
//!
//! Se vuelve a las franjas de siempre, con un aviso, si una forma es más grande que el lado de la
//! pieza en un eje (no hay modo de marcarla entera), si el trabajo lleva imágenes (su máscara no
//! se asigna a un objeto) o si hay una transformación de Print & Cut. Un arreglo virtual cuenta
//! como un solo objeto: todas sus copias van juntas.

use crate::ComputedJob;
use crate::place::place;
use crate::split::{MarkPlan, Piece, mark_sequence, mark_sequence_shapes};
use std::collections::BTreeMap;
use tlaku_core::Bounds;
use tlaku_doc::{DeviceProfile, Document, JobOptions, SplitOrder, bounds_of, union};
use tlaku_i18n::tr;

/// Tolerancia al comparar tamaños (mm).
const EPS: f64 = 1e-9;
/// Margen (mm) que se agrega a la caja de una pieza para que el recorte del tramo no toque las
/// formas y para que una pieza de una sola línea tenga área.
const PAD_MM: f64 = 0.01;

/// Una forma entera: los ids de sus objetos y su caja en mm del documento.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeUnit {
    pub ids: Vec<String>,
    pub bounds: Bounds,
}

/// Las formas de un trabajo y, si no se pueden usar, por qué.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShapeScan {
    pub shapes: Vec<ShapeUnit>,
    /// Texto del aviso cuando el trabajo no admite piezas por formas (ya traducido).
    pub unsupported: Option<String>,
}

/// Las formas que el trabajo marca, tal como las deja `place` (objetos seleccionados y con salida).
pub fn scan_shapes(doc: &Document, options: &JobOptions) -> Result<ShapeScan, String> {
    let mut warnings = Vec::new();
    let opts = JobOptions {
        tile: None,
        ..options.clone()
    };
    let placement = place(doc, &opts, None, &mut warnings)?;
    let mut scan = ShapeScan::default();
    if options.transform.is_some() {
        scan.unsupported = Some(tr!("pipeline.split.whole_transform"));
    }
    // (grupo | objeto) -> (posiciones de objeto, caja). Los grupos van antes que los objetos
    // sueltos para que el orden sea estable.
    let mut units: BTreeMap<(u8, usize), (Vec<usize>, Bounds)> = BTreeMap::new();
    for layer in &placement.layers {
        if !layer.images.is_empty() {
            scan.unsupported = Some(tr!("pipeline.split.whole_images"));
        }
        for (path, meta) in layer.paths.iter().zip(&layer.meta) {
            let Some(b) = bounds_of(std::slice::from_ref(path)) else {
                continue;
            };
            let key = match meta.group {
                Some(g) => (0, g),
                None => (1, meta.object),
            };
            let entry = units.entry(key).or_insert_with(|| (Vec::new(), b));
            if !entry.0.contains(&meta.object) {
                entry.0.push(meta.object);
            }
            entry.1 = union(entry.1, b);
        }
    }
    scan.shapes = units
        .into_values()
        .map(|(mut objects, bounds)| {
            objects.sort_unstable();
            ShapeUnit {
                ids: objects
                    .iter()
                    .filter_map(|&i| doc.objects.get(i))
                    .map(|o| o.id.clone())
                    .collect(),
                bounds,
            }
        })
        .collect();
    Ok(scan)
}

fn extent(b: &Bounds, axis: usize) -> (f64, f64) {
    if axis == 0 {
        (b.min.x, b.max.x)
    } else {
        (b.min.y, b.max.y)
    }
}

fn mid(b: &Bounds, axis: usize) -> f64 {
    let (lo, hi) = extent(b, axis);
    (lo + hi) / 2.0
}

/// Reparte `items` (índices en `shapes`) en grupos a lo largo de `axis` (0 = X, 1 = Y): se ordenan
/// por el centro y un grupo crece mientras su cobertura quepa en `side`.
fn group_along(
    items: &[usize],
    shapes: &[ShapeUnit],
    axis: usize,
    descending: bool,
    side: f64,
) -> Vec<Vec<usize>> {
    let mut order = items.to_vec();
    order.sort_by(|&a, &b| {
        let (ma, mb) = (mid(&shapes[a].bounds, axis), mid(&shapes[b].bounds, axis));
        let o = ma.total_cmp(&mb);
        if descending { o.reverse() } else { o }
    });
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut span: Option<(f64, f64)> = None;
    for i in order {
        let (lo, hi) = extent(&shapes[i].bounds, axis);
        match (&mut span, groups.last_mut()) {
            (Some((glo, ghi)), Some(g)) if hi.max(*ghi) - lo.min(*glo) <= side + EPS => {
                *glo = glo.min(lo);
                *ghi = ghi.max(hi);
                g.push(i);
            }
            _ => {
                span = Some((lo, hi));
                groups.push(vec![i]);
            }
        }
    }
    groups
}

/// Las piezas por formas enteras, o `None` (con un aviso en `warnings`) si alguna forma no cabe en
/// el lado `side`. `cut` es (se puede partir en columnas, se puede partir en filas).
pub(crate) fn whole_pieces(
    shapes: &[ShapeUnit],
    side: f64,
    cut: (bool, bool),
    order: SplitOrder,
    warnings: &mut Vec<String>,
) -> Option<Vec<Piece>> {
    if shapes.is_empty() {
        return None;
    }
    for s in shapes {
        let (w, h) = (
            s.bounds.max.x - s.bounds.min.x,
            s.bounds.max.y - s.bounds.min.y,
        );
        if w > side + EPS || h > side + EPS {
            warnings.push(tr!(
                "pipeline.split.whole_too_big",
                id = s.ids.first().map_or("?", String::as_str),
                w = format!("{w:.2}"),
                h = format!("{h:.2}"),
                side = format!("{side:.2}")
            ));
            return None;
        }
    }
    let all: Vec<usize> = (0..shapes.len()).collect();
    let rows = if cut.1 {
        group_along(&all, shapes, 1, true, side)
    } else {
        vec![all]
    };
    let mut out = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        let mut cols = if cut.0 {
            group_along(row, shapes, 0, false, side)
        } else {
            vec![row.clone()]
        };
        if order == SplitOrder::Snake && r % 2 == 1 {
            cols.reverse();
        }
        for group in cols {
            let b = group
                .iter()
                .map(|&i| shapes[i].bounds)
                .reduce(union)
                .expect("grupo no vacío");
            let mut ids: Vec<String> = group
                .iter()
                .flat_map(|&i| shapes[i].ids.iter().cloned())
                .collect();
            ids.dedup();
            out.push(Piece {
                rect: [
                    [b.min.x - PAD_MM, b.min.y - PAD_MM],
                    [b.max.x + PAD_MM, b.max.y + PAD_MM],
                ],
                objects: Some(ids),
            });
        }
    }
    Some(out)
}

/// La secuencia de marcado de un documento: con `split.whole_shapes` activo parte por formas
/// enteras (si el trabajo lo admite) y si no, como [`mark_sequence`]. `whole` es el trabajo sin
/// recorte (da la caja del diseño) y `options` las del trabajo.
pub fn doc_mark_sequence(
    doc: &Document,
    whole: &ComputedJob,
    device: &DeviceProfile,
    options: &JobOptions,
) -> Result<MarkPlan, String> {
    let design = whole
        .bounds()
        .ok_or_else(|| tr!("pipeline.split.no_motion"))?;
    let g = &device.galvo.split;
    if !(g.enabled && g.whole_shapes) {
        return mark_sequence(design, device);
    }
    let scan = scan_shapes(doc, options)?;
    if let Some(reason) = scan.unsupported {
        let mut plan = mark_sequence(design, device)?;
        plan.warnings.insert(0, reason);
        return Ok(plan);
    }
    mark_sequence_shapes(design, &scan.shapes, device)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_core::bezpath::BezPath;
    use tlaku_core::{Point, Polyline, Rgb};
    use tlaku_doc::{
        DesignObject, DeviceKind, GalvoDevice, LayerSettings, ObjectKind, RepeatMarking,
        SplitMarking, VectorObject, affine,
    };

    const RED: Rgb = Rgb(255, 0, 0);

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Bounds {
        Bounds {
            min: Point::new(x, y),
            max: Point::new(x + w, y + h),
        }
    }

    fn shape(id: &str, x: f64, y: f64, w: f64, h: f64) -> ShapeUnit {
        ShapeUnit {
            ids: vec![id.into()],
            bounds: rect(x, y, w, h),
        }
    }

    fn device(split: SplitMarking) -> DeviceProfile {
        DeviceProfile {
            kind: DeviceKind::Galvo,
            bed: [100.0, 100.0],
            galvo: GalvoDevice {
                field_mm: 100.0,
                split,
                repeat: RepeatMarking::default(),
                ..GalvoDevice::default()
            },
            ..DeviceProfile::default()
        }
    }

    fn whole() -> SplitMarking {
        SplitMarking {
            enabled: true,
            whole_shapes: true,
            ..SplitMarking::default()
        }
    }

    fn ids(plan: &MarkPlan) -> Vec<Vec<String>> {
        plan.steps
            .iter()
            .map(|s| s.objects.clone().expect("con formas enteras"))
            .collect()
    }

    /// Cálculo a mano. Campo de 100 mm. Cinco cuadrados de 30 mm en una fila de 260 mm:
    /// a x = 0, 40, 80 (caja 0..110, no cabe en 100), 120, 160.
    /// - Los tres primeros cubren 0..110 > 100: el voraz toma a y b (0..70) y cierra el grupo
    ///   porque con c llegaría a 110.
    /// - c (80..110) y d (120..150) cubren 80..150 = 70 y con e (160..190) llegan a 110 > 100.
    /// - e queda solo.
    ///   Un recorte fijo en 100 y 200 habría cortado a c (80..110) y a e/d: aquí ninguna se parte.
    #[test]
    fn a_row_is_cut_in_the_gaps_between_shapes() {
        let shapes = [
            shape("a", 0.0, 0.0, 30.0, 30.0),
            shape("b", 40.0, 0.0, 30.0, 30.0),
            shape("c", 80.0, 0.0, 30.0, 30.0),
            shape("d", 120.0, 0.0, 30.0, 30.0),
            shape("e", 160.0, 0.0, 30.0, 30.0),
        ];
        let d = device(whole());
        let plan = mark_sequence_shapes(rect(0.0, 0.0, 190.0, 30.0), &shapes, &d).unwrap();
        assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);
        assert_eq!(ids(&plan), vec![vec!["a", "b"], vec!["c", "d"], vec!["e"]]);
        let rects: Vec<_> = plan
            .steps
            .iter()
            .map(|s| s.tile.as_ref().unwrap().rect)
            .collect();
        // Cada pieza es la caja de sus formas más 0,01 mm por lado.
        assert_eq!(rects[0], [[-0.01, -0.01], [70.01, 30.01]]);
        assert_eq!(rects[1], [[79.99, -0.01], [150.01, 30.01]]);
        assert_eq!(rects[2], [[159.99, -0.01], [190.01, 30.01]]);
        // Ninguna forma está en dos piezas.
        let mut all: Vec<String> = ids(&plan).into_iter().flatten().collect();
        all.sort();
        assert_eq!(all, vec!["a", "b", "c", "d", "e"]);
        // El centro de cada pieza va al centro de la cama (50, 50): la primera, de (35, 15).
        let t = plan.steps[0].tile.as_ref().unwrap();
        assert!((t.offset[0] - 15.0).abs() < 1e-9 && (t.offset[1] - 35.0).abs() < 1e-9);
        // La mesa avanza lo que separa los centros: (35, 15) a (115, 15) = -80 en X.
        assert!((plan.steps[1].stage.dx + 80.0).abs() < 1e-9);
        assert_eq!(plan.steps[1].stage.dy, 0.0);
    }

    /// Filas primero (de arriba abajo) y columnas dentro de cada una; la segunda fila va al revés
    /// con el orden en zigzag.
    #[test]
    fn rows_then_columns_in_a_snake_order() {
        let shapes = [
            shape("a", 0.0, 120.0, 40.0, 40.0),
            shape("b", 60.0, 120.0, 40.0, 40.0),
            shape("c", 0.0, 0.0, 40.0, 40.0),
            shape("d", 60.0, 0.0, 40.0, 40.0),
            shape("e", 120.0, 0.0, 40.0, 40.0),
        ];
        let d = device(whole());
        let plan = mark_sequence_shapes(rect(0.0, 0.0, 160.0, 160.0), &shapes, &d).unwrap();
        // Arriba: a y b (0..100 de ancho); abajo: c, d (0..100) y e. En zigzag la fila de abajo
        // recorre las columnas de derecha a izquierda: e, luego c y d.
        assert_eq!(ids(&plan), vec![vec!["a", "b"], vec!["e"], vec!["c", "d"]]);
        let rows = SplitMarking {
            order: SplitOrder::Rows,
            ..whole()
        };
        let plan =
            mark_sequence_shapes(rect(0.0, 0.0, 160.0, 160.0), &shapes, &device(rows)).unwrap();
        assert_eq!(ids(&plan), vec![vec!["a", "b"], vec!["c", "d"], vec!["e"]]);
    }

    #[test]
    fn a_shape_larger_than_the_field_falls_back_to_strips_with_a_warning() {
        let shapes = [
            shape("grande", 0.0, 0.0, 130.0, 20.0),
            shape("chica", 140.0, 0.0, 10.0, 10.0),
        ];
        let d = device(whole());
        let plan = mark_sequence_shapes(rect(0.0, 0.0, 150.0, 20.0), &shapes, &d).unwrap();
        assert!(plan.steps.iter().all(|s| s.objects.is_none()));
        assert_eq!(plan.steps.len(), 2, "franjas de 100 mm: 0..100 y 100..150");
        assert_eq!(plan.warnings.len(), 1);
        assert!(plan.warnings[0].contains("grande"), "{:?}", plan.warnings);
    }

    #[test]
    fn rotary_strips_follow_the_axis_and_cut_only_along_the_surface() {
        // Rotativo con el eje en X: la superficie avanza en Y, solo se parte en filas.
        let mut d = device(whole());
        d.galvo.rotary.enabled = true;
        d.galvo.rotary.diameter_mm = 100.0 / std::f64::consts::PI; // 100 mm por vuelta
        let shapes = [
            shape("a", 0.0, 120.0, 90.0, 30.0),
            shape("b", 0.0, 60.0, 90.0, 30.0),
            shape("c", 0.0, 0.0, 90.0, 30.0),
        ];
        let plan = mark_sequence_shapes(rect(0.0, 0.0, 90.0, 150.0), &shapes, &d).unwrap();
        // Filas de a lo más 100 mm: a y b cubren 60..150 = 90; con c serían 150. c queda sola.
        assert_eq!(ids(&plan), vec![vec!["a", "b"], vec!["c"]]);
        // Centros en Y: (60+150)/2 = 105 y 15; baja 90 mm = 90 / 100 de vuelta = 324 grados.
        assert!(
            (plan.steps[1].stage.rotary_deg + 324.0).abs() < 1e-6,
            "{:?}",
            plan.steps[1].stage
        );
        assert_eq!((plan.steps[1].stage.dx, plan.steps[1].stage.dy), (0.0, 0.0));
    }

    #[test]
    fn without_whole_shapes_the_normal_strips_are_used() {
        let shapes = [
            shape("a", 0.0, 0.0, 30.0, 30.0),
            shape("b", 120.0, 0.0, 30.0, 30.0),
        ];
        let d = device(SplitMarking {
            enabled: true,
            ..SplitMarking::default()
        });
        let plan = mark_sequence_shapes(rect(0.0, 0.0, 150.0, 30.0), &shapes, &d).unwrap();
        assert!(plan.steps.iter().all(|s| s.objects.is_none()));
    }

    fn square_object(id: &str, x: f64, y: f64, s: f64, group: Option<&str>) -> DesignObject {
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
            group: group.map(String::from),
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
    fn scanning_a_document_gives_one_unit_per_object_and_per_group() {
        let doc = doc_of(vec![
            square_object("solo", 0.0, 0.0, 10.0, None),
            square_object("g1", 30.0, 0.0, 10.0, Some("grupo")),
            square_object("g2", 50.0, 20.0, 10.0, Some("grupo")),
        ]);
        let scan = scan_shapes(&doc, &JobOptions::default()).unwrap();
        assert_eq!(scan.unsupported, None);
        assert_eq!(scan.shapes.len(), 2);
        // Primero los grupos: g1 y g2 forman una forma de 30..60 x 0..30.
        assert_eq!(scan.shapes[0].ids, vec!["g1", "g2"]);
        assert_eq!(scan.shapes[0].bounds, rect(30.0, 0.0, 30.0, 30.0));
        assert_eq!(scan.shapes[1].ids, vec!["solo"]);
        // Un objeto oculto o fuera de la selección no entra.
        let opts = JobOptions {
            selection: Some(vec!["solo".into()]),
            ..JobOptions::default()
        };
        assert_eq!(scan_shapes(&doc, &opts).unwrap().shapes.len(), 1);
    }

    #[test]
    fn images_and_registration_transforms_make_the_scan_unsupported() {
        let opts = JobOptions {
            transform: Some(tlaku_doc::affine::IDENTITY),
            ..JobOptions::default()
        };
        let doc = doc_of(vec![square_object("a", 0.0, 0.0, 10.0, None)]);
        let scan = scan_shapes(&doc, &opts).unwrap();
        assert!(scan.unsupported.is_some());
    }
}
