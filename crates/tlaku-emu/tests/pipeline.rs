//! G-code generado por `tlaku_pipeline` enviado con el `Streamer` real a la controladora
//! emulada: el rastro del láser debe caer solo sobre la figura.

mod common;

use std::time::Duration;

use common::*;
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Point, Polyline, Rgb};
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{
    DesignObject, DeviceProfile, Document, JobOptions, LayerMode, LayerSettings, ObjectKind,
    Overscan, VectorObject,
};
use tlaku_emu::{LaserTrace, Profile, StateKind};
use tlaku_grbl::streamer::Outcome;
use tlaku_pipeline::{compute, gcode};

const RED: Rgb = Rgb(255, 0, 0);
const BLUE: Rgb = Rgb(0, 0, 255);

fn rect(id: &str, layer: Rgb, x: f64, y: f64, w: f64, h: f64) -> DesignObject {
    let pts = [(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
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

fn gcode_of(doc: &Document, device: &DeviceProfile) -> Vec<String> {
    let job = compute(doc, device, &JobOptions::default()).unwrap();
    gcode(&job, device).lines().map(str::to_string).collect()
}

fn cut_layer() -> LayerSettings {
    LayerSettings {
        speed: 600.0,
        power: 80.0,
        ..LayerSettings::for_color(RED)
    }
}

/// Vértices (cerrados) de un cuadrado.
fn outline(x: f64, y: f64, side: f64) -> Vec<[f64; 2]> {
    vec![
        [x, y],
        [x + side, y],
        [x + side, y + side],
        [x, y + side],
        [x, y],
    ]
}

/// Mayor distancia de lo quemado a la unión de contornos cerrados.
fn max_distance(trace: &LaserTrace, outlines: &[Vec<[f64; 2]>]) -> f64 {
    let edges: Vec<([f64; 2], [f64; 2])> = outlines
        .iter()
        .flat_map(|o| o.windows(2).map(|w| (w[0], w[1])))
        .collect();
    let mut worst = 0.0f64;
    for s in trace.segments() {
        for i in 0..=10 {
            let f = f64::from(i) / 10.0;
            let p = [
                s.start[0] + (s.end[0] - s.start[0]) * f,
                s.start[1] + (s.end[1] - s.start[1]) * f,
            ];
            let d = edges
                .iter()
                .map(|&(a, b)| tlaku_emu::trace::dist_point_segment(p, a, b))
                .fold(f64::INFINITY, f64::min);
            worst = worst.max(d);
        }
    }
    worst
}

fn run(profile: Profile, homed: bool, lines: &[String]) -> (tlaku_emu::Handle, Option<Outcome>) {
    let (vm, h) = scaled_vm(profile, 100.0, |b| b.homed(homed));
    let run = run_job(vm, fast_config(), lines, Duration::from_secs(30));
    assert!(!run.panicked && !run.timed_out, "{:?}", run.outcome);
    (h, run.outcome)
}

#[test]
fn square_cut_burns_only_on_the_square() {
    let doc = Document {
        objects: vec![rect("cuadrado", RED, 10.0, 10.0, 20.0, 20.0)],
        layers: vec![cut_layer()],
        ..Document::default()
    };
    let lines = gcode_of(&doc, &DeviceProfile::default());
    assert!(lines.iter().any(|l| l.starts_with("M4")), "{lines:?}");
    let (h, outcome) = run(Profile::grbl_1_1h(), false, &lines);
    assert_eq!(outcome, Some(Outcome::Completed { errors: 0 }));
    assert_eq!(h.state(), StateKind::Idle);
    assert_eq!(h.mpos(), [0.0; 3], "el trabajo regresa al origen");
    assert_eq!(h.rx_overflow_bytes(), 0);
    assert!(h.max_in_flight() <= 127);

    let t = h.trace();
    assert!(!t.is_empty());
    assert!(t.burned_during_rapids().is_empty());
    assert!(t.stationary_burns().is_empty());
    let d = max_distance(&t, &[outline(10.0, 10.0, 20.0)]);
    assert!(d < 0.02, "se quemó a {d} mm del contorno");
    assert!(
        (t.total_length() - 80.0).abs() < 0.1,
        "{}",
        t.total_length()
    );
    let (lo, hi) = t.bounds().unwrap();
    for (a, b) in [(lo[0], 10.0), (lo[1], 10.0), (hi[0], 30.0), (hi[1], 30.0)] {
        assert!((a - b).abs() < 0.02, "{lo:?} {hi:?}");
    }
    // Potencia del 80 % de $30=1000, nunca más.
    let peak = t.segments().iter().map(|s| s.s_peak).fold(0.0, f64::max);
    assert!((peak - 800.0).abs() < 1.0, "{peak}");
    assert!(t.segments().iter().all(|s| s.s_programmed == 800.0));
    // Y el G-code que llegó es el generado, sin errores de línea larga.
    assert!(h.received_lines().iter().all(|l| l.len() <= 80));
}

#[test]
fn two_squares_do_not_burn_between_them() {
    let doc = Document {
        objects: vec![
            rect("a", RED, 10.0, 10.0, 10.0, 10.0),
            rect("b", RED, 60.0, 40.0, 10.0, 10.0),
        ],
        layers: vec![cut_layer()],
        ..Document::default()
    };
    let lines = gcode_of(&doc, &DeviceProfile::default());
    let (h, outcome) = run(Profile::grbl_1_1h(), false, &lines);
    assert_eq!(outcome, Some(Outcome::Completed { errors: 0 }));
    let t = h.trace();
    assert!(t.burned_during_rapids().is_empty());
    let d = max_distance(&t, &[outline(10.0, 10.0, 10.0), outline(60.0, 40.0, 10.0)]);
    assert!(d < 0.02, "{d}");
    assert!(
        (t.total_length() - 80.0).abs() < 0.1,
        "{}",
        t.total_length()
    );
}

#[test]
fn fill_overscan_does_not_burn_outside_the_shape() {
    let fill = LayerSettings {
        mode: LayerMode::Fill,
        interval: 2.0,
        overscan: Overscan::Mm(3.0),
        speed: 3000.0,
        power: 50.0,
        ..LayerSettings::for_color(BLUE)
    };
    let doc = Document {
        objects: vec![rect("relleno", BLUE, 10.0, 10.0, 20.0, 20.0)],
        layers: vec![fill],
        ..Document::default()
    };
    let lines = gcode_of(&doc, &DeviceProfile::default());
    assert!(
        lines.iter().any(|l| l.contains("S0")),
        "se esperaba overscan con S0"
    );
    let (h, outcome) = run(Profile::grbl_1_1h(), false, &lines);
    assert_eq!(outcome, Some(Outcome::Completed { errors: 0 }));
    let t = h.trace();
    assert!(t.burned_during_rapids().is_empty());
    assert!(t.total_length() > 150.0, "{}", t.total_length());
    // El barrido llega 3 mm más allá de la figura, pero allí S vale 0: lo quemado queda dentro.
    let (lo, hi) = t.bounds().unwrap();
    let eps = 0.1;
    assert!(
        lo[0] >= 10.0 - eps && hi[0] <= 30.0 + eps && lo[1] >= 10.0 - eps && hi[1] <= 30.0 + eps,
        "{lo:?} {hi:?}"
    );
}

#[test]
fn the_same_square_on_every_grbl_family_profile() {
    let doc = Document {
        objects: vec![rect("cuadrado", RED, 10.0, 10.0, 20.0, 20.0)],
        layers: vec![cut_layer()],
        ..Document::default()
    };
    let lines = gcode_of(&doc, &DeviceProfile::default());
    for (p, homed) in [
        (Profile::falcon2_pro(), true),
        (Profile::grbl_hal(), false),
        (Profile::fluidnc(), false),
        (Profile::grbl_lpc().with_setting(30, 1000.0), false),
    ] {
        let name = p.name.clone();
        let (h, outcome) = run(p, homed, &lines);
        assert_eq!(outcome, Some(Outcome::Completed { errors: 0 }), "{name}");
        let t = h.trace();
        assert!(t.burned_during_rapids().is_empty(), "{name}");
        assert!(
            max_distance(&t, &[outline(10.0, 10.0, 20.0)]) < 0.02,
            "{name}"
        );
        assert!((t.total_length() - 80.0).abs() < 0.1, "{name}");
    }
}

#[test]
fn without_laser_mode_the_travel_moves_burn() {
    // `$32=0`: el husillo no se apaga en los G0. Es el riesgo del punto 19 de grbl.md, que el
    // rastro del emulador sabe detectar.
    let doc = Document {
        objects: vec![
            rect("a", RED, 10.0, 10.0, 10.0, 10.0),
            rect("b", RED, 60.0, 40.0, 10.0, 10.0),
        ],
        layers: vec![cut_layer()],
        ..Document::default()
    };
    let lines = gcode_of(&doc, &DeviceProfile::default());
    let (h, outcome) = run(Profile::grbl_1_1h().with_setting(32, 0.0), false, &lines);
    assert_eq!(outcome, Some(Outcome::Completed { errors: 0 }));
    let t = h.trace();
    assert!(
        !t.burned_during_rapids().is_empty(),
        "debió quemar en algún G0"
    );
    assert!(max_distance(&t, &[outline(10.0, 10.0, 10.0), outline(60.0, 40.0, 10.0)]) > 1.0);
}

#[test]
fn lpc_with_default_s_max_saturates_the_job() {
    // GRBL-LPC trae $30=1: las S de hasta 1000 saturan al máximo (punto de dialectos 6).
    let doc = Document {
        objects: vec![rect("cuadrado", RED, 10.0, 10.0, 10.0, 10.0)],
        layers: vec![cut_layer()],
        ..Document::default()
    };
    let lines = gcode_of(&doc, &DeviceProfile::default());
    let (h, outcome) = run(Profile::grbl_lpc(), false, &lines);
    assert_eq!(outcome, Some(Outcome::Completed { errors: 0 }));
    let peak = h
        .trace()
        .segments()
        .iter()
        .map(|s| s.s_peak)
        .fold(0.0, f64::max);
    assert!(
        (peak - 1.0).abs() < 1e-9,
        "S800 con $30=1 satura a 1.0: {peak}"
    );
    assert!(h.trace().segments().iter().all(|s| s.s_programmed == 800.0));
}
