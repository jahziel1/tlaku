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
        return Err("no se indicó ninguna imagen para calibrar".into());
    };
    let size = (first.width(), first.height());
    let mut warnings = Vec::new();
    let mut views: Vec<Vec<Point>> = Vec::new();
    let mut map: Vec<usize> = Vec::new();
    for (i, img) in images.iter().enumerate() {
        if (img.width(), img.height()) != size {
            return Err(format!(
                "la imagen {} mide {}x{} y la primera {}x{}; todas deben tener el mismo tamaño",
                i + 1,
                img.width(),
                img.height(),
                size.0,
                size.1
            ));
        }
        match find_chessboard(img, board) {
            Ok(pts) => {
                views.push(pts);
                map.push(i);
            }
            Err(e) => warnings.push(format!("imagen {}: {e}; se omite", i + 1)),
        }
    }
    if views.len() < 3 {
        return Err(format!(
            "solo {} de {} imágenes tienen el tablero visible; se necesitan al menos 3 (ideal 5 o más, con el tablero inclinado y en las esquinas)",
            views.len(),
            images.len()
        ));
    }
    let mut report = calibrate_views(&views, board, square_mm, size, opts)?;
    // Reasignar los índices a la lista de imágenes original.
    report.used_views = report.used_views.iter().map(|&k| map[k]).collect();
    for w in &mut report.warnings {
        for (k, &orig) in map.iter().enumerate().rev() {
            // Los avisos de vistas descartadas dicen "vista N " (base 1, sobre las
            // vistas detectadas); se reescriben con el número de imagen original.
            *w = w.replace(
                &format!("vista {} ", k + 1),
                &format!("imagen {} ", orig + 1),
            );
        }
    }
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
        let img = image::open(p.as_ref())
            .map_err(|e| format!("no se pudo abrir {}: {e}", p.as_ref().display()))?;
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
    let (cols, rows) = (board[0] as usize, board[1] as usize);
    if views.len() < 3 {
        return Err(format!(
            "se necesitan al menos 3 vistas del tablero (hay {})",
            views.len()
        ));
    }
    if !(square_mm.is_finite() && square_mm > 0.0) {
        return Err("el tamaño del cuadro debe ser mayor que 0 mm".into());
    }
    if let Some((i, v)) = views
        .iter()
        .enumerate()
        .find(|(_, v)| v.len() != cols * rows)
    {
        return Err(format!(
            "la vista {} tiene {} esquinas y se esperaban {}",
            i + 1,
            v.len(),
            cols * rows
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
            .unwrap_or_else(|| "no se pudo ajustar ningún modelo de lente".into())
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
                warnings.push(format!(
                    "vista {} descartada: error de reproyección {:.2} px frente a una mediana de {:.2} px (¿tablero movido, desenfocado o mal detectado?)",
                    active[worst] + 1, worst_rms, median
                ));
                active = trial;
                current = f;
            }
            _ => break,
        }
    }
    if active.len() < 5 {
        warnings.push(format!(
            "solo {} vistas útiles; con 5 o más (tablero inclinado y cerca de las esquinas) la calibración es más fiable",
            active.len()
        ));
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
            (Some(b), Some(f)) if chosen == LensModel::Fisheye => warnings.push(format!(
                "se eligió el modelo de ojo de pez: error en vistas reservadas {} px frente a {} px con Brown-Conrady",
                fmt_score(f.cv_rms_px.unwrap_or(f.rms_px)),
                fmt_score(b.cv_rms_px.unwrap_or(b.rms_px)),
            )),
            (None, Some(_)) => warnings.push(
                "Brown-Conrady no logró ajustar estas vistas; se usa el modelo de ojo de pez".into(),
            ),
            (Some(_), None) => warnings.push(
                "el modelo de ojo de pez no logró ajustar estas vistas; se usa Brown-Conrady".into(),
            ),
            _ => {}
        }
    }
    let current = fits
        .into_iter()
        .find(|(m, _)| *m == chosen)
        .map(|(_, f)| f)
        .expect("el modelo elegido ajustó");
    if current.rms > 1.0 {
        warnings.push(format!(
            "el error de reproyección es alto ({:.2} px); revisa el enfoque, que el tablero esté plano y el tamaño del cuadro",
            current.rms
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
