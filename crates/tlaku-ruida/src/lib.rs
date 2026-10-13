//! Núcleo experimental de soporte para controladoras Ruida (CO₂), sin red.
//!
//! Módulos, de abajo arriba:
//! - [`num`]: enteros de 7 bits por byte (coordenadas, potencia, velocidad...).
//! - [`swizzle`]: ofuscación por byte y tabla de magics.
//! - [`cmd`]: comandos, codificación y decodificador para depurar.
//! - [`safety`]: límites de potencia, velocidad y coordenadas.
//! - [`rd`]: de `Toolpath` a archivo `.rd`.
//! - [`packet`]: paquetes UDP con checksum de 2 bytes.
//! - `emu` (feature `emu` o pruebas): controladora simulada para probar el particionado.
//!
//! Todo lo que depende de dudas entre fuentes es configurable; ver `docs/hardware/ruida.md`.

pub mod cmd;
pub mod num;
pub mod packet;
pub mod rd;
pub mod safety;
pub mod swizzle;

#[cfg(any(test, feature = "emu"))]
pub mod emu;
