//! Importación de proyectos de LightBurn (`.lbrn`, `.lbrn2`, XML) con capas y ajustes.

use crate::import::ImportOptions;
use std::path::Path;
use tlaku_doc::ImportResult;

/// Objetos y capas (con sus ajustes traducidos) del proyecto.
pub(crate) fn import(
    path: &Path,
    data: &[u8],
    opts: &ImportOptions,
) -> Result<ImportResult, String> {
    // TODO(ola1): implementar.
    let _ = (data, opts);
    Err(format!(
        "{}: un proyecto de LightBurn aún no se puede importar",
        path.display()
    ))
}
