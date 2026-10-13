//! Lógica de los comandos que no dependen de Tauri: motor, archivos y perfiles de dispositivo.
//! Los manejadores de `commands.rs` solo adaptan argumentos y llaman aquí; así se prueba sin
//! ventana.

use laser_core::Point;
use laser_doc::{DeviceProfile, Document, ExportResult, ImportResult, JobOptions, JobPreview};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

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
        .ok_or_else(|| format!("{}: no es una ruta de archivo", path.display()))?;
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
    laser_pipeline::import_file(&abs)
}

pub fn save_project(path: &str, doc: &Document) -> Result<(), String> {
    write_atomic(Path::new(path), &doc.to_json()?)
}

pub fn load_project(path: &str) -> Result<Document, String> {
    let text = fs::read_to_string(path).map_err(|e| io_error(Path::new(path), &e))?;
    Document::from_json(&text).map_err(|e| format!("{path}: {e}"))
}

/// Rutas de las imágenes que referencia el documento (para permitirlas en el protocolo de assets).
pub fn image_paths(doc: &Document) -> Vec<PathBuf> {
    doc.objects
        .iter()
        .filter_map(|o| match &o.kind {
            laser_doc::ObjectKind::Image(i) => Some(PathBuf::from(&i.source_path)),
            laser_doc::ObjectKind::Vector(_) => None,
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
    Ok(laser_pipeline::compute_at(doc, device, options, point(head))?.preview)
}

pub fn export_gcode(
    doc: &Document,
    device: &DeviceProfile,
    options: &JobOptions,
    path: &str,
    head: Head,
) -> Result<ExportResult, String> {
    let job = laser_pipeline::compute_at(doc, device, options, point(head))?;
    let mut text = laser_pipeline::gcode(&job, device);
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

/// G-code listo para transmitir: el mismo que `export_gcode`, ya validado contra el búfer de
/// recepción del dispositivo (una línea que no cabe o con caracteres de tiempo real falla antes de
/// tocar la máquina).
pub fn job_lines(
    doc: &Document,
    device: &DeviceProfile,
    options: &JobOptions,
    head: Head,
) -> Result<Vec<String>, String> {
    let job = laser_pipeline::compute_at(doc, device, options, point(head))?;
    let text = laser_pipeline::gcode(&job, device);
    laser_grbl::preflight::validate_gcode(&text, device.rx_buffer as usize)?;
    Ok(text.lines().map(str::to_string).collect())
}

/// G-code del encuadre (láser apagado) del trabajo.
pub fn frame_lines(
    doc: &Document,
    device: &DeviceProfile,
    options: &JobOptions,
    speed: f64,
    head: Head,
) -> Result<Vec<String>, String> {
    if !(speed.is_finite() && speed > 0.0) {
        return Err("La velocidad del encuadre debe ser mayor que 0.".into());
    }
    let job = laser_pipeline::compute_at(doc, device, options, point(head))?;
    let lines = laser_pipeline::frame_gcode(&job, device, speed);
    if lines.is_empty() {
        return Err("No hay nada que encuadrar: el trabajo está vacío.".into());
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
        return Err("El perfil necesita un id.".into());
    }
    if d.name.trim().is_empty() {
        return Err("El perfil necesita un nombre.".into());
    }
    let positive = |what: &str, v: &[f64]| {
        if v.iter().all(|x| x.is_finite() && *x > 0.0) {
            Ok(())
        } else {
            Err(format!("Perfil '{}': {what} debe ser mayor que 0.", d.name))
        }
    };
    positive("el área de trabajo", &d.bed)?;
    positive("S máxima", &[d.s_max])?;
    positive("la aceleración", &d.accel)?;
    positive("la velocidad máxima", &d.max_rate)?;
    positive("la velocidad de movimientos rápidos", &[d.rapid_rate])?;
    if d.baud == 0 || d.rx_buffer < 16 {
        return Err(format!(
            "Perfil '{}': los baudios y el búfer de recepción (mínimo 16 bytes) deben ser válidos.",
            d.name
        ));
    }
    Ok(())
}

/// Perfiles guardados. Sin archivo todavía, devuelve el perfil de fábrica (sin guardarlo).
pub fn list_devices(config_dir: &Path) -> Result<Vec<DeviceProfile>, String> {
    let file = devices_file(config_dir);
    match fs::read_to_string(&file) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| {
            format!(
                "{}: el archivo de perfiles está dañado ({e}). Bórralo para volver al perfil de fábrica.",
                file.display()
            )
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![DeviceProfile::default()]),
        Err(e) => Err(io_error(&file, &e)),
    }
}

fn store_devices(config_dir: &Path, devices: &[DeviceProfile]) -> Result<(), String> {
    fs::create_dir_all(config_dir).map_err(|e| io_error(config_dir, &e))?;
    let text = serde_json::to_string_pretty(devices)
        .map_err(|e| format!("no se pudo serializar los perfiles: {e}"))?;
    write_atomic(&devices_file(config_dir), &text)
}

/// Crea o reemplaza el perfil con ese `id`.
pub fn save_device(config_dir: &Path, device: DeviceProfile) -> Result<(), String> {
    check_device(&device)?;
    let mut all = list_devices(config_dir)?;
    match all.iter_mut().find(|d| d.id == device.id) {
        Some(slot) => *slot = device,
        None => all.push(device),
    }
    store_devices(config_dir, &all)
}

pub fn delete_device(config_dir: &Path, id: &str) -> Result<(), String> {
    let mut all = list_devices(config_dir)?;
    let before = all.len();
    all.retain(|d| d.id != id);
    if all.len() == before {
        return Err(format!("No existe el perfil '{id}'."));
    }
    store_devices(config_dir, &all)
}

#[cfg(test)]
mod tests;
