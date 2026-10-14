//! Selección de objetos, `start_from` y `job_origin`.

use super::*;
use tlaku_core::Point;
use tlaku_doc::{JobOptions, LayerOrder, StartFrom};

#[test]
fn selection_limits_the_job_to_those_objects() {
    let dir = workdir("selection");
    let d = three_layers(&dir);
    let only = |ids: &[&str]| JobOptions {
        selection: Some(ids.iter().map(|s| s.to_string()).collect()),
        ..Default::default()
    };
    let job = compute(&d, &device(), &only(&["relleno"])).unwrap();
    assert_eq!(job.layers.len(), 1);
    assert_eq!(job.layers[0].color, BLUE);
    assert!(job.preview.order_note.is_none());

    let job = compute(&d, &device(), &only(&["marco", "foto", "no-existe"])).unwrap();
    assert_eq!(job.layers.len(), 2);
    assert!(
        job.preview
            .warnings
            .iter()
            .any(|w| w.contains("no-existe") && w.contains("no existe"))
    );

    // Seleccionar un objeto de una capa sin salida no lo envía.
    let mut d2 = d.clone();
    d2.layers[1].output = false;
    let e = compute(&d2, &device(), &only(&["relleno"])).unwrap_err();
    assert!(e.contains("no hay nada que producir"), "{e}");

    // Un objeto oculto tampoco se envía aunque esté seleccionado.
    let mut d3 = d.clone();
    d3.objects[1].hidden = true;
    assert!(compute(&d3, &device(), &only(&["relleno"])).is_err());
    assert_eq!(
        compute(&d3, &device(), &JobOptions::default())
            .unwrap()
            .layers
            .len(),
        2
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn absolute_keeps_the_document_coordinates() {
    let job = compute(&tall_rect(), &device(), &JobOptions::default()).unwrap();
    let b = job.bounds().unwrap();
    point(b.min, 10.0, 20.0);
    point(b.max, 30.0, 60.0);
    // El job_origin no cuenta en modo absoluto.
    let other = JobOptions {
        job_origin: 0,
        ..Default::default()
    };
    let job2 = compute(&tall_rect(), &device(), &other).unwrap();
    assert_eq!(job2.toolpath, job.toolpath);
}

#[test]
fn job_origin_puts_each_of_the_nine_points_on_the_user_origin() {
    // (columna, fila) del punto del rectángulo [10,30]x[20,60] que debe caer en el origen.
    let anchors = [
        (10.0, 60.0),
        (20.0, 60.0),
        (30.0, 60.0),
        (10.0, 40.0),
        (20.0, 40.0),
        (30.0, 40.0),
        (10.0, 20.0),
        (20.0, 20.0),
        (30.0, 20.0),
    ];
    let user = (200.0, 300.0);
    for (i, (ax, ay)) in anchors.into_iter().enumerate() {
        let opts = JobOptions {
            start_from: StartFrom::UserOrigin,
            job_origin: i as u32,
            user_origin: Some([user.0, user.1]),
            ..Default::default()
        };
        let mut dev = device();
        dev.bed = [1000.0, 1000.0];
        let job = compute(&tall_rect(), &dev, &opts).unwrap();
        let b = job.bounds().unwrap();
        // El diseño se desplaza (user - ancla) respecto de su sitio original.
        point(b.min, 10.0 + user.0 - ax, 20.0 + user.1 - ay);
        point(b.max, 30.0 + user.0 - ax, 60.0 + user.1 - ay);
        // Y el punto elegido coincide con el origen.
        let anchor = Point::new(ax + user.0 - ax, ay + user.1 - ay);
        point(anchor, user.0, user.1);
        let col = (i % 3) as f64 / 2.0;
        let row = (i / 3) as f64 / 2.0;
        point(
            Point::new(b.min.x + 20.0 * col, b.max.y - 40.0 * row),
            user.0,
            user.1,
        );
    }
}

#[test]
fn current_position_needs_the_position_and_uses_it() {
    let opts = JobOptions {
        start_from: StartFrom::CurrentPosition,
        job_origin: 6,
        ..Default::default()
    };
    let e = compute(&tall_rect(), &device(), &opts).unwrap_err();
    assert!(e.contains("posición actual"), "{e}");
    let job = compute_at(&tall_rect(), &device(), &opts, Some(Point::new(50.0, 70.0))).unwrap();
    let b = job.bounds().unwrap();
    point(b.min, 50.0, 70.0);
    point(b.max, 70.0, 110.0);
    // Sin origen de usuario tampoco se puede.
    let user = JobOptions {
        start_from: StartFrom::UserOrigin,
        ..Default::default()
    };
    assert!(
        compute(&tall_rect(), &device(), &user)
            .unwrap_err()
            .contains("origen de usuario")
    );
    let bad = JobOptions {
        job_origin: 9,
        ..Default::default()
    };
    assert!(
        compute(&tall_rect(), &device(), &bad)
            .unwrap_err()
            .contains("0-8")
    );
}

#[test]
fn relative_start_moves_images_and_vectors_together() {
    let dir = workdir("relative");
    let d = three_layers(&dir);
    let opts = JobOptions {
        start_from: StartFrom::UserOrigin,
        job_origin: 6,
        user_origin: Some([5.0, 5.0]),
        layer_order: LayerOrder::File,
        ..Default::default()
    };
    // El conjunto ocupa X 10..108, Y 10..40: la esquina inferior izquierda pasa a (5, 5).
    let moved = compute(&d, &device(), &opts).unwrap();
    let base = compute(
        &d,
        &device(),
        &JobOptions {
            layer_order: LayerOrder::File,
            ..Default::default()
        },
    )
    .unwrap();
    let (a, b) = (moved.bounds().unwrap(), base.bounds().unwrap());
    // Con el overscan de por medio la caja no coincide con la del diseño, pero la
    // traslación es la misma para todas las capas: -5 en X y en Y.
    approx(a.max.x - b.max.x, -5.0);
    approx(a.max.y - b.max.y, -5.0);
    for (m, o) in moved.layers.iter().zip(&base.layers) {
        let (mb, ob) = (m.bounds.unwrap(), o.bounds.unwrap());
        approx(mb.min.x - ob.min.x, -5.0);
        approx(mb.min.y - ob.min.y, -5.0);
    }
    std::fs::remove_dir_all(dir).unwrap();
}
