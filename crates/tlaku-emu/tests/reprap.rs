//! Marlin y Smoothieware: el `Streamer` real con un `LinkProfile` de cada familia contra la
//! controladora emulada. Un trabajo completo (generado por `tlaku_pipeline`), pausa, cancelación
//! (`M410` + apagado del láser; `M112` + `M999`), líneas numeradas con `Resend`, el rastro del
//! láser (que no queme en G0, tampoco en Marlin, donde G0 y G1 comparten F y S) y la comprobación
//! previa con `M115`.

mod common;

use std::time::{Duration, Instant};

use common::*;
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Point, Polyline, Rgb};
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{
    DesignObject, DeviceKind, DeviceProfile, Document, GcodeDialect, JobOptions, LayerSettings,
    ObjectKind, PowerMode, VectorObject,
};
use tlaku_emu::{Fault, Handle, LaserTrace, Profile, StateKind, Trigger};
use tlaku_grbl::link::{LinkProfile, StatusQuery};
use tlaku_grbl::preflight::{Advice, JobExpect, PreflightConfig, preflight_link};
use tlaku_grbl::serial::memory_pair;
use tlaku_grbl::streamer::{Event, Outcome, Streamer, StreamerConfig};
use tlaku_pipeline::{compute, gcode};

const RED: Rgb = Rgb(255, 0, 0);
const DEADLINE: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// Utilidades
// ---------------------------------------------------------------------------

fn rect(id: &str, x: f64, y: f64, w: f64, h: f64) -> DesignObject {
    let pts = [(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
    DesignObject {
        id: id.into(),
        layer: RED,
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

/// Dos cuadrados de 20 mm separados: entre ambos hay un G0 que no debe quemar.
fn two_squares() -> Document {
    Document {
        objects: vec![
            rect("a", 10.0, 10.0, 20.0, 20.0),
            rect("b", 50.0, 10.0, 20.0, 20.0),
        ],
        layers: vec![LayerSettings {
            speed: 600.0,
            power: 80.0,
            ..LayerSettings::for_color(RED)
        }],
        ..Document::default()
    }
}

fn outlines() -> Vec<Vec<[f64; 2]>> {
    [10.0, 50.0]
        .iter()
        .map(|&x| {
            vec![
                [x, 10.0],
                [x + 20.0, 10.0],
                [x + 20.0, 30.0],
                [x, 30.0],
                [x, 10.0],
            ]
        })
        .collect()
}

/// Perfil de máquina de un firmware con un dialecto dado.
fn device_with(kind: DeviceKind, dialect: GcodeDialect) -> DeviceProfile {
    let mut d = DeviceProfile::for_kind(kind);
    d.power_mode = if dialect.dynamic_on.is_some() {
        PowerMode::Dynamic
    } else {
        PowerMode::Constant
    };
    d.dialect = dialect;
    d
}

fn gcode_of(doc: &Document, device: &DeviceProfile) -> Vec<String> {
    let job = compute(doc, device, &JobOptions::default()).unwrap();
    gcode(&job, device).lines().map(str::to_string).collect()
}

/// Mayor distancia de lo quemado a los contornos.
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

fn config_for(link: &LinkProfile) -> StreamerConfig {
    StreamerConfig::for_link(link)
}

/// Trabajo completo sobre la máquina emulada con el enlace dado.
fn run(profile: Profile, link: LinkProfile, lines: &[String]) -> (Handle, JobRun) {
    let (vm, h) = scaled_vm(profile, 100.0, |b| b);
    let run = run_job_link(vm, config_for(&link), link, lines, DEADLINE, |_| {});
    assert!(!run.panicked && !run.timed_out, "{:?}", run.outcome);
    (h, run)
}

/// Circunferencia de radio 15 mm en `n` tramos: un trabajo largo para pausar y cancelar. `head`
/// va antes del primer corte (con `S` y `F`) y `tail` después del último.
fn circle(head: &[&str], first_cut: &str, tail: &[&str]) -> Vec<String> {
    let n = 160;
    let mut lines: Vec<String> = head.iter().map(|s| (*s).to_string()).collect();
    for i in 1..=n {
        let a = f64::from(i) * std::f64::consts::TAU / f64::from(n);
        let (x, y) = (30.0 + 15.0 * a.cos(), 30.0 + 15.0 * a.sin());
        if i == 1 {
            lines.push(format!("G1 X{x:.3} Y{y:.3} {first_cut}"));
        } else {
            lines.push(format!("G1 X{x:.3} Y{y:.3}"));
        }
    }
    lines.extend(tail.iter().map(|s| (*s).to_string()));
    lines
}

fn marlin_circle() -> Vec<String> {
    circle(
        &["G90", "G21", "M3 S0 I", "G0 X45 Y30 F6000"],
        "S128 F600",
        &["M5", "G0 X0 Y0 F6000"],
    )
}

fn smoothie_circle() -> Vec<String> {
    circle(
        &["G90", "G21", "M3 S0", "G0 X45 Y30 F6000"],
        "S0.5 F600",
        &["M5", "G0 X0 Y0 F6000"],
    )
}

fn marlin_link() -> LinkProfile {
    LinkProfile::marlin(&GcodeDialect::marlin_inline())
}

fn smoothie_link() -> LinkProfile {
    LinkProfile::for_device(&DeviceProfile::for_kind(DeviceKind::Smoothie))
}

fn assert_clean_trace(h: &Handle, expected_len: f64) {
    let t = h.trace();
    assert!(!t.is_empty());
    assert!(t.burned_during_rapids().is_empty(), "quemó en un G0");
    assert!(
        t.stationary_burns()
            .iter()
            .all(|s| s.duration() < Duration::from_millis(5))
    );
    let d = max_distance(&t, &outlines());
    assert!(d < 0.02, "se quemó a {d} mm de los contornos");
    assert!(
        (t.total_length() - expected_len).abs() < 0.2,
        "{} mm quemados, se esperaban {expected_len}",
        t.total_length()
    );
    assert!(!h.laser_on());
}

// ---------------------------------------------------------------------------
// Trabajo completo
// ---------------------------------------------------------------------------

#[test]
fn marlin_full_job_ping_pong_without_polling() {
    let device = DeviceProfile::for_kind(DeviceKind::Marlin);
    let lines = gcode_of(&two_squares(), &device);
    assert!(lines.iter().any(|l| l == "M3 S0 I"), "{lines:?}");
    let link = LinkProfile::for_device(&device);
    let (h, run) = run(Profile::marlin(), link, &lines);
    assert_eq!(run.outcome, Some(Outcome::Completed { errors: 0 }));
    assert_eq!(h.state(), StateKind::Idle);
    assert_eq!(h.mpos(), [0.0; 3]);
    // Ping-pong: nunca más de una línea sin responder; sin `?`, `!`, `~` ni `0x18`.
    assert!(h.max_in_flight() <= 96, "{}", h.max_in_flight());
    assert_eq!(h.rx_overflow_bytes(), 0);
    assert!(h.realtime_bytes().is_empty(), "{:?}", h.realtime_bytes());
    // Los espacios se conservan: `M3 S0 I` no se convierte en `M3S0I`.
    let got = h.received_lines();
    assert!(got.contains(&"M3 S0 I".to_string()), "{got:?}");
    assert!(got.iter().all(|l| !l.contains('?')), "{got:?}");
    // La última línea del dialecto (`M400`) sincroniza el fin y no se repite.
    assert_eq!(got.last().map(String::as_str), Some("M400"));
    assert_eq!(got.iter().filter(|l| *l == "M400").count(), 1);
    // Sin `M114` durante el trabajo (provoca tirones).
    assert!(got.iter().all(|l| !l.starts_with("M114")), "{got:?}");
    // El progreso sale de los `ok`.
    let last = run
        .events
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::Progress { acked, total, .. } => Some((*acked, *total)),
            _ => None,
        })
        .unwrap();
    assert_eq!(last.0, last.1);
    assert!(!run.events.iter().any(|e| matches!(e, Event::Status(_))));
    // Dos cuadrados de 20 mm: 160 mm de corte, y ni un milímetro quemado en el G0 entre ambos.
    assert_clean_trace(&h, 160.0);
    let peak = h
        .trace()
        .segments()
        .iter()
        .map(|s| s.s_peak)
        .fold(0.0, f64::max);
    assert!((peak - 204.0).abs() < 1e-9, "80 % de 255 = 204, fue {peak}");
}

#[test]
fn smoothie_full_job_ping_pong() {
    let device = DeviceProfile::for_kind(DeviceKind::Smoothie);
    let lines = gcode_of(&two_squares(), &device);
    let link = LinkProfile::for_device(&device);
    let (h, run) = run(Profile::smoothie(), link, &lines);
    assert_eq!(run.outcome, Some(Outcome::Completed { errors: 0 }));
    assert_eq!(h.state(), StateKind::Idle);
    assert_eq!(h.mpos(), [0.0; 3]);
    assert!(h.realtime_bytes().is_empty(), "{:?}", h.realtime_bytes());
    assert!(h.max_in_flight() < 100, "{}", h.max_in_flight());
    // S va de 0 a 1 en Smoothie.
    let peak = h
        .trace()
        .segments()
        .iter()
        .map(|s| s.s_peak)
        .fold(0.0, f64::max);
    assert!((peak - 0.8).abs() < 1e-9, "{peak}");
    assert_clean_trace(&h, 160.0);
}

#[test]
fn the_job_without_a_trailing_m400_gets_one_from_the_streamer() {
    let lines: Vec<String> = [
        "G90",
        "G21",
        "M3 S0 I",
        "G0 X10 Y10 F6000",
        "G1 X20 S128 F600",
        "M5",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    let (h, run) = run(Profile::marlin(), marlin_link(), &lines);
    assert_eq!(run.outcome, Some(Outcome::Completed { errors: 0 }));
    let got = h.received_lines();
    assert_eq!(got.last().map(String::as_str), Some("M400"));
    // El trabajo terminó de verdad: la cabeza ya está en el destino cuando `Completed` llega.
    assert_eq!(h.state(), StateKind::Idle);
    assert_eq!(h.mpos()[0], 20.0);
    // El `ok` de `M400` no cuenta como línea del trabajo.
    let total = run
        .events
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::Progress { acked, total, .. } => Some((*acked, *total)),
            _ => None,
        })
        .unwrap();
    assert_eq!(total, (6, 6));
}

#[test]
fn the_stream_api_builds_from_a_device_profile() {
    for (kind, profile) in [
        (DeviceKind::Marlin, Profile::marlin()),
        (DeviceKind::Smoothie, Profile::smoothie()),
        (DeviceKind::Grbl, Profile::grbl_1_1h()),
    ] {
        let device = DeviceProfile::for_kind(kind);
        let lines = gcode_of(&two_squares(), &device);
        let (vm, h) = scaled_vm(profile, 100.0, |b| b);
        let mut streamer = Streamer::for_device(vm, &device);
        let out = streamer.run(&lines, |_| {}).unwrap();
        assert_eq!(out, Outcome::Completed { errors: 0 }, "{kind:?}");
        assert_clean_trace(&h, 160.0);
        let polls = h.realtime_bytes().contains(&b'?');
        assert_eq!(polls, kind == DeviceKind::Grbl, "{kind:?}");
    }
}

// ---------------------------------------------------------------------------
// El láser no quema en G0 (tampoco en Marlin)
// ---------------------------------------------------------------------------

#[test]
fn marlin_dialects_never_burn_in_rapids() {
    let cases: [(&str, GcodeDialect, Profile); 3] = [
        ("inline", GcodeDialect::marlin_inline(), Profile::marlin()),
        (
            "estándar",
            GcodeDialect::marlin_standard(),
            Profile::marlin(),
        ),
        (
            "ventilador",
            GcodeDialect::marlin_fan(),
            Profile::marlin_fan_laser(),
        ),
    ];
    for (name, dialect, profile) in cases {
        let device = device_with(DeviceKind::Marlin, dialect.clone());
        let lines = gcode_of(&two_squares(), &device);
        let link = LinkProfile::marlin(&dialect);
        let (h, run) = run(profile, link, &lines);
        assert_eq!(
            run.outcome,
            Some(Outcome::Completed { errors: 0 }),
            "{name}"
        );
        let t = h.trace();
        assert!(
            t.burned_during_rapids().is_empty(),
            "{name}: quemó en un G0"
        );
        assert!(
            (t.total_length() - 160.0).abs() < 0.2,
            "{name}: {}",
            t.total_length()
        );
        assert!(max_distance(&t, &outlines()) < 0.02, "{name}");
        assert!(!h.laser_on(), "{name}: el láser quedó encendido");
        assert_eq!(h.mpos(), [0.0; 3], "{name}");
    }
}

#[test]
fn a_fan_laser_without_the_explicit_off_does_burn_in_g0() {
    // Control negativo: si el generador no apagara el ventilador antes del G0, el emulador lo
    // vería (en este firmware G0 no apaga el láser).
    let lines: Vec<String> = [
        "G90",
        "G21",
        "M106 P1 S128",
        "G1 X10 F600",
        "G0 X30 F6000",
        "M107 P1",
        "M400",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    let (h, _) = run(
        Profile::marlin_fan_laser(),
        LinkProfile::marlin(&GcodeDialect::marlin_fan()),
        &lines,
    );
    assert!(!h.trace().burned_during_rapids().is_empty());
}

#[test]
fn marlin_g0_shares_the_feed_with_g1() {
    // `G0 X30 F600`: en Marlin el G0 va a 600 mm/min (10 mm/s), no a la velocidad máxima.
    let mut marlin = Rig::new(Profile::marlin());
    marlin.drain();
    assert_eq!(marlin.ask("G0 X30 F600"), ["ok"]);
    marlin.ms(1000);
    let x = marlin.h.mpos()[0];
    assert!(x > 5.0 && x < 12.0, "en 1 s a 10 mm/s: {x}");
    // Smoothie tiene su propia velocidad de G0: a la máxima.
    let mut smoothie = Rig::new(Profile::smoothie());
    smoothie.drain();
    assert_eq!(smoothie.ask("G0 X30 F600"), ["ok"]);
    smoothie.ms(1000);
    assert_eq!(smoothie.h.mpos()[0], 30.0);
}

#[test]
fn inline_marlin_and_smoothie_do_not_fire_while_stationary() {
    for (profile, power) in [(Profile::marlin(), "S100"), (Profile::smoothie(), "S0.5")] {
        let mut r = Rig::new(profile);
        r.drain();
        let m3 = if r.h.with(|m| m.profile().name.starts_with("Marlin")) {
            "M3 S0 I"
        } else {
            "M3 S0"
        };
        assert_eq!(r.ask(m3), ["ok"]);
        r.line(&format!("G1 X5 {power} F6000"));
        r.ms(500);
        assert!(
            !r.h.laser_on(),
            "tras el movimiento el láser no dispara parado"
        );
    }
}

// ---------------------------------------------------------------------------
// Pausa
// ---------------------------------------------------------------------------

fn pause_resume_run(
    profile: Profile,
    link: LinkProfile,
    lines: &[String],
) -> (Handle, JobRun, [usize; 3]) {
    let (vm, h) = scaled_vm(profile, 30.0, |b| b);
    let probe = h.clone();
    let cfg = config_for(&link);
    let mut phase = 0;
    let mut paused_at = Instant::now();
    let mut counts = [0usize; 3];
    let run = run_job_link(vm, cfg, link, lines, DEADLINE, |c| match phase {
        0 if probe.lines_received() >= 30 => {
            c.pause();
            paused_at = Instant::now();
            phase = 1;
        }
        1 if paused_at.elapsed() > Duration::from_millis(150) => {
            counts[0] = probe.lines_received();
            phase = 2;
        }
        2 if paused_at.elapsed() > Duration::from_millis(400) => {
            counts[1] = probe.lines_received();
            counts[2] = usize::from(probe.state() == StateKind::Idle);
            c.resume();
            phase = 3;
        }
        _ => {}
    });
    (h, run, counts)
}

#[test]
fn marlin_pause_stops_sending_and_resume_continues() {
    let lines = marlin_circle();
    let (h, run, counts) = pause_resume_run(Profile::marlin(), marlin_link(), &lines);
    assert_eq!(run.outcome, Some(Outcome::Completed { errors: 0 }));
    // Pausado: no llegan líneas nuevas (la cola de la máquina se vació sola) y no se mandó `!`.
    assert!(
        counts[0] >= 30 && counts[0] < lines.len() - 20,
        "{counts:?}"
    );
    assert_eq!(counts[0], counts[1], "siguieron llegando líneas en pausa");
    assert_eq!(counts[2], 1, "la máquina debía haber vaciado su cola");
    assert!(h.realtime_bytes().is_empty());
    // Terminó el círculo entero, sin quemar en los G0 y con el láser apagado.
    assert_eq!(h.mpos(), [0.0; 3]);
    let t = h.trace();
    assert!(t.burned_during_rapids().is_empty());
    assert!((t.total_length() - 2.0 * std::f64::consts::PI * 15.0).abs() < 0.5);
    assert!(!h.laser_on());
}

#[test]
fn smoothie_pause_stops_sending_and_resume_continues() {
    let lines = smoothie_circle();
    let (h, run, counts) = pause_resume_run(Profile::smoothie(), smoothie_link(), &lines);
    assert_eq!(run.outcome, Some(Outcome::Completed { errors: 0 }));
    assert!(
        counts[0] >= 30 && counts[0] < lines.len() - 20,
        "{counts:?}"
    );
    assert_eq!(counts[0], counts[1]);
    assert!(h.realtime_bytes().is_empty());
    assert_eq!(h.mpos(), [0.0; 3]);
    assert!(h.trace().burned_during_rapids().is_empty());
}

#[test]
fn marlin_with_emergency_parser_pauses_with_m0_and_resumes_with_m108() {
    let link = marlin_link().with_emergency_parser(true);
    let lines = marlin_circle();
    let (h, run, counts) = pause_resume_run(Profile::marlin_emergency_parser(), link, &lines);
    assert_eq!(run.outcome, Some(Outcome::Completed { errors: 0 }));
    assert_eq!(counts[0], counts[1], "{counts:?}");
    let got = h.received_lines();
    let m0 = got.iter().position(|l| l == "M0").expect("M0");
    let m108 = got.iter().position(|l| l == "M108").expect("M108");
    assert!(m0 < m108);
    // Las líneas auxiliares no cuentan como avance del trabajo.
    let last = run
        .events
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::Progress { acked, total, .. } => Some((*acked, *total)),
            _ => None,
        })
        .unwrap();
    assert_eq!(last, (lines.len(), lines.len()));
    assert_eq!(h.mpos(), [0.0; 3]);
}

// ---------------------------------------------------------------------------
// Cancelación
// ---------------------------------------------------------------------------

/// Cancela el trabajo cuando la máquina lleva `after` líneas.
fn cancel_run(
    profile: Profile,
    link: LinkProfile,
    lines: &[String],
    after: usize,
) -> (Handle, JobRun) {
    let (vm, h) = scaled_vm(profile, 30.0, |b| b);
    let probe = h.clone();
    let cfg = config_for(&link);
    let mut done = false;
    let run = run_job_link(vm, cfg, link, lines, DEADLINE, |c| {
        if !done && probe.lines_received() >= after {
            c.cancel();
            done = true;
        }
    });
    assert!(!run.panicked && !run.timed_out);
    (h, run)
}

fn assert_stopped_midway(h: &Handle) {
    assert_eq!(h.planner_used(), 0);
    let at = h.mpos();
    assert!(
        at != [0.0; 3] && at[0] > 0.0,
        "debía estar a medio círculo: {at:?}"
    );
    assert!(
        h.trace().total_length() < 2.0 * std::f64::consts::PI * 15.0 - 5.0,
        "el círculo se completó pese a cancelar"
    );
    assert!(!h.laser_on(), "el láser quedó encendido tras cancelar");
}

#[test]
fn marlin_cancel_sends_m410_and_turns_the_laser_off() {
    let lines = marlin_circle();
    let (h, run) = cancel_run(Profile::marlin(), marlin_link(), &lines, 40);
    assert_eq!(run.outcome, Some(Outcome::Cancelled));
    let got = h.received_lines();
    assert_eq!(
        &got[got.len() - 2..],
        ["M410", "M5"],
        "{:?}",
        &got[got.len() - 4..]
    );
    assert!(
        h.realtime_bytes().is_empty(),
        "Marlin no entiende 0x18 ni '!'"
    );
    assert_stopped_midway(&h);
    let stop = run.streamer.as_ref().unwrap().last_stop().unwrap();
    assert!(stop.position_maybe_lost && !stop.settled);
    assert!(run.events.iter().any(|e| matches!(
        e,
        Event::Message(m) if m.contains("parada rápida")
    )));
}

#[test]
fn marlin_cancel_with_emergency_parser() {
    let lines = marlin_circle();
    let link = marlin_link().with_emergency_parser(true);
    let (h, run) = cancel_run(Profile::marlin_emergency_parser(), link, &lines, 40);
    assert_eq!(run.outcome, Some(Outcome::Cancelled));
    let got = h.received_lines();
    assert_eq!(&got[got.len() - 2..], ["M410", "M5"]);
    assert_stopped_midway(&h);
}

#[test]
fn marlin_standard_mode_cancel_needs_the_m5() {
    // En modo estándar `M3 S128` deja el láser encendido aunque la cabeza pare.
    let lines = circle(
        &["G90", "G21", "G0 X45 Y30 F6000", "M3 S128"],
        "F600",
        &["M5", "G0 X0 Y0 F6000"],
    );
    let link = LinkProfile::marlin(&GcodeDialect::marlin_standard());
    let (h, run) = cancel_run(Profile::marlin(), link, &lines, 40);
    assert_eq!(run.outcome, Some(Outcome::Cancelled));
    assert_stopped_midway(&h);
    assert_eq!(h.received_lines().last().map(String::as_str), Some("M5"));
}

#[test]
fn marlin_fan_laser_cancel_uses_the_dialect_off() {
    let lines = circle(
        &["G90", "G21", "G0 X45 Y30 F6000", "M106 P1 S128"],
        "F600",
        &["M107 P1", "G0 X0 Y0 F6000"],
    );
    let link = LinkProfile::marlin(&GcodeDialect::marlin_fan());
    let (h, run) = cancel_run(Profile::marlin_fan_laser(), link, &lines, 40);
    assert_eq!(run.outcome, Some(Outcome::Cancelled));
    let got = h.received_lines();
    assert_eq!(&got[got.len() - 2..], ["M410", "M107 P1"]);
    assert_eq!(h.with(|m| m.fan_speed(1)), 0.0);
    assert_stopped_midway(&h);
}

#[test]
fn smoothie_cancel_sends_m112_m999_and_leaves_the_machine_usable() {
    let lines = smoothie_circle();
    let (h, run) = cancel_run(Profile::smoothie(), smoothie_link(), &lines, 40);
    assert_eq!(run.outcome, Some(Outcome::Cancelled));
    let got = h.received_lines();
    assert_eq!(&got[got.len() - 3..], ["M112", "M999", "M5"]);
    assert!(!h.with(|m| m.is_halted()), "M999 debía limpiar el paro");
    assert_stopped_midway(&h);
    // Y acepta otro trabajo.
    let mut streamer = run.streamer.unwrap();
    let next: Vec<String> = ["G90", "G21", "G0 X5 Y5 F6000", "M400"]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    assert_eq!(
        streamer.run(&next, |_| {}).unwrap(),
        Outcome::Completed { errors: 0 }
    );
    assert_eq!(h.mpos()[0], 5.0);
}

// ---------------------------------------------------------------------------
// Errores y reinicios
// ---------------------------------------------------------------------------

#[test]
fn marlin_reboot_mid_job_is_reported_as_a_reset() {
    let lines = marlin_circle();
    let (vm, _h) = scaled_vm(Profile::marlin(), 30.0, |b| {
        b.fault(Trigger::Line(20), Fault::Reboot)
    });
    let link = marlin_link();
    let run = run_job_link(vm, config_for(&link), link, &lines, DEADLINE, |_| {});
    assert!(
        matches!(run.outcome, Some(Outcome::Reset(ref m)) if m == "start"),
        "{:?}",
        run.outcome
    );
}

#[test]
fn marlin_halt_error_ends_the_job() {
    let mut lines = marlin_circle();
    lines.insert(10, "M112".into());
    let (_h, run) = run_halted(&lines);
    assert!(
        matches!(&run, Some(Outcome::Reset(m)) if m.contains("halted")),
        "{run:?}"
    );
}

fn run_halted(lines: &[String]) -> (Handle, Option<Outcome>) {
    let (vm, h) = scaled_vm(Profile::marlin(), 30.0, |b| b);
    let link = marlin_link();
    let run = run_job_link(vm, config_for(&link), link, lines, DEADLINE, |_| {});
    (h, run.outcome)
}

// ---------------------------------------------------------------------------
// Líneas numeradas con checksum
// ---------------------------------------------------------------------------

#[test]
fn numbered_lines_resend_after_a_corrupted_line() {
    let lines: Vec<String> = [
        "G90",
        "G21",
        "M3 S0 I",
        "G0 X10 Y10 F6000",
        "G1 X20 S128 F600",
        "G1 Y20",
        "G1 X10",
        "G1 Y10",
        "M5",
        "M400",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    let link = marlin_link().with_line_numbering();
    // La línea 6 que ve la máquina es la `N5` (la primera es `N0 M110 N0`).
    let (vm, h) = scaled_vm(Profile::marlin(), 100.0, |b| {
        b.fault(Trigger::Line(6), Fault::CorruptLine)
    });
    let run = run_job_link(vm, config_for(&link), link, &lines, DEADLINE, |_| {});
    assert_eq!(
        run.outcome,
        Some(Outcome::Completed { errors: 0 }),
        "{:?}",
        run.events
    );
    let got = h.received_lines();
    // Todas llevan número y checksum; la primera fija el contador.
    assert!(
        got.iter().all(|l| l.starts_with('N') && l.contains('*')),
        "{got:?}"
    );
    assert!(got[0].starts_with("N0 M110 N0*"), "{got:?}");
    // La N5 llegó dos veces: la alterada y el reenvío.
    assert_eq!(
        got.iter().filter(|l| l.starts_with("N5 ")).count(),
        2,
        "{got:?}"
    );
    assert!(
        run.events
            .iter()
            .any(|e| matches!(e, Event::Message(m) if m.starts_with("Resend:")))
    );
    assert!(
        run.events
            .iter()
            .any(|e| matches!(e, Event::Message(m) if m.contains("checksum mismatch")))
    );
    // Nada se ejecutó dos veces ni se perdió: 4 mm... el cuadrado de 10 mm tiene 40 mm.
    assert_eq!(h.with(|m| m.last_line_number()), lines.len() as i64);
    let t = h.trace();
    assert!(
        (t.total_length() - 40.0).abs() < 0.1,
        "{}",
        t.total_length()
    );
    // El avance no cuenta la línea propia `M110`.
    let last = run
        .events
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::Progress { acked, total, .. } => Some((*acked, *total)),
            _ => None,
        })
        .unwrap();
    assert_eq!(last, (lines.len(), lines.len()));
}

#[test]
fn numbered_lines_without_noise_run_straight_through() {
    let device = DeviceProfile::for_kind(DeviceKind::Marlin);
    let lines = gcode_of(&two_squares(), &device);
    let link = LinkProfile::for_device(&device).with_line_numbering();
    let (h, run) = run(Profile::marlin(), link, &lines);
    assert_eq!(run.outcome, Some(Outcome::Completed { errors: 0 }));
    assert_clean_trace(&h, 160.0);
    assert!(
        !run.events
            .iter()
            .any(|e| matches!(e, Event::Message(m) if m.contains("Resend")))
    );
}

#[test]
fn a_line_that_never_gets_through_gives_up() {
    // Un cable que corrompe cada reenvío: tras varios intentos se abandona con un mensaje.
    let lines: Vec<String> = ["G90", "G21", "G0 X1 F6000", "M400"]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let link = marlin_link().with_line_numbering();
    let (vm, _h) = scaled_vm(Profile::marlin(), 100.0, |b| {
        (3..=40).fold(b, |b, n| b.fault(Trigger::Line(n), Fault::CorruptLine))
    });
    let run = run_job_link(vm, config_for(&link), link, &lines, DEADLINE, |_| {});
    assert!(
        matches!(&run.outcome, Some(Outcome::Disconnected(m)) if m.contains("reenvió")),
        "{:?}",
        run.outcome
    );
}

// ---------------------------------------------------------------------------
// Emulador: M106/M107, M410/M112, checksum, Smoothie `?`
// ---------------------------------------------------------------------------

#[test]
fn emulator_fans_and_laser_on_a_fan() {
    let mut r = Rig::new(Profile::marlin());
    r.drain();
    assert_eq!(r.ask("M106 P1 S128"), ["ok"]);
    assert_eq!(r.h.with(|m| m.fan_speed(1)), 128.0);
    assert_eq!(r.ask("M106 S255"), ["ok"]);
    assert_eq!(r.h.with(|m| m.fan_speed(0)), 255.0);
    assert_eq!(r.ask("M107 P1"), ["ok"]);
    assert_eq!(r.h.with(|m| m.fan_speed(1)), 0.0);
    assert_eq!(r.h.with(|m| m.fan_speed(0)), 255.0);
    // Sin `laser_fan`, el ventilador no enciende el láser.
    r.ask("M106 P1 S200");
    r.line("G1 X5 F6000");
    r.ms(300);
    assert!(r.h.trace().is_empty());
    // Con `laser_fan` sí, también en parado.
    let mut f = Rig::new(Profile::marlin_fan_laser());
    f.drain();
    f.ask("M106 P1 S200");
    f.ms(5);
    assert!(f.h.laser_on());
    f.ask("M107 P1");
    assert!(!f.h.laser_on());
}

#[test]
fn emulator_m410_is_queued_and_m112_halts() {
    let mut r = Rig::new(Profile::marlin());
    r.drain();
    r.ask("G90");
    r.ask("G1 X100 F600");
    r.ms(500);
    assert_eq!(r.h.state(), StateKind::Run);
    assert_eq!(r.ask("M410"), ["ok"]);
    assert_eq!(r.h.state(), StateKind::Idle);
    let x = r.h.mpos()[0];
    r.ms(500);
    assert_eq!(r.h.mpos()[0], x, "tras M410 no sigue moviéndose");
    assert!(x > 1.0 && x < 20.0);
    // M112: el mensaje de Marlin y silencio hasta reiniciar.
    assert_eq!(r.ask("M112"), ["Error:Printer halted. kill() called!"]);
    assert!(r.ask("G90").is_empty());
    assert!(r.h.with(|m| m.is_halted()));
    r.h.inject(Trigger::Now, Fault::Reboot);
    assert_eq!(r.lines(), ["start", "echo:Marlin 2.1.2.5"]);
    assert_eq!(r.ask("G90"), ["ok"]);
}

#[test]
fn emulator_smoothie_m112_needs_m999() {
    let mut r = Rig::new(Profile::smoothie());
    r.drain();
    let l = r.ask("M112");
    assert!(l[0].starts_with("!!"), "{l:?}");
    assert!(r.ask("G90").is_empty());
    assert_eq!(r.ask("M999"), ["ok"]);
    assert_eq!(r.ask("G90"), ["ok"]);
}

#[test]
fn emulator_emergency_parser_acts_while_the_queue_is_blocked() {
    for (profile, immediate) in [
        (Profile::marlin(), false),
        (Profile::marlin_emergency_parser(), true),
    ] {
        let mut r = Rig::new(profile);
        r.drain();
        r.ask("G90");
        // Llena el planificador (16 bloques) con movimientos largos: la 17.ª línea se bloquea.
        for i in 1..=17 {
            r.line(&format!("G1 X{} F60", i * 10));
        }
        r.ms(20);
        r.line("M410");
        r.ms(5);
        let stopped = r.h.planner_used() == 0;
        assert_eq!(stopped, immediate, "M410 con la cola llena");
    }
}

#[test]
fn emulator_checksum_and_resend() {
    let mut r = Rig::new(Profile::marlin());
    r.drain();
    let line = |n: usize, text: &str| {
        tlaku_grbl::link::numbered_line(n, text, tlaku_grbl::link::LineNumbering)
    };
    assert_eq!(r.ask(&line(0, "M110 N0")), ["ok"]);
    assert_eq!(r.ask(&line(1, "G90")), ["ok"]);
    // Checksum erróneo.
    let bad = format!("{}0", line(2, "G21"));
    let got = r.ask(&bad);
    assert_eq!(
        got,
        ["Error:checksum mismatch, Last Line: 1", "Resend: 2", "ok"]
    );
    // Salto de numeración.
    let got = r.ask(&line(5, "G21"));
    assert_eq!(
        got,
        [
            "Error:Line Number is not Last Line Number+1, Last Line: 1",
            "Resend: 2",
            "ok"
        ]
    );
    // N sin checksum y checksum sin N.
    let got = r.ask("N2 G21");
    assert_eq!(got[0], "Error:No Checksum with line number, Last Line: 1");
    let got = r.ask("G21*12");
    assert_eq!(got[0], "Error:No Line Number with checksum, Last Line: 1");
    // La correcta sigue funcionando; las líneas sin numerar también.
    assert_eq!(r.ask(&line(2, "G21")), ["ok"]);
    assert_eq!(r.ask("G90"), ["ok"]);
    assert_eq!(r.h.with(|m| m.last_line_number()), 2);
    // Ejemplo de la wiki de RepRap: `N3 T0*57` es válida (T0 no está soportada: eco + ok).
    let got = r.ask("N3 T0*57");
    assert_eq!(got.last().map(String::as_str), Some("ok"));
    assert!(!got.iter().any(|l| l.starts_with("Error")), "{got:?}");
}

#[test]
fn emulator_smoothie_question_mark_reports_state_and_position() {
    let mut r = Rig::new(Profile::smoothie());
    r.drain();
    r.rt(b'?');
    let l = r.lines();
    assert_eq!(
        l,
        ["<Idle,MPos:0.0000,0.0000,0.0000,WPos:0.0000,0.0000,0.0000>"]
    );
    // Sin `ok`, y el `?` no entra en el búfer de líneas.
    assert_eq!(r.h.lines_received(), 0);
    r.ask("G1 X10 F600");
    r.ms(300);
    r.rt(b'?');
    let l = r.lines();
    assert!(l[0].starts_with("<Run,MPos:"), "{l:?}");
    let rep = tlaku_grbl::streamer::StatusReport::parse_legacy(&l[0]).unwrap();
    assert_eq!(rep.state, tlaku_grbl::streamer::MachineState::Run);
    assert!(rep.mpos.unwrap().x > 0.0);
    assert_eq!(rep.wpos.unwrap().y, 0.0);
    // Marlin no tiene `?`.
    let mut m = Rig::new(Profile::marlin());
    m.drain();
    m.rt(b'?');
    assert!(m.lines().is_empty());
}

// ---------------------------------------------------------------------------
// Comprobación previa
// ---------------------------------------------------------------------------

fn preflight(
    profile: Profile,
    link: &LinkProfile,
    expect: &JobExpect,
) -> (Result<(), String>, String) {
    let (mut vm, _h) = scaled_vm(profile, 50.0, |b| b);
    let cfg = PreflightConfig {
        status_timeout: Duration::from_secs(3),
        ..PreflightConfig::default()
    };
    let mut out = Vec::new();
    let r = preflight_link(&mut vm, &cfg, Advice::Console, expect, link, &mut out);
    (r, String::from_utf8(out).unwrap())
}

#[test]
fn preflight_marlin_and_smoothie_check_m115_and_warn_about_the_rest() {
    let expect = JobExpect {
        require_laser_mode: true,
        s_max: Some(255.0),
        check_parameters: false,
    };
    let (r, out) = preflight(Profile::marlin(), &marlin_link(), &expect);
    assert_eq!(r, Ok(()), "{out}");
    assert!(out.contains("Firmware: Marlin 2.1.2.5"), "{out}");
    assert!(out.contains("no tiene consulta de estado"), "{out}");
    assert!(out.contains("CUTTER_POWER_UNIT"), "{out}");
    assert!(out.contains("LASER_FEATURE"), "{out}");
    assert!(out.contains("sin EMERGENCY_PARSER"), "{out}");
    assert!(
        out.lines().filter(|l| l.starts_with("Aviso")).count() >= 4,
        "{out}"
    );

    let (r, out) = preflight(Profile::marlin_emergency_parser(), &marlin_link(), &expect);
    assert_eq!(r, Ok(()));
    assert!(out.contains("el enlace no lo usa"), "{out}");
    let (r, out) = preflight(
        Profile::marlin_emergency_parser(),
        &marlin_link().with_emergency_parser(true),
        &expect,
    );
    assert_eq!(r, Ok(()));
    assert!(
        !out.contains("sin EMERGENCY_PARSER") && !out.contains("no lo usa"),
        "{out}"
    );

    let (r, out) = preflight(Profile::smoothie(), &smoothie_link(), &expect);
    assert_eq!(r, Ok(()), "{out}");
    assert!(out.contains("Firmware: Smoothieware"), "{out}");
    assert!(out.contains("Maximum S-value"), "{out}");
}

#[test]
fn preflight_refuses_the_wrong_firmware_and_silence() {
    let expect = JobExpect::default();
    // Perfil Marlin sobre una Smoothie, y al revés.
    let (r, _) = preflight(Profile::smoothie(), &marlin_link(), &expect);
    assert!(r.unwrap_err().contains("no Marlin"));
    let (r, _) = preflight(Profile::marlin(), &smoothie_link(), &expect);
    assert!(r.unwrap_err().contains("no Smoothieware"));
    // Una GRBL: contesta error:20 a M115.
    let (r, _) = preflight(Profile::grbl_1_1h(), &marlin_link(), &expect);
    let e = r.unwrap_err();
    assert!(e.contains("no parece Marlin"), "{e}");
    // Silencio.
    let (mut host, _dev) = memory_pair();
    let cfg = PreflightConfig {
        status_timeout: Duration::from_millis(300),
        ..PreflightConfig::default()
    };
    let mut out = Vec::new();
    let e = preflight_link(
        &mut host,
        &cfg,
        Advice::Console,
        &expect,
        &marlin_link(),
        &mut out,
    )
    .unwrap_err();
    assert!(e.contains("no respondió a M115"), "{e}");
}

#[test]
fn preflight_with_a_grbl_link_is_the_historic_one() {
    let (r, out) = preflight(
        Profile::grbl_1_1h(),
        &LinkProfile::grbl(),
        &JobExpect::default(),
    );
    assert_eq!(r, Ok(()), "{out}");
    assert!(out.contains("Estado de la máquina: reposo"), "{out}");
}

#[test]
fn link_profile_status_choices() {
    assert_eq!(LinkProfile::grbl().status_job, StatusQuery::Realtime(b'?'));
    assert_eq!(marlin_link().status_job, StatusQuery::None);
    assert_eq!(smoothie_link().status_job, StatusQuery::None);
}
