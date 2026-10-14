//! De `Toolpath` a archivo `.rd` (flujo de comandos Ruida con cabecera, capas, cuerpo y pie).
//!
//! La estructura sale de los archivos de RDWorks (ejemplo 1 de EduTech, ruida.md sección 5) y se
//! contrasta con lo que escriben MeerK40t y ruida-pa (ambos MIT, verificados por ellos en una
//! RDC6442S; ver ruida.md sección 12): `D8 00` va siempre al principio, el nombre de elemento es
//! `UNNAMED `, las matrices de copias son 1x1 y la máscara de tubos de `CA 03` es 1.
//!
//! Orden del flujo generado:
//! 1. Arranque: `D8 10`, `F0`, `E6 01`, `F1 02 00`, `D8 00`.
//! 2. Cajas del trabajo (`E7 06/03/07/50/51/04/05`).
//! 3. Cabecera de cada capa (`C9 04`, `C6 31..38`, `CA 06`, `CA 41`, `E7 52/53/61/62`).
//! 4. `CA 22`, offsets (siempre 0) y bloque de elemento (`E7 54/55`, `F1`, `F2`).
//! 5. `EA`, `E7 60/13/17/23/24/08`.
//! 6. Cuerpo de cada capa, **en el orden de los bloques**: `CA 01 00`, `CA 02`, `CA 01 30/10`,
//!    aire, velocidad, retardos, potencias, `CA 03`, `CA 10 00` y los movimientos.
//! 7. Pie: `EB`, `E7 00`, suma `E5 05` (opcional), `DA 01 06 20` (opcional) y `D7`.
//!
//! Cada `ToolpathBlock` con al menos un corte es una capa. Los bloques sin cortes se omiten.
//!
//! **Potencia variable** (ráster, relleno con overscan): si los cortes de un bloque no comparten
//! potencia, la capa lleva mín = 0 y máx = la mayor del bloque, y antes de cada corte cuya
//! potencia cambia se emite `C7 P` (potencia inmediata, la que usa ruida-pa para escribir
//! imágenes). Los cortes con potencia 0 (el overscan) se escriben como saltos con el láser
//! apagado ([`ZeroPower::Travel`]), como hacen MeerK40t y ruida-pa. No hay eje Z ni rotativo.

mod build;
mod options;
mod plan;
#[cfg(test)]
mod tests;

use crate::swizzle::Magic;
use crate::{cmd, safety};
pub use build::build_commands;
pub use options::{
    ELEMENT_NAME, FileChecksum, HomeCorner, LayerOptions, MAX_LAYERS, RdError, RuidaJobOptions,
    ZeroPower, decode_element_name, encode_element_name,
};
use tlaku_core::toolpath::Toolpath;

/// Genera los bytes del `.rd` **sin swizzle**. Antes de devolverlos verifica el flujo resultante
/// contra los límites de seguridad.
pub fn build_rd(tp: &Toolpath, opts: &RuidaJobOptions) -> Result<Vec<u8>, RdError> {
    let cmds = build_commands(tp, opts)?;
    safety::check_commands(&cmds, &opts.limits, opts.footer_stats)?;
    Ok(cmd::encode_stream(&cmds)?)
}

/// Aplica el swizzle a un flujo en claro (un `.rd` en disco y los datos por UDP/USB van así).
pub fn to_swizzled(plain: &[u8], magic: Magic) -> Vec<u8> {
    magic.swizzle_all(plain)
}

/// Inversa de [`to_swizzled`].
pub fn from_swizzled(wire: &[u8], magic: Magic) -> Vec<u8> {
    magic.unswizzle_all(wire)
}
