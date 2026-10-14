//! Pruebas de deformar, contorno envolvente y círculo tangente.

use crate::{
    Envelope, Grid, Point, Polyline, Warp, concave_hull, convex_hull, signed_area, tangent_circles,
    warp_path,
};
use tlaku_core::Bounds;
use tlaku_core::bezpath::{BezPath, PathSeg};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn circle_poly(cx: f64, cy: f64, r: f64, n: usize) -> Polyline {
    Polyline {
        points: (0..n)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / n as f64;
                pt(cx + r * a.cos(), cy + r * a.sin())
            })
            .collect(),
        closed: true,
    }
}

fn line(a: Point, b: Point) -> Polyline {
    Polyline {
        points: vec![a, b],
        closed: false,
    }
}

// ---- deformar --------------------------------------------------------------------------------

fn rect_path(x0: f64, y0: f64, x1: f64, y1: f64) -> BezPath {
    BezPath::from(&Polyline {
        points: vec![pt(x0, y0), pt(x1, y0), pt(x1, y1), pt(x0, y1)],
        closed: true,
    })
}

fn bounds(x0: f64, y0: f64, x1: f64, y1: f64) -> Bounds {
    Bounds {
        min: pt(x0, y0),
        max: pt(x1, y1),
    }
}

/// Puntos de destino de todos los segmentos de una trayectoria.
fn ends(p: &BezPath) -> Vec<Point> {
    p.0.iter()
        .filter_map(|s| match *s {
            PathSeg::MoveTo { to }
            | PathSeg::LineTo { to }
            | PathSeg::QuadTo { to, .. }
            | PathSeg::CubicTo { to, .. } => Some(to),
            PathSeg::Close => None,
        })
        .collect()
}

#[test]
fn identity_envelope_keeps_the_geometry() {
    let b = bounds(0.0, 0.0, 20.0, 10.0);
    let w = Warp::new(b, Envelope::identity(b)).unwrap();
    let out = warp_path(&rect_path(0.0, 0.0, 20.0, 10.0), &w, 0.01);
    for p in out.flatten(0.005).iter().flat_map(|p| &p.points) {
        let on_border = p.x.abs() < 1e-6
            || (p.x - 20.0).abs() < 1e-6
            || p.y.abs() < 1e-6
            || (p.y - 10.0).abs() < 1e-6;
        assert!(on_border, "{p:?} se salió del borde");
        assert!(p.x > -1e-6 && p.x < 20.0 + 1e-6 && p.y > -1e-6 && p.y < 10.0 + 1e-6);
    }
    // Las esquinas se conservan.
    let corners = ends(&out);
    for c in [pt(0.0, 0.0), pt(20.0, 0.0), pt(20.0, 10.0), pt(0.0, 10.0)] {
        assert!(
            corners.iter().any(|p| p.dist(c) < 1e-9),
            "falta la esquina {c:?}"
        );
    }
}

#[test]
fn moving_a_corner_moves_that_corner_only() {
    let b = bounds(0.0, 0.0, 20.0, 10.0);
    let mut env = Envelope::identity(b);
    // La esquina superior derecha sube 5 mm.
    env.top[3].y += 5.0;
    env.top[2].y += 5.0;
    env.right[3].y += 5.0;
    env.right[2].y += 5.0;
    let w = Warp::new(b, env).unwrap();
    assert!(w.map(pt(20.0, 10.0)).dist(pt(20.0, 15.0)) < 1e-9);
    assert!(w.map(pt(0.0, 0.0)).dist(pt(0.0, 0.0)) < 1e-9);
    assert!(w.map(pt(20.0, 0.0)).dist(pt(20.0, 0.0)) < 1e-9);
    assert!(w.map(pt(0.0, 10.0)).dist(pt(0.0, 10.0)) < 1e-9);
}

#[test]
fn warped_curves_follow_the_true_deformation() {
    // Un arco hacia arriba: ambos bordes horizontales suben 4 mm en el centro.
    let b = bounds(0.0, 0.0, 30.0, 10.0);
    let mut env = Envelope::identity(b);
    for e in [&mut env.bottom, &mut env.top] {
        e[1].y += 16.0 / 3.0;
        e[2].y += 16.0 / 3.0;
    }
    let w = Warp::new(b, env).unwrap();
    let tol = 0.01;
    let out = warp_path(&rect_path(0.0, 0.0, 30.0, 10.0), &w, tol);
    let flat = out.flatten(0.002);
    // Todo punto de la salida cae sobre la imagen del borde del rectángulo original.
    let mut checked = 0;
    for p in flat.iter().flat_map(|p| &p.points) {
        let near = (0..=400).any(|k| {
            let t = k as f64 / 400.0;
            [
                w.map(pt(30.0 * t, 0.0)),
                w.map(pt(30.0 * t, 10.0)),
                w.map(pt(0.0, 10.0 * t)),
                w.map(pt(30.0, 10.0 * t)),
            ]
            .iter()
            .any(|q| q.dist(*p) < 0.1)
        });
        assert!(near, "{p:?} no está sobre el borde deformado");
        checked += 1;
    }
    assert!(checked > 10);
    // El centro del borde inferior sube 4 mm (la altura de la flecha).
    let mid = w.map(pt(15.0, 0.0));
    assert!((mid.y - 4.0).abs() < 1e-9, "{mid:?}");
    assert!(
        flat.iter()
            .flat_map(|p| &p.points)
            .any(|p| (p.x - 15.0).abs() < 0.2 && (p.y - 4.0).abs() < 0.05)
    );
}

#[test]
fn warp_rejects_an_empty_source() {
    let b = bounds(0.0, 0.0, 0.0, 10.0);
    assert!(Warp::new(b, Envelope::identity(b)).is_none());
}

#[test]
fn warp_keeps_subpaths_and_open_ends() {
    let b = bounds(0.0, 0.0, 10.0, 10.0);
    let w = Warp::new(b, Envelope::identity(b)).unwrap();
    let path = BezPath::from(&line(pt(1.0, 1.0), pt(9.0, 9.0)));
    let out = warp_path(&path, &w, 0.01);
    assert!(matches!(out.0[0], PathSeg::MoveTo { .. }));
    assert!(!out.0.contains(&PathSeg::Close));
    let flat = out.flatten(0.005);
    assert_eq!(flat.len(), 1);
    assert!(!flat[0].closed);
    assert!(flat[0].points.last().unwrap().dist(pt(9.0, 9.0)) < 1e-9);
}

// ---- malla 4x4 y perspectiva -----------------------------------------------------------------

/// Malla uniforme sobre la región `b` (la identidad).
fn uniform_grid(b: Bounds) -> Grid {
    let (w, h) = (b.max.x - b.min.x, b.max.y - b.min.y);
    std::array::from_fn(|j| {
        std::array::from_fn(|i| pt(b.min.x + w * i as f64 / 3.0, b.min.y + h * j as f64 / 3.0))
    })
}

#[test]
fn uniform_mesh_is_the_identity() {
    let b = bounds(2.0, -3.0, 32.0, 17.0);
    let w = Warp::mesh(b, uniform_grid(b)).unwrap();
    for k in 0..=10 {
        for m in 0..=10 {
            let p = pt(2.0 + 30.0 * k as f64 / 10.0, -3.0 + 20.0 * m as f64 / 10.0);
            assert!(w.map(p).dist(p) < 1e-9, "{p:?} -> {:?}", w.map(p));
        }
    }
}

#[test]
fn mesh_surface_passes_through_every_point() {
    let b = bounds(0.0, 0.0, 30.0, 30.0);
    let mut g = uniform_grid(b);
    // Se mueven varios puntos, interiores y de borde.
    g[1][1] = pt(8.0, 14.0);
    g[2][1] = pt(12.0, 22.0);
    g[0][3] = pt(33.0, -4.0);
    g[3][0] = pt(-2.0, 31.0);
    let w = Warp::mesh(b, g).unwrap();
    for (j, row) in g.iter().enumerate() {
        for (i, target) in row.iter().enumerate() {
            let src = pt(10.0 * i as f64, 10.0 * j as f64);
            assert!(w.map(src).dist(*target) < 1e-9, "nodo ({i}, {j})");
        }
    }
}

#[test]
fn mesh_is_deterministic_and_rejects_bad_input() {
    let b = bounds(0.0, 0.0, 30.0, 30.0);
    let mut g = uniform_grid(b);
    g[1][2] = pt(21.0, 9.0);
    let (a, c) = (Warp::mesh(b, g).unwrap(), Warp::mesh(b, g).unwrap());
    let path = rect_path(2.0, 2.0, 28.0, 28.0);
    assert_eq!(warp_path(&path, &a, 0.01), warp_path(&path, &c, 0.01));
    g[0][0] = pt(f64::NAN, 0.0);
    assert!(Warp::mesh(b, g).is_none());
    let flat = bounds(0.0, 0.0, 0.0, 5.0);
    assert!(Warp::mesh(flat, uniform_grid(bounds(0.0, 0.0, 1.0, 1.0))).is_none());
}

#[test]
fn mesh_moves_a_curve_with_the_surface() {
    // Una línea horizontal por el centro sigue el punto del centro de la malla.
    let b = bounds(0.0, 0.0, 30.0, 30.0);
    let mut g = uniform_grid(b);
    g[1][1].y += 6.0;
    g[1][2].y += 6.0;
    let w = Warp::mesh(b, g).unwrap();
    let path = BezPath::from(&line(pt(0.0, 10.0), pt(30.0, 10.0)));
    let out = warp_path(&path, &w, 0.01);
    let flat = out.flatten(0.002);
    let ys: Vec<f64> = flat
        .iter()
        .flat_map(|p| p.points.iter().map(|q| q.y))
        .collect();
    assert!(ys.iter().cloned().fold(f64::MIN, f64::max) > 15.5);
    assert!(flat[0].points.first().unwrap().dist(pt(0.0, 10.0)) < 1e-9);
    assert!(flat[0].points.last().unwrap().dist(pt(30.0, 10.0)) < 1e-9);
    // Cada punto del resultado está sobre la superficie: imagen de algún punto de la recta.
    for q in flat.iter().flat_map(|p| &p.points) {
        assert!((0..=600).any(|k| w.map(pt(30.0 * k as f64 / 600.0, 10.0)).dist(*q) < 0.1));
    }
}

#[test]
fn perspective_maps_the_corners_and_keeps_lines_straight() {
    let b = bounds(0.0, 0.0, 20.0, 10.0);
    let q = [pt(0.0, 0.0), pt(20.0, 0.0), pt(15.0, 10.0), pt(5.0, 10.0)];
    let w = Warp::perspective(b, q).unwrap();
    for (src, dst) in [
        (pt(0.0, 0.0), q[0]),
        (pt(20.0, 0.0), q[1]),
        (pt(20.0, 10.0), q[2]),
        (pt(0.0, 10.0), q[3]),
    ] {
        assert!(w.map(src).dist(dst) < 1e-9, "{src:?}");
    }
    // Tres puntos alineados de la región siguen alineados tras la proyección.
    let (a, m, c) = (
        w.map(pt(2.0, 1.0)),
        w.map(pt(9.0, 4.0)),
        w.map(pt(16.0, 7.0)),
    );
    let cross = (m.x - a.x) * (c.y - a.y) - (m.y - a.y) * (c.x - a.x);
    assert!(cross.abs() < 1e-9, "{cross}");
    // No es bilineal: el centro del borde inferior-superior no cae en el promedio de las esquinas.
    let center = w.map(pt(10.0, 5.0));
    let bilinear = pt(10.0, 5.0);
    assert!(center.x.is_finite() && center.y.is_finite());
    assert!((center.y - bilinear.y).abs() > 0.1);
}

#[test]
fn perspective_of_a_parallelogram_is_affine() {
    let b = bounds(0.0, 0.0, 10.0, 10.0);
    let q = [pt(0.0, 0.0), pt(10.0, 0.0), pt(14.0, 10.0), pt(4.0, 10.0)];
    let w = Warp::perspective(b, q).unwrap();
    assert!(w.map(pt(5.0, 5.0)).dist(pt(7.0, 5.0)) < 1e-9);
}

#[test]
fn perspective_rejects_degenerate_or_concave_quads() {
    let b = bounds(0.0, 0.0, 10.0, 10.0);
    // Dos esquinas juntas.
    assert!(
        Warp::perspective(
            b,
            [pt(0.0, 0.0), pt(0.0, 0.0), pt(10.0, 10.0), pt(0.0, 10.0)]
        )
        .is_none()
    );
    // Cuadrilátero cóncavo (punta hacia adentro).
    let concave = [pt(0.0, 0.0), pt(10.0, 0.0), pt(2.0, 2.0), pt(0.0, 10.0)];
    assert!(Warp::perspective(b, concave).is_none());
    // Puntos colineales.
    let line_q = [pt(0.0, 0.0), pt(5.0, 0.0), pt(10.0, 0.0), pt(15.0, 0.0)];
    assert!(Warp::perspective(b, line_q).is_none());
    let nan = [pt(f64::NAN, 0.0), pt(5.0, 0.0), pt(10.0, 5.0), pt(0.0, 5.0)];
    assert!(Warp::perspective(b, nan).is_none());
}

#[test]
fn perspective_warp_path_follows_the_projection() {
    let b = bounds(0.0, 0.0, 20.0, 10.0);
    let q = [pt(0.0, 0.0), pt(20.0, 0.0), pt(15.0, 10.0), pt(5.0, 10.0)];
    let w = Warp::perspective(b, q).unwrap();
    let out = warp_path(&rect_path(0.0, 0.0, 20.0, 10.0), &w, 0.01);
    let corners = ends(&out);
    for c in q {
        assert!(corners.iter().any(|p| p.dist(c) < 1e-9), "falta {c:?}");
    }
    // Los lados siguen rectos: los puntos del lado derecho están sobre la recta q[1]-q[2].
    for p in out.flatten(0.002).iter().flat_map(|p| &p.points) {
        let on_edge = |a: Point, z: Point| {
            let cr = (z.x - a.x) * (p.y - a.y) - (z.y - a.y) * (p.x - a.x);
            cr.abs() / a.dist(z) < 0.02 && p.dist(a) + p.dist(z) < a.dist(z) + 0.02
        };
        assert!(
            on_edge(q[0], q[1])
                || on_edge(q[1], q[2])
                || on_edge(q[2], q[3])
                || on_edge(q[3], q[0]),
            "{p:?}"
        );
    }
}

// ---- contorno envolvente ---------------------------------------------------------------------

#[test]
fn convex_hull_of_a_square_with_interior_points() {
    let mut pts = vec![pt(0.0, 0.0), pt(10.0, 0.0), pt(10.0, 10.0), pt(0.0, 10.0)];
    pts.extend([pt(5.0, 5.0), pt(2.0, 3.0), pt(5.0, 0.0)]);
    let h = convex_hull(&pts);
    assert_eq!(h.len(), 4);
    assert!((signed_area(&h) - 100.0).abs() < 1e-9);
}

#[test]
fn convex_hull_degenerate_inputs() {
    assert!(convex_hull(&[]).is_empty());
    assert_eq!(convex_hull(&[pt(1.0, 1.0), pt(1.0, 1.0)]).len(), 1);
    assert_eq!(
        convex_hull(&[pt(0.0, 0.0), pt(1.0, 1.0), pt(2.0, 2.0)]).len(),
        2
    );
    assert!(convex_hull(&[pt(f64::NAN, 0.0)]).is_empty());
}

/// Puntos de una "C": una corona circular de radio 10 con una abertura a la derecha.
fn c_shape() -> Vec<Point> {
    let mut pts = Vec::new();
    for r in [8.0, 10.0, 12.0] {
        for k in 0..60 {
            let a = (40.0 + k as f64 * 280.0 / 59.0).to_radians();
            pts.push(pt(r * a.cos(), r * a.sin()));
        }
    }
    pts
}

#[test]
fn concave_hull_digs_into_the_opening() {
    let pts = c_shape();
    let convex = convex_hull(&pts);
    let concave = concave_hull(&pts, 6.0);
    assert!(signed_area(&concave) > 0.0, "debe quedar antihorario");
    assert!(
        signed_area(&concave) < signed_area(&convex) - 20.0,
        "el casco cóncavo debe ser menor: {} frente a {}",
        signed_area(&concave),
        signed_area(&convex)
    );
    // Todos los vértices vienen de los puntos de entrada.
    assert!(concave.iter().all(|c| pts.contains(c)));
    // La abertura de la derecha queda fuera del contorno.
    assert!(!crate::point_in_polygon(pt(11.0, 0.0), &concave));
    assert!(crate::point_in_polygon(pt(-10.0, 0.0), &concave));
}

#[test]
fn concave_hull_with_a_huge_edge_is_the_convex_hull() {
    let pts = c_shape();
    assert_eq!(concave_hull(&pts, 1e6), convex_hull(&pts));
    assert_eq!(concave_hull(&pts, 0.0), convex_hull(&pts));
    assert_eq!(concave_hull(&pts, f64::NAN), convex_hull(&pts));
}

#[test]
fn concave_hull_of_a_convex_set_does_not_dig() {
    // Un círculo: todos los puntos ya están en el casco; no hay nada que excavar.
    let pts = circle_poly(0.0, 0.0, 10.0, 40).points;
    assert_eq!(concave_hull(&pts, 1.0).len(), 40);
}

// ---- círculo tangente ------------------------------------------------------------------------

#[test]
fn tangent_to_two_circles() {
    let a = vec![circle_poly(0.0, 0.0, 10.0, 180)];
    let b = vec![circle_poly(30.0, 0.0, 10.0, 180)];
    let found = tangent_circles(&[a, b], Some(10.0)).unwrap();
    let want_y = (400.0f64 - 225.0).sqrt();
    // Dos soluciones simétricas por fuera, a distancia 20 de ambos centros.
    for sign in [1.0, -1.0] {
        assert!(
            found
                .iter()
                .any(|c| (c.radius - 10.0).abs() < 1e-9
                    && c.center.dist(pt(15.0, sign * want_y)) < 0.15),
            "falta la solución con y = {}: {found:?}",
            sign * want_y
        );
    }
}

#[test]
fn tangent_to_two_lines_is_the_corner_circle() {
    // Rectas perpendiculares: el círculo de radio 5 en el rincón entre ellas.
    let a = vec![line(pt(0.0, 0.0), pt(50.0, 0.0))];
    let b = vec![line(pt(0.0, 0.0), pt(0.0, 50.0))];
    let found = tangent_circles(&[a, b], Some(5.0)).unwrap();
    assert!(
        found.iter().any(|c| c.center.dist(pt(5.0, 5.0)) < 1e-6),
        "{found:?}"
    );
    // Tres rincones: el cuarto (-5, -5) queda a 7,07 mm del extremo, no a 5.
    assert_eq!(found.len(), 3, "{found:?}");
}

#[test]
fn tangent_inside_a_rectangle_corner() {
    // El círculo de radio 3 que toca dos lados contiguos de un rectángulo desde dentro.
    let rect = Polyline {
        points: vec![pt(0.0, 0.0), pt(40.0, 0.0), pt(40.0, 20.0), pt(0.0, 20.0)],
        closed: true,
    };
    let side = vec![line(pt(0.0, 0.0), pt(0.0, 20.0))];
    let found = tangent_circles(&[vec![rect], side], Some(3.0)).unwrap();
    assert!(found.iter().any(|c| c.center.dist(pt(3.0, 3.0)) < 1e-6));
}

#[test]
fn tangent_to_three_circles() {
    // Tres círculos iguales en un triángulo equilátero de lado 40: el círculo del hueco central.
    let h = 40.0 * 3.0f64.sqrt() / 2.0;
    let shapes: Vec<Vec<Polyline>> = [(0.0, 0.0), (40.0, 0.0), (20.0, h)]
        .iter()
        .map(|&(x, y)| vec![circle_poly(x, y, 10.0, 120)])
        .collect();
    let found = tangent_circles(&shapes, None).unwrap();
    let centroid = pt(20.0, h / 3.0);
    let want_r = 40.0 / 3.0f64.sqrt() - 10.0;
    let hit = found
        .iter()
        .find(|c| c.center.dist(centroid) < 0.1)
        .unwrap_or_else(|| panic!("no hay círculo en el hueco central: {found:?}"));
    assert!(
        (hit.radius - want_r).abs() < 0.1,
        "{hit:?} frente a r = {want_r}"
    );
}

#[test]
fn tangent_input_errors() {
    let a = vec![circle_poly(0.0, 0.0, 10.0, 60)];
    assert!(tangent_circles(std::slice::from_ref(&a), Some(1.0)).is_err());
    assert!(tangent_circles(&[a.clone(), a.clone()], None).is_err());
    assert!(tangent_circles(&[a.clone(), a.clone()], Some(-1.0)).is_err());
    assert!(tangent_circles(&[a.clone(), vec![]], Some(1.0)).is_err());
    // Lejos entre sí y con un radio chico: sin solución, pero sin error.
    let far = vec![circle_poly(500.0, 0.0, 10.0, 60)];
    assert!(tangent_circles(&[a, far], Some(1.0)).unwrap().is_empty());
}
