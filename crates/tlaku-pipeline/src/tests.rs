use super::*;
use crate::layers::{KERF_REASON, POWER_REASON, order_note};
use std::path::{Path, PathBuf};
use tlaku_core::bezpath::BezPath;
use tlaku_core::toolpath::{Move, PowerMode, ToolpathBlock};
use tlaku_core::{CutSettings, Point, Polyline, Rgb};
use tlaku_doc::affine::{self, IDENTITY};
use tlaku_doc::{
    DesignObject, DeviceProfile, Document, ImageObject, JobOptions, LayerMode, LayerOrder,
    LayerSettings, ObjectKind, Origin, Overscan, StartFrom, VectorObject,
};
use tlaku_fill::FillSettings;
use tlaku_raster::RasterSettings;

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
            paths: vec![BezPath::from(&Polyline {
                points: vec![
                    Point::new(x, y),
                    Point::new(x + w, y),
                    Point::new(x + w, y + h),
                    Point::new(x, y + h),
                ],
                closed: true,
            })],
        }),
        locked: false,
        hidden: false,
        group: None,
        priority: None,
        power_scale: None,
        shape: None,
    }
}

fn object_open(id: &str, layer: Rgb, pts: &[(f64, f64)]) -> DesignObject {
    let mut o = rect_object(id, layer, 0.0, 0.0, 1.0, 1.0);
    o.kind = ObjectKind::Vector(VectorObject {
        paths: vec![BezPath::from(&Polyline {
            points: pts.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            closed: false,
        })],
    });
    o
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
        group: None,
        priority: None,
        power_scale: None,
        shape: None,
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
    let dir = std::env::temp_dir().join(format!("tlaku-pipeline-{}-{test}", std::process::id()));
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
    assert_eq!(img.mode, tlaku_raster::RasterMode::default());
    assert_eq!(img.interval_mm, 0.1);
    assert_eq!(img.adjust, tlaku_raster::ImageAdjust::default());
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
        z_mm: None,
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
        tabs: 0,
        z_mm: None,
        pass: None,
    };
    let job = ComputedJob {
        toolpath: tlaku_core::toolpath::Toolpath {
            blocks: vec![block],
        },
        preview: crate::preview::preview(
            &tlaku_core::toolpath::Toolpath::default(),
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
        g.starts_with("; Generado por Tlaku\nG90\nG21\nM4 S0\n"),
        "{g}"
    );
    assert!(
        g.contains("G0 X10 Y20") && g.trim_end().ends_with("G0 X0 Y0"),
        "{g}"
    );
    let mut dev = device();
    dev.power_mode = tlaku_doc::PowerMode::Constant;
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

// ---------------------------------------------------------------------------
// Curvas y texto
// ---------------------------------------------------------------------------

fn svg_with_circle(dir: &Path) -> PathBuf {
    let svg = dir.join("c.svg");
    std::fs::write(
        &svg,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="60mm" viewBox="0 0 100 60">
  <circle cx="50" cy="30" r="20" fill="none" stroke="#ff0000"/></svg>"##,
    )
    .unwrap();
    svg
}

#[test]
fn svg_import_keeps_curves_and_the_cli_mode_flattens_them() {
    let dir = workdir("curvas");
    let svg = svg_with_circle(&dir);
    let r = import_file(&svg).unwrap();
    let ObjectKind::Vector(v) = &r.objects[0].kind else {
        panic!("debía ser vectorial");
    };
    assert!(
        v.paths[0]
            .0
            .iter()
            .any(|s| matches!(s, tlaku_core::bezpath::PathSeg::CubicTo { .. })),
        "el círculo debe seguir siendo curvas"
    );
    // Con keep_curves = false (CLI) solo hay rectas, con la tolerancia pedida.
    let flat = import_file_with(
        &svg,
        &ImportOptions {
            tolerance_mm: 0.5,
            keep_curves: false,
            ..Default::default()
        },
    )
    .unwrap();
    let ObjectKind::Vector(f) = &flat.objects[0].kind else {
        panic!("debía ser vectorial");
    };
    assert!(f.paths[0].0.iter().all(|s| matches!(
        s,
        tlaku_core::bezpath::PathSeg::MoveTo { .. }
            | tlaku_core::bezpath::PathSeg::LineTo { .. }
            | tlaku_core::bezpath::PathSeg::Close
    )));
    // El trabajo aplana el círculo curvo con 0.01 mm: más puntos que a 0.5 mm.
    let curved = r.objects[0].polylines()[0].points.len();
    let coarse = flat.objects[0].polylines()[0].points.len();
    assert!(curved > coarse * 3, "{curved} vs {coarse}");
    for p in &r.objects[0].polylines()[0].points {
        let radius = (p.x - 50.0).hypot(p.y - 30.0);
        assert!((radius - 20.0).abs() < 0.02, "{p:?}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

fn text_object(id: &str, outline: Vec<BezPath>) -> DesignObject {
    DesignObject {
        kind: ObjectKind::Text(tlaku_doc::TextObject {
            text: "A".into(),
            font_family: "Arial".into(),
            bold: false,
            italic: false,
            size_mm: 10.0,
            letter_spacing_mm: 0.0,
            line_spacing: 1.0,
            align: tlaku_doc::TextAlign::Left,
            on_path: None,
            bend_deg: None,
            welded: None,
            outline,
        }),
        ..rect_object(id, RED, 0.0, 0.0, 1.0, 1.0)
    }
}

#[test]
fn text_without_outline_is_skipped_with_a_warning() {
    let mut empty = text_object("t", vec![]);
    empty.transform = IDENTITY;
    let d = doc(
        vec![empty, rect_object("r", RED, 10.0, 10.0, 5.0, 5.0)],
        vec![line_layer(RED)],
    );
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    assert!(
        job.preview
            .warnings
            .iter()
            .any(|w| w.contains("'t'") && w.contains("el texto no tiene contorno generado")),
        "{:?}",
        job.preview.warnings
    );
    assert_eq!(job.layers[0].count, 1);
}

#[test]
fn text_with_outline_is_cut_like_a_vector() {
    let outline = vec![BezPath::from(&Polyline {
        points: vec![
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 6.0),
        ],
        closed: true,
    })];
    let mut t = text_object("t", outline);
    t.transform = affine::translate(20.0, 20.0);
    let d = doc(vec![t], vec![line_layer(RED)]);
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    assert_eq!(job.layers[0].count, 1);
    let b = job.bounds().unwrap();
    point(b.min, 20.0, 20.0);
    point(b.max, 24.0, 26.0);
}

// ---------------------------------------------------------------------------
// Fase 4: puentes, eje Z, rotativo y scan offset
// ---------------------------------------------------------------------------

fn tabbed(layer: LayerSettings) -> LayerSettings {
    let mut l = layer;
    l.tabs.enabled = true;
    l.tabs.width = 2.0;
    l.tabs.count = 4;
    l
}

/// (largo cortado con potencia, largo recorrido con el láser apagado dentro de cortes) del bloque.
fn lengths(block: &ToolpathBlock) -> (f64, f64) {
    let mut at = Point::new(0.0, 0.0);
    let (mut on, mut off) = (0.0, 0.0);
    for mv in &block.moves {
        let to = mv.to();
        match *mv {
            Move::Cut { power_pct, .. } if power_pct > 0.0 => on += at.dist(to),
            Move::Cut { .. } => off += at.dist(to),
            Move::Travel { .. } => {}
        }
        at = to;
    }
    (on, off)
}

#[test]
fn tabs_leave_gaps_crossed_with_the_laser_off() {
    let d = doc(
        vec![rect_object("a", RED, 10.0, 10.0, 40.0, 40.0)],
        vec![tabbed(line_layer(RED))],
    );
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    let (on, off) = lengths(&job.toolpath.blocks[0]);
    // Perímetro 160 mm con 4 huecos de 2 mm.
    approx(on, 152.0);
    approx(off, 8.0);
    assert_eq!(job.layers[0].tabs, 4);
    assert!(job.preview.layers[0].summary.contains("4 puentes"));
    // La vista previa trae los huecos como movimientos con potencia 0 dentro del corte.
    let gaps = job
        .preview
        .moves
        .chunks(4)
        .filter(|m| m[0] == 1.0 && m[3] == 0.0)
        .count();
    assert_eq!(gaps, 4);
    // Y el G-code, como G1 con S0.
    let g = gcode(&job, &device());
    assert_eq!(
        g.lines()
            .filter(|l| l.starts_with("G1") && l.contains("S0"))
            .count(),
        4
    );
    assert!(
        job.preview.warnings.is_empty(),
        "{:?}",
        job.preview.warnings
    );
}

#[test]
fn tabs_do_not_change_the_planned_order_or_touch_open_and_inner_shapes() {
    let open = DesignObject {
        kind: ObjectKind::Vector(VectorObject {
            paths: vec![BezPath::from(&Polyline {
                points: vec![Point::new(60.0, 10.0), Point::new(90.0, 10.0)],
                closed: false,
            })],
        }),
        ..rect_object("open", RED, 0.0, 0.0, 1.0, 1.0)
    };
    let objects = vec![
        rect_object("outer", RED, 10.0, 10.0, 40.0, 40.0),
        rect_object("hole", RED, 20.0, 20.0, 10.0, 10.0),
        open,
    ];
    let plain = compute(
        &doc(objects.clone(), vec![line_layer(RED)]),
        &device(),
        &JobOptions::default(),
    )
    .unwrap();
    let with = compute(
        &doc(objects.clone(), vec![tabbed(line_layer(RED))]),
        &device(),
        &JobOptions::default(),
    )
    .unwrap();
    let order = |j: &ComputedJob| -> Vec<usize> {
        j.layers[0]
            .plan
            .as_ref()
            .unwrap()
            .steps
            .iter()
            .map(|s| s.item)
            .collect()
    };
    assert_eq!(order(&plain), order(&with));
    // Solo el contorno exterior lleva puentes: el agujero y la línea abierta se cortan enteros.
    assert_eq!(with.layers[0].tabs, 4);
    let (_, off) = lengths(&with.toolpath.blocks[0]);
    approx(off, 8.0);
    // Con skip_inner = false, el agujero también.
    let mut all = tabbed(line_layer(RED));
    all.tabs.skip_inner = false;
    let every = compute(&doc(objects, vec![all]), &device(), &JobOptions::default()).unwrap();
    assert_eq!(every.layers[0].tabs, 8);
}

#[test]
fn tabs_go_after_the_kerf() {
    let mut l = tabbed(line_layer(RED));
    l.kerf = 1.0;
    let d = doc(vec![rect_object("a", RED, 10.0, 10.0, 40.0, 40.0)], vec![l]);
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    // El contorno compensado mide 42x42 (perímetro 168, con esquinas redondeadas un poco menos).
    let b = job.layers[0].bounds.unwrap();
    approx(b.max.x - b.min.x, 42.0);
    assert_eq!(job.layers[0].tabs, 4);
}

#[test]
fn tabs_that_do_not_fit_warn_and_bad_values_fail() {
    let small = doc(
        vec![
            rect_object("big", RED, 0.0, 0.0, 40.0, 40.0),
            rect_object("tiny", RED, 60.0, 0.0, 3.0, 3.0),
        ],
        vec![tabbed(line_layer(RED))],
    );
    let job = compute(&small, &device(), &JobOptions::default()).unwrap();
    // El cuadrado de 3 mm (perímetro 12) admite como mucho 3 puentes de 2 mm: 4 + 3.
    assert_eq!(job.layers[0].tabs, 7);
    let w = job.preview.warnings.join("\n");
    assert!(
        w.contains("#ff0000") && w.contains("no caben") && w.contains("12.00 mm"),
        "{w}"
    );

    let mut bad = tabbed(line_layer(RED));
    bad.tabs.width = 0.0;
    let e = compute(
        &doc(vec![rect_object("a", RED, 0.0, 0.0, 9.0, 9.0)], vec![bad]),
        &device(),
        &JobOptions::default(),
    )
    .unwrap_err();
    assert!(e.contains("capa #ff0000") && e.contains("puentes"), "{e}");

    // Sin ninguna forma cerrada donde ponerlos, se avisa.
    let open_only = DesignObject {
        kind: ObjectKind::Vector(VectorObject {
            paths: vec![BezPath::from(&Polyline {
                points: vec![Point::new(0.0, 0.0), Point::new(9.0, 0.0)],
                closed: false,
            })],
        }),
        ..rect_object("o", RED, 0.0, 0.0, 1.0, 1.0)
    };
    let job = compute(
        &doc(vec![open_only], vec![tabbed(line_layer(RED))]),
        &device(),
        &JobOptions::default(),
    )
    .unwrap();
    assert!(
        job.preview
            .warnings
            .iter()
            .any(|w| w.contains("ninguna forma cerrada"))
    );
}

#[test]
fn tabs_apply_to_every_pass_and_ignore_other_modes() {
    let mut two = tabbed(line_layer(RED));
    two.passes = 2;
    let d = doc(vec![rect_object("a", RED, 0.0, 0.0, 40.0, 40.0)], vec![two]);
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    let (_, off) = lengths(&job.toolpath.blocks[0]);
    approx(off, 16.0);
    // En una capa fill, los puentes no hacen nada (el ajuste se conserva al cambiar de modo).
    let fill = LayerSettings {
        tabs: tabbed(line_layer(RED)).tabs,
        ..fill_layer(RED)
    };
    let d = doc(
        vec![rect_object("a", RED, 5.0, 5.0, 10.0, 10.0)],
        vec![fill],
    );
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    assert_eq!(job.layers[0].tabs, 0);
}

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

fn scan_device(offsets: &[(f64, f64)]) -> DeviceProfile {
    DeviceProfile {
        scan_offsets: offsets
            .iter()
            .map(|&(speed, offset)| tlaku_doc::ScanOffset { speed, offset })
            .collect(),
        ..device()
    }
}

/// Diferencia punto a punto entre dos bloques con los mismos movimientos.
fn shifts(a: &ToolpathBlock, b: &ToolpathBlock) -> Vec<(f64, f64)> {
    assert_eq!(a.moves.len(), b.moves.len());
    a.moves
        .iter()
        .zip(&b.moves)
        .map(|(m, n)| (n.to().x - m.to().x, n.to().y - m.to().y))
        .collect()
}

#[test]
fn scan_offset_splits_the_correction_between_forward_and_return_lines() {
    let l = LayerSettings {
        speed: 3000.0,
        interval: 2.0,
        overscan: Overscan::Mm(1.0),
        ..fill_layer(RED)
    };
    let d = doc(vec![rect_object("a", RED, 20.0, 20.0, 12.0, 8.0)], vec![l]);
    let base = compute(&d, &device(), &JobOptions::default()).unwrap();
    // Entre 1000 y 6000 mm/min: a 3000 le toca 0.1 + (0.6 - 0.1) * 2000 / 5000 = 0.3, o sea
    // -0.15 mm en las líneas de ida y +0.15 mm en las de vuelta.
    let dev = scan_device(&[(6000.0, 0.6), (1000.0, 0.1)]);
    let job = compute(&d, &dev, &JobOptions::default()).unwrap();
    let diffs = shifts(&base.toolpath.blocks[0], &job.toolpath.blocks[0]);
    assert!(
        diffs
            .iter()
            .all(|&(dx, dy)| dy == 0.0 && ((dx + 0.15).abs() < 1e-9 || (dx - 0.15).abs() < 1e-9))
    );
    let lines = base.layers[0].count;
    assert_eq!(lines, 4);
    // Las líneas 2 y 4 son de vuelta; cada una tiene 4 movimientos (viaje, overscan, corte, overscan).
    assert_eq!(diffs.iter().filter(|d| d.0 > 0.0).count(), 2 * 4);
    assert_eq!(diffs.iter().filter(|d| d.0 < 0.0).count(), 2 * 4);
    // El centro de las líneas coincide con el nominal: el grabado no se corre respecto de los
    // vectores (antes, las de ida quedaban sin corregir y todo se corría).
    let total: f64 = diffs.iter().map(|d| d.0).sum();
    assert!(total.abs() < 1e-9);
    // Fuera del rango se usa el extremo: a 300 mm/min, 0.1 en total.
    let slow = LayerSettings {
        speed: 300.0,
        interval: 2.0,
        overscan: Overscan::Mm(1.0),
        ..fill_layer(RED)
    };
    let d2 = doc(
        vec![rect_object("a", RED, 20.0, 20.0, 12.0, 8.0)],
        vec![slow],
    );
    let b2 = compute(&d2, &device(), &JobOptions::default()).unwrap();
    let j2 = compute(&d2, &dev, &JobOptions::default()).unwrap();
    let max = shifts(&b2.toolpath.blocks[0], &j2.toolpath.blocks[0])
        .iter()
        .map(|d| d.0)
        .fold(0.0, f64::max);
    approx(max, 0.05);
}

#[test]
fn scan_offset_follows_the_sweep_angle_and_skips_unidirectional_layers() {
    let angle = 30.0f64;
    let l = LayerSettings {
        speed: 1000.0,
        interval: 2.0,
        angle,
        overscan: Overscan::Mm(1.0),
        ..fill_layer(RED)
    };
    let d = doc(
        vec![rect_object("a", RED, 20.0, 20.0, 20.0, 20.0)],
        vec![l.clone()],
    );
    let base = compute(&d, &device(), &JobOptions::default()).unwrap();
    let job = compute(&d, &scan_device(&[(1000.0, 0.5)]), &JobOptions::default()).unwrap();
    let diffs = shifts(&base.toolpath.blocks[0], &job.toolpath.blocks[0]);
    let (cx, cy) = (
        0.25 * angle.to_radians().cos(),
        0.25 * angle.to_radians().sin(),
    );
    assert!(
        diffs.iter().all(|&(dx, dy)| {
            ((dx - cx).abs() < 1e-9 && (dy - cy).abs() < 1e-9)
                || ((dx + cx).abs() < 1e-9 && (dy + cy).abs() < 1e-9)
        }),
        "{diffs:?}"
    );
    assert!(diffs.iter().any(|d| d.0 > 0.0) && diffs.iter().any(|d| d.0 < 0.0));
    // Sin bidireccional no hay líneas de vuelta.
    let uni = LayerSettings {
        bidirectional: false,
        ..l
    };
    let d = doc(
        vec![rect_object("a", RED, 20.0, 20.0, 20.0, 20.0)],
        vec![uni],
    );
    let a = compute(&d, &device(), &JobOptions::default()).unwrap();
    let b = compute(&d, &scan_device(&[(1000.0, 0.5)]), &JobOptions::default()).unwrap();
    assert_eq!(a.toolpath, b.toolpath);
}

#[test]
fn scan_initial_offset_moves_every_line_even_without_a_table() {
    let l = LayerSettings {
        speed: 1000.0,
        interval: 2.0,
        overscan: Overscan::Mm(1.0),
        bidirectional: false,
        ..fill_layer(RED)
    };
    let d = doc(vec![rect_object("a", RED, 20.0, 20.0, 12.0, 8.0)], vec![l]);
    let base = compute(&d, &device(), &JobOptions::default()).unwrap();
    let dev = DeviceProfile {
        scan_initial_offset: 0.2,
        ..device()
    };
    let job = compute(&d, &dev, &JobOptions::default()).unwrap();
    let diffs = shifts(&base.toolpath.blocks[0], &job.toolpath.blocks[0]);
    assert!(
        diffs
            .iter()
            .all(|&(dx, dy)| dy == 0.0 && (dx - 0.2).abs() < 1e-9)
    );
}

#[test]
fn scan_offset_warns_when_the_overscan_is_shorter_than_the_offset() {
    let l = |overscan| LayerSettings {
        speed: 3000.0,
        interval: 2.0,
        overscan: Overscan::Mm(overscan),
        ..fill_layer(RED)
    };
    let dev = scan_device(&[(3000.0, 0.4)]);
    let d = doc(
        vec![rect_object("a", RED, 20.0, 20.0, 12.0, 8.0)],
        vec![l(0.2)],
    );
    let job = compute(&d, &dev, &JobOptions::default()).unwrap();
    let w = job.preview.warnings.join("\n");
    assert!(
        w.contains("overscan") && w.contains("0.20") && w.contains("0.40"),
        "{w}"
    );
    let d = doc(
        vec![rect_object("a", RED, 20.0, 20.0, 12.0, 8.0)],
        vec![l(1.0)],
    );
    let job = compute(&d, &dev, &JobOptions::default()).unwrap();
    assert!(
        job.preview.warnings.is_empty(),
        "{:?}",
        job.preview.warnings
    );
}

#[test]
fn scan_offset_shifts_the_rows_of_an_image() {
    let dir = workdir("scan-image");
    let path = write_gradient(&dir);
    let l = LayerSettings {
        speed: 3000.0,
        overscan: Overscan::Mm(0.0),
        interval: 1.0,
        ..LayerSettings::for_images()
    };
    let d = doc(
        vec![image_object("img", &path, [16.0, 4.0], IDENTITY)],
        vec![l],
    );
    let base = build(&d, &device(), &JobOptions::default(), None).unwrap();
    let dev = scan_device(&[(3000.0, 0.25)]);
    let job = build(&d, &dev, &JobOptions::default(), None).unwrap();
    let diffs = shifts(&base.toolpath.blocks[0], &job.toolpath.blocks[0]);
    assert!(
        diffs
            .iter()
            .all(|&(dx, dy)| dy == 0.0 && ((dx - 0.125).abs() < 1e-9 || (dx + 0.125).abs() < 1e-9))
    );
    assert!(diffs.iter().any(|d| d.0 > 0.0) && diffs.iter().any(|d| d.0 < 0.0));
    // Sin overscan, el aviso del overscan menor que el offset.
    assert!(
        job.preview.warnings.iter().any(|w| w.contains("overscan")),
        "{:?}",
        job.preview.warnings
    );
    std::fs::remove_dir_all(dir).unwrap();
}

// ---------------------------------------------------------------------------
// Fase 4 (revisión del taller): Z segura, puentes y kerf, espejo del rotativo, dialectos
// ---------------------------------------------------------------------------

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

#[test]
fn tab_width_is_checked_against_the_kerf() {
    let tabbed_with = |width: f64, kerf: f64| LayerSettings {
        kerf,
        ..LayerSettings {
            tabs: tlaku_doc::TabSettings {
                width,
                ..tabbed(line_layer(RED)).tabs
            },
            ..tabbed(line_layer(RED))
        }
    };
    let run = |width: f64, kerf: f64| {
        let d = doc(
            vec![rect_object("a", RED, 5.0, 5.0, 30.0, 30.0)],
            vec![tabbed_with(width, kerf)],
        );
        compute(&d, &device(), &JobOptions::default())
            .unwrap()
            .preview
            .warnings
            .join("\n")
    };
    // El kerf es el desplazamiento por lado: el haz quita 2 * kerf del puente.
    let strong = run(0.1, 0.1);
    assert!(
        strong.contains("no sobreviven") && strong.contains("0.2"),
        "{strong}"
    );
    let soft = run(0.3, 0.1);
    assert!(soft.contains("frágiles") && soft.contains("0.10"), "{soft}");
    assert_eq!(run(0.4, 0.1), "");
    // Sin kerf no hay nada que comparar.
    assert_eq!(run(0.1, 0.0), "");
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

#[test]
fn dialect_limits_are_reported() {
    let d = doc(
        vec![rect_object("a", RED, 5.0, 5.0, 10.0, 10.0)],
        vec![LayerSettings {
            air: true,
            ..line_layer(RED)
        }],
    );
    // GRBL 1.1: nada que decir.
    let job = compute(&d, &device(), &JobOptions::default()).unwrap();
    assert!(job.preview.warnings.is_empty());
    // Sin aire en el dialecto y con potencia dinámica pedida en un firmware que no la tiene.
    let dev = DeviceProfile {
        dialect: tlaku_doc::GcodeDialect {
            air_on: None,
            air_off: None,
            dynamic_on: None,
            ..Default::default()
        },
        ..device()
    };
    let w = compute(&d, &dev, &JobOptions::default())
        .unwrap()
        .preview
        .warnings
        .join("\n");
    assert!(
        w.contains("aire") && w.contains("potencia constante"),
        "{w}"
    );
    // Un perfil nuevo de Marlin usa su dialecto y no escribe M4.
    let marlin = DeviceProfile {
        name: "Marlin".into(),
        ..DeviceProfile::for_kind(tlaku_doc::DeviceKind::Marlin)
    };
    let job = compute(&d, &marlin, &JobOptions::default()).unwrap();
    assert!(
        job.preview.warnings.is_empty(),
        "{:?}",
        job.preview.warnings
    );
    let g = gcode(&job, &marlin);
    assert!(
        g.contains("M3 S0 I\n") && !g.contains("M4 S") && g.ends_with("M400\n"),
        "{g}"
    );
    assert!(g.contains("G0 X5 Y5 F12000\n"), "{g}");
    // Las imágenes con la potencia en una línea aparte salen con manchas: se avisa.
    let dir = workdir("dialect-image");
    let path = write_gradient(&dir);
    let img = doc(
        vec![image_object("img", &path, [16.0, 4.0], IDENTITY)],
        vec![LayerSettings::for_images()],
    );
    let sep = DeviceProfile {
        dialect: tlaku_doc::GcodeDialect::marlin_standard(),
        ..marlin.clone()
    };
    let job = build(&img, &sep, &JobOptions::default(), None).unwrap();
    assert!(
        job.preview
            .warnings
            .iter()
            .any(|w| w.contains("línea aparte")),
        "{:?}",
        job.preview.warnings
    );
    std::fs::remove_dir_all(dir).unwrap();
}
