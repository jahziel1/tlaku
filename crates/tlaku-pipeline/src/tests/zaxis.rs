//! Eje Z: pasadas, altura segura y valores no finitos.

use super::*;
use tlaku_doc::{DeviceProfile, JobOptions, LayerSettings};

fn z_device() -> DeviceProfile {
    DeviceProfile {
        z_axis: true,
        ..device()
    }
}

#[test]
fn z_step_makes_one_block_and_one_report_per_pass() {
    let l = LayerSettings {
        passes: 3,
        z_offset: 1.0,
        z_step: 0.5,
        ..line_layer(RED)
    };
    let d = doc(vec![rect_object("a", RED, 0.0, 0.0, 10.0, 10.0)], vec![l]);
    let job = compute(&d, &z_device(), &JobOptions::default()).unwrap();
    let zs: Vec<_> = job.toolpath.blocks.iter().map(|b| b.z_mm).collect();
    assert_eq!(zs, [Some(1.0), Some(1.5), Some(2.0)]);
    // 1:1 entre bloques, informes y filas del resumen.
    assert_eq!(job.layers.len(), 3);
    assert_eq!(job.preview.layers.len(), 3);
    assert!(job.preview.layers[1].summary.contains("pasada 2/3"));
    assert!(job.preview.layers[1].summary.contains("Z 1.5 mm"));
    assert!(job.layers.iter().all(|l| l.passes == 1));
    // El plan se muestra una sola vez.
    assert!(job.layers[0].plan.is_some() && job.layers[1].plan.is_none());
    assert!(job.toolpath.blocks.iter().all(|b| b.moves.len() == 5));
    // Z relativa: cada pasada baja (profundidad positiva = Z negativa de GRBL) y vuelve a la
    // altura inicial; lo emitido suma 0.
    let g = gcode(&job, &z_device());
    let zlines: Vec<_> = g.lines().filter(|l| l.contains('Z')).collect();
    assert_eq!(
        zlines,
        ["G0 Z-1", "G0 Z1", "G0 Z-1.5", "G0 Z1.5", "G0 Z-2", "G0 Z2"]
    );
    // Con z_invert el signo de todas las palabras Z se invierte, y nada más cambia.
    let inverted = DeviceProfile {
        z_invert: true,
        ..z_device()
    };
    let gi = gcode(&job, &inverted);
    assert_eq!(
        gi.lines().filter(|l| l.contains('Z')).collect::<Vec<_>>(),
        ["G0 Z1", "G0 Z-1", "G0 Z1.5", "G0 Z-1.5", "G0 Z2", "G0 Z-2"]
    );
    let strip = |g: &str| {
        g.lines()
            .filter(|l| !l.contains('Z'))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(strip(&g), strip(&gi));
    // Las tres pasadas suman lo mismo que una capa de tres pasadas.
    let flat = compute(
        &doc(
            vec![rect_object("a", RED, 0.0, 0.0, 10.0, 10.0)],
            vec![LayerSettings {
                passes: 3,
                ..line_layer(RED)
            }],
        ),
        &device(),
        &JobOptions::default(),
    )
    .unwrap();
    let moves = |j: &ComputedJob| {
        j.toolpath
            .blocks
            .iter()
            .map(|b| b.moves.len())
            .sum::<usize>()
    };
    assert_eq!(moves(&job), moves(&flat));
}

#[test]
fn z_without_step_keeps_the_passes_in_one_block() {
    let l = LayerSettings {
        passes: 2,
        z_offset: 2.5,
        ..fill_layer(RED)
    };
    let d = doc(vec![rect_object("a", RED, 5.0, 5.0, 10.0, 10.0)], vec![l]);
    let job = compute(&d, &z_device(), &JobOptions::default()).unwrap();
    assert_eq!(job.toolpath.blocks.len(), 1);
    assert_eq!(job.toolpath.blocks[0].z_mm, Some(2.5));
    assert_eq!(job.layers[0].passes, 2);
    // Con eje Z y todo en 0 el foco se queda donde está: Z relativa, nada que mover.
    let d = doc(
        vec![rect_object("a", RED, 0.0, 0.0, 10.0, 10.0)],
        vec![line_layer(RED)],
    );
    let job = compute(&d, &z_device(), &JobOptions::default()).unwrap();
    assert_eq!(job.toolpath.blocks[0].z_mm, Some(0.0));
    let g = gcode(&job, &z_device());
    assert!(!g.contains('Z') && !g.contains("G91"), "{g}");
}

#[test]
fn z_values_without_a_z_axis_warn_and_emit_nothing() {
    let l = LayerSettings {
        passes: 2,
        z_offset: 1.0,
        z_step: 0.5,
        ..line_layer(RED)
    };
    let d = doc(vec![rect_object("a", RED, 0.0, 0.0, 10.0, 10.0)], vec![l]);
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    assert_eq!(job.toolpath.blocks.len(), 1);
    assert_eq!(job.toolpath.blocks[0].z_mm, None);
    assert!(
        job.preview.warnings.iter().any(|w| w.contains("eje Z")),
        "{:?}",
        job.preview.warnings
    );
    assert!(!gcode(&job, &device()).contains('Z'));
    // Sin valores de Z no hay nada que avisar.
    let d = doc(
        vec![rect_object("a", RED, 0.0, 0.0, 10.0, 10.0)],
        vec![line_layer(RED)],
    );
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    assert!(job.preview.warnings.is_empty());
}

#[test]
fn non_finite_z_is_an_error() {
    let l = LayerSettings {
        z_step: f64::INFINITY,
        ..line_layer(RED)
    };
    let d = doc(vec![rect_object("a", RED, 0.0, 0.0, 10.0, 10.0)], vec![l]);
    let e = compute(&d, &z_device(), &JobOptions::default()).unwrap_err();
    assert!(e.contains("z_step"), "{e}");
}

#[test]
fn z_returns_to_the_start_height_before_the_origin_move() {
    let l = LayerSettings {
        passes: 2,
        z_offset: 1.0,
        z_step: 0.5,
        ..line_layer(RED)
    };
    let d = doc(vec![rect_object("a", RED, 5.0, 5.0, 10.0, 10.0)], vec![l]);
    let g = gcode(
        &compute(&d, &z_device(), &JobOptions::default()).unwrap(),
        &z_device(),
    );
    let lines: Vec<&str> = g.lines().collect();
    // Z es siempre relativa (G91 / G0 Z / G90), solo G0 y sin F ni S, y suma 0.
    let z: Vec<f64> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("G0 Z"))
        .map(|v| v.parse().unwrap())
        .collect();
    assert_eq!(z.iter().sum::<f64>(), 0.0);
    assert_eq!(lines.iter().filter(|l| **l == "G91").count(), z.len());
    assert_eq!(lines.iter().filter(|l| **l == "G90").count(), z.len() + 1);
    assert!(
        lines
            .iter()
            .filter(|l| l.contains('Z'))
            .all(|l| l.starts_with("G0 Z"))
    );
    // La última subida llega antes del regreso al origen.
    let last_z = lines.iter().rposition(|l| l.starts_with("G0 Z")).unwrap();
    let origin = lines.iter().position(|l| *l == "G0 X0 Y0").unwrap();
    assert!(last_z < origin, "{g}");
}
