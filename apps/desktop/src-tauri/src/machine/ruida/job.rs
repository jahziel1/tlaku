//! Del documento a la controladora: el flujo `.rd` y las esquinas del encuadre.

use super::*;
use tlaku_i18n::tr;
use tlaku_ruida::rd::RefMode;

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

/// Rango de un eje para un texto (mm con un decimal; `∞` si no tiene límite, como la Y del
/// rotativo).
fn mm_text(v: f64) -> String {
    if v.is_finite() {
        format!("{v:.1}")
    } else if v < 0.0 {
        "-∞".into()
    } else {
        "∞".into()
    }
}

/// Calcula el trabajo y lo convierte en un flujo `.rd` listo para subir.
///
/// «Iniciar desde» decide la referencia del archivo (ruida.md 12.15):
/// - **Coordenadas absolutas**: `D8 10` y la posición del diseño en la cama.
/// - **Posición actual** / **Origen de usuario**: `D8 12` / `D8 11` y el trabajo con su punto de
///   anclaje en (0, 0). La controladora lo resuelve al **arrancar** (el trabajo normalmente se
///   guarda y se arranca después desde el panel), así que `head` no entra en el archivo: solo
///   sirve para avisar de que el trabajo no cabría si se arrancara con el cabezal donde está
///   (y para negarse cuando el trabajo se ejecuta nada más subirlo). El origen de usuario es el
///   que la controladora tiene guardado, no el `user_origin` de la aplicación.
///
/// `head` es la posición del cabezal en mm del sistema de la máquina, si se conoce.
pub fn build_upload(
    doc: &tlaku_doc::Document,
    device: &DeviceProfile,
    options: &tlaku_doc::JobOptions,
    head: Option<[f64; 2]>,
) -> Result<Upload, String> {
    let reference = tlaku_pipeline::ref_mode_for(options.start_from);
    let run = device.ruida.run_after_upload;
    let mut notes = Vec::new();
    let (rd, estimate_s) = if reference.is_relative() {
        let job = tlaku_pipeline::compute_anchored(doc, device, options)?;
        let rd = tlaku_pipeline::export_rd_for_device_ref(&job, device, reference)?;
        if let Some([(x0, x1), (y0, y1)]) = tlaku_pipeline::head_range(&job, device) {
            let (tx0, tx1, ty0, ty1) = (mm_text(x0), mm_text(x1), mm_text(y0), mm_text(y1));
            notes.push(match reference {
                RefMode::UserOrigin => tr!(
                    "desktop.ruida.note_user_origin",
                    x0 = tx0,
                    x1 = tx1,
                    y0 = ty0,
                    y1 = ty1
                ),
                _ => tr!(
                    "desktop.ruida.note_current",
                    x0 = tx0,
                    x1 = tx1,
                    y0 = ty0,
                    y1 = ty1
                ),
            });
            if reference == RefMode::Current {
                let fits = |h: [f64; 2]| (x0..=x1).contains(&h[0]) && (y0..=y1).contains(&h[1]);
                match head {
                    Some(h) if !fits(h) => {
                        let (hx, hy) = (format!("{:.1}", h[0]), format!("{:.1}", h[1]));
                        if run {
                            return Err(tr!(
                                "desktop.ruida.run_head_out_of_range",
                                x = hx,
                                y = hy,
                                x0 = mm_text(x0),
                                x1 = mm_text(x1),
                                y0 = mm_text(y0),
                                y1 = mm_text(y1)
                            ));
                        }
                        notes.push(tr!("desktop.ruida.head_out_of_range", x = hx, y = hy));
                    }
                    // Ejecutar al subir sin saber dónde está el cabezal: no se puede comprobar.
                    None if run => return Err(tr!("pipeline.place.no_current_position")),
                    _ => {}
                }
            }
        }
        (rd, job.preview.total_s)
    } else {
        let at = head.map(|[x, y]| tlaku_core::Point::new(x, y));
        let job = tlaku_pipeline::compute_at(doc, device, options, at)?;
        (
            tlaku_pipeline::export_rd_for_device(&job, device)?,
            job.preview.total_s,
        )
    };
    if device.rotary.enabled {
        notes.push(rotary_note());
    }
    Ok(Upload {
        rd,
        mode: if run {
            SendMode::Run
        } else {
            SendMode::Store { name: store_name() }
        },
        limits: tlaku_pipeline::limits_for_device(device),
        home: tlaku_pipeline::home_for_origin(device.origin),
        estimate_s,
        notes,
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

/// Esquinas del encuadre de una máquina Ruida. Como [`frame_corners`] con «Absolutas»; con
/// «Posición actual» recorre la caja del trabajo alrededor del cabezal (que hace falta conocer);
/// con «Origen de usuario» no se puede: ese origen vive en la controladora y no se puede leer.
pub fn ruida_frame_corners(
    doc: &tlaku_doc::Document,
    device: &DeviceProfile,
    options: &tlaku_doc::JobOptions,
    head: Option<[f64; 2]>,
) -> Result<Vec<(f64, f64)>, String> {
    use tlaku_doc::StartFrom;
    match options.start_from {
        StartFrom::Absolute => frame_corners(doc, device, options, head),
        StartFrom::UserOrigin => Err(tr!("desktop.ruida.frame_user_origin")),
        StartFrom::CurrentPosition => {
            let head = head.ok_or_else(|| tr!("pipeline.place.no_current_position"))?;
            let job = tlaku_pipeline::compute_anchored(doc, device, options)?;
            let [(x0, x1), (y0, y1)] = tlaku_pipeline::head_range(&job, device)
                .ok_or_else(|| tr!("desktop.common.nothing_to_frame"))?;
            if !((x0..=x1).contains(&head[0]) && (y0..=y1).contains(&head[1])) {
                return Err(tr!(
                    "desktop.ruida.frame_head_out_of_range",
                    x = format!("{:.1}", head[0]),
                    y = format!("{:.1}", head[1]),
                    x0 = mm_text(x0),
                    x1 = mm_text(x1),
                    y0 = mm_text(y0),
                    y1 = mm_text(y1)
                ));
            }
            tlaku_pipeline::anchored_frame_corners(&job, device, head)
                .ok_or_else(|| tr!("desktop.common.nothing_to_frame"))
        }
    }
}
