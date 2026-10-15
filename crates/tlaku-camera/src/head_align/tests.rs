//! Pruebas del plan, el detector (LED y diferencia) y el ajuste con una cámara simulada.

use super::sim::SimScene;
use super::*;
use crate::undistort::UndistortMap;
use image::RgbImage;
use tlaku_core::Point;

const BED: [f64; 2] = [400.0, 415.0];

fn plan3() -> HeadAlignPlan {
    plan_grid(&PlanOptions {
        bed: BED,
        grid: 3,
        margin_mm: 60.0,
        region: None,
    })
    .unwrap()
}

/// Rejilla `n x n` dentro de `[x0, y0, x1, y1]` con la referencia de `plan_grid`.
fn grid(n: usize, area: [f64; 4]) -> HeadAlignPlan {
    plan_grid(&PlanOptions {
        bed: BED,
        grid: n,
        margin_mm: 0.0,
        region: Some(area),
    })
    .unwrap()
}

/// Fotos (a color) en cada punto del plan y la de referencia, con sus grupos por fila.
fn shoot(
    scene: &SimScene,
    plan: &HeadAlignPlan,
    undistort: bool,
) -> (Vec<RgbImage>, Vec<usize>, Vec<[f64; 2]>) {
    let mut positions = plan.points.clone();
    positions.push(plan.reference);
    let map = undistort
        .then(|| UndistortMap::new(&scene.lens, (scene.lens.width, scene.lens.height)).unwrap());
    let frames = positions
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let img = scene.render_rgb(Some(*p), i as u64);
            match &map {
                Some(m) => m.undistort(&img).unwrap(),
                None => img,
            }
        })
        .collect();
    // Misma fila = misma Y de la máquina.
    let mut ys: Vec<f64> = Vec::new();
    let groups = positions
        .iter()
        .map(|p| {
            ys.iter()
                .position(|y| (y - p[1]).abs() < 0.5)
                .unwrap_or_else(|| {
                    ys.push(p[1]);
                    ys.len() - 1
                })
        })
        .collect();
    (frames, groups, positions)
}

fn observations(outcome: &DetectOutcome, positions: &[[f64; 2]]) -> Vec<HeadObservation> {
    outcome
        .results
        .iter()
        .zip(positions)
        .filter_map(|(r, p)| {
            r.as_ref().ok().map(|d| HeadObservation {
                machine_mm: *p,
                px: d.px,
            })
        })
        .collect()
}

#[test]
fn plan_grid_covers_the_safe_area_in_a_serpentine() {
    let p = plan3();
    assert_eq!(p.points.len(), 9);
    assert_eq!(p.area, [60.0, 60.0, 340.0, 355.0]);
    // Fila 0 de izquierda a derecha, fila 1 de derecha a izquierda.
    assert_eq!(p.points[0], [60.0, 60.0]);
    assert_eq!(p.points[2], [340.0, 60.0]);
    assert_eq!(p.points[3], [340.0, 207.5]);
    assert_eq!(p.points[5], [60.0, 207.5]);
    // La referencia no coincide con ningún punto de la rejilla, está en la primera fila y dentro del área.
    assert!(
        p.points
            .iter()
            .all(|q| (q[0] - p.reference[0]).hypot(q[1] - p.reference[1]) > 40.0)
    );
    assert_eq!(p.reference, [130.0, 60.0]);
    // Se recorre sin saltos largos: cada paso es de una celda.
    let max_step = p
        .points
        .windows(2)
        .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
        .fold(0.0, f64::max);
    assert!(max_step <= 281.0);
}

#[test]
fn plan_grid_respects_the_region_and_rejects_bad_input() {
    let p = plan_grid(&PlanOptions {
        bed: BED,
        grid: 4,
        margin_mm: 20.0,
        region: Some([100.0, 150.0, 300.0, 415.0]),
    })
    .unwrap();
    assert_eq!(p.points.len(), 16);
    assert_eq!(p.area, [100.0, 150.0, 300.0, 395.0]);
    assert!(
        p.points
            .iter()
            .all(|q| q[0] >= 100.0 && q[0] <= 300.0 && q[1] >= 150.0 && q[1] <= 395.0)
    );
    let bad = |grid, margin_mm, region| PlanOptions {
        bed: BED,
        grid,
        margin_mm,
        region,
    };
    assert!(plan_grid(&bad(2, 20.0, None)).is_err());
    assert!(plan_grid(&bad(9, 20.0, None)).is_err());
    assert!(plan_grid(&bad(3, 200.0, None)).is_err());
    assert!(plan_grid(&bad(3, f64::NAN, None)).is_err());
    assert!(plan_grid(&bad(3, 0.0, Some([0.0, 0.0, 10.0, 10.0]))).is_err());
}

#[test]
fn plan_refine_grows_around_what_was_seen() {
    let area = [20.0, 20.0, 380.0, 395.0];
    // Un solo punto visto: se abre alrededor.
    let p = plan_refine(area, &[[200.0, 207.0]], 3, 0.4, 60.0).unwrap();
    assert_eq!(p.area, [140.0, 147.0, 260.0, 267.0]);
    // Varios: la caja crece una fracción de su tamaño.
    let p = plan_refine(area, &[[150.0, 200.0], [250.0, 300.0]], 3, 0.5, 60.0).unwrap();
    assert_eq!(p.area, [100.0, 150.0, 300.0, 350.0]);
    // Nada visto, o ya cubre todo el área: no hay nada que afinar.
    assert!(plan_refine(area, &[], 3, 0.5, 60.0).is_none());
    assert!(plan_refine(area, &[[20.0, 20.0], [380.0, 395.0]], 3, 0.5, 60.0).is_none());
}

#[test]
fn led_is_found_with_subpixel_precision_and_the_gantry_does_not_matter() {
    let scene = SimScene::fisheye(BED, 330.0);
    let plan = plan3();
    let (frames, groups, positions) = shoot(&scene, &plan, false);
    let out = detect_heads(&frames, &groups, &HeadDetectOptions::default()).unwrap();
    assert_eq!(out.method, HeadMethod::Led);
    for (r, p) in out.results.iter().zip(&positions) {
        let d = r.as_ref().expect("LED visible");
        let truth = scene.project([p[0] - 12.0, p[1] - 14.0, scene.head.body_bottom_mm + 6.0]);
        let e = (d.px[0] - truth.x).hypot(d.px[1] - truth.y);
        assert!(e < 0.6, "LED a {e:.2} px de lo esperado en {p:?}");
    }
}

#[test]
fn a_static_green_light_is_ignored() {
    let scene = SimScene::fisheye(BED, 330.0);
    let plan = plan3();
    let (mut frames, groups, _) = shoot(&scene, &plan, false);
    // Un LED de la cámara: igual de verde y en el mismo lugar en todas las fotos.
    for f in &mut frames {
        for dy in 0..4 {
            for dx in 0..4 {
                f.put_pixel(60 + dx, 40 + dy, image::Rgb([80, 255, 140]));
            }
        }
    }
    let opts = HeadDetectOptions {
        method: HeadMethod::Led,
        ..Default::default()
    };
    let out = detect_heads(&frames, &groups, &opts).unwrap();
    assert!(
        out.results
            .iter()
            .all(|r| r.as_ref().map_or(true, |d| (d.px[0] - 61.5).abs() > 5.0))
    );
}

#[test]
fn diff_finds_the_head_moving_under_the_gantry() {
    let scene = SimScene::fisheye(BED, 330.0);
    let plan = plan3();
    let (frames, groups, positions) = shoot(&scene, &plan, false);
    let opts = HeadDetectOptions {
        method: HeadMethod::Diff,
        ..Default::default()
    };
    let out = detect_heads(&frames, &groups, &opts).unwrap();
    assert_eq!(out.method, HeadMethod::Diff);
    // El pórtico es igual en las fotos de una fila: solo cambia el cabezal. La fila 0 tiene 4
    // fotos (con la referencia) y las demás 3, todas con fondo.
    let mid = scene.head_mid_height();
    let mut worst: f64 = 0.0;
    for (r, p) in out.results.iter().zip(&positions) {
        let d = r
            .as_ref()
            .unwrap_or_else(|e| panic!("sin cabezal en {p:?}: {e:?}"));
        let truth = scene.project([p[0], p[1], mid]);
        worst = worst.max((d.px[0] - truth.x).hypot(d.px[1] - truth.y));
        assert!(d.area_px > 800.0 && d.confidence > 0.3);
    }
    // La silueta incluye la tapa y las paredes: el centro difiere unos píxeles del eje.
    assert!(worst < 14.0, "peor error {worst:.1} px");
}

#[test]
fn diff_survives_exposure_changes_noise_and_the_bottom_tip_feature() {
    let mut scene = SimScene::fisheye(BED, 330.0);
    scene.noise = 4.0;
    let plan = plan3();
    let (frames, groups, positions) = shoot(&scene, &plan, false);
    let opts = HeadDetectOptions {
        method: HeadMethod::Diff,
        feature: HeadFeature::BottomTip,
        ..Default::default()
    };
    let out = detect_heads(&frames, &groups, &opts).unwrap();
    for (r, p) in out.results.iter().zip(&positions) {
        let d = r
            .as_ref()
            .unwrap_or_else(|e| panic!("sin cabezal en {p:?}: {e:?}"));
        assert_eq!(d.px, d.tip_px);
        assert!(d.tip_px[1] >= d.centroid_px[1]);
    }
}

#[test]
fn head_out_of_view_or_cut_by_the_border_is_reported() {
    let scene = SimScene::fisheye(BED, 330.0);
    let plan = plan3();
    let (mut frames, groups, positions) = shoot(&scene, &plan, false);
    // Foto 0 sin cabezal a la vista (fuera del campo), con el pórtico donde le toca en esa fila.
    frames[0] = scene.render_rgb_with(None, Some(positions[0][1]), 0);
    // Foto 4 con el cabezal cortado por el borde: se pinta a la fuerza un borde oscuro pegado.
    let (w, h) = (frames[4].width(), frames[4].height());
    for y in 0..h {
        for x in 0..40 {
            frames[4].put_pixel(w - 1 - x, y, image::Rgb([5, 3, 3]));
        }
    }
    let opts = HeadDetectOptions {
        method: HeadMethod::Diff,
        ..Default::default()
    };
    let out = detect_heads(&frames, &groups, &opts).unwrap();
    assert!(out.results[0].is_err(), "foto 0: {:?}", out.results[0]);
    assert!(out.results[4].is_err(), "foto 4: {:?}", out.results[4]);
    let ok = out.results.iter().filter(|r| r.is_ok()).count();
    assert!(ok >= positions.len() - 4, "{ok} detecciones");
}

#[test]
fn rows_with_too_few_photos_have_no_background() {
    let scene = SimScene::fisheye(BED, 330.0);
    let plan = plan3();
    let (frames, mut groups, _) = shoot(&scene, &plan, false);
    // Cada foto en su propia fila: no hay con qué formar el fondo.
    for (i, g) in groups.iter_mut().enumerate() {
        *g = i;
    }
    let opts = HeadDetectOptions {
        method: HeadMethod::Diff,
        ..Default::default()
    };
    let out = detect_heads(&frames, &groups, &opts).unwrap();
    assert!(
        out.results
            .iter()
            .all(|r| *r == Err(DetectFailure::NoBackground))
    );
    assert!(detect_heads(&frames, &[0, 1], &opts).is_err());
    assert!(detect_heads(&[], &[], &opts).is_err());
}

/// Cuánto se desvía el mapeo en posiciones del cabezal que no se usaron para ajustar: se
/// fotografía, se detecta con el mismo método y se pasa por `to_bed` (en el plano del punto).
fn held_out_error(
    scene: &SimScene,
    fit: &HeadAlignFit,
    spots: &[[f64; 2]],
    undistort: bool,
    opts: &HeadDetectOptions,
) -> f64 {
    let map = undistort
        .then(|| UndistortMap::new(&scene.lens, (scene.lens.width, scene.lens.height)).unwrap());
    // La lente estimada también corrige las fotos de comprobación.
    let est = fit
        .lens
        .as_ref()
        .map(|l| UndistortMap::new(l, (l.width, l.height)).unwrap());
    let h = crate::homography::Homography { m: fit.to_bed };
    // Cada foto de comprobación se detecta junto a dos más (el método con fondo lo necesita).
    let mut worst: f64 = 0.0;
    for (i, s) in spots.iter().enumerate() {
        let others = [spots[(i + 1) % spots.len()], spots[(i + 2) % spots.len()]];
        let imgs: Vec<RgbImage> = [*s, others[0], others[1]]
            .iter()
            .enumerate()
            .map(|(k, p)| {
                let img = scene.render_rgb(Some(*p), 100 + k as u64);
                let img = match &map {
                    Some(m) => m.undistort(&img).unwrap(),
                    None => img,
                };
                match &est {
                    Some(m) => m.undistort(&img).unwrap(),
                    None => img,
                }
            })
            .collect();
        let out = detect_heads(&imgs, &[], opts).unwrap();
        let d = out.results[0].as_ref().expect("detectado");
        let q = h.apply(Point::new(d.px[0] + 0.5, d.px[1] + 0.5));
        worst = worst.max((q.x - s[0]).hypot(q.y - s[1]));
    }
    worst
}

#[test]
fn led_with_a_known_lens_recovers_the_mapping_under_one_millimetre() {
    let scene = SimScene::fisheye(BED, 330.0);
    let plan = grid(4, [60.0, 60.0, 340.0, 355.0]);
    let (frames, groups, positions) = shoot(&scene, &plan, true);
    let out = detect_heads(&frames, &groups, &HeadDetectOptions::default()).unwrap();
    assert_eq!(out.method, HeadMethod::Led);
    let obs = observations(&out, &positions);
    assert_eq!(obs.len(), 17);
    let opts = HeadAlignOptions {
        image_size: [800, 600],
        lens: Some(scene.lens.clone()),
        estimate_lens: false,
        ..Default::default()
    };
    let fit = fit_head_alignment(&obs, &opts).unwrap();
    assert!(fit.lens.is_none());
    assert_eq!(fit.dropped, 0);
    assert!(fit.rms_mm < 0.3, "rms {:.3} mm", fit.rms_mm);
    assert!(
        fit.loo_rms_mm < 0.5 && fit.loo_max_mm < 1.0,
        "loo {:.3}/{:.3}",
        fit.loo_rms_mm,
        fit.loo_max_mm
    );
    assert_eq!(fit.quality, Quality::Good);
    assert!(fit.points.iter().all(|p| p.used && p.error_mm < 1.0));
    assert_eq!(fit.covered_mm, [60.0, 60.0, 340.0, 355.0]);
    let held = held_out_error(
        &scene,
        &fit,
        &[
            [110.0, 130.0],
            [250.0, 300.0],
            [180.0, 80.0],
            [310.0, 200.0],
        ],
        true,
        &HeadDetectOptions::default(),
    );
    assert!(held < 1.0, "error fuera de la rejilla {held:.3} mm");
}

#[test]
fn diff_with_a_known_lens_recovers_the_mapping_at_the_millimetre_level() {
    let scene = SimScene::fisheye(BED, 330.0).scaled(1.6);
    let plan = grid(4, [60.0, 60.0, 340.0, 355.0]);
    let (frames, groups, positions) = shoot(&scene, &plan, true);
    let opts = HeadDetectOptions {
        method: HeadMethod::Diff,
        ..Default::default()
    };
    let out = detect_heads(&frames, &groups, &opts).unwrap();
    let obs = observations(&out, &positions);
    assert!(obs.len() >= 15, "{} detecciones", obs.len());
    let fit = fit_head_alignment(
        &obs,
        &HeadAlignOptions {
            image_size: [scene.lens.width, scene.lens.height],
            lens: Some(scene.lens.clone()),
            estimate_lens: false,
            ..Default::default()
        },
    )
    .unwrap();
    // La silueta de un cabezal alto (26 mm) cambia de forma con la perspectiva: su centro no es
    // un punto físico y deja ~1 mm que ninguna homografía absorbe. El LED baja de 0,5 mm.
    assert!(fit.loo_rms_mm < 1.5, "loo {:.3} mm", fit.loo_rms_mm);
    assert!(fit.rms_mm < 1.5, "rms {:.3} mm", fit.rms_mm);
    assert_ne!(fit.quality, Quality::Poor);
}

#[test]
fn without_a_lens_a_radial_model_is_estimated_and_gets_under_one_millimetre() {
    let scene = SimScene::barrel(BED, 330.0);
    let plan = grid(4, [60.0, 60.0, 340.0, 355.0]);
    let (frames, groups, positions) = shoot(&scene, &plan, false);
    let out = detect_heads(&frames, &groups, &HeadDetectOptions::default()).unwrap();
    let obs = observations(&out, &positions);
    // Sin lente, con el estimador apagado, la distorsión se come el ajuste...
    let plain = fit_head_alignment(
        &obs,
        &HeadAlignOptions {
            image_size: [800, 600],
            estimate_lens: false,
            ..Default::default()
        },
    )
    .unwrap();
    // ...y con él se corrige.
    let fit = fit_head_alignment(
        &obs,
        &HeadAlignOptions {
            image_size: [800, 600],
            ..Default::default()
        },
    )
    .unwrap();
    let lens = fit.lens.as_ref().expect("lente estimada");
    assert!(
        fit.issues
            .iter()
            .any(|i| matches!(i, HeadAlignIssue::LensEstimated { .. }))
    );
    assert!(
        fit.loo_rms_mm < plain.loo_rms_mm,
        "{:.3} contra {:.3}",
        fit.loo_rms_mm,
        plain.loo_rms_mm
    );
    assert!(
        fit.loo_rms_mm < 1.0,
        "loo {:.3} mm (sin lente {:.3})",
        fit.loo_rms_mm,
        plain.loo_rms_mm
    );
    assert_eq!((lens.width, lens.height), (800, 600));
    let held = held_out_error(
        &scene,
        &fit,
        &[
            [110.0, 130.0],
            [250.0, 300.0],
            [180.0, 80.0],
            [310.0, 200.0],
        ],
        false,
        &HeadDetectOptions::default(),
    );
    assert!(held < 1.0, "error fuera de la rejilla {held:.3} mm");
}

#[test]
fn with_a_fisheye_and_no_lens_the_estimate_helps_and_the_error_is_reported_honestly() {
    let scene = SimScene::fisheye(BED, 330.0);
    let plan = grid(4, [60.0, 60.0, 340.0, 355.0]);
    let (frames, groups, positions) = shoot(&scene, &plan, false);
    let out = detect_heads(&frames, &groups, &HeadDetectOptions::default()).unwrap();
    let obs = observations(&out, &positions);
    let opts = |estimate| HeadAlignOptions {
        image_size: [800, 600],
        estimate_lens: estimate,
        ..Default::default()
    };
    let plain = fit_head_alignment(&obs, &opts(false)).unwrap();
    let fit = fit_head_alignment(&obs, &opts(true)).unwrap();
    assert!(fit.loo_rms_mm <= plain.loo_rms_mm);
    assert!(
        fit.loo_rms_mm < 1.0,
        "loo {:.3} mm (sin lente {:.3})",
        fit.loo_rms_mm,
        plain.loo_rms_mm
    );
}

#[test]
fn an_outlier_is_dropped_and_reported() {
    let scene = SimScene::fisheye(BED, 330.0);
    let plan = grid(4, [60.0, 60.0, 340.0, 355.0]);
    let (frames, groups, positions) = shoot(&scene, &plan, true);
    let out = detect_heads(&frames, &groups, &HeadDetectOptions::default()).unwrap();
    let mut obs = observations(&out, &positions);
    obs[5].px[0] += 60.0; // detección errónea
    let fit = fit_head_alignment(
        &obs,
        &HeadAlignOptions {
            image_size: [800, 600],
            lens: Some(scene.lens.clone()),
            estimate_lens: false,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(fit.dropped, 1);
    assert!(!fit.points[5].used && fit.points[5].error_mm > 5.0);
    assert!(fit.issues.contains(&HeadAlignIssue::Dropped { count: 1 }));
    assert!(fit.loo_rms_mm < 0.5);
}

#[test]
fn poor_data_is_flagged() {
    let scene = SimScene::fisheye(BED, 330.0);
    let plan = grid(3, [60.0, 60.0, 340.0, 355.0]);
    let (frames, groups, positions) = shoot(&scene, &plan, true);
    let out = detect_heads(&frames, &groups, &HeadDetectOptions::default()).unwrap();
    let mut obs = observations(&out, &positions);
    // Ruido de unos píxeles en todas las detecciones: el error de validación cruzada lo delata.
    for (i, o) in obs.iter_mut().enumerate() {
        o.px[0] += [4.0, -5.0, 3.0, -4.0, 5.0][i % 5];
        o.px[1] += [-3.0, 4.0, -5.0, 3.0, -4.0][i % 5];
    }
    let fit = fit_head_alignment(
        &obs,
        &HeadAlignOptions {
            image_size: [800, 600],
            lens: Some(scene.lens.clone()),
            estimate_lens: false,
            outlier_mm: 100.0,
            ..Default::default()
        },
    )
    .unwrap();
    assert_ne!(fit.quality, Quality::Good);
    assert!(
        fit.issues
            .iter()
            .any(|i| matches!(i, HeadAlignIssue::HighError { .. }))
    );
    // Con pocos puntos se avisa y con menos de cinco no se ajusta.
    assert!(
        fit_head_alignment(
            &obs[..4],
            &HeadAlignOptions {
                image_size: [800, 600],
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(fit_head_alignment(&obs, &HeadAlignOptions::default()).is_err());
}

#[test]
fn parallax_and_a_reference_point_move_the_mapping_to_the_material_plane() {
    let mut scene = SimScene::fisheye(BED, 330.0);
    scene.dots = vec![[150.0, 250.0], [300.0, 120.0]];
    let plan = grid(4, [60.0, 60.0, 340.0, 355.0]);
    let (frames, groups, positions) = shoot(&scene, &plan, true);
    let out = detect_heads(&frames, &groups, &HeadDetectOptions::default()).unwrap();
    let obs = observations(&out, &positions);
    // El LED está a 26 mm sobre la cama y la cámara a 330 mm.
    let led_h = scene.head.body_bottom_mm + 6.0;
    let base = HeadAlignOptions {
        image_size: [800, 600],
        lens: Some(scene.lens.clone()),
        estimate_lens: false,
        ..Default::default()
    };
    let corrected = UndistortMap::new(&scene.lens, (800, 600)).unwrap();
    let ground = |g: [f64; 2]| {
        let raw = scene.ground_px(g);
        let c = corrected.raw_to_corrected(raw);
        [c.x + 0.5, c.y + 0.5]
    };
    let err_at = |fit: &HeadAlignFit, g: [f64; 2]| {
        let q = crate::homography::Homography { m: fit.to_bed }
            .apply(Point::new(ground(g)[0], ground(g)[1]));
        (q.x - g[0]).hypot(q.y - g[1])
    };
    // El LED no está sobre la boquilla: ese desfase fijo y el paralaje se corrigen juntos con un
    // único punto de referencia conocido (un disparo de prueba visible que el usuario marca).
    let plain = fit_head_alignment(&obs, &base).unwrap();
    let off = reference_offset(&plain, ground([150.0, 250.0]), [150.0, 250.0]);
    let plain = fit_head_alignment(
        &obs,
        &HeadAlignOptions {
            offset_mm: off,
            ..base.clone()
        },
    )
    .unwrap();
    assert!(err_at(&plain, [150.0, 250.0]) < 0.3);
    // Sin paralaje, lejos de la referencia el punto del material cae a varios mm.
    let e_plain = err_at(&plain, [300.0, 120.0]);
    assert!(e_plain > 5.0, "sin paralaje {e_plain:.2} mm");
    // Con las alturas, el error en el material baja de golpe.
    let para = HeadAlignOptions {
        parallax: with_parallax(led_h),
        ..base
    };
    let with = fit_head_alignment(&obs, &para).unwrap();
    assert!(with.parallax_scale > 1.05);
    let off = reference_offset(&with, ground([150.0, 250.0]), [150.0, 250.0]);
    let fixed = fit_head_alignment(
        &obs,
        &HeadAlignOptions {
            offset_mm: off,
            ..para
        },
    )
    .unwrap();
    assert!(err_at(&fixed, [150.0, 250.0]) < 0.3);
    let e_with = err_at(&fixed, [300.0, 120.0]);
    assert!(
        e_with < 1.5 && e_with < e_plain / 4.0,
        "con paralaje {e_with:.2} mm (sin {e_plain:.2})"
    );
}

fn with_parallax(led_h: f64) -> Option<Parallax> {
    Some(Parallax {
        camera_height_mm: 330.0,
        feature_height_mm: led_h,
    })
}

#[test]
fn bad_parallax_values_are_rejected() {
    let scene = SimScene::fisheye(BED, 330.0);
    let plan = grid(3, [60.0, 60.0, 340.0, 355.0]);
    let (frames, groups, positions) = shoot(&scene, &plan, true);
    let out = detect_heads(&frames, &groups, &HeadDetectOptions::default()).unwrap();
    let obs = observations(&out, &positions);
    let opts = HeadAlignOptions {
        image_size: [800, 600],
        lens: Some(scene.lens.clone()),
        estimate_lens: false,
        parallax: Some(Parallax {
            camera_height_mm: 30.0,
            feature_height_mm: 50.0,
        }),
        ..Default::default()
    };
    assert!(fit_head_alignment(&obs, &opts).is_err());
}

fn real_photo(name: &str) -> RgbImage {
    let path = format!(
        "{}/tests/fixtures/falcon2_lid/{name}.jpg",
        env!("CARGO_MANIFEST_DIR")
    );
    image::open(path).expect("foto").to_rgb8()
}

/// Fotos reales de la tapa de una Falcon2 Pro (1920x1080 reducidas a la mitad, ojo de pez, tinte
/// rojizo): la cámara solo ve el cabezal en el centro; en las filas delanteras no aparece y en la
/// trasera solo asoma una esquina. El LED verde de la boquilla se halla sin comparar fotos.
#[test]
fn real_lid_photos_the_led_is_found_only_where_the_head_is_in_view() {
    let names = ["p20_20", "p380_20", "p20_395", "p380_395", "p200_207"];
    let frames: Vec<RgbImage> = names.iter().map(|n| real_photo(n)).collect();
    let groups = [0, 0, 1, 1, 2];
    let out = detect_heads(&frames, &groups, &HeadDetectOptions::default()).unwrap();
    assert_eq!(out.method, HeadMethod::Led);
    let centre = out.results[4].as_ref().expect("LED en el centro");
    // El LED está a la izquierda de la boquilla, hacia (393, 380) en esta escala.
    assert!(
        (centre.px[0] - 393.0).abs() < 6.0 && (centre.px[1] - 380.0).abs() < 6.0,
        "{:?}",
        centre.px
    );
    assert!(centre.area_px >= 3.0 && centre.area_px < 400.0);
    for (i, r) in out.results.iter().enumerate().take(4) {
        assert_eq!(
            *r,
            Err(DetectFailure::NoChange),
            "foto {} ({})",
            i,
            names[i]
        );
    }
}

/// Con solo dos fotos por fila no hay fondo: el método por diferencia no inventa nada.
#[test]
fn real_lid_photos_diff_needs_three_per_row() {
    let names = ["p20_20", "p380_20", "p20_395", "p380_395", "p200_207"];
    let frames: Vec<RgbImage> = names.iter().map(|n| real_photo(n)).collect();
    let opts = HeadDetectOptions {
        method: HeadMethod::Diff,
        ..Default::default()
    };
    let out = detect_heads(&frames, &[0, 0, 1, 1, 2], &opts).unwrap();
    assert!(
        out.results
            .iter()
            .all(|r| *r == Err(DetectFailure::NoBackground))
    );
}
