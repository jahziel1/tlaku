//! Vista previa «por tonos» (mejora 8): `tonal_preview` devuelve la imagen PNG de cuánto se
//! oscurecería el material con el trabajo, sobre `tlaku_pipeline::render_tonal`.
//!
//! La simulación es una aproximación (ver la cabecera de `tlaku_pipeline::tonal`). El trabajo se
//! calcula igual que `compute_job` (misma posición de cabeza, mismas opciones) y el último
//! resultado se guarda: pedir otra zona o otra curva del mismo trabajo (acercar la vista, cambiar
//! de material) no lo recalcula. Solo guarda el último, para no retener trabajos grandes.

use crate::commands::{AppState, blocking};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use tauri::State;
use tlaku_doc::{DeviceProfile, Document, JobOptions, ToneCurve};
use tlaku_i18n::tr;
use tlaku_pipeline::{ComputedJob, TonalOptions, render_tonal};

/// Qué simular.
#[derive(Debug, Clone, Deserialize)]
pub struct TonalRequest {
    pub curve: ToneCurve,
    /// Ancho del punto del láser, mm.
    pub spot_mm: f64,
    /// Zona a dibujar `[x0, y0, x1, y1]` (mm del documento); `null` = toda la hoja.
    pub region_mm: Option<[f64; 4]>,
    /// Lado mayor de la imagen, en píxeles.
    pub max_px: u32,
}

/// La imagen lista para mostrar.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TonalPreview {
    /// PNG RGBA en base64 (sin prefijo `data:`).
    pub png_base64: String,
    pub width_px: u32,
    pub height_px: u32,
    /// Rectángulo que cubre `[x0, y0, x1, y1]`, mm del documento (Y hacia arriba).
    pub covered_mm: [f64; 4],
    /// Hoja de material supuesta `[x0, y0, x1, y1]`, mm.
    pub sheet_mm: [f64; 4],
    pub px_mm: f64,
    pub max_dose: f64,
}

type Cached = Option<(u64, Arc<ComputedJob>)>;
static LAST_JOB: Mutex<Cached> = Mutex::new(None);

fn key(
    doc: &Document,
    device: &DeviceProfile,
    options: &JobOptions,
    head: Option<[f64; 2]>,
) -> Option<u64> {
    let bytes = serde_json::to_vec(&(doc, device, options, head)).ok()?;
    let mut h = DefaultHasher::new();
    bytes.hash(&mut h);
    Some(h.finish())
}

fn job_for(
    doc: &Document,
    device: &DeviceProfile,
    options: &JobOptions,
    head: Option<[f64; 2]>,
) -> Result<Arc<ComputedJob>, String> {
    let k = key(doc, device, options, head);
    if let Some(k) = k
        && let Ok(g) = LAST_JOB.lock()
        && let Some((have, job)) = g.as_ref()
        && *have == k
    {
        return Ok(Arc::clone(job));
    }
    let point = head.map(|[x, y]| tlaku_core::Point::new(x, y));
    let job = Arc::new(tlaku_pipeline::compute_at(doc, device, options, point)?);
    if let (Some(k), Ok(mut g)) = (k, LAST_JOB.lock()) {
        *g = Some((k, Arc::clone(&job)));
    }
    Ok(job)
}

/// Calcula (o reutiliza) el trabajo y lo simula.
pub fn run_tonal(
    doc: &Document,
    device: &DeviceProfile,
    options: &JobOptions,
    head: Option<[f64; 2]>,
    request: &TonalRequest,
) -> Result<TonalPreview, String> {
    let job = job_for(doc, device, options, head)?;
    let image = render_tonal(
        &job,
        device,
        &TonalOptions {
            curve: request.curve,
            spot_mm: request.spot_mm,
            region_mm: request.region_mm,
            max_px: request.max_px,
        },
    )?;
    Ok(TonalPreview {
        png_base64: STANDARD.encode(&image.png),
        width_px: image.size_px[0],
        height_px: image.size_px[1],
        covered_mm: image.covered_mm,
        sheet_mm: image.sheet_mm,
        px_mm: image.px_mm,
        max_dose: image.max_dose,
    })
}

#[tauri::command]
pub async fn tonal_preview(
    state: State<'_, AppState>,
    doc: Document,
    device: DeviceProfile,
    options: JobOptions,
    request: TonalRequest,
) -> Result<TonalPreview, String> {
    let head = state.head();
    blocking(move || {
        if request.max_px == 0 {
            return Err(tr!("desktop.edit.tonal.bad_size"));
        }
        run_tonal(&doc, &device, &options, head, &request)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_doc::{DesignObject, LayerSettings, ObjectKind, VectorObject, tone_preset};

    fn square_doc() -> Document {
        let mut layer = LayerSettings::for_color(tlaku_core::Rgb(255, 0, 0));
        layer.power = 40.0;
        let obj = DesignObject {
            id: "a".into(),
            layer: layer.color,
            transform: tlaku_doc::affine::IDENTITY,
            kind: ObjectKind::Vector(VectorObject::new(vec![tlaku_core::bezpath::BezPath::from(
                &tlaku_core::Polyline {
                    points: vec![
                        tlaku_core::Point::new(10.0, 10.0),
                        tlaku_core::Point::new(30.0, 10.0),
                        tlaku_core::Point::new(30.0, 30.0),
                        tlaku_core::Point::new(10.0, 30.0),
                    ],
                    closed: true,
                },
            )])),
            locked: false,
            hidden: false,
            group: None,
            priority: None,
            power_scale: None,
            shape: None,
            start: None,
            manual_tabs: Vec::new(),
            array: None,
        };
        Document {
            objects: vec![obj],
            layers: vec![layer],
            ..Document::default()
        }
    }

    fn request(region: Option<[f64; 4]>) -> TonalRequest {
        TonalRequest {
            curve: tone_preset("plywood").unwrap(),
            spot_mm: 0.1,
            region_mm: region,
            max_px: 300,
        }
    }

    #[test]
    fn renders_a_png_and_reuses_the_job_for_a_zoom() {
        let doc = square_doc();
        let device = DeviceProfile::default();
        let options = JobOptions::default();
        let full = run_tonal(&doc, &device, &options, None, &request(None)).unwrap();
        let png = STANDARD.decode(&full.png_base64).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        assert!(full.width_px <= 300 && full.max_dose > 0.0);
        assert_eq!(full.sheet_mm, [7.0, 7.0, 33.0, 33.0]);
        // El mismo trabajo con otra zona sale de la caché: misma clave, sin recalcular.
        let k = key(&doc, &device, &options, None).unwrap();
        assert_eq!(LAST_JOB.lock().unwrap().as_ref().unwrap().0, k);
        let zoom = run_tonal(
            &doc,
            &device,
            &options,
            None,
            &request(Some([8.0, 8.0, 14.0, 14.0])),
        )
        .unwrap();
        assert_eq!(zoom.covered_mm[0], 8.0);
        assert!(zoom.px_mm < full.px_mm);
        // Cambiar el documento invalida la clave.
        let mut other = square_doc();
        other.layers[0].power = 80.0;
        assert_ne!(key(&other, &device, &options, None), Some(k));
    }

    #[test]
    fn explains_why_nothing_can_be_drawn() {
        let device = DeviceProfile::default();
        let options = JobOptions::default();
        let empty = Document::default();
        assert!(run_tonal(&empty, &device, &options, None, &request(None)).is_err());
        let doc = square_doc();
        let far = request(Some([500.0, 500.0, 510.0, 510.0]));
        assert!(run_tonal(&doc, &device, &options, None, &far).is_err());
        let mut bad = request(None);
        bad.curve.dose_char = -1.0;
        assert!(run_tonal(&doc, &device, &options, None, &bad).is_err());
    }
}
