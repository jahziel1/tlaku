//! Imagen de la cama y buscador de centro.

use super::*;

fn rgb_to_b64(img: &image::RgbImage) -> String {
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, ImageFormat::Png).unwrap();
    STANDARD.encode(buf.into_inner())
}

/// Foto de 320x240 con un disco claro de 20 px de radio en (200, 100).
fn tile_photo() -> image::RgbImage {
    image::RgbImage::from_fn(320, 240, |x, y| {
        let inside = (f64::from(x) + 0.5 - 200.0).hypot(f64::from(y) + 0.5 - 100.0) < 20.0;
        if inside {
            image::Rgb([250, 240, 230])
        } else {
            image::Rgb([(x / 2) as u8, 90, (y / 2) as u8])
        }
    })
}

#[test]
fn la_imagen_de_la_cama_cae_donde_estaba_y_se_guarda() {
    let tmp = TempDir::new();
    fs::create_dir_all(&tmp.0).unwrap();
    // 320x240 px a 2 px/mm = 160x120 mm con la esquina superior izquierda en (50, 180).
    let tile = BedTile {
        image_png_base64: rgb_to_b64(&tile_photo()),
        to_bed: [[0.5, 0.0, 50.0], [0.0, -0.5, 180.0], [0.0, 0.0, 1.0]],
    };
    let args = ComposeArgs {
        bed: [300.0, 200.0],
        px_per_mm: Some(2.0),
        grayscale: false,
        feather_px: 24.0,
        crop_px: 0.0,
        rect: None,
        dir: Some(tmp.0.clone()),
        stem: "proyecto".into(),
        save: true,
    };
    let r = run_compose_bed(std::slice::from_ref(&tile), &args).unwrap();
    assert_eq!((r.width_px, r.height_px), (320, 240));
    assert!((r.origin_mm[0] - 50.0).abs() < 1e-9 && (r.origin_mm[1] - 60.0).abs() < 1e-9);
    assert!((r.size_mm[0] - 160.0).abs() < 1e-9 && (r.size_mm[1] - 120.0).abs() < 1e-9);
    assert!(!r.in_temp_dir && r.path.ends_with("proyecto-camara-1.png"));
    // El disco (200, 100) px está en (50 + 100, 180 - 50) = (150, 130) mm: en la imagen guardada
    // cae en la columna 200 y la fila 100, igual que en la foto.
    let saved = image::open(&r.path).unwrap().to_rgb8();
    assert_eq!(saved.get_pixel(200, 100).0, [250, 240, 230]);
    assert!(r.coverage > 0.999 && !r.preview_png_base64.is_empty());
    // Otro guardado no pisa el primero.
    let r2 = run_compose_bed(std::slice::from_ref(&tile), &args).unwrap();
    assert!(r2.path.ends_with("proyecto-camara-2.png"));
    // En gris, los tres canales coinciden.
    let g = run_compose_bed(
        std::slice::from_ref(&tile),
        &ComposeArgs {
            grayscale: true,
            save: false,
            ..args.clone()
        },
    )
    .unwrap();
    assert!(g.path.is_empty());
    let preview = image::load_from_memory(&STANDARD.decode(&g.preview_png_base64).unwrap())
        .unwrap()
        .to_rgb8();
    let p = preview.get_pixel(10, 10).0;
    assert!(p[0] == p[1] && p[1] == p[2]);
    // Sin carpeta de proyecto va a la temporal.
    let t = run_compose_bed(&[tile], &ComposeArgs { dir: None, ..args }).unwrap();
    assert!(t.in_temp_dir);
    let _ = fs::remove_file(&t.path);
}

#[test]
fn la_imagen_de_la_cama_rechaza_entradas_invalidas() {
    let base = ComposeArgs {
        bed: [300.0, 200.0],
        px_per_mm: None,
        grayscale: false,
        feather_px: 24.0,
        crop_px: 0.0,
        rect: None,
        dir: None,
        stem: "x".into(),
        save: false,
    };
    assert!(
        run_compose_bed(&[], &base)
            .unwrap_err()
            .contains("ninguna foto")
    );
    let fuera = BedTile {
        image_png_base64: rgb_to_b64(&tile_photo()),
        to_bed: [[0.5, 0.0, 900.0], [0.0, -0.5, 900.0], [0.0, 0.0, 1.0]],
    };
    assert!(
        run_compose_bed(&[fuera], &base)
            .unwrap_err()
            .contains("dentro de la cama")
    );
    let nan = BedTile {
        image_png_base64: rgb_to_b64(&tile_photo()),
        to_bed: [[f64::NAN, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };
    assert!(run_compose_bed(&[nan], &base).is_err());
    let roto = BedTile {
        image_png_base64: "###".into(),
        to_bed: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };
    assert!(
        run_compose_bed(&[roto], &base)
            .unwrap_err()
            .contains("Foto 1")
    );
}

#[test]
fn detecta_un_circulo_en_una_foto() {
    let img = GrayImage::from_fn(400, 300, |x, y| {
        let inside = (f64::from(x) + 0.5 - 180.5).hypot(f64::from(y) + 0.5 - 120.25) < 60.0;
        image::Luma([if inside { 40 } else { 200 }])
    });
    let hint = CircleHint {
        center: [180.0, 120.0],
        radius: None,
    };
    let f = run_detect_circle(&to_b64(&img), Some(hint)).unwrap();
    assert!(
        (f.center[0] - 180.5).abs() < 0.5 && (f.center[1] - 120.25).abs() < 0.5,
        "{:?}",
        f.center
    );
    assert!((f.radius - 60.0).abs() < 0.7);
    assert!(f.points.len() >= 30);
    assert!(
        run_detect_circle(
            &to_b64(&GrayImage::from_pixel(100, 100, image::Luma([90]))),
            None
        )
        .is_err()
    );
}
