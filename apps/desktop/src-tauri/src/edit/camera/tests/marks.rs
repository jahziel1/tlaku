//! Marcas grabadas y su detección.

use super::*;

// ---- marcas grabadas y altura -------------------------------------------------------

const BED: [f64; 2] = [300.0, 200.0];

#[test]
fn el_documento_de_marcas_tiene_una_capa_line_de_baja_potencia() {
    let doc = build_marks_document(BED, 9).unwrap();
    assert_eq!(doc.version, 2);
    assert_eq!(doc.objects.len(), 9);
    assert_eq!(doc.layers.len(), 1);
    let layer = &doc.layers[0];
    assert_eq!(layer.mode, LayerMode::Line);
    assert!(layer.power <= 15.0 && layer.output);
    for o in &doc.objects {
        assert_eq!(o.layer, layer.color);
        let b = o.bounds().expect("dibuja algo");
        assert!(b.min.x >= 0.0 && b.min.y >= 0.0 && b.max.x <= BED[0] && b.max.y <= BED[1]);
        // El centro de cada marca cae en la rejilla del 7.5 % al 92.5 %.
        let (cx, cy) = ((b.min.x + b.max.x) / 2.0, (b.min.y + b.max.y) / 2.0);
        assert!((22.4..=277.6).contains(&cx) && (14.9..=185.1).contains(&cy));
    }
    // La marca 1 lleva el círculo (3 trazos); las demás, solo la cruz.
    let paths = |i: usize| match &doc.objects[i].kind {
        ObjectKind::Vector(v) => v.paths.len(),
        _ => 0,
    };
    assert_eq!((paths(0), paths(1)), (3, 2));
    // Sobrevive a un viaje por JSON.
    let again = Document::from_json(&doc.to_json().unwrap()).unwrap();
    assert_eq!(again.objects.len(), doc.objects.len());
    assert_eq!(again.layers, doc.layers);
    assert_eq!(build_marks_document(BED, 4).unwrap().objects.len(), 4);
    assert!(build_marks_document(BED, 6).is_err());
    assert!(build_marks_document([20.0, 20.0], 4).is_err());
}

/// Cama a imagen (origen en el borde del píxel) de la cama de prueba, con algo de perspectiva.
fn bed_to_image() -> Homography {
    Homography {
        m: [
            [1.85, 0.06, 40.0],
            [0.03, -1.8, 410.0],
            [2.0e-5, -1.0e-5, 1.0],
        ],
    }
}

fn dist_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    };
    p.dist(Point::new(a.x + t * dx, a.y + t * dy))
}

/// Foto sintética (sin lente) de las marcas del documento, con desenfoque y ruido.
fn render_marks(layout: &MarkLayout, h: &Homography, size: (u32, u32)) -> GrayImage {
    let inv = h.inverse().unwrap();
    let strokes = layout.strokes();
    let mut state = 0x1234_5678_9abc_def1u64;
    let mut noise = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        ((state >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 5.0
    };
    let img = GrayImage::from_fn(size.0, size.1, |i, j| {
        let n = 3;
        let mut dark = 0.0;
        for sj in 0..n {
            for si in 0..n {
                let u = f64::from(i) + (f64::from(si) + 0.5) / f64::from(n);
                let v = f64::from(j) + (f64::from(sj) + 0.5) / f64::from(n);
                let bed = inv.apply(Point::new(u, v));
                let hit = strokes.iter().any(|st| {
                    let m = layout.marks[st.mark];
                    if (bed.x - m.x).abs() > 9.0 || (bed.y - m.y).abs() > 9.0 {
                        return false;
                    }
                    let pts = &st.points;
                    let last = if st.closed { pts.len() } else { pts.len() - 1 };
                    (0..last).any(|k| dist_to_segment(bed, pts[k], pts[(k + 1) % pts.len()]) <= 0.2)
                });
                if hit {
                    dark += 1.0;
                }
            }
        }
        let f = dark / f64::from(n * n);
        image::Luma([(205.0 * (1.0 - f) + 60.0 * f) as u8])
    });
    let blurred = image::imageops::blur(&img, 0.8);
    GrayImage::from_fn(size.0, size.1, |i, j| {
        let v = f64::from(blurred.get_pixel(i, j)[0]) + noise();
        image::Luma([v.clamp(0.0, 255.0) as u8])
    })
}

#[test]
fn detecta_las_marcas_de_un_documento_y_mide_la_precision() {
    let layout = MarkLayout::new(BED, 9).unwrap();
    let truth = bed_to_image();
    let png = to_b64(&render_marks(&layout, &truth, (640, 480)));
    let report = run_detect_marks(&png, BED, 9, None).unwrap();
    assert_eq!((report.found, report.used), (9, 9), "{:?}", report.warnings);
    let to_bed = Homography { m: report.to_bed };
    // Compara con la verdad en el sistema de la alineación (origen en el borde del píxel).
    let mut worst: f64 = 0.0;
    for j in 0..=8 {
        for i in 0..=12 {
            let bed = Point::new(f64::from(i) * 25.0, f64::from(j) * 25.0);
            worst = worst.max(to_bed.apply(truth.apply(bed)).dist(bed));
        }
    }
    assert!(worst < 0.3, "error real máximo {worst:.3} mm");
    assert!(report.loo_rms_mm.unwrap() < 0.3);
    // Con una alineación provisional también se encuentra, y se avisa que se usó.
    let prior = truth.inverse().unwrap();
    let with_prior = run_detect_marks(&png, BED, 9, Some(prior.m)).unwrap();
    assert!(with_prior.used_prior && with_prior.used == 9);
    assert!(run_detect_marks(&png, BED, 5, None).is_err());
    let blank = to_b64(&GrayImage::from_pixel(640, 480, image::Luma([150])));
    assert!(
        run_detect_marks(&blank, BED, 9, None)
            .unwrap_err()
            .contains("marcas")
    );
}
