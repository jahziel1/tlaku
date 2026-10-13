//! Diseño avanzado (ola 3): deformar, contorno envolvente (banda elástica) y círculo tangente.
//!
//! Los objetos llegan en coordenadas de documento (con su `transform`) y los resultados vuelven
//! igual, con transform identidad:
//! - `warp_objects`: deforma cada objeto con una envolvente de cuatro bordes Bézier (ver
//!   `tlaku_geom::Envelope`). Cada objeto sustituye al original (mismo id, capa y grupo) como
//!   trayectoria vectorial; un texto pasa a ser su contorno. Las imágenes no se pueden deformar.
//! - `hull_outline`: una forma cerrada nueva con el casco convexo de todos los puntos de los
//!   objetos, o un casco cóncavo si se da `max_edge` (mm).
//! - `tangent_circles`: círculos tangentes a 2 o 3 operandos (ver `tlaku_geom::tangent_circles`).

use serde::{Deserialize, Serialize};
use tlaku_core::{Bounds, Point, Polyline, Rgb};
use tlaku_doc::{BezPath, DesignObject, ObjectKind, VectorObject, affine};
use tlaku_geom::{Envelope, Warp, concave_hull, convex_hull, warp_path};

/// Error máximo (mm) al aplanar los objetos para el contorno envolvente.
const FLATTEN_MM: f64 = 0.01;
/// Error máximo (mm) al aplanar las formas para el círculo tangente.
const TANGENT_FLATTEN_MM: f64 = 0.005;
/// Error máximo (mm) de las curvas deformadas respecto a la deformación exacta.
const DEFAULT_WARP_TOLERANCE_MM: f64 = 0.01;

/// Borde Bézier cúbico del contrato: cuatro puntos `[x, y]`.
type EdgeArg = [[f64; 2]; 4];

/// Envolvente del contrato (ver `tlaku_geom::Envelope`).
#[derive(Debug, Clone, Deserialize)]
pub struct EnvelopeArg {
    pub bottom: EdgeArg,
    pub top: EdgeArg,
    pub left: EdgeArg,
    pub right: EdgeArg,
}

fn edge(e: EdgeArg) -> [Point; 4] {
    e.map(|[x, y]| Point::new(x, y))
}

/// Círculo tangente del contrato.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct TangentCircle {
    pub center: [f64; 2],
    pub radius: f64,
}

fn vector_object(source: &DesignObject, paths: Vec<BezPath>) -> DesignObject {
    DesignObject {
        transform: affine::IDENTITY,
        kind: ObjectKind::Vector(VectorObject { paths }),
        // La forma paramétrica, el punto de inicio y los puentes dejan de valer tras deformar.
        shape: None,
        start: None,
        manual_tabs: Vec::new(),
        ..source.clone()
    }
}

/// Núcleo de `warp_objects` (sin Tauri).
pub fn warp(
    objects: &[DesignObject],
    bounds: [[f64; 2]; 2],
    envelope: &EnvelopeArg,
    tolerance: Option<f64>,
) -> Result<Vec<DesignObject>, String> {
    if objects.is_empty() {
        return Err("No hay objetos que deformar.".into());
    }
    let images: Vec<String> = objects
        .iter()
        .filter(|o| matches!(o.kind, ObjectKind::Image(_)))
        .map(|o| format!("«{}»", o.id))
        .collect();
    if !images.is_empty() {
        return Err(format!(
            "Las imágenes no se pueden deformar: {}.",
            images.join(", ")
        ));
    }
    let tol = tolerance.unwrap_or(DEFAULT_WARP_TOLERANCE_MM);
    if !(tol.is_finite() && tol > 0.0) {
        return Err("La tolerancia de la deformación no es válida.".into());
    }
    let source = Bounds {
        min: Point::new(bounds[0][0], bounds[0][1]),
        max: Point::new(bounds[1][0], bounds[1][1]),
    };
    let env = Envelope {
        bottom: edge(envelope.bottom),
        top: edge(envelope.top),
        left: edge(envelope.left),
        right: edge(envelope.right),
    };
    let warp = Warp::new(source, env)
        .ok_or("La selección no tiene ancho y alto: no se puede deformar.")?;
    let mut out = Vec::with_capacity(objects.len());
    for o in objects {
        let curves = o.curves();
        if curves.is_empty() {
            return Err(format!(
                "El objeto «{}» no tiene contorno que deformar (¿un texto sin contorno generado?).",
                o.id
            ));
        }
        out.push(vector_object(
            o,
            curves.iter().map(|c| warp_path(c, &warp, tol)).collect(),
        ));
    }
    Ok(out)
}

/// Núcleo de `hull_outline` (sin Tauri). El resultado va en `layer`, o en la capa del primer objeto.
pub fn hull(
    objects: &[DesignObject],
    max_edge: Option<f64>,
    layer: Option<Rgb>,
) -> Result<Vec<DesignObject>, String> {
    let Some(first) = objects.first() else {
        return Err("No hay objetos de los que sacar el contorno.".into());
    };
    let points: Vec<Point> = objects
        .iter()
        .flat_map(|o| o.polylines_with(FLATTEN_MM))
        .flat_map(|p| p.points)
        .collect();
    let outline = match max_edge {
        Some(edge) if edge.is_finite() && edge > 0.0 => concave_hull(&points, edge),
        Some(_) => return Err("La distancia máxima de arista debe ser un número positivo.".into()),
        None => convex_hull(&points),
    };
    if outline.len() < 3 {
        return Err("La selección no tiene área: no se puede envolver.".into());
    }
    let path = BezPath::from(&Polyline {
        points: outline,
        closed: true,
    });
    Ok(vec![DesignObject {
        id: "hull-1".into(),
        layer: layer.unwrap_or(first.layer),
        transform: affine::IDENTITY,
        kind: ObjectKind::Vector(VectorObject { paths: vec![path] }),
        locked: false,
        hidden: false,
        group: None,
        priority: None,
        power_scale: None,
        shape: None,
        start: None,
        manual_tabs: Vec::new(),
        array: None,
    }])
}

/// Núcleo de `tangent_circles` (sin Tauri): cada objeto es un operando.
pub fn tangent(
    objects: &[DesignObject],
    radius: Option<f64>,
) -> Result<Vec<TangentCircle>, String> {
    if objects
        .iter()
        .any(|o| matches!(o.kind, ObjectKind::Image(_)))
    {
        return Err("Las imágenes no pueden participar en el círculo tangente.".into());
    }
    let shapes: Vec<Vec<Polyline>> = objects
        .iter()
        .map(|o| o.polylines_with(TANGENT_FLATTEN_MM))
        .collect();
    Ok(tlaku_geom::tangent_circles(&shapes, radius)?
        .into_iter()
        .map(|c| TangentCircle {
            center: [c.center.x, c.center.y],
            radius: c.radius,
        })
        .collect())
}

/// Deforma los objetos con una envolvente. Corre fuera del hilo principal.
#[tauri::command]
pub async fn warp_objects(
    objects: Vec<DesignObject>,
    bounds: [[f64; 2]; 2],
    envelope: EnvelopeArg,
    tolerance: Option<f64>,
) -> Result<Vec<DesignObject>, String> {
    tauri::async_runtime::spawn_blocking(move || warp(&objects, bounds, &envelope, tolerance))
        .await
        .map_err(|e| format!("La tarea se interrumpió: {e}"))?
}

/// Contorno envolvente de los objetos: convexo, o cóncavo con `max_edge` (mm).
#[tauri::command(rename_all = "snake_case")]
pub async fn hull_outline(
    objects: Vec<DesignObject>,
    max_edge: Option<f64>,
    layer: Option<Rgb>,
) -> Result<Vec<DesignObject>, String> {
    tauri::async_runtime::spawn_blocking(move || hull(&objects, max_edge, layer))
        .await
        .map_err(|e| format!("La tarea se interrumpió: {e}"))?
}

/// Círculos tangentes a 2 o 3 objetos (con 2 hace falta `radius`).
#[tauri::command]
pub async fn tangent_circles(
    objects: Vec<DesignObject>,
    radius: Option<f64>,
) -> Result<Vec<TangentCircle>, String> {
    tauri::async_runtime::spawn_blocking(move || tangent(&objects, radius))
        .await
        .map_err(|e| format!("La tarea se interrumpió: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn object(id: &str, points: &[(f64, f64)], closed: bool) -> DesignObject {
        DesignObject {
            id: id.into(),
            layer: Rgb(255, 0, 0),
            transform: affine::IDENTITY,
            kind: ObjectKind::Vector(VectorObject {
                paths: vec![BezPath::from(&Polyline {
                    points: points.iter().map(|&(x, y)| pt(x, y)).collect(),
                    closed,
                })],
            }),
            locked: false,
            hidden: false,
            group: Some("g1".into()),
            priority: None,
            power_scale: None,
            shape: None,
            start: None,
            manual_tabs: vec![[1.0, 2.0]],
            array: None,
        }
    }

    fn rect(id: &str, x0: f64, y0: f64, x1: f64, y1: f64) -> DesignObject {
        object(id, &[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], true)
    }

    fn identity(b: [[f64; 2]; 2]) -> EnvelopeArg {
        let [[x0, y0], [x1, y1]] = b;
        let line = |a: [f64; 2], z: [f64; 2]| -> EdgeArg {
            let at = |t: f64| [a[0] + (z[0] - a[0]) * t, a[1] + (z[1] - a[1]) * t];
            [a, at(1.0 / 3.0), at(2.0 / 3.0), z]
        };
        EnvelopeArg {
            bottom: line([x0, y0], [x1, y0]),
            top: line([x0, y1], [x1, y1]),
            left: line([x0, y0], [x0, y1]),
            right: line([x1, y0], [x1, y1]),
        }
    }

    #[test]
    fn warp_identity_keeps_bounds_and_metadata() {
        let b = [[0.0, 0.0], [20.0, 10.0]];
        let out = warp(&[rect("a", 0.0, 0.0, 20.0, 10.0)], b, &identity(b), None).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "a");
        assert_eq!(out[0].group.as_deref(), Some("g1"));
        assert!(out[0].manual_tabs.is_empty());
        let bb = out[0].bounds().unwrap();
        assert!(bb.min.dist(pt(0.0, 0.0)) < 1e-6 && bb.max.dist(pt(20.0, 10.0)) < 1e-6);
    }

    #[test]
    fn warp_perspective_narrows_the_top() {
        let b = [[0.0, 0.0], [20.0, 10.0]];
        let mut env = identity(b);
        // La parte de arriba se estrecha 5 mm por lado.
        env.top = [[5.0, 10.0], [8.0, 10.0], [12.0, 10.0], [15.0, 10.0]];
        let out = warp(&[rect("a", 0.0, 0.0, 20.0, 10.0)], b, &env, None).unwrap();
        let polys = out[0].polylines();
        let top: Vec<_> = polys[0].points.iter().filter(|p| p.y > 9.99).collect();
        assert!(top.iter().all(|p| p.x >= 5.0 - 1e-6 && p.x <= 15.0 + 1e-6));
        assert!(top.iter().any(|p| (p.x - 5.0).abs() < 1e-6));
    }

    #[test]
    fn warp_errors_are_in_spanish() {
        let b = [[0.0, 0.0], [20.0, 10.0]];
        assert!(
            warp(&[], b, &identity(b), None)
                .unwrap_err()
                .contains("No hay objetos")
        );
        let flat = [[0.0, 0.0], [0.0, 10.0]];
        let e = warp(
            &[rect("a", 0.0, 0.0, 1.0, 1.0)],
            flat,
            &identity(flat),
            None,
        )
        .unwrap_err();
        assert!(e.contains("ancho y alto"), "{e}");
        assert!(
            warp(
                &[rect("a", 0.0, 0.0, 1.0, 1.0)],
                b,
                &identity(b),
                Some(-1.0)
            )
            .is_err()
        );
    }

    #[test]
    fn hull_of_two_squares_is_one_closed_shape() {
        let objs = [
            rect("a", 0.0, 0.0, 10.0, 10.0),
            rect("b", 20.0, 0.0, 30.0, 10.0),
        ];
        let out = hull(&objs, None, None).unwrap();
        assert_eq!(out.len(), 1);
        let bb = out[0].bounds().unwrap();
        assert!(bb.min.dist(pt(0.0, 0.0)) < 1e-9 && bb.max.dist(pt(30.0, 10.0)) < 1e-9);
        assert!(out[0].polylines()[0].closed);
        assert_eq!(out[0].layer, Rgb(255, 0, 0));
        let other = hull(&objs, None, Some(Rgb(0, 0, 255))).unwrap();
        assert_eq!(other[0].layer, Rgb(0, 0, 255));
    }

    #[test]
    fn concave_hull_is_smaller_than_the_convex_one() {
        // Una "L" gruesa hecha de dos rectángulos: el casco cóncavo recorta la esquina vacía.
        let objs = [
            rect("a", 0.0, 0.0, 30.0, 5.0),
            rect("b", 0.0, 5.0, 5.0, 30.0),
        ];
        let area = |o: &DesignObject| tlaku_geom::signed_area(&o.polylines()[0].points);
        let convex = hull(&objs, None, None).unwrap();
        let concave = hull(&objs, Some(12.0), None).unwrap();
        assert!(area(&concave[0]) < area(&convex[0]) - 1.0);
    }

    #[test]
    fn hull_rejects_flat_input_and_bad_edge() {
        assert!(hull(&[], None, None).is_err());
        let line = object("l", &[(0.0, 0.0), (10.0, 0.0)], false);
        assert!(hull(&[line], None, None).unwrap_err().contains("área"));
        let r = rect("a", 0.0, 0.0, 1.0, 1.0);
        assert!(hull(&[r], Some(-3.0), None).is_err());
    }

    #[test]
    fn tangent_between_two_lines() {
        let a = object("a", &[(0.0, 0.0), (50.0, 0.0)], false);
        let b = object("b", &[(0.0, 0.0), (0.0, 50.0)], false);
        let found = tangent(&[a, b], Some(5.0)).unwrap();
        assert!(
            found
                .iter()
                .any(|c| (c.center[0] - 5.0).abs() < 1e-6 && (c.center[1] - 5.0).abs() < 1e-6)
        );
        assert!(tangent(&[rect("r", 0.0, 0.0, 1.0, 1.0)], Some(1.0)).is_err());
    }
}
