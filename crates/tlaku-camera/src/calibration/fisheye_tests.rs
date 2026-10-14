//! Pruebas del ojo de pez y de la elección automática de modelo.

use super::select::cross_validate;
use tlaku_core::Point;

use super::*;
use crate::testutil::*;
use image::GrayImage;

const BOARD: [u32; 2] = [9, 6];
const SQ: f64 = 25.0;

fn views_of(cam: &LensCalibration, noise_px: f64, seed: u64) -> Vec<Vec<Point>> {
    let mut rng = Rng::new(seed);
    fisheye_poses(BOARD, SQ)
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let pts = board_corners(cam, p, BOARD, SQ);
            for c in &pts {
                assert!(
                    c.x > 3.0
                        && c.y > 3.0
                        && c.x < f64::from(cam.width) - 3.0
                        && c.y < f64::from(cam.height) - 3.0,
                    "vista {i}: esquina fuera de la imagen ({:.0}, {:.0})",
                    c.x,
                    c.y
                );
            }
            pts.into_iter()
                .map(|c| Point::new(c.x + noise_px * rng.gauss(), c.y + noise_px * rng.gauss()))
                .collect()
        })
        .collect()
}

/// Error de posición en un plano a 350 mm de la cámara, para los píxeles de la imagen
/// cuyo rayo verdadero está a menos de `max_deg` del eje: rms y máximo en mm. Es lo que
/// importa a la hora de colocar un diseño sobre la cama.
fn plane_error_mm(est: &LensCalibration, truth: &LensCalibration, max_deg: f64) -> (f64, f64) {
    const Z: f64 = 350.0;
    let (mut sum, mut worst, mut n) = (0.0, 0.0_f64, 0usize);
    for v in (0..480).step_by(16) {
        for u in (0..640).step_by(16) {
            let p = Point::new(f64::from(u), f64::from(v));
            let (tx, ty) = truth.pixel_to_normalized(p);
            if tx.hypot(ty).atan().to_degrees() > max_deg {
                continue;
            }
            let (ex, ey) = est.pixel_to_normalized(p);
            let e = if ex.is_finite() && ey.is_finite() {
                (Z * (ex - tx)).hypot(Z * (ey - ty))
            } else {
                f64::INFINITY
            };
            sum += e * e;
            worst = worst.max(e);
            n += 1;
        }
    }
    ((sum / n as f64).sqrt(), worst)
}

fn opts(model: LensModelChoice) -> CalibrationOptions {
    CalibrationOptions {
        model,
        ..Default::default()
    }
}

#[test]
fn kannala_brandt_va_y_vuelve() {
    let cam = fisheye_camera();
    for deg in [0.0_f64, 10.0, 35.0, 60.0, 80.0] {
        let th = deg.to_radians();
        let (x, y) = (th.tan() * 0.6, th.tan() * 0.8);
        let (xd, yd) = cam.distort_normalized(x, y);
        let (bx, by) = cam.undistort_normalized(xd, yd);
        assert!(
            (bx - x).hypot(by - y) < 1e-9 * (1.0 + x.hypot(y)),
            "{deg} grados"
        );
    }
    // Cerca del eje equivale a la identidad (θd ≈ θ ≈ r).
    let (xd, yd) = cam.distort_normalized(1e-4, 2e-4);
    assert!((xd - 1e-4).abs() < 1e-9 && (yd - 2e-4).abs() < 1e-9);
    assert_eq!(cam.distort_normalized(0.0, 0.0), (0.0, 0.0));
}

#[test]
fn el_ojo_de_pez_se_recupera_y_baja_el_error_frente_a_brown_conrady() {
    let cam = fisheye_camera();
    let views = views_of(&cam, 0.1, 7);
    let size = (640, 480);
    let fish = calibrate_views(&views, BOARD, SQ, size, &opts(LensModelChoice::Fisheye)).unwrap();
    let brown = calibrate_views(
        &views,
        BOARD,
        SQ,
        size,
        &opts(LensModelChoice::BrownConrady),
    );
    let fc = &fish.calibration;
    let (f_rms, f_max) = plane_error_mm(fc, &cam, 55.0);
    eprintln!(
        "KB: fx {:.1} fy {:.1} cx {:.1} cy {:.1} k1 {:.4} k2 {:.4} k3 {:.4} k4 {:.4} zoom {:.2} rms {:.3} px; plano: rms {f_rms:.3} mm, max {f_max:.3} mm",
        fc.fx, fc.fy, fc.cx, fc.cy, fc.k1, fc.k2, fc.k3, fc.k4, fc.view_zoom, fc.rms_error_px
    );
    assert_eq!(fc.model, LensModel::Fisheye);
    assert!(fc.rms_error_px < 0.25, "rms {}", fc.rms_error_px);
    assert!(((fc.fx - cam.fx) / cam.fx).abs() < 0.01, "fx {}", fc.fx);
    assert!((fc.cx - cam.cx).abs() < 2.0 && (fc.cy - cam.cy).abs() < 2.0);
    assert!(
        f_rms < 1.6 && f_max < 4.0,
        "KB: rms {f_rms:.2} max {f_max:.2} mm"
    );
    match brown {
        Ok(b) => {
            let (b_rms, b_max) = plane_error_mm(&b.calibration, &cam, 55.0);
            eprintln!(
                "BC: rms {:.3} px; plano: rms {b_rms:.3} mm, max {b_max:.3} mm",
                b.calibration.rms_error_px
            );
            assert!(
                b_rms > 4.0 * f_rms.max(0.1),
                "BC {b_rms:.2} mm vs KB {f_rms:.2} mm"
            );
        }
        Err(e) => eprintln!("BC no converge: {e}"),
    }
}

#[test]
fn auto_elige_ojo_de_pez_con_distorsion_fuerte_y_brown_con_distorsion_normal() {
    // Ojo de pez fuerte.
    let cam = fisheye_camera();
    let views = views_of(&cam, 0.1, 11);
    let rep = calibrate_views(&views, BOARD, SQ, (640, 480), &opts(LensModelChoice::Auto)).unwrap();
    eprintln!(
        "auto (ojo de pez): {:?} avisos {:?}",
        rep.model_scores, rep.warnings
    );
    assert_eq!(rep.calibration.model, LensModel::Fisheye);
    assert!(
        rep.model_scores
            .iter()
            .any(|s| s.chosen && s.model == LensModel::Fisheye)
    );
    assert!(rep.warnings.iter().any(|w| w.contains("ojo de pez")));
    assert!(
        rep.calibration.view_zoom < 1.0,
        "zoom {}",
        rep.calibration.view_zoom
    );

    // Lente normal con distorsión de barril: se queda en Brown-Conrady.
    let normal = truth_camera();
    let mut rng = Rng::new(5);
    let views: Vec<Vec<Point>> = standard_poses(BOARD, SQ)
        .iter()
        .map(|p| {
            board_corners(&normal, p, BOARD, SQ)
                .into_iter()
                .map(|c| Point::new(c.x + 0.1 * rng.gauss(), c.y + 0.1 * rng.gauss()))
                .collect()
        })
        .collect();
    let rep = calibrate_views(&views, BOARD, SQ, (640, 480), &opts(LensModelChoice::Auto)).unwrap();
    eprintln!("auto (normal): {:?}", rep.model_scores);
    assert_eq!(rep.calibration.model, LensModel::BrownConrady);
    assert_eq!(rep.calibration.view_zoom, 1.0);
    assert_eq!(rep.model_scores.len(), 2);
    // Las dos tienen validación cruzada (hay 10 vistas) y está cerca del ruido.
    assert!(rep.model_scores.iter().all(|s| s.cv_rms_px.is_some()));
}

#[test]
fn la_validacion_cruzada_delata_el_sobreajuste_de_brown_en_el_borde() {
    // Con vistas solo del centro Brown-Conrady parece bueno; las vistas reservadas del
    // borde lo delatan.
    let cam = fisheye_camera();
    let views = views_of(&cam, 0.1, 3);
    let refs: Vec<&[Point]> = views.iter().map(|v| v.as_slice()).collect();
    let obj: Vec<[f64; 2]> = (0..6)
        .flat_map(|j| (0..9).map(move |i| [f64::from(i) * SQ, f64::from(j) * SQ]))
        .collect();
    let o = CalibrationOptions::default();
    let cv_fish = cross_validate(LensModel::Fisheye, &refs, &obj, (640, 480), &o).unwrap();
    let cv_brown = cross_validate(LensModel::BrownConrady, &refs, &obj, (640, 480), &o).unwrap();
    eprintln!("validación cruzada: KB {cv_fish:.3} px, BC {cv_brown:.3} px");
    assert!(cv_fish < 0.5, "KB {cv_fish}");
    assert!(cv_brown > 4.0 * cv_fish, "BC {cv_brown} vs KB {cv_fish}");
}

#[test]
fn desde_imagenes_renderizadas_el_ojo_de_pez_se_calibra_y_se_elige_solo() {
    let cam = fisheye_camera();
    let imgs: Vec<GrayImage> = fisheye_poses(BOARD, SQ)
        .iter()
        .enumerate()
        .map(|(i, p)| render_board(&cam, p, BOARD, SQ, 1.5, 300 + i as u64))
        .collect();
    let rep = calibrate_lens(&imgs, BOARD, SQ).unwrap();
    let c = &rep.calibration;
    let (rms, max) = plane_error_mm(c, &cam, 50.0);
    eprintln!(
        "desde imágenes: {:?} vistas {}/16 rms {:.3} px; plano rms {rms:.2} mm max {max:.2} mm; {:?} {:?}",
        c.model, c.views_used, c.rms_error_px, rep.model_scores, rep.warnings
    );
    assert_eq!(c.model, LensModel::Fisheye);
    assert!(c.views_used >= 8, "vistas {}", c.views_used);
    assert!(c.rms_error_px < 0.6);
    assert!(rms < 3.0, "plano rms {rms:.2} mm");
}

#[test]
fn el_json_viejo_sin_modelo_sigue_siendo_brown_conrady() {
    let json = r#"{"width":640,"height":480,"fx":620.0,"fy":627.0,"cx":322.5,"cy":238.0,
            "k1":-0.2,"k2":0.05,"p1":0.0006,"p2":-0.0005,"k3":0.0,"rms_error_px":0.3,"views_used":8}"#;
    let c: LensCalibration = serde_json::from_str(json).unwrap();
    assert_eq!(c.model, LensModel::BrownConrady);
    assert_eq!(c.k4, 0.0);
    assert_eq!(c.view_zoom, 1.0);
    c.validate().unwrap();
    // Lo nuevo se guarda con el nombre del modelo.
    let back = serde_json::to_string(&fisheye_camera()).unwrap();
    assert!(back.contains("\"model\":\"fisheye\""), "{back}");
}
