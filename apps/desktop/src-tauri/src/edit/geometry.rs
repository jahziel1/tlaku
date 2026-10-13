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

use serde::Deserialize;
use tlaku_doc::{DesignObject, ObjectKind, VectorObject, affine};
use tlaku_geom::{BoolOp, Join, Polyline, boolean, offset, signed_area, weld};

/// Error máximo (mm) al aplanar las curvas y en los arcos del offset.
const TOLERANCE_MM: f64 = 0.01;
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
            .map(|id| format!("«{id}»"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    if !images.is_empty() {
        return Err(format!(
            "Las imágenes no pueden participar en esta operación: {}.",
            list(&images)
        ));
    }
    if !open.is_empty() {
        return Err(format!(
            "Hay formas abiertas que no pueden participar: {}. Ciérralas antes (por ejemplo con «Cerrar polilínea»).",
            list(&open)
        ));
    }
    if !empty.is_empty() {
        return Err(format!(
            "Estos objetos no tienen contorno que operar (¿un texto sin contorno generado?): {}.",
            list(&empty)
        ));
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
        kind: ObjectKind::Vector(VectorObject {
            paths: piece.iter().map(tlaku_doc::BezPath::from).collect(),
        }),
        locked: false,
        hidden: false,
        group: None,
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
    let Some(first) = objects.first() else {
        return Err("No hay objetos sobre los que operar.".into());
    };
    let layer = first.layer;
    let ops = operands(objects)?;
    let mut counter = 0usize;
    match op {
        GeometryOp::Union | GeometryOp::Difference | GeometryOp::Intersection | GeometryOp::Xor => {
            if ops.len() < 2 {
                return Err("Las booleanas necesitan al menos dos objetos.".into());
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
            let d = distance.ok_or("Falta la distancia del offset.")?;
            if !d.is_finite() {
                return Err("La distancia del offset no es un número válido.".into());
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
    }
}

/// Booleanas, soldar y offset. `objects` en coordenadas de documento (cada uno con su
/// `transform`); el resultado también (transform identidad). Corre fuera del hilo principal.
#[tauri::command]
pub async fn geometry_op(
    op: GeometryOp,
    objects: Vec<DesignObject>,
    distance: Option<f64>,
    join: Option<JoinStyle>,
) -> Result<Vec<DesignObject>, String> {
    tauri::async_runtime::spawn_blocking(move || run(op, &objects, distance, join))
        .await
        .map_err(|e| format!("La tarea se interrumpió: {e}"))?
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
            kind: ObjectKind::Vector(VectorObject {
                paths: vec![BezPath::from(&poly)],
            }),
            locked: false,
            hidden: false,
            group: None,
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
            o.kind = ObjectKind::Vector(VectorObject {
                paths: vec![BezPath::from(&Polyline {
                    points: vec![Point::new(0.0, 0.0), Point::new(10.0, 10.0)],
                    closed: false,
                })],
            });
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
            o.kind = ObjectKind::Vector(VectorObject {
                paths: vec![BezPath(segs)],
            });
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
}
