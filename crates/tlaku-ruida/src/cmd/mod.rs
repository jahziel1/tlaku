//! Comandos Ruida con su codificación y un decodificador (bytes a comandos legibles).
//!
//! Los bytes son los del flujo **en claro** (sin swizzle ni checksum de paquete). Un byte con el
//! bit 7 en 1 inicia un comando y los datos solo usan 7 bits, así que el corte del flujo en
//! comandos no necesita una tabla de longitudes (ruida.md sección 3). Aun así cada comando
//! conocido valida su longitud exacta.
//!
//! Las variantes se dividen en dos grupos:
//! - **Con significado** (ruida.md sección 4): movimiento y corte, potencia y velocidad, capas,
//!   aire, fin de archivo y de bloque, control del proceso, saltos (`D9`) y lectura de memoria.
//! - **De estructura** (`Point2`, `PartPoint`, `PartAbs`, `Copies`, `Small`, `Marker`, `ShortArg`,
//!   `ElementName`): campos de la cabecera de los archivos de RDWorks. El nombre de cada
//!   campo sale de ruida-pa y MeerK40t (ambos MIT; ver ruida.md sección 12); se codifican y
//!   decodifican sin más interpretación.
//!
//! Todo lo demás se decodifica como [`Command::Unknown`] (opcode y datos) para poder depurar.

mod decode;
mod display;
mod encode;
#[cfg(test)]
mod tests;
mod types;

pub use decode::{command_ranges, decode_one, decode_stream, disassemble, encode_stream};
pub use types::{Command, DecodeError, Limit, ProcessOp, RapidRef};
