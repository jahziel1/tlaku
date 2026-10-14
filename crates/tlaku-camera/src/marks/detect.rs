//! Detección completa de marcas: candidatos, asignación, refinamiento y homografía.

use super::assign::{Assignment, assign_blind, assign_with_prior, local_scale};
use super::candidates::{Grad, find_candidates};
use super::refine::refine_cross;
use super::{DetectOptions, MarkLayout, MarkResult, MarksReport, RESIDUAL_WARN_MM};
use crate::homography::{Homography, ImageToBed, RansacOptions, estimate_homography};
use crate::imgproc::{FloatImage, gaussian_blur};
use image::GrayImage;
use tlaku_core::Point;

// ---------------------------------------------------------------------------
// Detección completa
// ---------------------------------------------------------------------------

fn offset(p: Point, d: f64) -> Point {
    Point::new(p.x + d, p.y + d)
}

/// Detecta las marcas de `layout` en `img` (ya corregida de lente), asigna y ajusta la
/// homografía imagen a cama, y mide la precisión por validación cruzada.
pub fn detect_marks(
    img: &GrayImage,
    layout: &MarkLayout,
    opts: &DetectOptions,
) -> Result<MarksReport, String> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    if w < 64 || h < 64 {
        return Err("la imagen es demasiado pequeña para buscar marcas".into());
    }
    let m = layout.marks.len();
    let off = opts.pixel_offset;
    let raw = FloatImage::from_gray(img);
    let blurred = gaussian_blur(&raw, 1.0);
    let grad = Grad::new(&blurred);
    let cands = find_candidates(&blurred, &grad, 2 * m + 6);
    if cands.len() < 4 {
        return Err(format!(
            "no se encontraron suficientes marcas en la imagen (se vieron {} y hacen falta al menos 4); \
             revisa la iluminación, el enfoque y que las marcas estén dentro del encuadre",
            cands.len()
        ));
    }

    // Asignación.
    let mut used_prior = false;
    let mut assign: Option<Assignment> = None;
    if let Some(prior) = &opts.prior_to_bed
        && let Ok(inv) = prior.inverse()
    {
        // La alineación entra en las coordenadas de salida; el detector trabaja en centros.
        let bed_to_img = Homography::from_matrix(
            &(nalgebra::Matrix3::new(1.0, 0.0, -off, 0.0, 1.0, -off, 0.0, 0.0, 1.0) * inv.matrix()),
        );
        let radius = 0.07 * w.max(h) as f64;
        let a = assign_with_prior(layout, &cands, &bed_to_img, radius);
        if a.iter().flatten().count() >= 4 {
            used_prior = true;
            assign = Some(a);
        }
    }
    let assign = match assign {
        Some(a) => a,
        None => assign_blind(layout, &cands, &blurred, (w, h)).ok_or(
            "no se pudo reconocer la disposición de las marcas (¿son de otro documento, o la \
             imagen está espejada?); con una alineación manual de 4 puntos previa la búsqueda \
             es más fiable",
        )?,
    };
    let mut warnings = Vec::new();

    // Homografía inicial con los candidatos asignados, para fijar el tamaño de la ventana.
    let init: Vec<(usize, Point)> = assign
        .iter()
        .enumerate()
        .filter_map(|(j, a)| a.map(|i| (j, cands[i].p)))
        .collect();
    let init_pairs: Vec<(Point, Point)> = init.iter().map(|&(j, p)| (p, layout.marks[j])).collect();
    let init_h = estimate_homography(&init_pairs)
        .map_err(|e| format!("las marcas encontradas no definen una alineación: {e}"))?;

    // Refinamiento de cada marca: se ajustan las rectas de sus dos brazos y se intersectan.
    // Las direcciones y el tamaño en píxeles salen de la alineación inicial.
    let init_bed_to_img = init_h.inverse()?;
    let mut refined: Vec<Option<(Point, f64)>> = vec![None; m];
    for &(j, p0) in &init {
        let bed = layout.marks[j];
        let c = init_bed_to_img.apply(bed);
        let unit = |dx: f64, dy: f64| {
            let q = init_bed_to_img.apply(Point::new(bed.x + dx, bed.y + dy));
            let (vx, vy) = (q.x - c.x, q.y - c.y);
            let len = vx.hypot(vy);
            ((vx / len, vy / len), len)
        };
        let (u1, s1) = unit(1.0, 0.0);
        let (u2, _) = unit(0.0, 1.0);
        if !(s1.is_finite() && s1 > 0.0 && u1.0.is_finite() && u2.0.is_finite()) {
            continue;
        }
        let arm_px = layout.arm_mm * s1;
        let sym = cands[assign[j].unwrap_or(0)].sym;
        if let Some((p, rms)) = refine_cross(&blurred, p0, u1, u2, arm_px)
            && rms <= 0.8
        {
            refined[j] = Some((p, sym));
        }
    }
    let refined_count = refined.iter().flatten().count();
    if refined_count < 4 {
        return Err(format!(
            "solo se pudo medir con precisión el centro de {refined_count} marcas (hacen falta 4); \
             las líneas grabadas son muy tenues o están desenfocadas: sube un poco la potencia o enfoca"
        ));
    }

    // Ajuste con RANSAC sobre las posiciones refinadas (en coordenadas de salida).
    let present: Vec<usize> = (0..m).filter(|&j| refined[j].is_some()).collect();
    let image_pts: Vec<Point> = present
        .iter()
        .map(|&j| offset(refined[j].map(|r| r.0).unwrap_or(layout.marks[j]), off))
        .collect();
    let bed_pts: Vec<Point> = present.iter().map(|&j| layout.marks[j]).collect();
    let ransac = RansacOptions {
        threshold: opts.ransac_threshold_mm,
        ..RansacOptions::default()
    };
    let fit = ImageToBed::fit(&image_pts, &bed_pts, Some(&ransac))?;
    let to_bed = fit.alignment.to_bed;
    let mut used_flags = vec![false; m];
    for (k, &j) in present.iter().enumerate() {
        used_flags[j] = fit.inliers[k];
    }
    let used_count = used_flags.iter().filter(|&&u| u).count();
    if used_count < 4 {
        return Err(
            "las marcas no son coherentes entre sí; revisa que se hayan grabado completas".into(),
        );
    }

    // Residuos y validación cruzada.
    let mut results: Vec<MarkResult> = (0..m)
        .map(|j| MarkResult {
            index: j,
            bed: [layout.marks[j].x, layout.marks[j].y],
            image: None,
            confidence: 0.0,
            used: used_flags[j],
            residual_mm: None,
            loo_mm: None,
        })
        .collect();
    for (k, &j) in present.iter().enumerate() {
        let r = &mut results[j];
        r.image = Some([image_pts[k].x, image_pts[k].y]);
        r.confidence = refined[j].map_or(0.0, |x| x.1);
        r.residual_mm = Some(to_bed.apply(image_pts[k]).dist(bed_pts[k]));
    }
    let used_idx: Vec<usize> = (0..present.len())
        .filter(|&k| used_flags[present[k]])
        .collect();
    let mut loo_errs = Vec::new();
    if used_idx.len() >= 5 {
        for &k in &used_idx {
            let rest: Vec<(Point, Point)> = used_idx
                .iter()
                .filter(|&&q| q != k)
                .map(|&q| (image_pts[q], bed_pts[q]))
                .collect();
            if let Ok(hh) = estimate_homography(&rest) {
                let e = hh.apply(image_pts[k]).dist(bed_pts[k]);
                results[present[k]].loo_mm = Some(e);
                loo_errs.push(e);
            }
        }
    }
    let residuals: Vec<f64> = used_idx
        .iter()
        .map(|&k| to_bed.apply(image_pts[k]).dist(bed_pts[k]))
        .collect();
    let rms_mm = (residuals.iter().map(|e| e * e).sum::<f64>() / residuals.len() as f64).sqrt();
    let max_residual_mm = residuals.iter().copied().fold(0.0, f64::max);
    let (loo_rms_mm, loo_max_mm) = if loo_errs.is_empty() {
        (None, None)
    } else {
        (
            Some((loo_errs.iter().map(|e| e * e).sum::<f64>() / loo_errs.len() as f64).sqrt()),
            Some(loo_errs.iter().copied().fold(0.0, f64::max)),
        )
    };

    let found = present.len();
    if found < m {
        warnings.push(format!("Se encontraron {found} de {m} marcas."));
    }
    if used_count < found {
        warnings.push(format!(
            "{} marca(s) se descartaron por no concordar con las demás.",
            found - used_count
        ));
    }
    let judge = loo_max_mm.unwrap_or(max_residual_mm);
    if judge > RESIDUAL_WARN_MM {
        warnings.push(format!(
            "Hay un error de {judge:.2} mm en alguna marca (se recomienda menos de {RESIDUAL_WARN_MM} mm)."
        ));
    }

    let center = Point::new(layout.bed_mm[0] / 2.0, layout.bed_mm[1] / 2.0);
    let scale_px_per_mm = local_scale(&fit.alignment.to_image, center);
    Ok(MarksReport {
        marks: results,
        candidates: cands
            .iter()
            .map(|c| {
                let p = offset(c.p, off);
                [p.x, p.y]
            })
            .collect(),
        to_bed: to_bed.m,
        expected: m,
        found,
        used: used_count,
        rms_mm,
        max_residual_mm,
        loo_rms_mm,
        loo_max_mm,
        scale_px_per_mm,
        used_prior,
        warnings,
    })
}
