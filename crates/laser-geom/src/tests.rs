use super::*;
use std::f64::consts::PI;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn poly(pts: &[(f64, f64)], closed: bool) -> Polyline {
    Polyline {
        points: pts.iter().map(|&(x, y)| pt(x, y)).collect(),
        closed,
    }
}

/// Rectángulo antihorario.
fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Polyline {
    poly(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], true)
}

fn reversed(p: &Polyline) -> Polyline {
    let mut q = p.clone();
    q.points.reverse();
    q
}

/// Área neta de una región: exteriores positivos y agujeros negativos.
fn region_area(paths: &[Polyline]) -> f64 {
    paths.iter().map(|p| signed_area(&p.points)).sum()
}

fn assert_close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} != {b} (tol {tol})");
}

// --- utilidades ---

#[test]
fn area_and_orientation() {
    let sq = rect(0.0, 0.0, 3.0, 2.0);
    assert_close(signed_area(&sq.points), 6.0, 1e-12);
    assert_eq!(orientation(&sq.points), Orientation::Ccw);
    assert_close(signed_area(&reversed(&sq).points), -6.0, 1e-12);
    assert_eq!(orientation(&reversed(&sq).points), Orientation::Cw);
    assert_eq!(orientation(&[]), Orientation::Degenerate);
    assert_eq!(
        orientation(&[pt(0.0, 0.0), pt(1.0, 1.0)]),
        Orientation::Degenerate
    );
    let line = [pt(0.0, 0.0), pt(1.0, 1.0), pt(2.0, 2.0)];
    assert_eq!(orientation(&line), Orientation::Degenerate);
    assert!(signed_area(&[pt(f64::NAN, 0.0), pt(1.0, 0.0), pt(0.0, 1.0)]).is_nan());
}

#[test]
fn area_is_translation_invariant_for_large_offsets() {
    let sq = rect(1e6, 1e6, 1e6 + 1.0, 1e6 + 1.0);
    assert_close(signed_area(&sq.points), 1.0, 1e-9);
}

#[test]
fn point_location() {
    let sq = rect(0.0, 0.0, 4.0, 4.0).points;
    assert!(point_in_polygon(pt(2.0, 2.0), &sq));
    assert!(!point_in_polygon(pt(5.0, 2.0), &sq));
    assert!(!point_in_polygon(pt(2.0, -1.0), &sq));
    assert_eq!(locate_point(pt(0.0, 2.0), &sq), PointLocation::Boundary);
    assert_eq!(locate_point(pt(4.0, 4.0), &sq), PointLocation::Boundary);
    assert_eq!(locate_point(pt(2.0, 2.0), &[]), PointLocation::Outside);
    assert_eq!(locate_point(pt(2.0, 2.0), &sq[..2]), PointLocation::Outside);
    // Cóncava (en U): el hueco de la U está fuera.
    let u = poly(
        &[
            (0.0, 0.0),
            (6.0, 0.0),
            (6.0, 6.0),
            (4.0, 6.0),
            (4.0, 2.0),
            (2.0, 2.0),
            (2.0, 6.0),
            (0.0, 6.0),
        ],
        true,
    )
    .points;
    assert!(point_in_polygon(pt(1.0, 5.0), &u));
    assert!(!point_in_polygon(pt(3.0, 5.0), &u));
    assert!(point_in_polygon(pt(3.0, 1.0), &u));
    assert!(!point_in_polygon(pt(f64::NAN, 1.0), &u));
}

#[test]
fn redundant_points_open() {
    let p = poly(
        &[
            (0.0, 0.0),
            (0.0, 0.0),
            (1.0, 0.0),
            (2.0, 0.0),
            (2.0, 2.0),
            (2.0, 2.0),
        ],
        false,
    );
    let r = remove_redundant_points(&p, 0.0);
    assert_eq!(r, poly(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0)], false));
}

#[test]
fn redundant_points_closed_wraps_around() {
    // Punto repetido al final y colineales en la esquina de cierre.
    let p = poly(
        &[
            (2.0, 0.0),
            (4.0, 0.0),
            (4.0, 4.0),
            (0.0, 4.0),
            (0.0, 0.0),
            (1.0, 0.0),
            (2.0, 0.0),
        ],
        true,
    );
    let r = remove_redundant_points(&p, 1e-9);
    assert!(r.closed);
    assert_eq!(r.points.len(), 4);
    assert_close(signed_area(&r.points), 16.0, 1e-12);
}

#[test]
fn redundant_points_keeps_spikes_and_minimum() {
    // La punta (0,0)->(5,0)->(1,0) retrocede: no es "pasar de largo".
    let p = poly(&[(0.0, 0.0), (5.0, 0.0), (1.0, 0.0)], false);
    assert_eq!(remove_redundant_points(&p, 1e-9).points.len(), 3);
    let line = poly(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)], true);
    assert!(remove_redundant_points(&line, 1e-9).points.len() >= 2);
    let empty = poly(&[], true);
    assert!(remove_redundant_points(&empty, 1.0).points.is_empty());
    let nan = poly(&[(f64::NAN, 0.0), (1.0, 1.0), (2.0, 0.0)], false);
    assert_eq!(remove_redundant_points(&nan, f64::NAN).points.len(), 2);
}

// --- anidamiento ---

fn donut_with_island() -> Vec<Polyline> {
    vec![
        rect(0.0, 0.0, 20.0, 20.0),
        reversed(&rect(4.0, 4.0, 16.0, 16.0)),
        rect(8.0, 8.0, 12.0, 12.0),
    ]
}

#[test]
fn depth_of_donut_and_island() {
    assert_eq!(nesting_depth(&donut_with_island()), vec![0, 1, 2]);
}

#[test]
fn depth_ignores_winding_and_input_order() {
    let mut paths = donut_with_island();
    paths.reverse();
    // El orden de entrada no importa, ni el sentido de giro.
    paths[0] = reversed(&paths[0]);
    assert_eq!(nesting_depth(&paths), vec![2, 1, 0]);
}

#[test]
fn depth_degenerate_and_side_by_side() {
    assert!(nesting_depth(&[]).is_empty());
    let paths = vec![
        rect(0.0, 0.0, 4.0, 4.0),
        rect(10.0, 0.0, 14.0, 4.0),
        poly(&[(1.0, 1.0), (2.0, 2.0), (3.0, 1.0)], false),
        poly(&[(1.0, 1.0)], true),
        poly(&[(1.0, 1.0), (2.0, 2.0), (3.0, 3.0)], true),
        // idéntica a la primera: no se contienen entre sí
        rect(0.0, 0.0, 4.0, 4.0),
    ];
    assert_eq!(nesting_depth(&paths), vec![0; 6]);
}

#[test]
fn depth_crossing_squares_do_not_contain() {
    let paths = vec![rect(0.0, 0.0, 4.0, 4.0), rect(2.0, 2.0, 6.0, 6.0)];
    assert_eq!(nesting_depth(&paths), vec![0, 0]);
}

#[test]
fn depth_inner_touching_border_still_contained() {
    let paths = vec![rect(0.0, 0.0, 10.0, 10.0), rect(0.0, 0.0, 5.0, 5.0)];
    assert_eq!(nesting_depth(&paths), vec![0, 1]);
}

// --- booleanas ---

#[test]
fn boolean_overlapping_squares() {
    let a = [rect(0.0, 0.0, 4.0, 4.0)];
    let b = [rect(2.0, 2.0, 6.0, 6.0)];
    let area = |op| region_area(&boolean(&a, &b, op));
    assert_close(area(BoolOp::Union), 28.0, 1e-6);
    assert_close(area(BoolOp::Difference), 12.0, 1e-6);
    assert_close(area(BoolOp::Intersection), 4.0, 1e-6);
    assert_close(area(BoolOp::Xor), 24.0, 1e-6);
    // La diferencia no es simétrica pero tiene la misma área aquí; comprueba la forma.
    let inter = boolean(&a, &b, BoolOp::Intersection);
    assert_eq!(inter.len(), 1);
    assert_eq!(inter[0].points.len(), 4);
    assert_eq!(orientation(&inter[0].points), Orientation::Ccw);
}

#[test]
fn boolean_difference_orientation_matters() {
    let a = [rect(0.0, 0.0, 4.0, 4.0)];
    let b = [rect(3.0, 0.0, 10.0, 4.0)];
    assert_close(
        region_area(&boolean(&a, &b, BoolOp::Difference)),
        12.0,
        1e-6,
    );
    assert_close(
        region_area(&boolean(&b, &a, BoolOp::Difference)),
        24.0,
        1e-6,
    );
}

#[test]
fn boolean_creates_hole() {
    let a = [rect(0.0, 0.0, 10.0, 10.0)];
    let b = [rect(3.0, 3.0, 7.0, 7.0)];
    let r = boolean(&a, &b, BoolOp::Difference);
    assert_eq!(r.len(), 2);
    assert_eq!(orientation(&r[0].points), Orientation::Ccw);
    assert_eq!(orientation(&r[1].points), Orientation::Cw);
    assert_close(region_area(&r), 84.0, 1e-6);
    // A ∪ B con B dentro de A (conjuntos distintos) no genera agujero.
    let u = boolean(&a, &b, BoolOp::Union);
    assert_eq!(u.len(), 1);
    assert_close(region_area(&u), 100.0, 1e-6);
}

#[test]
fn boolean_disjoint_and_empty_inputs() {
    let a = [rect(0.0, 0.0, 1.0, 1.0)];
    let b = [rect(5.0, 5.0, 6.0, 6.0)];
    assert!(boolean(&a, &b, BoolOp::Intersection).is_empty());
    assert_eq!(boolean(&a, &b, BoolOp::Union).len(), 2);
    assert_close(region_area(&boolean(&a, &b, BoolOp::Xor)), 2.0, 1e-9);
    for op in [
        BoolOp::Union,
        BoolOp::Difference,
        BoolOp::Intersection,
        BoolOp::Xor,
    ] {
        assert!(boolean(&[], &[], op).is_empty());
    }
    assert_close(region_area(&boolean(&a, &[], BoolOp::Union)), 1.0, 1e-9);
    assert_close(
        region_area(&boolean(&a, &[], BoolOp::Difference)),
        1.0,
        1e-9,
    );
    assert!(boolean(&a, &[], BoolOp::Intersection).is_empty());
    assert!(boolean(&[], &a, BoolOp::Difference).is_empty());
    assert_close(region_area(&boolean(&[], &a, BoolOp::Xor)), 1.0, 1e-9);
}

#[test]
fn boolean_ignores_open_and_degenerate() {
    let a = [
        rect(0.0, 0.0, 4.0, 4.0),
        poly(&[(0.0, 0.0), (9.0, 9.0)], false),
    ];
    let b = [
        poly(&[(1.0, 1.0)], true),
        poly(&[(1.0, 1.0), (2.0, 1.0), (3.0, 1.0)], true),
    ];
    assert_close(region_area(&boolean(&a, &b, BoolOp::Union)), 16.0, 1e-9);
}

#[test]
fn boolean_independent_of_winding() {
    let a = [reversed(&rect(0.0, 0.0, 4.0, 4.0))];
    let b = [reversed(&rect(2.0, 2.0, 6.0, 6.0))];
    assert_close(region_area(&boolean(&a, &b, BoolOp::Union)), 28.0, 1e-6);
}

#[test]
fn boolean_touching_squares_merge() {
    let a = [rect(0.0, 0.0, 4.0, 4.0)];
    let b = [rect(4.0, 0.0, 8.0, 4.0)];
    let u = boolean(&a, &b, BoolOp::Union);
    assert_eq!(u.len(), 1);
    assert_close(region_area(&u), 32.0, 1e-6);
}

// --- weld ---

#[test]
fn weld_overlapping() {
    let r = weld(&[
        rect(0.0, 0.0, 4.0, 4.0),
        rect(2.0, 2.0, 6.0, 6.0),
        rect(5.0, 5.0, 9.0, 9.0),
    ]);
    assert_eq!(r.len(), 1);
    assert_close(region_area(&r), 16.0 + 16.0 - 4.0 + 16.0 - 1.0, 1e-6);
}

#[test]
fn weld_keeps_holes_and_content_inside_them() {
    // Anillo (exterior + agujero) con una letra dentro del agujero.
    let paths = [
        rect(0.0, 0.0, 20.0, 20.0),
        reversed(&rect(4.0, 4.0, 16.0, 16.0)),
        rect(8.0, 8.0, 12.0, 12.0),
    ];
    let r = weld(&paths);
    assert_eq!(r.len(), 3);
    assert_close(region_area(&r), 400.0 - 144.0 + 16.0, 1e-6);
    // Una pieza que cubre parte del agujero lo recorta.
    let mut with_bar = paths.to_vec();
    with_bar.push(rect(-2.0, 9.0, 22.0, 11.0));
    let r = weld(&with_bar);
    let ring = 400.0 - 144.0 + 16.0;
    let bar_extra =
        2.0 * 24.0 - (2.0 * 4.0 * 2.0 /* barra sobre el aro */ + 4.0 * 2.0/* sobre la isla */);
    assert_close(region_area(&r), ring + bar_extra, 1e-6);
}

#[test]
fn weld_empty_and_degenerate() {
    assert!(weld(&[]).is_empty());
    assert!(weld(&[poly(&[(0.0, 0.0), (1.0, 0.0)], false)]).is_empty());
    assert!(weld(&[poly(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)], true)]).is_empty());
}

// --- offset ---

#[test]
fn offset_square_round_matches_analytic_area() {
    let sq = [rect(0.0, 0.0, 10.0, 10.0)];
    let d = 2.0;
    let analytic = 100.0 + 40.0 * d + PI * d * d;
    let r = offset(&sq, d, Join::Round, 0.001);
    assert_eq!(r.len(), 1);
    let area = region_area(&r);
    // El polígono inscrito queda ligeramente por debajo del círculo ideal.
    assert!(
        area <= analytic + 1e-6 && area > analytic - 0.02,
        "{area} vs {analytic}"
    );
}

#[test]
fn offset_square_miter_and_bevel() {
    let sq = [rect(0.0, 0.0, 10.0, 10.0)];
    let miter = offset(&sq, 1.0, Join::Miter { limit: 2.0 }, 0.01);
    assert_close(region_area(&miter), 144.0, 1e-6);
    let bevel = offset(&sq, 1.0, Join::Bevel, 0.01);
    assert_close(region_area(&bevel), 142.0, 1e-6);
}

#[test]
fn offset_miter_limit_clips_sharp_corners() {
    let tri = [poly(&[(0.0, 0.0), (10.0, 0.0), (10.0, 1.0)], true)];
    let long = region_area(&offset(&tri, 1.0, Join::Miter { limit: 100.0 }, 0.01));
    let short = region_area(&offset(&tri, 1.0, Join::Miter { limit: 1.5 }, 0.01));
    assert!(short < long);
    assert!(short > 5.0);
}

#[test]
fn offset_inward_and_vanishing_shapes() {
    let sq = [rect(0.0, 0.0, 10.0, 10.0)];
    assert_close(
        region_area(&offset(&sq, -1.0, Join::Round, 0.01)),
        64.0,
        1e-6,
    );
    assert!(offset(&sq, -5.0, Join::Round, 0.01).is_empty());
    assert!(offset(&sq, -6.0, Join::Round, 0.01).is_empty());
    assert!(offset(&sq, -1e9, Join::Round, 0.01).is_empty());
    // Pieza fina que desaparece antes que otra gruesa.
    let two = [rect(0.0, 0.0, 20.0, 20.0), rect(30.0, 0.0, 31.0, 20.0)];
    let r = offset(&two, -1.0, Join::Miter { limit: 2.0 }, 0.01);
    assert_eq!(r.len(), 1);
    assert_close(region_area(&r), 18.0 * 18.0, 1e-6);
}

#[test]
fn offset_huge_distance_terminates() {
    let sq = [rect(0.0, 0.0, 10.0, 10.0)];
    let start = std::time::Instant::now();
    for d in [1e3, 1e5, 1e7, 1e12, f64::MAX] {
        let r = offset(&sq, d, Join::Round, 0.01);
        assert!(!r.is_empty());
        assert!(region_area(&r) > 100.0);
    }
    assert!(start.elapsed().as_secs() < 20);
}

#[test]
fn offset_non_finite_and_zero() {
    let sq = [rect(0.0, 0.0, 10.0, 10.0)];
    assert!(offset(&sq, f64::NAN, Join::Round, 0.01).is_empty());
    assert!(offset(&sq, f64::INFINITY, Join::Bevel, 0.01).is_empty());
    assert_close(
        region_area(&offset(&sq, 0.0, Join::Round, 0.01)),
        100.0,
        1e-9,
    );
    // Tolerancias y límites inválidos no rompen nada.
    for tol in [0.0, -1.0, f64::NAN, 1e9] {
        assert!(!offset(&sq, 1.0, Join::Round, tol).is_empty());
    }
    for limit in [0.0, -3.0, f64::NAN, f64::INFINITY, 1e12] {
        assert!(!offset(&sq, 1.0, Join::Miter { limit }, 0.01).is_empty());
    }
    assert!(offset(&[], 1.0, Join::Round, 0.01).is_empty());
}

#[test]
fn offset_donut_hole_moves_opposite() {
    let donut = [
        rect(0.0, 0.0, 20.0, 20.0),
        reversed(&rect(5.0, 5.0, 15.0, 15.0)),
    ];
    let r = offset(&donut, 1.0, Join::Miter { limit: 2.0 }, 0.01);
    assert_eq!(r.len(), 2);
    assert_close(signed_area(&r[0].points), 22.0 * 22.0, 1e-6);
    assert_close(signed_area(&r[1].points), -8.0 * 8.0, 1e-6);
    // Un agujero que se cierra desaparece y deja la pieza maciza.
    let r = offset(&donut, 5.0, Join::Miter { limit: 2.0 }, 0.01);
    assert_eq!(r.len(), 1);
    assert_close(region_area(&r), 30.0 * 30.0, 1e-6);
}

#[test]
fn offset_concave_inward_trims_overlaps() {
    // Forma en L: el offset interior no se autointersecta.
    let l = [poly(
        &[
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 4.0),
            (4.0, 4.0),
            (4.0, 10.0),
            (0.0, 10.0),
        ],
        true,
    )];
    let r = offset(&l, -1.0, Join::Miter { limit: 2.0 }, 0.01);
    assert_eq!(r.len(), 1);
    // L de brazos de ancho 4 -> ancho 2: 8 x 2 + 2 x 6 con el codo compartido.
    assert_close(region_area(&r), 8.0 * 2.0 + 2.0 * 6.0, 1e-6);
}

// --- kerf ---

#[test]
fn kerf_donut_outer_grows_hole_shrinks() {
    let donut = [
        rect(0.0, 0.0, 20.0, 20.0),
        reversed(&rect(5.0, 5.0, 15.0, 15.0)),
    ];
    let r = kerf_compensate_with(&donut, 0.5, Join::Miter { limit: 2.0 }, 0.01);
    assert_eq!(r.len(), 2);
    assert_close(signed_area(&r[0].points), 21.0 * 21.0, 1e-6);
    assert_close(signed_area(&r[1].points), -9.0 * 9.0, 1e-6);
    // Con signo negativo, al revés.
    let r = kerf_compensate_with(&donut, -0.5, Join::Miter { limit: 2.0 }, 0.01);
    assert_close(signed_area(&r[0].points), 19.0 * 19.0, 1e-6);
    assert_close(signed_area(&r[1].points), -11.0 * 11.0, 1e-6);
    // Por defecto usa uniones redondas.
    let r = kerf_compensate(&donut, 0.5);
    // 400 + perímetro·d + π d²
    assert_close(
        signed_area(&r[0].points),
        400.0 + 80.0 * 0.5 + PI * 0.25,
        0.01,
    );
    assert_close(signed_area(&r[1].points), -81.0, 1e-6);
}

#[test]
fn kerf_classification_ignores_winding() {
    // Dona dibujada con ambos contornos antihorarios: sigue siendo dona.
    let donut = [rect(0.0, 0.0, 20.0, 20.0), rect(5.0, 5.0, 15.0, 15.0)];
    let r = kerf_compensate_with(&donut, 0.5, Join::Miter { limit: 2.0 }, 0.01);
    assert_eq!(r.len(), 2);
    assert_close(region_area(&r), 441.0 - 81.0, 1e-6);
}

#[test]
fn kerf_island_in_hole_grows() {
    let paths = [
        rect(0.0, 0.0, 30.0, 30.0),
        reversed(&rect(5.0, 5.0, 25.0, 25.0)),
        rect(10.0, 10.0, 20.0, 20.0),
    ];
    let r = kerf_compensate_with(&paths, 1.0, Join::Miter { limit: 2.0 }, 0.01);
    let mut areas: Vec<f64> = r.iter().map(|p| signed_area(&p.points)).collect();
    areas.sort_by(f64::total_cmp);
    assert_eq!(areas.len(), 3);
    assert_close(areas[0], -18.0 * 18.0, 1e-6);
    assert_close(areas[1], 12.0 * 12.0, 1e-6);
    assert_close(areas[2], 32.0 * 32.0, 1e-6);
}

#[test]
fn kerf_pieces_stay_independent_and_open_pass_through() {
    let open = poly(&[(0.0, 0.0), (5.0, 5.0)], false);
    let paths = [
        rect(0.0, 0.0, 4.0, 4.0),
        rect(3.0, 0.0, 8.0, 4.0),
        open.clone(),
    ];
    let r = kerf_compensate_with(&paths, 0.5, Join::Miter { limit: 2.0 }, 0.01);
    let closed: Vec<_> = r.iter().filter(|p| p.closed).collect();
    assert_eq!(closed.len(), 2);
    assert_eq!(r.last(), Some(&open));
    assert!(kerf_compensate(&[], 1.0).is_empty());
    assert_eq!(
        kerf_compensate(std::slice::from_ref(&open), 1.0),
        vec![open]
    );
}

#[test]
fn kerf_zero_keeps_geometry_and_vanishing_piece_dropped() {
    let sq = [rect(0.0, 0.0, 2.0, 2.0)];
    assert_close(region_area(&kerf_compensate(&sq, 0.0)), 4.0, 1e-9);
    assert!(kerf_compensate(&sq, -1.5).is_empty());
}

// --- auto_join / close ---

#[test]
fn auto_join_four_loose_lines_make_a_rectangle() {
    // Cuatro lados sueltos, algunos invertidos y desordenados, con holguras < 0,05.
    let lines = vec![
        poly(&[(0.0, 0.0), (10.0, 0.0)], false),
        poly(&[(10.0, 5.0), (10.02, 0.01)], false),
        poly(&[(0.01, 5.0), (10.0, 5.01)], false),
        poly(&[(0.0, 0.0), (0.0, 5.0)], false),
    ];
    let r = auto_join(&lines, 0.05);
    assert_eq!(r.len(), 1);
    assert!(r[0].closed);
    assert_eq!(r[0].points.len(), 4);
    assert_close(signed_area(&r[0].points).abs(), 50.0, 0.5);
}

#[test]
fn auto_join_respects_tolerance_and_direction() {
    let lines = vec![
        poly(&[(0.0, 0.0), (1.0, 0.0)], false),
        poly(&[(3.0, 0.0), (2.0, 0.0)], false),
        poly(&[(1.0, 0.0), (2.0, 0.0)], false),
        poly(&[(20.0, 0.0), (21.0, 0.0)], false),
    ];
    let r = auto_join(&lines, 0.05);
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].points.len(), 4);
    assert!(!r[0].closed);
    assert_eq!(r[0].points[0], pt(0.0, 0.0));
    assert_eq!(r[0].points[3], pt(3.0, 0.0));
    assert_eq!(r[1], lines[3]);
    // Fuera de tolerancia no se une.
    let apart = vec![
        poly(&[(0.0, 0.0), (1.0, 0.0)], false),
        poly(&[(1.2, 0.0), (2.0, 0.0)], false),
    ];
    assert_eq!(auto_join(&apart, 0.05).len(), 2);
    assert_eq!(auto_join(&apart, 0.3).len(), 1);
}

#[test]
fn auto_join_prepends_to_head() {
    let lines = vec![
        poly(&[(5.0, 0.0), (6.0, 0.0)], false),
        poly(&[(4.0, 0.0), (5.0, 0.0)], false),
        poly(&[(3.0, 0.0), (4.0, 0.0)], false),
    ];
    let r = auto_join(&lines, 0.01);
    assert_eq!(r.len(), 1);
    let xs: Vec<f64> = r[0].points.iter().map(|p| p.x).collect();
    assert_eq!(xs, vec![3.0, 4.0, 5.0, 6.0]);
}

#[test]
fn auto_join_closes_self_closing_and_keeps_closed() {
    let loop_path = poly(
        &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0), (0.0, 0.01)],
        false,
    );
    let closed = rect(10.0, 10.0, 11.0, 11.0);
    let r = auto_join(&[loop_path, closed.clone()], 0.05);
    assert_eq!(r.len(), 2);
    assert!(r[0].closed);
    assert_eq!(r[0].points.len(), 4);
    assert_eq!(r[1], closed);
}

#[test]
fn auto_join_picks_nearest_candidate_deterministically() {
    let lines = vec![
        poly(&[(0.0, 0.0), (1.0, 0.0)], false),
        poly(&[(1.04, 0.0), (1.0, 5.0)], false),
        poly(&[(1.01, 0.0), (1.0, -5.0)], false),
    ];
    let r = auto_join(&lines, 0.05);
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].points.last(), Some(&pt(1.0, -5.0)));
    assert_eq!(auto_join(&lines, 0.05), r);
}

#[test]
fn auto_join_degenerate_inputs() {
    assert!(auto_join(&[], 0.05).is_empty());
    let odd = vec![
        poly(&[], false),
        poly(&[(1.0, 1.0)], false),
        poly(&[(f64::NAN, 0.0), (1.0, 0.0)], false),
        poly(&[(0.0, 0.0), (1.0, 0.0)], false),
    ];
    assert_eq!(auto_join(&odd, f64::NAN).len(), 4);
    // Tolerancia 0: solo coincidencias exactas.
    let exact = vec![
        poly(&[(0.0, 0.0), (1.0, 0.0)], false),
        poly(&[(1.0, 0.0), (1.0, 1.0)], false),
    ];
    assert_eq!(auto_join(&exact, 0.0).len(), 1);
    let near = vec![
        poly(&[(0.0, 0.0), (1.0, 0.0)], false),
        poly(&[(1.001, 0.0), (1.0, 1.0)], false),
    ];
    assert_eq!(auto_join(&near, 0.0).len(), 2);
    // Dos trazos con los mismos extremos no pueden formar una cerrada de 2 puntos.
    let twins = vec![
        poly(&[(0.0, 0.0), (1.0, 0.0)], false),
        poly(&[(1.0, 0.0), (0.0, 0.0)], false),
    ];
    let r = auto_join(&twins, 0.01);
    assert_eq!(r.len(), 1);
    assert!(r[0].points.len() >= 2);
}

#[test]
fn close_paths_by_tolerance() {
    let near = poly(
        &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0), (0.0, 0.3)],
        false,
    );
    let far = poly(
        &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0), (0.0, 1.0)],
        false,
    );
    let exact = poly(
        &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0), (0.0, 0.0)],
        false,
    );
    let line = poly(&[(0.0, 0.0), (0.1, 0.0)], false);
    let r = close_paths(&[near, far.clone(), exact, line.clone()], 0.5);
    assert!(r[0].closed && r[0].points.len() == 5);
    assert_eq!(r[1], far);
    assert!(r[2].closed && r[2].points.len() == 4);
    assert_eq!(r[3], line);
    assert!(close_paths(&[], 0.5).is_empty());
}

// --- robustez ---

fn every_function(paths: &[Polyline]) {
    let other = [rect(0.0, 0.0, 2.0, 2.0)];
    for op in [
        BoolOp::Union,
        BoolOp::Difference,
        BoolOp::Intersection,
        BoolOp::Xor,
    ] {
        boolean(paths, &other, op);
        boolean(&other, paths, op);
    }
    weld(paths);
    for join in [Join::Round, Join::Bevel, Join::Miter { limit: 3.0 }] {
        for d in [-100.0, -0.5, 0.0, 0.5, 100.0] {
            offset(paths, d, join, 0.01);
        }
    }
    nesting_depth(paths);
    kerf_compensate(paths, 0.1);
    kerf_compensate(paths, -0.1);
    auto_join(paths, 0.05);
    close_paths(paths, 0.5);
    for p in paths {
        signed_area(&p.points);
        orientation(&p.points);
        remove_redundant_points(p, 1e-6);
        point_in_polygon(pt(0.5, 0.5), &p.points);
    }
}

#[test]
fn degenerate_inputs_never_panic() {
    let inf = f64::INFINITY;
    let cases: Vec<Vec<Polyline>> = vec![
        vec![],
        vec![poly(&[], true), poly(&[], false)],
        vec![poly(&[(0.0, 0.0)], true), poly(&[(1.0, 1.0)], false)],
        vec![poly(&[(0.0, 0.0), (1.0, 1.0)], true)],
        vec![poly(&[(0.0, 0.0), (0.0, 0.0), (0.0, 0.0)], true)],
        vec![poly(
            &[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (3.0, 0.0)],
            true,
        )],
        // autointersección (moño) y estrella
        vec![poly(
            &[(0.0, 0.0), (4.0, 4.0), (4.0, 0.0), (0.0, 4.0)],
            true,
        )],
        vec![poly(
            &[(0.0, 3.0), (4.0, 3.0), (0.5, 0.0), (2.0, 5.0), (3.5, 0.0)],
            true,
        )],
        // segmentos diminutos
        vec![poly(
            &[(0.0, 0.0), (1e-12, 0.0), (1e-12, 1e-12), (0.0, 1e-12)],
            true,
        )],
        vec![poly(
            &[(0.0, 0.0), (1e-7, 0.0), (1e-7, 1e-7), (0.0, 1e-7)],
            true,
        )],
        // duplicados y puntos no finitos o enormes
        vec![rect(0.0, 0.0, 1.0, 1.0), rect(0.0, 0.0, 1.0, 1.0)],
        vec![poly(&[(f64::NAN, 0.0), (1.0, 0.0), (1.0, 1.0)], true)],
        vec![poly(&[(inf, 0.0), (1.0, 0.0), (1.0, 1.0)], true)],
        vec![poly(&[(-1e300, 0.0), (1e300, 0.0), (1e300, 1e300)], true)],
        vec![poly(&[(0.0, 0.0), (1e8, 0.0), (1e8, 1e8)], true)],
        // coordenadas lejanas con detalle fino
        vec![rect(1e6, 1e6, 1e6 + 1e-3, 1e6 + 1e-3)],
        // el primer punto repetido al final de una cerrada
        vec![poly(
            &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 0.0)],
            true,
        )],
    ];
    for case in &cases {
        every_function(case);
    }
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }

    /// Polígono simple (estrellado respecto a su centro, ángulos crecientes).
    fn star(&mut self) -> Polyline {
        let (cx, cy) = (self.range(0.0, 40.0), self.range(0.0, 40.0));
        let n = 3 + (self.next() * 9.0) as usize;
        let points = (0..n)
            .map(|i| {
                let a = 2.0 * PI * (i as f64 + self.range(0.0, 0.8)) / n as f64;
                let r = self.range(1.0, 15.0);
                pt(cx + r * a.cos(), cy + r * a.sin())
            })
            .collect();
        Polyline {
            points,
            closed: true,
        }
    }

    /// Polígono con puntos al azar (casi siempre autointersecante).
    fn scribble(&mut self) -> Polyline {
        let n = 3 + (self.next() * 8.0) as usize;
        Polyline {
            points: (0..n)
                .map(|_| pt(self.range(0.0, 50.0), self.range(0.0, 50.0)))
                .collect(),
            closed: true,
        }
    }
}

#[test]
fn random_polygons_have_coherent_areas() {
    let mut rng = Lcg(0x5eed);
    for _ in 0..150 {
        let a = rng.star();
        let b = rng.star();
        let (sa, sb) = (signed_area(&a.points).abs(), signed_area(&b.points).abs());
        let (a, b) = ([a], [b]);
        let area = |op| region_area(&boolean(&a, &b, op));
        let (u, i, d, x) = (
            area(BoolOp::Union),
            area(BoolOp::Intersection),
            area(BoolOp::Difference),
            area(BoolOp::Xor),
        );
        let tol = 1e-4 * (sa + sb).max(1.0);
        assert_close(u + i, sa + sb, tol);
        assert_close(d, sa - i, tol);
        assert_close(x, u - i, tol);
        assert!(i >= -tol && i <= sa.min(sb) + tol);
        assert!(u >= sa.max(sb) - tol);
        // Weld de dos piezas = unión.
        assert_close(region_area(&weld(&[a[0].clone(), b[0].clone()])), u, tol);
    }
}

#[test]
fn random_offsets_grow_monotonically() {
    let mut rng = Lcg(42);
    for _ in 0..60 {
        let p = [rng.star()];
        let base = signed_area(&p[0].points).abs();
        let d = rng.range(0.05, 3.0);
        let out = offset(&p, d, Join::Round, 0.01);
        let inn = offset(&p, -d, Join::Round, 0.01);
        let a_out = region_area(&out);
        let a_in = region_area(&inn);
        assert!(a_out > base, "{a_out} <= {base}");
        assert!(a_in < base + 1e-6, "{a_in} >= {base}");
        // Con Miter el offset exterior de cualquier polígono es al menos el de Bevel.
        let bevel = region_area(&offset(&p, d, Join::Bevel, 0.01));
        let miter = region_area(&offset(&p, d, Join::Miter { limit: 4.0 }, 0.01));
        assert!(bevel <= miter + 1e-6, "{bevel} > {miter}");
        // Outset y luego inset de vuelta no pierde la forma original (cierre morfológico).
        if !out.is_empty() {
            let back = region_area(&offset(&out, -d, Join::Round, 0.01));
            assert!(
                back >= base - 1e-3 * base.max(1.0) - 0.05 * d * d,
                "{back} < {base}"
            );
        }
    }
}

#[test]
fn random_scribbles_never_panic_and_stay_bounded() {
    let mut rng = Lcg(7);
    for round in 0..60 {
        let count = 1 + (round % 6);
        let paths: Vec<Polyline> = (0..count)
            .map(|_| {
                if rng.next() < 0.5 {
                    rng.scribble()
                } else {
                    rng.star()
                }
            })
            .collect();
        let other: Vec<Polyline> = (0..2).map(|_| rng.scribble()).collect();
        let w = weld(&paths);
        // Todo punto soldado cae en la caja de las entradas.
        for p in &w {
            assert!(p.closed && p.points.len() >= 3);
            for q in &p.points {
                assert!(q.x > -20.0 && q.x < 70.0 && q.y > -20.0 && q.y < 70.0);
            }
        }
        for op in [
            BoolOp::Union,
            BoolOp::Difference,
            BoolOp::Intersection,
            BoolOp::Xor,
        ] {
            let r = boolean(&paths, &other, op);
            assert!(region_area(&r).is_finite());
        }
        let wa = region_area(&w);
        assert!(wa >= -1e-6);
        let off = offset(&paths, 0.7, Join::Miter { limit: 3.0 }, 0.01);
        assert!(region_area(&off) >= wa - 1e-3);
        kerf_compensate(&paths, 0.2);
        let depth = nesting_depth(&paths);
        assert_eq!(depth.len(), paths.len());
        let mut opened = paths.clone();
        for p in &mut opened {
            p.closed = false;
        }
        auto_join(&opened, 1.0);
    }
}

#[test]
fn random_segments_join_into_fewer_paths() {
    // Un anillo de N puntos partido en segmentos sueltos y desordenados, algunos invertidos.
    let mut rng = Lcg(99);
    let n = 40;
    let ring: Vec<Point> = (0..n)
        .map(|i| {
            let a = 2.0 * PI * i as f64 / n as f64;
            pt(20.0 * a.cos(), 20.0 * a.sin())
        })
        .collect();
    let mut segs: Vec<Polyline> = (0..n)
        .map(|i| {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            let jitter = pt(rng.range(-0.01, 0.01), rng.range(-0.01, 0.01));
            let b = pt(b.x + jitter.x, b.y + jitter.y);
            if rng.next() < 0.5 {
                Polyline {
                    points: vec![a, b],
                    closed: false,
                }
            } else {
                Polyline {
                    points: vec![b, a],
                    closed: false,
                }
            }
        })
        .collect();
    // Baraja con el mismo LCG.
    for i in (1..segs.len()).rev() {
        segs.swap(i, (rng.next() * (i + 1) as f64) as usize);
    }
    let r = auto_join(&segs, 0.05);
    assert_eq!(r.len(), 1);
    assert!(r[0].closed);
    assert_eq!(r[0].points.len(), n);
    let expected = n as f64 / 2.0 * 400.0 * (2.0 * PI / n as f64).sin();
    assert_close(signed_area(&r[0].points).abs(), expected, 0.5);
}

#[test]
fn many_polygons_weld_is_fast_and_consistent() {
    let mut rng = Lcg(2024);
    let paths: Vec<Polyline> = (0..300).map(|_| rng.star()).collect();
    let w = weld(&paths);
    let union_area = region_area(&w);
    let sum: f64 = paths.iter().map(|p| signed_area(&p.points).abs()).sum();
    assert!(union_area > 0.0 && union_area <= sum + 1e-6);
    // Soldar dos veces no cambia nada.
    assert_close(region_area(&weld(&w)), union_area, 1e-6 * union_area);
    // Offset + kerf sobre el resultado no entran en bucles.
    assert!(!offset(&w, 1.0, Join::Round, 0.05).is_empty());
    kerf_compensate(&w, 0.1);
}
