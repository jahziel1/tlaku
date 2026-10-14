//! `compute`: capas line, fill e imagen.

use super::*;
use tlaku_doc::{Document, JobOptions, LayerOrder, LayerSettings};

#[test]
fn compute_line_fill_and_image() {
    let dir = workdir("compute3");
    let doc = three_layers(&dir);
    let job = compute(&doc, &device(), &JobOptions::default()).unwrap();
    let modes: Vec<_> = job.layers.iter().map(|l| (l.color, l.mode)).collect();
    // Auto: grabados (relleno, imagen) primero y el corte (100 % con 2 pasadas) al final.
    assert_eq!(
        modes,
        vec![
            (BLUE, "fill"),
            (LayerSettings::IMAGE_COLOR, "image"),
            (RED, "line")
        ]
    );
    assert_eq!(job.toolpath.blocks.len(), 3);
    assert_eq!(job.layers[2].count, 1);
    let p = &job.preview;
    assert_eq!(p.layers.len(), 3);
    assert_eq!(p.layers[0].mode, "fill");
    assert!(p.layers[2].travel_saved_pct.is_some());
    assert!(p.layers[0].travel_saved_pct.is_none());
    assert!(
        p.layers
            .iter()
            .all(|l| l.seconds > 0.0 && !l.summary.is_empty())
    );
    assert!(!p.layers[2].explanations.is_empty());
    assert!(
        p.order_note
            .as_deref()
            .unwrap()
            .starts_with("orden automático")
    );
    let n: usize = job.toolpath.blocks.iter().map(|b| b.moves.len()).sum();
    assert_eq!(p.moves.len(), n * 4);
    assert!(p.moves.chunks(4).all(|m| m[0] == 0.0 || m[0] == 1.0));
    assert!(p.total_s > p.layers.iter().map(|l| l.seconds).sum::<f64>() * 0.99);
    // La caja incluye el overscan del relleno y llega hasta la imagen (X 100..108).
    assert!(p.bounds[0][0] < 10.0 || p.bounds[0][0] <= 60.0);
    assert!(p.bounds[1][0] >= 108.0 && p.bounds[1][1] >= 40.0);
    assert_eq!(job.bounds().unwrap().max.x, p.bounds[1][0]);
    // Misma entrada, misma salida.
    let again = compute(&doc, &device(), &JobOptions::default()).unwrap();
    assert_eq!(again.toolpath, job.toolpath);
    // Con el orden del archivo, el corte va primero.
    let file = compute(
        &doc,
        &device(),
        &JobOptions {
            layer_order: LayerOrder::File,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(file.layers[0].color, RED);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn forced_order_beats_auto_and_duplicates_fail() {
    let mk = |a: Option<u32>, b: Option<u32>| {
        doc(
            vec![
                rect_object("a", RED, 10.0, 10.0, 20.0, 20.0),
                rect_object("b", BLUE, 40.0, 10.0, 20.0, 20.0),
            ],
            vec![
                LayerSettings {
                    power: 100.0,
                    passes: 2,
                    order: a,
                    ..line_layer(RED)
                },
                LayerSettings {
                    order: b,
                    ..fill_layer(BLUE)
                },
            ],
        )
    };
    let job = compute(&mk(Some(1), None), &device(), &JobOptions::default()).unwrap();
    assert_eq!(job.layers[0].color, RED);
    assert!(
        job.preview
            .order_note
            .unwrap()
            .contains("#ff0000 en la posición 1")
    );
    let e = compute(&mk(Some(1), Some(1)), &device(), &JobOptions::default()).unwrap_err();
    assert!(e.contains("misma posición: order=1"), "{e}");
    let e = compute(&mk(Some(0), None), &device(), &JobOptions::default()).unwrap_err();
    assert!(e.contains("entero desde 1"), "{e}");
    let job = compute(&mk(Some(9), None), &device(), &JobOptions::default()).unwrap();
    assert!(job.preview.warnings.iter().any(|w| w.contains("excede")));
}

#[test]
fn invalid_layer_values_fail_before_processing() {
    let d = doc(
        vec![rect_object("a", RED, 10.0, 10.0, 20.0, 20.0)],
        vec![LayerSettings {
            power: 150.0,
            ..line_layer(RED)
        }],
    );
    let e = compute(&d, &device(), &JobOptions::default()).unwrap_err();
    assert!(e.contains("capa #ff0000") && e.contains("potencia"), "{e}");
}

#[test]
fn objects_without_output_make_an_empty_job_error() {
    let mut d = doc(
        vec![rect_object("a", RED, 10.0, 10.0, 20.0, 20.0)],
        vec![line_layer(RED)],
    );
    d.layers[0].output = false;
    let e = compute(&d, &device(), &JobOptions::default()).unwrap_err();
    assert!(e.contains("no hay nada que producir"), "{e}");
    assert!(compute(&Document::default(), &device(), &JobOptions::default()).is_err());
}

#[test]
fn object_with_unknown_layer_is_an_error() {
    let d = doc(vec![rect_object("a", RED, 10.0, 10.0, 20.0, 20.0)], vec![]);
    let e = compute(&d, &device(), &JobOptions::default()).unwrap_err();
    assert!(e.contains("'a'") && e.contains("#ff0000"), "{e}");
}

#[test]
fn objects_in_a_layer_of_another_mode_are_reported() {
    let d = doc(
        vec![
            rect_object("a", RED, 10.0, 10.0, 20.0, 20.0),
            rect_object("v", LayerSettings::IMAGE_COLOR, 10.0, 10.0, 5.0, 5.0),
        ],
        vec![line_layer(RED), LayerSettings::for_images()],
    );
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    assert_eq!(job.layers.len(), 1);
    assert!(
        job.preview
            .warnings
            .iter()
            .any(|w| w.contains("'v'") && w.contains("image")),
        "{:?}",
        job.preview.warnings
    );
}
