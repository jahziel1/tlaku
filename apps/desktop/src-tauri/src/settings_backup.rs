//! Respaldo de los ajustes `$n=valor` de la controladora GRBL en un archivo de texto.
//!
//! Rust solo guarda y devuelve el texto, con límites claros: la interfaz decide qué líneas son
//! ajustes y las valida una por una antes de escribir nada en la máquina. Un respaldo es texto
//! plano (`.tlakuset` o `.txt`) de pocos KiB; lo que pase de [`MAX_BYTES`] no lo es.

use crate::engine::write_atomic;
use std::fs;
use std::io::Read as _;
use std::path::Path;
use tlaku_i18n::tr;

/// Tamaño máximo de un respaldo, al escribirlo y al leerlo (256 KiB).
pub const MAX_BYTES: usize = 256 * 1024;

fn has_valid_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("tlakuset") || e.eq_ignore_ascii_case("txt"))
}

/// Guarda `text` de forma atómica. Exige la extensión `.tlakuset` o `.txt` y un tamaño razonable.
pub fn write_backup(path: &Path, text: &str) -> Result<(), String> {
    if !has_valid_extension(path) {
        return Err(tr!(
            "desktop.settings_backup.bad_ext",
            path = path.display()
        ));
    }
    if text.len() > MAX_BYTES {
        return Err(tr!(
            "desktop.settings_backup.too_big_write",
            size = text.len(),
            max = MAX_BYTES
        ));
    }
    write_atomic(path, text)
}

/// Lee un respaldo como UTF-8. Rechaza archivos de más de [`MAX_BYTES`] sin leerlos enteros.
pub fn read_backup(path: &Path) -> Result<String, String> {
    let io = |e: std::io::Error| format!("{}: {e}", path.display());
    let file = fs::File::open(path).map_err(io)?;
    if !file.metadata().map_err(io)?.is_file() {
        return Err(tr!(
            "desktop.settings_backup.not_a_file",
            path = path.display()
        ));
    }
    // Un byte de más basta para saber que se pasó, sin cargar un archivo gigante.
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io)?;
    if bytes.len() > MAX_BYTES {
        return Err(tr!(
            "desktop.settings_backup.too_big_read",
            path = path.display(),
            max = MAX_BYTES
        ));
    }
    String::from_utf8(bytes)
        .map_err(|_| tr!("desktop.settings_backup.not_utf8", path = path.display()))
}

#[tauri::command]
pub async fn write_settings_backup(path: String, text: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || write_backup(Path::new(&path), &text))
        .await
        .map_err(|e| tr!("desktop.common.task_interrupted", err = e))?
}

#[tauri::command]
pub async fn read_settings_backup(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || read_backup(Path::new(&path)))
        .await
        .map_err(|e| tr!("desktop.common.task_interrupted", err = e))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir()
                .join(format!("tlaku-settings-backup-{}-{n}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn file(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const SAMPLE: &str = "; Tlaku: respaldo de ajustes GRBL\n$0=10\n$30=1000\n";

    #[test]
    fn ida_y_vuelta_con_ambas_extensiones() {
        let dir = TempDir::new();
        for name in ["a.tlakuset", "b.txt", "c.TLAKUSET"] {
            let path = dir.file(name);
            write_backup(&path, SAMPLE).unwrap();
            assert_eq!(read_backup(&path).unwrap(), SAMPLE);
        }
    }

    #[test]
    fn exige_la_extension() {
        let dir = TempDir::new();
        for name in ["a.json", "sin_extension", "a.tlakuset.exe"] {
            let path = dir.file(name);
            let err = write_backup(&path, SAMPLE).unwrap_err();
            assert!(err.contains(".tlakuset"), "{err}");
            assert!(!path.exists());
        }
    }

    #[test]
    fn rechaza_lo_demasiado_grande_al_escribir_y_al_leer() {
        let dir = TempDir::new();
        let big = "x".repeat(MAX_BYTES + 1);
        let path = dir.file("grande.txt");
        assert!(
            write_backup(&path, &big)
                .unwrap_err()
                .contains("demasiado grande")
        );
        assert!(!path.exists());
        // Justo en el límite sí pasa.
        let limit = "y".repeat(MAX_BYTES);
        write_backup(&path, &limit).unwrap();
        assert_eq!(read_backup(&path).unwrap().len(), MAX_BYTES);
        // Un archivo que creció por fuera tampoco se lee.
        fs::write(&path, &big).unwrap();
        assert!(read_backup(&path).unwrap_err().contains("demasiado grande"));
    }

    #[test]
    fn lectura_rechaza_lo_que_no_es_utf8_ni_existe_ni_es_carpeta() {
        let dir = TempDir::new();
        let bin = dir.file("bin.txt");
        fs::write(&bin, [0xff, 0xfe, 0x00, 0x41]).unwrap();
        assert!(read_backup(&bin).unwrap_err().contains("UTF-8"));
        assert!(read_backup(&dir.file("no-existe.txt")).is_err());
        assert!(
            read_backup(&dir.0)
                .unwrap_err()
                .contains("no es un archivo")
        );
    }

    #[test]
    fn la_escritura_no_deja_temporales() {
        let dir = TempDir::new();
        let path = dir.file("r.tlakuset");
        write_backup(&path, SAMPLE).unwrap();
        write_backup(&path, "; otro\n").unwrap();
        assert_eq!(read_backup(&path).unwrap(), "; otro\n");
        let names: Vec<_> = fs::read_dir(&dir.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["r.tlakuset".to_string()]);
    }
}
