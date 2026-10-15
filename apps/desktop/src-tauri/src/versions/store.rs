//! Almacén de versiones de UN proyecto: una carpeta con, por versión,
//!
//! - `<id>.tlaku.gz`: el documento (el texto JSON del proyecto) comprimido con gzip;
//! - `<id>.thumb.svg.gz`: la miniatura (SVG), si se pudo hacer;
//! - `<id>.meta.json`: lo que se muestra en la lista (tipo, nombre, hora, tamaños, resumen).
//!
//! **Atomicidad.** Cada archivo se escribe a un temporal y se renombra; el `meta` va al final y es
//! el que define que la versión existe: un corte a media escritura deja archivos sin `meta` que
//! la lista ignora y la limpieza borra. Nunca hay una versión a medias.
//!
//! **Duplicados.** Una versión con el mismo contenido que la más reciente no se guarda (si la
//! nueva es manual, la existente pasa a ser la manual con ese nombre); tampoco se repite un id
//! (mismo instante y mismo contenido).

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tlaku_i18n::tr;

const DATA_SUFFIX: &str = ".tlaku.gz";
const THUMB_SUFFIX: &str = ".thumb.svg.gz";
const META_SUFFIX: &str = ".meta.json";
/// Una versión no puede pesar más (sin comprimir): es el mismo tope de las copias de recuperación.
pub const MAX_VERSION_BYTES: u64 = 128 * 1024 * 1024;
/// Tope al descomprimir (defensa contra un archivo gz manipulado).
const MAX_INFLATED: u64 = 256 * 1024 * 1024;

/// De dónde viene la versión.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// El contenido que tenía el archivo justo antes de un guardado.
    Saved,
    /// Instantánea periódica mientras se edita.
    Auto,
    /// «Guardar versión…» con nombre. La poda automática no la toca.
    Manual,
    /// El estado que había antes de restaurar otra versión.
    Restore,
}

/// Lo que se sabe de una versión sin abrirla.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionInfo {
    pub id: String,
    pub kind: Kind,
    /// Nombre (solo las manuales).
    pub label: Option<String>,
    /// Momento de esa versión, en milisegundos desde 1970 (para `Saved`, la fecha del archivo).
    pub created_ms: u64,
    /// Tamaño del documento sin comprimir.
    pub size_bytes: u64,
    /// Lo que ocupa en disco (documento comprimido más miniatura).
    pub stored_bytes: u64,
    /// Huella del contenido (para no repetir).
    pub hash: String,
    pub objects: u32,
    pub layers: u32,
    pub has_thumb: bool,
}

/// Lo que se quiere guardar.
pub struct NewVersion<'a> {
    pub kind: Kind,
    pub label: Option<String>,
    pub created_ms: u64,
    /// El texto JSON del proyecto, tal como iría a un `.tlaku`.
    pub text: &'a str,
    pub objects: u32,
    pub layers: u32,
    pub thumb: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Store {
    pub dir: PathBuf,
}

fn io_error(path: &Path, e: &std::io::Error) -> String {
    format!("{}: {e}", path.display())
}

/// FNV-1a de 64 bits: huella para detectar contenido repetido (no es criptográfica ni hace falta).
pub fn content_hash(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Escribe `bytes` en un temporal junto a `path` y lo renombra; un fallo no deja nada a medias.
pub fn write_atomic_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let name = path
        .file_name()
        .ok_or_else(|| tr!("desktop.engine.not_a_file_path", path = path.display()))?;
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut tmp_name = name.to_os_string();
    tmp_name.push(format!(".{}.{n}.tmp", std::process::id()));
    let tmp = path.with_file_name(tmp_name);
    let write = || -> std::io::Result<()> {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        fs::rename(&tmp, path)
    };
    write().map_err(|e| {
        let _ = fs::remove_file(&tmp);
        io_error(path, &e)
    })
}

fn gzip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut enc = GzEncoder::new(Vec::new(), Compression::new(6));
    enc.write_all(bytes)
        .and_then(|()| enc.finish())
        .map_err(|e| tr!("desktop.versions.compress_failed", err = e))
}

fn gunzip(path: &Path) -> Result<String, String> {
    let file = fs::File::open(path).map_err(|e| io_error(path, &e))?;
    let mut out = String::new();
    GzDecoder::new(file)
        .take(MAX_INFLATED)
        .read_to_string(&mut out)
        .map_err(|e| io_error(path, &e))?;
    Ok(out)
}

/// El id solo usa letras, dígitos, guion y guion bajo: se usa como nombre de archivo.
pub fn check_id(id: &str) -> Result<(), String> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(tr!("desktop.versions.bad_id", id = id))
    }
}

impl Store {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn file(&self, id: &str, suffix: &str) -> PathBuf {
        self.dir.join(format!("{id}{suffix}"))
    }

    /// Todas las versiones con `meta` legible y documento presente, de la más nueva a la más vieja.
    pub fn list(&self) -> Vec<VersionInfo> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut out: Vec<VersionInfo> = entries
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(META_SUFFIX))
            .filter_map(|e| fs::read_to_string(e.path()).ok())
            .filter_map(|text| serde_json::from_str::<VersionInfo>(&text).ok())
            .filter(|v| check_id(&v.id).is_ok() && self.file(&v.id, DATA_SUFFIX).is_file())
            .collect();
        out.sort_by(|a, b| b.created_ms.cmp(&a.created_ms).then(b.id.cmp(&a.id)));
        out
    }

    fn write_meta(&self, info: &VersionInfo) -> Result<(), String> {
        let text =
            serde_json::to_vec(info).map_err(|e| tr!("desktop.versions.meta_failed", err = e))?;
        write_atomic_bytes(&self.file(&info.id, META_SUFFIX), &text)
    }

    /// Guarda una versión. `Ok(None)` si no hacía falta (mismo contenido que la última).
    pub fn add(&self, new: NewVersion<'_>) -> Result<Option<VersionInfo>, String> {
        let size = new.text.len() as u64;
        if size > MAX_VERSION_BYTES {
            return Err(tr!(
                "desktop.versions.too_big",
                size = size / (1024 * 1024),
                max = MAX_VERSION_BYTES / (1024 * 1024)
            ));
        }
        let hash = content_hash(new.text.as_bytes());
        let id = format!("{:013}-{}", new.created_ms, &hash[..8]);
        let existing = self.list();
        if let Some(latest) = existing.first()
            && ((latest.hash == hash && latest.size_bytes == size) || latest.id == id)
        {
            return self.upgrade_duplicate(latest, &new);
        }
        if existing.iter().any(|v| v.id == id) {
            return Ok(None);
        }
        fs::create_dir_all(&self.dir).map_err(|e| io_error(&self.dir, &e))?;
        let data = gzip(new.text.as_bytes())?;
        let mut stored = data.len() as u64;
        write_atomic_bytes(&self.file(&id, DATA_SUFFIX), &data)?;
        let mut has_thumb = false;
        if let Some(svg) = &new.thumb {
            // La miniatura es opcional: si falla no se pierde la versión.
            if let Ok(z) = gzip(svg.as_bytes())
                && write_atomic_bytes(&self.file(&id, THUMB_SUFFIX), &z).is_ok()
            {
                stored += z.len() as u64;
                has_thumb = true;
            }
        }
        let info = VersionInfo {
            id,
            kind: new.kind,
            label: new.label,
            created_ms: new.created_ms,
            size_bytes: size,
            stored_bytes: stored,
            hash,
            objects: new.objects,
            layers: new.layers,
            has_thumb,
        };
        if let Err(e) = self.write_meta(&info) {
            let _ = self.remove_files(&info.id);
            return Err(e);
        }
        Ok(Some(info))
    }

    /// El contenido ya estaba: si lo que llega es manual, la última versión pasa a serlo.
    fn upgrade_duplicate(
        &self,
        latest: &VersionInfo,
        new: &NewVersion<'_>,
    ) -> Result<Option<VersionInfo>, String> {
        if new.kind != Kind::Manual || (latest.kind == Kind::Manual && latest.label == new.label) {
            return Ok(None);
        }
        let mut info = latest.clone();
        info.kind = Kind::Manual;
        info.label = new.label.clone();
        self.write_meta(&info)?;
        Ok(Some(info))
    }

    /// El texto JSON de la versión.
    pub fn read_text(&self, id: &str) -> Result<String, String> {
        check_id(id)?;
        gunzip(&self.file(id, DATA_SUFFIX))
    }

    /// La miniatura SVG, si hay.
    pub fn read_thumb(&self, id: &str) -> Result<Option<String>, String> {
        check_id(id)?;
        let file = self.file(id, THUMB_SUFFIX);
        if !file.is_file() {
            return Ok(None);
        }
        gunzip(&file).map(Some)
    }

    pub fn has(&self, id: &str) -> bool {
        check_id(id).is_ok() && self.file(id, META_SUFFIX).is_file()
    }

    fn remove_files(&self, id: &str) -> Result<(), String> {
        // El meta primero: sin meta la versión deja de existir aunque falle el resto.
        for suffix in [META_SUFFIX, DATA_SUFFIX, THUMB_SUFFIX] {
            let file = self.file(id, suffix);
            match fs::remove_file(&file) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(io_error(&file, &e)),
            }
        }
        Ok(())
    }

    /// Borra las versiones `ids` (las que no existan se ignoran). Devuelve cuántas se borraron.
    pub fn delete(&self, ids: &[String]) -> Result<u32, String> {
        let mut n = 0;
        for id in ids {
            check_id(id)?;
            if self.has(id) {
                self.remove_files(id)?;
                n += 1;
            }
        }
        Ok(n)
    }

    /// Borra todas las versiones y los restos. Devuelve cuántas versiones había.
    pub fn clear(&self) -> Result<u32, String> {
        let ids: Vec<String> = self.list().into_iter().map(|v| v.id).collect();
        let n = self.delete(&ids)?;
        self.remove_orphans(&HashSet::new());
        Ok(n)
    }

    /// Borra lo que dice la política y los restos de escrituras interrumpidas. Devuelve los ids borrados.
    pub fn prune(&self, now_ms: u64, policy: super::retention::Policy) -> Vec<String> {
        let versions = self.list();
        let mut removed = Vec::new();
        for id in super::retention::plan(&versions, now_ms, policy) {
            if self.remove_files(&id).is_ok() {
                removed.push(id);
            }
        }
        let keep: HashSet<String> = self.list().into_iter().map(|v| v.id).collect();
        self.remove_orphans(&keep);
        removed
    }

    /// Archivos de versión sin `meta` (corte a media escritura) y temporales.
    fn remove_orphans(&self, keep: &HashSet<String>) {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let base = [DATA_SUFFIX, THUMB_SUFFIX]
                .iter()
                .find_map(|s| name.strip_suffix(s));
            let orphan = base.is_some_and(|id| !keep.contains(id));
            if name.ends_with(".tmp") || orphan {
                let _ = fs::remove_file(e.path());
            }
        }
    }
}
