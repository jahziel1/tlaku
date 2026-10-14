//! Importación y tipos de archivo.

use super::*;
use std::path::Path;
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{LayerSettings, ObjectKind};

#[test]
fn input_kinds() {
    assert_eq!(input_kind(Path::new("a.SVG")), Ok(InputKind::Svg));
    assert_eq!(input_kind(Path::new("a.dxf")), Ok(InputKind::Dxf));
    assert_eq!(input_kind(Path::new("a.JPEG")), Ok(InputKind::Image));
    assert_eq!(input_kind(Path::new("a.pdf")), Ok(InputKind::Pdf));
    assert_eq!(input_kind(Path::new("a.lbrn2")), Ok(InputKind::Lbrn));
    assert!(
        input_kind(Path::new("a.cdr"))
            .unwrap_err()
            .contains("no soportado")
    );
    assert!(input_kind(Path::new("sin_extension")).is_err());
}

#[test]
fn svg_import_groups_by_color_with_identity_transform() {
    let dir = workdir("svg");
    let svg = dir.join("a.svg");
    std::fs::write(
        &svg,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="60mm" viewBox="0 0 100 60">
  <rect x="5" y="5" width="60" height="50" fill="none" stroke="#ff0000"/>
  <circle cx="35" cy="30" r="8" fill="none" stroke="#ff0000"/>
  <rect x="75" y="10" width="20" height="10" fill="none" stroke="#0000ff"/>
</svg>"##,
    )
    .unwrap();
    let r = import_file(&svg).unwrap();
    assert_eq!(r.objects.len(), 2);
    assert_eq!(r.new_layers.len(), 2);
    assert_eq!(r.objects[0].layer, RED);
    assert_eq!(r.new_layers[0], LayerSettings::for_color(RED));
    assert_eq!(r.new_layers[1].color, BLUE);
    assert_ne!(r.objects[0].id, r.objects[1].id);
    assert!(r.objects.iter().all(|o| o.transform == IDENTITY));
    let ObjectKind::Vector(red) = &r.objects[0].kind else {
        panic!("debía ser vectorial");
    };
    assert_eq!(red.paths.len(), 2);
    // Y hacia arriba: el rectángulo azul (y 10..20 en el SVG de 60 mm) queda en 40..50.
    let b = r.objects[1].bounds().unwrap();
    point(b.min, 75.0, 40.0);
    point(b.max, 95.0, 50.0);
    // Sin voltear, queda tal cual está en el SVG.
    let flat = import_file_with(
        &svg,
        &ImportOptions {
            flip_y: false,
            ..Default::default()
        },
    )
    .unwrap();
    approx(flat.objects[1].bounds().unwrap().min.y, 10.0);
    // Dos importaciones no repiten ids.
    let again = import_file(&svg).unwrap();
    assert!(
        r.objects
            .iter()
            .all(|a| again.objects.iter().all(|b| a.id != b.id))
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn image_import_makes_an_image_object_and_layer() {
    let dir = workdir("import-image");
    let bmp = write_gradient(&dir);
    let r = import_file(&bmp).unwrap();
    assert_eq!(r.objects.len(), 1);
    let ObjectKind::Image(img) = &r.objects[0].kind else {
        panic!("debía ser imagen");
    };
    assert_eq!((img.width_px, img.height_px), (16, 16));
    approx(img.size_mm[0], 16.0 * 25.4 / 96.0);
    assert_eq!(r.new_layers, vec![LayerSettings::for_images()]);
    assert_eq!(r.objects[0].layer, r.new_layers[0].color);
    assert!(import_file(&dir.join("no_existe.bmp")).is_err());
    assert!(
        import_file(&dir.join("x.cdr"))
            .unwrap_err()
            .contains("no soportado")
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn empty_vector_file_is_an_error() {
    let dir = workdir("empty");
    let svg = dir.join("vacio.svg");
    std::fs::write(
        &svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10mm" height="10mm"/>"#,
    )
    .unwrap();
    assert!(
        import_file(&svg)
            .unwrap_err()
            .contains("no contiene trayectorias")
    );
    std::fs::remove_dir_all(dir).unwrap();
}
