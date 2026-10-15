//! Curvas y texto.

use super::*;
use std::path::{Path, PathBuf};
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Point, Polyline};
use tlaku_doc::affine::{self, IDENTITY};
use tlaku_doc::{DesignObject, JobOptions, ObjectKind};

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
            features: None,
            variations: None,
            uppercase: None,
            valign: None,
            word_spacing_mm: None,
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
