//! Importación de HPGL (`.plt`, `.hpgl`).

use crate::import::ImportOptions;
use std::path::Path;
use tlaku_doc::ImportResult;

/// Trayectorias del HPGL, una capa por pluma.
pub(crate) fn import(
    path: &Path,
    data: &[u8],
    opts: &ImportOptions,
) -> Result<ImportResult, String> {
    // TODO(ola1): implementar.
    let _ = (data, opts);
    Err(format!(
        "{}: un archivo HPGL aún no se puede importar",
        path.display()
    ))
}
