//! Límites de la cama.

use super::*;
use tlaku_core::Point;
use tlaku_core::toolpath::{Move, PowerMode, ToolpathBlock};
use tlaku_doc::{JobOptions, LayerSettings, Overscan};

#[test]
fn bounds_error_counts_the_overscan() {
    let overscan = LayerSettings {
        overscan: Overscan::Mm(2.0),
        ..fill_layer(BLUE)
    };
    let d = doc(
        vec![rect_object("r", BLUE, 10.0, 10.0, 40.0, 10.0)],
        vec![overscan],
    );
    let mut dev = device();
    // El relleno llega a X50 y con el overscan a X52.
    dev.bed = [51.0, 100.0];
    let e = compute(&d, &dev, &JobOptions::default()).unwrap_err();
    assert!(
        e.contains("incluye 2.00 mm de overscan") && e.contains("#0000ff"),
        "{e}"
    );
    assert!(e.contains("51x100"), "{e}");
    // `build` no verifica la cama y la vista previa sigue disponible.
    assert!(build(&d, &dev, &JobOptions::default(), None).is_ok());
    dev.bed = [53.0, 100.0];
    assert!(compute(&d, &dev, &JobOptions::default()).is_ok());
    // También por abajo y a la izquierda.
    let outside = doc(
        vec![rect_object("r", RED, -5.0, 10.0, 10.0, 10.0)],
        vec![line_layer(RED)],
    );
    assert!(compute(&outside, &device(), &JobOptions::default()).is_err());
}

#[test]
fn bounds_check_covers_every_move() {
    let block = ToolpathBlock {
        label: "x".into(),
        speed_mm_min: 1000.0,
        air_assist: false,
        power_mode: PowerMode::Dynamic,
        moves: vec![
            Move::Travel {
                to: Point::new(-0.5, 5.0),
            },
            Move::Cut {
                to: Point::new(10.0, 5.0),
                power_pct: 0.0,
            },
        ],
        z_mm: None,
        galvo: None,
    };
    let layer = LayerReport {
        name: "#ff0000".into(),
        color: RED,
        mode: "fill",
        count: 1,
        unit: "líneas",
        speed_mm_min: 1000.0,
        power: "10%".into(),
        passes: 1,
        overscan_mm: Some(2.0),
        plan: None,
        seconds: 0.0,
        bounds: block_bounds(&block),
        tabs: 0,
        z_mm: None,
        pass: None,
        sub: None,
        u_offset_mm: 0.0,
        laser2: false,
    };
    let job = ComputedJob {
        toolpath: tlaku_core::toolpath::Toolpath {
            blocks: vec![block],
        },
        preview: crate::preview::preview(
            &tlaku_core::toolpath::Toolpath::default(),
            &[],
            0.0,
            vec![],
            None,
            vec![],
        ),
        layers: vec![layer],
    };
    let e = check_bounds(&job, [100.0, 100.0]).unwrap_err();
    assert!(e.contains("#ff0000") && e.contains("overscan"), "{e}");
}

#[test]
fn bounds_warning_counts_the_virtual_copies() {
    // El original (10 x 10 mm) cabe en la cama de 100 x 100, pero la rejilla de 5 columnas
    // (paso de 30 mm) llega a X = 130 mm: debe avisar, y la caja del trabajo la incluye.
    let mut a = rect_object("a", RED, 0.0, 0.0, 10.0, 10.0);
    a.array = Some(tlaku_doc::VirtualArray::Grid {
        cols: 5,
        rows: 1,
        dx: 20.0,
        dy: 0.0,
    });
    let d = doc(vec![a], vec![line_layer(RED)]);
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    point(job.bounds().unwrap().max, 130.0, 10.0);
    let e = check_bounds(&job, [100.0, 100.0]).unwrap_err();
    assert!(e.contains("#ff0000"), "{e}");
    point(d.bounds().unwrap().max, 130.0, 10.0);
}

#[test]
fn invalid_device_is_rejected() {
    let mut dev = device();
    dev.accel = [0.0, 1000.0];
    let e = compute(&tall_rect(), &dev, &JobOptions::default()).unwrap_err();
    assert!(e.contains("aceleración"), "{e}");
}
