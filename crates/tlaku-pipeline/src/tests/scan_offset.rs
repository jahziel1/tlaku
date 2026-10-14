//! Scan offset: corrección entre líneas de ida y vuelta.

use super::*;
use tlaku_core::toolpath::ToolpathBlock;
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{DeviceProfile, JobOptions, LayerSettings, Overscan};

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
