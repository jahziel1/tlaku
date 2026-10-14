//! Comando `geometry_op`: booleanas, soldar y offset sobre los objetos del diseño.
//!
//! Los objetos llegan con sus curvas y su `transform`; se aplanan en coordenadas de documento
//! (tolerancia 0,01 mm), se opera con `tlaku-geom` y el resultado vuelve como objetos vectoriales
//! nuevos (transform identidad, curvas de rectas), listos para sustituir a los operandos.
//!
//! Convenciones:
//! - Booleanas (`union`, `difference`, `intersection`, `xor`): el **primer** objeto de la lista es A
//!   y **todos los demás forman el conjunto B**. `difference` es A menos B; `intersection` es A
//!   con la unión de B; `xor` es A contra la unión de B. Quien llama decide cuál es A (la
//!   interfaz deja elegirlo; el orden de selección nunca es implícito). Un grupo se manda como un
//!   único objeto con todas sus curvas.
//! - `weld`: une todos los objetos en una región; conserva agujeros e islas.
//! - `offset`: cada objeto se desplaza por separado (un grupo, como uno solo). El resultado de
//!   cada uno conserva la capa de su origen. `distance` positiva = hacia afuera.
//! - Las trayectorias abiertas no pueden participar: se rechazan diciendo qué objetos son.
//! - Cada pieza del resultado (contorno exterior con sus agujeros) es un objeto; el resultado de
//!   una booleana o de `weld` va en la capa del primer operando.
//!
//! Limpieza de trazos (`join`, `close`, `dedupe`), con `tolerance` en mm:
//! - Aceptan trayectorias abiertas (a diferencia de las booleanas) pero no imágenes ni textos
//!   (un texto que cambiara dejaría de ser texto: conviene convertirlo antes).
//! - Operan por capa: nunca mezclan objetos de capas distintas. `join` (por defecto 0,05 mm) une
//!   extremos de trayectorias abiertas, también entre objetos distintos, invirtiendo sentidos si
//!   hace falta, y marca cerrada la que termina donde empieza. `close` (por defecto 0,5 mm; es
//!   mayor porque cierra una sola trayectoria consigo misma, donde un hueco de medio milímetro
//!   casi siempre es un descuido de dibujo y no hay otra pieza a la que confundir) cierra las
//!   abiertas cuyos extremos distan eso o menos. `dedupe` (por defecto 0,01 mm, el aplanado del
//!   documento) quita los tramos colineales repetidos o superpuestos entre todos los objetos de
//!   la capa, conserva el primero y parte los segmentos que se solapan solo en parte.
//! - Trabajan con las curvas aplanadas (0,01 mm). **Solo los objetos que cambian** pierden sus
//!   curvas (el resultado son rectas); los que no cambian se devuelven intactos.
//! - Resultado: la lista completa de objetos de entrada, en su orden, donde cada objeto sin
//!   cambios aparece idéntico (mismo id) y cada objeto que cambia se sustituye por uno nuevo con
//!   id `join-N`, `close-N` o `dedupe-N`, capa y grupo de su origen (grupo `None` si las fuentes
//!   eran de grupos distintos). Los objetos que dejan de tener trazo (un duplicado exacto)
//!   desaparecen de la lista. Quien llama: los ids de entrada que ya no están en el resultado se
//!   eliminan, los que siguen se dejan como están y los ids nuevos se insertan donde estaba el
//!   primero eliminado. `join` fusiona en un solo objeto los que quedaron enlazados por un trazo
//!   común; `close` y `dedupe` nunca fusionan objetos.

use serde::Deserialize;
use std::collections::{BTreeSet, HashSet};
use tlaku_doc::{DesignObject, ObjectKind, VectorObject, affine};
use tlaku_geom::{
    BoolOp, Join, Polyline, auto_join_tracked, boolean, close_paths, dedupe_paths, offset,
    signed_area, weld,
};
use tlaku_i18n::tr;

/// Error máximo (mm) al aplanar las curvas y en los arcos del offset.
const TOLERANCE_MM: f64 = 0.01;
/// Tolerancia por defecto (mm) de `join`: solo extremos que prácticamente se tocan.
const DEFAULT_JOIN_MM: f64 = 0.05;
/// Tolerancia por defecto (mm) de `close`: ver la nota del módulo.
const DEFAULT_CLOSE_MM: f64 = 0.5;
/// Tolerancia por defecto (mm) de `dedupe`: la del aplanado de curvas.
const DEFAULT_DEDUPE_MM: f64 = 0.01;
/// Largo máximo de la punta de una esquina en ángulo, en múltiplos de la distancia.
const MITER_LIMIT: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryOp {
    Union,
    Difference,
    Intersection,
    Xor,
    Weld,
    Offset,
    Join,
    Close,
    Dedupe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JoinStyle {
    Round,
    Miter,
    Bevel,
}

/// Geometría de un objeto en coordenadas de documento.
struct Operand {
    id: String,
    paths: Vec<Polyline>,
}

fn flatten(object: &DesignObject) -> Vec<Polyline> {
    object.polylines_with(TOLERANCE_MM)
}

/// Verifica que los operandos se puedan usar y los aplana. Los errores nombran los objetos.
fn operands(objects: &[DesignObject]) -> Result<Vec<Operand>, String> {
    let mut images = Vec::new();
    let mut empty = Vec::new();
    let mut open = Vec::new();
    let mut out = Vec::new();
    for o in objects {
        if matches!(o.kind, ObjectKind::Image(_)) {
            images.push(o.id.as_str());
            continue;
        }
        let paths = flatten(o);
        if paths.is_empty() {
            empty.push(o.id.as_str());
        } else if paths.iter().any(|p| !p.closed) {
            open.push(o.id.as_str());
        } else {
            out.push(Operand {
                id: o.id.clone(),
                paths,
            });
        }
    }
    let list = |ids: &[&str]| {
        ids.iter()
            .map(crate::edit::quoted)
            .collect::<Vec<_>>()
            .join(", ")
    };
    if !images.is_empty() {
        return Err(tr!(
            "desktop.edit.geometry.images_not_allowed",
            list = list(&images)
        ));
    }
    if !open.is_empty() {
        return Err(tr!("desktop.edit.geometry.open_shapes", list = list(&open)));
    }
    if !empty.is_empty() {
        return Err(tr!("desktop.edit.geometry.no_outline", list = list(&empty)));
    }
    Ok(out)
}

/// Parte una región (cada exterior seguido de sus agujeros, como la entrega `tlaku-geom`) en
/// piezas: una por exterior, con sus agujeros.
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

fn vector_object(id: String, layer: tlaku_core::Rgb, piece: &[Polyline]) -> DesignObject {
    DesignObject {
        id,
        layer,
        transform: affine::IDENTITY,
        kind: ObjectKind::Vector(VectorObject::new(
            piece.iter().map(tlaku_doc::BezPath::from).collect(),
        )),
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

/// Convierte una región en objetos (uno por pieza) con ids `{prefix}-{n}`.
fn region_objects(
    prefix: &str,
    first: &mut usize,
    layer: tlaku_core::Rgb,
    region: Vec<Polyline>,
) -> Vec<DesignObject> {
    pieces(region)
        .iter()
        .map(|piece| {
            *first += 1;
            vector_object(format!("{prefix}-{first}"), layer, piece)
        })
        .collect()
}

fn join_of(style: Option<JoinStyle>) -> Join {
    match style.unwrap_or(JoinStyle::Round) {
        JoinStyle::Round => Join::Round,
        JoinStyle::Miter => Join::Miter { limit: MITER_LIMIT },
        JoinStyle::Bevel => Join::Bevel,
    }
}

/// Núcleo puro de `geometry_op` (sin Tauri), para poder probarlo.
pub fn run(
    op: GeometryOp,
    objects: &[DesignObject],
    distance: Option<f64>,
    join: Option<JoinStyle>,
) -> Result<Vec<DesignObject>, String> {
    if matches!(
        op,
        GeometryOp::Join | GeometryOp::Close | GeometryOp::Dedupe
    ) {
        return cleanup(op, objects, None);
    }
    let Some(first) = objects.first() else {
        return Err(tr!("desktop.edit.nothing_to_operate"));
    };
    let layer = first.layer;
    let ops = operands(objects)?;
    let mut counter = 0usize;
    match op {
        GeometryOp::Union | GeometryOp::Difference | GeometryOp::Intersection | GeometryOp::Xor => {
            if ops.len() < 2 {
                return Err(tr!("desktop.edit.geometry.boolean_needs_two"));
            }
            let bool_op = match op {
                GeometryOp::Union => BoolOp::Union,
                GeometryOp::Difference => BoolOp::Difference,
                GeometryOp::Intersection => BoolOp::Intersection,
                _ => BoolOp::Xor,
            };
            let a = &ops[0].paths;
            let b: Vec<Polyline> = ops[1..].iter().flat_map(|o| o.paths.clone()).collect();
            let region = boolean(a, &b, bool_op);
            Ok(region_objects("geo", &mut counter, layer, region))
        }
        GeometryOp::Weld => {
            let all: Vec<Polyline> = ops.iter().flat_map(|o| o.paths.clone()).collect();
            Ok(region_objects("geo", &mut counter, layer, weld(&all)))
        }
        GeometryOp::Offset => {
            let d = distance.ok_or_else(|| tr!("desktop.edit.geometry.offset_missing"))?;
            if !d.is_finite() {
                return Err(tr!("desktop.edit.geometry.offset_invalid"));
            }
            let style = join_of(join);
            let mut out = Vec::new();
            for (operand, object) in ops.iter().zip(objects) {
                let region = offset(&operand.paths, d, style, TOLERANCE_MM);
                if region.is_empty() {
                    continue;
                }
                let prefix = format!("{}-off", operand.id);
                let mut n = 0usize;
                out.extend(region_objects(&prefix, &mut n, object.layer, region));
            }
            Ok(out)
        }
        GeometryOp::Join | GeometryOp::Close | GeometryOp::Dedupe => cleanup(op, objects, None),
    }
}

/// Agrupación de objetos por componentes (unión-búsqueda con compresión de caminos).
struct Components(Vec<usize>);

impl Components {
    fn new(n: usize) -> Self {
        Self((0..n).collect())
    }
    fn find(&mut self, i: usize) -> usize {
        let mut root = i;
        while self.0[root] != root {
            root = self.0[root];
        }
        let mut cur = i;
        while self.0[cur] != root {
            let next = self.0[cur];
            self.0[cur] = root;
            cur = next;
        }
        root
    }
    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        // La raíz es siempre el menor índice: el objeto nuevo ocupa el lugar del primero.
        self.0[ra.max(rb)] = ra.min(rb);
    }
}

/// Qué pasa con cada objeto de entrada tras una limpieza.
enum Slot {
    Keep,
    Gone,
    New(Box<DesignObject>),
}

/// Objeto nuevo en la capa y el grupo de sus fuentes (`members`, índices en `objects`).
fn replacement(
    id: String,
    objects: &[DesignObject],
    members: &[usize],
    paths: &[Polyline],
) -> DesignObject {
    let first = &objects[members[0]];
    let same_group = members.iter().all(|&m| objects[m].group == first.group);
    DesignObject {
        group: if same_group {
            first.group.clone()
        } else {
            None
        },
        hidden: first.hidden,
        ..vector_object(id, first.layer, paths)
    }
}

/// Núcleo de `join`, `close` y `dedupe` (ver la nota del módulo).
pub fn cleanup(
    op: GeometryOp,
    objects: &[DesignObject],
    tolerance: Option<f64>,
) -> Result<Vec<DesignObject>, String> {
    if objects.is_empty() {
        return Err(tr!("desktop.edit.nothing_to_operate"));
    }
    let (prefix, default_tol) = match op {
        GeometryOp::Join => ("join", DEFAULT_JOIN_MM),
        GeometryOp::Close => ("close", DEFAULT_CLOSE_MM),
        GeometryOp::Dedupe => ("dedupe", DEFAULT_DEDUPE_MM),
        _ => return Err(tr!("desktop.edit.geometry.not_cleanup")),
    };
    let tol = tolerance.unwrap_or(default_tol);
    if !tol.is_finite() || tol < 0.0 {
        return Err(tr!("desktop.edit.bad_tolerance"));
    }
    let unsupported: Vec<String> = objects
        .iter()
        .filter(|o| !matches!(o.kind, ObjectKind::Vector(_)))
        .map(|o| crate::edit::quoted(&o.id))
        .collect();
    if !unsupported.is_empty() {
        return Err(tr!(
            "desktop.edit.geometry.cleanup_vectors_only",
            list = unsupported.join(", ")
        ));
    }

    let flat: Vec<Vec<Polyline>> = objects.iter().map(flatten).collect();
    let mut slots: Vec<Slot> = objects.iter().map(|_| Slot::Keep).collect();
    let taken: HashSet<&str> = objects.iter().map(|o| o.id.as_str()).collect();
    let mut counter = 0usize;
    let mut fresh_id = || loop {
        counter += 1;
        let id = format!("{prefix}-{counter}");
        if !taken.contains(id.as_str()) {
            break id;
        }
    };

    // Capas en orden de aparición; cada una con los índices de sus objetos.
    let mut layers: Vec<(tlaku_core::Rgb, Vec<usize>)> = Vec::new();
    for (i, o) in objects.iter().enumerate() {
        match layers.iter_mut().find(|(l, _)| *l == o.layer) {
            Some((_, list)) => list.push(i),
            None => layers.push((o.layer, vec![i])),
        }
    }

    for (_, members) in &layers {
        // Todas las trayectorias de la capa, con el objeto del que vienen.
        let mut all: Vec<Polyline> = Vec::new();
        let mut owner: Vec<usize> = Vec::new();
        for &m in members {
            for p in &flat[m] {
                all.push(p.clone());
                owner.push(m);
            }
        }
        match op {
            GeometryOp::Join => {
                let joined = auto_join_tracked(&all, tol);
                let mut comps = Components::new(objects.len());
                let mut dirty = vec![false; objects.len()];
                for (path, sources) in &joined {
                    for &s in &sources[1..] {
                        comps.union(owner[sources[0]], owner[s]);
                    }
                    // Una sola fuente cambia solo si se cerró sobre sí misma.
                    if sources.len() > 1 || path.closed != all[sources[0]].closed {
                        dirty[owner[sources[0]]] = true;
                    }
                }
                let mut dirty_roots = BTreeSet::new();
                for &m in members {
                    if dirty[m] {
                        dirty_roots.insert(comps.find(m));
                    }
                }
                for root in dirty_roots {
                    let group: Vec<usize> = members
                        .iter()
                        .copied()
                        .filter(|&m| comps.find(m) == root)
                        .collect();
                    let paths: Vec<Polyline> = joined
                        .iter()
                        .filter(|(_, src)| comps.find(owner[src[0]]) == root)
                        .map(|(p, _)| p.clone())
                        .collect();
                    for &m in &group[1..] {
                        slots[m] = Slot::Gone;
                    }
                    slots[group[0]] =
                        Slot::New(Box::new(replacement(fresh_id(), objects, &group, &paths)));
                }
            }
            GeometryOp::Close => {
                for &m in members {
                    let closed = close_paths(&flat[m], tol);
                    if closed != flat[m] {
                        slots[m] =
                            Slot::New(Box::new(replacement(fresh_id(), objects, &[m], &closed)));
                    }
                }
            }
            _ => {
                let result = dedupe_paths(&all, tol);
                let mut touched = vec![false; objects.len()];
                for (k, &c) in result.changed.iter().enumerate() {
                    if c {
                        touched[owner[k]] = true;
                    }
                }
                for &m in members.iter().filter(|&&m| touched[m]) {
                    let paths: Vec<Polyline> = result
                        .paths
                        .iter()
                        .zip(&result.origin)
                        .filter(|(_, o)| owner[**o] == m)
                        .map(|(p, _)| p.clone())
                        .collect();
                    slots[m] = if paths.is_empty() {
                        Slot::Gone
                    } else {
                        Slot::New(Box::new(replacement(fresh_id(), objects, &[m], &paths)))
                    };
                }
            }
        }
    }

    Ok(objects
        .iter()
        .zip(slots)
        .filter_map(|(o, slot)| match slot {
            Slot::Keep => Some(o.clone()),
            Slot::Gone => None,
            Slot::New(n) => Some(*n),
        })
        .collect())
}

/// Booleanas, soldar, offset y limpieza de trazos (`join`, `close`, `dedupe`; ver la nota del
/// módulo). `objects` en coordenadas de documento (cada uno con su
/// `transform`); el resultado también (transform identidad). Corre fuera del hilo principal.
#[tauri::command]
pub async fn geometry_op(
    op: GeometryOp,
    objects: Vec<DesignObject>,
    distance: Option<f64>,
    join: Option<JoinStyle>,
    tolerance: Option<f64>,
) -> Result<Vec<DesignObject>, String> {
    tauri::async_runtime::spawn_blocking(move || match op {
        GeometryOp::Join | GeometryOp::Close | GeometryOp::Dedupe => {
            cleanup(op, &objects, tolerance)
        }
        _ => run(op, &objects, distance, join),
    })
    .await
    .map_err(crate::edit::task_interrupted)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_core::{Point, Rgb};
    use tlaku_doc::BezPath;

    fn rect(id: &str, x0: f64, y0: f64, x1: f64, y1: f64) -> DesignObject {
        let poly = Polyline {
            points: vec![
                Point::new(x0, y0),
                Point::new(x1, y0),
                Point::new(x1, y1),
                Point::new(x0, y1),
            ],
            closed: true,
        };
        DesignObject {
            id: id.into(),
            layer: Rgb(255, 0, 0),
            transform: affine::IDENTITY,
            kind: ObjectKind::Vector(VectorObject::new(vec![BezPath::from(&poly)])),
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

    fn area(objects: &[DesignObject]) -> f64 {
        objects
            .iter()
            .flat_map(|o| o.polylines())
            .map(|p| signed_area(&p.points))
            .sum()
    }

    #[test]
    fn union_de_dos_rectangulos() {
        let r = run(
            GeometryOp::Union,
            &[
                rect("a", 0.0, 0.0, 10.0, 10.0),
                rect("b", 5.0, 0.0, 15.0, 10.0),
            ],
            None,
            None,
        )
        .unwrap();
        assert_eq!(r.len(), 1);
        assert!((area(&r) - 150.0).abs() < 1e-6);
        assert_eq!(r[0].transform, affine::IDENTITY);
    }

    #[test]
    fn diferencia_es_a_menos_b_y_el_orden_importa() {
        let a = rect("a", 0.0, 0.0, 10.0, 10.0);
        let b = rect("b", 5.0, 0.0, 15.0, 10.0);
        let ab = run(GeometryOp::Difference, &[a.clone(), b.clone()], None, None).unwrap();
        assert!((area(&ab) - 50.0).abs() < 1e-6);
        let b_menos_a = run(GeometryOp::Difference, &[b, a], None, None).unwrap();
        let bb = b_menos_a[0].bounds().unwrap();
        assert!((bb.min.x - 10.0).abs() < 1e-6 && (bb.max.x - 15.0).abs() < 1e-6);
    }

    #[test]
    fn el_resto_forma_el_conjunto_b() {
        // A menos (B1 y B2): dos mordidas a los lados.
        let r = run(
            GeometryOp::Difference,
            &[
                rect("a", 0.0, 0.0, 30.0, 10.0),
                rect("b1", -5.0, 0.0, 5.0, 10.0),
                rect("b2", 25.0, 0.0, 35.0, 10.0),
            ],
            None,
            None,
        )
        .unwrap();
        assert!((area(&r) - 200.0).abs() < 1e-6);
    }

    #[test]
    fn interseccion_vacia_devuelve_lista_vacia() {
        let r = run(
            GeometryOp::Intersection,
            &[
                rect("a", 0.0, 0.0, 5.0, 5.0),
                rect("b", 10.0, 10.0, 20.0, 20.0),
            ],
            None,
            None,
        )
        .unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn xor_deja_las_partes_no_comunes() {
        let r = run(
            GeometryOp::Xor,
            &[
                rect("a", 0.0, 0.0, 10.0, 10.0),
                rect("b", 5.0, 0.0, 15.0, 10.0),
            ],
            None,
            None,
        )
        .unwrap();
        assert!((area(&r) - 100.0).abs() < 1e-6);
        assert_eq!(r.len(), 2);
    }

    #[test]
    fn respeta_el_transform_del_operando() {
        let mut b = rect("b", 0.0, 0.0, 10.0, 10.0);
        b.transform = affine::translate(5.0, 0.0);
        let r = run(
            GeometryOp::Intersection,
            &[rect("a", 0.0, 0.0, 10.0, 10.0), b],
            None,
            None,
        )
        .unwrap();
        let bb = r[0].bounds().unwrap();
        assert!((bb.min.x - 5.0).abs() < 1e-6 && (bb.max.x - 10.0).abs() < 1e-6);
    }

    #[test]
    fn el_resultado_va_en_la_capa_del_primero() {
        let mut b = rect("b", 5.0, 0.0, 15.0, 10.0);
        b.layer = Rgb(0, 0, 255);
        let r = run(
            GeometryOp::Union,
            &[rect("a", 0.0, 0.0, 10.0, 10.0), b],
            None,
            None,
        )
        .unwrap();
        assert_eq!(r[0].layer, Rgb(255, 0, 0));
    }

    #[test]
    fn weld_conserva_el_agujero() {
        // Un marco (cuadrado con hueco) soldado con un cuadrado aparte que lo toca por fuera.
        let marco = {
            let mut o = rect("m", 0.0, 0.0, 20.0, 20.0);
            if let ObjectKind::Vector(v) = &mut o.kind {
                let hueco = Polyline {
                    points: vec![
                        Point::new(5.0, 5.0),
                        Point::new(15.0, 5.0),
                        Point::new(15.0, 15.0),
                        Point::new(5.0, 15.0),
                    ],
                    closed: true,
                };
                v.paths.push(BezPath::from(&hueco));
            }
            o
        };
        let r = run(
            GeometryOp::Weld,
            &[marco, rect("c", 18.0, 0.0, 30.0, 10.0)],
            None,
            None,
        )
        .unwrap();
        assert_eq!(r.len(), 1);
        // Marco (400 - 100) más la parte del cuadrado que queda fuera (10 x 10).
        assert!((area(&r) - 400.0).abs() < 1e-6, "{}", area(&r));
        let paths = match &r[0].kind {
            ObjectKind::Vector(v) => v.paths.len(),
            _ => 0,
        };
        assert_eq!(paths, 2, "contorno exterior y agujero");
    }

    #[test]
    fn weld_de_piezas_separadas_da_un_objeto_por_pieza() {
        let r = run(
            GeometryOp::Weld,
            &[
                rect("a", 0.0, 0.0, 5.0, 5.0),
                rect("b", 10.0, 0.0, 15.0, 5.0),
            ],
            None,
            None,
        )
        .unwrap();
        assert_eq!(r.len(), 2);
    }

    #[test]
    fn offset_hacia_afuera_y_hacia_adentro() {
        let cuadrado = rect("a", 0.0, 0.0, 10.0, 10.0);
        let fuera = run(
            GeometryOp::Offset,
            std::slice::from_ref(&cuadrado),
            Some(2.0),
            Some(JoinStyle::Miter),
        )
        .unwrap();
        let bb = fuera[0].bounds().unwrap();
        assert!((bb.min.x + 2.0).abs() < 1e-6 && (bb.max.x - 12.0).abs() < 1e-6);
        let dentro = run(
            GeometryOp::Offset,
            std::slice::from_ref(&cuadrado),
            Some(-2.0),
            Some(JoinStyle::Round),
        )
        .unwrap();
        assert!((area(&dentro) - 36.0).abs() < 1e-6);
    }

    #[test]
    fn offset_redondo_es_mas_chico_que_en_angulo() {
        let cuadrado = rect("a", 0.0, 0.0, 10.0, 10.0);
        let redondo = run(
            GeometryOp::Offset,
            std::slice::from_ref(&cuadrado),
            Some(2.0),
            None,
        )
        .unwrap();
        let angulo = run(
            GeometryOp::Offset,
            std::slice::from_ref(&cuadrado),
            Some(2.0),
            Some(JoinStyle::Miter),
        )
        .unwrap();
        assert!(area(&redondo) < area(&angulo));
        // Cuadrado + 4 lados + 4 cuartos de círculo.
        let esperada = 100.0 + 4.0 * 10.0 * 2.0 + std::f64::consts::PI * 4.0;
        assert!(
            (area(&redondo) - esperada).abs() < 0.2,
            "{}",
            area(&redondo)
        );
    }

    #[test]
    fn offset_trata_cada_objeto_por_separado() {
        let mut b = rect("b", 20.0, 0.0, 30.0, 10.0);
        b.layer = Rgb(0, 0, 255);
        let r = run(
            GeometryOp::Offset,
            &[rect("a", 0.0, 0.0, 10.0, 10.0), b],
            Some(1.0),
            Some(JoinStyle::Miter),
        )
        .unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r[1].layer, Rgb(0, 0, 255));
        assert_ne!(r[0].id, r[1].id);
    }

    #[test]
    fn offset_que_borra_la_forma_devuelve_vacio() {
        let r = run(
            GeometryOp::Offset,
            &[rect("a", 0.0, 0.0, 4.0, 4.0)],
            Some(-3.0),
            None,
        )
        .unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn rechaza_formas_abiertas_diciendo_cuales() {
        let abierta = {
            let mut o = rect("linea", 0.0, 0.0, 10.0, 10.0);
            o.kind = ObjectKind::Vector(VectorObject::new(vec![BezPath::from(&Polyline {
                points: vec![Point::new(0.0, 0.0), Point::new(10.0, 10.0)],
                closed: false,
            })]));
            o
        };
        let e = run(
            GeometryOp::Union,
            &[rect("a", 0.0, 0.0, 10.0, 10.0), abierta],
            None,
            None,
        )
        .unwrap_err();
        assert!(e.contains("abiertas") && e.contains("«linea»"), "{e}");
        assert!(!e.contains("«a»"));
    }

    #[test]
    fn rechaza_imagenes_y_pocos_operandos() {
        let mut img = rect("foto", 0.0, 0.0, 1.0, 1.0);
        img.kind = ObjectKind::Image(tlaku_doc::ImageObject {
            source_path: "x.png".into(),
            width_px: 1,
            height_px: 1,
            size_mm: [1.0, 1.0],
            mask: Vec::new(),
        });
        let e = run(GeometryOp::Weld, &[img], None, None).unwrap_err();
        assert!(e.contains("«foto»"));
        let e = run(
            GeometryOp::Union,
            &[rect("a", 0.0, 0.0, 1.0, 1.0)],
            None,
            None,
        )
        .unwrap_err();
        assert!(e.contains("al menos dos"));
        assert!(
            run(
                GeometryOp::Offset,
                &[rect("a", 0.0, 0.0, 1.0, 1.0)],
                None,
                None
            )
            .is_err()
        );
        assert!(run(GeometryOp::Weld, &[], None, None).is_err());
    }

    #[test]
    fn aplana_curvas_antes_de_operar() {
        // Un círculo (cúbicas) menos su mitad derecha: queda cerca de media área.
        let circulo = {
            let k = 0.552_284_749_830_793_6 * 10.0;
            let p = |x, y| Point::new(x, y);
            let segs = vec![
                tlaku_doc::PathSeg::MoveTo { to: p(10.0, 0.0) },
                tlaku_doc::PathSeg::CubicTo {
                    ctrl1: p(10.0, k),
                    ctrl2: p(k, 10.0),
                    to: p(0.0, 10.0),
                },
                tlaku_doc::PathSeg::CubicTo {
                    ctrl1: p(-k, 10.0),
                    ctrl2: p(-10.0, k),
                    to: p(-10.0, 0.0),
                },
                tlaku_doc::PathSeg::CubicTo {
                    ctrl1: p(-10.0, -k),
                    ctrl2: p(-k, -10.0),
                    to: p(0.0, -10.0),
                },
                tlaku_doc::PathSeg::CubicTo {
                    ctrl1: p(k, -10.0),
                    ctrl2: p(10.0, -k),
                    to: p(10.0, 0.0),
                },
                tlaku_doc::PathSeg::Close,
            ];
            let mut o = rect("c", 0.0, 0.0, 1.0, 1.0);
            o.kind = ObjectKind::Vector(VectorObject::new(vec![BezPath(segs)]));
            o
        };
        let r = run(
            GeometryOp::Difference,
            &[circulo, rect("m", 0.0, -20.0, 20.0, 20.0)],
            None,
            None,
        )
        .unwrap();
        let esperada = std::f64::consts::PI * 100.0 / 2.0;
        assert!((area(&r) - esperada).abs() < 0.3, "{}", area(&r));
    }

    // --- Limpieza de trazos: join, close y dedupe ---

    fn trazo(id: &str, layer: Rgb, pts: &[(f64, f64)]) -> DesignObject {
        let mut o = rect(id, 0.0, 0.0, 1.0, 1.0);
        o.layer = layer;
        o.kind = ObjectKind::Vector(VectorObject::new(vec![BezPath::from(&Polyline {
            points: pts.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            closed: false,
        })]));
        o
    }

    const ROJO: Rgb = Rgb(255, 0, 0);
    const AZUL: Rgb = Rgb(0, 0, 255);

    fn ids(objects: &[DesignObject]) -> Vec<&str> {
        objects.iter().map(|o| o.id.as_str()).collect()
    }

    fn largo(objects: &[DesignObject]) -> f64 {
        objects
            .iter()
            .flat_map(|o| o.polylines())
            .map(|p| {
                let n = p.points.len();
                let count = if p.closed { n } else { n - 1 };
                (0..count)
                    .map(|k| p.points[k].dist(p.points[(k + 1) % n]))
                    .sum::<f64>()
            })
            .sum()
    }

    #[test]
    fn join_une_trazos_de_objetos_distintos_e_invierte() {
        let r = run(
            GeometryOp::Join,
            &[
                trazo("a", ROJO, &[(0.0, 0.0), (10.0, 0.0)]),
                // Va en sentido contrario y queda a 0,03 mm del extremo de a.
                trazo("b", ROJO, &[(10.0, 10.0), (10.03, 0.0)]),
            ],
            None,
            None,
        )
        .unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "join-1");
        assert_eq!(r[0].layer, ROJO);
        assert_eq!(r[0].transform, affine::IDENTITY);
        let paths = r[0].polylines();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].points.len(), 3);
        assert!(!paths[0].closed);
    }

    #[test]
    fn join_respeta_la_tolerancia_y_el_valor_por_defecto() {
        let dos = [
            trazo("a", ROJO, &[(0.0, 0.0), (10.0, 0.0)]),
            trazo("b", ROJO, &[(10.2, 0.0), (20.0, 0.0)]),
        ];
        // 0,2 mm de hueco: con 0,05 por defecto no se unen y todo sale intacto.
        let r = run(GeometryOp::Join, &dos, None, None).unwrap();
        assert_eq!(r, dos);
        let r = cleanup(GeometryOp::Join, &dos, Some(0.25)).unwrap();
        assert_eq!(ids(&r), vec!["join-1"]);
    }

    #[test]
    fn join_cierra_el_camino_que_se_cierra_sobre_si_mismo() {
        let r = run(
            GeometryOp::Join,
            &[
                trazo("l1", ROJO, &[(0.0, 0.0), (10.0, 0.0)]),
                trazo("l2", ROJO, &[(10.0, 0.0), (10.0, 10.0)]),
                trazo("l3", ROJO, &[(10.0, 10.0), (0.0, 10.0)]),
                trazo("l4", ROJO, &[(0.0, 10.0), (0.0, 0.0)]),
            ],
            None,
            None,
        )
        .unwrap();
        assert_eq!(r.len(), 1);
        let paths = r[0].polylines();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].closed);
        assert!((signed_area(&paths[0].points).abs() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn join_no_mezcla_capas_ni_toca_lo_que_no_cambia() {
        let circulo = {
            let mut c = rect("c", 0.0, 0.0, 1.0, 1.0);
            let k = 0.552_284_749_830_793_6 * 5.0;
            let p = |x, y| Point::new(x, y);
            c.kind = ObjectKind::Vector(VectorObject::new(vec![BezPath(vec![
                tlaku_doc::PathSeg::MoveTo { to: p(5.0, 0.0) },
                tlaku_doc::PathSeg::CubicTo {
                    ctrl1: p(5.0, k),
                    ctrl2: p(k, 5.0),
                    to: p(0.0, 5.0),
                },
                tlaku_doc::PathSeg::Close,
            ])]));
            c
        };
        let objs = [
            circulo.clone(),
            trazo("a", ROJO, &[(0.0, 0.0), (10.0, 0.0)]),
            trazo("b", AZUL, &[(10.0, 0.0), (20.0, 0.0)]),
            trazo("c2", ROJO, &[(10.0, 0.0), (20.0, 0.0)]),
        ];
        let r = run(GeometryOp::Join, &objs, None, None).unwrap();
        // El círculo (mismo id y mismas curvas) y el azul quedan igual; a y c2 se unen (en rojo)
        // y ocupan el lugar del primero.
        assert_eq!(r[0], circulo);
        assert_eq!(ids(&r), vec!["c", "join-1", "b"]);
        assert_eq!(r[2], objs[2]);
        assert_eq!(r[1].layer, ROJO);
        assert_eq!(r[1].polylines()[0].points.len(), 3);
    }

    #[test]
    fn join_conserva_grupo_solo_si_es_comun() {
        let mut a = trazo("a", ROJO, &[(0.0, 0.0), (10.0, 0.0)]);
        let mut b = trazo("b", ROJO, &[(10.0, 0.0), (20.0, 0.0)]);
        a.group = Some("g".into());
        b.group = Some("g".into());
        let r = run(GeometryOp::Join, &[a.clone(), b.clone()], None, None).unwrap();
        assert_eq!(r[0].group.as_deref(), Some("g"));
        b.group = Some("otro".into());
        let r = run(GeometryOp::Join, &[a, b], None, None).unwrap();
        assert_eq!(r[0].group, None);
    }

    #[test]
    fn close_cierra_dentro_de_la_tolerancia() {
        // U casi cerrada: hueco de 0,3 mm.
        let u = trazo(
            "u",
            ROJO,
            &[
                (0.3, 0.0),
                (0.0, 0.0),
                (0.0, 10.0),
                (10.0, 10.0),
                (10.0, 0.0),
                (0.6, 0.0),
            ],
        );
        let r = run(GeometryOp::Close, std::slice::from_ref(&u), None, None).unwrap();
        assert_eq!(ids(&r), vec!["close-1"]);
        assert!(r[0].polylines()[0].closed);
        // Con una tolerancia menor que el hueco no se toca.
        let r = cleanup(GeometryOp::Close, std::slice::from_ref(&u), Some(0.1)).unwrap();
        assert_eq!(r, vec![u]);
    }

    #[test]
    fn close_no_une_trayectorias_distintas_ni_cierra_lineas() {
        let objs = [
            trazo("a", ROJO, &[(0.0, 0.0), (10.0, 0.0)]),
            trazo("b", ROJO, &[(10.0, 0.0), (20.0, 0.0)]),
            trazo("tres", ROJO, &[(0.0, 5.0), (3.0, 5.0), (0.0, 5.2)]),
        ];
        let r = run(GeometryOp::Close, &objs, None, None).unwrap();
        // Solo la de tres puntos con extremos a 0,2 mm se cierra.
        assert_eq!(ids(&r), vec!["a", "b", "close-1"]);
    }

    #[test]
    fn dedupe_cuadrados_con_borde_comun() {
        let r = run(
            GeometryOp::Dedupe,
            &[
                rect("a", 0.0, 0.0, 10.0, 10.0),
                rect("b", 10.0, 0.0, 20.0, 10.0),
            ],
            None,
            None,
        )
        .unwrap();
        // El primero queda intacto; el segundo pierde el borde compartido.
        assert_eq!(ids(&r), vec!["a", "dedupe-1"]);
        assert!((largo(&r) - 70.0).abs() < 1e-9, "{}", largo(&r));
        assert_eq!(r[1].layer, ROJO);
        assert!(!r[1].polylines()[0].closed);
    }

    #[test]
    fn dedupe_parte_el_solape_parcial_y_quita_duplicados_exactos() {
        let r = run(
            GeometryOp::Dedupe,
            &[
                trazo("a", ROJO, &[(0.0, 0.0), (10.0, 0.0)]),
                trazo("b", ROJO, &[(5.0, 0.0), (15.0, 0.0)]),
                trazo("c", ROJO, &[(10.0, 0.0), (0.0, 0.0)]),
            ],
            None,
            None,
        )
        .unwrap();
        // c es copia de a: desaparece. b conserva solo [10, 15].
        assert_eq!(ids(&r), vec!["a", "dedupe-1"]);
        assert!((largo(&r) - 15.0).abs() < 1e-9);
        let bb = r[1].bounds().unwrap();
        assert!((bb.min.x - 10.0).abs() < 1e-9 && (bb.max.x - 15.0).abs() < 1e-9);
    }

    #[test]
    fn dedupe_no_cruza_capas_y_no_toca_lo_distinto() {
        let objs = [
            trazo("a", ROJO, &[(0.0, 0.0), (10.0, 0.0)]),
            trazo("b", AZUL, &[(0.0, 0.0), (10.0, 0.0)]),
            trazo("c", ROJO, &[(0.0, 1.0), (10.0, 1.0)]),
        ];
        let r = run(GeometryOp::Dedupe, &objs, None, None).unwrap();
        assert_eq!(r, objs);
    }

    #[test]
    fn dedupe_con_miles_de_objetos() {
        let mut objs = Vec::new();
        for i in 0..1500 {
            let x = (i % 50) as f64 * 3.0;
            let y = (i / 50) as f64 * 3.0;
            objs.push(rect(&format!("r{i}"), x, y, x + 3.0, y + 3.0));
        }
        let antes = largo(&objs);
        let r = run(GeometryOp::Dedupe, &objs, None, None).unwrap();
        // Cada borde interior compartido se corta una sola vez.
        assert!(largo(&r) < antes);
        let esperado = 3.0 * (51.0 * 30.0 + 31.0 * 50.0);
        assert!(
            (largo(&r) - esperado).abs() < 1e-6,
            "{} vs {esperado}",
            largo(&r)
        );
    }

    #[test]
    fn limpieza_rechaza_imagenes_textos_y_tolerancia_invalida() {
        let mut img = rect("foto", 0.0, 0.0, 1.0, 1.0);
        img.kind = ObjectKind::Image(tlaku_doc::ImageObject {
            source_path: "x.png".into(),
            width_px: 1,
            height_px: 1,
            size_mm: [1.0, 1.0],
            mask: Vec::new(),
        });
        for op in [GeometryOp::Join, GeometryOp::Close, GeometryOp::Dedupe] {
            let e = run(op, std::slice::from_ref(&img), None, None).unwrap_err();
            assert!(e.contains("«foto»"), "{e}");
            assert!(run(op, &[], None, None).is_err());
        }
        let e = cleanup(
            GeometryOp::Join,
            &[rect("a", 0.0, 0.0, 1.0, 1.0)],
            Some(-1.0),
        )
        .unwrap_err();
        assert!(e.contains("tolerancia"));
        assert!(
            cleanup(
                GeometryOp::Dedupe,
                &[rect("a", 0.0, 0.0, 1.0, 1.0)],
                Some(f64::NAN)
            )
            .is_err()
        );
    }

    #[test]
    fn los_ids_nuevos_no_chocan_con_los_existentes() {
        let r = run(
            GeometryOp::Join,
            &[
                trazo("join-1", ROJO, &[(0.0, 0.0), (10.0, 0.0)]),
                trazo("join-2", ROJO, &[(10.0, 0.0), (20.0, 0.0)]),
            ],
            None,
            None,
        )
        .unwrap();
        assert_eq!(ids(&r), vec!["join-3"]);
    }
}
