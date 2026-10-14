//! Del documento a la controladora: el flujo `.rd` y las esquinas del encuadre.

use super::*;
use tlaku_i18n::tr;

// ---------------------------------------------------------------------------
// Del documento a la controladora
// ---------------------------------------------------------------------------

/// Nombre del documento que se guarda en la memoria: `TLAKU` y tres cifras del reloj, para no
/// repetirlo entre envíos seguidos (ASCII y corto, como los que escribe RDWorks).
fn store_name() -> String {
    let minutes = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 60);
    format!("TLAKU{:03}", minutes % 1000)
}

/// Calcula el trabajo y lo convierte en un flujo `.rd` listo para subir. `head` es la posición
/// del cabezal en mm del sistema de la máquina (para empezar desde la posición actual).
pub fn build_upload(
    doc: &tlaku_doc::Document,
    device: &DeviceProfile,
    options: &tlaku_doc::JobOptions,
    head: Option<[f64; 2]>,
) -> Result<Upload, String> {
    let at = head.map(|[x, y]| tlaku_core::Point::new(x, y));
    let job = tlaku_pipeline::compute_at(doc, device, options, at)?;
    let rd = tlaku_pipeline::export_rd_for_device(&job, device)?;
    Ok(Upload {
        rd,
        mode: if device.ruida.run_after_upload {
            SendMode::Run
        } else {
            SendMode::Store { name: store_name() }
        },
        limits: tlaku_pipeline::limits_for_device(device),
        home: tlaku_pipeline::home_for_origin(device.origin),
        estimate_s: job.preview.total_s,
        notes: if device.rotary.enabled {
            vec![rotary_note()]
        } else {
            Vec::new()
        },
        overwrite: false,
        auto_name: true,
    })
}

/// Esquinas del encuadre (rectángulo de todos los movimientos, overscan incluido) en mm del
/// sistema de la máquina, cerrando el recorrido en la primera.
pub fn frame_corners(
    doc: &tlaku_doc::Document,
    device: &DeviceProfile,
    options: &tlaku_doc::JobOptions,
    head: Option<[f64; 2]>,
) -> Result<Vec<(f64, f64)>, String> {
    let at = head.map(|[x, y]| tlaku_core::Point::new(x, y));
    let job = tlaku_pipeline::compute_at(doc, device, options, at)?;
    let b = job
        .bounds()
        .ok_or_else(|| tr!("desktop.common.nothing_to_frame"))?;
    let home = tlaku_pipeline::home_for_origin(device.origin);
    let (ax, ay) = home.to_machine(b.min, device.bed);
    let (cx, cy) = home.to_machine(b.max, device.bed);
    let (x0, x1, y0, y1) = (ax.min(cx), ax.max(cx), ay.min(cy), ay.max(cy));
    Ok(vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)])
}
