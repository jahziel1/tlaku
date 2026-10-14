//! Dorados de la ola 2: sub-capas, optimización por objeto y capa, e incremento de ángulo. El
//! G-code completo debe coincidir byte a byte con el aprobado. Si un cambio es intencional,
//! regenera con `UPDATE_GOLDEN=1 cargo test` y revisa el diff.

use std::path::Path;
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Point, Polyline, Rgb};
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{
    DesignObject, DeviceProfile, Document, JobOptions, LayerMode, LayerSettings, ObjectKind,
    Optimization, Overscan, VectorObject,
};
use tlaku_pipeline::{compute, gcode};

const RED: Rgb = Rgb(255, 0, 0);
const BLUE: Rgb = Rgb(0, 0, 255);

fn object(id: &str, layer: Rgb, pts: &[(f64, f64)]) -> DesignObject {
    DesignObject {
        id: id.into(),
        layer,
        transform: IDENTITY,
        kind: ObjectKind::Vector(VectorObject::new(vec![BezPath::from(&Polyline {
            points: pts.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            closed: true,
        })])),
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
    object(id, layer, &[(x, y), (x + w, y), (x + w, y + h), (x, y + h)])
}

fn check(name: &str, doc: &Document, options: &JobOptions) {
    let device = DeviceProfile::default();
    let job = compute(doc, &device, options).unwrap();
    let actual = gcode(&job, &device);
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

fn fill(color: Rgb) -> LayerSettings {
    LayerSettings {
        mode: LayerMode::Fill,
        interval: 1.0,
        speed: 3000.0,
        power: 30.0,
        overscan: Overscan::Mm(2.0),
        ..LayerSettings::for_color(color)
    }
}

#[test]
fn sub_layers() {
    // Grabado en la capa roja con un contorno de corte como sub-capa, y otro grabado azul que
    // debe ir antes del corte aunque esté después en el archivo.
    let red = LayerSettings {
        sub_layers: vec![LayerSettings {
            speed: 300.0,
            power: 90.0,
            passes: 2,
            kerf: 0.1,
            ..LayerSettings::for_color(RED)
        }],
        ..fill(RED)
    };
    let doc = Document {
        objects: vec![
            rect("a", RED, 10.0, 10.0, 20.0, 12.0),
            rect("b", BLUE, 50.0, 10.0, 10.0, 10.0),
        ],
        layers: vec![red, fill(BLUE)],
        ..Document::default()
    };
    check("orden_sub_capas", &doc, &JobOptions::default());
}

#[test]
fn optimization() {
    // Prioridad por objeto, grupos, un borde compartido que se corta una vez y un cuadrado en
    // sentido horario que hide_backlash gira.
    let mut a = rect("a", RED, 10.0, 10.0, 10.0, 10.0);
    a.priority = Some(2);
    let mut b = rect("b", RED, 20.0, 10.0, 10.0, 10.0);
    b.priority = Some(1);
    let mut c = object(
        "c",
        RED,
        &[(60.0, 10.0), (60.0, 20.0), (70.0, 20.0), (70.0, 10.0)],
    );
    c.priority = Some(0);
    let line = LayerSettings {
        speed: 600.0,
        power: 80.0,
        ..LayerSettings::for_color(RED)
    };
    let doc = Document {
        objects: vec![a, b, c],
        layers: vec![line],
        ..Document::default()
    };
    let options = JobOptions {
        optimization: Optimization {
            remove_overlaps: true,
            hide_backlash: true,
            ..Optimization::default()
        },
        ..JobOptions::default()
    };
    check("orden_optimizacion", &doc, &options);
}

#[test]
fn angle_increment() {
    let layer = LayerSettings {
        passes: 3,
        angle: 15.0,
        angle_increment: 30.0,
        ..fill(RED)
    };
    let doc = Document {
        objects: vec![rect("a", RED, 10.0, 10.0, 12.0, 8.0)],
        layers: vec![layer],
        ..Document::default()
    };
    check("orden_incremento_angulo", &doc, &JobOptions::default());
}
