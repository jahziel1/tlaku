//! Importación de PDF y AI (vectores del contenido de página).

use crate::import::ImportOptions;
use std::path::Path;
use tlaku_doc::ImportResult;

/// Trayectorias vectoriales del PDF, una capa por color.
pub(crate) fn import(
    path: &Path,
    data: &[u8],
    opts: &ImportOptions,
) -> Result<ImportResult, String> {
    // TODO(ola1): implementar.
    let _ = (data, opts);
    Err(format!(
        "{}: un PDF o AI aún no se puede importar",
        path.display()
    ))
}
