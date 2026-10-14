//! G-code: perfil de máquina, origen, vista previa, encuadre y dialectos.

use super::*;
use tlaku_core::{Point, Rgb};
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{DeviceProfile, JobOptions, LayerSettings, Origin, StartFrom};

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
fn laser2_layers_select_the_second_laser_only_on_machines_that_have_it() {
    let both = doc(
        vec![
            rect_object("a", RED, 10.0, 20.0, 20.0, 40.0),
            rect_object("b", Rgb(0, 0, 255), 50.0, 20.0, 20.0, 40.0),
        ],
        vec![
            line_layer(RED),
            LayerSettings {
                laser2: true,
                ..line_layer(Rgb(0, 0, 255))
            },
        ],
    );
    let mut dev = device();
    dev.laser2 = true;
    dev.dialect.laser2_select = Some("M114 L2".into());
    dev.dialect.laser1_select = Some("M114 L1".into());
    let job = compute(&both, &dev, &JobOptions::default()).unwrap();
    assert!(job.layers.iter().any(|l| l.laser2));
    let g = gcode(&job, &dev);
    let sel: Vec<&str> = g.lines().filter(|l| l.starts_with("M114")).collect();
    // Un solo cambio al láser 2 (y, si la capa azul va antes, uno de vuelta al 1).
    assert_eq!(sel.iter().filter(|l| **l == "M114 L2").count(), 1, "{g}");
    assert!(sel.len() <= 2, "{g}");
    assert!(job.preview.warnings.iter().all(|w| !w.contains("láser 2")));

    // Sin segundo láser en el perfil: nada se selecciona y se avisa.
    let mut single = dev.clone();
    single.laser2 = false;
    let job = compute(&both, &single, &JobOptions::default()).unwrap();
    assert!(!gcode(&job, &single).contains("M114"));
    assert!(
        job.preview
            .warnings
            .iter()
            .any(|w| w.contains("no tiene segundo láser"))
    );

    // Con láser 2 pero sin comando en el dialecto: se avisa y tampoco se emite nada.
    let mut nocmd = dev.clone();
    nocmd.dialect.laser2_select = None;
    let job = compute(&both, &nocmd, &JobOptions::default()).unwrap();
    assert!(!gcode(&job, &nocmd).contains("M114 L2"));
    assert!(
        job.preview
            .warnings
            .iter()
            .any(|w| w.contains("comando para seleccionar el láser 2"))
    );
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

#[test]
fn link_load_warns_for_dense_engraving_but_not_for_a_fused_polyline() {
    use tlaku_core::toolpath::{Move, PowerMode, Toolpath, ToolpathBlock};
    let dev = device();
    let mut job = compute(&tall_rect(), &dev, &JobOptions::default()).unwrap();
    let block = |moves| ToolpathBlock {
        label: "prueba".into(),
        speed_mm_min: 12_000.0,
        air_assist: false,
        power_mode: PowerMode::Dynamic,
        moves,
        z_mm: None,
        galvo: None,
    };
    // Grabado: puntos de 0,02 mm con potencia que cambia en cada uno (no se puede fusionar).
    let mut moves = vec![Move::Travel {
        to: Point::new(10.0, 10.0),
    }];
    for i in 1..=3000 {
        moves.push(Move::Cut {
            to: Point::new(10.0 + 0.02 * f64::from(i), 10.0),
            power_pct: if i % 2 == 0 { 20.0 } else { 30.0 },
        });
    }
    job.toolpath = Toolpath {
        blocks: vec![block(moves)],
    };
    let g = gcode(&job, &dev);
    let load = link_load(&job, &dev, &g, 115_200);
    assert!(load.peak_lines_per_s > 1000.0, "{load:?}");
    // El planificador de 16 bloques con segmentos de 0,02 mm solo ve 0,3 mm: unas 1 260 líneas
    // por segundo, más de las ~800 que caben a 115 200 baudios.
    let w = load.warning().expect("debe avisar");
    assert!(w.contains("115200 baudios") && w.contains("pausas"), "{w}");
    // Los mismos puntos a una sola potencia son una recta: se fusionan en tramos de 256 puntos y no hay aviso.
    let mut moves = vec![Move::Travel {
        to: Point::new(10.0, 10.0),
    }];
    for i in 1..=3000 {
        moves.push(Move::Cut {
            to: Point::new(10.0 + 0.02 * f64::from(i), 10.0),
            power_pct: 30.0,
        });
    }
    job.toolpath = Toolpath {
        blocks: vec![block(moves)],
    };
    let g = gcode(&job, &dev);
    assert!(g.lines().count() < 40, "{g}");
    let load = link_load(&job, &dev, &g, 115_200);
    assert_eq!(load.warning(), None, "{load:?}");
}
