//! Pruebas de la disposición y de la detección con imágenes sintéticas.

use crate::homography::Homography;
use crate::imgproc::{FloatImage, gaussian_blur};
use image::GrayImage;
use tlaku_core::Point;

use super::*;
use crate::calibration::LensCalibration;
use crate::height::{HeightSample, fit_height_model};
use crate::testutil::{Rng, truth_camera};
use crate::undistort::UndistortMap;
use nalgebra::{Matrix3, Rotation3, Vector3};

const BED: [f64; 2] = [300.0, 200.0];

/// Pose de la cama frente a la cámara de prueba (X derecha, Y arriba, Z hacia la cámara).
#[derive(Clone, Copy)]
struct BedPose {
    r: Matrix3<f64>,
    t: Vector3<f64>,
}

fn bed_pose(tilt: (f64, f64, f64), center: (f64, f64, f64)) -> BedPose {
    let d = std::f64::consts::PI / 180.0;
    let rot = Rotation3::from_euler_angles(tilt.0 * d, tilt.1 * d, tilt.2 * d);
    let flip = Matrix3::new(1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, -1.0);
    let r = rot.matrix() * flip;
    // `center` es adónde cae el centro de la cama (a altura 0) en la cámara.
    let c = Vector3::new(BED[0] / 2.0, BED[1] / 2.0, 0.0);
    let t = Vector3::new(center.0, center.1, center.2) - r * c;
    BedPose { r, t }
}

fn std_pose() -> BedPose {
    bed_pose((7.0, -5.0, 3.0), (4.0, -3.0, 345.0))
}

/// Matriz cama a imagen corregida (centro de píxel) a altura `h`.
fn truth_h(cam: &LensCalibration, p: &BedPose, h: f64) -> Homography {
    let k = Matrix3::new(cam.fx, 0.0, cam.cx, 0.0, cam.fy, cam.cy, 0.0, 0.0, 1.0);
    let cols = [
        p.r.column(0).into_owned(),
        p.r.column(1).into_owned(),
        p.t + p.r.column(2) * h,
    ];
    Homography::from_matrix(&(k * Matrix3::from_columns(&cols)))
}

#[derive(Clone, Copy)]
struct Look {
    noise: f32,
    blur: f32,
    line_mm: f64,
    texture: f32,
    shade: f32,
    height: f64,
}

const LOOK: Look = Look {
    noise: 2.0,
    blur: 0.8,
    line_mm: 0.30,
    texture: 3.0,
    shade: 25.0,
    height: 0.0,
};

fn mark_dark(layout: &MarkLayout, x: f64, y: f64, half_w: f64) -> bool {
    let a = layout.arm_mm;
    for (i, c) in layout.marks.iter().enumerate() {
        let (dx, dy) = (x - c.x, y - c.y);
        if dx.abs() > a + 1.0 || dy.abs() > a + 1.0 {
            continue;
        }
        if (dy.abs() <= half_w && dx.abs() <= a) || (dx.abs() <= half_w && dy.abs() <= a) {
            return true;
        }
        if i == 0 && (dx.hypot(dy) - layout.ring_mm).abs() <= half_w {
            return true;
        }
    }
    false
}

/// Foto cruda (con distorsión de lente) de las marcas grabadas sobre un cartón.
fn render(
    cam: &LensCalibration,
    pose: &BedPose,
    layout: &MarkLayout,
    look: Look,
    seed: u64,
) -> GrayImage {
    let (w, h) = (cam.width as usize, cam.height as usize);
    let paper = |u: f64, v: f64| -> f32 {
        // Cartón con un degradado de iluminación (viñeteo) suave.
        let (nx, ny) = ((u - 320.0) / 320.0, (v - 240.0) / 240.0);
        215.0 - look.shade * (nx * nx + ny * ny) as f32 * 0.5 + look.shade * 0.3 * nx as f32
    };
    let n = 4;
    let mut data = vec![0.0f32; w * h];
    // Rayo (x, y, 1) con el plano a altura `look.height`.
    let inv = {
        let r = pose.r;
        let t = pose.t + r.column(2) * look.height;
        Matrix3::from_columns(&[r.column(0).into_owned(), r.column(1).into_owned(), t])
            .try_inverse()
            .unwrap()
    };
    for j in 0..h {
        for i in 0..w {
            let mut dark = 0.0;
            for sj in 0..n {
                for si in 0..n {
                    let u = i as f64 - 0.5 + (si as f64 + 0.5) / n as f64;
                    let v = j as f64 - 0.5 + (sj as f64 + 0.5) / n as f64;
                    let (xd, yd) = ((u - cam.cx) / cam.fx, (v - cam.cy) / cam.fy);
                    let (x, y) = cam.undistort_normalized(xd, yd);
                    let q = inv * Vector3::new(x, y, 1.0);
                    let (bx, by) = (q.x / q.z, q.y / q.z);
                    if mark_dark(layout, bx, by, look.line_mm / 2.0) {
                        dark += 1.0;
                    }
                }
            }
            let frac = dark / (n * n) as f32;
            let base = paper(i as f64, j as f64);
            data[j * w + i] = base * (1.0 - frac) + 55.0 * frac;
        }
    }
    let blurred = gaussian_blur(&FloatImage { w, h, data }, look.blur);
    let mut rng = Rng::new(seed);
    // Textura de fibras: ruido gaussiano de baja frecuencia más ruido de sensor.
    let mut tex = FloatImage {
        w,
        h,
        data: (0..w * h).map(|_| rng.gauss() as f32).collect(),
    };
    tex = gaussian_blur(&tex, 1.5);
    let pixels: Vec<u8> = blurred
        .data
        .iter()
        .zip(&tex.data)
        .map(|(&v, &tx)| {
            (v + look.texture * 6.0 * tx + look.noise * rng.gauss() as f32)
                .round()
                .clamp(0.0, 255.0) as u8
        })
        .collect();
    GrayImage::from_raw(cam.width, cam.height, pixels).unwrap()
}

fn corrected(cam: &LensCalibration, raw: &GrayImage) -> GrayImage {
    UndistortMap::new(cam, (cam.width, cam.height))
        .unwrap()
        .undistort(raw)
        .unwrap()
}

/// Error máximo y rms (mm) de la alineación estimada frente a la verdad, en una rejilla.
fn alignment_error(
    cam: &LensCalibration,
    pose: &BedPose,
    h: f64,
    to_bed: &Homography,
) -> (f64, f64) {
    let truth = truth_h(cam, pose, h);
    let mut worst: f64 = 0.0;
    let mut sum = 0.0;
    let mut n = 0.0;
    for j in 0..=8 {
        for i in 0..=12 {
            let bed = Point::new(i as f64 * 25.0, j as f64 * 25.0);
            let px = truth.apply(bed);
            let e = to_bed.apply(px).dist(bed);
            worst = worst.max(e);
            sum += e * e;
            n += 1.0;
        }
    }
    (worst, (sum / n).sqrt())
}

fn rep_to_bed(r: &MarksReport) -> Homography {
    Homography { m: r.to_bed }
}

#[test]
fn la_disposicion_cubre_el_85_por_ciento_y_tiene_orden_fijo() {
    let l4 = MarkLayout::new(BED, 4).unwrap();
    assert_eq!(l4.marks.len(), 4);
    assert!((l4.marks[0].x - 22.5).abs() < 1e-9 && (l4.marks[2].x - 277.5).abs() < 1e-9);
    assert!((l4.marks[2].x - l4.marks[0].x - 0.85 * 300.0).abs() < 1e-9);
    let l9 = MarkLayout::new(BED, 9).unwrap();
    assert_eq!(l9.marks.len(), 9);
    assert_eq!(l9.marks[..4], l4.marks[..]);
    assert!((l9.marks[8].x - 150.0).abs() < 1e-9 && (l9.marks[8].y - 100.0).abs() < 1e-9);
    // Dos trazos por cruz y un círculo (solo en la marca 0).
    let s = l9.strokes();
    assert_eq!(s.len(), 9 * 2 + 1);
    assert_eq!(s.iter().filter(|x| x.closed).count(), 1);
    assert!(MarkLayout::new(BED, 5).is_err());
    assert!(MarkLayout::new([30.0, 200.0], 4).is_err());
}

#[test]
fn detecta_9_marcas_con_alineacion_provisional_y_mide_la_precision() {
    let cam = truth_camera();
    let pose = std_pose();
    let layout = MarkLayout::new(BED, 9).unwrap();
    let img = corrected(&cam, &render(&cam, &pose, &layout, LOOK, 11));
    // Alineación provisional con error: 8 px de desplazamiento y 2 % de escala.
    let truth = truth_h(&cam, &pose, 0.0);
    let wobble = Homography {
        m: [[1.02, 0.0, 8.0], [0.0, 1.02, -6.0], [0.0, 0.0, 1.0]],
    };
    let prior = truth.then(&wobble).inverse().unwrap();
    let rep = detect_marks(
        &img,
        &layout,
        &DetectOptions {
            prior_to_bed: Some(prior),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(rep.used_prior);
    assert_eq!((rep.found, rep.used), (9, 9));
    let (worst, rms) = alignment_error(&cam, &pose, 0.0, &rep_to_bed(&rep));
    eprintln!(
        "9 marcas: ajuste {:.3} mm, LOO rms {:?} max {:?}, error real max {worst:.3} rms {rms:.3}",
        rep.rms_mm, rep.loo_rms_mm, rep.loo_max_mm
    );
    assert!(worst < 0.35, "error real máximo {worst:.3} mm");
    assert!(rms < 0.2, "error real rms {rms:.3} mm");
    let loo = rep.loo_rms_mm.expect("con 9 marcas hay validación cruzada");
    assert!(loo < 0.35, "LOO rms {loo:.3}");
    // La precisión medida es honesta: no subestima el error real por un factor grande.
    assert!(loo > rms * 0.4, "LOO {loo:.3} vs real {rms:.3}");
}

#[test]
fn detecta_sin_alineacion_previa_y_resuelve_la_orientacion() {
    let cam = truth_camera();
    let layout = MarkLayout::new(BED, 9).unwrap();
    // Normal y girada 180 grados (cámara montada al revés): el círculo fija el origen.
    for (rot_z, dist, seed) in [
        (3.0, 345.0, 21u64),
        (183.0, 345.0, 22u64),
        (95.0, 480.0, 23u64),
    ] {
        let pose = bed_pose((6.0, 4.0, rot_z), (0.0, 0.0, dist));
        let img = corrected(&cam, &render(&cam, &pose, &layout, LOOK, seed));
        let rep = detect_marks(&img, &layout, &DetectOptions::default()).unwrap();
        assert!(!rep.used_prior);
        assert_eq!(rep.used, 9, "rot {rot_z}: {:?}", rep.warnings);
        let (worst, rms) = alignment_error(&cam, &pose, 0.0, &rep_to_bed(&rep));
        eprintln!(
            "sin alineación previa, giro {rot_z}: error real máx {worst:.3} rms {rms:.3} mm; LOO rms {:?}",
            rep.loo_rms_mm
        );
        assert!(worst < 0.4, "rot {rot_z}: error {worst:.3} mm");
    }
}

#[test]
fn tolera_una_marca_ausente() {
    let cam = truth_camera();
    let pose = std_pose();
    let full = MarkLayout::new(BED, 9).unwrap();
    // Se borra la marca 6 (centro superior) de la foto: no se graba.
    let mut partial = full.clone();
    partial.marks[6] = Point::new(-500.0, -500.0);
    let img = corrected(&cam, &render(&cam, &pose, &partial, LOOK, 31));
    let rep = detect_marks(&img, &full, &DetectOptions::default()).unwrap();
    assert_eq!(rep.found, 8, "{:?}", rep.warnings);
    assert!(rep.marks[6].image.is_none());
    let (worst, _) = alignment_error(&cam, &pose, 0.0, &rep_to_bed(&rep));
    assert!(worst < 0.4, "error {worst:.3} mm");
    assert!(rep.warnings.iter().any(|w| w.contains("8 de 9")));
}

#[test]
fn con_4_marcas_funciona_pero_avisa_que_no_mide_la_precision() {
    let cam = truth_camera();
    let pose = std_pose();
    let layout = MarkLayout::new(BED, 4).unwrap();
    let img = corrected(&cam, &render(&cam, &pose, &layout, LOOK, 41));
    let rep = detect_marks(&img, &layout, &DetectOptions::default()).unwrap();
    assert_eq!(rep.used, 4);
    assert!(rep.loo_rms_mm.is_none() && rep.loo_max_mm.is_none());
    let (worst, rms) = alignment_error(&cam, &pose, 0.0, &rep_to_bed(&rep));
    eprintln!("4 marcas: error real máx {worst:.3} rms {rms:.3} mm");
    // Con 4 marcas el centro de la cama queda más lejos de los puntos de ajuste.
    assert!(worst < 0.8, "error {worst:.3} mm");
}

#[test]
fn una_marca_falsa_cerca_de_una_cruz_se_descarta_con_ransac() {
    let cam = truth_camera();
    let pose = std_pose();
    let layout = MarkLayout::new(BED, 9).unwrap();
    let img = corrected(&cam, &render(&cam, &pose, &layout, LOOK, 51));
    let truth = truth_h(&cam, &pose, 0.0);
    // Alineación provisional muy buena; se estropea una cruz dibujando una mancha oscura
    // grande al lado, que mueve su centro varios mm.
    let mut img = img;
    let spot = truth.apply(Point::new(layout.marks[5].x + 4.0, layout.marks[5].y + 3.0));
    for dy in -5i32..=5 {
        for dx in -5i32..=5 {
            if dx * dx + dy * dy <= 25 {
                let (x, y) = ((spot.x as i32 + dx) as u32, (spot.y as i32 + dy) as u32);
                img.put_pixel(x, y, image::Luma([40]));
            }
        }
    }
    let rep = detect_marks(
        &img,
        &layout,
        &DetectOptions {
            prior_to_bed: Some(truth.inverse().unwrap()),
            ..Default::default()
        },
    )
    .unwrap();
    // Con la marca 5 estropeada, el ajuste final no debe empeorar.
    let (worst, _) = alignment_error(&cam, &pose, 0.0, &rep_to_bed(&rep));
    assert!(worst < 0.5, "error {worst:.3} mm; usadas {}", rep.used);
}

#[test]
fn sin_marcas_da_un_error_claro() {
    let blank = GrayImage::from_pixel(640, 480, image::Luma([180]));
    let layout = MarkLayout::new(BED, 9).unwrap();
    let e = detect_marks(&blank, &layout, &DetectOptions::default()).unwrap_err();
    assert!(e.contains("marcas"), "{e}");
}

#[test]
fn el_desplazamiento_de_pixel_se_aplica_a_la_salida() {
    let cam = truth_camera();
    let pose = std_pose();
    let layout = MarkLayout::new(BED, 9).unwrap();
    let img = corrected(&cam, &render(&cam, &pose, &layout, LOOK, 61));
    let a = detect_marks(&img, &layout, &DetectOptions::default()).unwrap();
    let b = detect_marks(
        &img,
        &layout,
        &DetectOptions {
            pixel_offset: 0.5,
            ..Default::default()
        },
    )
    .unwrap();
    let (pa, pb) = (a.marks[8].image.unwrap(), b.marks[8].image.unwrap());
    assert!((pb[0] - pa[0] - 0.5).abs() < 1e-9 && (pb[1] - pa[1] - 0.5).abs() < 1e-9);
    // Las dos alineaciones describen lo mismo en sus coordenadas.
    let (wa, _) = alignment_error(&cam, &pose, 0.0, &rep_to_bed(&a));
    assert!(wa < 0.4);
}

/// De la foto a dos alturas a la corrección: lo que usará la aplicación.
#[test]
fn dos_alturas_de_punta_a_punta_corrigen_el_paralaje() {
    let cam = truth_camera();
    let pose = std_pose();
    let layout = MarkLayout::new(BED, 9).unwrap();
    let mut samples = Vec::new();
    for (k, h) in [0.0, 20.0].into_iter().enumerate() {
        let look = Look { height: h, ..LOOK };
        let img = corrected(&cam, &render(&cam, &pose, &layout, look, 70 + k as u64));
        let rep = detect_marks(&img, &layout, &DetectOptions::default()).unwrap();
        assert_eq!(rep.used, 9, "h {h}: {:?}", rep.warnings);
        let (image_px, bed_mm): (Vec<_>, Vec<_>) = rep
            .marks
            .iter()
            .filter(|m| m.used)
            .map(|m| {
                let i = m.image.unwrap();
                (Point::new(i[0], i[1]), Point::new(m.bed[0], m.bed[1]))
            })
            .unzip();
        samples.push(HeightSample {
            height_mm: h,
            image_px,
            bed_mm,
        });
    }
    let fit = fit_height_model(&samples).unwrap();
    // Alturas intermedias (no medidas): 5, 10 y 15 mm.
    let mut worst_corrected: f64 = 0.0;
    let mut worst_plain: f64 = 0.0;
    let at0 = fit.model.image_to_bed_at(0.0).unwrap();
    for h in [0.0, 5.0, 10.0, 15.0, 20.0] {
        let truth = truth_h(&cam, &pose, h);
        let model = fit.model.image_to_bed_at(h).unwrap();
        for j in 0..=8 {
            for i in 0..=12 {
                let bed = Point::new(i as f64 * 25.0, j as f64 * 25.0);
                let px = truth.apply(bed);
                worst_corrected = worst_corrected.max(model.apply(px).dist(bed));
                worst_plain = worst_plain.max(at0.apply(px).dist(bed));
            }
        }
    }
    eprintln!(
        "altura: error máx corregido {worst_corrected:.3} mm; sin corregir (alineación a 0 mm) {worst_plain:.3} mm"
    );
    assert!(worst_corrected < 0.6, "corregido {worst_corrected:.3} mm");
    assert!(worst_plain > 3.0, "sin corregir {worst_plain:.3} mm");
}
