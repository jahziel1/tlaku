//! Detección de tablero y calibración de lente (incluye ojo de pez).

use super::*;

#[test]
fn detecta_las_esquinas_de_un_tablero_sintetico() {
    let p = pose((15.0, -20.0, 10.0), (-20.0, 10.0, 480.0));
    let found = run_detect_board(&to_b64(&render(&p)), BOARD)
        .unwrap()
        .expect("el tablero se debe encontrar");
    let truth = truth_corners(&p);
    assert_eq!(found.len(), 54);
    // El orden puede venir girado 180 grados; se compara con la mejor de las dos.
    let err = |order: &dyn Fn(usize) -> usize| {
        let sum: f64 = found
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let t = truth[order(i)];
                (f[0] - t[0]).hypot(f[1] - t[1])
            })
            .sum();
        sum / found.len() as f64
    };
    let direct = err(&|i| i);
    let flipped = err(&|i| truth.len() - 1 - i);
    assert!(
        direct.min(flipped) < 0.5,
        "error medio {direct:.3}/{flipped:.3} px"
    );
}

#[test]
fn sin_tablero_devuelve_none() {
    let blank = GrayImage::from_pixel(W, H, image::Luma([128]));
    assert_eq!(run_detect_board(&to_b64(&blank), BOARD).unwrap(), None);
}

#[test]
fn acepta_prefijo_data_url() {
    let blank = GrayImage::from_pixel(W, H, image::Luma([200]));
    let url = format!("data:image/png;base64,{}", to_b64(&blank));
    assert_eq!(run_detect_board(&url, BOARD).unwrap(), None);
}

#[test]
fn rechaza_entradas_invalidas() {
    assert!(
        run_detect_board("###", BOARD)
            .unwrap_err()
            .contains("base64")
    );
    let png_roto = STANDARD.encode(b"no soy una imagen");
    assert!(
        run_detect_board(&png_roto, BOARD)
            .unwrap_err()
            .contains("decodificar")
    );
    assert!(
        run_detect_board("", [1, 1])
            .unwrap_err()
            .contains("al menos")
    );
    assert!(run_calibrate(&[], BOARD, SQ).is_err());
    let img = to_b64(&GrayImage::new(W, H));
    assert!(
        run_calibrate(&[img], BOARD, 0.0)
            .unwrap_err()
            .contains("mayor que 0")
    );
}

#[test]
fn calibra_con_vistas_sinteticas() {
    let imgs: Vec<String> = views().iter().map(|p| to_b64(&render(p))).collect();
    let report = run_calibrate(&imgs, BOARD, SQ).unwrap();
    let c = &report.calibration;
    assert_eq!((c.width, c.height), (W, H));
    assert!(
        report.used_views.len() >= 6,
        "vistas usadas: {:?}",
        report.used_views
    );
    assert!(c.rms_error_px < 0.5, "rms {}", c.rms_error_px);
    assert!((c.fx - CAM.fx).abs() / CAM.fx < 0.02, "fx {}", c.fx);
    assert!((c.fy - CAM.fy).abs() / CAM.fy < 0.02, "fy {}", c.fy);
    assert!((c.k1 - CAM.k1).abs() < 0.05, "k1 {}", c.k1);
    // El informe mantiene la forma de CalibrationReport en types.ts.
    let json = serde_json::to_value(&report).unwrap();
    for key in ["calibration", "warnings", "used_views", "view_rms_px"] {
        assert!(json.get(key).is_some(), "falta {key}");
    }
    assert!(json["calibration"].get("rms_error_px").is_some());
}

#[test]
fn calibrar_exige_vistas_con_tablero() {
    let blank = to_b64(&GrayImage::from_pixel(W, H, image::Luma([128])));
    let err = run_calibrate(&vec![blank; 4], BOARD, SQ).unwrap_err();
    assert!(err.contains("al menos 3"), "{err}");
}

#[test]
fn calibra_un_ojo_de_pez_sintetico_y_elige_el_modelo() {
    use tlaku_camera::LensModelChoice;
    // Cámara de ojo de pez (KB) con un tablero en perspectiva, sin renderizar: se proyectan
    // las esquinas y se calibra por el motor con el modelo automático.
    let cam = LensCalibration {
        width: W,
        height: H,
        fx: 330.0,
        fy: 331.0,
        cx: 321.0,
        cy: 241.0,
        k1: -0.04,
        k2: 0.012,
        p1: 0.0,
        p2: 0.0,
        k3: 0.0,
        model: tlaku_camera::LensModel::Fisheye,
        k4: 0.0,
        view_zoom: 1.0,
        rms_error_px: 0.0,
        views_used: 0,
    };
    let specs = [
        ((20.0, -15.0, 8.0), (-60.0, -30.0, 300.0)),
        ((-25.0, 12.0, -10.0), (70.0, 40.0, 310.0)),
        ((8.0, 30.0, 20.0), (0.0, 0.0, 330.0)),
        ((-15.0, -25.0, 95.0), (-150.0, 50.0, 250.0)),
        ((30.0, 5.0, -35.0), (160.0, -90.0, 270.0)),
        ((-30.0, 25.0, 10.0), (-160.0, -110.0, 260.0)),
        ((12.0, -30.0, 4.0), (150.0, 100.0, 250.0)),
        ((0.0, 8.0, -6.0), (0.0, 10.0, 320.0)),
    ];
    let views: Vec<Vec<Point>> = specs
        .iter()
        .map(|&(d, c)| {
            let p = pose(d, c);
            let mut v = Vec::new();
            for j in 0..BOARD[1] {
                for i in 0..BOARD[0] {
                    let (x, y) = (f64::from(i) * SQ, f64::from(j) * SQ);
                    let q = |r: usize| p.r[r][0] * x + p.r[r][1] * y + p.t[r];
                    let (xd, yd) = cam.distort_normalized(q(0) / q(2), q(1) / q(2));
                    v.push(Point::new(cam.fx * xd + cam.cx, cam.fy * yd + cam.cy));
                }
            }
            v
        })
        .collect();
    let opts = CalibrationOptions {
        model: LensModelChoice::Auto,
        ..CalibrationOptions::default()
    };
    let rep = tlaku_camera::calibrate_views(&views, BOARD, SQ, (W, H), &opts).unwrap();
    assert_eq!(
        rep.calibration.model,
        tlaku_camera::LensModel::Fisheye,
        "{:?}",
        rep.model_scores
    );
    assert!(rep.calibration.rms_error_px < 0.05);
    // El informe viaja a la interfaz con el puntaje de cada modelo.
    let json = serde_json::to_value(&rep).unwrap();
    assert_eq!(json["calibration"]["model"], "fisheye");
    assert!(json["model_scores"].as_array().unwrap().len() == 2);
}
