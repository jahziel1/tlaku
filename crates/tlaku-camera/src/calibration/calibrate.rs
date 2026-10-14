//! Puntos de entrada: calibrar desde imágenes, archivos o esquinas ya detectadas.

use super::fit::{Fit, fit_model};
use super::select::{auto_view_zoom, cross_validate, fmt_score, pick_leader, select_model};
use super::{
    CalibrationOptions, CalibrationReport, LensCalibration, LensModel, LensModelChoice, ModelScore,
};
use crate::chessboard::find_chessboard;
use image::GrayImage;
use std::path::Path;
use tlaku_core::Point;
use tlaku_i18n::tr;

/// Detecta el tablero en cada imagen y calibra. Las imágenes sin tablero se omiten
/// con un aviso; hacen falta al menos 3 vistas válidas.
pub fn calibrate_lens(
    images: &[GrayImage],
    board: [u32; 2],
    square_mm: f64,
) -> Result<CalibrationReport, String> {
    calibrate_lens_with(images, board, square_mm, &CalibrationOptions::default())
}

pub fn calibrate_lens_with(
    images: &[GrayImage],
    board: [u32; 2],
    square_mm: f64,
    opts: &CalibrationOptions,
) -> Result<CalibrationReport, String> {
    let Some(first) = images.first() else {
        return Err(tr!("camera.calibrate.no_images"));
    };
    let size = (first.width(), first.height());
    let mut warnings = Vec::new();
    let mut views: Vec<Vec<Point>> = Vec::new();
    let mut map: Vec<usize> = Vec::new();
    for (i, img) in images.iter().enumerate() {
        if (img.width(), img.height()) != size {
            return Err(tr!(
                "camera.calibrate.size_mismatch",
                n = i + 1,
                w = img.width(),
                h = img.height(),
                first_w = size.0,
                first_h = size.1
            ));
        }
        match find_chessboard(img, board) {
            Ok(pts) => {
                views.push(pts);
                map.push(i);
            }
            Err(e) => warnings.push(tr!("camera.calibrate.image_skipped", n = i + 1, error = e)),
        }
    }
    if views.len() < 3 {
        return Err(tr!(
            "camera.calibrate.few_boards",
            found = views.len(),
            total = images.len()
        ));
    }
    // Los avisos de vistas descartadas usan el número de la imagen original (base 1), no el de
    // la vista detectada: `map` traduce de una a otra.
    let mut report = calibrate_views_inner(&views, board, square_mm, size, opts, Some(&map))?;
    // Reasignar los índices a la lista de imágenes original.
    report.used_views = report.used_views.iter().map(|&k| map[k]).collect();
    warnings.extend(std::mem::take(&mut report.warnings));
    report.warnings = warnings;
    Ok(report)
}

/// Igual que [`calibrate_lens`] pero leyendo las imágenes desde rutas.
pub fn calibrate_lens_files<P: AsRef<Path>>(
    paths: &[P],
    board: [u32; 2],
    square_mm: f64,
) -> Result<CalibrationReport, String> {
    let mut images = Vec::with_capacity(paths.len());
    for p in paths {
        let img = image::open(p.as_ref()).map_err(|e| {
            tr!(
                "camera.calibrate.open_failed",
                path = p.as_ref().display(),
                error = e
            )
        })?;
        images.push(img.to_luma8());
    }
    calibrate_lens(&images, board, square_mm)
}

// ---------------------------------------------------------------------------
// Calibración a partir de esquinas ya detectadas
// ---------------------------------------------------------------------------

/// Calibra con esquinas ya detectadas (`cols * rows` puntos por vista, fila por
/// fila). Descarta vistas atípicas mientras queden al menos 3.
pub fn calibrate_views(
    views: &[Vec<Point>],
    board: [u32; 2],
    square_mm: f64,
    size: (u32, u32),
    opts: &CalibrationOptions,
) -> Result<CalibrationReport, String> {
    calibrate_views_inner(views, board, square_mm, size, opts, None)
}

/// `image_of`: si las vistas salen de una lista de imágenes, a qué imagen corresponde cada vista
/// (los avisos nombran la imagen en lugar de la vista).
fn calibrate_views_inner(
    views: &[Vec<Point>],
    board: [u32; 2],
    square_mm: f64,
    size: (u32, u32),
    opts: &CalibrationOptions,
    image_of: Option<&[usize]>,
) -> Result<CalibrationReport, String> {
    let (cols, rows) = (board[0] as usize, board[1] as usize);
    if views.len() < 3 {
        return Err(tr!("camera.calibrate.few_views", n = views.len()));
    }
    if !(square_mm.is_finite() && square_mm > 0.0) {
        return Err(tr!("camera.calibrate.bad_square"));
    }
    if let Some((i, v)) = views
        .iter()
        .enumerate()
        .find(|(_, v)| v.len() != cols * rows)
    {
        return Err(tr!(
            "camera.calibrate.corner_count",
            n = i + 1,
            found = v.len(),
            expected = cols * rows
        ));
    }
    let obj: Vec<[f64; 2]> = (0..rows)
        .flat_map(|j| (0..cols).map(move |i| [i as f64 * square_mm, j as f64 * square_mm]))
        .collect();

    let mut active: Vec<usize> = (0..views.len()).collect();
    let mut warnings = Vec::new();
    let run = |model: LensModel, act: &[usize]| {
        let refs: Vec<&[Point]> = act.iter().map(|&i| views[i].as_slice()).collect();
        fit_model(model, &refs, &obj, size, opts)
    };
    let models: &[LensModel] = match opts.model {
        LensModelChoice::Auto => &[LensModel::BrownConrady, LensModel::Fisheye],
        LensModelChoice::BrownConrady => &[LensModel::BrownConrady],
        LensModelChoice::Fisheye => &[LensModel::Fisheye],
    };
    // Primer ajuste con todas las vistas; el modelo que mejor ajusta decide qué vistas son atípicas.
    let first: Vec<(LensModel, Result<Fit, String>)> =
        models.iter().map(|&m| (m, run(m, &active))).collect();
    let leader = pick_leader(&first).ok_or_else(|| {
        first
            .iter()
            .find_map(|(_, r)| r.as_ref().err().cloned())
            .unwrap_or_else(|| tr!("camera.calibrate.no_model"))
    })?;
    let mut current = first
        .into_iter()
        .find(|(m, _)| *m == leader)
        .and_then(|(_, r)| r.ok())
        .expect("el líder ajustó");
    while active.len() > 3 {
        let (worst, &worst_rms) = current
            .view_rms
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .expect("hay vistas");
        let mut sorted = current.view_rms.clone();
        sorted.sort_by(f64::total_cmp);
        let median = sorted[sorted.len() / 2];
        if worst_rms <= (2.5 * median).max(0.75) {
            break;
        }
        let mut trial = active.clone();
        trial.remove(worst);
        match run(leader, &trial) {
            Ok(f) if f.rms < 0.6 * current.rms => {
                let (rms, med) = (format!("{worst_rms:.2}"), format!("{median:.2}"));
                warnings.push(match image_of {
                    Some(map) => tr!(
                        "camera.calibrate.image_dropped",
                        n = map[active[worst]] + 1,
                        rms = rms,
                        median = med
                    ),
                    None => tr!(
                        "camera.calibrate.view_dropped",
                        n = active[worst] + 1,
                        rms = rms,
                        median = med
                    ),
                });
                active = trial;
                current = f;
            }
            _ => break,
        }
    }
    if active.len() < 5 {
        warnings.push(tr!("camera.calibrate.few_useful", n = active.len()));
    }

    // Con el conjunto final de vistas se compara cada modelo (ajuste y validación cruzada).
    let refs: Vec<&[Point]> = active.iter().map(|&i| views[i].as_slice()).collect();
    let mut fits: Vec<(LensModel, Fit)> = Vec::new();
    for &m in models {
        if m == leader {
            fits.push((m, current.clone()));
        } else if let Ok(f) = run(m, &active) {
            fits.push((m, f));
        }
    }
    let mut scores: Vec<ModelScore> = fits
        .iter()
        .map(|(m, f)| ModelScore {
            model: *m,
            rms_px: f.rms,
            cv_rms_px: if models.len() > 1 {
                cross_validate(*m, &refs, &obj, size, opts)
            } else {
                None
            },
            chosen: false,
        })
        .collect();
    let chosen = select_model(&scores);
    for sc in &mut scores {
        sc.chosen = sc.model == chosen;
    }
    if models.len() > 1 {
        let brown = scores.iter().find(|s| s.model == LensModel::BrownConrady);
        let fish = scores.iter().find(|s| s.model == LensModel::Fisheye);
        match (brown, fish) {
            (Some(b), Some(f)) if chosen == LensModel::Fisheye => warnings.push(tr!(
                "camera.calibrate.fisheye_chosen",
                fisheye = fmt_score(f.cv_rms_px.unwrap_or(f.rms_px)),
                brown = fmt_score(b.cv_rms_px.unwrap_or(b.rms_px)),
            )),
            (None, Some(_)) => warnings.push(tr!("camera.calibrate.brown_failed")),
            (Some(_), None) => warnings.push(tr!("camera.calibrate.fisheye_failed")),
            _ => {}
        }
    }
    let current = fits
        .into_iter()
        .find(|(m, _)| *m == chosen)
        .map(|(_, f)| f)
        .expect("el modelo elegido ajustó");
    if current.rms > 1.0 {
        warnings.push(tr!(
            "camera.calibrate.high_error",
            rms = format!("{:.2}", current.rms)
        ));
    }
    let p = &current.intr;
    let view_zoom = if chosen == LensModel::Fisheye {
        auto_view_zoom(p, size)
    } else {
        1.0
    };
    let calibration = LensCalibration {
        width: size.0,
        height: size.1,
        fx: p[0],
        fy: p[1],
        cx: p[2],
        cy: p[3],
        k1: p[4],
        k2: p[5],
        p1: p[6],
        p2: p[7],
        k3: p[8],
        model: chosen,
        k4: p[9],
        view_zoom,
        rms_error_px: current.rms,
        views_used: active.len(),
    };
    Ok(CalibrationReport {
        calibration,
        warnings,
        view_rms_px: current.view_rms.clone(),
        used_views: active,
        model_scores: scores,
    })
}
