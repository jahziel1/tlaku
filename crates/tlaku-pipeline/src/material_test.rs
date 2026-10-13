//! Prueba de materiales: una cuadrícula de celdas donde cada una es una capa con su propia
//! combinación de parámetros, con rótulos de trazo sencillo.

use tlaku_doc::{Document, MaterialTestParams};

/// Documento con la cuadrícula de prueba lista para enviarse.
pub fn material_test(params: &MaterialTestParams) -> Result<Document, String> {
    // TODO(fase4): generar la cuadrícula.
    let _ = params;
    Err("la prueba de materiales aún no está implementada".into())
}
