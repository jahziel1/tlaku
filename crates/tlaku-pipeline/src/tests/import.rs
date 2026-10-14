//! Importación y tipos de archivo.

use super::*;
use std::path::Path;
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{JobOptions, LayerSettings, ObjectKind};

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

#[test]
fn dxf_sin_trayectorias_conserva_la_causa() {
    let dir = workdir("dxf-causa");
    let dxf = dir.join("rellenos.dxf");
    std::fs::write(
        &dxf,
        "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
         0\nSECTION\n2\nENTITIES\n0\nHATCH\n8\n0\n0\nHATCH\n8\n0\n\
         0\nTEXT\n8\n0\n10\n0\n20\n0\n30\n0\n40\n2\n1\nhola\n\
         0\nENDSEC\n0\nEOF\n",
    )
    .unwrap();
    let e = import_file(&dxf).unwrap_err();
    assert!(e.contains("no contiene trayectorias"), "{e}");
    assert!(e.contains("2 entidades HATCH"), "{e}");
    assert!(e.contains("texto"), "{e}");
    assert!(e.contains("contornos"), "{e}");
    // El aviso genérico no se repite como causa.
    assert!(!e.contains("no contiene geometría importable"), "{e}");
    std::fs::remove_dir_all(dir).unwrap();
}

/// PNG de 1x1 píxel.
const PNG_1PX: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

#[test]
fn svg_con_solo_una_imagen_se_importa_como_imagen() {
    let dir = workdir("svg-imagen");
    let svg = dir.join("foto.svg");
    std::fs::write(
        &svg,
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="40mm" height="30mm" viewBox="0 0 40 30"><image x="5" y="3" width="10" height="8" href="data:image/png;base64,{PNG_1PX}"/></svg>"#
        ),
    )
    .unwrap();
    let r = import_file(&svg).unwrap();
    assert_eq!(r.objects.len(), 1);
    assert_eq!(r.new_layers, vec![LayerSettings::for_images()]);
    let ObjectKind::Image(img) = &r.objects[0].kind else {
        panic!("se esperaba una imagen");
    };
    assert_eq!((img.width_px, img.height_px), (1, 1));
    // La imagen de 1x1 px conserva su proporción dentro de la caja de 10 x 8: queda de 8 x 8,
    // centrada (6 mm desde el borde izquierdo).
    approx(img.size_mm[0], 8.0);
    approx(img.size_mm[1], 8.0);
    assert!(Path::new(&img.source_path).exists());
    // Y hacia arriba: la imagen está a 30 - 3 - 8 = 19 mm del borde inferior.
    point(
        Point::new(r.objects[0].transform[4], r.objects[0].transform[5]),
        6.0,
        19.0,
    );
    assert!(
        r.warnings.iter().any(|w| w.contains("solo 1 imagen")),
        "{:?}",
        r.warnings
    );
    assert!(!r.warnings.iter().any(|w| w.contains("omitida")));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn capa_sin_salida_lo_dice_en_el_error_de_diseno_vacio() {
    let off = LayerSettings {
        name: "Corte".into(),
        output: false,
        ..line_layer(RED)
    };
    let d = doc(vec![rect_object("a", RED, 0.0, 0.0, 10.0, 10.0)], vec![off]);
    let e = compute(&d, &device(), &JobOptions::default()).unwrap_err();
    assert!(e.contains("no hay nada que producir"), "{e}");
    assert!(e.contains("\"Corte\"") && e.contains("sin salida"), "{e}");
    // Un documento sin objetos sigue diciendo que está vacío.
    let e = compute(&doc(vec![], vec![]), &device(), &JobOptions::default()).unwrap_err();
    assert!(e.contains("vacío"), "{e}");
}
