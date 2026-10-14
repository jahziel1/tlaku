//! Rotativo: escala de Y, marco, colocación, límites y espejo.

use super::*;
use tlaku_doc::{DeviceProfile, JobOptions, LayerSettings, StartFrom};

fn rotary_device(kind: tlaku_doc::RotaryKind) -> DeviceProfile {
    let mut d = device();
    d.rotary.enabled = true;
    d.rotary.kind = kind;
    d.rotary.mm_per_rotation = 360.0;
    d.rotary.roller_diameter = 20.0;
    d.rotary.object_diameter = 80.0;
    d
}

fn y_of(g: &str, prefix: &str) -> f64 {
    let l = g.lines().find(|l| l.starts_with(prefix)).unwrap();
    let y = l.split(' ').find(|w| w.starts_with('Y')).unwrap();
    y[1..].parse().unwrap()
}

#[test]
fn rotary_scales_y_by_roller_or_chuck() {
    let d = doc(
        vec![rect_object("a", RED, 0.0, 0.0, 10.0, 10.0)],
        vec![line_layer(RED)],
    );
    let roller = rotary_device(tlaku_doc::RotaryKind::Roller);
    let job = compute(&d, &roller, &JobOptions::default()).unwrap();
    let g = gcode(&job, &roller);
    // 10 mm de superficie * 360 / (pi * 20) con 3 decimales.
    let k = 360.0 / (std::f64::consts::PI * 20.0);
    assert!((y_of(&g, "G1 Y") - 10.0 * k).abs() < 1e-3, "{g}");
    let chuck = rotary_device(tlaku_doc::RotaryKind::Chuck);
    let g = gcode(&job, &chuck);
    let k = 360.0 / (std::f64::consts::PI * 80.0);
    assert!((y_of(&g, "G1 Y") - 10.0 * k).abs() < 1e-3, "{g}");
    // El trabajo calculado sigue en mm de superficie.
    approx(job.bounds().unwrap().max.y, 10.0);
}

#[test]
fn rotary_frame_uses_the_same_scale() {
    let d = doc(
        vec![rect_object("a", RED, 0.0, 0.0, 10.0, 10.0)],
        vec![line_layer(RED)],
    );
    let dev = rotary_device(tlaku_doc::RotaryKind::Roller);
    let job = compute(&d, &dev, &JobOptions::default()).unwrap();
    let k = 360.0 / (std::f64::consts::PI * 20.0);
    let frame = frame_gcode(&job, &dev, 1500.0);
    let top = frame
        .iter()
        .filter(|l| l.starts_with("G1 X10 Y"))
        .nth(1)
        .unwrap();
    let y: f64 = top.split(' ').find(|w| w.starts_with('Y')).unwrap()[1..]
        .parse()
        .unwrap();
    assert!((y - 10.0 * k).abs() < 1e-3, "{frame:?}");
    // Sin rotativo, el encuadre queda como antes.
    assert!(frame_gcode(&job, &device(), 1500.0).contains(&"G1 X10 Y10 S0 F1500".to_string()));
}

#[test]
fn rotary_places_the_job_from_controller_coordinates() {
    let d = doc(
        vec![rect_object("a", RED, 0.0, 0.0, 10.0, 10.0)],
        vec![line_layer(RED)],
    );
    let dev = rotary_device(tlaku_doc::RotaryKind::Roller);
    let k = 360.0 / (std::f64::consts::PI * 20.0);
    // El origen de usuario llega en mm de la controladora: 2 * k equivale a 2 mm de superficie.
    let opts = JobOptions {
        start_from: StartFrom::UserOrigin,
        job_origin: 6,
        user_origin: Some([5.0, 2.0 * k]),
        ..Default::default()
    };
    let job = compute(&d, &dev, &opts).unwrap();
    let b = job.bounds().unwrap();
    point(b.min, 5.0, 2.0);
    let g = gcode(&job, &dev);
    assert!(
        g.contains(&format!("G0 X5 Y{}", (2.0 * k * 1000.0).round() / 1000.0)),
        "{g}"
    );
}

#[test]
fn rotary_validates_and_warns_about_the_circumference() {
    let d = doc(
        vec![rect_object("a", RED, 0.0, 0.0, 10.0, 50.0)],
        vec![line_layer(RED)],
    );
    for edit in [
        |d: &mut DeviceProfile| d.rotary.mm_per_rotation = 0.0,
        |d: &mut DeviceProfile| d.rotary.roller_diameter = 0.0,
    ] {
        let mut dev = rotary_device(tlaku_doc::RotaryKind::Roller);
        edit(&mut dev);
        let e = compute(&d, &dev, &JobOptions::default()).unwrap_err();
        assert!(e.contains("rotativo") && e.contains("mayor que 0"), "{e}");
    }
    // El diámetro de la pieza solo se exige con mandril; con rodillo no afecta al trabajo y,
    // si no es válido, solo falta el aviso de la circunferencia.
    let mut chuck = rotary_device(tlaku_doc::RotaryKind::Chuck);
    chuck.rotary.object_diameter = -5.0;
    let e = compute(&d, &chuck, &JobOptions::default()).unwrap_err();
    assert!(e.contains("rotativo") && e.contains("la pieza"), "{e}");
    let mut roller = rotary_device(tlaku_doc::RotaryKind::Roller);
    roller.rotary.object_diameter = -5.0;
    let job = compute(&d, &roller, &JobOptions::default()).unwrap();
    assert!(
        job.preview
            .warnings
            .iter()
            .all(|w| !w.contains("circunferencia"))
    );
    let mut other = roller.clone();
    other.rotary.object_diameter = 300.0;
    assert_eq!(
        gcode(&job, &roller),
        gcode(
            &compute(&d, &other, &JobOptions::default()).unwrap(),
            &other
        )
    );
    // Con la pieza de 10 mm de diámetro (circunferencia 31.4) un diseño de 50 mm no cabe.
    let mut dev = rotary_device(tlaku_doc::RotaryKind::Chuck);
    dev.rotary.object_diameter = 10.0;
    let job = compute(&d, &dev, &JobOptions::default()).unwrap();
    let w = job.preview.warnings.join("\n");
    assert!(w.contains("circunferencia") && w.contains("31.42"), "{w}");
    // Con 80 mm de diámetro sí cabe.
    let ok = compute(
        &d,
        &rotary_device(tlaku_doc::RotaryKind::Chuck),
        &JobOptions::default(),
    )
    .unwrap();
    assert!(ok.preview.warnings.is_empty());
}

#[test]
fn rotary_has_no_y_limit_but_the_bed_x_limit_stays() {
    let tall = doc(
        vec![rect_object("a", RED, 0.0, 0.0, 10.0, 600.0)],
        vec![line_layer(RED)],
    );
    // Sin rotativo, 600 mm de alto no caben en la cama de 415.
    assert!(compute(&tall, &device(), &JobOptions::default()).is_err());
    let mut dev = rotary_device(tlaku_doc::RotaryKind::Chuck);
    dev.rotary.object_diameter = 400.0;
    assert!(compute(&tall, &dev, &JobOptions::default()).is_ok());
    let wide = doc(
        vec![rect_object("a", RED, 0.0, 0.0, 500.0, 10.0)],
        vec![line_layer(RED)],
    );
    let e = compute(&wide, &dev, &JobOptions::default()).unwrap_err();
    assert!(e.contains("se sale del área"), "{e}");
}

#[test]
fn rotary_scales_the_y_limits_of_the_estimate() {
    // Un segmento vertical: con rotativo la Y de la controladora recorre k veces más, así que
    // con la misma aceleración y velocidad máxima tarda más.
    let d = doc(
        vec![rect_object("a", RED, 0.0, 0.0, 1.0, 100.0)],
        vec![LayerSettings {
            speed: 12000.0,
            ..line_layer(RED)
        }],
    );
    let plain = compute(&d, &device(), &JobOptions::default()).unwrap();
    let mut dev = rotary_device(tlaku_doc::RotaryKind::Roller);
    dev.rotary.object_diameter = 400.0;
    let rot = compute(&d, &dev, &JobOptions::default()).unwrap();
    assert!(rot.preview.total_s > plain.preview.total_s * 2.0);
}

#[test]
fn rotary_mirror_reflects_y_inside_the_job_box() {
    let d = doc(
        vec![object_open(
            "a",
            RED,
            &[(0.0, 0.0), (5.0, 2.0), (5.0, 10.0)],
        )],
        vec![line_layer(RED)],
    );
    let mut dev = rotary_device(tlaku_doc::RotaryKind::Chuck);
    let job = compute(&d, &dev, &JobOptions::default()).unwrap();
    let plain = gcode(&job, &dev);
    dev.rotary.mirror = true;
    let mirrored = gcode(&job, &dev);
    let k = 360.0 / (std::f64::consts::PI * 80.0);
    let ys = |g: &str| -> Vec<f64> {
        g.lines()
            .filter(|l| l.starts_with("G0 X") || l.starts_with("G1"))
            .filter_map(|l| l.split(' ').find(|w| w.starts_with('Y')))
            .map(|w| w[1..].parse().unwrap())
            .collect()
    };
    // Y de superficie 0, 2, 10 sale como 10, 8, 0 (min + max - y) y luego se escala.
    let (a, b) = (ys(&plain), ys(&mirrored));
    assert!((a[0] - 0.0).abs() < 1e-3 && (a[1] - 2.0 * k).abs() < 1e-3);
    assert!((b[0] - 10.0 * k).abs() < 1e-3 && (b[1] - 8.0 * k).abs() < 1e-3);
    // El área que ocupa no cambia y el espejo sin rotativo no hace nada.
    approx(job.bounds().unwrap().max.y, 10.0);
    let mut flat = device();
    flat.rotary.mirror = true;
    let j = compute(&d, &flat, &JobOptions::default()).unwrap();
    assert_eq!(gcode(&j, &flat), gcode(&j, &device()));
}

#[test]
fn rotary_warns_when_the_controller_travel_exceeds_the_soft_limits() {
    let d = doc(
        vec![rect_object("a", RED, 0.0, 0.0, 10.0, 100.0)],
        vec![line_layer(RED)],
    );
    // 100 mm de superficie * 5.73 = 573 mm de Y de la controladora > 415 de la cama.
    let dev = rotary_device(tlaku_doc::RotaryKind::Roller);
    let job = compute(&d, &dev, &JobOptions::default()).unwrap();
    let w = job.preview.warnings.join("\n");
    assert!(w.contains("ALARM:2") && w.contains("$20=0"), "{w}");
    // Con una escala realista cabe y no hay aviso.
    let ok = compute(
        &d,
        &DeviceProfile {
            rotary: tlaku_doc::RotarySettings {
                enabled: true,
                ..Default::default()
            },
            ..device()
        },
        &JobOptions::default(),
    )
    .unwrap();
    assert!(ok.preview.warnings.is_empty(), "{:?}", ok.preview.warnings);
}

#[test]
fn rotary_one_turn_test_goes_through_the_normal_pipeline() {
    let mut dev = rotary_device(tlaku_doc::RotaryKind::Chuck);
    dev.rotary.mm_per_rotation = 40.0;
    dev.rotary.object_diameter = 75.0;
    let doc = rotary_test(&dev, 1200.0, 5.0).unwrap();
    let job = compute(&doc, &dev, &JobOptions::default()).unwrap();
    let b = job.bounds().unwrap();
    approx(b.max.y - b.min.y, std::f64::consts::PI * 75.0);
}
