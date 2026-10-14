//! Imágenes colocadas por transform y máscaras.

use super::*;
use std::path::Path;
use tlaku_doc::affine::{self, IDENTITY};
use tlaku_doc::{JobOptions, LayerSettings, ObjectKind, Overscan};

#[test]
fn image_transform_scales_mirrors_and_rejects_rotation() {
    let dir = workdir("image-transform");
    let bmp = write_gradient(&dir);
    let layers = vec![LayerSettings {
        overscan: Overscan::Mm(0.0),
        interval: 0.5,
        ..LayerSettings::for_images()
    }];
    let run = |t: affine::Transform| {
        compute(
            &doc(vec![image_object("i", &bmp, [8.0, 8.0], t)], layers.clone()),
            &device(),
            &JobOptions::default(),
        )
    };
    let plain = run(affine::translate(20.0, 30.0)).unwrap();
    let b = plain.bounds().unwrap();
    assert!(b.min.x >= 20.0 - 1e-9 && b.max.x <= 28.0 + 1e-9, "{b:?}");
    assert!(b.min.y >= 30.0 - 1e-9 && b.max.y <= 38.0 + 1e-9, "{b:?}");
    // Escala 2x: ocupa 16x16 mm desde el mismo punto.
    let big = run(affine::compose(
        &affine::translate(20.0, 30.0),
        &affine::scale(2.0, 2.0),
    ))
    .unwrap();
    assert!(big.bounds().unwrap().max.x > 28.0);
    // Reflejo en X con el ancla a la derecha: ocupa el mismo sitio y cambia la potencia por columna.
    let mirrored = run(affine::compose(
        &affine::translate(28.0, 30.0),
        &affine::scale(-1.0, 1.0),
    ))
    .unwrap();
    assert_ne!(mirrored.toolpath, plain.toolpath);
    let mb = mirrored.bounds().unwrap();
    assert!(mb.min.x >= 20.0 - 1e-9 && mb.max.x <= 28.0 + 1e-9, "{mb:?}");
    // Una rotación no se soporta: se avisa y no queda nada que producir.
    let e = run(affine::rotate(0.3)).unwrap_err();
    assert!(e.contains("sin rotación"), "{e}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn missing_image_file_names_the_path() {
    let d = doc(
        vec![image_object(
            "i",
            Path::new("/no/existe.png"),
            [8.0, 8.0],
            IDENTITY,
        )],
        vec![LayerSettings::for_images()],
    );
    let e = compute(&d, &device(), &JobOptions::default()).unwrap_err();
    assert!(e.contains("/no/existe.png"), "{e}");
}

#[test]
fn image_mask_limits_the_engraving_to_the_shape() {
    use tlaku_core::bezpath::{BezPath, PathSeg};
    let dir = workdir("image-mask");
    let bmp = write_gradient(&dir);
    let layers = vec![LayerSettings {
        overscan: Overscan::Mm(0.0),
        interval: 0.5,
        ..LayerSettings::for_images()
    }];
    let run = |mask: Vec<BezPath>| {
        let mut obj = image_object("i", &bmp, [8.0, 8.0], affine::translate(20.0, 30.0));
        let ObjectKind::Image(img) = &mut obj.kind else {
            unreachable!()
        };
        img.mask = mask;
        compute(
            &doc(vec![obj], layers.clone()),
            &device(),
            &JobOptions::default(),
        )
        .unwrap()
    };
    // Cuadrado local de 2 a 6 mm: en el documento, de 22 a 26 en X y de 32 a 36 en Y.
    let p = |x: f64, y: f64| tlaku_core::Point::new(x, y);
    let square = BezPath(vec![
        PathSeg::MoveTo { to: p(2.0, 2.0) },
        PathSeg::LineTo { to: p(6.0, 2.0) },
        PathSeg::LineTo { to: p(6.0, 6.0) },
        PathSeg::LineTo { to: p(2.0, 6.0) },
        PathSeg::Close,
    ]);
    let full = run(Vec::new()).bounds().unwrap();
    let cut = run(vec![square]).bounds().unwrap();
    assert!(full.min.x < 20.5 && full.max.x > 26.5, "{full:?}");
    assert!(
        cut.min.x >= 22.0 - 1e-9 && cut.max.x <= 26.0 + 1e-9,
        "{cut:?}"
    );
    assert!(
        cut.min.y >= 32.0 - 1e-9 && cut.max.y <= 36.0 + 1e-9,
        "{cut:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
