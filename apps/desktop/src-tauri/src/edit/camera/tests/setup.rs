//! Configuración persistente: archivo, validaciones y compatibilidad con `camera.json` viejos.

use super::*;

#[test]
fn la_configuracion_sobrevive_al_reinicio() {
    let tmp = TempDir::new();
    assert_eq!(load_setup(&tmp.0).unwrap(), CameraSetup::default());
    let saved = store_setup(&tmp.0, sample_setup()).unwrap();
    // "Reiniciar": se lee otra vez desde el disco.
    let loaded = load_setup(&tmp.0).unwrap();
    assert_eq!(loaded, saved);
    let a = loaded.alignment.unwrap();
    assert!(a.rms_mm < 1e-6);
    // La homografía recalculada lleva cada punto marcado a su coordenada de cama.
    let h = tlaku_camera::Homography { m: a.to_bed };
    for (img, bed) in a.image_px.iter().zip(&a.bed_mm) {
        let q = h.apply(Point::new(img[0], img[1]));
        assert!((q.x - bed[0]).hypot(q.y - bed[1]) < 1e-6);
    }
    assert_eq!(loaded.calibration.unwrap().views_used, 8);
}

#[test]
fn rechaza_alineaciones_degeneradas() {
    let tmp = TempDir::new();
    let mut s = sample_setup();
    s.alignment.as_mut().unwrap().image_px = [[0.0, 0.0], [10.0, 10.0], [20.0, 20.0], [30.0, 30.0]];
    assert!(store_setup(&tmp.0, s).is_err());
    let mut s = sample_setup();
    s.alignment.as_mut().unwrap().bed_mm[0][0] = f64::NAN;
    assert!(store_setup(&tmp.0, s).is_err());
    let mut s = sample_setup();
    s.calibration.as_mut().unwrap().fx = -1.0;
    assert!(store_setup(&tmp.0, s).is_err());
    assert!(!setup_file(&tmp.0).exists());
}

#[test]
fn un_archivo_danado_se_aparta_sin_perder_nada() {
    let tmp = TempDir::new();
    fs::create_dir_all(&tmp.0).unwrap();
    fs::write(setup_file(&tmp.0), "{ esto no es json").unwrap();
    assert_eq!(load_setup(&tmp.0).unwrap(), CameraSetup::default());
    assert!(tmp.0.join("camera.json.dañado").exists());
    assert!(!setup_file(&tmp.0).exists());
}

#[test]
fn la_opacidad_se_limita() {
    let tmp = TempDir::new();
    let mut s = CameraSetup::default();
    s.prefs.opacity = 7.0;
    assert_eq!(store_setup(&tmp.0, s).unwrap().prefs.opacity, 1.0);
}

#[test]
fn guarda_el_tablero_solo_como_svg_o_pdf() {
    let tmp = TempDir::new();
    fs::create_dir_all(&tmp.0).unwrap();
    let ok = tmp.0.join("tablero.svg");
    write_board_file(ok.to_str().unwrap(), &STANDARD.encode(b"<svg/>")).unwrap();
    assert_eq!(fs::read(&ok).unwrap(), b"<svg/>");
    let bad = tmp.0.join("tablero.sh");
    assert!(write_board_file(bad.to_str().unwrap(), &STANDARD.encode(b"x")).is_err());
    assert!(!bad.exists());
}

#[test]
fn un_camera_json_anterior_sin_los_campos_nuevos_sigue_cargando() {
    let tmp = TempDir::new();
    fs::create_dir_all(&tmp.0).unwrap();
    let old = r#"{
          "version": 1,
          "calibration": null,
          "alignment": {
            "image_size": [640, 480],
            "image_px": [[100.0, 400.0], [540.0, 410.0], [530.0, 60.0], [110.0, 70.0]],
            "bed_mm": [[10.0, 10.0], [289.4, 10.0], [289.4, 225.9], [10.0, 225.9]],
            "to_bed": [[0,0,0],[0,0,0],[0,0,0]],
            "rms_mm": 0.0
          },
          "prefs": { "opacity": 0.5, "visible": true }
        }"#;
    fs::write(setup_file(&tmp.0), old).unwrap();
    let s = load_setup(&tmp.0).unwrap();
    let a = s.alignment.expect("la alineación vieja se conserva");
    assert_eq!(a.method, AlignmentMethod::Manual);
    assert!(a.samples.is_empty() && a.height.is_none());
    assert_eq!(a.reference_height_mm, 0.0);
    assert_eq!(s.prefs.material_height_mm, 0.0);
    assert!(s.prefs.visible);
    assert!(!tmp.0.join("camera.json.dañado").exists());
}

fn head_calibration() -> HeadCalibration {
    HeadCalibration {
        image_size: [W, H],
        ref_px: [320.0, 240.0],
        px_to_mm: [[0.1, 0.0], [0.0, -0.1]],
        offset_mm: [12.5, -30.0],
        rms_mm: 0.05,
        observations: 4,
    }
}

#[test]
fn la_camara_del_cabezal_y_los_ajustes_nuevos_sobreviven_al_reinicio() {
    let tmp = TempDir::new();
    let mut s = sample_setup();
    s.head = HeadCamera {
        enabled: true,
        calibration: Some(head_calibration()),
    };
    s.prefs.grayscale = true;
    s.prefs.update_on_job_finish = true;
    s.calibration.as_mut().unwrap().model = tlaku_camera::LensModel::Fisheye;
    s.calibration.as_mut().unwrap().k4 = 0.002;
    s.calibration.as_mut().unwrap().view_zoom = 0.6;
    let saved = store_setup(&tmp.0, s).unwrap();
    let loaded = load_setup(&tmp.0).unwrap();
    assert_eq!(loaded, saved);
    assert!(loaded.head.enabled && loaded.prefs.grayscale && loaded.prefs.update_on_job_finish);
    assert_eq!(
        loaded.calibration.unwrap().model,
        tlaku_camera::LensModel::Fisheye
    );
    // Calibración de cabezal inválida: se rechaza al guardar.
    let mut bad = sample_setup();
    let mut c = head_calibration();
    c.px_to_mm = [[0.0, 0.0], [0.0, 0.0]];
    bad.head.calibration = Some(c);
    assert!(store_setup(&tmp.0, bad).unwrap_err().contains("escala"));
    let mut bad = sample_setup();
    let mut c = head_calibration();
    c.offset_mm[0] = f64::INFINITY;
    bad.head.calibration = Some(c);
    assert!(store_setup(&tmp.0, bad).is_err());
}

#[test]
fn un_camera_json_de_antes_de_la_ola_4_se_lee_igual() {
    let tmp = TempDir::new();
    fs::create_dir_all(&tmp.0).unwrap();
    let old = r#"{"version":1,
          "calibration":{"width":640,"height":480,"fx":620.0,"fy":625.0,"cx":320.0,"cy":240.0,
            "k1":-0.12,"k2":0.01,"p1":0.0,"p2":0.0,"k3":0.0,"rms_error_px":0.31,"views_used":8},
          "alignment":null,
          "prefs":{"device_id":null,"device_label":null,"width":null,"height":null,"opacity":0.5,"visible":true}}"#;
    fs::write(setup_file(&tmp.0), old).unwrap();
    let loaded = load_setup(&tmp.0).unwrap();
    let c = loaded
        .calibration
        .expect("la calibración vieja se conserva");
    assert_eq!(c.model, tlaku_camera::LensModel::BrownConrady);
    assert_eq!((c.k4, c.view_zoom), (0.0, 1.0));
    assert!(!loaded.head.enabled && loaded.head.calibration.is_none());
    assert!(!loaded.prefs.grayscale && loaded.prefs.visible);
    assert!(!tmp.0.join("camera.json.dañado").exists());
}
