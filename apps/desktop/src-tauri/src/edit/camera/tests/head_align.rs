//! Alinear con el cabezal: la solución con fotos sintéticas, el ajuste con un punto de
//! referencia y que la alineación se guarde y se lea como las demás.

use super::*;
use tlaku_camera::head_align::sim::SimScene;
use tlaku_camera::{HeadAlignIssue, HeadMethod, Parallax, PlanOptions, UndistortMap};

const BED: [f64; 2] = [400.0, 415.0];

fn frames(scene: &SimScene, corrected: bool) -> Vec<HeadAlignFrame> {
    let plan = tlaku_camera::plan_grid(&PlanOptions {
        bed: BED,
        grid: 3,
        margin_mm: 60.0,
        region: None,
    })
    .unwrap();
    let mut spots = plan.points;
    spots.push(plan.reference);
    let map = UndistortMap::new(&scene.lens, (scene.lens.width, scene.lens.height)).unwrap();
    spots
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let img = scene.render_rgb(Some(*p), i as u64);
            let img = if corrected {
                map.undistort(&img).unwrap()
            } else {
                img
            };
            let mut buf = std::io::Cursor::new(Vec::new());
            img.write_to(&mut buf, ImageFormat::Png).unwrap();
            HeadAlignFrame {
                png_base64: STANDARD.encode(buf.into_inner()),
                machine_mm: *p,
            }
        })
        .collect()
}

fn config(scene: &SimScene) -> HeadAlignConfig {
    HeadAlignConfig {
        bed: BED,
        lens: Some(scene.lens.clone()),
        ..HeadAlignConfig::default()
    }
}

#[test]
fn la_solucion_localiza_el_cabezal_y_entrega_una_alineacion_guardable() {
    let scene = SimScene::fisheye(BED, 330.0);
    let r = run_head_align_solve(&frames(&scene, true), &config(&scene)).unwrap();
    assert_eq!(r.method, HeadMethod::Led);
    assert_eq!((r.detected, r.failed), (10, 0));
    assert_eq!(r.image_size, [800, 600]);
    assert!(r.frames.iter().all(|f| f.used && f.error_mm.unwrap() < 1.0));
    // Los píxeles del informe tienen el origen en el borde del píxel (suman 0.5).
    assert!(r.frames[0].px.unwrap()[0].fract() != 0.0 || r.frames[0].px.unwrap()[1].fract() != 0.0);
    let a = r.alignment.clone().expect("alineación");
    assert_eq!(a.method, AlignmentMethod::HeadSweep);
    assert_eq!(a.reference_height_mm, 0.0);
    // Se guarda con el mismo `camera.json` que las demás y sobrevive a normalizar y a releer.
    let setup = CameraSetup {
        alignment: Some(a.clone()),
        calibration: Some(scene.lens.clone()),
        ..CameraSetup::default()
    };
    let tmp = TempDir::new();
    let dir = &tmp.0;
    let saved = store_setup(dir, setup).unwrap();
    let back = load_setup(dir).unwrap();
    // Al releer el JSON los decimales pueden diferir en el último dígito.
    let (sa, ba) = (saved.alignment.unwrap(), back.alignment.unwrap());
    assert_eq!(
        (sa.method, sa.image_size, sa.samples.len()),
        (ba.method, ba.image_size, ba.samples.len())
    );
    assert!(
        sa.to_bed
            .iter()
            .flatten()
            .zip(ba.to_bed.iter().flatten())
            .all(|(x, y)| (x - y).abs() < 1e-9)
    );
    let b = ba;
    assert_eq!(b.method, AlignmentMethod::HeadSweep);
    assert_eq!(b.samples.len(), 1);
    assert!((b.rms_mm - a.rms_mm).abs() < 1e-6);
    assert!(
        b.height.is_some(),
        "con lente, el modelo de altura de respaldo se calcula"
    );
    let json = std::fs::read_to_string(setup_file(dir)).unwrap();
    assert!(json.contains("\"head_sweep\""));
    // El mapeo guardado lleva un píxel de la imagen a su punto de la cama.
    let h = Homography { m: b.to_bed };
    let f = &r.frames[4];
    let q = h.apply(Point::new(f.px.unwrap()[0], f.px.unwrap()[1]));
    assert!((q.x - f.machine_mm[0]).hypot(q.y - f.machine_mm[1]) < 1.0);
}

#[test]
fn sin_cabezal_a_la_vista_no_hay_ajuste_pero_si_informe() {
    let scene = SimScene::fisheye(BED, 330.0);
    let mut fr = frames(&scene, true);
    // Cama vacía en todas las fotos: nada que localizar.
    let map = UndistortMap::new(&scene.lens, (800, 600)).unwrap();
    for (i, f) in fr.iter_mut().enumerate() {
        let img = map
            .undistort(&scene.render_rgb_with(None, Some(f.machine_mm[1]), i as u64))
            .unwrap();
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png).unwrap();
        f.png_base64 = STANDARD.encode(buf.into_inner());
    }
    let r = run_head_align_solve(&fr, &config(&scene)).unwrap();
    assert_eq!(r.detected, 0);
    assert!(r.fit.is_none() && r.alignment.is_none() && r.fit_error.is_some());
    assert!(r.seen_mm.is_empty());
    assert_eq!(r.frames.len(), 10);
    assert!(
        r.frames
            .iter()
            .all(|f| f.failure.is_some() && f.px.is_none())
    );
    assert!(
        r.issues
            .iter()
            .any(|i| matches!(i, HeadAlignIssue::NotDetected { count: 10 }))
    );
}

#[test]
fn entradas_invalidas_se_rechazan() {
    let scene = SimScene::fisheye(BED, 330.0);
    assert!(run_head_align_solve(&[], &config(&scene)).is_err());
    let mut fr = frames(&scene, true);
    fr[0].machine_mm[0] = f64::NAN;
    assert!(run_head_align_solve(&fr, &config(&scene)).is_err());
    let mut fr = frames(&scene, true);
    fr[1].png_base64 = "no-es-una-imagen".into();
    assert!(run_head_align_solve(&fr, &config(&scene)).is_err());
    let mut cfg = config(&scene);
    cfg.parallax = Some(Parallax {
        camera_height_mm: 10.0,
        feature_height_mm: 30.0,
    });
    assert!(run_head_align_solve(&frames(&scene, true), &cfg).is_err());
}

#[test]
fn un_punto_de_referencia_corrige_el_desfase_de_la_alineacion_guardada() {
    let scene = SimScene::fisheye(BED, 330.0);
    let r = run_head_align_solve(&frames(&scene, true), &config(&scene)).unwrap();
    let a = r.alignment.unwrap();
    // El usuario marca en la foto un disparo de prueba que cayó 3 mm a la derecha y 2 más arriba
    // de donde dice la alineación.
    let ref_px = [400.0, 300.0];
    let at = Homography { m: a.to_bed }.apply(Point::new(ref_px[0], ref_px[1]));
    let truth = [at.x + 3.0, at.y + 2.0];
    let (shifted, delta) =
        run_head_align_shift(a.clone(), Some(scene.lens.clone()), ref_px, truth).unwrap();
    assert!((delta[0] - 3.0).abs() < 1e-9 && (delta[1] - 2.0).abs() < 1e-9);
    let q = Homography { m: shifted.to_bed }.apply(Point::new(ref_px[0], ref_px[1]));
    assert!((q.x - truth[0]).hypot(q.y - truth[1]) < 0.01);
    assert_eq!(shifted.method, AlignmentMethod::HeadSweep);
    // El error de ajuste no cambia: es un desplazamiento rígido.
    assert!((shifted.rms_mm - a.rms_mm).abs() < 0.05);
    assert!(run_head_align_shift(a, None, [f64::NAN, 0.0], truth).is_err());
}

#[test]
fn la_alineacion_manual_tambien_se_puede_desplazar() {
    let mut a = sample_setup().alignment.expect("alineación de ejemplo");
    a.samples.clear();
    let a = normalize_setup(CameraSetup {
        alignment: Some(a),
        ..CameraSetup::default()
    })
    .unwrap()
    .alignment
    .unwrap();
    let (s, d) = run_head_align_shift(
        a.clone(),
        None,
        a.image_px[0],
        [a.bed_mm[0][0] + 5.0, a.bed_mm[0][1]],
    )
    .unwrap();
    assert!((d[0] - 5.0).abs() < 1e-6 && d[1].abs() < 1e-6);
    assert_eq!(s.method, AlignmentMethod::Manual);
}
