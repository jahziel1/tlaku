//! Pausas con el láser encendido (`Move::Dwell`) del pipeline enviadas a la controladora
//! emulada: el modo punto, las pausas de inicio y fin y el corte pasante deben quemar parados
//! (GRBL en `M4` no lo hace, así que el generador pasa a `M3` durante la pausa) y nunca en los
//! rápidos.

mod common;

use std::time::Duration;

use common::*;
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Point, Polyline, Rgb};
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{
    DesignObject, DeviceProfile, Document, JobOptions, LayerSettings, ObjectKind, VectorObject,
};
use tlaku_emu::{Profile, StateKind};
use tlaku_grbl::streamer::Outcome;
use tlaku_pipeline::{compute, gcode};

const RED: Rgb = Rgb(255, 0, 0);

fn object(id: &str, closed: bool, pts: &[(f64, f64)]) -> DesignObject {
    DesignObject {
        id: id.into(),
        layer: RED,
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

fn run_layer(
    objects: Vec<DesignObject>,
    edit: impl FnOnce(&mut LayerSettings),
) -> (tlaku_emu::Handle, Vec<String>) {
    let mut layer = LayerSettings {
        speed: 600.0,
        power: 40.0,
        ..LayerSettings::for_color(RED)
    };
    edit(&mut layer);
    let doc = Document {
        objects,
        layers: vec![layer],
        ..Document::default()
    };
    let device = DeviceProfile::default();
    let job = compute(&doc, &device, &JobOptions::default()).unwrap();
    let lines: Vec<String> = gcode(&job, &device).lines().map(str::to_string).collect();
    let (vm, h) = scaled_vm(Profile::grbl_1_1h(), 100.0, |b| b.homed(false));
    let run = run_job(vm, fast_config(), &lines, Duration::from_secs(30));
    assert!(!run.panicked && !run.timed_out, "{:?}", run.outcome);
    assert_eq!(run.outcome, Some(Outcome::Completed { errors: 0 }));
    assert_eq!(h.state(), StateKind::Idle);
    (h, lines)
}

#[test]
fn dot_mode_burns_one_stationary_dot_per_spacing_and_never_in_rapids() {
    let (h, lines) = run_layer(object_line(), |l| {
        l.dot_mode.enabled = true;
        l.dot_mode.spacing_mm = 2.0;
        l.dot_mode.dwell_ms = 50.0;
    });
    assert!(lines.iter().any(|l| l.starts_with("G4 P0.05")), "{lines:?}");
    let t = h.trace();
    assert!(t.burned_during_rapids().is_empty());
    let dots = t.stationary_burns();
    // Línea de 10 mm cada 2 mm: seis puntos, en x = 10, 12, ..., 20.
    assert_eq!(dots.len(), 6, "{dots:?}");
    for (i, d) in dots.iter().enumerate() {
        assert!(
            (d.start[0] - (10.0 + 2.0 * i as f64)).abs() < 0.01,
            "{:?}",
            d.start
        );
        assert!(
            d.duration() >= Duration::from_millis(45),
            "{:?}",
            d.duration()
        );
        assert_eq!(d.s_programmed, 400.0);
    }
    // Y no hay ningún tramo de corte continuo: solo puntos.
    assert!(t.segments().iter().all(|s| s.is_stationary()));
}

fn object_line() -> Vec<DesignObject> {
    vec![object("linea", false, &[(10.0, 10.0), (20.0, 10.0)])]
}

#[test]
fn start_and_end_delays_burn_at_the_ends_of_the_stroke() {
    let (h, lines) = run_layer(object_line(), |l| {
        l.start_delay_ms = 100.0;
        l.end_delay_ms = 200.0;
    });
    assert!(lines.iter().any(|l| l == "G4 P0.1") && lines.iter().any(|l| l == "G4 P0.2"));
    let t = h.trace();
    assert!(t.burned_during_rapids().is_empty());
    let parked = t.stationary_burns();
    assert_eq!(parked.len(), 2, "{parked:?}");
    assert!((parked[0].start[0] - 10.0).abs() < 0.01);
    assert!((parked[1].start[0] - 20.0).abs() < 0.01);
    assert!(parked[1].duration() > parked[0].duration());
    // Entre las dos pausas, el corte normal de 10 mm.
    let moving: f64 = t
        .segments()
        .iter()
        .filter(|s| !s.is_stationary())
        .map(|s| s.length())
        .sum();
    assert!((moving - 10.0).abs() < 0.05, "{moving}");
}

#[test]
fn cut_through_burns_with_its_own_power_before_the_cut() {
    let square = [(10.0, 10.0), (20.0, 10.0), (20.0, 20.0), (10.0, 20.0)];
    let (h, _) = run_layer(vec![object("c", true, &square)], |l| {
        l.cut_through.enabled = true;
        l.cut_through.power = 90.0;
        l.cut_through.ms = 150.0;
    });
    let t = h.trace();
    assert!(t.burned_during_rapids().is_empty());
    let parked = t.stationary_burns();
    assert_eq!(parked.len(), 1, "{parked:?}");
    assert_eq!(parked[0].s_programmed, 900.0);
    // Luego el contorno completo a la potencia de la capa.
    let moving: Vec<_> = t.segments().iter().filter(|s| !s.is_stationary()).collect();
    assert!(moving.iter().all(|s| s.s_programmed == 400.0));
    let len: f64 = moving.iter().map(|s| s.length()).sum();
    assert!((len - 40.0).abs() < 0.1, "{len}");
}
