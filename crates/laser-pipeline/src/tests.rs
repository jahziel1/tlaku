use super::*;
use crate::layers::{KERF_REASON, POWER_REASON, order_note};
use laser_core::toolpath::{Move, PowerMode, ToolpathBlock};
use laser_core::{CutSettings, Point, Rgb};
use laser_doc::affine::{self, IDENTITY};
use laser_doc::{
    DesignObject, DeviceProfile, Document, ImageObject, JobOptions, LayerMode, LayerOrder,
    LayerSettings, ObjectKind, Origin, Overscan, PathData, StartFrom, VectorObject,
};
use laser_fill::FillSettings;
use laser_raster::RasterSettings;
use std::path::{Path, PathBuf};

const RED: Rgb = Rgb(255, 0, 0);
const BLUE: Rgb = Rgb(0, 0, 255);

// ---------------------------------------------------------------------------
// Apoyos
// ---------------------------------------------------------------------------

fn rect_object(id: &str, layer: Rgb, x: f64, y: f64, w: f64, h: f64) -> DesignObject {
    DesignObject {
        id: id.into(),
        layer,
        transform: IDENTITY,
        kind: ObjectKind::Vector(VectorObject {
            paths: vec![PathData {
                points: vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]],
                closed: true,
            }],
        }),
        locked: false,
        hidden: false,
    }
}

fn image_object(
    id: &str,
    path: &Path,
    size: [f64; 2],
    transform: affine::Transform,
) -> DesignObject {
    DesignObject {
        id: id.into(),
        layer: LayerSettings::IMAGE_COLOR,
        transform,
        kind: ObjectKind::Image(ImageObject {
            source_path: path.to_str().unwrap().into(),
            width_px: 16,
            height_px: 16,
            size_mm: size,
        }),
        locked: false,
        hidden: false,
    }
}

fn line_layer(color: Rgb) -> LayerSettings {
    LayerSettings::for_color(color)
}

fn fill_layer(color: Rgb) -> LayerSettings {
    LayerSettings {
        mode: LayerMode::Fill,
        interval: 1.0,
        ..LayerSettings::for_color(color)
    }
}

fn workdir(test: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("lasersoft-pipeline-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// BMP de 24 bits; la fila 0 de `pixel` queda arriba.
fn gray_bmp(width: u32, height: u32, pixel: impl Fn(u32, u32) -> u8) -> Vec<u8> {
    let row_bytes = (width * 3).div_ceil(4) * 4;
    let size = 54 + row_bytes * height;
    let mut bmp = b"BM".to_vec();
    bmp.extend(size.to_le_bytes());
    bmp.extend([0u8; 4]);
    bmp.extend(54u32.to_le_bytes());
    bmp.extend(40u32.to_le_bytes());
    bmp.extend(width.to_le_bytes());
    bmp.extend(height.to_le_bytes());
    bmp.extend(1u16.to_le_bytes());
    bmp.extend(24u16.to_le_bytes());
    bmp.extend([0u8; 24]);
    for y in (0..height).rev() {
        let mut row: Vec<u8> = (0..width).flat_map(|x| [pixel(x, y); 3]).collect();
        row.resize(row_bytes as usize, 0);
        bmp.extend(row);
    }
    bmp
}

/// Degradado horizontal: negro a la izquierda, casi blanco a la derecha.
fn write_gradient(dir: &Path) -> PathBuf {
    let path = dir.join("degradado.bmp");
    std::fs::write(&path, gray_bmp(16, 16, |x, _| (x * 16).min(255) as u8)).unwrap();
    path
}

fn doc(objects: Vec<DesignObject>, layers: Vec<LayerSettings>) -> Document {
    Document {
        objects,
        layers,
        ..Document::default()
    }
}

fn device() -> DeviceProfile {
    DeviceProfile::default()
}

fn approx(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-4, "{a} != {b}");
}

fn point(p: Point, x: f64, y: f64) {
    approx(p.x, x);
    approx(p.y, y);
}

// ---------------------------------------------------------------------------
// Importación y tipos de archivo
// ---------------------------------------------------------------------------

#[test]
fn input_kinds() {
    assert_eq!(input_kind(Path::new("a.SVG")), Ok(InputKind::Svg));
    assert_eq!(input_kind(Path::new("a.dxf")), Ok(InputKind::Dxf));
    assert_eq!(input_kind(Path::new("a.JPEG")), Ok(InputKind::Image));
    assert!(
        input_kind(Path::new("a.pdf"))
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
        import_file(&dir.join("x.pdf"))
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

// ---------------------------------------------------------------------------
// Capas: valores por defecto, resolución, clasificación y orden
// ---------------------------------------------------------------------------

#[test]
fn layer_defaults_match_the_engine_defaults() {
    match resolve_layer(&line_layer(RED)).unwrap() {
        ResolvedLayer::Line { settings, kerf_mm } => {
            assert_eq!(settings, CutSettings::default());
            assert_eq!(kerf_mm, 0.0);
        }
        _ => panic!("debía ser line"),
    }
    let fill = LayerSettings {
        mode: LayerMode::Fill,
        ..line_layer(RED)
    };
    match resolve_layer(&fill).unwrap() {
        ResolvedLayer::Fill {
            settings, overscan, ..
        } => {
            assert_eq!(settings, FillSettings::default());
            assert_eq!(overscan, Overscan::Auto);
        }
        _ => panic!("debía ser fill"),
    }
    let img = resolve_image(&LayerSettings::for_images()).unwrap();
    assert_eq!(
        img.raster,
        RasterSettings {
            overscan_mm: 0.0, // "auto": se calcula al armar el trabajo
            ..RasterSettings::default()
        }
    );
    assert_eq!(img.mode, laser_raster::RasterMode::default());
    assert_eq!(img.interval_mm, 0.1);
    assert_eq!(img.adjust, laser_raster::ImageAdjust::default());
}

#[test]
fn resolve_rejects_bad_values_with_the_layer_name() {
    let bad = |f: &dyn Fn(&mut LayerSettings)| {
        let mut l = line_layer(RED);
        f(&mut l);
        resolve_layer(&l).unwrap_err()
    };
    assert!(bad(&|l| l.power = 150.0).contains("capa #ff0000: potencia fuera de 0-100"));
    assert!(bad(&|l| l.passes = 0).contains("al menos 1"));
    assert!(bad(&|l| l.passes = 5000).contains("demasiadas pasadas"));
    assert!(bad(&|l| l.speed = 0.0).contains("velocidad"));
    assert!(
        bad(&|l| {
            l.mode = LayerMode::Fill;
            l.interval = 0.0;
        })
        .contains("intervalo inválido")
    );
    let mut img = LayerSettings::for_images();
    img.image.gamma = 0.0;
    assert!(resolve_image(&img).unwrap_err().contains("gamma"));
    let mut img = LayerSettings::for_images();
    img.min_power = 50.0;
    img.power = 10.0;
    assert!(resolve_image(&img).unwrap_err().contains("mínima"));
}

use LayerRole::{Cut, Engrave, Line};

#[test]
fn auto_order_is_stable_and_cuts_go_last() {
    let roles = [Cut, Line, Engrave, Line, Engrave, Cut];
    let none = [None; 6];
    assert_eq!(
        layer_sequence(&roles, &none, LayerOrder::Auto),
        [2, 4, 1, 3, 0, 5]
    );
    assert_eq!(
        layer_sequence(&roles, &none, LayerOrder::File),
        [0, 1, 2, 3, 4, 5]
    );
}

#[test]
fn explicit_order_wins_over_auto_and_clamps() {
    let roles = [Cut, Engrave, Line];
    assert_eq!(
        layer_sequence(&roles, &[Some(1), None, None], LayerOrder::Auto),
        [0, 1, 2]
    );
    assert_eq!(
        layer_sequence(&roles, &[None, Some(3), Some(1)], LayerOrder::Auto),
        [2, 0, 1]
    );
    assert_eq!(
        layer_sequence(&roles, &[None, Some(9), None], LayerOrder::File),
        [0, 2, 1]
    );
}

#[test]
fn classification_rules() {
    let line = |power_pct: f64, passes: u32, kerf_mm: f64| ResolvedLayer::Line {
        settings: CutSettings {
            power_pct,
            passes,
            ..Default::default()
        },
        kerf_mm,
    };
    assert_eq!(classify(&line(100.0, 1, 0.0)).0, Line);
    assert_eq!(classify(&line(50.0, 5, 0.0)).0, Line);
    assert_eq!(classify(&line(80.0, 2, 0.0)), (Cut, Some(POWER_REASON)));
    assert_eq!(classify(&line(10.0, 1, 0.1)), (Cut, Some(KERF_REASON)));
    assert_eq!(classify(&line(10.0, 1, -0.1)).0, Cut);
}

#[test]
fn order_note_explains_the_groups() {
    let note = order_note(
        LayerOrder::Auto,
        &[Engrave, Cut],
        &[(RED, KERF_REASON)],
        &[],
    );
    assert_eq!(
        note,
        "orden automático: grabados primero, cortes al final (corte: #ff0000 por kerf)"
    );
    let note = order_note(LayerOrder::Auto, &[Engrave, Line, Cut], &[], &[]);
    assert_eq!(
        note,
        "orden automático: grabados primero, líneas después, cortes al final"
    );
    let note = order_note(LayerOrder::Auto, &[Line, Line], &[], &[]);
    assert!(note.contains("mismo tipo"), "{note}");
    let note = order_note(LayerOrder::Auto, &[], &[], &[(RED, 1)]);
    assert!(note.starts_with("orden fijado con order="), "{note}");
    let note = order_note(LayerOrder::File, &[Cut, Engrave], &[], &[(RED, 2)]);
    assert!(
        note.starts_with("orden del archivo") && note.contains("#ff0000 en la posición 2"),
        "{note}"
    );
}

// ---------------------------------------------------------------------------
// compute: capas line, fill e imagen
// ---------------------------------------------------------------------------

fn three_layers(dir: &Path) -> Document {
    let bmp = write_gradient(dir);
    doc(
        vec![
            rect_object("marco", RED, 10.0, 10.0, 40.0, 30.0),
            rect_object("relleno", BLUE, 60.0, 10.0, 20.0, 10.0),
            image_object("foto", &bmp, [8.0, 8.0], affine::translate(100.0, 10.0)),
        ],
        vec![
            LayerSettings {
                power: 100.0,
                passes: 2,
                ..line_layer(RED)
            },
            fill_layer(BLUE),
            LayerSettings::for_images(),
        ],
    )
}

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

// ---------------------------------------------------------------------------
// selection
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// start_from y job_origin
// ---------------------------------------------------------------------------

/// Rectángulo de 20x40 con la esquina inferior izquierda en (10, 20).
fn tall_rect() -> Document {
    doc(
        vec![rect_object("a", RED, 10.0, 20.0, 20.0, 40.0)],
        vec![line_layer(RED)],
    )
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

// ---------------------------------------------------------------------------
// Límites de la cama
// ---------------------------------------------------------------------------

#[test]
fn bounds_error_counts_the_overscan() {
    let overscan = LayerSettings {
        overscan: Overscan::Mm(2.0),
        ..fill_layer(BLUE)
    };
    let d = doc(
        vec![rect_object("r", BLUE, 10.0, 10.0, 40.0, 10.0)],
        vec![overscan],
    );
    let mut dev = device();
    // El relleno llega a X50 y con el overscan a X52.
    dev.bed = [51.0, 100.0];
    let e = compute(&d, &dev, &JobOptions::default()).unwrap_err();
    assert!(
        e.contains("incluye 2.00 mm de overscan") && e.contains("#0000ff"),
        "{e}"
    );
    assert!(e.contains("51x100"), "{e}");
    // `build` no verifica la cama y la vista previa sigue disponible.
    assert!(build(&d, &dev, &JobOptions::default(), None).is_ok());
    dev.bed = [53.0, 100.0];
    assert!(compute(&d, &dev, &JobOptions::default()).is_ok());
    // También por abajo y a la izquierda.
    let outside = doc(
        vec![rect_object("r", RED, -5.0, 10.0, 10.0, 10.0)],
        vec![line_layer(RED)],
    );
    assert!(compute(&outside, &device(), &JobOptions::default()).is_err());
}

#[test]
fn bounds_check_covers_every_move() {
    let block = ToolpathBlock {
        label: "x".into(),
        speed_mm_min: 1000.0,
        air_assist: false,
        power_mode: PowerMode::Dynamic,
        moves: vec![
            Move::Travel {
                to: Point::new(-0.5, 5.0),
            },
            Move::Cut {
                to: Point::new(10.0, 5.0),
                power_pct: 0.0,
            },
        ],
    };
    let layer = LayerReport {
        name: "#ff0000".into(),
        color: RED,
        mode: "fill",
        count: 1,
        unit: "líneas",
        speed_mm_min: 1000.0,
        power: "10%".into(),
        passes: 1,
        overscan_mm: Some(2.0),
        plan: None,
        seconds: 0.0,
        bounds: block_bounds(&block),
    };
    let job = ComputedJob {
        toolpath: laser_core::toolpath::Toolpath {
            blocks: vec![block],
        },
        preview: crate::preview::preview(
            &laser_core::toolpath::Toolpath::default(),
            &[],
            0.0,
            vec![],
            None,
            vec![],
        ),
        layers: vec![layer],
    };
    let e = check_bounds(&job, [100.0, 100.0]).unwrap_err();
    assert!(e.contains("#ff0000") && e.contains("overscan"), "{e}");
}

#[test]
fn invalid_device_is_rejected() {
    let mut dev = device();
    dev.accel = [0.0, 1000.0];
    let e = compute(&tall_rect(), &dev, &JobOptions::default()).unwrap_err();
    assert!(e.contains("aceleración"), "{e}");
}

// ---------------------------------------------------------------------------
// Imágenes colocadas por transform
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// G-code
// ---------------------------------------------------------------------------

#[test]
fn gcode_uses_the_device_profile() {
    let job = compute(&tall_rect(), &device(), &JobOptions::default()).unwrap();
    let g = gcode(&job, &device());
    assert!(
        g.starts_with("; Generado por lasersoft\nG90\nG21\nM4 S0\n"),
        "{g}"
    );
    assert!(
        g.contains("G0 X10 Y20") && g.trim_end().ends_with("G0 X0 Y0"),
        "{g}"
    );
    let mut dev = device();
    dev.power_mode = laser_doc::PowerMode::Constant;
    dev.s_max = 255.0;
    dev.start_gcode = "G54".into();
    dev.end_gcode = "M2".into();
    dev.return_to_origin = false;
    let g = gcode(&job, &dev);
    assert!(g.contains("G21\nG54\nM3 S0\n"), "{g}");
    assert!(g.contains("S51") && g.trim_end().ends_with("M5\nM2"), "{g}");
}

#[test]
fn gcode_origin_flips_toward_the_machine_corner() {
    let job = compute(&tall_rect(), &device(), &JobOptions::default()).unwrap();
    // Cama 400x415: la esquina (10, 20) del documento.
    let at = |origin: Origin| {
        let mut dev = device();
        dev.origin = origin;
        gcode(&job, &dev)
    };
    assert!(at(Origin::BottomLeft).contains("G0 X10 Y20"));
    assert!(at(Origin::TopLeft).contains("G0 X10 Y395"));
    assert!(at(Origin::BottomRight).contains("G0 X390 Y20"));
    assert!(at(Origin::TopRight).contains("G0 X390 Y395"));
    // El trabajo calculado no cambia: solo la salida.
    assert_eq!(job.bounds().unwrap().min.x, 10.0);
}

#[test]
fn preview_has_one_cumulative_time_per_move() {
    let d = doc(
        vec![rect_object("a", RED, 10.0, 10.0, 20.0, 10.0)],
        vec![line_layer(RED)],
    );
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    let p = &job.preview;
    assert_eq!(p.move_times.len() * 4, p.moves.len());
    assert!(p.move_times.windows(2).all(|w| w[0] <= w[1]));
    approx(*p.move_times.last().unwrap(), p.total_s);
}

#[test]
fn frame_gcode_follows_the_machine_origin() {
    let d = doc(
        vec![rect_object("a", RED, 10.0, 20.0, 30.0, 10.0)],
        vec![line_layer(RED)],
    );
    let mut dev = device();
    let job = compute(&d, &dev, &JobOptions::default()).unwrap();
    let g = frame_gcode(&job, &dev, 1500.0);
    assert!(g.contains(&"G0 X10 Y20".to_string()), "{g:?}");
    assert!(g.contains(&"G1 X40 Y30 S0 F1500".to_string()), "{g:?}");
    dev.origin = Origin::TopLeft;
    let g = frame_gcode(&job, &dev, 1500.0);
    let bed_h = dev.bed[1];
    assert!(g.contains(&format!("G0 X10 Y{}", bed_h - 30.0)), "{g:?}");
}

#[test]
fn user_origin_and_current_position_are_machine_coordinates() {
    // Con el origen arriba a la izquierda, el punto que muestra GRBL debe coincidir en el
    // G-code con la esquina elegida del trabajo, no con su reflejo en la cama.
    let d = doc(
        vec![rect_object("a", RED, 100.0, 100.0, 30.0, 10.0)],
        vec![line_layer(RED)],
    );
    let mut dev = device();
    dev.origin = Origin::TopLeft;
    let bottom_left = 6;
    let user = JobOptions {
        start_from: StartFrom::UserOrigin,
        job_origin: bottom_left,
        user_origin: Some([10.0, 12.0]),
        ..JobOptions::default()
    };
    let g = gcode(&compute(&d, &dev, &user).unwrap(), &dev);
    assert!(g.contains("X10 Y12"), "{g}");

    let current = JobOptions {
        start_from: StartFrom::CurrentPosition,
        job_origin: bottom_left,
        ..JobOptions::default()
    };
    let job = compute_at(&d, &dev, &current, Some(Point::new(10.0, 12.0))).unwrap();
    assert!(gcode(&job, &dev).contains("X10 Y12"));
}
