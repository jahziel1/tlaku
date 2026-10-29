//! Lógica de los comandos que no dependen de Tauri: motor, archivos y perfiles de dispositivo.
//! Los manejadores de `commands.rs` solo adaptan argumentos y llaman aquí; así se prueba sin
//! ventana.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use tlaku_core::Point;
use tlaku_doc::{
    Connection, DeviceKind, DeviceProfile, Document, ExportResult, ImportResult, JobOptions,
    JobPreview,
};
use tlaku_i18n::tr;

/// Posición de trabajo de la cabeza, `[x, y]`, si se conoce.
pub type Head = Option<[f64; 2]>;

fn point(head: Head) -> Option<Point> {
    head.map(|[x, y]| Point::new(x, y))
}

fn io_error(path: &Path, e: &std::io::Error) -> String {
    format!("{}: {e}", path.display())
}

/// Escribe `text` en un archivo temporal junto a `path` y lo renombra: un fallo a mitad no deja
/// un archivo a medias.
pub fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    let name = path
        .file_name()
        .ok_or_else(|| tr!("desktop.engine.not_a_file_path", path = path.display()))?;
    let mut tmp_name = name.to_os_string();
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);
    let write = || -> std::io::Result<()> {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        fs::rename(&tmp, path)
    };
    write().map_err(|e| {
        let _ = fs::remove_file(&tmp);
        io_error(path, &e)
    })
}

// ---------------------------------------------------------------------------
// Importación y proyectos
// ---------------------------------------------------------------------------

/// Importa SVG, DXF o una imagen. La ruta se vuelve absoluta para que `source_path` de las
/// imágenes sirva después con el protocolo de assets.
pub fn import(path: &str) -> Result<ImportResult, String> {
    let abs = std::path::absolute(path).map_err(|e| format!("{path}: {e}"))?;
    tlaku_pipeline::import_file(&abs)
}

/// Carpeta (absoluta) del archivo de proyecto: contra ella se guardan y leen las rutas relativas
/// de las fuentes de datos.
fn project_dir(path: &str) -> Option<PathBuf> {
    std::path::absolute(path)
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

/// Guarda el proyecto. Las fuentes de datos cercanas al archivo se escriben con ruta relativa
/// (el proyecto se puede mover con sus CSV); en memoria siguen siendo absolutas.
pub fn save_project(path: &str, doc: &Document) -> Result<(), String> {
    match project_dir(path) {
        Some(dir)
            if !doc.variable_text.sources.is_empty() || doc.variable_text.csv_path.is_some() =>
        {
            let mut doc = doc.clone();
            doc.relativize_data_paths(&dir);
            write_atomic(Path::new(path), &doc.to_json()?)
        }
        _ => write_atomic(Path::new(path), &doc.to_json()?),
    }
}

/// Lee el proyecto: las rutas relativas de las fuentes de datos se vuelven absolutas.
pub fn load_project(path: &str) -> Result<Document, String> {
    let text = fs::read_to_string(path).map_err(|e| io_error(Path::new(path), &e))?;
    let mut doc = Document::from_json(&text).map_err(|e| format!("{path}: {e}"))?;
    if let Some(dir) = project_dir(path) {
        doc.absolutize_data_paths(&dir);
    }
    Ok(doc)
}

/// Rutas de las imágenes que referencia el documento (para permitirlas en el protocolo de assets).
pub fn image_paths(doc: &Document) -> Vec<PathBuf> {
    doc.objects
        .iter()
        .filter_map(|o| match &o.kind {
            tlaku_doc::ObjectKind::Image(i) => Some(PathBuf::from(&i.source_path)),
            tlaku_doc::ObjectKind::Vector(_) | tlaku_doc::ObjectKind::Text(_) => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Trabajos
// ---------------------------------------------------------------------------

pub fn compute_job(
    doc: &Document,
    device: &DeviceProfile,
    options: &JobOptions,
    head: Head,
) -> Result<JobPreview, String> {
    if let Some(preview) = ruida_deferred_preview(doc, device, options, head) {
        return preview;
    }
    Ok(tlaku_pipeline::compute_at(doc, device, options, point(head))?.preview)
}

/// Vista previa de un trabajo Ruida cuyo punto de partida resuelve la controladora al arrancar
/// (`D8 11/12`, ruida.md 12.15): ni el origen de la aplicación ni, sin conexión, el cabezal
/// sirven. Se comprueba que el trabajo **quepa** en la cama y se muestra en su posición del
/// diseño. Con «Posición actual» y el cabezal conocido, la vista previa es la normal (colocada en
/// el cabezal). `None` si no es el caso.
fn ruida_deferred_preview(
    doc: &Document,
    device: &DeviceProfile,
    options: &JobOptions,
    head: Head,
) -> Option<Result<JobPreview, String>> {
    use tlaku_doc::StartFrom;
    let at_head = options.start_from == StartFrom::CurrentPosition && head.is_some();
    if device.kind != DeviceKind::Ruida || options.start_from == StartFrom::Absolute || at_head {
        return None;
    }
    Some((|| {
        tlaku_pipeline::compute_anchored(doc, device, options)?;
        let shown = JobOptions {
            start_from: StartFrom::Absolute,
            ..options.clone()
        };
        Ok(tlaku_pipeline::build(doc, device, &shown, None)?.preview)
    })())
}

pub fn export_gcode(
    doc: &Document,
    device: &DeviceProfile,
    options: &JobOptions,
    path: &str,
    head: Head,
) -> Result<ExportResult, String> {
    let job = tlaku_pipeline::compute_at(doc, device, options, point(head))?;
    let mut text = tlaku_pipeline::gcode(&job, device);
    if !text.ends_with('\n') {
        text.push('\n');
    }
    write_atomic(Path::new(path), &text)?;
    let lines = text.lines().filter(|l| !l.trim().is_empty()).count();
    Ok(ExportResult {
        lines: u32::try_from(lines).unwrap_or(u32::MAX),
        estimate_s: job.preview.total_s,
        warnings: job.preview.warnings,
    })
}

/// G-code listo para transmitir y lo que exige al enlace serie.
pub struct JobGcode {
    pub lines: Vec<String>,
    /// Carga sobre el enlace serie del perfil; `None` si no aplica (red, USB propio...).
    pub load: Option<tlaku_grbl::preflight::LinkLoad>,
}

/// El enlace serie del perfil limita al trabajo (baudios): solo con conexión serie y un firmware
/// que habla G-code. Por red o con un driver propio (Ruida, K40, galvo) no hay baudios.
fn serial_gcode_link(device: &DeviceProfile) -> bool {
    device.connection == Connection::Serial
        && !matches!(
            device.kind,
            DeviceKind::Ruida | DeviceKind::LhyMicro | DeviceKind::Galvo
        )
}

/// G-code listo para transmitir: el mismo que `export_gcode`, ya validado contra el búfer de
/// recepción del dispositivo (una línea que no cabe o con caracteres de tiempo real falla antes de
/// tocar la máquina), con la carga que pone al enlace serie.
pub fn job_gcode(
    doc: &Document,
    device: &DeviceProfile,
    options: &JobOptions,
    head: Head,
) -> Result<JobGcode, String> {
    let job = tlaku_pipeline::compute_at(doc, device, options, point(head))?;
    let text = tlaku_pipeline::gcode(&job, device);
    tlaku_grbl::preflight::validate_gcode(&text, device.rx_buffer as usize)?;
    let load = serial_gcode_link(device)
        .then(|| tlaku_pipeline::link_load(&job, device, &text, device.baud));
    Ok(JobGcode {
        lines: text.lines().map(str::to_string).collect(),
        load,
    })
}

/// Solo las líneas de [`job_gcode`].
#[cfg(test)]
pub fn job_lines(
    doc: &Document,
    device: &DeviceProfile,
    options: &JobOptions,
    head: Head,
) -> Result<Vec<String>, String> {
    job_gcode(doc, device, options, head).map(|g| g.lines)
}

/// Aviso (sin bloquear) de que el enlace serie del perfil no da abasto para este trabajo: el
/// que se muestra en el diálogo de inicio. `None` si el enlace alcanza o no aplica.
pub fn link_warning(
    doc: &Document,
    device: &DeviceProfile,
    options: &JobOptions,
    head: Head,
) -> Result<Option<String>, String> {
    if !serial_gcode_link(device) {
        return Ok(None);
    }
    let job = tlaku_pipeline::compute_at(doc, device, options, point(head))?;
    let text = tlaku_pipeline::gcode(&job, device);
    let load = tlaku_pipeline::link_load(&job, device, &text, device.baud);
    Ok(load
        .warning()
        .map(|w| w.trim_start_matches("Aviso: ").to_string()))
}

/// G-code del encuadre (láser apagado) del trabajo. Con `pointer`, es el puntero rojo (no el
/// rayo) el que recorre el rectángulo: la cabeza va desfasada `DeviceProfile::pointer_offset`.
pub fn frame_lines(
    doc: &Document,
    device: &DeviceProfile,
    options: &JobOptions,
    speed: f64,
    head: Head,
    pointer: bool,
) -> Result<Vec<String>, String> {
    if !(speed.is_finite() && speed > 0.0) {
        return Err(tr!("desktop.common.frame_speed"));
    }
    let job = tlaku_pipeline::compute_at(doc, device, options, point(head))?;
    let lines = if pointer {
        tlaku_pipeline::frame_gcode_pointer(&job, device, speed)?
    } else {
        tlaku_pipeline::frame_gcode(&job, device, speed)
    };
    if lines.is_empty() {
        return Err(tr!("desktop.common.nothing_to_frame"));
    }
    Ok(lines)
}

// ---------------------------------------------------------------------------
// Perfiles de dispositivo
// ---------------------------------------------------------------------------

/// Archivo de perfiles dentro de la carpeta de configuración de la app.
pub fn devices_file(config_dir: &Path) -> PathBuf {
    config_dir.join("devices.json")
}

fn check_device(d: &DeviceProfile) -> Result<(), String> {
    if d.id.trim().is_empty() {
        return Err(tr!("desktop.engine.profile_needs_id"));
    }
    if d.name.trim().is_empty() {
        return Err(tr!("desktop.engine.profile_needs_name"));
    }
    let positive = |v: &[f64]| v.iter().all(|x| x.is_finite() && *x > 0.0);
    if !positive(&d.bed) {
        return Err(tr!("desktop.engine.profile_bed", name = d.name));
    }
    if !positive(&[d.s_max]) {
        return Err(tr!("desktop.engine.profile_s_max", name = d.name));
    }
    if !positive(&d.accel) {
        return Err(tr!("desktop.engine.profile_accel", name = d.name));
    }
    if !positive(&d.max_rate) {
        return Err(tr!("desktop.engine.profile_max_rate", name = d.name));
    }
    if !positive(&[d.rapid_rate]) {
        return Err(tr!("desktop.engine.profile_rapid_rate", name = d.name));
    }
    if !d.pointer_offset.iter().all(|v| v.is_finite()) || !d.u_offset.is_finite() {
        return Err(tr!("desktop.engine.profile_offsets", name = d.name));
    }
    if d.baud == 0 || d.rx_buffer < 16 {
        return Err(tr!("desktop.engine.profile_serial", name = d.name));
    }
    Ok(())
}

/// Perfiles guardados. Sin archivo todavía, devuelve el perfil de fábrica (sin guardarlo).
pub fn list_devices(config_dir: &Path) -> Result<Vec<DeviceProfile>, String> {
    let file = devices_file(config_dir);
    match fs::read_to_string(&file) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| {
            tr!(
                "desktop.engine.profiles_corrupt",
                path = file.display(),
                err = e
            )
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![DeviceProfile::default()]),
        Err(e) => Err(io_error(&file, &e)),
    }
}

fn store_devices(config_dir: &Path, devices: &[DeviceProfile]) -> Result<(), String> {
    fs::create_dir_all(config_dir).map_err(|e| io_error(config_dir, &e))?;
    let text = serde_json::to_string_pretty(devices)
        .map_err(|e| tr!("desktop.engine.profiles_serialize", err = e))?;
    write_atomic(&devices_file(config_dir), &text)
}

/// Crea o reemplaza el perfil con ese `id`. Si queda como el perfil por defecto, se lo quita a
/// los demás (solo uno se activa al abrir).
pub fn save_device(config_dir: &Path, device: DeviceProfile) -> Result<(), String> {
    check_device(&device)?;
    let mut all = list_devices(config_dir)?;
    let (id, default) = (device.id.clone(), device.is_default);
    match all.iter_mut().find(|d| d.id == device.id) {
        Some(slot) => *slot = device,
        None => all.push(device),
    }
    if default {
        for d in all.iter_mut().filter(|d| d.id != id) {
            d.is_default = false;
        }
    }
    store_devices(config_dir, &all)
}

/// Ordena la lista de perfiles como `ids` (los que no se nombran quedan al final, en su orden).
/// Un id que no existe se ignora; repetidos cuentan una vez.
pub fn reorder_devices(config_dir: &Path, ids: &[String]) -> Result<(), String> {
    let mut rest = list_devices(config_dir)?;
    let mut sorted = Vec::with_capacity(rest.len());
    for id in ids {
        if let Some(i) = rest.iter().position(|d| &d.id == id) {
            sorted.push(rest.remove(i));
        }
    }
    sorted.extend(rest);
    store_devices(config_dir, &sorted)
}

pub fn delete_device(config_dir: &Path, id: &str) -> Result<(), String> {
    let mut all = list_devices(config_dir)?;
    let before = all.len();
    all.retain(|d| d.id != id);
    if all.len() == before {
        return Err(tr!("desktop.engine.no_such_profile", id = id));
    }
    store_devices(config_dir, &all)
}

#[cfg(test)]
mod tests;
