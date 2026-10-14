//! Capas: valores por defecto, resolución, clasificación y orden.

use super::*;
use crate::layers::{KERF_REASON, POWER_REASON, order_note};
use tlaku_core::CutSettings;
use tlaku_doc::{LayerMode, LayerOrder, LayerSettings, Overscan};
use tlaku_fill::FillSettings;
use tlaku_raster::RasterSettings;

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
// Velocidad por encima de la máquina y capas de Ruida
// ---------------------------------------------------------------------------

fn ruida_device() -> DeviceProfile {
    DeviceProfile {
        kind: tlaku_doc::DeviceKind::Ruida,
        bed: [600.0, 400.0],
        max_rate: [36_000.0, 36_000.0],
        ..DeviceProfile::default()
    }
}

/// `n` cuadrados de 5 mm, uno por color (rojo creciente y verde creciente).
fn many_color_doc(n: usize, speed: f64) -> Document {
    let color = |i: usize| Rgb((i / 256) as u8, (i % 256) as u8, 7);
    let objects = (0..n)
        .map(|i| {
            rect_object(
                &format!("r{i}"),
                color(i),
                (i % 20) as f64 * 10.0,
                (i / 20) as f64 * 10.0,
                5.0,
                5.0,
            )
        })
        .collect();
    let layers = (0..n)
        .map(|i| LayerSettings {
            speed,
            ..line_layer(color(i))
        })
        .collect();
    doc(objects, layers)
}

fn warnings(job: &crate::ComputedJob) -> &[String] {
    &job.preview.warnings
}

#[test]
fn layer_faster_than_the_machine_warns_and_leaves_gcode_alone() {
    let d = DeviceProfile::default();
    assert_eq!(d.max_rate, [12000.0, 12000.0]);
    let slow = compute(
        &many_color_doc(1, 12_000.0),
        &d,
        &tlaku_doc::JobOptions::default(),
    )
    .unwrap();
    assert!(
        warnings(&slow)
            .iter()
            .all(|w| !w.contains("llega como máximo"))
    );

    let fast_doc = many_color_doc(1, 48_000.0);
    let job = compute(&fast_doc, &d, &tlaku_doc::JobOptions::default()).unwrap();
    let w: Vec<_> = warnings(&job)
        .iter()
        .filter(|w| w.contains("llega como máximo"))
        .collect();
    assert_eq!(w.len(), 1, "{:?}", warnings(&job));
    for needle in [
        "#00",
        "48000 mm/min",
        "12000 mm/min",
        "irá al máximo",
        "más oscuro o más quemado",
    ] {
        assert!(w[0].contains(needle), "falta {needle:?} en {}", w[0]);
    }
    // Solo avisa: el trabajo y su velocidad son los pedidos.
    assert_eq!(job.toolpath.blocks[0].speed_mm_min, 48_000.0);
}

#[test]
fn speed_warning_names_each_fast_layer_and_skips_galvo() {
    let d = DeviceProfile::default();
    let mut document = many_color_doc(3, 6000.0);
    document.layers[1].speed = 30_000.0;
    document.layers[2].speed = 20_000.0;
    let job = compute(&document, &d, &tlaku_doc::JobOptions::default()).unwrap();
    let w: Vec<_> = warnings(&job)
        .iter()
        .filter(|w| w.contains("llega como máximo"))
        .collect();
    assert_eq!(w.len(), 2);
    assert!(w[0].contains("30000") && w[1].contains("20000"));
    let galvo = DeviceProfile {
        kind: tlaku_doc::DeviceKind::Galvo,
        ..DeviceProfile::default()
    };
    assert!(crate::layers::speed_limit(&galvo).is_none());
}

#[test]
fn ruida_layer_over_36000_warns_and_the_rd_clamps_it() {
    let d = DeviceProfile {
        max_rate: [60_000.0, 60_000.0],
        ..ruida_device()
    };
    let job = compute(
        &many_color_doc(1, 48_000.0),
        &d,
        &tlaku_doc::JobOptions::default(),
    )
    .unwrap();
    let w: Vec<_> = warnings(&job)
        .iter()
        .filter(|w| w.contains("Ruida admite como máximo 36000"))
        .collect();
    assert_eq!(w.len(), 1, "{:?}", warnings(&job));
    assert!(w[0].contains("48000"));
    // Antes fallaba: ahora el .rd sale con la velocidad recortada a 600 mm/s.
    let rd = crate::export_rd_for_device(&job, &d).unwrap();
    let cmds = tlaku_ruida::cmd::decode_stream(&rd).unwrap();
    assert!(
        cmds.iter()
            .any(|c| matches!(c, tlaku_ruida::cmd::Command::Speed(600_000)))
    );
    // El trabajo calculado no se toca.
    assert_eq!(job.toolpath.blocks[0].speed_mm_min, 48_000.0);
}

#[test]
fn ruida_does_not_clamp_to_the_profile_max_rate() {
    let d = DeviceProfile {
        max_rate: [12_000.0, 12_000.0],
        ..ruida_device()
    };
    // 30 000 mm/min (500 mm/s): pasa el max_rate del perfil, no el de Ruida.
    let job = compute(
        &many_color_doc(1, 30_000.0),
        &d,
        &tlaku_doc::JobOptions::default(),
    )
    .unwrap();
    let w = warnings(&job);
    assert!(w.iter().any(|w| w.contains("llega como máximo a 12000")
        && w.contains("sube la velocidad máxima del perfil")));
    assert!(w.iter().all(|w| !w.contains("Ruida admite como máximo")));
    let rd = crate::export_rd_for_device(&job, &d).unwrap();
    let cmds = tlaku_ruida::cmd::decode_stream(&rd).unwrap();
    assert!(
        cmds.iter()
            .any(|c| matches!(c, tlaku_ruida::cmd::Command::Speed(500_000)))
    );
}

#[test]
fn ruida_129_colors_with_equal_settings_warn_and_export() {
    let d = ruida_device();
    let job = compute(
        &many_color_doc(129, 6000.0),
        &d,
        &tlaku_doc::JobOptions::default(),
    )
    .unwrap();
    assert_eq!(job.toolpath.blocks.len(), 129);
    let w: Vec<_> = warnings(&job)
        .iter()
        .filter(|w| w.contains("Ruida admite 128"))
        .collect();
    assert_eq!(w.len(), 1, "{:?}", warnings(&job));
    assert!(
        w[0].contains("129") && w[0].contains("128 capas vecinas") && w[0].contains("quedarán 1")
    );
    let rd = crate::export_rd_for_device(&job, &d).unwrap();
    assert!(!rd.is_empty());
    // Con una máquina que no es Ruida no hay aviso.
    let grbl = compute(
        &many_color_doc(129, 6000.0),
        &DeviceProfile::default(),
        &tlaku_doc::JobOptions::default(),
    )
    .unwrap();
    assert!(warnings(&grbl).iter().all(|w| !w.contains("Ruida")));
}

#[test]
fn ruida_129_different_settings_warn_that_it_cannot_export() {
    let d = ruida_device();
    let mut document = many_color_doc(129, 6000.0);
    for (i, l) in document.layers.iter_mut().enumerate() {
        l.speed = 1000.0 + i as f64 * 10.0;
    }
    let job = compute(&document, &d, &tlaku_doc::JobOptions::default()).unwrap();
    let w: Vec<_> = warnings(&job)
        .iter()
        .filter(|w| w.contains("Ruida admite 128"))
        .collect();
    assert_eq!(w.len(), 1, "{:?}", warnings(&job));
    assert!(w[0].contains("no se podrá exportar") && w[0].contains("Unifica colores o ajustes"));
    let err = crate::export_rd_for_device(&job, &d).unwrap_err();
    assert!(err.contains("unifica colores o ajustes"), "{err}");
}
