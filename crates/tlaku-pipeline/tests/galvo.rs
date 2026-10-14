//! Trabajos para máquinas galvo (ola 5): parámetros por bloque, campo, bamboleo, grabado 3D por
//! cortes, corrección de cilindro y secuencia de marcado dividido y repetido. Los dorados
//! (`tests/fixtures/galvo_*.txt`) son un volcado del `Toolpath`; si un cambio es intencional,
//! regenera con `UPDATE_GOLDEN=1 cargo test` y revisa el diff.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use tlaku_core::bezpath::BezPath;
use tlaku_core::toolpath::{Move, PowerMode};
use tlaku_core::{Point, Polyline, Rgb};
use tlaku_doc::affine::{self, IDENTITY};
use tlaku_doc::{
    CylinderCorrection, DesignObject, DeviceKind, DeviceProfile, Document, GalvoDevice, ImageMode,
    ImageObject, JobOptions, LaserSource, LayerMode, LayerSettings, ObjectKind, Overscan,
    RepeatMarking, Slice3d, SplitMarking, VectorObject, Wobble,
};
use tlaku_pipeline::{
    ComputedJob, MarkPlan, build, compute, doc_mark_sequence, job_mark_sequence, mark_sequence,
};

const RED: Rgb = Rgb(255, 0, 0);

fn object(id: &str, closed: bool, pts: &[(f64, f64)]) -> DesignObject {
    DesignObject {
        id: id.into(),
        layer: RED,
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

fn rect(id: &str, x: f64, y: f64, w: f64, h: f64) -> DesignObject {
    object(id, true, &[(x, y), (x + w, y), (x + w, y + h), (x, y + h)])
}

fn layer(edit: impl FnOnce(&mut LayerSettings)) -> LayerSettings {
    let mut l = LayerSettings {
        speed: 6000.0,
        power: 40.0,
        ..LayerSettings::for_color(RED)
    };
    edit(&mut l);
    l
}

fn document(objects: Vec<DesignObject>, layer: LayerSettings) -> Document {
    Document {
        objects,
        layers: vec![layer],
        ..Document::default()
    }
}

/// Galvo de campo 110 mm con la cama del mismo tamaño.
fn galvo_device() -> DeviceProfile {
    DeviceProfile {
        name: "Galvo 110".into(),
        bed: [110.0, 110.0],
        galvo: GalvoDevice {
            field_mm: 110.0,
            ..GalvoDevice::default()
        },
        ..DeviceProfile::for_kind(DeviceKind::Galvo)
    }
}

fn run(doc: &Document, device: &DeviceProfile) -> ComputedJob {
    compute(doc, device, &JobOptions::default()).unwrap()
}

fn warnings(job: &ComputedJob) -> String {
    job.preview.warnings.join("\n")
}

/// Volcado de texto del `Toolpath` para los dorados.
fn dump(job: &ComputedJob) -> String {
    let mut out = String::new();
    for (b, l) in job.toolpath.blocks.iter().zip(&job.layers) {
        writeln!(
            out,
            "bloque '{}' velocidad={} modo={:?} aire={} z={:?} informe='{}' segundos={:.3}",
            b.label, b.speed_mm_min, b.power_mode, b.air_assist, b.z_mm, l.name, l.seconds
        )
        .unwrap();
        match &b.galvo {
            Some(g) => writeln!(out, "  galvo {g:?}").unwrap(),
            None => writeln!(out, "  galvo -").unwrap(),
        }
        for m in &b.moves {
            match m {
                Move::Travel { to } => writeln!(out, "  T {:.4} {:.4}", to.x, to.y),
                Move::Cut { to, power_pct } => {
                    writeln!(out, "  C {:.4} {:.4} p={power_pct:.2}", to.x, to.y)
                }
                Move::Dwell { to, ms, power_pct } => {
                    writeln!(out, "  D {:.4} {:.4} ms={ms} p={power_pct}", to.x, to.y)
                }
            }
            .unwrap();
        }
    }
    out
}

fn golden(name: &str, actual: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("{name}.txt"));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("falta {}; ejecuta con UPDATE_GOLDEN=1", path.display()));
    assert_eq!(actual, expected, "el volcado de '{name}' cambió");
}

// ---------------------------------------------------------------------------
// Parámetros por bloque, overscan, campo
// ---------------------------------------------------------------------------

#[test]
fn block_takes_layer_frequency_and_profile_timings() {
    let mut device = galvo_device();
    device.galvo.timing.laser_on_us = 111.0;
    device.galvo.jump.speed = 5555.0;
    let doc = document(
        vec![rect("a", 30.0, 30.0, 20.0, 20.0)],
        layer(|l| {
            l.galvo.frequency_khz = 45.0;
            l.galvo.timing.laser_on_us = -50.0;
            l.galvo.jump.speed = 1.0;
            l.galvo.repeat = 3;
            l.galvo.pause_before_ms = 120.0;
            l.galvo.pause_after_ms = 80.0;
        }),
    );
    let job = run(&doc, &device);
    let g = job.toolpath.blocks[0].galvo.as_ref().unwrap();
    // Frecuencia, repeat y pausas de la capa; retardos y saltos del perfil (sin override).
    assert_eq!(g.frequency_khz, 45.0);
    assert_eq!(g.laser_on_us, 111.0);
    assert_eq!(g.jump_speed, 5555.0);
    assert_eq!(
        (g.repeat, g.pause_before_ms, g.pause_after_ms),
        (3, 120.0, 80.0)
    );
    assert_eq!(job.toolpath.blocks[0].power_mode, PowerMode::Constant);
}

#[test]
fn override_timings_uses_the_layer_values() {
    let mut device = galvo_device();
    device.galvo.timing.laser_on_us = 111.0;
    let doc = document(
        vec![rect("a", 30.0, 30.0, 20.0, 20.0)],
        layer(|l| {
            l.galvo.override_timings = true;
            l.galvo.timing.laser_on_us = -50.0;
            l.galvo.timing.end_us = 450.0;
            l.galvo.jump.speed = 3000.0;
            l.galvo.jump.delay_min_us = 7.0;
        }),
    );
    let job = run(&doc, &device);
    let g = job.toolpath.blocks[0].galvo.as_ref().unwrap();
    assert_eq!((g.laser_on_us, g.end_us), (-50.0, 450.0));
    assert_eq!((g.jump_speed, g.jump_delay_min_us), (3000.0, 7.0));
}

#[test]
fn gantry_machines_get_no_galvo_params() {
    let doc = document(vec![rect("a", 30.0, 30.0, 20.0, 20.0)], layer(|_| {}));
    let job = run(&doc, &DeviceProfile::default());
    assert!(job.toolpath.blocks.iter().all(|b| b.galvo.is_none()));
    assert!(
        job.toolpath
            .blocks
            .iter()
            .all(|b| b.power_mode == PowerMode::Dynamic)
    );
}

#[test]
fn repeat_is_a_parameter_not_repeated_moves() {
    let device = galvo_device();
    let moves = |repeat: u32| {
        let doc = document(
            vec![rect("a", 30.0, 30.0, 20.0, 20.0)],
            layer(|l| l.galvo.repeat = repeat),
        );
        run(&doc, &device).toolpath.blocks[0].moves.len()
    };
    assert_eq!(moves(1), moves(50));
    let doc = document(
        vec![rect("a", 30.0, 30.0, 20.0, 20.0)],
        layer(|l| l.galvo.repeat = 0),
    );
    let e = compute(&doc, &device, &JobOptions::default()).unwrap_err();
    assert!(e.contains("repeat"), "{e}");
}

#[test]
fn estimate_includes_repeat_and_pauses_without_axis_acceleration() {
    let device = galvo_device();
    let line = || vec![object("l", false, &[(5.0, 55.0), (105.0, 55.0)])];
    let secs =
        |edit: fn(&mut LayerSettings)| run(&document(line(), layer(edit)), &device).preview.total_s;
    // 100 mm a 6000 mm/min = 100 mm/s: un segundo, sin rampas de aceleración.
    let once = secs(|_| {});
    assert!((once - 1.0).abs() < 0.05, "{once} s");
    let thrice = secs(|l| l.galvo.repeat = 3);
    assert!((thrice - 3.0 * once).abs() < 0.1, "{thrice} s");
    let paused = secs(|l| l.galvo.pause_before_ms = 500.0);
    assert!((paused - once - 0.5).abs() < 0.01, "{paused} s");
}

#[test]
fn fills_have_no_overscan_on_galvo() {
    let doc = document(
        vec![rect("a", 30.0, 30.0, 40.0, 40.0)],
        layer(|l| {
            l.mode = LayerMode::Fill;
            l.interval = 2.0;
            l.overscan = Overscan::Mm(5.0);
        }),
    );
    let job = run(&doc, &galvo_device());
    assert_eq!(job.layers[0].overscan_mm, Some(0.0));
    let b = job.layers[0].bounds.unwrap();
    assert!(b.min.x >= 30.0 - 1e-9 && b.max.x <= 70.0 + 1e-9, "{b:?}");
    assert!(
        job.toolpath.blocks[0]
            .moves
            .iter()
            .all(|m| !matches!(m, Move::Cut { power_pct, .. } if *power_pct == 0.0)),
        "no debe haber cortes con potencia 0 (overscan)"
    );
    assert!(warnings(&job).contains("overscan"), "{}", warnings(&job));
    // En una máquina de pórtico el overscan automático sí existe.
    let gantry = run(
        &document(
            vec![rect("a", 100.0, 100.0, 40.0, 40.0)],
            layer(|l| {
                l.mode = LayerMode::Fill;
                l.interval = 2.0;
            }),
        ),
        &DeviceProfile::default(),
    );
    assert!(gantry.layers[0].overscan_mm.unwrap() > 0.0);
}

#[test]
fn the_profile_can_ask_for_overscan_on_fills_and_the_layer_keeps_its_own() {
    let mut device = galvo_device();
    device.galvo.overscan_mm = 3.0;
    let fill = |own: Overscan| {
        document(
            vec![rect("a", 30.0, 30.0, 40.0, 40.0)],
            layer(|l| {
                l.mode = LayerMode::Fill;
                l.interval = 2.0;
                l.overscan = own;
            }),
        )
    };
    // Sin overscan propio (automático) se usa el del perfil: los cortes con potencia 0 salen del
    // dibujo en 3 mm por cada lado y no hay aviso de overscan ignorado.
    let job = run(&fill(Overscan::Auto), &device);
    assert_eq!(job.layers[0].overscan_mm, Some(3.0));
    let b = job.layers[0].bounds.unwrap();
    assert!(
        (b.min.x - 27.0).abs() < 1e-6 && (b.max.x - 73.0).abs() < 1e-6,
        "{b:?}"
    );
    assert!(
        job.toolpath.blocks[0]
            .moves
            .iter()
            .any(|m| matches!(m, Move::Cut { power_pct, .. } if *power_pct == 0.0)),
        "el overscan sale como cortes con potencia 0"
    );
    assert!(!warnings(&job).contains("overscan"), "{}", warnings(&job));
    // La capa que fija el suyo en mm lo conserva.
    let own = run(&fill(Overscan::Mm(5.0)), &device);
    assert_eq!(own.layers[0].overscan_mm, Some(5.0));
    // El valor por omisión del perfil (0) deja todo como antes.
    let none = run(&fill(Overscan::Auto), &galvo_device());
    assert_eq!(none.layers[0].overscan_mm, Some(0.0));
}

#[test]
fn limits_are_the_field_and_not_the_bed() {
    // Cama de 400 mm y campo de 110 centrado en (200, 200): el campo va de 145 a 255.
    let mut device = galvo_device();
    device.bed = [400.0, 400.0];
    let inside = document(vec![rect("a", 150.0, 150.0, 100.0, 100.0)], layer(|_| {}));
    assert!(compute(&inside, &device, &JobOptions::default()).is_ok());
    let outside = document(vec![rect("a", 100.0, 100.0, 30.0, 30.0)], layer(|_| {}));
    let e = compute(&outside, &device, &JobOptions::default()).unwrap_err();
    assert!(e.contains("campo del galvo"), "{e}");
    // El desborde lleva el mismo código que el de la cama, para que la interfaz muestre la misma pista.
    assert_eq!(tlaku_i18n::split_code(&e).0, Some("bed_overflow"), "{e}");
    assert!(tlaku_i18n::split_code(&e).1.starts_with("la capa "), "{e}");
    let en = tlaku_i18n::with_locale(tlaku_i18n::Locale::En, || {
        compute(&outside, &device, &JobOptions::default()).unwrap_err()
    });
    assert_eq!(tlaku_i18n::split_code(&en).0, Some("bed_overflow"), "{en}");
    assert!(en.contains("mm galvo field"), "{en}");
    // `build` no falla pero avisa.
    let job = build(&outside, &device, &JobOptions::default(), None).unwrap();
    assert!(
        warnings(&job).contains("se sale del campo"),
        "{}",
        warnings(&job)
    );
    // Con el marcado dividido el diseño puede pasar del campo.
    device.galvo.split.enabled = true;
    assert!(compute(&outside, &device, &JobOptions::default()).is_ok());
    // Un diseño que cabe en la cama pero no en el campo es un error en galvo y no en pórtico.
    assert!(compute(&outside, &DeviceProfile::default(), &JobOptions::default()).is_ok());
}

#[test]
fn source_limits_give_warnings() {
    let mut device = galvo_device();
    device.galvo.max_power_pct = 30.0;
    let doc = document(
        vec![rect("a", 30.0, 30.0, 20.0, 20.0)],
        layer(|l| {
            l.galvo.frequency_khz = 500.0;
            l.galvo.pulse_width_ns = Some(100);
            l.galvo.ppi = 100.0;
        }),
    );
    let w = warnings(&run(&doc, &device));
    assert!(w.contains("frecuencia"), "{w}");
    assert!(w.contains("ancho de pulso"), "{w}");
    assert!(w.contains("límite de la máquina"), "{w}");
    assert!(w.contains("ppi"), "{w}");
    // Con una fuente MOPA el pulso permitido se conserva y uno fuera de lista se avisa.
    device.galvo.source = LaserSource::FiberMopa;
    device.galvo.pulse_widths_ns = vec![4, 100, 200];
    let job = run(&doc, &device);
    assert_eq!(
        job.toolpath.blocks[0]
            .galvo
            .as_ref()
            .unwrap()
            .pulse_width_ns,
        Some(100)
    );
    assert!(!warnings(&job).contains("ancho de pulso"));
    let doc2 = document(
        vec![rect("a", 30.0, 30.0, 20.0, 20.0)],
        layer(|l| l.galvo.pulse_width_ns = Some(77)),
    );
    assert!(warnings(&run(&doc2, &device)).contains("77 ns"));
}

#[test]
fn golden_layers_with_params() {
    let mut device = galvo_device();
    device.galvo.timing.end_us = 250.0;
    let doc = Document {
        objects: vec![rect("caja", 35.0, 35.0, 40.0, 40.0), {
            let mut o = rect("relleno", 40.0, 40.0, 10.0, 10.0);
            o.layer = Rgb(0, 0, 255);
            o
        }],
        layers: vec![
            layer(|l| {
                l.galvo.repeat = 2;
                l.galvo.pause_before_ms = 100.0;
            }),
            LayerSettings {
                mode: LayerMode::Fill,
                interval: 2.5,
                speed: 9000.0,
                power: 60.0,
                ..{
                    let mut l = LayerSettings::for_color(Rgb(0, 0, 255));
                    l.galvo.override_timings = true;
                    l.galvo.frequency_khz = 60.0;
                    l.galvo.timing.laser_on_us = -20.0;
                    l.galvo.pause_after_ms = 40.0;
                    l
                }
            },
        ],
        ..Document::default()
    };
    golden("galvo_layers", &dump(&run(&doc, &device)));
}

// ---------------------------------------------------------------------------
// Bamboleo
// ---------------------------------------------------------------------------

fn wobble_layer(size: f64, step: f64) -> LayerSettings {
    layer(|l| {
        l.galvo.wobble = Wobble {
            enabled: true,
            size,
            step,
        }
    })
}

#[test]
fn wobble_widens_the_stroke_by_size() {
    let doc = document(
        vec![object("l", false, &[(20.0, 55.0), (80.0, 55.0)])],
        wobble_layer(0.4, 0.1),
    );
    let job = run(&doc, &galvo_device());
    let moves = &job.toolpath.blocks[0].moves;
    assert!(moves.len() > 600, "{} movimientos", moves.len());
    let ys: Vec<f64> = moves.iter().map(|m| m.to().y).collect();
    let (lo, hi) = ys
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), &y| (a.min(y), b.max(y)));
    assert!((hi - lo - 0.4).abs() < 1e-9, "ancho {}", hi - lo);
    assert_eq!(moves.last().unwrap().to(), Point::new(80.0, 55.0));
    let b = job.layers[0].bounds.unwrap();
    assert!((b.max.y - b.min.y - 0.4).abs() < 1e-9);
}

#[test]
fn wobble_only_in_line_layers_and_only_on_galvo() {
    let objs = || vec![rect("a", 30.0, 30.0, 20.0, 20.0)];
    let fill = document(
        objs(),
        layer(|l| {
            l.mode = LayerMode::Fill;
            l.galvo.wobble.enabled = true;
        }),
    );
    assert!(warnings(&run(&fill, &galvo_device())).contains("solo se aplica a capas line"));
    let line = document(objs(), wobble_layer(0.2, 0.05));
    let w = warnings(&run(&line, &DeviceProfile::default()));
    assert!(w.contains("solo funciona con máquinas galvo"), "{w}");
    // En pórtico la geometría no cambia.
    let plain = document(objs(), layer(|_| {}));
    assert_eq!(
        run(&line, &DeviceProfile::default()).toolpath,
        run(&plain, &DeviceProfile::default()).toolpath
    );
    let bad = document(objs(), wobble_layer(0.0, 0.05));
    assert!(compute(&bad, &galvo_device(), &JobOptions::default()).is_err());
}

#[test]
fn golden_wobble() {
    let doc = document(
        vec![object(
            "l",
            false,
            &[(50.0, 50.0), (51.0, 50.0), (51.0, 50.5)],
        )],
        wobble_layer(0.2, 0.25),
    );
    golden("galvo_wobble", &dump(&run(&doc, &galvo_device())));
}

// ---------------------------------------------------------------------------
// Cilindro
// ---------------------------------------------------------------------------

#[test]
fn cylinder_correction_bends_the_job() {
    let mut device = galvo_device();
    // Cilindro a lo largo de X (se curva en Y), centrado en la cama (55, 55).
    device.galvo.cylinder = CylinderCorrection {
        enabled: true,
        diameter_mm: 60.0,
        mirror_distance_mm: 160.0,
        ..CylinderCorrection::default()
    };
    let doc = document(
        vec![object("l", false, &[(55.0, 55.0), (55.0, 85.0)])],
        layer(|_| {}),
    );
    let corrected = run(&doc, &device);
    let moves = &corrected.toolpath.blocks[0].moves;
    // Subdividida cada milímetro: 1 + 30 movimientos.
    assert_eq!(moves.len(), 31);
    let end = moves.last().unwrap().to();
    assert!((end.x - 55.0).abs() < 1e-9);
    // 30 mm de arco en R = 30 mm es 1 radián: el campo plano recibe menos de 30 mm.
    assert!(end.y > 55.0 && end.y - 55.0 < 30.0, "{end:?}");
    // Sin la corrección sale tal cual.
    let plain = run(&doc, &galvo_device());
    assert_eq!(
        plain.toolpath.blocks[0].moves.last().unwrap().to(),
        Point::new(55.0, 85.0)
    );
    assert_eq!(plain.toolpath.blocks[0].moves.len(), 2);
}

#[test]
fn cylinder_beyond_the_horizon_warns() {
    let mut device = galvo_device();
    device.galvo.cylinder = CylinderCorrection {
        enabled: true,
        diameter_mm: 20.0,
        mirror_distance_mm: 100.0,
        ..CylinderCorrection::default()
    };
    let doc = document(
        vec![object("l", false, &[(55.0, 55.0), (55.0, 85.0)])],
        layer(|_| {}),
    );
    let job = run(&doc, &device);
    assert!(
        warnings(&job).contains("fuera de lo visible"),
        "{}",
        warnings(&job)
    );
    device.galvo.cylinder.diameter_mm = -1.0;
    assert!(compute(&doc, &device, &JobOptions::default()).is_err());
}

#[test]
fn golden_cylinder_and_taper() {
    let mut device = galvo_device();
    device.galvo.cylinder = CylinderCorrection {
        enabled: true,
        diameter_mm: 50.0,
        mirror_distance_mm: 150.0,
        ..CylinderCorrection::default()
    };
    device.galvo.taper.enabled = true;
    device.galvo.taper.top_diameter_mm = 50.0;
    device.galvo.taper.bottom_diameter_mm = 30.0;
    device.galvo.taper.length_mm = 80.0;
    let doc = document(
        vec![rect("r", 35.0, 45.0, 6.0, 10.0)],
        layer(|l| l.galvo.frequency_khz = 30.0),
    );
    golden("galvo_cylinder", &dump(&run(&doc, &device)));
}

// ---------------------------------------------------------------------------
// Grabado 3D por cortes
// ---------------------------------------------------------------------------

fn workdir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tlaku-galvo-{}-{test}", std::process::id()));
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

fn image_doc(path: &Path, layer: LayerSettings) -> Document {
    let obj = DesignObject {
        id: "mapa".into(),
        layer: LayerSettings::IMAGE_COLOR,
        transform: affine::translate(40.0, 40.0),
        kind: ObjectKind::Image(ImageObject {
            source_path: path.to_str().unwrap().into(),
            width_px: 16,
            height_px: 16,
            size_mm: [16.0, 16.0],
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
    };
    Document {
        objects: vec![obj],
        layers: vec![layer],
        ..Document::default()
    }
}

fn slice_layer(slices: u32, angle_step: f64) -> LayerSettings {
    LayerSettings {
        speed: 6000.0,
        power: 50.0,
        interval: 1.0,
        image: tlaku_doc::ImageSettings {
            mode: ImageMode::Threshold,
            ..Default::default()
        },
        galvo: tlaku_doc::GalvoLayer {
            slice3d: Slice3d {
                enabled: true,
                slices,
                angle_step_deg: angle_step,
            },
            pause_before_ms: 10.0,
            pause_after_ms: 20.0,
            ..Default::default()
        },
        ..LayerSettings::for_images()
    }
}

/// Mapa de profundidad: de negro (hondo) a casi blanco de izquierda a derecha.
fn gradient(dir: &Path) -> PathBuf {
    let path = dir.join("profundidad.bmp");
    std::fs::write(&path, gray_bmp(16, 16, |x, _| (x * 16).min(255) as u8)).unwrap();
    path
}

fn cut_length(job: &ComputedJob, block: usize) -> f64 {
    let mut head = None;
    let mut total = 0.0;
    for m in &job.toolpath.blocks[block].moves {
        if let (Move::Cut { to, power_pct }, Some(a)) = (m, head)
            && *power_pct > 0.0
        {
            total += Point::dist(a, *to);
        }
        head = Some(m.to());
    }
    total
}

#[test]
fn slice3d_makes_one_block_per_depth_on_galvo_and_gantry() {
    let dir = workdir("slice");
    let doc = image_doc(&gradient(&dir), slice_layer(4, 0.0));
    for device in [galvo_device(), DeviceProfile::default()] {
        let job = if device.kind == DeviceKind::Galvo {
            run(&doc, &device)
        } else {
            // Pórtico: la imagen cae lejos de los bordes, pero con overscan.
            run(&image_doc_moved(&doc), &device)
        };
        assert_eq!(job.toolpath.blocks.len(), 4, "{:?}", warnings(&job));
        let names: Vec<&str> = job.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "imagen · corte 1/4",
                "imagen · corte 2/4",
                "imagen · corte 3/4",
                "imagen · corte 4/4"
            ]
        );
        // Cada corte graba menos que el anterior (umbral más oscuro).
        let lens: Vec<f64> = (0..4).map(|k| cut_length(&job, k)).collect();
        assert!(lens.windows(2).all(|w| w[1] < w[0]), "{lens:?}");
        assert!(lens[3] > 0.0);
    }
}

/// Lo mismo desplazado dentro de la cama grande del pórtico.
fn image_doc_moved(doc: &Document) -> Document {
    let mut d = doc.clone();
    d.objects[0].transform = affine::translate(150.0, 150.0);
    d
}

#[test]
fn slice3d_off_is_one_block_and_angle_steps_per_slice() {
    let dir = workdir("slice-off");
    let path = gradient(&dir);
    let mut off = slice_layer(4, 0.0);
    off.galvo.slice3d.enabled = false;
    let job = run(&image_doc(&path, off), &galvo_device());
    assert_eq!(job.toolpath.blocks.len(), 1);
    assert_eq!(job.layers[0].name, "imagen");
    // Con 90 grados por corte, el segundo barre en vertical: distinta primera dirección.
    let job = run(&image_doc(&path, slice_layer(2, 90.0)), &galvo_device());
    let first_cut = |k: usize| {
        let m = &job.toolpath.blocks[k].moves;
        let t = m
            .iter()
            .position(|m| matches!(m, Move::Travel { .. }))
            .unwrap();
        let (a, b) = (m[t].to(), m[t + 1].to());
        (
            (b.x - a.x).abs() > (b.y - a.y).abs(),
            job.toolpath.blocks[k].moves.len(),
        )
    };
    assert_ne!(first_cut(0).0, first_cut(1).0, "la dirección debía cambiar");
}

#[test]
fn slice3d_pauses_only_around_the_whole_layer_and_bad_counts_fail() {
    let dir = workdir("slice-pauses");
    let path = gradient(&dir);
    let job = run(&image_doc(&path, slice_layer(3, 0.0)), &galvo_device());
    let g = |k: usize| job.toolpath.blocks[k].galvo.as_ref().unwrap();
    assert_eq!((g(0).pause_before_ms, g(0).pause_after_ms), (10.0, 0.0));
    assert_eq!((g(1).pause_before_ms, g(1).pause_after_ms), (0.0, 0.0));
    assert_eq!((g(2).pause_before_ms, g(2).pause_after_ms), (0.0, 20.0));
    let e = compute(
        &image_doc(&path, slice_layer(1, 0.0)),
        &galvo_device(),
        &JobOptions::default(),
    )
    .unwrap_err();
    assert!(e.contains("cortes"), "{e}");
}

#[test]
fn golden_slice3d() {
    let dir = workdir("slice-golden");
    let path = dir.join("mini.bmp");
    // 4x4: cada columna un nivel de gris distinto.
    std::fs::write(
        &path,
        gray_bmp(16, 16, |x, _| [0u8, 70, 140, 210][(x / 4) as usize]),
    )
    .unwrap();
    let mut l = slice_layer(4, 45.0);
    l.interval = 4.0;
    let job = run(&image_doc(&path, l), &galvo_device());
    golden("galvo_slice3d", &dump(&job));
}

// ---------------------------------------------------------------------------
// Marcado dividido, repetido y rotativo galvo
// ---------------------------------------------------------------------------

fn describe(plan: &MarkPlan) -> String {
    let mut out = String::new();
    for s in &plan.steps {
        match &s.tile {
            Some(t) => writeln!(
                out,
                "r{} p{} rect={:?} offset={:?} stage=({:.3},{:.3},{:.3}deg) pausa={}",
                s.repeat,
                s.piece,
                t.rect,
                t.offset,
                s.stage.dx,
                s.stage.dy,
                s.stage.rotary_deg,
                s.pause_ms
            ),
            None => writeln!(
                out,
                "r{} p{} entero stage=({:.3},{:.3},{:.3}deg) pausa={}",
                s.repeat, s.piece, s.stage.dx, s.stage.dy, s.stage.rotary_deg, s.pause_ms
            ),
        }
        .unwrap();
    }
    for w in &plan.warnings {
        writeln!(out, "aviso: {w}").unwrap();
    }
    out
}

#[test]
fn split_sequence_from_a_computed_job() {
    let mut device = galvo_device();
    device.bed = [400.0, 400.0];
    device.galvo.split = SplitMarking {
        enabled: true,
        overlap_mm: 2.0,
        pause_after_move_ms: 300.0,
        ..SplitMarking::default()
    };
    // 250 x 130 mm: cabe en 3 x 2 piezas de 110 mm con 2 mm de solape.
    let doc = document(
        vec![rect("grande", 20.0, 20.0, 250.0, 130.0)],
        layer(|_| {}),
    );
    let job = build(&doc, &device, &JobOptions::default(), None).unwrap();
    let plan = job_mark_sequence(&job, &device).unwrap();
    assert_eq!(plan.steps.len(), 6);
    assert!(plan.steps[0].stage.is_zero());
    for s in &plan.steps {
        let r = s.tile.as_ref().unwrap().rect;
        assert!(r[1][0] - r[0][0] <= 110.0 + 1e-9 && r[1][1] - r[0][1] <= 110.0 + 1e-9);
        // Todo el offset lleva el centro de la pieza al centro de la cama (200, 200).
        let t = s.tile.as_ref().unwrap();
        let c = [(r[0][0] + r[1][0]) / 2.0, (r[0][1] + r[1][1]) / 2.0];
        assert!(
            (c[0] + t.offset[0] - 200.0).abs() < 1e-9 && (c[1] + t.offset[1] - 200.0).abs() < 1e-9
        );
    }
    golden("galvo_split", &describe(&plan));
    // Cada pieza, calculada con su tramo, cae dentro del campo y entre todas cubren el diseño.
    let field = tlaku_pipeline::field_rect(&device);
    let mut covered = 0.0;
    for s in &plan.steps {
        let options = JobOptions {
            tile: s.tile.clone(),
            ..JobOptions::default()
        };
        let piece = build(&doc, &device, &options, None).unwrap();
        let b = piece.bounds().unwrap();
        assert!(
            b.min.x >= field.min.x - 1e-6
                && b.max.x <= field.max.x + 1e-6
                && b.min.y >= field.min.y - 1e-6
                && b.max.y <= field.max.y + 1e-6,
            "pieza {} fuera del campo: {b:?}",
            s.piece
        );
        covered += cut_length(&piece, 0);
    }
    // El contorno de 250 x 130 mm mide 760 mm; los solapes repiten algo de trazo, nunca menos.
    assert!(covered >= 760.0 - 1e-6, "{covered}");
}

/// Piezas por formas enteras (M40) de punta a punta: la secuencia del documento, un trabajo por
/// pieza con `tile` y `selection` y el resultado. Tres rectángulos de 60 mm en un campo de 110:
/// con franjas fijas el segundo (x de 90 a 150) cruza el borde de x = 130 y se marca en dos
/// partes; por formas enteras cada uno va en su pieza y ninguno se parte.
#[test]
fn whole_shapes_split_marks_every_shape_in_one_piece() {
    let mut device = galvo_device();
    device.bed = [400.0, 400.0];
    device.galvo.split = SplitMarking {
        enabled: true,
        ..SplitMarking::default()
    };
    let doc = document(
        vec![
            rect("a", 20.0, 100.0, 60.0, 20.0),
            rect("b", 90.0, 100.0, 60.0, 20.0),
            rect("c", 170.0, 100.0, 60.0, 20.0),
        ],
        layer(|_| {}),
    );
    let options = JobOptions::default();
    let whole_job = build(&doc, &device, &options, None).unwrap();
    let field = tlaku_pipeline::field_rect(&device);
    let widths = |plan: &MarkPlan, device: &DeviceProfile| -> Vec<f64> {
        plan.steps
            .iter()
            .map(|s| {
                let o = JobOptions {
                    tile: s.tile.clone(),
                    selection: s.objects.clone(),
                    ..JobOptions::default()
                };
                let piece = build(&doc, device, &o, None).unwrap();
                let b = piece.bounds().unwrap();
                assert!(
                    b.min.x >= field.min.x - 1e-6 && b.max.x <= field.max.x + 1e-6,
                    "pieza fuera del campo: {b:?}"
                );
                b.max.x - b.min.x
            })
            .collect()
    };
    // Franjas fijas: el rectángulo del medio sale en dos mitades.
    let plain = doc_mark_sequence(&doc, &whole_job, &device, &options).unwrap();
    assert!(plain.steps.iter().all(|s| s.objects.is_none()));
    let w = widths(&plain, &device);
    // Dos franjas (20..130 y 130..230): la primera lleva a y la mitad de b (110 mm de trazo), la
    // segunda la otra mitad de b y c (100 mm).
    assert_eq!(w.len(), 2, "{w:?}");
    assert!(
        (w[0] - 110.0).abs() < 1e-6 && (w[1] - 100.0).abs() < 1e-6,
        "{w:?}"
    );
    // Formas enteras: tres piezas, cada rectángulo entero (60 mm).
    device.galvo.split.whole_shapes = true;
    let plan = doc_mark_sequence(&doc, &whole_job, &device, &options).unwrap();
    assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);
    assert_eq!(plan.steps.len(), 3);
    let ids: Vec<_> = plan
        .steps
        .iter()
        .map(|s| s.objects.clone().unwrap())
        .collect();
    assert_eq!(ids, vec![vec!["a"], vec!["b"], vec!["c"]]);
    for w in widths(&plan, &device) {
        assert!((w - 60.0).abs() < 1e-6, "{w}");
    }
    // La mesa avanza lo que separa los centros: a (50) a b (120) y b a c (200).
    assert!((plan.steps[1].stage.dx + 70.0).abs() < 1e-9);
    assert!((plan.steps[2].stage.dx + 80.0).abs() < 1e-9);
    // Con una selección del usuario solo entran sus objetos.
    let only_ac = JobOptions {
        selection: Some(vec!["a".into(), "c".into()]),
        ..JobOptions::default()
    };
    let job = build(&doc, &device, &only_ac, None).unwrap();
    let plan = doc_mark_sequence(&doc, &job, &device, &only_ac).unwrap();
    let ids: Vec<_> = plan
        .steps
        .iter()
        .map(|s| s.objects.clone().unwrap())
        .collect();
    assert_eq!(ids, vec![vec!["a"], vec!["c"]]);
}

#[test]
fn rotary_galvo_and_repeat_sequence_golden() {
    let mut device = galvo_device();
    device.rotary.enabled = true;
    device.rotary.kind = tlaku_doc::RotaryKind::Chuck;
    device.rotary.object_diameter = 40.0;
    device.galvo.split = SplitMarking {
        enabled: true,
        piece_mm: 50.0,
        pause_after_move_ms: 200.0,
        ..SplitMarking::default()
    };
    device.galvo.repeat = RepeatMarking {
        count: 2,
        increment_mm: [0.0, 10.0],
        pause_ms: 1000.0,
    };
    // Circunferencia de pi * 40 = 125.66 mm; diseño de 100 mm de alto en franjas de 50.
    let bounds = tlaku_core::Bounds {
        min: Point::new(40.0, 5.0),
        max: Point::new(70.0, 105.0),
    };
    let plan = mark_sequence(bounds, &device).unwrap();
    assert_eq!(plan.steps.len(), 4);
    golden("galvo_rotary", &describe(&plan));
}

#[test]
fn galvo_messages_come_out_in_english() {
    use tlaku_i18n::{Locale, with_locale};
    let mut device = galvo_device();
    device.bed = [400.0, 400.0];
    let outside = document(vec![rect("a", 100.0, 100.0, 30.0, 30.0)], layer(|_| {}));
    let e = with_locale(Locale::En, || {
        compute(&outside, &device, &JobOptions::default()).unwrap_err()
    });
    assert!(e.contains("goes outside the"), "{e}");
    assert!(e.contains("galvo field"), "{e}");
    assert!(e.contains("shrink the design to fit the field"), "{e}");
    assert!(!e.contains("se sale"), "{e}");
    let w = with_locale(Locale::En, || {
        warnings(&build(&outside, &device, &JobOptions::default(), None).unwrap())
    });
    assert!(w.contains("goes outside the galvo field"), "{w}");
    // Un error de capa de la capa: frecuencia inválida.
    let bad = document(
        vec![rect("a", 150.0, 150.0, 10.0, 10.0)],
        layer(|l| l.galvo.frequency_khz = f64::NAN),
    );
    let e = with_locale(Locale::En, || {
        compute(&bad, &device, &JobOptions::default()).unwrap_err()
    });
    assert!(e.contains("invalid frequency"), "{e}");
}
