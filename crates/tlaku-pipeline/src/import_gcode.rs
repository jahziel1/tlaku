//! Importación de G-code de otros programas como trayectorias.

use crate::import::ImportOptions;
use std::path::Path;
use tlaku_doc::ImportResult;

/// Trayectorias de los movimientos con láser encendido.
pub(crate) fn import(
    path: &Path,
    data: &[u8],
    opts: &ImportOptions,
) -> Result<ImportResult, String> {
    // TODO(ola1): implementar.
    let _ = (data, opts);
    Err(format!(
        "{}: un G-code aún no se puede importar",
        path.display()
    ))
}
