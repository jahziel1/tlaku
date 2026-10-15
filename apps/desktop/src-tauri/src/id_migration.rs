//! Migración de las carpetas de la app al cambiar el identificador (`org.tlaku.app` a
//! `io.github.jahziel1.tlaku`, requisito de Flathub).
//!
//! Tauri cuelga las carpetas de la app del `identifier`: `app_config_dir`, `app_data_dir`,
//! `app_local_data_dir` (y `app_cache_dir`, que Tlaku no usa). Al cambiar el id la app arrancaría
//! con todo vacío (dispositivos, materiales, preferencias, historial de trabajos, versiones,
//! retazos...). Al arrancar, [`run`] **copia** (no mueve) el contenido de cada carpeta antigua a la
//! nueva si esta no existe o está vacía.
//!
//! Reglas, por carpeta:
//! - Si la nueva trae el marcador [`DONE`], no se hace nada (idempotente).
//! - Si la antigua no existe, no se hace nada.
//! - Si la nueva ya tiene datos, **no se pisa**: se deja el marcador y se conserva lo que hay.
//! - Se copia a archivos temporales que se renombran al final, y [`PROGRESS`] marca que la copia
//!   está en curso; si la app muere a medias, el siguiente arranque la retoma. El marcador [`DONE`]
//!   se escribe al terminar.
//! - Si algo falla se deshace lo copiado (la carpeta nueva queda como estaba), se avisa por la
//!   salida de error y la app sigue con la carpeta nueva vacía; el siguiente arranque reintenta.
//!
//! La carpeta antigua nunca se borra ni se modifica: quien quiera puede borrarla a mano. Las
//! carpetas del propio WebView (`EBWebView` de WebView2) no se copian: el WebView las bloquea y su
//! contenido (`localStorage`) no se puede trasladar entre ids; ver `docs/publicar.md`.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Identificador anterior de la app.
pub const OLD_ID: &str = "org.tlaku.app";
/// Marcador de que la carpeta ya se atendió (se copió, o ya tenía datos propios).
pub const DONE: &str = ".migrado-desde-org.tlaku.app";
/// Marcador de copia en curso (si sigue ahí al arrancar, la copia se interrumpió).
pub const PROGRESS: &str = ".migrando-desde-org.tlaku.app";
/// Sufijo de los archivos temporales de la copia.
const TMP_SUFFIX: &str = ".migrando";
/// Entradas que son del WebView (no datos del usuario): ni cuentan como «datos» ni se copian.
const WEBVIEW_ENTRIES: &[&str] = &["EBWebView"];

/// Resultado de migrar una carpeta.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Ya estaba hecho (marcador presente).
    AlreadyDone,
    /// No hay carpeta antigua.
    NoOldDir,
    /// La carpeta nueva ya tenía datos: se respetan y se deja el marcador.
    KeptNew,
    /// Se copiaron `files` archivos.
    Migrated { files: usize },
    /// La copia falló y se deshizo; el texto dice por qué.
    Failed(String),
}

fn is_marker(name: &str) -> bool {
    name == DONE || name == PROGRESS
}

fn is_webview(name: &str) -> bool {
    WEBVIEW_ENTRIES.contains(&name)
}

/// ¿Hay algo del usuario en `dir`? (Ignora marcadores y carpetas del WebView.)
fn has_user_data(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|e| {
        let name = e.file_name();
        let name = name.to_string_lossy();
        !is_marker(&name) && !is_webview(&name)
    })
}

/// Lo creado por una copia, para deshacerla si falla.
#[derive(Default)]
struct Created {
    files: Vec<PathBuf>,
    dirs: Vec<PathBuf>,
}

impl Created {
    fn rollback(&self) {
        for f in &self.files {
            let _ = fs::remove_file(f);
        }
        // Las carpetas se crearon de la más externa a la más interna: se quitan al revés.
        for d in self.dirs.iter().rev() {
            let _ = fs::remove_dir(d);
        }
    }
}

/// Copia `from` dentro de `to` de forma recursiva. Los enlaces simbólicos se ignoran.
fn copy_tree(from: &Path, to: &Path, created: &mut Created, copied: &mut usize) -> io::Result<()> {
    if !to.exists() {
        fs::create_dir(to)?;
        created.dirs.push(to.to_path_buf());
    }
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if is_marker(&name_str) || is_webview(&name_str) || name_str.ends_with(TMP_SUFFIX) {
            continue;
        }
        let kind = entry.file_type()?;
        let target = to.join(&name);
        if kind.is_dir() {
            copy_tree(&entry.path(), &target, created, copied)?;
        } else if kind.is_file() {
            let mut tmp = target.clone().into_os_string();
            tmp.push(TMP_SUFFIX);
            let tmp = PathBuf::from(tmp);
            fs::copy(entry.path(), &tmp)?;
            created.files.push(tmp.clone());
            fs::rename(&tmp, &target)?;
            created.files.push(target);
            *copied += 1;
        }
    }
    Ok(())
}

/// Migra una carpeta (ver las reglas del módulo). Nunca entra en pánico ni propaga errores.
pub fn migrate_dir(old: &Path, new: &Path) -> Outcome {
    if new.join(DONE).exists() {
        return Outcome::AlreadyDone;
    }
    if !old.is_dir() {
        return Outcome::NoOldDir;
    }
    let resuming = new.join(PROGRESS).exists();
    if !resuming && has_user_data(new) {
        // Mejor esfuerzo: sin marcador solo se vuelve a evaluar en el próximo arranque.
        let _ = fs::create_dir_all(new).and_then(|()| fs::write(new.join(DONE), b""));
        return Outcome::KeptNew;
    }
    let mut created = Created::default();
    let mut copied = 0;
    let result = (|| {
        if let Some(parent) = new.parent() {
            fs::create_dir_all(parent)?;
        }
        if !new.exists() {
            fs::create_dir(new)?;
            created.dirs.push(new.to_path_buf());
        }
        fs::write(new.join(PROGRESS), b"")?;
        copy_tree(old, new, &mut created, &mut copied)?;
        fs::write(new.join(DONE), b"")?;
        let _ = fs::remove_file(new.join(PROGRESS));
        Ok::<(), io::Error>(())
    })();
    match result {
        Ok(()) => Outcome::Migrated { files: copied },
        Err(e) => {
            let _ = fs::remove_file(new.join(PROGRESS));
            created.rollback();
            Outcome::Failed(format!("{}: {e}", old.display()))
        }
    }
}

/// Pares (antigua, nueva) a partir de las carpetas actuales de la app: la antigua es la hermana
/// con el id anterior. Se descarta lo que no termina en `new_id` y las carpetas repetidas (en
/// macOS y Windows configuración y datos son la misma).
pub fn plan(new_dirs: &[PathBuf], new_id: &str) -> Vec<(PathBuf, PathBuf)> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for new in new_dirs {
        if new.file_name().and_then(|n| n.to_str()) != Some(new_id) || new_id == OLD_ID {
            continue;
        }
        if !seen.insert(new.clone()) {
            continue;
        }
        if let Some(parent) = new.parent() {
            out.push((parent.join(OLD_ID), new.clone()));
        }
    }
    out
}

/// Migra todas las carpetas e informa por la salida de error (la app no tiene otro registro).
/// Devuelve los resultados para las pruebas.
pub fn migrate_all(new_dirs: &[PathBuf], new_id: &str) -> Vec<Outcome> {
    plan(new_dirs, new_id)
        .into_iter()
        .map(|(old, new)| {
            let outcome = migrate_dir(&old, &new);
            match &outcome {
                Outcome::Migrated { files } => eprintln!(
                    "tlaku: migrados {files} archivos de {} a {}",
                    old.display(),
                    new.display()
                ),
                Outcome::KeptNew => eprintln!(
                    "tlaku: {} ya tiene datos; no se copia {}",
                    new.display(),
                    old.display()
                ),
                Outcome::Failed(why) => eprintln!(
                    "tlaku: AVISO: no se pudieron migrar los datos de {} a {} ({why}); la app sigue con la carpeta nueva vacía",
                    old.display(),
                    new.display()
                ),
                Outcome::AlreadyDone | Outcome::NoOldDir => {}
            }
            outcome
        })
        .collect()
}

/// Gancho de `setup`: migra configuración, datos y datos locales (la caché no se usa).
pub fn run(app: &tauri::AppHandle) {
    use tauri::Manager;
    let paths = app.path();
    let dirs: Vec<PathBuf> = [
        paths.app_config_dir(),
        paths.app_data_dir(),
        paths.app_local_data_dir(),
    ]
    .into_iter()
    .flatten()
    .collect();
    migrate_all(&dirs, &app.config().identifier);
}

#[cfg(test)]
mod tests;
