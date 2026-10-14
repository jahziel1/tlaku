//! Ola 2: sub-capas, prioridad y grupo por objeto, ajustes de optimización, incremento de
//! ángulo y cortar solo la selección. Todo por la API pública (`build` / `compute`).

use std::path::{Path, PathBuf};
use tlaku_core::bezpath::BezPath;
use tlaku_core::toolpath::{Move, ToolpathBlock};
use tlaku_core::{Point, Polyline, Rgb};
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{
    DesignObject, DeviceProfile, Document, FillStyle, ImageObject, JobOptions, LayerMode,
    LayerOrder, LayerSettings, ObjectKind, Optimization, Overscan, StartFrom, VectorObject,
};
use tlaku_pipeline::{ComputedJob, build};

/// Como `compute` sin verificar la cama: los diseños de prueba tocan el origen y el overscan
/// se sale un poco.
fn compute(
    doc: &Document,
    device: &DeviceProfile,
    options: &JobOptions,
) -> Result<ComputedJob, String> {
    build(doc, device, options, None)
}

const RED: Rgb = Rgb(255, 0, 0);
const BLUE: Rgb = Rgb(0, 0, 255);
const GREEN: Rgb = Rgb(0, 255, 0);

// ---------------------------------------------------------------------------
// Apoyos
// ---------------------------------------------------------------------------

fn shape(id: &str, layer: Rgb, closed: bool, pts: &[(f64, f64)]) -> DesignObject {
    DesignObject {
        id: id.into(),
        layer,
        transform: IDENTITY,
        kind: ObjectKind::Vector(VectorObject::new(vec![BezPath::from(&Polyline {
            points: pts.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            closed,
        })])),
        locked: false,
        hidden: false,
        group: None,
        priority: None,
        power_scale: None,
        shape: None,
        start: None,
        manual_tabs: Vec::new(),
        array: None,
    }
}

fn rect(id: &str, layer: Rgb, x: f64, y: f64, w: f64, h: f64) -> DesignObject {
    shape(
        id,
        layer,
        true,
        &[(x, y), (x + w, y), (x + w, y + h), (x, y + h)],
    )
}

fn doc(objects: Vec<DesignObject>, layers: Vec<LayerSettings>) -> Document {
    Document {
        objects,
        layers,
        ..Document::default()
    }
}

fn line(color: Rgb) -> LayerSettings {
    LayerSettings::for_color(color)
}

fn fill(color: Rgb) -> LayerSettings {
    LayerSettings {
        mode: LayerMode::Fill,
        interval: 1.0,
        ..LayerSettings::for_color(color)
    }
}

fn with_opt(f: impl FnOnce(&mut Optimization)) -> JobOptions {
    let mut optimization = Optimization::default();
    f(&mut optimization);
    JobOptions {
        optimization,
        ..JobOptions::default()
    }
}

fn dev() -> DeviceProfile {
    DeviceProfile::default()
}

/// Inicio de cada figura de un bloque de líneas (cada `Travel` abre una).
fn starts(block: &ToolpathBlock) -> Vec<Point> {
    block
        .moves
        .iter()
        .filter_map(|m| match m {
            Move::Travel { to } => Some(*to),
            _ => None,
        })
        .collect()
}

/// Cada figura del bloque como lista de puntos.
fn polygons(block: &ToolpathBlock) -> Vec<Vec<Point>> {
    let mut out: Vec<Vec<Point>> = Vec::new();
    for m in &block.moves {
        match m {
            Move::Travel { to } => out.push(vec![*to]),
            Move::Cut { to, .. } => out.last_mut().unwrap().push(*to),
            Move::Dwell { .. } => {}
        }
    }
    out
}

fn signed_area2(pts: &[Point]) -> f64 {
    let n = pts.len();
    (0..n)
        .map(|i| pts[i].x * pts[(i + 1) % n].y - pts[(i + 1) % n].x * pts[i].y)
        .sum()
}

fn cut_length(block: &ToolpathBlock) -> f64 {
    let mut pos = Point::new(0.0, 0.0);
    let mut total = 0.0;
    for m in &block.moves {
        if let Move::Cut { to, .. } = m {
            total += pos.dist(*to);
        }
        pos = m.to();
    }
    total
}

fn colors(job: &ComputedJob) -> Vec<(Rgb, &'static str)> {
    job.layers.iter().map(|l| (l.color, l.mode)).collect()
}

fn workdir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tlaku-orden-{}-{test}", std::process::id()));
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

fn gradient_image(dir: &Path) -> DesignObject {
    let path = dir.join("degradado.bmp");
    std::fs::write(&path, gray_bmp(16, 16, |x, _| (x * 16).min(255) as u8)).unwrap();
    DesignObject {
        id: "foto".into(),
        layer: LayerSettings::IMAGE_COLOR,
        transform: IDENTITY,
        kind: ObjectKind::Image(ImageObject {
            source_path: path.to_str().unwrap().into(),
            width_px: 16,
            height_px: 16,
            size_mm: [8.0, 8.0],
            mask: Vec::new(),
        }),
        locked: false,
        hidden: false,
        group: None,
        priority: None,
        power_scale: None,
        shape: None,
        start: None,
        manual_tabs: Vec::new(),
        array: None,
    }
}

// ---------------------------------------------------------------------------
// Sub-capas
// ---------------------------------------------------------------------------

/// Relleno con una sub-capa de línea que contornea los mismos objetos.
fn fill_with_outline(color: Rgb) -> LayerSettings {
    LayerSettings {
        sub_layers: vec![LayerSettings {
            speed: 300.0,
            power: 50.0,
            ..line(color)
        }],
        ..fill(color)
    }
}

#[test]
fn a_sub_layer_runs_on_the_same_objects_after_the_main_one() {
    let d = doc(
        vec![rect("a", RED, 0.0, 0.0, 10.0, 10.0)],
        vec![fill_with_outline(RED)],
    );
    let job = compute(&d, &dev(), &JobOptions::default()).unwrap();
    assert_eq!(colors(&job), vec![(RED, "fill"), (RED, "line")]);
    assert_eq!(job.layers[0].name, "#ff0000 · 1/2");
    assert_eq!(job.layers[1].name, "#ff0000 · 2/2");
    assert_eq!(job.layers[0].sub, Some((1, 2)));
    assert_eq!(job.layers[1].sub, Some((2, 2)));
    // La sub-capa usa sus propios ajustes y el contorno de los mismos objetos.
    assert_eq!(job.layers[1].speed_mm_min, 300.0);
    assert_eq!(job.layers[1].power, "50%");
    let outline = &job.toolpath.blocks[1];
    assert!((cut_length(outline) - 40.0).abs() < 1e-6);
    // Una fila de vista previa por operación, identificada como sub-capa.
    assert_eq!(job.preview.layers.len(), 2);
    assert!(job.preview.layers[0].summary.contains("sub-capa 1/2"));
    assert!(job.preview.layers[1].summary.contains("sub-capa 2/2"));
    // Sin sub-capas nada de esto aparece.
    let plain = doc(vec![rect("a", RED, 0.0, 0.0, 10.0, 10.0)], vec![fill(RED)]);
    let job = compute(&plain, &dev(), &JobOptions::default()).unwrap();
    assert_eq!(job.layers[0].name, "#ff0000");
    assert_eq!(job.layers[0].sub, None);
}

#[test]
fn a_cutting_sub_layer_goes_last_with_the_other_cuts_in_auto_order() {
    // Capa roja: relleno + sub-capa de corte (kerf). Capa azul: otro relleno después en el archivo.
    let red = LayerSettings {
        sub_layers: vec![LayerSettings {
            kerf: 0.1,
            ..line(RED)
        }],
        ..fill(RED)
    };
    let d = doc(
        vec![
            rect("a", RED, 0.0, 0.0, 10.0, 10.0),
            rect("b", BLUE, 20.0, 0.0, 10.0, 10.0),
        ],
        vec![red, fill(BLUE)],
    );
    let auto = compute(&d, &dev(), &JobOptions::default()).unwrap();
    assert_eq!(
        colors(&auto),
        vec![(RED, "fill"), (BLUE, "fill"), (RED, "line")]
    );
    let note = auto.preview.order_note.as_deref().unwrap();
    assert!(
        note.contains("sub-capas se ordenan por su propio rol"),
        "{note}"
    );
    // En el orden del archivo cada capa va seguida de sus sub-capas.
    let file = compute(
        &d,
        &dev(),
        &JobOptions {
            layer_order: LayerOrder::File,
            ..JobOptions::default()
        },
    )
    .unwrap();
    assert_eq!(
        colors(&file),
        vec![(RED, "fill"), (RED, "line"), (BLUE, "fill")]
    );
}

#[test]
fn a_sub_layer_never_runs_before_its_own_layer() {
    // Capa de corte con una sub-capa de relleno: el relleno no se adelanta al corte de su capa.
    let cut = LayerSettings {
        kerf: 0.1,
        sub_layers: vec![fill(RED)],
        ..line(RED)
    };
    let d = doc(
        vec![
            rect("a", RED, 0.0, 0.0, 10.0, 10.0),
            rect("b", BLUE, 20.0, 0.0, 10.0, 10.0),
        ],
        vec![cut, fill(BLUE)],
    );
    let job = compute(&d, &dev(), &JobOptions::default()).unwrap();
    // El grabado azul va primero; la capa roja y su sub-capa quedan juntas y en su orden.
    assert_eq!(
        colors(&job),
        vec![(BLUE, "fill"), (RED, "line"), (RED, "fill")]
    );
}

#[test]
fn a_pinned_layer_takes_its_sub_layers_with_it() {
    let red = LayerSettings {
        order: Some(2),
        ..fill_with_outline(RED)
    };
    let d = doc(
        vec![
            rect("a", RED, 0.0, 0.0, 10.0, 10.0),
            rect("b", BLUE, 20.0, 0.0, 10.0, 10.0),
            rect("c", GREEN, 40.0, 0.0, 10.0, 10.0),
        ],
        vec![red, fill(BLUE), fill(GREEN)],
    );
    let job = compute(&d, &dev(), &JobOptions::default()).unwrap();
    assert_eq!(
        colors(&job),
        vec![
            (BLUE, "fill"),
            (RED, "fill"),
            (RED, "line"),
            (GREEN, "fill")
        ]
    );
}

#[test]
fn sub_layers_ignore_color_name_visible_order_and_their_own_sub_layers() {
    let inner = LayerSettings {
        power: 99.0,
        ..line(Rgb(1, 2, 3))
    };
    let sub = LayerSettings {
        color: Rgb(9, 9, 9),
        name: "otro".into(),
        visible: false,
        order: Some(7),
        sub_layers: vec![inner],
        power: 33.0,
        ..line(RED)
    };
    let main = LayerSettings {
        sub_layers: vec![sub],
        ..fill(RED)
    };
    let d = doc(vec![rect("a", RED, 0.0, 0.0, 10.0, 10.0)], vec![main]);
    let job = compute(&d, &dev(), &JobOptions::default()).unwrap();
    // Dos filas (la sub-capa de la sub-capa no existe) y con el color de la capa.
    assert_eq!(colors(&job), vec![(RED, "fill"), (RED, "line")]);
    assert_eq!(job.layers[1].power, "33%");
    // Su `order` no cuenta para el "order=" duplicado ni para colocarla.
    assert!(!job.preview.warnings.iter().any(|w| w.contains("order")));
}

#[test]
fn layer_output_governs_over_sub_layers_and_a_sub_layer_can_be_off() {
    let mut off = LayerSettings {
        output: false,
        ..line(RED)
    };
    off.speed = 123.0;
    let main = LayerSettings {
        sub_layers: vec![off, line(RED)],
        ..fill(RED)
    };
    let d = doc(
        vec![rect("a", RED, 0.0, 0.0, 10.0, 10.0)],
        vec![main.clone()],
    );
    let job = compute(&d, &dev(), &JobOptions::default()).unwrap();
    // La sub-capa apagada no sale; la numeración sigue siendo la de la capa (1/3, 3/3).
    assert_eq!(job.layers.len(), 2);
    assert_eq!(job.layers[0].sub, Some((1, 3)));
    assert_eq!(job.layers[1].sub, Some((3, 3)));
    let mut d2 = d.clone();
    d2.layers[0].output = false;
    assert!(compute(&d2, &dev(), &JobOptions::default()).is_err());
}

#[test]
fn too_many_sub_layers_and_unusable_sub_layers_are_reported() {
    let many = LayerSettings {
        sub_layers: vec![line(RED); 12],
        ..fill(RED)
    };
    let d = doc(vec![rect("a", RED, 0.0, 0.0, 10.0, 10.0)], vec![many]);
    let e = compute(&d, &dev(), &JobOptions::default()).unwrap_err();
    assert!(
        e.contains("demasiadas sub-capas") && e.contains("11"),
        "{e}"
    );

    // Una sub-capa de imagen sobre objetos vectoriales no puede grabarlos: se avisa una vez.
    let main = LayerSettings {
        sub_layers: vec![LayerSettings::for_images()],
        ..fill(RED)
    };
    let d = doc(
        vec![
            rect("a", RED, 0.0, 0.0, 10.0, 10.0),
            rect("b", RED, 20.0, 0.0, 10.0, 10.0),
        ],
        vec![main],
    );
    let job = compute(&d, &dev(), &JobOptions::default()).unwrap();
    assert_eq!(job.layers.len(), 1);
    let w: Vec<&String> = job
        .preview
        .warnings
        .iter()
        .filter(|w| w.contains("sub-capa 1"))
        .collect();
    assert_eq!(w.len(), 1, "{:?}", job.preview.warnings);
    assert!(w[0].contains("2 objeto(s)") && w[0].contains("image"));
}

#[test]
fn sub_layers_split_into_pass_blocks_with_their_own_z() {
    let sub = LayerSettings {
        passes: 2,
        z_step: 0.5,
        ..line(RED)
    };
    let main = LayerSettings {
        sub_layers: vec![sub],
        ..fill(RED)
    };
    let d = doc(vec![rect("a", RED, 0.0, 0.0, 10.0, 10.0)], vec![main]);
    let device = DeviceProfile {
        z_axis: true,
        ..dev()
    };
    let job = compute(&d, &device, &JobOptions::default()).unwrap();
    assert_eq!(job.layers.len(), 3);
    assert_eq!(job.layers[1].pass, Some((1, 2)));
    assert_eq!(job.layers[2].pass, Some((2, 2)));
    assert_eq!(job.layers[2].sub, Some((2, 2)));
    assert_eq!(job.layers[2].z_mm, Some(0.5));
}

// ---------------------------------------------------------------------------
// Prioridad y grupo por objeto, y opciones de optimización
// ---------------------------------------------------------------------------

/// Tres cuadrados de 5 mm a 30 mm uno de otro.
fn three_squares(priorities: [Option<i32>; 3]) -> Document {
    let mut objects: Vec<DesignObject> = (0..3)
        .map(|i| rect(&format!("r{i}"), RED, 30.0 * i as f64, 0.0, 5.0, 5.0))
        .collect();
    for (o, p) in objects.iter_mut().zip(priorities) {
        o.priority = p;
    }
    doc(objects, vec![line(RED)])
}

/// Qué cuadrado es cada figura del bloque (0, 1 o 2 según su X).
fn which(block: &ToolpathBlock) -> Vec<usize> {
    starts(block)
        .iter()
        .map(|p| (p.x / 30.0).round() as usize)
        .collect()
}

#[test]
fn object_priority_orders_the_cut_unless_by_priority_is_off() {
    let d = three_squares([Some(2), Some(0), Some(1)]);
    let job = compute(&d, &dev(), &JobOptions::default()).unwrap();
    assert_eq!(which(&job.toolpath.blocks[0]), vec![1, 2, 0]);
    let explain = &job.preview.layers[0].explanations;
    assert!(
        explain.iter().any(|e| e.contains("prioridad 1")),
        "{explain:?}"
    );

    let off = with_opt(|o| o.by_priority = false);
    let job = compute(&d, &dev(), &off).unwrap();
    assert_eq!(which(&job.toolpath.blocks[0]), vec![0, 1, 2]);
    // Cambiar una opción se explica en la vista previa.
    let note = job.preview.order_note.as_deref().unwrap();
    assert!(
        note.contains("optimización:") && note.contains("orden por capa"),
        "{note}"
    );
    assert!(!note.contains("prioridad"), "{note}");

    // Sin prioridades todo sigue igual que antes.
    let plain = three_squares([None, None, None]);
    let job = compute(&plain, &dev(), &JobOptions::default()).unwrap();
    assert_eq!(which(&job.toolpath.blocks[0]), vec![0, 1, 2]);
    assert!(job.preview.order_note.is_none());
}

#[test]
fn object_groups_are_cut_together_only_with_by_group() {
    let mut objects: Vec<DesignObject> = (0..4)
        .map(|i| rect(&format!("r{i}"), RED, 10.0 * i as f64, 0.0, 2.0, 2.0))
        .collect();
    for (i, o) in objects.iter_mut().enumerate() {
        o.group = Some(if i % 2 == 0 { "a" } else { "b" }.into());
    }
    let d = doc(objects, vec![line(RED)]);
    let by10 = |b: &ToolpathBlock| -> Vec<usize> {
        starts(b)
            .iter()
            .map(|p| (p.x / 10.0).round() as usize)
            .collect()
    };
    let job = compute(&d, &dev(), &JobOptions::default()).unwrap();
    assert_eq!(by10(&job.toolpath.blocks[0]), vec![0, 1, 2, 3]);
    let job = compute(&d, &dev(), &with_opt(|o| o.by_group = true)).unwrap();
    assert_eq!(by10(&job.toolpath.blocks[0]), vec![0, 2, 1, 3]);
    let explain = &job.preview.layers[0].explanations;
    assert!(explain.iter().any(|e| e.contains("grupo")), "{explain:?}");
}

#[test]
fn travel_and_start_options_reach_the_planner() {
    // Un cuadrado grande con uno pequeño dentro, y uno lejos.
    let d = doc(
        vec![
            rect("grande", RED, 0.0, 0.0, 20.0, 20.0),
            rect("lejos", RED, 100.0, 0.0, 5.0, 5.0),
            rect("dentro", RED, 5.0, 5.0, 5.0, 5.0),
        ],
        vec![line(RED)],
    );
    // Área (por 2) de cada figura en el orden de corte: dentro = 50, lejos = 50, grande = 800.
    let areas = |job: &ComputedJob| -> Vec<f64> {
        polygons(&job.toolpath.blocks[0])
            .iter()
            .map(|p| signed_area2(p).abs().round())
            .collect()
    };
    let on = compute(&d, &dev(), &JobOptions::default()).unwrap();
    let xs_on: Vec<f64> = starts(&on.toolpath.blocks[0]).iter().map(|p| p.x).collect();
    // "dentro" sale antes que "grande".
    let (dentro, grande) = (
        polygons(&on.toolpath.blocks[0])
            .iter()
            .position(|p| p.iter().all(|q| q.x <= 10.0 && q.y <= 10.0))
            .unwrap(),
        areas(&on).iter().position(|a| *a == 800.0).unwrap(),
    );
    assert!(dentro < grande, "{xs_on:?}");

    let inner_off = compute(&d, &dev(), &with_opt(|o| o.inner_first = false)).unwrap();
    let note = inner_off.preview.order_note.as_deref().unwrap();
    assert!(note.contains("interiores primero apagado"), "{note}");

    // Sin reducir recorridos ni interiores primero se respeta el orden del archivo.
    let plain = compute(
        &d,
        &dev(),
        &with_opt(|o| {
            o.reduce_travel = false;
            o.inner_first = false;
        }),
    )
    .unwrap();
    assert_eq!(areas(&plain), vec![800.0, 50.0, 50.0]);
    let xs: Vec<f64> = polygons(&plain.toolpath.blocks[0])
        .iter()
        .map(|p| p[0].x)
        .collect();
    assert!(xs[1] >= 100.0 && xs[2] < 20.0, "{xs:?}");

    // best_start apagado: cada cerrada empieza en su primer vértice.
    let first = compute(&d, &dev(), &with_opt(|o| o.best_start = false)).unwrap();
    let mut got: Vec<(i64, i64)> = starts(&first.toolpath.blocks[0])
        .iter()
        .map(|p| (p.x.round() as i64, p.y.round() as i64))
        .collect();
    got.sort_unstable();
    assert_eq!(got, vec![(0, 0), (5, 5), (100, 0)]);
}

#[test]
fn fill_by_group_sweeps_each_document_group_together() {
    let mut objects = vec![
        rect("a", RED, 0.0, 0.0, 10.0, 6.0),
        rect("b", RED, 20.0, 0.0, 10.0, 6.0),
        rect("c", RED, 40.0, 0.0, 10.0, 6.0),
    ];
    // a y c son del grupo de más afuera "g" (c además en uno interno); b es de otro.
    objects[0].group = Some("g".into());
    objects[1].group = Some("h".into());
    objects[2].group = Some("g/x".into());
    let layer = |grouping| LayerSettings {
        fill_grouping: grouping,
        overscan: Overscan::Mm(0.0),
        ..fill(RED)
    };
    // Región de cada tramo grabado (potencia > 0), en el orden de ejecución.
    let order = |job: &ComputedJob| -> String {
        let mut prev = Point::new(0.0, 0.0);
        let mut out = String::new();
        for m in &job.toolpath.blocks[0].moves {
            if let Move::Cut { to, power_pct } = m
                && *power_pct > 0.0
            {
                let mid = (prev.x + to.x) / 2.0;
                out.push(if (15.0..35.0).contains(&mid) {
                    'b'
                } else {
                    'g'
                });
            }
            prev = m.to();
        }
        out
    };
    let all = doc(objects.clone(), vec![layer(tlaku_doc::FillGrouping::All)]);
    let by = doc(objects, vec![layer(tlaku_doc::FillGrouping::ByGroup)]);
    let all = order(&compute(&all, &dev(), &JobOptions::default()).unwrap());
    let by = order(&compute(&by, &dev(), &JobOptions::default()).unwrap());
    // Todo junto: las líneas de b se intercalan con las de a y c.
    let first_b = all.find('b').unwrap();
    assert!(all[first_b..].contains('g'), "{all}");
    // Por grupo: primero todo `g` (a y c) y después todo `h` (b).
    let first_b = by.find('b').unwrap();
    assert!(by[first_b..].chars().all(|c| c == 'b'), "{by}");
    assert!(first_b > 0, "{by}");
}

#[test]
fn remove_overlaps_cuts_a_shared_edge_once() {
    let d = doc(
        vec![
            rect("a", RED, 0.0, 0.0, 10.0, 10.0),
            rect("b", RED, 10.0, 0.0, 10.0, 10.0),
        ],
        vec![line(RED)],
    );
    let before = compute(&d, &dev(), &JobOptions::default()).unwrap();
    assert!((cut_length(&before.toolpath.blocks[0]) - 80.0).abs() < 1e-6);
    let job = compute(&d, &dev(), &with_opt(|o| o.remove_overlaps = true)).unwrap();
    assert!((cut_length(&job.toolpath.blocks[0]) - 70.0).abs() < 1e-6);
    assert!(
        job.preview
            .warnings
            .iter()
            .any(|w| w.contains("remove_overlaps quitó 10.00 mm")),
        "{:?}",
        job.preview.warnings
    );
    let note = job.preview.order_note.as_deref().unwrap();
    assert!(note.contains("tramos repetidos quitados"), "{note}");

    // Con kerf no se aplica (las abiertas que quedan no se podrían compensar): se avisa.
    let mut kerf = d.clone();
    kerf.layers[0].kerf = 0.05;
    let job = compute(&kerf, &dev(), &with_opt(|o| o.remove_overlaps = true)).unwrap();
    assert!(
        job.preview
            .warnings
            .iter()
            .any(|w| w.contains("remove_overlaps no se aplica con kerf"))
    );

    // Una tolerancia inválida se rechaza.
    let e = compute(&d, &dev(), &with_opt(|o| o.overlap_tolerance = -1.0)).unwrap_err();
    assert!(e.contains("overlap_tolerance"), "{e}");
    let e = compute(&d, &dev(), &with_opt(|o| o.overlap_tolerance = f64::NAN)).unwrap_err();
    assert!(e.contains("overlap_tolerance"), "{e}");
}

#[test]
fn remove_overlaps_keeps_each_survivor_priority() {
    // `b` se solapa con `a` y tiene prioridad 0; `c` va después por prioridad.
    let mut a = rect("a", RED, 0.0, 0.0, 10.0, 10.0);
    a.priority = Some(5);
    let mut b = rect("b", RED, 10.0, 0.0, 10.0, 10.0);
    b.priority = Some(0);
    let mut c = rect("c", RED, 50.0, 0.0, 10.0, 10.0);
    c.priority = Some(3);
    let d = doc(vec![a, b, c], vec![line(RED)]);
    let job = compute(&d, &dev(), &with_opt(|o| o.remove_overlaps = true)).unwrap();
    let block = &job.toolpath.blocks[0];
    // `a` pierde el tramo compartido (queda abierta) pero conserva su prioridad 5: es la última.
    let last = polygons(block).pop().unwrap();
    assert!(last.iter().all(|p| p.x <= 10.0 + 1e-9), "{last:?}");
    let first = &polygons(block)[0];
    assert!(first.iter().all(|p| p.x >= 10.0 - 1e-9), "{first:?}");
}

#[test]
fn hide_backlash_cuts_every_closed_shape_the_same_way_and_scans_one_way() {
    let cw = shape(
        "cw",
        RED,
        true,
        &[(40.0, 0.0), (40.0, 10.0), (50.0, 10.0), (50.0, 0.0)],
    );
    let d = doc(
        vec![
            rect("ccw", RED, 0.0, 0.0, 10.0, 10.0),
            cw,
            shape("abierta", RED, false, &[(80.0, 0.0), (90.0, 5.0)]),
        ],
        vec![line(RED)],
    );
    let signs = |job: &ComputedJob| -> Vec<f64> {
        polygons(&job.toolpath.blocks[0])
            .iter()
            .filter(|p| p.len() > 3)
            .map(|p| signed_area2(p).signum())
            .collect()
    };
    let plain = compute(&d, &dev(), &JobOptions::default()).unwrap();
    assert!(signs(&plain).contains(&-1.0) && signs(&plain).contains(&1.0));
    let job = compute(&d, &dev(), &with_opt(|o| o.hide_backlash = true)).unwrap();
    assert_eq!(signs(&job), vec![1.0, 1.0]);
    assert!(
        job.preview
            .warnings
            .iter()
            .any(|w| w.contains("hide_backlash") && w.contains("más lento"))
    );
    // La abierta conserva su sentido (de izquierda a derecha, como se dibujó).
    let open = polygons(&job.toolpath.blocks[0])
        .into_iter()
        .find(|p| p.len() == 2)
        .unwrap();
    assert!(open[0].x < open[1].x);

    // Los rellenos se graban en un solo sentido.
    let f = doc(
        vec![rect("a", RED, 0.0, 0.0, 10.0, 10.0)],
        vec![LayerSettings {
            overscan: Overscan::Mm(2.0),
            ..fill(RED)
        }],
    );
    let going_back = |job: &ComputedJob| {
        let mut back = false;
        let mut prev = Point::new(0.0, 0.0);
        for m in &job.toolpath.blocks[0].moves {
            match m {
                Move::Travel { to } => prev = *to,
                Move::Cut { to, .. } => {
                    back |= to.x < prev.x - 1e-9;
                    prev = *to;
                }
                Move::Dwell { .. } => {}
            }
        }
        back
    };
    assert!(going_back(
        &compute(&f, &dev(), &JobOptions::default()).unwrap()
    ));
    assert!(!going_back(
        &compute(&f, &dev(), &with_opt(|o| o.hide_backlash = true)).unwrap()
    ));
}

#[test]
fn without_by_layer_cut_layers_with_the_same_settings_are_planned_together() {
    // Rojo en 0 y 40, azul en 20 con los mismos ajustes: mezclados se cortan 0, 20, 40.
    let d = doc(
        vec![
            rect("r1", RED, 0.0, 0.0, 5.0, 5.0),
            rect("b1", BLUE, 20.0, 0.0, 5.0, 5.0),
            rect("r2", RED, 40.0, 0.0, 5.0, 5.0),
        ],
        vec![line(RED), line(BLUE)],
    );
    let by20 = |b: &ToolpathBlock| -> Vec<usize> {
        starts(b)
            .iter()
            .map(|p| (p.x / 20.0).round() as usize)
            .collect()
    };
    let sep = compute(&d, &dev(), &JobOptions::default()).unwrap();
    assert_eq!(sep.layers.len(), 2);
    assert_eq!(by20(&sep.toolpath.blocks[0]), vec![0, 2]);
    assert_eq!(by20(&sep.toolpath.blocks[1]), vec![1]);

    let mixed = compute(&d, &dev(), &with_opt(|o| o.by_layer = false)).unwrap();
    assert_eq!(mixed.layers.len(), 1);
    assert_eq!(mixed.layers[0].count, 3);
    assert_eq!(by20(&mixed.toolpath.blocks[0]), vec![0, 1, 2]);
    let note = mixed.preview.order_note.as_deref().unwrap();
    assert!(note.contains("#ff0000 + #0000ff"), "{note}");

    // Con ajustes distintos no se mezclan, y se dice por qué.
    let mut other = d.clone();
    other.layers[1].power = 90.0;
    let job = compute(&other, &dev(), &with_opt(|o| o.by_layer = false)).unwrap();
    assert_eq!(job.layers.len(), 2);
    let note = job.preview.order_note.as_deref().unwrap();
    assert!(
        note.contains("no hay capas de corte consecutivas"),
        "{note}"
    );
}

// ---------------------------------------------------------------------------
// Incremento de ángulo
// ---------------------------------------------------------------------------

fn moves_of(job: &ComputedJob) -> Vec<Move> {
    job.toolpath
        .blocks
        .iter()
        .flat_map(|b| b.moves.clone())
        .collect()
}

fn fill_at(angle: f64, passes: u32, increment: f64) -> Document {
    doc(
        vec![rect("a", RED, 0.0, 0.0, 10.0, 6.0)],
        vec![LayerSettings {
            angle,
            passes,
            angle_increment: increment,
            ..fill(RED)
        }],
    )
}

#[test]
fn fill_angle_increases_on_every_pass() {
    let all = compute(&fill_at(0.0, 3, 30.0), &dev(), &JobOptions::default()).unwrap();
    assert_eq!(all.layers.len(), 1);
    assert_eq!(all.layers[0].passes, 3);
    let singles: Vec<Move> = [0.0, 30.0, 60.0]
        .iter()
        .flat_map(|&a| {
            moves_of(&compute(&fill_at(a, 1, 0.0), &dev(), &JobOptions::default()).unwrap())
        })
        .collect();
    assert_eq!(moves_of(&all), singles);
    assert!(all.toolpath.blocks[0].label.contains("pasadas=3"));
    assert_eq!(
        all.layers[0].count,
        all.toolpath.blocks[0]
            .moves
            .iter()
            .filter(|m| matches!(m, Move::Travel { .. }))
            .count()
    );

    // Una sola pasada, o incremento 0: nada cambia.
    let one = compute(&fill_at(0.0, 1, 30.0), &dev(), &JobOptions::default()).unwrap();
    let base = compute(&fill_at(0.0, 1, 0.0), &dev(), &JobOptions::default()).unwrap();
    assert_eq!(moves_of(&one), moves_of(&base));
    let zero = compute(&fill_at(0.0, 3, 0.0), &dev(), &JobOptions::default()).unwrap();
    let rep = moves_of(&base).repeat(3);
    assert_eq!(moves_of(&zero), rep);
}

#[test]
fn angle_increment_with_z_step_gives_one_block_per_pass() {
    let mut d = fill_at(10.0, 3, 25.0);
    d.layers[0].z_step = 0.5;
    let device = DeviceProfile {
        z_axis: true,
        ..dev()
    };
    let job = compute(&d, &device, &JobOptions::default()).unwrap();
    assert_eq!(job.toolpath.blocks.len(), 3);
    for (k, a) in [10.0, 35.0, 60.0].iter().enumerate() {
        let single = compute(&fill_at(*a, 1, 0.0), &dev(), &JobOptions::default()).unwrap();
        assert_eq!(
            job.toolpath.blocks[k].moves,
            single.toolpath.blocks[0].moves
        );
        assert_eq!(job.layers[k].pass, Some((k as u32 + 1, 3)));
        assert_eq!(job.layers[k].z_mm, Some(0.5 * k as f64));
    }
}

#[test]
fn angle_increment_is_ignored_with_a_warning_in_offset_fill_and_validated() {
    let mut d = fill_at(0.0, 2, 30.0);
    d.layers[0].fill_style = FillStyle::Offset;
    let job = compute(&d, &dev(), &JobOptions::default()).unwrap();
    assert!(
        job.preview
            .warnings
            .iter()
            .any(|w| w.contains("angle_increment") && w.contains("contornos"))
    );
    let mut bad = fill_at(0.0, 2, f64::NAN);
    bad.layers[0].angle_increment = f64::INFINITY;
    let e = compute(&bad, &dev(), &JobOptions::default()).unwrap_err();
    assert!(e.contains("angle_increment"), "{e}");
    // En una capa line no hace nada.
    let mut l = fill_at(0.0, 2, 30.0);
    l.layers[0].mode = LayerMode::Line;
    let a = compute(&l, &dev(), &JobOptions::default()).unwrap();
    l.layers[0].angle_increment = 0.0;
    let b = compute(&l, &dev(), &JobOptions::default()).unwrap();
    assert_eq!(moves_of(&a), moves_of(&b));
}

#[test]
fn image_angle_increases_on_every_pass() {
    let dir = workdir("imagen");
    let layer = |angle: f64, passes: u32, inc: f64| LayerSettings {
        angle,
        passes,
        angle_increment: inc,
        interval: 0.5,
        overscan: Overscan::Mm(1.0),
        ..LayerSettings::for_images()
    };
    let d = |l: LayerSettings| doc(vec![gradient_image(&dir)], vec![l]);
    let all = compute(&d(layer(0.0, 2, 45.0)), &dev(), &JobOptions::default()).unwrap();
    let singles: Vec<Move> = [0.0, 45.0]
        .iter()
        .flat_map(|&a| {
            moves_of(&compute(&d(layer(a, 1, 0.0)), &dev(), &JobOptions::default()).unwrap())
        })
        .collect();
    assert_eq!(all.toolpath.blocks.len(), 1);
    assert_eq!(moves_of(&all), singles);
    std::fs::remove_dir_all(dir).unwrap();
}

// ---------------------------------------------------------------------------
// Cortar solo la selección
// ---------------------------------------------------------------------------

fn two_rects() -> Document {
    doc(
        vec![
            rect("a", RED, 10.0, 10.0, 10.0, 10.0),
            rect("b", RED, 50.0, 10.0, 10.0, 10.0),
        ],
        vec![line(RED)],
    )
}

fn select(ids: &[&str], use_origin: bool, start_from: StartFrom) -> JobOptions {
    JobOptions {
        start_from,
        job_origin: 6, // abajo a la izquierda
        selection: Some(ids.iter().map(|s| s.to_string()).collect()),
        use_selection_origin: use_origin,
        ..JobOptions::default()
    }
}

fn min_xy(job: &ComputedJob) -> (f64, f64) {
    let b = job.bounds().unwrap();
    (b.min.x, b.min.y)
}

#[test]
fn cutting_the_selection_keeps_its_place_in_the_design_unless_selection_origin() {
    let d = two_rects();
    let here = Some(Point::new(100.0, 100.0));
    // Absoluto: solo b, donde está.
    let abs = build(
        &d,
        &dev(),
        &select(&["b"], false, StartFrom::Absolute),
        None,
    )
    .unwrap();
    assert_eq!(abs.layers.len(), 1);
    assert_eq!(min_xy(&abs), (50.0, 10.0));
    // Desde la posición actual con el origen del diseño completo: la esquina del diseño (10, 10)
    // va a (100, 100), así que b queda 40 mm a su derecha.
    let design = build(
        &d,
        &dev(),
        &select(&["b"], false, StartFrom::CurrentPosition),
        here,
    )
    .unwrap();
    assert_eq!(min_xy(&design), (140.0, 100.0));
    // Con el origen de la selección es la esquina de b la que va a (100, 100).
    let sel = build(
        &d,
        &dev(),
        &select(&["b"], true, StartFrom::CurrentPosition),
        here,
    )
    .unwrap();
    assert_eq!(min_xy(&sel), (100.0, 100.0));
    // Y con user_origin pasa lo mismo.
    let user = JobOptions {
        user_origin: Some([20.0, 30.0]),
        ..select(&["b"], true, StartFrom::UserOrigin)
    };
    let user = build(&d, &dev(), &user, None).unwrap();
    assert_eq!(min_xy(&user), (20.0, 30.0));
    // Seleccionar todo da lo mismo con o sin la opción.
    let all = build(
        &d,
        &dev(),
        &select(&["a", "b"], true, StartFrom::CurrentPosition),
        here,
    )
    .unwrap();
    let all2 = build(
        &d,
        &dev(),
        &select(&["a", "b"], false, StartFrom::CurrentPosition),
        here,
    )
    .unwrap();
    assert_eq!(min_xy(&all), (100.0, 100.0));
    assert_eq!(min_xy(&all2), (100.0, 100.0));
}

#[test]
fn selection_origin_without_effect_is_reported() {
    let d = two_rects();
    let w =
        |job: &ComputedJob, needle: &str| job.preview.warnings.iter().any(|w| w.contains(needle));
    let abs = build(&d, &dev(), &select(&["b"], true, StartFrom::Absolute), None).unwrap();
    assert!(w(&abs, "use_selection_origin se ignora") && w(&abs, "absoluto"));
    let none = JobOptions {
        use_selection_origin: true,
        ..JobOptions::default()
    };
    let job = build(&d, &dev(), &none, None).unwrap();
    assert!(w(&job, "no hay selección"));
}

#[test]
fn an_empty_or_unusable_selection_explains_itself() {
    let d = two_rects();
    let e = build(&d, &dev(), &select(&[], false, StartFrom::Absolute), None).unwrap_err();
    assert!(
        e.contains("la selección no incluye ningún objeto con salida"),
        "{e}"
    );
    let e = build(
        &d,
        &dev(),
        &select(&["zzz"], false, StartFrom::Absolute),
        None,
    )
    .unwrap_err();
    assert!(e.contains("zzz") && e.contains("no existe"), "{e}");
}

#[test]
fn selection_works_with_sub_layers_and_keeps_object_data() {
    let mut d = doc(
        vec![
            rect("a", RED, 0.0, 0.0, 10.0, 10.0),
            rect("b", RED, 30.0, 0.0, 10.0, 10.0),
        ],
        vec![fill_with_outline(RED)],
    );
    d.objects[1].priority = Some(1);
    let job = compute(&d, &dev(), &select(&["b"], false, StartFrom::Absolute)).unwrap();
    assert_eq!(job.layers.len(), 2);
    assert_eq!(starts(&job.toolpath.blocks[1]).len(), 1);
    assert!(
        job.toolpath.blocks[1]
            .moves
            .iter()
            .all(|m| m.to().x >= 30.0 - 1e-9)
    );
}

#[test]
fn flood_fill_gets_the_scan_offset_correction_like_the_sweep() {
    let layer = |style| LayerSettings {
        fill_style: style,
        overscan: Overscan::Mm(2.0),
        ..fill(RED)
    };
    let d = |style| {
        doc(
            vec![rect("a", RED, 0.0, 0.0, 10.0, 6.0)],
            vec![layer(style)],
        )
    };
    let shifted = DeviceProfile {
        scan_initial_offset: 0.3,
        ..dev()
    };
    for style in [FillStyle::Scan, FillStyle::Flood] {
        let base = compute(&d(style), &dev(), &JobOptions::default()).unwrap();
        let moved = compute(&d(style), &shifted, &JobOptions::default()).unwrap();
        assert_ne!(moves_of(&base), moves_of(&moved), "{style:?}");
    }
}
