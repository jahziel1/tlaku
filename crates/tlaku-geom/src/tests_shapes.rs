//! Pruebas de las operaciones de forma: recortar, separar, optimizar, redondear, cortar y ranuras.

use super::*;
use std::f64::consts::PI;
use tlaku_core::bezpath::{BezPath, PathSeg};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn poly(pts: &[(f64, f64)], closed: bool) -> Polyline {
    Polyline {
        points: pts.iter().map(|&(x, y)| pt(x, y)).collect(),
        closed,
    }
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Polyline {
    poly(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], true)
}

fn length(p: &Polyline) -> f64 {
    let n = p.points.len();
    let segs = if p.closed { n } else { n - 1 };
    (0..segs)
        .map(|i| p.points[i].dist(p.points[(i + 1) % n]))
        .sum()
}

fn near(a: Point, x: f64, y: f64) -> bool {
    a.dist(pt(x, y)) < 1e-6
}

fn area_of(path: &BezPath) -> f64 {
    path.flatten(0.0005)
        .iter()
        .map(|p| signed_area(&p.points))
        .sum()
}

// --- recortar ---

#[test]
fn trim_en_cruz_borra_el_tramo_hasta_la_interseccion() {
    let paths = vec![
        poly(&[(0.0, 0.0), (10.0, 0.0)], false),
        poly(&[(5.0, -5.0), (5.0, 5.0)], false),
    ];
    let r = trim_at(&paths, pt(2.0, 0.0), None).unwrap();
    assert_eq!(r.path_index, 0);
    assert!(!r.whole);
    assert!((length(&r.removed) - 5.0).abs() < 1e-9);
    assert_eq!(r.remaining.len(), 1);
    assert!(near(r.remaining[0].points[0], 5.0, 0.0) && near(r.remaining[0].points[1], 10.0, 0.0));
    // Por el otro lado queda el tramo inicial.
    let r = trim_at(&paths, pt(8.0, 0.0), None).unwrap();
    assert!(near(r.remaining[0].points[1], 5.0, 0.0));
    // Sobre la vertical: se borra la mitad de arriba.
    let r = trim_at(&paths, pt(5.0, 3.0), None).unwrap();
    assert_eq!(r.path_index, 1);
    assert!((length(&r.removed) - 5.0).abs() < 1e-9);
}

#[test]
fn trim_de_una_linea_aislada_la_borra_entera() {
    let paths = vec![
        poly(&[(0.0, 0.0), (10.0, 0.0)], false),
        poly(&[(0.0, 5.0), (10.0, 5.0)], false),
    ];
    let r = trim_at(&paths, pt(4.0, 5.2), None).unwrap();
    assert!(r.whole && r.remaining.is_empty());
    assert_eq!(r.path_index, 1);
}

#[test]
fn trim_en_cerrada_deja_el_resto_del_contorno() {
    let paths = vec![
        rect(0.0, 0.0, 10.0, 10.0),
        poly(&[(-5.0, 5.0), (15.0, 5.0)], false),
    ];
    let r = trim_at(&paths, pt(5.0, 0.0), None).unwrap();
    assert!(
        (length(&r.removed) - 20.0).abs() < 1e-9,
        "{}",
        length(&r.removed)
    );
    assert_eq!(r.remaining.len(), 1);
    assert!((length(&r.remaining[0]) - 20.0).abs() < 1e-9);
    assert!(!r.remaining[0].closed);
    // Un clic arriba borra el tramo de arriba.
    let r = trim_at(&paths, pt(5.0, 10.0), None).unwrap();
    assert!(r.removed.points.iter().all(|p| p.y >= 5.0 - 1e-9));
}

#[test]
fn trim_cuenta_el_extremo_que_toca_otra_linea() {
    let paths = vec![
        poly(&[(0.0, 0.0), (10.0, 0.0)], false),
        poly(&[(5.0, 0.0), (5.0, 5.0)], false),
    ];
    let r = trim_at(&paths, pt(2.0, 0.0), None).unwrap();
    assert!((length(&r.removed) - 5.0).abs() < 1e-9);
}

#[test]
fn trim_respeta_la_distancia_maxima() {
    let paths = vec![poly(&[(0.0, 0.0), (10.0, 0.0)], false)];
    assert!(trim_at(&paths, pt(5.0, 3.0), Some(1.0)).is_none());
    assert!(trim_at(&paths, pt(5.0, 0.5), Some(1.0)).is_some());
    assert!(trim_at(&[], pt(0.0, 0.0), None).is_none());
    assert!(trim_at(&paths, pt(f64::NAN, 0.0), None).is_none());
}

#[test]
fn trim_con_autointersecion() {
    // Un lazo: la línea se cruza consigo misma en (5,5).
    let lazo = poly(&[(0.0, 0.0), (10.0, 10.0), (10.0, 0.0), (0.0, 10.0)], false);
    let r = trim_at(&[lazo], pt(2.0, 2.0), None).unwrap();
    // Se borra desde el inicio hasta el cruce en (5,5).
    assert!(
        near(r.removed.points[0], 0.0, 0.0) && near(*r.removed.points.last().unwrap(), 5.0, 5.0)
    );
}

// --- separar ---

#[test]
fn split_subpaths_separa_y_conserva_curvas() {
    let path = BezPath(vec![
        PathSeg::MoveTo { to: pt(0.0, 0.0) },
        PathSeg::CubicTo {
            ctrl1: pt(1.0, 1.0),
            ctrl2: pt(2.0, 1.0),
            to: pt(3.0, 0.0),
        },
        PathSeg::Close,
        PathSeg::MoveTo { to: pt(10.0, 10.0) },
        PathSeg::LineTo { to: pt(12.0, 10.0) },
        PathSeg::MoveTo { to: pt(50.0, 50.0) },
    ]);
    let parts = split_subpaths(&path);
    assert_eq!(parts.len(), 2, "el MoveTo solitario se descarta");
    assert!(matches!(parts[0].0[1], PathSeg::CubicTo { .. }));
    assert!(matches!(parts[0].0.last(), Some(PathSeg::Close)));
    assert_eq!(parts[1].0.len(), 2);
}

#[test]
fn break_groups_con_y_sin_huecos() {
    let paths = vec![
        rect(0.0, 0.0, 20.0, 20.0),
        rect(5.0, 5.0, 15.0, 15.0),
        rect(8.0, 8.0, 12.0, 12.0),
        rect(30.0, 0.0, 40.0, 10.0),
        poly(&[(50.0, 0.0), (60.0, 0.0)], false),
    ];
    assert_eq!(break_groups(&paths, false).len(), 5);
    let g = break_groups(&paths, true);
    // Marco con su hueco; la isla va aparte; el cuadrado suelto; la línea abierta.
    assert_eq!(g, vec![vec![0, 1], vec![2], vec![3], vec![4]]);
}

// --- optimizar ---

fn circle(n: usize, r: f64) -> Polyline {
    Polyline {
        points: (0..n)
            .map(|i| {
                let a = 2.0 * PI * i as f64 / n as f64;
                pt(r * a.cos(), r * a.sin())
            })
            .collect(),
        closed: true,
    }
}

#[test]
fn optimizar_quita_puntos_colineales_de_un_cuadrado_denso() {
    let mut pts = Vec::new();
    for i in 0..100 {
        pts.push((i as f64 / 10.0, 0.0));
    }
    for i in 0..100 {
        pts.push((10.0, i as f64 / 10.0));
    }
    for i in 0..100 {
        pts.push((10.0 - i as f64 / 10.0, 10.0));
    }
    for i in 0..100 {
        pts.push((0.0, 10.0 - i as f64 / 10.0));
    }
    let p = poly(&pts, true);
    let out = optimize_polyline(
        &p,
        &OptimizeOptions {
            tolerance: 0.01,
            smooth: true,
            corner_angle_deg: 35.0,
        },
    );
    assert_eq!(node_count(&out), 4);
    assert!((area_of(&out) - 100.0).abs() < 1e-6);
}

#[test]
fn optimizar_convierte_un_circulo_denso_en_pocas_curvas() {
    let c = circle(400, 10.0);
    let lines = optimize_polyline(
        &c,
        &OptimizeOptions {
            tolerance: 0.01,
            smooth: false,
            corner_angle_deg: 35.0,
        },
    );
    let curves = optimize_polyline(
        &c,
        &OptimizeOptions {
            tolerance: 0.01,
            smooth: true,
            corner_angle_deg: 35.0,
        },
    );
    assert!(node_count(&lines) < 400 && node_count(&lines) > 20);
    let n = node_count(&curves);
    assert!(n <= 12, "{n} nodos");
    assert!(
        curves
            .0
            .iter()
            .any(|s| matches!(s, PathSeg::CubicTo { .. }))
    );
    // El error radial queda dentro de la tolerancia.
    for q in curves.flatten(0.001).iter().flat_map(|p| &p.points) {
        assert!(
            (q.dist(pt(0.0, 0.0)) - 10.0).abs() < 0.02,
            "{}",
            q.dist(pt(0.0, 0.0))
        );
    }
}

#[test]
fn optimizar_con_tolerancia_cero_solo_limpia() {
    let p = poly(
        &[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (2.0, 0.0), (2.0, 2.0)],
        false,
    );
    let out = optimize_polyline(
        &p,
        &OptimizeOptions {
            tolerance: 0.0,
            smooth: false,
            corner_angle_deg: 35.0,
        },
    );
    assert_eq!(node_count(&out), 3);
}

#[test]
fn optimizar_no_entra_en_panico_con_entradas_raras() {
    let o = OptimizeOptions::default();
    let _ = optimize_polyline(&poly(&[], false), &o);
    let _ = optimize_polyline(&poly(&[(0.0, 0.0)], true), &o);
    let _ = optimize_polyline(&poly(&[(f64::NAN, 0.0), (1.0, 1.0), (2.0, 0.0)], false), &o);
}

#[test]
fn node_count_no_repite_el_cierre() {
    assert_eq!(node_count(&BezPath::from(&rect(0.0, 0.0, 1.0, 1.0))), 4);
    assert_eq!(
        node_count(&BezPath::from(&poly(&[(0.0, 0.0), (1.0, 0.0)], false))),
        2
    );
}

// --- redondear ---

fn square_path() -> BezPath {
    BezPath::from(&rect(0.0, 0.0, 10.0, 10.0))
}

fn opts(r: f64, style: FilletStyle) -> FilletOptions {
    FilletOptions { radius: r, style }
}

#[test]
fn fillet_redondea_las_cuatro_esquinas_con_arcos_exactos() {
    let r = fillet_path(&square_path(), &opts(2.0, FilletStyle::Round), None);
    assert_eq!((r.rounded, r.clamped, r.no_space), (4, 0, 0));
    let expected = 100.0 - (4.0 - PI) * 4.0;
    assert!(
        (area_of(&r.path) - expected).abs() < 0.01,
        "{}",
        area_of(&r.path)
    );
    assert_eq!(node_count(&r.path), 8);
}

#[test]
fn fillet_con_radio_grande_reduce_en_vez_de_fallar() {
    let r = fillet_path(&square_path(), &opts(8.0, FilletStyle::Round), None);
    assert_eq!(r.rounded, 4);
    assert_eq!(r.clamped, 4);
    let expected = 100.0 - (4.0 - PI) * 25.0;
    assert!(
        (area_of(&r.path) - expected).abs() < 0.05,
        "{}",
        area_of(&r.path)
    );
}

#[test]
fn fillet_solo_en_los_nodos_elegidos() {
    let r = fillet_path(
        &square_path(),
        &opts(2.0, FilletStyle::Round),
        Some(&[pt(10.0, 10.0)]),
    );
    assert_eq!(r.rounded, 1);
    let expected = 100.0 - (4.0 - PI);
    assert!((area_of(&r.path) - expected).abs() < 0.01);
    let ninguno = fillet_path(
        &square_path(),
        &opts(2.0, FilletStyle::Round),
        Some(&[pt(3.0, 3.0)]),
    );
    assert_eq!(ninguno.rounded, 0);
    assert_eq!(ninguno.path, square_path());
}

#[test]
fn fillet_chaflan_y_mordisco() {
    let ch = fillet_path(&square_path(), &opts(2.0, FilletStyle::Chamfer), None);
    assert!((area_of(&ch.path) - 92.0).abs() < 1e-9);
    let bite = fillet_path(&square_path(), &opts(2.0, FilletStyle::Bite), None);
    assert!(
        (area_of(&bite.path) - (100.0 - 4.0 * PI)).abs() < 0.01,
        "{}",
        area_of(&bite.path)
    );
}

#[test]
fn fillet_deja_vivas_las_esquinas_sin_espacio() {
    // Un triángulo cuyo cateto corto no deja sitio a un radio grande en el vértice vecino.
    let tri = BezPath::from(&poly(&[(0.0, 0.0), (10.0, 0.0), (0.0, 1.0)], true));
    let r = fillet_path(&tri, &opts(3.0, FilletStyle::Round), Some(&[pt(10.0, 0.0)]));
    assert_eq!(r.rounded + r.no_space, 1);
}

#[test]
fn fillet_conserva_curvas_y_abiertas() {
    let circle = BezPath(vec![
        PathSeg::MoveTo { to: pt(1.0, 0.0) },
        PathSeg::CubicTo {
            ctrl1: pt(1.0, 0.55),
            ctrl2: pt(0.55, 1.0),
            to: pt(0.0, 1.0),
        },
        PathSeg::Close,
    ]);
    let r = fillet_path(&circle, &opts(0.2, FilletStyle::Round), None);
    assert_eq!(r.rounded, 0);
    assert_eq!(r.path, circle);
    // Una "L" abierta: solo se redondea el vértice del medio.
    let ele = BezPath::from(&poly(&[(0.0, 10.0), (0.0, 0.0), (10.0, 0.0)], false));
    let r = fillet_path(&ele, &opts(2.0, FilletStyle::Round), None);
    assert_eq!(r.rounded, 1);
    assert!(!r.path.0.iter().any(|s| matches!(s, PathSeg::Close)));
    assert!(fillet_path(&ele, &opts(-1.0, FilletStyle::Round), None).rounded == 0);
}

// --- cortar formas ---

#[test]
fn cut_closed_parte_en_dentro_y_fuera() {
    let cut = cut_closed(
        &[rect(0.0, 0.0, 10.0, 10.0)],
        &[rect(5.0, -5.0, 15.0, 15.0)],
    );
    let area = |v: &[Polyline]| v.iter().map(|p| signed_area(&p.points)).sum::<f64>();
    assert!((area(&cut.inside) - 50.0).abs() < 1e-6);
    assert!((area(&cut.outside) - 50.0).abs() < 1e-6);
}

#[test]
fn cut_open_reparte_los_tramos_de_una_linea() {
    let cut = cut_open(
        &[poly(&[(0.0, 0.0), (10.0, 0.0)], false)],
        &[rect(4.0, -1.0, 6.0, 1.0)],
    );
    assert_eq!(cut.inside.len(), 1);
    assert!((length(&cut.inside[0]) - 2.0).abs() < 1e-9);
    assert_eq!(cut.outside.len(), 2);
    assert!((cut.outside.iter().map(length).sum::<f64>() - 8.0).abs() < 1e-9);
}

#[test]
fn cut_open_de_una_cerrada_no_añade_la_linea_de_corte() {
    let cut = cut_open(
        &[rect(0.0, 0.0, 10.0, 10.0)],
        &[rect(5.0, -5.0, 15.0, 15.0)],
    );
    assert_eq!(cut.inside.len(), 1);
    assert_eq!(cut.outside.len(), 1);
    assert!((length(&cut.inside[0]) - 20.0).abs() < 1e-9);
    assert!((length(&cut.outside[0]) - 20.0).abs() < 1e-9);
    // Sin cruce: la cerrada se reparte entera y sigue cerrada.
    let lejos = cut_open(&[rect(0.0, 0.0, 1.0, 1.0)], &[rect(5.0, 5.0, 6.0, 6.0)]);
    assert!(lejos.inside.is_empty() && lejos.outside[0].closed);
}

// --- ranuras ---

fn slot_opts(from: f64, to: f64, anchor: SlotAnchor) -> SlotOptions {
    SlotOptions {
        from_width: from,
        tolerance: 0.1,
        to_width: to,
        anchor,
        notches: true,
        holes: true,
        tabs: false,
    }
}

/// Pieza de 40 x 20 con una muesca de 3 mm de ancho y 5 de fondo en el borde de abajo.
fn piece() -> Polyline {
    poly(
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
    )
}

fn xs_at_bottom(p: &Polyline) -> Vec<f64> {
    p.points
        .iter()
        .filter(|q| q.y.abs() < 1e-9)
        .map(|q| q.x)
        .collect()
}

#[test]
fn ranura_en_el_borde_cambia_de_ancho_desde_el_centro() {
    let (out, rep) = resize_slots(&[piece()], &slot_opts(3.0, 3.5, SlotAnchor::Center));
    assert_eq!((rep.found, rep.resized, rep.blocked), (1, 1, 0));
    assert!((signed_area(&out[0].points) - (800.0 - 17.5)).abs() < 1e-6);
    let xs = xs_at_bottom(&out[0]);
    assert!(
        xs.iter().any(|x| (x - 9.75).abs() < 1e-9) && xs.iter().any(|x| (x - 13.25).abs() < 1e-9)
    );
}

#[test]
fn ranura_respeta_el_ancla() {
    let (inicio, _) = resize_slots(&[piece()], &slot_opts(3.0, 4.0, SlotAnchor::Start));
    let xs = xs_at_bottom(&inicio[0]);
    assert!(
        xs.iter().any(|x| (x - 10.0).abs() < 1e-9) && xs.iter().any(|x| (x - 14.0).abs() < 1e-9)
    );
    let (fin, _) = resize_slots(&[piece()], &slot_opts(3.0, 4.0, SlotAnchor::End));
    let xs = xs_at_bottom(&fin[0]);
    assert!(
        xs.iter().any(|x| (x - 9.0).abs() < 1e-9) && xs.iter().any(|x| (x - 13.0).abs() < 1e-9)
    );
}

#[test]
fn ranura_de_otro_ancho_no_se_toca() {
    let (out, rep) = resize_slots(&[piece()], &slot_opts(5.0, 6.0, SlotAnchor::Center));
    assert_eq!(rep, SlotReport::default());
    assert_eq!(out[0], piece());
}

#[test]
fn ranura_que_invertiria_un_borde_se_bloquea() {
    let (out, rep) = resize_slots(&[piece()], &slot_opts(3.0, 40.0, SlotAnchor::Center));
    assert_eq!((rep.found, rep.resized, rep.blocked), (1, 0, 1));
    assert_eq!(out[0], piece());
}

#[test]
fn agujero_rectangular_cambia_el_lado_que_coincide() {
    let marco = rect(0.0, 0.0, 50.0, 50.0);
    let hueco = rect(10.0, 10.0, 30.0, 13.0);
    let (out, rep) = resize_slots(
        &[marco.clone(), hueco],
        &slot_opts(3.0, 4.0, SlotAnchor::Center),
    );
    assert_eq!((rep.found, rep.resized), (1, 1));
    assert_eq!(out[0], marco);
    assert!((signed_area(&out[1].points) - 80.0).abs() < 1e-6);
    let ys: Vec<f64> = out[1].points.iter().map(|p| p.y).collect();
    assert!(
        ys.iter().any(|y| (y - 9.5).abs() < 1e-9) && ys.iter().any(|y| (y - 13.5).abs() < 1e-9)
    );
}

#[test]
fn un_rectangulo_exterior_no_es_una_ranura() {
    let (out, rep) = resize_slots(
        &[rect(0.0, 0.0, 20.0, 3.0)],
        &slot_opts(3.0, 4.0, SlotAnchor::Center),
    );
    assert_eq!(rep, SlotReport::default());
    assert_eq!(out[0], rect(0.0, 0.0, 20.0, 3.0));
}

#[test]
fn slot_widths_lista_los_anchos_detectados() {
    let marco = rect(100.0, 100.0, 150.0, 150.0);
    let w = slot_widths(
        &[piece(), marco, rect(110.0, 110.0, 130.0, 113.0)],
        true,
        true,
        true,
    );
    // La muesca de 3 y el agujero de 3 x 20.
    assert_eq!(w, vec![(3.0, 2), (20.0, 1)]);
}

#[test]
fn ranuras_con_parametros_invalidos_no_cambian_nada() {
    let (out, rep) = resize_slots(&[piece()], &slot_opts(3.0, f64::NAN, SlotAnchor::Center));
    assert_eq!(rep, SlotReport::default());
    assert_eq!(out[0], piece());
}
