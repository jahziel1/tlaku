//! Imágenes incrustadas de LightBurn: se extraen a archivos y se enlazan como `ImageObject`.
//!
//! Dónde quedan: en la carpeta `<nombre del proyecto>_imagenes/`, junto al archivo `.lbrn`/`.lbrn2`,
//! con el nombre `img-<huella del contenido>.<ext>` (la misma imagen incrustada varias veces es un
//! solo archivo, y reimportar el proyecto reescribe los mismos). Si esa carpeta no se puede
//! escribir (solo lectura, por ejemplo) se usa una carpeta en el directorio temporal del sistema y
//! se avisa.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tlaku_doc::ImageObject;
use tlaku_i18n::tr;

pub(super) struct Extractor {
    preferred: PathBuf,
    fallback: PathBuf,
    /// Huella del contenido -> (ruta, ancho, alto) ya extraídos.
    done: HashMap<u64, (String, u32, u32)>,
    pub warnings: Vec<String>,
    warned_fallback: bool,
}

/// FNV-1a de 64 bits: huella estable (no criptográfica) para nombrar los archivos.
fn fingerprint(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Extensión según la firma de los bytes; `None` si no es un formato de imagen conocido.
fn extension(data: &[u8]) -> Option<&'static str> {
    Some(match data {
        [0x89, b'P', b'N', b'G', ..] => "png",
        [0xFF, 0xD8, 0xFF, ..] => "jpg",
        [b'B', b'M', ..] => "bmp",
        [b'G', b'I', b'F', b'8', ..] => "gif",
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => "webp",
        [b'I', b'I', 0x2A, 0x00, ..] | [b'M', b'M', 0x00, 0x2A, ..] => "tif",
        _ => return None,
    })
}

impl Extractor {
    pub(super) fn new(project_path: &Path) -> Self {
        let stem = project_path
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("proyecto");
        let dir = project_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let hash = fingerprint(project_path.to_string_lossy().as_bytes());
        Self {
            preferred: dir.join(format!("{stem}_imagenes")),
            fallback: std::env::temp_dir().join(format!("tlaku-lbrn-{stem}-{hash:08x}")),
            done: HashMap::new(),
            warnings: Vec::new(),
            warned_fallback: false,
        }
    }

    fn write(&mut self, name: &str, data: &[u8]) -> Result<PathBuf, String> {
        let try_dir = |dir: &Path| -> std::io::Result<PathBuf> {
            std::fs::create_dir_all(dir)?;
            let file = dir.join(name);
            std::fs::write(&file, data)?;
            Ok(file)
        };
        match try_dir(&self.preferred) {
            Ok(f) => Ok(f),
            Err(first) => {
                let file = try_dir(&self.fallback).map_err(|e| {
                    tr!(
                        "pipeline.import_lbrn_bitmap.save_failed",
                        preferred = self.preferred.display(),
                        first = first,
                        fallback = self.fallback.display(),
                        error = e
                    )
                })?;
                if !self.warned_fallback {
                    self.warned_fallback = true;
                    self.warnings.push(tr!(
                        "pipeline.import_lbrn_bitmap.saved_in_fallback",
                        first = first,
                        fallback = self.fallback.display()
                    ));
                }
                Ok(file)
            }
        }
    }

    /// Extrae una imagen y devuelve su `ImageObject` con la caja local `w` x `h` mm.
    pub(super) fn extract(&mut self, data: &[u8], w: f64, h: f64) -> Result<ImageObject, String> {
        let ext =
            extension(data).ok_or_else(|| tr!("pipeline.import_lbrn_bitmap.unknown_format"))?;
        let key = fingerprint(data);
        if let Some((path, wp, hp)) = self.done.get(&key) {
            return Ok(ImageObject {
                source_path: path.clone(),
                width_px: *wp,
                height_px: *hp,
                size_mm: [w, h],
                mask: Vec::new(),
                variable: None,
            });
        }
        // Se comprueba la cabecera antes de dejar un archivo inútil en disco (y no se decodifica
        // la imagen entera solo para saber su tamaño).
        let (wp, hp) = tlaku_raster::gray::dimensions_of_bytes(data)?;
        let file = self.write(&format!("img-{key:016x}.{ext}"), data)?;
        let path = file.to_string_lossy().into_owned();
        self.done.insert(key, (path.clone(), wp, hp));
        Ok(ImageObject {
            source_path: path,
            width_px: wp,
            height_px: hp,
            size_mm: [w, h],
            mask: Vec::new(),
            variable: None,
        })
    }
}
