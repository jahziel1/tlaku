//! Validación y normalización de la configuración (`normalize_setup`) y el modelo de altura.

use super::*;

pub(super) fn pt(p: [f64; 2]) -> Point {
    Point::new(p[0], p[1])
}

/// Máximo de alturas distintas y de marcas por altura que se guardan.
pub(super) const MAX_SAMPLES: usize = 6;
pub(super) const MAX_POINTS: usize = 100;
/// Alturas dentro de este margen (mm) cuentan como la misma: la captura nueva reemplaza a la vieja.
pub(super) const SAME_HEIGHT_MM: f64 = 0.25;

/// Matriz de la cámara de la imagen corregida, `[fx, fy, cx, cy]`, en las coordenadas de la
/// alineación (origen en el borde del píxel, así que el punto principal suma 0.5 antes de
/// escalar). Es la misma que usa el corrector de la interfaz.
pub(super) fn corrected_camera(c: &LensCalibration, size: [u32; 2]) -> [f64; 4] {
    let sx = f64::from(size[0]) / f64::from(c.width);
    let sy = f64::from(size[1]) / f64::from(c.height);
    // La imagen corregida usa la focal por `view_zoom` (1 salvo en ojo de pez).
    let (zfx, zfy) = c.corrected_focal();
    [zfx * sx, zfy * sy, sx * (c.cx + 0.5), sy * (c.cy + 0.5)]
}

/// Modelo de altura de respaldo con una sola alineación y la matriz de la cámara.
pub(super) fn modelled_height(
    to_bed: &Homography,
    reference_mm: f64,
    calib: Option<&LensCalibration>,
    size: [u32; 2],
) -> Option<HeightModel> {
    let bed_to_image = to_bed.inverse().ok()?;
    let model =
        HeightModel::from_intrinsics(&bed_to_image, reference_mm, corrected_camera(calib?, size))
            .ok()?;
    let finite = model.gradient.iter().all(|v| v.is_finite())
        && model.bed_to_image.iter().flatten().all(|v| v.is_finite());
    finite.then_some(model)
}

pub(super) fn points(v: &[[f64; 2]]) -> Vec<Point> {
    v.iter().copied().map(pt).collect()
}

/// Alineación manual: la homografía exacta de los 4 puntos.
pub(super) fn normalize_manual(a: &mut CameraAlignment) -> Result<(), String> {
    let img: Vec<Point> = a.image_px.iter().copied().map(pt).collect();
    let bed: Vec<Point> = a.bed_mm.iter().copied().map(pt).collect();
    let fit = ImageToBed::fit(&img, &bed, None)
        .map_err(|e| tr!("desktop.edit.camera.align.bad_four_points", err = e))?;
    a.to_bed = fit.alignment.to_bed.m;
    a.rms_mm = fit.rms_mm;
    // Una homografía con valores no finitos (puntos casi alineados) no sirve.
    if a.to_bed.iter().flatten().any(|v| !v.is_finite()) {
        return Err(tr!("desktop.edit.camera.align.collinear"));
    }
    a.method = AlignmentMethod::Manual;
    Ok(())
}

/// Alineación con marcas: ajusta a una altura (homografía) o a varias (modelo de altura).
pub(super) fn normalize_samples(
    a: &mut CameraAlignment,
    calib: Option<&LensCalibration>,
) -> Result<(), String> {
    for s in &a.samples {
        if !s.height_mm.is_finite() || !(-10.0..=500.0).contains(&s.height_mm) {
            return Err(tr!("desktop.edit.camera.align.height_range"));
        }
        if s.image_px.len() != s.bed_mm.len() {
            return Err(tr!("desktop.edit.camera.align.point_count_mismatch"));
        }
        if s.image_px.len() < 4 || s.image_px.len() > MAX_POINTS {
            return Err(tr!(
                "desktop.edit.camera.align.marks_per_height",
                max = MAX_POINTS,
                height = format!("{:.1}", s.height_mm),
                n = s.image_px.len()
            ));
        }
        if s.image_px
            .iter()
            .chain(&s.bed_mm)
            .flatten()
            .any(|v| !v.is_finite())
        {
            return Err(tr!("desktop.edit.camera.align.marks_not_finite"));
        }
    }
    // Ordenadas por altura; una captura a la misma altura reemplaza a la anterior.
    let mut sorted: Vec<AlignmentSample> = Vec::new();
    let mut input = std::mem::take(&mut a.samples);
    input.sort_by(|x, y| x.height_mm.total_cmp(&y.height_mm));
    for s in input {
        match sorted.last_mut() {
            Some(last) if (s.height_mm - last.height_mm).abs() < SAME_HEIGHT_MM => *last = s,
            _ => sorted.push(s),
        }
    }
    if sorted.len() > MAX_SAMPLES {
        return Err(tr!(
            "desktop.edit.camera.align.max_heights",
            max = MAX_SAMPLES
        ));
    }
    let lowest = sorted[0].height_mm;
    a.reference_height_mm = lowest;
    // La alineación con el cabezal usa el mismo ajuste que las marcas y conserva su origen.
    if a.method != AlignmentMethod::HeadSweep {
        a.method = AlignmentMethod::Marks;
    }
    if sorted.len() == 1 {
        let s = &sorted[0];
        let fit = ImageToBed::fit(&points(&s.image_px), &points(&s.bed_mm), None)
            .map_err(|e| tr!("desktop.edit.camera.align.bad_marks", err = e))?;
        a.to_bed = fit.alignment.to_bed.m;
        a.rms_mm = fit.rms_mm;
        a.height = modelled_height(&fit.alignment.to_bed, lowest, calib, a.image_size);
    } else {
        let samples: Vec<HeightSample> = sorted
            .iter()
            .map(|s| HeightSample {
                height_mm: s.height_mm,
                image_px: points(&s.image_px),
                bed_mm: points(&s.bed_mm),
            })
            .collect();
        let fit = fit_height_model(&samples)
            .map_err(|e| tr!("desktop.edit.camera.align.bad_height_model", err = e))?;
        let to_bed = fit.model.image_to_bed_at(lowest)?;
        a.to_bed = to_bed.m;
        a.rms_mm = fit.sample_rms_mm.iter().sum::<f64>() / fit.sample_rms_mm.len() as f64;
        a.height = Some(fit.model);
    }
    if a.to_bed.iter().flatten().any(|v| !v.is_finite()) || !a.rms_mm.is_finite() {
        return Err(tr!("desktop.edit.camera.align.marks_degenerate"));
    }
    // Las 4 esquinas del rectángulo de las marcas, proyectadas, para mostrarlas y para que las
    // versiones sin marcas sigan viendo una alineación de 4 puntos.
    let bed_pts = points(&sorted[0].bed_mm);
    let (x0, x1) = bed_pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
        (lo.min(p.x), hi.max(p.x))
    });
    let (y0, y1) = bed_pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
        (lo.min(p.y), hi.max(p.y))
    });
    let to_image = Homography { m: a.to_bed }.inverse()?;
    let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
    for (i, (x, y)) in corners.into_iter().enumerate() {
        let q = to_image.apply(Point::new(x, y));
        a.bed_mm[i] = [x, y];
        a.image_px[i] = [q.x, q.y];
    }
    a.samples = sorted;
    Ok(())
}

/// Máximo de configuraciones de cámara con nombre.
pub(super) const MAX_PRESETS: usize = 32;
/// Largo máximo del nombre de una configuración y de una URL de cámara.
const MAX_PRESET_NAME: usize = 60;
const MAX_URL_LEN: usize = 2048;
/// Tope del desplazamiento manual del fondo (mm) y de la resolución manual (puntos por mm).
const MAX_OFFSET_MM: f64 = 1000.0;
const MAX_DPM: f64 = 10_000.0;

/// URL de la cámara de red: sin espacios, `http://` o `https://` y de largo razonable.
fn check_url(url: Option<String>) -> Result<Option<String>, String> {
    let Some(url) = url.map(|u| u.trim().to_string()).filter(|u| !u.is_empty()) else {
        return Ok(None);
    };
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err(tr!("desktop.edit.camera.align.url_scheme"));
    }
    if url.len() > MAX_URL_LEN || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(tr!("desktop.edit.camera.align.url_invalid"));
    }
    Ok(Some(url))
}

fn clean_dpm(v: Option<f64>) -> Option<f64> {
    v.filter(|d| d.is_finite() && *d > 0.0 && *d <= MAX_DPM)
}

fn clean_offset(v: f64) -> f64 {
    if v.is_finite() {
        v.clamp(-MAX_OFFSET_MM, MAX_OFFSET_MM)
    } else {
        0.0
    }
}

/// Ajustes de fuente y de fondo de las preferencias, y las configuraciones con nombre.
fn normalize_sources(setup: &mut CameraSetup) -> Result<(), String> {
    let prefs = &mut setup.prefs;
    prefs.network_url = check_url(prefs.network_url.take())?;
    prefs.dpm = clean_dpm(prefs.dpm);
    prefs.offset_x_mm = clean_offset(prefs.offset_x_mm);
    prefs.offset_y_mm = clean_offset(prefs.offset_y_mm);
    // Última con el mismo nombre (sin distinguir mayúsculas) gana, conservando su lugar.
    let mut out: Vec<CameraPreset> = Vec::new();
    for mut p in std::mem::take(&mut setup.presets) {
        p.name = p.name.trim().to_string();
        if p.name.is_empty() || p.name.chars().count() > MAX_PRESET_NAME {
            return Err(tr!(
                "desktop.edit.camera.align.preset_name_len",
                max = MAX_PRESET_NAME
            ));
        }
        p.network_url = check_url(p.network_url.take())?;
        p.dpm = clean_dpm(p.dpm);
        p.offset_x_mm = clean_offset(p.offset_x_mm);
        p.offset_y_mm = clean_offset(p.offset_y_mm);
        p.width = p.width.filter(|v| *v > 0);
        p.height = p.height.filter(|v| *v > 0);
        match out
            .iter_mut()
            .find(|q| q.name.to_lowercase() == p.name.to_lowercase())
        {
            Some(q) => *q = p,
            None => out.push(p),
        }
    }
    if out.len() > MAX_PRESETS {
        return Err(tr!(
            "desktop.edit.camera.align.max_presets",
            max = MAX_PRESETS
        ));
    }
    setup.presets = out;
    Ok(())
}

/// Valida la configuración y recalcula la homografía de la alineación con tlaku-camera.
pub fn normalize_setup(mut setup: CameraSetup) -> Result<CameraSetup, String> {
    setup.version = SETUP_VERSION;
    if let Some(c) = &setup.calibration {
        c.validate()?;
    }
    check_head(&setup.head)?;
    let calib = setup.calibration.clone();
    if let Some(a) = &mut setup.alignment {
        if a.image_size[0] == 0 || a.image_size[1] == 0 {
            return Err(tr!("desktop.edit.camera.align.zero_size"));
        }
        if !a.reference_height_mm.is_finite() {
            a.reference_height_mm = 0.0;
        }
        if a.samples.is_empty() {
            let all = a.image_px.iter().chain(a.bed_mm.iter()).flatten();
            if all.clone().any(|v| !v.is_finite()) {
                return Err(tr!("desktop.edit.camera.align.not_finite"));
            }
            normalize_manual(a)?;
            a.height = modelled_height(
                &Homography { m: a.to_bed },
                a.reference_height_mm,
                calib.as_ref(),
                a.image_size,
            );
        } else {
            normalize_samples(a, calib.as_ref())?;
        }
    }
    if !(0.0..=1.0).contains(&setup.prefs.opacity) {
        setup.prefs.opacity = setup.prefs.opacity.clamp(0.0, 1.0);
    }
    let h = setup.prefs.material_height_mm;
    setup.prefs.material_height_mm = if h.is_finite() {
        h.clamp(0.0, 300.0)
    } else {
        0.0
    };
    normalize_sources(&mut setup)?;
    Ok(setup)
}
