//! Pruebas de D60: las esquinas guardadas al aplicar un chaflán o un redondeo y el modo
//! «Revertir».

use super::*;
use tlaku_core::Rgb;

fn square(id: &str, side: f64) -> DesignObject {
    let poly = Polyline {
        points: vec![
            Point::new(0.0, 0.0),
            Point::new(side, 0.0),
            Point::new(side, side),
            Point::new(0.0, side),
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

fn corners(o: &DesignObject) -> Vec<tlaku_doc::CornerMark> {
    match &o.kind {
        ObjectKind::Vector(v) => v.corners.clone(),
        _ => Vec::new(),
    }
}

fn area(o: &DesignObject) -> f64 {
    o.polylines_with(0.001)
        .iter()
        .map(|p| signed_area(&p.points))
        .sum()
}

fn apply(o: &DesignObject, kind: FilletKind, radius: f64, at: [f64; 2]) -> DesignObject {
    let p = ShapeParams {
        radius: Some(radius),
        style: Some(kind),
        nodes: Some(vec![at]),
        ..ShapeParams::default()
    };
    run(ShapeOp::Fillet, std::slice::from_ref(o), &p)
        .unwrap()
        .objects
        .remove(0)
}

fn revert(o: &DesignObject, at: [f64; 2]) -> ShapeOpResult {
    let p = ShapeParams {
        revert: true,
        point: Some(at),
        ..ShapeParams::default()
    };
    run(ShapeOp::Fillet, std::slice::from_ref(o), &p).unwrap()
}

#[test]
fn el_chaflan_guarda_la_punta_original() {
    let c = apply(&square("o", 10.0), FilletKind::Chamfer, 2.0, [10.0, 10.0]);
    let marks = corners(&c);
    assert_eq!(marks.len(), 1);
    assert_eq!(marks[0].vertex, [10.0, 10.0]);
    let mut ends = [marks[0].from, marks[0].to];
    ends.sort_by(|a, b| a[0].total_cmp(&b[0]));
    assert_eq!(ends, [[8.0, 10.0], [10.0, 8.0]]);
    assert!((area(&c) - 98.0).abs() < 1e-9);
}

#[test]
fn revertir_un_chaflan_devuelve_la_punta() {
    let c = apply(&square("o", 10.0), FilletKind::Chamfer, 2.0, [10.0, 10.0]);
    // Sin la marca, la geometría no lo distingue de un lado recto.
    let mut sin = c.clone();
    if let ObjectKind::Vector(v) = &mut sin.kind {
        v.corners.clear();
    }
    assert_eq!(revert(&sin, [9.0, 9.0]).stats.count, 0);
    // Con la marca, vuelve a punta en un solo objeto y la marca se gasta.
    let r = revert(&c, [9.0, 9.0]);
    assert_eq!(r.stats.count, 1);
    assert_eq!(r.objects.len(), 1);
    assert!((area(&r.objects[0]) - 100.0).abs() < 1e-9);
    assert!(corners(&r.objects[0]).is_empty());
}

#[test]
fn revertir_un_redondeo_marcado_y_uno_sin_marca() {
    let c = apply(&square("o", 10.0), FilletKind::Round, 2.0, [10.0, 10.0]);
    assert_eq!(corners(&c).len(), 1);
    let r = revert(&c, [9.5, 9.5]);
    assert_eq!(r.stats.count, 1);
    assert!((area(&r.objects[0]) - 100.0).abs() < 1e-6);
    // Un redondeo sin marca (archivo viejo) sigue revirtiéndose por geometría.
    let mut sin = c.clone();
    if let ObjectKind::Vector(v) = &mut sin.kind {
        v.corners.clear();
    }
    let r = revert(&sin, [9.5, 9.5]);
    assert_eq!(r.stats.count, 1);
    assert!((area(&r.objects[0]) - 100.0).abs() < 1e-6);
}

#[test]
fn con_dos_esquinas_se_revierte_solo_la_del_clic() {
    let a = apply(&square("o", 10.0), FilletKind::Chamfer, 2.0, [10.0, 10.0]);
    let b = apply(&a, FilletKind::Chamfer, 2.0, [0.0, 0.0]);
    assert_eq!(corners(&b).len(), 2);
    assert!((area(&b) - 96.0).abs() < 1e-9);
    let r = revert(&b, [9.0, 9.0]);
    assert_eq!(r.stats.count, 1);
    assert!((area(&r.objects[0]) - 98.0).abs() < 1e-9);
    // La otra marca sigue valiendo y se puede revertir después.
    assert_eq!(corners(&r.objects[0]).len(), 1);
    let r2 = revert(&r.objects[0], [1.0, 1.0]);
    assert_eq!(r2.stats.count, 1);
    assert!((area(&r2.objects[0]) - 100.0).abs() < 1e-9);
}

#[test]
fn una_marca_con_los_nodos_movidos_no_revierte_y_se_descarta() {
    let mut c = apply(&square("o", 10.0), FilletKind::Chamfer, 2.0, [10.0, 10.0]);
    // El usuario mueve un nodo del chaflán sin tocar `corners`.
    if let ObjectKind::Vector(v) = &mut c.kind {
        v.paths = vec![BezPath::from(&Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(10.0, 0.0),
                Point::new(10.0, 7.0),
                Point::new(8.0, 10.0),
                Point::new(0.0, 10.0),
            ],
            closed: true,
        })];
    }
    assert_eq!(revert(&c, [9.0, 9.0]).stats.count, 0);
    // Otra operación sobre el objeto no arrastra la marca caducada.
    let again = apply(&c, FilletKind::Chamfer, 1.0, [0.0, 0.0]);
    assert_eq!(corners(&again).len(), 1);
    assert_eq!(corners(&again)[0].vertex, [0.0, 0.0]);
}

#[test]
fn las_marcas_siguen_al_objeto_movido() {
    let mut c = apply(&square("o", 10.0), FilletKind::Chamfer, 2.0, [10.0, 10.0]);
    // Se mueve el objeto: cambia el `transform`, no las curvas locales.
    c.transform = [1.0, 0.0, 0.0, 1.0, 50.0, 20.0];
    // Hasta aquí las marcas locales son las de la geometría local (el objeto se creó con identidad).
    let r = revert(&c, [59.0, 29.0]);
    assert_eq!(r.stats.count, 1);
    assert!((area(&r.objects[0]) - 100.0).abs() < 1e-9);
}

#[test]
fn las_marcas_sobreviven_a_guardar_y_abrir() {
    let c = apply(&square("o", 10.0), FilletKind::Chamfer, 2.0, [10.0, 10.0]);
    let json = serde_json::to_string(&c).unwrap();
    assert!(json.contains("\"corners\""));
    let back: DesignObject = serde_json::from_str(&json).unwrap();
    assert_eq!(corners(&back), corners(&c));
    // Un documento sin el campo (anterior a D60) se lee con la lista vacía y no la escribe.
    let plain = square("p", 5.0);
    let json = serde_json::to_string(&plain).unwrap();
    assert!(!json.contains("corners"));
    let back: DesignObject = serde_json::from_str(&json).unwrap();
    assert!(corners(&back).is_empty());
}

#[test]
fn rectas_sin_marca_cerca_no_se_tocan() {
    let c = apply(&square("o", 10.0), FilletKind::Chamfer, 2.0, [10.0, 10.0]);
    // Lejos de la esquina marcada (a más de 2 mm): no cambia nada.
    assert_eq!(revert(&c, [2.0, 2.0]).stats.count, 0);
}
