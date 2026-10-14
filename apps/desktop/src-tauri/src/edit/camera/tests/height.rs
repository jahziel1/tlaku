//! Modelo de altura del material.

use super::*;

// Cámara de prueba para el modelo de altura: pinhole inclinada, cama con Y arriba.
const K: [f64; 4] = [620.0, 625.0, 322.0, 241.0];

fn matrix_at(h: f64) -> Mat {
    let r = mul(
        &rot(12.0, -8.0, 5.0),
        &[[1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]],
    );
    let t = [-140.0, 95.0, 380.0];
    let cols = |c: usize| -> [f64; 3] {
        if c < 2 {
            [r[0][c], r[1][c], r[2][c]]
        } else {
            [t[0] + h * r[0][2], t[1] + h * r[1][2], t[2] + h * r[2][2]]
        }
    };
    let k = [[K[0], 0.0, K[2]], [0.0, K[1], K[3]], [0.0, 0.0, 1.0]];
    let mut m = [[0.0; 3]; 3];
    for c in 0..3 {
        let col = cols(c);
        for (i, row) in m.iter_mut().enumerate() {
            row[c] = (0..3).map(|q| k[i][q] * col[q]).sum();
        }
    }
    m
}

fn sample_at(h: f64) -> AlignmentSample {
    let truth = Homography { m: matrix_at(h) };
    let bed: Vec<[f64; 2]> = (0..9)
        .map(|k| {
            [
                25.0 + 125.0 * f64::from(k % 3),
                20.0 + 80.0 * f64::from(k / 3),
            ]
        })
        .collect();
    let image_px = bed
        .iter()
        .map(|b| {
            let q = truth.apply(pt(*b));
            [q.x, q.y]
        })
        .collect();
    AlignmentSample {
        height_mm: h,
        image_px,
        bed_mm: bed,
        loo_rms_mm: None,
        loo_max_mm: None,
    }
}

fn marks_alignment(samples: Vec<AlignmentSample>) -> CameraAlignment {
    CameraAlignment {
        image_size: [W, H],
        image_px: [[0.0; 2]; 4],
        bed_mm: [[0.0; 2]; 4],
        to_bed: [[0.0; 3]; 3],
        rms_mm: 0.0,
        method: AlignmentMethod::Marks,
        reference_height_mm: 0.0,
        samples,
        height: None,
    }
}

/// Calibración cuya matriz corregida coincide con `K` (el punto principal va en el borde del píxel).
fn lens_for_k() -> LensCalibration {
    LensCalibration {
        width: W,
        height: H,
        fx: K[0],
        fy: K[1],
        cx: K[2] - 0.5,
        cy: K[3] - 0.5,
        k1: 0.0,
        k2: 0.0,
        p1: 0.0,
        p2: 0.0,
        k3: 0.0,
        model: tlaku_camera::LensModel::BrownConrady,
        k4: 0.0,
        view_zoom: 1.0,
        rms_error_px: 0.2,
        views_used: 8,
    }
}

fn bed_error_at(a: &CameraAlignment, h: f64) -> f64 {
    let model = a.height.expect("hay modelo de altura");
    let truth = Homography { m: matrix_at(h) };
    let mut worst: f64 = 0.0;
    for j in 0..=8 {
        for i in 0..=12 {
            let bed = Point::new(f64::from(i) * 25.0, f64::from(j) * 25.0);
            let est = model.pixel_to_bed(truth.apply(bed), h).unwrap();
            worst = worst.max(est.dist(bed));
        }
    }
    worst
}

#[test]
fn dos_alturas_dan_un_modelo_medido_que_interpola() {
    let tmp = TempDir::new();
    let mut setup = sample_setup();
    setup.alignment = Some(marks_alignment(vec![sample_at(20.0), sample_at(0.0)]));
    setup.prefs.material_height_mm = 7.5;
    let saved = store_setup(&tmp.0, setup).unwrap();
    let a = saved.alignment.as_ref().unwrap();
    assert_eq!(a.method, AlignmentMethod::Marks);
    assert_eq!(a.samples.len(), 2);
    assert_eq!(a.samples[0].height_mm, 0.0, "ordenadas por altura");
    let model = a.height.expect("modelo");
    assert_eq!(model.source, tlaku_camera::HeightSource::Measured);
    assert!(a.rms_mm < 1e-6);
    for h in [0.0, 3.0, 7.5, 12.0, 20.0] {
        assert!(bed_error_at(a, h) < 1e-5, "h {h}");
    }
    // `to_bed` corresponde a la altura de referencia (la más baja).
    assert_eq!(a.reference_height_mm, 0.0);
    let truth = Homography { m: matrix_at(0.0) };
    let q = Homography { m: a.to_bed }.apply(truth.apply(Point::new(100.0, 60.0)));
    assert!(q.dist(Point::new(100.0, 60.0)) < 1e-5);
    // Las 4 esquinas de mostrar son las del rectángulo de las marcas.
    assert_eq!(a.bed_mm[0], [25.0, 20.0]);
    assert_eq!(a.bed_mm[2], [275.0, 180.0]);
    // Persiste y se recarga igual; el grosor del material también.
    // (el JSON puede redondear el último decimal: se compara con tolerancia)
    let loaded = load_setup(&tmp.0).unwrap();
    assert_eq!(loaded.prefs, saved.prefs);
    assert_eq!(loaded.prefs.material_height_mm, 7.5);
    let b = loaded.alignment.as_ref().unwrap();
    assert_eq!((b.method, b.samples.len()), (a.method, a.samples.len()));
    assert_eq!(b.height.map(|m| m.source), a.height.map(|m| m.source));
    for h in [0.0, 7.5, 20.0] {
        assert!(bed_error_at(b, h) < 1e-5, "recargado, h {h}");
    }
}

#[test]
fn una_sola_altura_con_lente_usa_el_modelo_de_la_camara() {
    let mut setup = sample_setup();
    setup.calibration = Some(lens_for_k());
    setup.alignment = Some(marks_alignment(vec![sample_at(0.0)]));
    let tmp = TempDir::new();
    let saved = store_setup(&tmp.0, setup).unwrap();
    let a = saved.alignment.unwrap();
    assert_eq!(
        a.height.unwrap().source,
        tlaku_camera::HeightSource::Modelled
    );
    // Con la matriz exacta, el respaldo predice bien las demás alturas.
    for h in [5.0, 20.0] {
        assert!(bed_error_at(&a, h) < 1e-4, "h {h}: {}", bed_error_at(&a, h));
    }
    // Sin lente no hay respaldo: queda solo la homografía de esa altura.
    let mut no_lens = sample_setup();
    no_lens.calibration = None;
    no_lens.alignment = Some(marks_alignment(vec![sample_at(0.0)]));
    let a = store_setup(&tmp.0, no_lens).unwrap().alignment.unwrap();
    assert!(a.height.is_none());
    assert!(a.rms_mm < 1e-6);
}

#[test]
fn la_alineacion_manual_tambien_recibe_el_respaldo_de_altura() {
    let mut setup = sample_setup();
    setup.calibration = Some(lens_for_k());
    let tmp = TempDir::new();
    let a = store_setup(&tmp.0, setup).unwrap().alignment.unwrap();
    assert_eq!(a.method, AlignmentMethod::Manual);
    assert_eq!(
        a.height.map(|m| m.source),
        Some(tlaku_camera::HeightSource::Modelled)
    );
}

#[test]
fn una_captura_a_la_misma_altura_reemplaza_a_la_anterior() {
    let mut older = sample_at(0.0);
    older.image_px[0][0] += 50.0; // mala captura
    let mut setup = sample_setup();
    setup.alignment = Some(marks_alignment(vec![
        older,
        sample_at(0.1),
        sample_at(20.0),
    ]));
    let tmp = TempDir::new();
    let a = store_setup(&tmp.0, setup).unwrap().alignment.unwrap();
    assert_eq!(a.samples.len(), 2);
    assert!(a.rms_mm < 1e-6);
}

#[test]
fn rechaza_marcas_invalidas() {
    let tmp = TempDir::new();
    let bad = |f: &dyn Fn(&mut AlignmentSample)| {
        let mut s = sample_at(0.0);
        f(&mut s);
        let mut setup = sample_setup();
        setup.alignment = Some(marks_alignment(vec![s]));
        store_setup(&tmp.0, setup)
    };
    assert!(bad(&|s| s.image_px.truncate(3)).is_err());
    assert!(bad(&|s| s.bed_mm.pop().map(|_| ()).unwrap_or(())).is_err());
    assert!(bad(&|s| s.image_px[2][1] = f64::NAN).is_err());
    assert!(bad(&|s| s.height_mm = 900.0).is_err());
    // Dos alturas casi iguales pero distintas: no hay modelo (menos de 2 mm).
    let mut setup = sample_setup();
    setup.alignment = Some(marks_alignment(vec![sample_at(0.0), sample_at(1.0)]));
    let e = store_setup(&tmp.0, setup).unwrap_err();
    assert!(e.contains("difieren"), "{e}");
    assert!(!setup_file(&tmp.0).exists());
}

#[test]
fn la_altura_con_ojo_de_pez_usa_la_focal_de_la_imagen_corregida() {
    let mut lens = lens_for_k();
    let plain = corrected_camera(&lens, [W, H]);
    lens.view_zoom = 0.5;
    let zoomed = corrected_camera(&lens, [W, H]);
    assert!((zoomed[0] - 0.5 * plain[0]).abs() < 1e-9 && (zoomed[2] - plain[2]).abs() < 1e-9);
}
