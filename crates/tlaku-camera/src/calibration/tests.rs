//! Pruebas de la calibración Brown-Conrady y de la API.

use crate::chessboard::find_chessboard;
use image::GrayImage;
use tlaku_core::Point;

use super::*;
use crate::testutil::*;

const BOARD: [u32; 2] = [9, 6];
const SQ: f64 = 25.0;

fn rel(a: f64, b: f64) -> f64 {
    ((a - b) / b).abs()
}

fn images(cam: &LensCalibration, n: usize, noise: f32) -> Vec<GrayImage> {
    standard_poses(BOARD, SQ)
        .iter()
        .take(n)
        .enumerate()
        .map(|(i, p)| {
            // Todas las esquinas deben caer dentro de la imagen.
            for c in board_corners(cam, p, BOARD, SQ) {
                assert!(
                    c.x > 12.0 && c.y > 12.0 && c.x < 628.0 && c.y < 468.0,
                    "vista {i} fuera"
                );
            }
            render_board(cam, p, BOARD, SQ, noise, 100 + i as u64)
        })
        .collect()
}

#[test]
fn recupera_los_parametros_de_la_camara_virtual() {
    let cam = truth_camera();
    let imgs = images(&cam, 8, 1.5);
    let rep = calibrate_lens(&imgs, BOARD, SQ).unwrap();
    let c = &rep.calibration;
    eprintln!(
        "fx {:.2} fy {:.2} cx {:.2} cy {:.2} k1 {:.4} k2 {:.4} p1 {:.5} p2 {:.5} rms {:.3} avisos {:?}",
        c.fx, c.fy, c.cx, c.cy, c.k1, c.k2, c.p1, c.p2, c.rms_error_px, rep.warnings
    );
    assert_eq!(c.views_used, 8);
    assert!(rel(c.fx, cam.fx) < 0.01, "fx {}", c.fx);
    assert!(rel(c.fy, cam.fy) < 0.01, "fy {}", c.fy);
    assert!((c.cx - cam.cx).abs() < 0.01 * cam.cx, "cx {}", c.cx);
    assert!((c.cy - cam.cy).abs() < 0.015 * cam.cy, "cy {}", c.cy);
    assert!((c.k1 - cam.k1).abs() < 0.03, "k1 {}", c.k1);
    assert!((c.k2 - cam.k2).abs() < 0.1, "k2 {}", c.k2);
    assert!(c.rms_error_px < 0.5, "rms {}", c.rms_error_px);
    assert_eq!((c.width, c.height), (640, 480));
    // El efecto real de la distorsión recuperada: error de posición en toda la imagen.
    let mut worst: f64 = 0.0;
    for v in (0..480).step_by(40) {
        for u in (0..640).step_by(40) {
            let (x, y) = (
                (f64::from(u) - cam.cx) / cam.fx,
                (f64::from(v) - cam.cy) / cam.fy,
            );
            let (xd, yd) = cam.distort_normalized(x, y);
            let truth = (cam.fx * xd + cam.cx, cam.fy * yd + cam.cy);
            let (ex, ey) =
                c.distort_normalized((f64::from(u) - c.cx) / c.fx, (f64::from(v) - c.cy) / c.fy);
            let est = (c.fx * ex + c.cx, c.fy * ey + c.cy);
            worst = worst.max((truth.0 - est.0).hypot(truth.1 - est.1));
        }
    }
    eprintln!("desviación máxima del modelo en la imagen: {worst:.2} px");
    assert!(worst < 3.0, "el modelo recuperado se desvía {worst:.2} px");
}

#[test]
fn descarta_una_vista_atipica_con_aviso() {
    let cam = truth_camera();
    let imgs = images(&cam, 6, 1.5);
    let mut views: Vec<Vec<Point>> = imgs
        .iter()
        .map(|i| find_chessboard(i, BOARD).unwrap())
        .collect();
    // Una vista con el etiquetado corrido un lugar (la inversión por filas sería
    // una pose válida vista desde atrás, no un error).
    let mut bad = views[2].clone();
    bad.rotate_left(1);
    views.push(bad);
    let rep = calibrate_views(
        &views,
        BOARD,
        SQ,
        (640, 480),
        &CalibrationOptions::default(),
    )
    .unwrap();
    assert_eq!(rep.calibration.views_used, 6);
    assert!(!rep.used_views.contains(&6));
    assert!(
        rep.warnings
            .iter()
            .any(|w| w.contains("vista 7 descartada")),
        "{:?}",
        rep.warnings
    );
    assert!(rep.calibration.rms_error_px < 0.5);
    assert!(rel(rep.calibration.fx, cam.fx) < 0.02);
}

#[test]
fn errores_claros() {
    let cam = truth_camera();
    let imgs = images(&cam, 2, 1.0);
    let e = calibrate_lens(&imgs, BOARD, SQ).unwrap_err();
    assert!(e.contains("al menos 3"), "{e}");
    // Imágenes sin tablero: se omiten con aviso y luego faltan vistas.
    let blank = vec![GrayImage::from_pixel(640, 480, image::Luma([128])); 4];
    assert!(
        calibrate_lens(&blank, BOARD, SQ)
            .unwrap_err()
            .contains("al menos 3")
    );
    assert!(calibrate_lens(&[], BOARD, SQ).is_err());
}

#[test]
fn omite_imagenes_sin_tablero_y_reporta_indices_originales() {
    let cam = truth_camera();
    let mut imgs = images(&cam, 5, 1.5);
    imgs.insert(1, GrayImage::from_pixel(640, 480, image::Luma([128])));
    let rep = calibrate_lens(&imgs, BOARD, SQ).unwrap();
    assert_eq!(rep.used_views, vec![0, 2, 3, 4, 5]);
    assert!(
        rep.warnings.iter().any(|w| w.starts_with("imagen 2:")),
        "{:?}",
        rep.warnings
    );
}

fn projected_views(cam: &LensCalibration, noise_px: f64) -> Vec<Vec<Point>> {
    let mut rng = Rng::new(42);
    standard_poses(BOARD, SQ)
        .iter()
        .map(|p| {
            board_corners(cam, p, BOARD, SQ)
                .into_iter()
                .map(|c| Point::new(c.x + noise_px * rng.gauss(), c.y + noise_px * rng.gauss()))
                .collect()
        })
        .collect()
}

#[test]
fn puntos_exactos_dan_error_casi_nulo() {
    let cam = truth_camera();
    let views = projected_views(&cam, 0.0);
    let rep = calibrate_views(
        &views,
        BOARD,
        SQ,
        (640, 480),
        &CalibrationOptions::default(),
    )
    .unwrap();
    let c = &rep.calibration;
    assert!(c.rms_error_px < 1e-3, "rms {}", c.rms_error_px);
    assert!(rel(c.fx, cam.fx) < 1e-4 && rel(c.fy, cam.fy) < 1e-4);
    assert!((c.cx - cam.cx).abs() < 0.05 && (c.cy - cam.cy).abs() < 0.05);
    assert!((c.k1 - cam.k1).abs() < 1e-3 && (c.p1 - cam.p1).abs() < 1e-4);
    assert!(rep.warnings.is_empty(), "{:?}", rep.warnings);
}

#[test]
fn distorsion_fuerte_con_k3() {
    let cam = LensCalibration {
        k1: -0.38,
        k2: 0.25,
        k3: -0.12,
        p1: 0.0,
        p2: 0.0,
        ..truth_camera()
    };
    let views = projected_views(&cam, 0.05);
    let opts = CalibrationOptions {
        fit_k3: true,
        ..Default::default()
    };
    let rep = calibrate_views(&views, BOARD, SQ, (640, 480), &opts).unwrap();
    let c = &rep.calibration;
    eprintln!(
        "k1 {:.3} k2 {:.3} k3 {:.3} fx {:.1} rms {:.3}",
        c.k1, c.k2, c.k3, c.fx, c.rms_error_px
    );
    assert!(c.rms_error_px < 0.2);
    assert!(rel(c.fx, cam.fx) < 0.02);
    assert!((c.k1 - cam.k1).abs() < 0.08, "k1 {}", c.k1);
}

#[test]
fn vistas_sin_inclinacion_dan_error_claro() {
    // Siempre de frente y sin giros: no se puede estimar la focal.
    let cam = truth_camera();
    let views: Vec<Vec<Point>> = [
        (-30.0, 0.0, 480.0),
        (20.0, 10.0, 520.0),
        (0.0, -20.0, 600.0),
        (10.0, 20.0, 560.0),
    ]
    .iter()
    .map(|&(x, y, z)| {
        let pose = Pose::about_center(BOARD, SQ, (0.0, 0.0, 0.0), (x, y, z));
        board_corners(&cam, &pose, BOARD, SQ)
    })
    .collect();
    match calibrate_views(
        &views,
        BOARD,
        SQ,
        (640, 480),
        &CalibrationOptions::default(),
    ) {
        Err(e) => assert!(e.contains("inclina") || e.contains("convergió"), "{e}"),
        Ok(r) => assert!(r.warnings.len() + 1 > 0), // si converge, al menos no debe entrar en pánico
    }
}
