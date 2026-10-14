//! Escala de potencia por objeto en capas fill.

use super::*;
use tlaku_core::toolpath::Move;
use tlaku_doc::{DesignObject, JobOptions, LayerSettings};

fn scaled(mut o: DesignObject, scale: f64) -> DesignObject {
    o.power_scale = Some(scale);
    o
}

/// Potencias (%) distintas de los `Cut` con láser de todos los bloques (sin el overscan), de menor a mayor.
fn cut_powers(job: &ComputedJob) -> Vec<f64> {
    let mut v: Vec<f64> = job
        .toolpath
        .blocks
        .iter()
        .flat_map(|b| &b.moves)
        .filter_map(|m| match m {
            Move::Cut { power_pct, .. } if *power_pct > 0.0 => Some(*power_pct),
            _ => None,
        })
        .collect();
    v.sort_by(f64::total_cmp);
    v.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    v
}

fn fill_at(power: f64) -> LayerSettings {
    LayerSettings {
        power,
        ..fill_layer(RED)
    }
}

fn warned(job: &ComputedJob, text: &str) -> bool {
    job.preview.warnings.iter().any(|w| w.contains(text))
}

#[test]
fn fill_applies_the_power_scale_of_a_single_scale_layer() {
    let d = doc(
        vec![scaled(rect_object("a", RED, 10.0, 10.0, 10.0, 10.0), 50.0)],
        vec![fill_at(40.0)],
    );
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    assert_eq!(cut_powers(&job), [20.0]);
    assert_eq!(job.layers[0].power, "20%");
    assert_eq!(job.toolpath.blocks.len(), 1);
    assert!(!warned(&job, "escalas de potencia"));
}

#[test]
fn fill_without_scales_keeps_the_layer_power() {
    let d = doc(
        vec![
            rect_object("a", RED, 10.0, 10.0, 10.0, 10.0),
            scaled(rect_object("b", RED, 30.0, 10.0, 10.0, 10.0), 100.0),
        ],
        vec![fill_at(40.0)],
    );
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    assert_eq!(cut_powers(&job), [40.0]);
    assert!(!warned(&job, "escalas de potencia"));
}

#[test]
fn fill_splits_objects_with_different_scales_into_one_block() {
    let d = doc(
        vec![
            scaled(rect_object("a", RED, 10.0, 10.0, 10.0, 10.0), 100.0),
            scaled(rect_object("b", RED, 40.0, 10.0, 10.0, 10.0), 50.0),
            scaled(rect_object("c", RED, 70.0, 10.0, 10.0, 10.0), 100.0),
        ],
        vec![fill_at(40.0)],
    );
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    assert_eq!(cut_powers(&job), [20.0, 40.0]);
    // Un solo bloque y una fila en el resumen, con el rango de potencias.
    assert_eq!(job.toolpath.blocks.len(), 1);
    assert_eq!(job.layers.len(), 1);
    assert_eq!(job.layers[0].power, "20-40%");
    assert!(warned(&job, "2 escalas de potencia distintas"));
    // No se superponen: no hay aviso de superposición.
    assert!(!warned(&job, "se superponen"));
    // Las líneas de cada escala cubren solo sus objetos: la de 50 % está entre x 40 y 50.
    let xs: Vec<f64> = job.toolpath.blocks[0]
        .moves
        .iter()
        .filter_map(|m| match m {
            Move::Cut { to, power_pct } if (*power_pct - 20.0).abs() < 1e-9 => Some(to.x),
            _ => None,
        })
        .collect();
    assert!(
        !xs.is_empty() && xs.iter().all(|x| (39.9..=50.1).contains(x)),
        "{xs:?}"
    );
}

#[test]
fn fill_clamps_scales_over_100_and_skips_scale_zero() {
    let d = doc(
        vec![
            scaled(rect_object("a", RED, 10.0, 10.0, 10.0, 10.0), 300.0),
            scaled(rect_object("b", RED, 40.0, 10.0, 10.0, 10.0), 0.0),
            rect_object("c", RED, 70.0, 10.0, 10.0, 10.0),
        ],
        vec![fill_at(50.0)],
    );
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    assert_eq!(cut_powers(&job), [50.0, 100.0]);
    assert!(warned(&job, "1 objeto(s) pasa de 100 %"));
    assert!(warned(&job, "1 objeto(s) con potencia 0"));
    // Nada de lo de escala 0 en el trabajo (x 40..50).
    let in_gap = job.toolpath.blocks[0].moves.iter().any(|m| match m {
        Move::Cut { to, .. } => (40.5..49.5).contains(&to.x),
        _ => false,
    });
    assert!(!in_gap);
}

#[test]
fn fill_warns_when_objects_of_different_scales_overlap() {
    let d = doc(
        vec![
            scaled(rect_object("a", RED, 10.0, 10.0, 20.0, 20.0), 100.0),
            scaled(rect_object("b", RED, 15.0, 15.0, 5.0, 5.0), 50.0),
        ],
        vec![fill_at(40.0)],
    );
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    assert_eq!(cut_powers(&job), [20.0, 40.0]);
    assert!(warned(&job, "se superponen"));
}

#[test]
fn fill_scales_apply_in_every_angle_pass() {
    let layer = LayerSettings {
        passes: 2,
        angle_increment: 90.0,
        ..fill_at(40.0)
    };
    let d = doc(
        vec![
            scaled(rect_object("a", RED, 10.0, 10.0, 10.0, 10.0), 100.0),
            scaled(rect_object("b", RED, 40.0, 10.0, 10.0, 10.0), 25.0),
        ],
        vec![layer],
    );
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    assert!(job.toolpath.blocks[0].label.contains("pasadas=2"));
    assert_eq!(cut_powers(&job), [10.0, 40.0]);
    // El aviso de varias escalas sale una sola vez.
    let n = job
        .preview
        .warnings
        .iter()
        .filter(|w| w.contains("escalas de potencia distintas"))
        .count();
    assert_eq!(n, 1);
}
