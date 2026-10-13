//! Pruebas con archivos dorados del taller (Fase 4): el G-code del documento completo
//! (puentes, eje Z, rotativo y scan offset) debe coincidir byte a byte con el aprobado. Si un
//! cambio es intencional, regenera con `UPDATE_GOLDEN=1 cargo test` y revisa el diff.

use std::path::Path;
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Point, Polyline, Rgb};
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{
    DesignObject, DeviceProfile, Document, JobOptions, LayerMode, LayerSettings, ObjectKind,
    Overscan, RotaryKind, ScanOffset, VectorObject,
};
use tlaku_pipeline::{compute, gcode};

const RED: Rgb = Rgb(255, 0, 0);
const BLUE: Rgb = Rgb(0, 0, 255);

fn object(id: &str, layer: Rgb, closed: bool, pts: &[(f64, f64)]) -> DesignObject {
    DesignObject {
        id: id.into(),
        layer,
        transform: IDENTITY,
        kind: ObjectKind::Vector(VectorObject {
            paths: vec![BezPath::from(&Polyline {
                points: pts.iter().map(|&(x, y)| Point::new(x, y)).collect(),
                closed,
            })],
        }),
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

fn rect(id: &str, layer: Rgb, x: f64, y: f64, w: f64, h: f64) -> DesignObject {
    object(
        id,
        layer,
        true,
        &[(x, y), (x + w, y), (x + w, y + h), (x, y + h)],
    )
}

fn document(objects: Vec<DesignObject>, layers: Vec<LayerSettings>) -> Document {
    Document {
        objects,
        layers,
        ..Document::default()
    }
}

fn check(name: &str, doc: &Document, device: &DeviceProfile) {
    let job = compute(doc, device, &JobOptions::default()).unwrap();
    let actual = gcode(&job, device);
    let golden = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("{name}.gcode"));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&golden, &actual).unwrap();
    }
    let expected = std::fs::read_to_string(&golden)
        .unwrap_or_else(|_| panic!("falta {}; ejecuta con UPDATE_GOLDEN=1", golden.display()));
    assert_eq!(actual, expected, "el G-code de '{name}' cambió");
}

fn cut_layer(color: Rgb) -> LayerSettings {
    LayerSettings {
        speed: 600.0,
        power: 80.0,
        ..LayerSettings::for_color(color)
    }
}

#[test]
fn tabs() {
    // Cuadrado de 30 mm con un agujero (sin puentes, por skip_inner) y una línea abierta.
    let mut layer = cut_layer(RED);
    layer.tabs.enabled = true;
    layer.tabs.width = 3.0;
    layer.tabs.count = 4;
    let doc = document(
        vec![
            rect("pieza", RED, 10.0, 10.0, 30.0, 30.0),
            rect("agujero", RED, 20.0, 20.0, 10.0, 10.0),
            object("guia", RED, false, &[(10.0, 5.0), (40.0, 5.0)]),
        ],
        vec![layer],
    );
    check("tabs", &doc, &DeviceProfile::default());
}

#[test]
fn z_axis() {
    // Una línea de tres pasadas que baja 0.5 mm por pasada y un relleno con foco propio.
    let line = LayerSettings {
        passes: 3,
        z_offset: 1.0,
        z_step: 0.5,
        ..cut_layer(RED)
    };
    let fill = LayerSettings {
        mode: LayerMode::Fill,
        interval: 2.0,
        overscan: Overscan::Mm(0.0),
        z_offset: 3.0,
        ..cut_layer(BLUE)
    };
    let doc = document(
        vec![
            rect("linea", RED, 10.0, 10.0, 12.0, 8.0),
            rect("relleno", BLUE, 30.0, 10.0, 8.0, 6.0),
        ],
        vec![line, fill],
    );
    let device = DeviceProfile {
        z_axis: true,
        ..DeviceProfile::default()
    };
    check("z_axis", &doc, &device);
}

#[test]
fn rotary_roller() {
    // Un rectángulo (lados en X y en Y) y un triángulo (lados oblicuos) sobre un rodillo de
    // 20 mm con 360 mm de Y por vuelta.
    let doc = document(
        vec![
            rect("caja", RED, 10.0, 10.0, 20.0, 15.0),
            object(
                "flecha",
                RED,
                true,
                &[(40.0, 10.0), (60.0, 10.0), (50.0, 25.0)],
            ),
        ],
        vec![cut_layer(RED)],
    );
    let mut device = DeviceProfile::default();
    device.rotary.enabled = true;
    device.rotary.kind = RotaryKind::Roller;
    device.rotary.mm_per_rotation = 360.0;
    device.rotary.roller_diameter = 20.0;
    device.rotary.object_diameter = 80.0;
    check("rotary_roller", &doc, &device);
}

#[test]
fn scan_offset() {
    // Relleno bidireccional a 30°: las líneas de vuelta se corren en la dirección del barrido.
    let fill = LayerSettings {
        mode: LayerMode::Fill,
        speed: 3000.0,
        power: 25.0,
        interval: 2.0,
        angle: 30.0,
        overscan: Overscan::Mm(1.0),
        ..LayerSettings::for_color(RED)
    };
    let doc = document(vec![rect("placa", RED, 10.0, 10.0, 12.0, 6.0)], vec![fill]);
    let device = DeviceProfile {
        scan_offsets: vec![
            ScanOffset {
                speed: 1000.0,
                offset: 0.1,
            },
            ScanOffset {
                speed: 6000.0,
                offset: 0.6,
            },
        ],
        ..DeviceProfile::default()
    };
    check("scan_offset", &doc, &device);
}
