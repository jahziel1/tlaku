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
