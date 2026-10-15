//! Alinear la cámara fija con el cabezal, sin grabar ni gastar material: comandos del plan, de la
//! solución (detectar el cabezal en las fotos y ajustar) y del ajuste fino con un punto de
//! referencia. La captura de las fotos y los movimientos los hace quien llama (la interfaz con su
//! cámara, o `hwtest` con la de la máquina): aquí solo se planea y se calcula. Ver
//! `tlaku_camera::head_align`.

use super::*;
use tlaku_camera::{
    DetectFailure, HeadAlignFit, HeadAlignIssue, HeadAlignOptions, HeadAlignPlan,
    HeadDetectOptions, HeadMethod, HeadObservation, Parallax, PlanOptions, detect_heads,
    fit_head_alignment, plan_grid, plan_refine,
};

/// Tope de fotos por solución (la rejilla más fina de 6x6, las rondas y las de referencia).
const MAX_FRAMES: usize = 80;
/// Misma fila: posiciones con la misma Y de la máquina (mm).
const ROW_TOL_MM: f64 = 0.5;
/// Fracción del área segura bajo la cual se avisa que la cámara cubre solo una parte.
const PARTIAL_COVERAGE: f64 = 0.6;

/// Una foto con el cabezal en una posición conocida (mm de documento).
#[derive(Debug, Clone, Deserialize)]
pub struct HeadAlignFrame {
    /// PNG o JPEG en base64, ya corregido de lente si hay calibración.
    pub png_base64: String,
    pub machine_mm: [f64; 2],
}

/// Cómo resolver.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct HeadAlignConfig {
    /// Cama (mm): si falta `safe_area`, el área que debería cubrir la cámara.
    pub bed: [f64; 2],
    /// Área segura recorrida `[x0, y0, x1, y1]` (para avisar si la cámara cubre poco).
    pub safe_area: Option<[f64; 4]>,
    pub detect: HeadDetectOptions,
    /// Calibración con que ya se corrigieron las fotos; `None` = fotos crudas.
    pub lens: Option<LensCalibration>,
    /// Sin calibración, estimar una distorsión aproximada si mejora el ajuste.
    pub estimate_lens: bool,
    pub parallax: Option<Parallax>,
    /// Desfase fijo (mm) entre el punto detectado y el punto del láser en la cama.
    pub offset_mm: [f64; 2],
}

impl Default for HeadAlignConfig {
    fn default() -> Self {
        Self {
            bed: [400.0, 400.0],
            safe_area: None,
            detect: HeadDetectOptions::default(),
            lens: None,
            estimate_lens: true,
            parallax: None,
            offset_mm: [0.0, 0.0],
        }
    }
}

/// Qué pasó con una foto.
#[derive(Debug, Clone, Serialize)]
pub struct HeadFrameReport {
    pub machine_mm: [f64; 2],
    /// Punto del cabezal en la imagen (origen en el borde del píxel, imagen de las fotos).
    pub px: Option<[f64; 2]>,
    /// Por qué no se encontró.
    pub failure: Option<DetectFailure>,
    pub confidence: f64,
    /// Error (mm): validación cruzada si se usó, ajuste final si se descartó.
    pub error_mm: Option<f64>,
    pub used: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HeadAlignReport {
    /// Método con que se localizó el cabezal.
    pub method: HeadMethod,
    pub image_size: [u32; 2],
    pub frames: Vec<HeadFrameReport>,
    /// Posiciones de la máquina donde se vio el cabezal (para afinar la rejilla).
    pub seen_mm: Vec<[f64; 2]>,
    pub detected: usize,
    pub failed: usize,
    /// `None` si no hubo puntos suficientes (ver `fit_error`).
    pub fit: Option<HeadAlignFit>,
    pub fit_error: Option<String>,
    /// Alineación lista para guardar (con `lens` si se estimó una).
    pub alignment: Option<CameraAlignment>,
    /// Lente aproximada estimada: la alineación vale sobre la imagen corregida con ella.
    pub lens: Option<LensCalibration>,
    pub covered_mm: Option<[f64; 4]>,
    pub issues: Vec<HeadAlignIssue>,
}

fn rows(frames: &[HeadAlignFrame]) -> Vec<usize> {
    let mut ys: Vec<f64> = Vec::new();
    frames
        .iter()
        .map(|f| {
            let y = f.machine_mm[1];
            ys.iter()
                .position(|r| (r - y).abs() < ROW_TOL_MM)
                .unwrap_or_else(|| {
                    ys.push(y);
                    ys.len() - 1
                })
        })
        .collect()
}

/// Alineación guardable a partir del ajuste: una muestra a altura 0 con los puntos usados.
fn alignment_from(
    fit: &HeadAlignFit,
    size: [u32; 2],
    lens: Option<&LensCalibration>,
) -> Result<CameraAlignment, String> {
    let used: Vec<_> = fit.points.iter().filter(|p| p.used).collect();
    let mut a = CameraAlignment {
        image_size: size,
        image_px: [[0.0; 2]; 4],
        bed_mm: [[0.0; 2]; 4],
        to_bed: fit.to_bed,
        rms_mm: fit.rms_mm,
        method: AlignmentMethod::HeadSweep,
        reference_height_mm: 0.0,
        samples: vec![AlignmentSample {
            height_mm: 0.0,
            image_px: used.iter().map(|p| p.image_px).collect(),
            bed_mm: used.iter().map(|p| p.bed_mm).collect(),
            loo_rms_mm: Some(fit.loo_rms_mm),
            loo_max_mm: Some(fit.loo_max_mm),
        }],
        height: None,
    };
    normalize_samples(&mut a, lens)?;
    Ok(a)
}

/// Localiza el cabezal en las fotos y ajusta la alineación. Con muy pocos puntos devuelve el
/// informe sin ajuste (para que quien llama pueda pedir una rejilla más fina) en vez de fallar.
pub fn run_head_align_solve(
    frames: &[HeadAlignFrame],
    cfg: &HeadAlignConfig,
) -> Result<HeadAlignReport, String> {
    if frames.is_empty() || frames.len() > MAX_FRAMES {
        return Err(tr!(
            "desktop.edit.camera.head_align.frame_count",
            n = frames.len(),
            max = MAX_FRAMES
        ));
    }
    if frames
        .iter()
        .flat_map(|f| f.machine_mm.iter())
        .any(|v| !v.is_finite())
    {
        return Err(tr!("desktop.edit.camera.head_align.bad_position"));
    }
    if let Some(p) = &cfg.parallax
        && !(p.camera_height_mm.is_finite()
            && p.feature_height_mm.is_finite()
            && p.feature_height_mm >= 0.0
            && p.camera_height_mm > p.feature_height_mm + 5.0)
    {
        return Err(tr!("camera.head_align.fit.bad_parallax"));
    }
    let images = frames
        .iter()
        .map(|f| decode_rgb(&f.png_base64))
        .collect::<Result<Vec<_>, _>>()?;
    let size = [images[0].width(), images[0].height()];
    let outcome = detect_heads(&images, &rows(frames), &cfg.detect)?;
    let obs_idx: Vec<usize> = (0..frames.len())
        .filter(|&i| outcome.results[i].is_ok())
        .collect();
    let obs: Vec<HeadObservation> = obs_idx
        .iter()
        .map(|&i| HeadObservation {
            machine_mm: frames[i].machine_mm,
            px: outcome.results[i].as_ref().map_or([0.0; 2], |d| d.px),
        })
        .collect();
    let seen_mm: Vec<[f64; 2]> = obs.iter().map(|o| o.machine_mm).collect();
    let opts = HeadAlignOptions {
        image_size: size,
        lens: cfg.lens.clone(),
        estimate_lens: cfg.estimate_lens,
        parallax: cfg.parallax,
        offset_mm: cfg.offset_mm,
        ..HeadAlignOptions::default()
    };
    let (fit, fit_error) = match fit_head_alignment(&obs, &opts) {
        Ok(f) => (Some(f), None),
        Err(e) => (None, Some(e)),
    };
    // Informe por foto: los índices del ajuste siguen el orden de las detectadas.
    let mut frame_reports: Vec<HeadFrameReport> = Vec::with_capacity(frames.len());
    let mut k = 0;
    for (i, f) in frames.iter().enumerate() {
        let r = match &outcome.results[i] {
            Ok(d) => {
                let point = fit.as_ref().map(|fit| &fit.points[k]);
                k += 1;
                HeadFrameReport {
                    machine_mm: f.machine_mm,
                    px: Some([d.px[0] + 0.5, d.px[1] + 0.5]),
                    failure: None,
                    confidence: d.confidence,
                    error_mm: point.map(|p| p.error_mm),
                    used: point.is_some_and(|p| p.used),
                }
            }
            Err(e) => HeadFrameReport {
                machine_mm: f.machine_mm,
                px: None,
                failure: Some(*e),
                confidence: 0.0,
                error_mm: None,
                used: false,
            },
        };
        frame_reports.push(r);
    }
    let failed = frames.len() - obs.len();
    let mut issues = Vec::new();
    if failed > 0 {
        issues.push(HeadAlignIssue::NotDetected { count: failed });
    }
    let area = cfg
        .safe_area
        .unwrap_or([0.0, 0.0, cfg.bed[0], cfg.bed[1]]);
    if let Some(f) = &fit {
        issues.extend(f.issues.iter().cloned());
        let (cw, ch) = (f.covered_mm[2] - f.covered_mm[0], f.covered_mm[3] - f.covered_mm[1]);
        let full = (area[2] - area[0]).max(1.0) * (area[3] - area[1]).max(1.0);
        if cw * ch < PARTIAL_COVERAGE * full {
            issues.push(HeadAlignIssue::PartialCoverage {
                covered_mm: f.covered_mm,
                area_mm: area,
            });
        }
    }
    let lens_for_alignment = fit
        .as_ref()
        .and_then(|f| f.lens.as_ref())
        .or(cfg.lens.as_ref());
    let alignment = match &fit {
        Some(f) => Some(alignment_from(f, size, lens_for_alignment)?),
        None => None,
    };
    Ok(HeadAlignReport {
        method: outcome.method,
        image_size: size,
        frames: frame_reports,
        seen_mm,
        detected: obs.len(),
        failed,
        lens: fit.as_ref().and_then(|f| f.lens.clone()),
        covered_mm: fit.as_ref().map(|f| f.covered_mm),
        fit,
        fit_error,
        alignment,
        issues,
    })
}

/// Mueve la alineación guardada para que el píxel `ref_px` (imagen corregida, origen en el borde
/// del píxel) caiga en `ref_bed`: calcula el desfase con un único punto de referencia conocido
/// (p. ej. un disparo de prueba visible que el usuario marca en la foto). Devuelve la alineación
/// nueva y el desplazamiento aplicado (mm).
pub fn run_head_align_shift(
    alignment: CameraAlignment,
    lens: Option<LensCalibration>,
    ref_px: [f64; 2],
    ref_bed: [f64; 2],
) -> Result<(CameraAlignment, [f64; 2]), String> {
    if ref_px.iter().chain(&ref_bed).any(|v| !v.is_finite()) {
        return Err(tr!("desktop.edit.camera.head_align.bad_position"));
    }
    let mut a = alignment;
    let at = Homography { m: a.to_bed }.apply(pt(ref_px));
    let delta = [ref_bed[0] - at.x, ref_bed[1] - at.y];
    if !delta.iter().all(|v| v.is_finite()) {
        return Err(tr!("desktop.edit.camera.head_align.bad_position"));
    }
    if a.samples.is_empty() {
        // Alineación manual o antigua sin muestras: se mueven los 4 puntos de la cama.
        for p in &mut a.bed_mm {
            *p = [p[0] + delta[0], p[1] + delta[1]];
        }
        normalize_manual(&mut a)?;
    } else {
        for s in &mut a.samples {
            for p in &mut s.bed_mm {
                *p = [p[0] + delta[0], p[1] + delta[1]];
            }
        }
        normalize_samples(&mut a, lens.as_ref())?;
    }
    Ok((a, delta))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn camera_head_align_plan(options: PlanOptions) -> Result<HeadAlignPlan, String> {
    plan_grid(&options)
}

/// Nueva rejilla alrededor de donde ya se vio el cabezal; `None` si no hay nada que afinar.
#[tauri::command(rename_all = "snake_case")]
pub async fn camera_head_align_refine(
    area: [f64; 4],
    seen: Vec<[f64; 2]>,
    grid: usize,
) -> Result<Option<HeadAlignPlan>, String> {
    Ok(plan_refine(area, &seen, grid, 0.5, 60.0))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn camera_head_align_solve(
    frames: Vec<HeadAlignFrame>,
    config: HeadAlignConfig,
) -> Result<HeadAlignReport, String> {
    tauri::async_runtime::spawn_blocking(move || run_head_align_solve(&frames, &config))
        .await
        .map_err(crate::edit::task_interrupted)?
}

#[derive(Debug, Clone, Serialize)]
pub struct HeadAlignShift {
    pub alignment: CameraAlignment,
    pub shift_mm: [f64; 2],
}

#[tauri::command(rename_all = "snake_case")]
pub async fn camera_head_align_shift(
    alignment: CameraAlignment,
    lens: Option<LensCalibration>,
    ref_px: [f64; 2],
    ref_bed: [f64; 2],
) -> Result<HeadAlignShift, String> {
    tauri::async_runtime::spawn_blocking(move || {
        run_head_align_shift(alignment, lens, ref_px, ref_bed)
            .map(|(alignment, shift_mm)| HeadAlignShift { alignment, shift_mm })
    })
    .await
    .map_err(crate::edit::task_interrupted)?
}
