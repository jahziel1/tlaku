// Las estructuras de la biblioteca dxf se arman partiendo de Default.
#![allow(clippy::field_reassign_with_default)]

use std::f64::consts::PI;

use dxf::entities::{
    Arc, Circle, Ellipse, Insert, Line, LwPolyline, Polyline as DxfPolyline, Spline, Text, Vertex,
};
use dxf::enums::{AcadVersion, Units};
use dxf::tables::Layer as DxfLayer;
use dxf::{Block, Color, Drawing, LwPolylineVertex, Point as DxfPoint, Vector};

use super::*;

const TOL: f64 = 0.01;

fn drawing(units: Units) -> Drawing {
    let mut d = Drawing::new();
    d.header.version = AcadVersion::R2013;
    d.header.default_drawing_units = units;
    d
}

fn bytes(d: &Drawing) -> Vec<u8> {
    let mut v = Vec::new();
    d.save(&mut v).unwrap();
    v
}

fn pt(x: f64, y: f64) -> DxfPoint {
    DxfPoint::new(x, y, 0.0)
}

fn entity(e: EntityType) -> Entity {
    Entity::new(e)
}

fn line(a: (f64, f64), b: (f64, f64)) -> Entity {
    entity(EntityType::Line(Line::new(pt(a.0, a.1), pt(b.0, b.1))))
}

fn import(d: &Drawing) -> DxfDocument {
    import_dxf(&bytes(d), TOL).unwrap()
}

fn assert_pt(p: Point, x: f64, y: f64) {
    assert!(
        (p.x - x).abs() < 1e-6 && (p.y - y).abs() < 1e-6,
        "esperaba ({x}, {y}), salió ({}, {})",
        p.x,
        p.y
    );
}

/// Comprueba que la polilínea aproxima un círculo/arco de centro `c` y radio
/// `r` sin pasarse de `tol` (vértices sobre la curva, cuerdas por dentro).
fn assert_on_circle(poly: &Polyline, c: (f64, f64), r: f64, tol: f64) {
    let n = poly.points.len();
    let segs = if poly.closed { n } else { n - 1 };
    for p in &poly.points {
        let d = (p.x - c.0).hypot(p.y - c.1);
        assert!((d - r).abs() < 1e-6, "vértice a {d} del centro, radio {r}");
    }
    for i in 0..segs {
        let (a, b) = (poly.points[i], poly.points[(i + 1) % n]);
        let d = ((a.x + b.x) / 2.0 - c.0).hypot((a.y + b.y) / 2.0 - c.1);
        assert!(r - d <= tol + 1e-9, "flecha {} mayor que {tol}", r - d);
    }
}

#[test]
fn lwpolyline_bulge_is_a_semicircle_within_tolerance() {
    let mut d = drawing(Units::Millimeters);
    let mut p = LwPolyline::default();
    for (x, bulge) in [(0.0, 1.0), (10.0, 0.0)] {
        p.vertices.push(LwPolylineVertex {
            x,
            y: 0.0,
            bulge,
            ..Default::default()
        });
    }
    d.add_entity(entity(EntityType::LwPolyline(p)));
    let doc = import(&d);
    assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
    assert_eq!(doc.paths.len(), 1);
    let poly = &doc.paths[0].1;
    assert!(!poly.closed);
    assert_pt(poly.points[0], 0.0, 0.0);
    assert_pt(*poly.points.last().unwrap(), 10.0, 0.0);
    // Bulge positivo = antihorario: el arco pasa por debajo de la cuerda.
    let min_y = poly.points.iter().map(|p| p.y).fold(f64::MAX, f64::min);
    assert!((min_y + 5.0).abs() < TOL, "min_y = {min_y}");
    assert_on_circle(poly, (5.0, 0.0), 5.0, TOL);
    assert!(poly.points.len() > 8);
}

#[test]
fn negative_bulge_goes_the_other_way() {
    let mut d = drawing(Units::Millimeters);
    let mut p = LwPolyline::default();
    p.vertices.push(LwPolylineVertex {
        x: 0.0,
        bulge: -1.0,
        ..Default::default()
    });
    p.vertices.push(LwPolylineVertex {
        x: 10.0,
        ..Default::default()
    });
    d.add_entity(entity(EntityType::LwPolyline(p)));
    let poly = &import(&d).paths[0].1;
    let max_y = poly.points.iter().map(|p| p.y).fold(f64::MIN, f64::max);
    assert!((max_y - 5.0).abs() < TOL);
}

#[test]
fn closed_lwpolyline_with_two_bulges_is_a_circle() {
    let mut d = drawing(Units::Millimeters);
    let mut p = LwPolyline::default();
    p.flags = 1;
    for x in [0.0, 10.0] {
        p.vertices.push(LwPolylineVertex {
            x,
            bulge: 1.0,
            ..Default::default()
        });
    }
    d.add_entity(entity(EntityType::LwPolyline(p)));
    let poly = &import(&d).paths[0].1;
    assert!(poly.closed);
    assert_on_circle(poly, (5.0, 0.0), 5.0, TOL);
}

#[test]
fn closed_flag_and_implicit_closure() {
    let mut d = drawing(Units::Millimeters);
    let mut square = LwPolyline::default();
    square.flags = 1;
    for (x, y) in [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)] {
        square.vertices.push(LwPolylineVertex {
            x,
            y,
            ..Default::default()
        });
    }
    d.add_entity(entity(EntityType::LwPolyline(square)));
    // Cerrada por repetir el primer vértice, sin bandera.
    let mut tri = LwPolyline::default();
    for (x, y) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (0.0, 0.0)] {
        tri.vertices.push(LwPolylineVertex {
            x,
            y,
            ..Default::default()
        });
    }
    d.add_entity(entity(EntityType::LwPolyline(tri)));
    let doc = import(&d);
    assert!(doc.paths[0].1.closed);
    assert_eq!(doc.paths[0].1.points.len(), 4);
    assert!(doc.paths[1].1.closed);
    assert_eq!(doc.paths[1].1.points.len(), 3);
}

#[test]
fn heavy_polyline_with_bulge_and_closed_flag() {
    let mut d = drawing(Units::Millimeters);
    let mut p = DxfPolyline::default();
    p.flags = 1;
    for (x, y, b) in [
        (0.0, 0.0, 0.0),
        (10.0, 0.0, 1.0),
        (10.0, 10.0, 0.0),
        (0.0, 10.0, 0.0),
    ] {
        let mut v = Vertex::new(pt(x, y));
        v.bulge = b;
        p.add_vertex(&mut d, v);
    }
    d.add_entity(entity(EntityType::Polyline(p)));
    let poly = &import(&d).paths[0].1;
    assert!(poly.closed);
    // El tramo (10,0)->(10,10) con bulge 1 abulta hacia +X hasta x = 15.
    let max_x = poly.points.iter().map(|p| p.x).fold(f64::MIN, f64::max);
    assert!((max_x - 15.0).abs() < TOL, "max_x = {max_x}");
}

#[test]
fn circle_is_closed_and_within_tolerance() {
    let mut d = drawing(Units::Millimeters);
    d.add_entity(entity(EntityType::Circle(Circle::new(pt(3.0, -2.0), 25.0))));
    let doc = import(&d);
    let poly = &doc.paths[0].1;
    assert!(poly.closed);
    assert_pt(poly.points[0], 28.0, -2.0);
    assert_on_circle(poly, (3.0, -2.0), 25.0, TOL);
    // No debe pasarse de segmentos: error cercano a la tolerancia.
    let a = poly.points[0];
    let b = poly.points[1];
    let sagitta = 25.0 - ((a.x + b.x) / 2.0 - 3.0).hypot((a.y + b.y) / 2.0 + 2.0);
    assert!(
        sagitta > TOL / 2.0,
        "demasiados segmentos: flecha {sagitta}"
    );
}

#[test]
fn tiny_and_huge_tolerances_are_safe() {
    let mut d = drawing(Units::Millimeters);
    d.add_entity(entity(EntityType::Circle(Circle::new(pt(0.0, 0.0), 10.0))));
    let fine = import_dxf(&bytes(&d), 1e-9).unwrap();
    assert!(fine.paths[0].1.points.len() <= 20_000);
    let coarse = import_dxf(&bytes(&d), 1e6).unwrap();
    assert!(coarse.paths[0].1.points.len() >= 3);
    assert!(import_dxf(&bytes(&d), 0.0).is_err());
    assert!(import_dxf(&bytes(&d), f64::NAN).is_err());
    assert!(import_dxf(&bytes(&d), -1.0).is_err());
}

#[test]
fn arc_goes_counterclockwise_and_wraps_past_360() {
    let mut d = drawing(Units::Millimeters);
    d.add_entity(entity(EntityType::Arc(Arc::new(
        pt(0.0, 0.0),
        10.0,
        0.0,
        90.0,
    ))));
    d.add_entity(entity(EntityType::Arc(Arc::new(
        pt(0.0, 0.0),
        10.0,
        270.0,
        90.0,
    ))));
    let doc = import(&d);
    let quarter = &doc.paths[0].1;
    assert!(!quarter.closed);
    assert_pt(quarter.points[0], 10.0, 0.0);
    assert_pt(*quarter.points.last().unwrap(), 0.0, 10.0);
    assert_on_circle(quarter, (0.0, 0.0), 10.0, TOL);
    // 270 -> 90 pasa por 0 grados: es el semicírculo derecho.
    let half = &doc.paths[1].1;
    assert_pt(half.points[0], 0.0, -10.0);
    assert_pt(*half.points.last().unwrap(), 0.0, 10.0);
    assert!(half.points.iter().all(|p| p.x > -1e-9));
}

#[test]
fn full_arc_becomes_closed() {
    let mut d = drawing(Units::Millimeters);
    d.add_entity(entity(EntityType::Arc(Arc::new(
        pt(0.0, 0.0),
        5.0,
        0.0,
        360.0,
    ))));
    assert!(import(&d).paths[0].1.closed);
}

#[test]
fn mirrored_extrusion_flips_x() {
    let mut d = drawing(Units::Millimeters);
    let mut arc = Arc::new(pt(5.0, 0.0), 1.0, 0.0, 90.0);
    arc.normal = Vector::new(0.0, 0.0, -1.0);
    d.add_entity(entity(EntityType::Arc(arc)));
    let poly = &import(&d).paths[0].1;
    // Centro (5,0) en OCS espejado cae en (-5,0); el arco va de 180 a 90 grados.
    assert_pt(poly.points[0], -6.0, 0.0);
    assert_pt(*poly.points.last().unwrap(), -5.0, 1.0);
}

#[test]
fn full_ellipse_is_closed_and_on_the_curve() {
    let mut d = drawing(Units::Millimeters);
    let mut e = Ellipse::default();
    e.center = pt(1.0, 2.0);
    e.major_axis = Vector::new(10.0, 0.0, 0.0);
    e.minor_axis_ratio = 0.5;
    e.start_parameter = 0.0;
    e.end_parameter = TAU;
    d.add_entity(entity(EntityType::Ellipse(e)));
    let poly = &import(&d).paths[0].1;
    assert!(poly.closed);
    for p in &poly.points {
        let v = ((p.x - 1.0) / 10.0).powi(2) + ((p.y - 2.0) / 5.0).powi(2);
        assert!((v - 1.0).abs() < 1e-9);
    }
    // Las cuerdas no se alejan más de la tolerancia (el centro del lado tiene v < 1).
    let n = poly.points.len();
    for i in 0..n {
        let (a, b) = (poly.points[i], poly.points[(i + 1) % n]);
        let (mx, my) = ((a.x + b.x) / 2.0 - 1.0, (a.y + b.y) / 2.0 - 2.0);
        // Distancia aproximada a la elipse por la normal del gradiente.
        let v = (mx / 10.0).powi(2) + (my / 5.0).powi(2);
        let grad = (2.0 * mx / 100.0).hypot(2.0 * my / 25.0);
        assert!(
            (1.0 - v) / grad <= TOL * 1.2,
            "cuerda {i} se separa demasiado"
        );
    }
}

#[test]
fn partial_ellipse_is_open_with_exact_endpoints() {
    let mut d = drawing(Units::Millimeters);
    let mut e = Ellipse::default();
    e.major_axis = Vector::new(0.0, 4.0, 0.0);
    e.minor_axis_ratio = 0.5;
    e.start_parameter = 0.0;
    e.end_parameter = PI / 2.0;
    d.add_entity(entity(EntityType::Ellipse(e)));
    let poly = &import(&d).paths[0].1;
    assert!(!poly.closed);
    // Eje mayor en +Y: t = 0 en (0,4); t = pi/2 en el eje menor, hacia -X.
    assert_pt(poly.points[0], 0.0, 4.0);
    assert_pt(*poly.points.last().unwrap(), -2.0, 0.0);
}

fn spline(degree: i32, knots: &[f64], pts: &[(f64, f64)], weights: &[f64]) -> Spline {
    let mut s = Spline::default();
    s.degree_of_curve = degree;
    s.knot_values = knots.to_vec();
    s.control_points = pts.iter().map(|&(x, y)| pt(x, y)).collect();
    s.weight_values = weights.to_vec();
    s
}

#[test]
fn quadratic_bspline_is_a_parabola() {
    let mut d = drawing(Units::Millimeters);
    let s = spline(
        2,
        &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        &[(0.0, 0.0), (5.0, 10.0), (10.0, 0.0)],
        &[],
    );
    d.add_entity(entity(EntityType::Spline(s)));
    let poly = &import(&d).paths[0].1;
    assert!(!poly.closed);
    assert_pt(poly.points[0], 0.0, 0.0);
    assert_pt(*poly.points.last().unwrap(), 10.0, 0.0);
    // y = 20 x (10 - x) / 100 = x (10 - x) / 5 es la parábola exacta.
    for p in &poly.points {
        assert!((p.y - p.x * (10.0 - p.x) / 5.0).abs() < 1e-9);
    }
    let mid = poly.points.iter().map(|p| p.y).fold(f64::MIN, f64::max);
    assert!((mid - 5.0).abs() < TOL);
}

#[test]
fn rational_spline_is_an_exact_quarter_circle() {
    let mut d = drawing(Units::Millimeters);
    let w = std::f64::consts::FRAC_1_SQRT_2;
    let s = spline(
        2,
        &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        &[(10.0, 0.0), (10.0, 10.0), (0.0, 10.0)],
        &[1.0, w, 1.0],
    );
    d.add_entity(entity(EntityType::Spline(s)));
    let poly = &import(&d).paths[0].1;
    assert_on_circle(poly, (0.0, 0.0), 10.0, TOL);
    assert_pt(poly.points[0], 10.0, 0.0);
    assert_pt(*poly.points.last().unwrap(), 0.0, 10.0);
}

#[test]
fn cubic_spline_with_interior_knots_is_continuous() {
    let mut d = drawing(Units::Millimeters);
    let s = spline(
        3,
        &[0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0],
        &[(0.0, 0.0), (2.0, 5.0), (5.0, -5.0), (8.0, 5.0), (10.0, 0.0)],
        &[],
    );
    d.add_entity(entity(EntityType::Spline(s)));
    let poly = &import(&d).paths[0].1;
    assert_pt(poly.points[0], 0.0, 0.0);
    assert_pt(*poly.points.last().unwrap(), 10.0, 0.0);
    assert!(poly.points.len() > 10);
    let max_step = poly
        .points
        .windows(2)
        .map(|w| w[0].dist(w[1]))
        .fold(0.0, f64::max);
    assert!(max_step < 2.0, "salto de {max_step}");
}

#[test]
fn spline_without_knots_uses_a_clamped_uniform_vector() {
    let mut d = drawing(Units::Millimeters);
    let s = spline(2, &[], &[(0.0, 0.0), (5.0, 10.0), (10.0, 0.0)], &[]);
    d.add_entity(entity(EntityType::Spline(s)));
    let poly = &import(&d).paths[0].1;
    assert_pt(*poly.points.last().unwrap(), 10.0, 0.0);
}

#[test]
fn spline_with_only_fit_points_uses_them_and_warns() {
    let mut d = drawing(Units::Millimeters);
    let mut s = Spline::default();
    s.degree_of_curve = 3;
    s.fit_points = vec![pt(0.0, 0.0), pt(1.0, 2.0), pt(3.0, 1.0)];
    d.add_entity(entity(EntityType::Spline(s)));
    let doc = import(&d);
    assert_eq!(doc.paths[0].1.points.len(), 3);
    assert!(doc.warnings.iter().any(|w| w.contains("puntos de ajuste")));
}

#[test]
fn empty_spline_is_skipped_with_warning() {
    let mut d = drawing(Units::Millimeters);
    d.add_entity(entity(EntityType::Spline(Spline::default())));
    let doc = import(&d);
    assert!(doc.paths.is_empty());
    assert!(doc.warnings.iter().any(|w| w.contains("degenerada")));
}

#[test]
fn inches_are_scaled_by_25_4() {
    let mut d = drawing(Units::Inches);
    d.add_entity(line((0.0, 0.0), (2.0, 1.0)));
    let doc = import(&d);
    assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
    let p = &doc.paths[0].1.points;
    assert_pt(p[0], 0.0, 0.0);
    assert_pt(p[1], 50.8, 25.4);
}

#[test]
fn other_units_scale_correctly() {
    for (units, expected) in [
        (Units::Centimeters, 10.0),
        (Units::Meters, 1000.0),
        (Units::Microns, 0.001),
        (Units::Feet, 304.8),
        (Units::Millimeters, 1.0),
    ] {
        let mut d = drawing(units);
        d.add_entity(line((0.0, 0.0), (1.0, 0.0)));
        let doc = import(&d);
        assert_pt(doc.paths[0].1.points[1], expected, 0.0);
    }
}

#[test]
fn tolerance_applies_in_millimeters_after_unit_scaling() {
    // Círculo de 1 pulgada de radio: la tolerancia sigue siendo 0,01 mm.
    let mut d = drawing(Units::Inches);
    d.add_entity(entity(EntityType::Circle(Circle::new(pt(0.0, 0.0), 1.0))));
    let poly = &import(&d).paths[0].1;
    assert_on_circle(poly, (0.0, 0.0), 25.4, TOL);
}

#[test]
fn missing_units_warn_and_assume_mm() {
    let d = {
        let mut d = Drawing::new();
        d.add_entity(line((0.0, 0.0), (3.0, 0.0)));
        d
    };
    let doc = import(&d);
    assert_pt(doc.paths[0].1.points[1], 3.0, 0.0);
    assert_eq!(doc.warnings.len(), 1);
    assert!(doc.warnings[0].contains("unidades"));
    assert!(doc.warnings[0].contains("milímetros"));
}

fn block_with(d: &mut Drawing, name: &str, base: (f64, f64), entities: Vec<Entity>) {
    let mut b = Block::default();
    b.name = name.to_string();
    b.base_point = pt(base.0, base.1);
    b.entities = entities;
    d.add_block(b);
}

fn insert(name: &str, at: (f64, f64)) -> Insert {
    let mut i = Insert::default();
    i.name = name.to_string();
    i.location = pt(at.0, at.1);
    i
}

#[test]
fn rotated_and_scaled_insert() {
    let mut d = drawing(Units::Millimeters);
    block_with(&mut d, "B", (0.0, 0.0), vec![line((0.0, 0.0), (1.0, 0.0))]);
    let mut i = insert("B", (10.0, 10.0));
    i.x_scale_factor = 2.0;
    i.y_scale_factor = 2.0;
    i.rotation = 90.0;
    d.add_entity(entity(EntityType::Insert(i)));
    let doc = import(&d);
    assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
    let p = &doc.paths[0].1.points;
    assert_pt(p[0], 10.0, 10.0);
    assert_pt(p[1], 10.0, 12.0);
}

#[test]
fn insert_applies_block_base_point_and_non_uniform_mirror_scale() {
    let mut d = drawing(Units::Millimeters);
    block_with(&mut d, "B", (1.0, 1.0), vec![line((1.0, 1.0), (3.0, 1.0))]);
    let mut i = insert("b", (0.0, 0.0)); // el nombre no distingue mayúsculas
    i.x_scale_factor = -1.0;
    d.add_entity(entity(EntityType::Insert(i)));
    let p = &import(&d).paths[0].1.points;
    assert_pt(p[0], 0.0, 0.0);
    assert_pt(p[1], -2.0, 0.0);
}

#[test]
fn nested_inserts_compose_and_scale_the_tolerance() {
    let mut d = drawing(Units::Millimeters);
    block_with(
        &mut d,
        "INNER",
        (0.0, 0.0),
        vec![entity(EntityType::Circle(Circle::new(pt(0.0, 0.0), 1.0)))],
    );
    let mut inner = insert("INNER", (1.0, 0.0));
    inner.x_scale_factor = 10.0;
    inner.y_scale_factor = 10.0;
    block_with(
        &mut d,
        "OUTER",
        (0.0, 0.0),
        vec![entity(EntityType::Insert(inner))],
    );
    let mut outer = insert("OUTER", (100.0, 0.0));
    outer.rotation = 180.0;
    d.add_entity(entity(EntityType::Insert(outer)));
    let poly = &import(&d).paths[0].1;
    // Círculo de radio 10 centrado en 100 + (-1) = 99, con la tolerancia respetada.
    assert_on_circle(poly, (99.0, 0.0), 10.0, TOL);
}

#[test]
fn insert_array_rows_and_columns() {
    let mut d = drawing(Units::Millimeters);
    block_with(&mut d, "B", (0.0, 0.0), vec![line((0.0, 0.0), (1.0, 0.0))]);
    let mut i = insert("B", (0.0, 0.0));
    i.column_count = 3;
    i.row_count = 2;
    i.column_spacing = 10.0;
    i.row_spacing = 5.0;
    i.rotation = 90.0;
    i.x_scale_factor = 2.0;
    d.add_entity(entity(EntityType::Insert(i)));
    let doc = import(&d);
    assert_eq!(doc.paths.len(), 6);
    // Con 90 grados, las columnas avanzan hacia +Y y las filas hacia -X;
    // el espaciado no se escala. La línea escalada x2 apunta hacia +Y.
    let starts: Vec<(f64, f64)> = doc
        .paths
        .iter()
        .map(|(_, p)| (p.points[0].x, p.points[0].y))
        .collect();
    for col in 0..3 {
        for row in 0..2 {
            let expected = (-5.0 * row as f64, 10.0 * col as f64);
            assert!(
                starts
                    .iter()
                    .any(|s| (s.0 - expected.0).abs() < 1e-6 && (s.1 - expected.1).abs() < 1e-6),
                "falta la copia {expected:?} en {starts:?}"
            );
        }
    }
    assert_pt(doc.paths[0].1.points[1], 0.0, 2.0);
}

#[test]
fn self_referencing_block_does_not_loop() {
    let mut d = drawing(Units::Millimeters);
    block_with(
        &mut d,
        "LOOP",
        (0.0, 0.0),
        vec![
            line((0.0, 0.0), (1.0, 0.0)),
            entity(EntityType::Insert(insert("LOOP", (5.0, 0.0)))),
        ],
    );
    d.add_entity(entity(EntityType::Insert(insert("LOOP", (0.0, 0.0)))));
    let doc = import(&d);
    assert_eq!(doc.paths.len(), 1);
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.contains("se contiene a sí mismo"))
    );
}

#[test]
fn missing_block_warns() {
    let mut d = drawing(Units::Millimeters);
    d.add_entity(entity(EntityType::Insert(insert("NOPE", (0.0, 0.0)))));
    let doc = import(&d);
    assert!(doc.paths.is_empty());
    assert!(doc.warnings.iter().any(|w| w.contains("'NOPE'")));
}

fn add_layer(d: &mut Drawing, name: &str, color: Color) {
    let mut l = DxfLayer::default();
    l.name = name.to_string();
    l.color = color;
    d.add_layer(l);
}

fn on_layer(mut e: Entity, layer: &str) -> Entity {
    e.common.layer = layer.to_string();
    e
}

#[test]
fn bylayer_color_comes_from_the_layer() {
    let mut d = drawing(Units::Millimeters);
    add_layer(&mut d, "CORTE", Color::from_index(1));
    add_layer(&mut d, "GRABADO", Color::from_index(5));
    d.add_entity(on_layer(line((0.0, 0.0), (1.0, 0.0)), "CORTE"));
    d.add_entity(on_layer(line((0.0, 1.0), (1.0, 1.0)), "GRABADO"));
    let mut explicit = on_layer(line((0.0, 2.0), (1.0, 2.0)), "CORTE");
    explicit.common.color = Color::from_index(3);
    d.add_entity(explicit);
    let doc = import(&d);
    assert_eq!(doc.paths[0].0, Rgb(255, 0, 0));
    assert_eq!(doc.paths[1].0, Rgb(0, 0, 255));
    assert_eq!(doc.paths[2].0, Rgb(0, 255, 0));
}

#[test]
fn true_color_wins_over_aci() {
    let mut d = drawing(Units::Millimeters);
    add_layer(&mut d, "L", Color::from_index(1));
    let mut e = on_layer(line((0.0, 0.0), (1.0, 0.0)), "L");
    e.common.color_24_bit = 0x00_12_34_56;
    d.add_entity(e);
    assert_eq!(import(&d).paths[0].0, Rgb(0x12, 0x34, 0x56));
}

#[test]
fn byblock_takes_the_insert_color_and_layer_zero_the_insert_layer() {
    let mut d = drawing(Units::Millimeters);
    add_layer(&mut d, "ROJO", Color::from_index(1));
    add_layer(&mut d, "AZUL", Color::from_index(5));
    let mut by_block = line((0.0, 0.0), (1.0, 0.0));
    by_block.common.color = Color::by_block();
    let by_layer0 = line((0.0, 1.0), (1.0, 1.0)); // capa "0", BYLAYER
    let mut own_layer = on_layer(line((0.0, 2.0), (1.0, 2.0)), "AZUL");
    own_layer.common.color = Color::by_layer();
    block_with(
        &mut d,
        "B",
        (0.0, 0.0),
        vec![by_block, by_layer0, own_layer],
    );

    let mut green_insert = entity(EntityType::Insert(insert("B", (0.0, 0.0))));
    green_insert.common.color = Color::from_index(3);
    green_insert.common.layer = "ROJO".to_string();
    d.add_entity(green_insert);
    // INSERT BYLAYER en la capa ROJO: lo BYBLOCK toma el color de la capa.
    d.add_entity(on_layer(
        entity(EntityType::Insert(insert("B", (10.0, 0.0)))),
        "ROJO",
    ));
    let doc = import(&d);
    let colors: Vec<Rgb> = doc.paths.iter().map(|(c, _)| *c).collect();
    assert_eq!(colors[0], Rgb(0, 255, 0)); // BYBLOCK -> verde del INSERT
    assert_eq!(colors[1], Rgb(255, 0, 0)); // capa 0 -> capa del INSERT (ROJO)
    assert_eq!(colors[2], Rgb(0, 0, 255));
    assert_eq!(colors[3], Rgb(255, 0, 0)); // BYBLOCK con INSERT BYLAYER
}

#[test]
fn byblock_at_top_level_falls_back_to_default() {
    let mut d = drawing(Units::Millimeters);
    let mut e = line((0.0, 0.0), (1.0, 0.0));
    e.common.color = Color::by_block();
    d.add_entity(e);
    assert_eq!(import(&d).paths[0].0, DEFAULT_COLOR);
}

#[test]
fn text_is_skipped_with_a_counted_warning() {
    let mut d = drawing(Units::Millimeters);
    d.add_entity(line((0.0, 0.0), (1.0, 0.0)));
    for _ in 0..3 {
        d.add_entity(entity(EntityType::Text(Text::default())));
    }
    let doc = import(&d);
    assert_eq!(doc.paths.len(), 1);
    assert_eq!(doc.warnings.len(), 1);
    assert!(
        doc.warnings[0].contains("3 entidades de texto"),
        "{:?}",
        doc.warnings
    );
}

#[test]
fn single_skipped_entity_uses_singular() {
    let mut d = drawing(Units::Millimeters);
    d.add_entity(entity(EntityType::Text(Text::default())));
    let doc = import(&d);
    assert!(doc.warnings[0].starts_with("Se omitió 1 entidad "));
}

#[test]
fn layers_off_and_invisible_entities_are_skipped() {
    let mut d = drawing(Units::Millimeters);
    let mut layer = DxfLayer::default();
    layer.name = "OFF".into();
    layer.is_layer_on = false;
    d.add_layer(layer);
    d.add_entity(on_layer(line((0.0, 0.0), (1.0, 0.0)), "OFF"));
    let mut hidden = line((0.0, 1.0), (1.0, 1.0));
    hidden.common.is_visible = false;
    d.add_entity(hidden);
    d.add_entity(line((0.0, 2.0), (1.0, 2.0)));
    let doc = import(&d);
    assert_eq!(doc.paths.len(), 1);
    assert!(doc.warnings.iter().any(|w| w.contains("capas apagadas")));
    assert!(doc.warnings.iter().any(|w| w.contains("invisibles")));
}

const HAND_WRITTEN: &str = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n8\n0\n2\nB\n70\n0\n10\n0\n20\n0\n30\n0\n3\nB\n1\n\n\
0\nHATCH\n8\n0\n91\n0\n0\nENDBLK\n8\n0\n0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n\
0\nHATCH\n8\n0\n0\nHATCH\n8\n0\n\
0\nINSERT\n8\n0\n2\nB\n10\n0\n20\n0\n30\n0\n\
0\nLINE\n8\n0\n10\n0\n20\n0\n30\n0\n11\n5\n21\n0\n31\n0\n\
0\nENDSEC\n0\nEOF\n";

#[test]
fn hatch_is_counted_from_the_raw_text() {
    let doc = import_dxf(HAND_WRITTEN.as_bytes(), TOL).unwrap();
    assert_eq!(doc.paths.len(), 1);
    assert_pt(doc.paths[0].1.points[1], 5.0, 0.0);
    // 2 en ENTITIES + 1 dentro del bloque insertado una vez.
    assert!(
        doc.warnings.iter().any(|w| w.contains("3 entidades HATCH")),
        "{:?}",
        doc.warnings
    );
    assert!(!doc.warnings.iter().any(|w| w.contains("unidades")));
}

#[test]
fn crlf_line_endings_are_supported_by_the_hatch_counter() {
    let text = HAND_WRITTEN.replace('\n', "\r\n");
    let doc = import_dxf(text.as_bytes(), TOL).unwrap();
    assert!(doc.warnings.iter().any(|w| w.contains("3 entidades HATCH")));
}

#[test]
fn dimension_and_unsupported_entities_are_reported() {
    let mut d = drawing(Units::Millimeters);
    d.add_entity(entity(EntityType::ModelPoint(Default::default())));
    d.add_entity(entity(EntityType::ModelPoint(Default::default())));
    let doc = import(&d);
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.contains("2 entidades de tipos no soportados (POINT: 2)")),
        "{:?}",
        doc.warnings
    );
}

#[test]
fn garbage_input_is_an_error_not_a_panic() {
    assert!(import_dxf(b"", TOL).is_err());
    assert!(import_dxf(b"esto no es un dxf", TOL).is_err());
    assert!(import_dxf(&[0xff, 0xfe, 0x00, 0x01], TOL).is_err());
}

#[test]
fn degenerate_geometry_is_dropped() {
    let mut d = drawing(Units::Millimeters);
    d.add_entity(line((1.0, 1.0), (1.0, 1.0)));
    d.add_entity(entity(EntityType::Circle(Circle::new(pt(0.0, 0.0), 0.0))));
    d.add_entity(line((f64::NAN, 0.0), (1.0, 1.0)));
    let doc = import(&d);
    assert!(doc.paths.is_empty());
}

#[test]
fn zero_scale_insert_is_dropped() {
    let mut d = drawing(Units::Millimeters);
    block_with(&mut d, "B", (0.0, 0.0), vec![line((0.0, 0.0), (1.0, 0.0))]);
    let mut i = insert("B", (0.0, 0.0));
    i.x_scale_factor = 0.0;
    i.y_scale_factor = 0.0;
    d.add_entity(entity(EntityType::Insert(i)));
    assert!(import(&d).paths.is_empty());
}

#[test]
fn into_job_groups_by_color_and_bounds() {
    let mut d = drawing(Units::Millimeters);
    add_layer(&mut d, "A", Color::from_index(1));
    add_layer(&mut d, "B", Color::from_index(5));
    d.add_entity(on_layer(line((0.0, 0.0), (1.0, 0.0)), "A"));
    d.add_entity(on_layer(line((0.0, 1.0), (4.0, 3.0)), "B"));
    d.add_entity(on_layer(line((2.0, 2.0), (3.0, 2.0)), "A"));
    let doc = import(&d);
    let b = doc.bounds().unwrap();
    assert_pt(b.min, 0.0, 0.0);
    assert_pt(b.max, 4.0, 3.0);
    let job = doc.into_job(|_| CutSettings::default());
    assert_eq!(job.layers.len(), 2);
    assert_eq!(job.layers[0].color, Rgb(255, 0, 0));
    assert_eq!(job.layers[0].paths.len(), 2);
    assert!(import(&drawing(Units::Millimeters)).bounds().is_none());
}

#[test]
fn paper_space_entities_are_skipped() {
    let mut d = drawing(Units::Millimeters);
    let mut e = line((0.0, 0.0), (1.0, 0.0));
    e.common.is_in_paper_space = true;
    d.add_entity(e);
    let doc = import(&d);
    assert!(doc.paths.is_empty());
    assert!(doc.warnings.iter().any(|w| w.contains("espacio papel")));
}
