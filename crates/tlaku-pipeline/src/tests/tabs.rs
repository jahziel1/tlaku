//! Puentes (tabs) automáticos y su relación con el kerf.

use super::*;
use tlaku_core::bezpath::BezPath;
use tlaku_core::toolpath::{Move, ToolpathBlock};
use tlaku_core::{Point, Polyline};
use tlaku_doc::{DesignObject, JobOptions, LayerSettings, ObjectKind, VectorObject};

/// (largo cortado con potencia, largo recorrido con el láser apagado dentro de cortes) del bloque.
fn lengths(block: &ToolpathBlock) -> (f64, f64) {
    let mut at = Point::new(0.0, 0.0);
    let (mut on, mut off) = (0.0, 0.0);
    for mv in &block.moves {
        let to = mv.to();
        match *mv {
            Move::Cut { power_pct, .. } if power_pct > 0.0 => on += at.dist(to),
            Move::Cut { .. } => off += at.dist(to),
            Move::Travel { .. } | Move::Dwell { .. } => {}
        }
        at = to;
    }
    (on, off)
}

#[test]
fn tabs_leave_gaps_crossed_with_the_laser_off() {
    let d = doc(
        vec![rect_object("a", RED, 10.0, 10.0, 40.0, 40.0)],
        vec![tabbed(line_layer(RED))],
    );
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    let (on, off) = lengths(&job.toolpath.blocks[0]);
    // Perímetro 160 mm con 4 huecos de 2 mm.
    approx(on, 152.0);
    approx(off, 8.0);
    assert_eq!(job.layers[0].tabs, 4);
    assert!(job.preview.layers[0].summary.contains("4 puentes"));
    // La vista previa trae los huecos como movimientos con potencia 0 dentro del corte.
    let gaps = job
        .preview
        .moves
        .chunks(4)
        .filter(|m| m[0] == 1.0 && m[3] == 0.0)
        .count();
    assert_eq!(gaps, 4);
    // Y el G-code, como G1 con S0.
    let g = gcode(&job, &device());
    assert_eq!(
        g.lines()
            .filter(|l| l.starts_with("G1") && l.contains("S0"))
            .count(),
        4
    );
    assert!(
        job.preview.warnings.is_empty(),
        "{:?}",
        job.preview.warnings
    );
}

#[test]
fn tabs_do_not_change_the_planned_order_or_touch_open_and_inner_shapes() {
    let open = DesignObject {
        kind: ObjectKind::Vector(VectorObject::new(vec![BezPath::from(&Polyline {
            points: vec![Point::new(60.0, 10.0), Point::new(90.0, 10.0)],
            closed: false,
        })])),
        ..rect_object("open", RED, 0.0, 0.0, 1.0, 1.0)
    };
    let objects = vec![
        rect_object("outer", RED, 10.0, 10.0, 40.0, 40.0),
        rect_object("hole", RED, 20.0, 20.0, 10.0, 10.0),
        open,
    ];
    let plain = compute(
        &doc(objects.clone(), vec![line_layer(RED)]),
        &device(),
        &JobOptions::default(),
    )
    .unwrap();
    let with = compute(
        &doc(objects.clone(), vec![tabbed(line_layer(RED))]),
        &device(),
        &JobOptions::default(),
    )
    .unwrap();
    let order = |j: &ComputedJob| -> Vec<usize> {
        j.layers[0]
            .plan
            .as_ref()
            .unwrap()
            .steps
            .iter()
            .map(|s| s.item)
            .collect()
    };
    assert_eq!(order(&plain), order(&with));
    // Solo el contorno exterior lleva puentes: el agujero y la línea abierta se cortan enteros.
    assert_eq!(with.layers[0].tabs, 4);
    let (_, off) = lengths(&with.toolpath.blocks[0]);
    approx(off, 8.0);
    // Con skip_inner = false, el agujero también.
    let mut all = tabbed(line_layer(RED));
    all.tabs.skip_inner = false;
    let every = compute(&doc(objects, vec![all]), &device(), &JobOptions::default()).unwrap();
    assert_eq!(every.layers[0].tabs, 8);
}

#[test]
fn tabs_go_after_the_kerf() {
    let mut l = tabbed(line_layer(RED));
    l.kerf = 1.0;
    let d = doc(vec![rect_object("a", RED, 10.0, 10.0, 40.0, 40.0)], vec![l]);
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    // El contorno compensado mide 42x42 (perímetro 168, con esquinas redondeadas un poco menos).
    let b = job.layers[0].bounds.unwrap();
    approx(b.max.x - b.min.x, 42.0);
    assert_eq!(job.layers[0].tabs, 4);
}

#[test]
fn tabs_that_do_not_fit_warn_and_bad_values_fail() {
    let small = doc(
        vec![
            rect_object("big", RED, 0.0, 0.0, 40.0, 40.0),
            rect_object("tiny", RED, 60.0, 0.0, 3.0, 3.0),
        ],
        vec![tabbed(line_layer(RED))],
    );
    let job = compute(&small, &device(), &JobOptions::default()).unwrap();
    // El cuadrado de 3 mm (perímetro 12) admite como mucho 3 puentes de 2 mm: 4 + 3.
    assert_eq!(job.layers[0].tabs, 7);
    let w = job.preview.warnings.join("\n");
    assert!(
        w.contains("#ff0000") && w.contains("no caben") && w.contains("12.00 mm"),
        "{w}"
    );

    let mut bad = tabbed(line_layer(RED));
    bad.tabs.width = 0.0;
    let e = compute(
        &doc(vec![rect_object("a", RED, 0.0, 0.0, 9.0, 9.0)], vec![bad]),
        &device(),
        &JobOptions::default(),
    )
    .unwrap_err();
    assert!(e.contains("capa #ff0000") && e.contains("puentes"), "{e}");

    // Sin ninguna forma cerrada donde ponerlos, se avisa.
    let open_only = DesignObject {
        kind: ObjectKind::Vector(VectorObject::new(vec![BezPath::from(&Polyline {
            points: vec![Point::new(0.0, 0.0), Point::new(9.0, 0.0)],
            closed: false,
        })])),
        ..rect_object("o", RED, 0.0, 0.0, 1.0, 1.0)
    };
    let job = compute(
        &doc(vec![open_only], vec![tabbed(line_layer(RED))]),
        &device(),
        &JobOptions::default(),
    )
    .unwrap();
    assert!(
        job.preview
            .warnings
            .iter()
            .any(|w| w.contains("ninguna forma cerrada"))
    );
}

#[test]
fn tabs_apply_to_every_pass_and_ignore_other_modes() {
    let mut two = tabbed(line_layer(RED));
    two.passes = 2;
    let d = doc(vec![rect_object("a", RED, 0.0, 0.0, 40.0, 40.0)], vec![two]);
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    let (_, off) = lengths(&job.toolpath.blocks[0]);
    approx(off, 16.0);
    // En una capa fill, los puentes no hacen nada (el ajuste se conserva al cambiar de modo).
    let fill = LayerSettings {
        tabs: tabbed(line_layer(RED)).tabs,
        ..fill_layer(RED)
    };
    let d = doc(
        vec![rect_object("a", RED, 5.0, 5.0, 10.0, 10.0)],
        vec![fill],
    );
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    assert_eq!(job.layers[0].tabs, 0);
}

#[test]
fn tab_width_is_checked_against_the_kerf() {
    let tabbed_with = |width: f64, kerf: f64| LayerSettings {
        kerf,
        ..LayerSettings {
            tabs: tlaku_doc::TabSettings {
                width,
                ..tabbed(line_layer(RED)).tabs
            },
            ..tabbed(line_layer(RED))
        }
    };
    let run = |width: f64, kerf: f64| {
        let d = doc(
            vec![rect_object("a", RED, 5.0, 5.0, 30.0, 30.0)],
            vec![tabbed_with(width, kerf)],
        );
        compute(&d, &device(), &JobOptions::default())
            .unwrap()
            .preview
            .warnings
            .join("\n")
    };
    // El kerf es el desplazamiento por lado: el haz quita 2 * kerf del puente.
    let strong = run(0.1, 0.1);
    assert!(
        strong.contains("no sobreviven") && strong.contains("0.2"),
        "{strong}"
    );
    let soft = run(0.3, 0.1);
    assert!(soft.contains("frágiles") && soft.contains("0.10"), "{soft}");
    assert_eq!(run(0.4, 0.1), "");
    // Sin kerf no hay nada que comparar.
    assert_eq!(run(0.1, 0.0), "");
}
