//! Dorados de los ajustes de trazo de las capas line (ola 2): perforación, modo punto, entradas,
//! salidas, overcut, rampa, pausas, corte pasante y escala de potencia por objeto. El G-code debe
//! coincidir byte a byte con el aprobado; si un cambio es intencional, regenera con
//! `UPDATE_GOLDEN=1 cargo test` y revisa el diff.

use std::path::Path;
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Point, Polyline, Rgb};
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{
    DesignObject, DeviceProfile, Document, JobOptions, LayerSettings, LeadKind, ObjectKind,
    VectorObject,
};
use tlaku_pipeline::{compute, gcode};

const RED: Rgb = Rgb(255, 0, 0);

fn object(id: &str, closed: bool, pts: &[(f64, f64)]) -> DesignObject {
    DesignObject {
        id: id.into(),
        layer: RED,
        transform: IDENTITY,
        kind: ObjectKind::Vector(VectorObject::new(vec![BezPath::from(&Polyline {
            points: pts.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            closed,
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

fn rect(id: &str, x: f64, y: f64, w: f64, h: f64) -> DesignObject {
    object(id, true, &[(x, y), (x + w, y), (x + w, y + h), (x, y + h)])
}

fn layer(edit: impl FnOnce(&mut LayerSettings)) -> LayerSettings {
    let mut l = LayerSettings {
        speed: 600.0,
        power: 80.0,
        ..LayerSettings::for_color(RED)
    };
    edit(&mut l);
    l
}

fn document(objects: Vec<DesignObject>, layer: LayerSettings) -> Document {
    Document {
        objects,
        layers: vec![layer],
        ..Document::default()
    }
}

fn check(name: &str, doc: &Document) {
    let device = DeviceProfile::default();
    let job = compute(doc, &device, &JobOptions::default()).unwrap();
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

fn warnings(doc: &Document) -> Vec<String> {
    compute(doc, &DeviceProfile::default(), &JobOptions::default())
        .unwrap()
        .preview
        .warnings
}

/// Una pieza de 30 mm con un agujero de 10 mm y una línea abierta.
fn piece_with_hole() -> Vec<DesignObject> {
    vec![
        rect("pieza", 10.0, 10.0, 30.0, 30.0),
        rect("agujero", 20.0, 20.0, 10.0, 10.0),
        object("guia", false, &[(10.0, 5.0), (40.0, 5.0)]),
    ]
}

#[test]
fn perforation() {
    let doc = document(
        piece_with_hole(),
        layer(|l| {
            l.perforation.enabled = true;
            l.perforation.cut_mm = 4.0;
            l.perforation.skip_mm = 1.5;
        }),
    );
    check("stroke_perforation", &doc);
}

#[test]
fn dot_mode() {
    let doc = document(
        vec![
            rect("cuadro", 10.0, 10.0, 12.0, 12.0),
            object("linea", false, &[(10.0, 5.0), (22.0, 5.0)]),
        ],
        layer(|l| {
            l.dot_mode.enabled = true;
            l.dot_mode.spacing_mm = 3.0;
            l.dot_mode.dwell_ms = 12.0;
        }),
    );
    check("stroke_dot_mode", &doc);
}

#[test]
fn straight_leads_and_overcut() {
    // La entrada del exterior va por fuera y la del agujero por dentro; la línea abierta queda igual.
    let doc = document(
        piece_with_hole(),
        layer(|l| {
            l.lead_in.enabled = true;
            l.lead_in.length = 3.0;
            l.lead_in.angle = 60.0;
            l.lead_out.enabled = true;
            l.lead_out.length = 2.0;
            l.lead_out.angle = 45.0;
            l.overcut = 2.0;
        }),
    );
    check("stroke_leads_straight", &doc);
    assert!(
        warnings(&doc)
            .iter()
            .any(|w| w.starts_with("capa #ff0000: ") && w.contains("abierta"))
    );
}

#[test]
fn arc_leads() {
    let doc = document(
        piece_with_hole(),
        layer(|l| {
            l.lead_in.enabled = true;
            l.lead_in.kind = LeadKind::Arc;
            l.lead_in.length = 4.0;
            l.lead_in.angle = 90.0;
            l.lead_out.enabled = true;
            l.lead_out.kind = LeadKind::Arc;
            l.lead_out.length = 3.0;
            l.lead_out.angle = 60.0;
        }),
    );
    check("stroke_leads_arc", &doc);
}

#[test]
fn ramp_only_outer() {
    let doc = document(
        piece_with_hole(),
        layer(|l| {
            l.ramp.enabled = true;
            l.ramp.length_mm = 3.0;
            l.ramp.start_pct = 25.0;
            l.ramp.outer_only = true;
        }),
    );
    check("stroke_ramp_outer", &doc);
}

#[test]
fn pauses_and_cut_through() {
    let doc = document(
        piece_with_hole(),
        layer(|l| {
            l.start_delay_ms = 30.0;
            l.end_delay_ms = 20.0;
            l.cut_through.enabled = true;
            l.cut_through.power = 100.0;
            l.cut_through.ms = 250.0;
        }),
    );
    check("stroke_pauses", &doc);
}

#[test]
fn power_scale_per_object_with_kerf_and_tabs() {
    let mut half = rect("mitad", 10.0, 10.0, 20.0, 20.0);
    half.power_scale = Some(50.0);
    let mut over = rect("exceso", 50.0, 10.0, 20.0, 20.0);
    over.power_scale = Some(150.0);
    let full = rect("completo", 90.0, 10.0, 20.0, 20.0);
    let doc = document(
        vec![half, over, full],
        layer(|l| {
            l.power = 60.0;
            l.kerf = 0.1;
            l.tabs.enabled = true;
            l.tabs.width = 2.0;
            l.tabs.count = 2;
            l.perforation.enabled = true;
            l.perforation.cut_mm = 8.0;
            l.perforation.skip_mm = 2.0;
        }),
    );
    check("stroke_power_scale", &doc);
    let g = gcode(
        &compute(&doc, &DeviceProfile::default(), &JobOptions::default()).unwrap(),
        &DeviceProfile::default(),
    );
    // 60 % * 0,5 = 30 %, 60 % * 1,5 = 90 % (con S máxima 1000) y 60 % a secas.
    for s in ["S300", "S900", "S600"] {
        assert!(g.contains(s), "falta {s}");
    }
    assert!(
        warnings(&doc).iter().all(|w| !w.contains("100 %")),
        "90 % no excede"
    );
}

#[test]
fn scaled_power_over_100_is_clamped_with_a_warning() {
    let mut o = rect("fuerte", 10.0, 10.0, 10.0, 10.0);
    o.power_scale = Some(200.0);
    let doc = document(vec![o], layer(|l| l.power = 70.0));
    let w = warnings(&doc);
    assert!(
        w.iter()
            .any(|w| w.starts_with("capa #ff0000: ") && w.contains("100 %")),
        "{w:?}"
    );
    let device = DeviceProfile::default();
    let g = gcode(
        &compute(&doc, &device, &JobOptions::default()).unwrap(),
        &device,
    );
    assert!(g.contains("S1000") && !g.contains("S1400"));
}

#[test]
fn incompatible_combinations_warn_and_the_dot_mode_wins() {
    let doc = document(
        piece_with_hole(),
        layer(|l| {
            l.dot_mode.enabled = true;
            l.perforation.enabled = true;
            l.ramp.enabled = true;
            l.overcut = 1.0;
        }),
    );
    let w = warnings(&doc);
    for needle in ["perforación", "rampa", "overcut"] {
        assert!(
            w.iter()
                .any(|m| m.starts_with("capa #ff0000: ") && m.contains(needle)),
            "falta el aviso de {needle}: {w:?}"
        );
    }
    // El resultado es el del modo punto solo.
    let plain = document(piece_with_hole(), layer(|l| l.dot_mode.enabled = true));
    let device = DeviceProfile::default();
    let make = |d: &Document| {
        gcode(
            &compute(d, &device, &JobOptions::default()).unwrap(),
            &device,
        )
    };
    assert_eq!(make(&doc), make(&plain));
}

#[test]
fn bad_stroke_settings_fail_with_the_layer_name() {
    let doc = document(
        piece_with_hole(),
        layer(|l| {
            l.ramp.enabled = true;
            l.ramp.length_mm = 0.0;
        }),
    );
    let e = compute(&doc, &DeviceProfile::default(), &JobOptions::default()).unwrap_err();
    assert!(e.starts_with("capa #ff0000: rampa"), "{e}");
}
