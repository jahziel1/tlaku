//! Soporte experimental para controladoras Ruida (CO₂): archivo `.rd`, paquetes UDP y sesión.
//!
//! Módulos, de abajo arriba:
//! - [`num`]: enteros de 7 bits por byte (coordenadas, potencia, velocidad...).
//! - [`swizzle`]: ofuscación por byte y tabla de magics.
//! - [`cmd`]: comandos, codificación y decodificador para depurar.
//! - [`safety`]: límites de potencia, velocidad y coordenadas.
//! - [`rd`]: de `Toolpath` a archivo `.rd`.
//! - [`packet`]: paquetes UDP con checksum de 2 bytes y bytes de ACK.
//! - [`link`]: enlace por datagramas (`PacketLink`) con `UdpLink` y `MemoryLink`.
//! - [`session`]: sondeo, envío de trabajos, estado, pausa/reanudar/detener, saltos y encuadre.
//! - `emu` (feature `emu` o pruebas): controladora simulada, en memoria y por UDP en localhost.
//!
//! Las dudas entre fuentes se resolvieron con ruida-pa y MeerK40t (ruida.md sección 12); lo que
//! sigue sin confirmar con hardware se indica en cada documento y es configurable.

pub mod cmd;
pub mod link;
pub mod num;
pub mod packet;
pub mod rd;
pub mod safety;
pub mod session;
pub mod swizzle;
mod text;

#[cfg(any(test, feature = "emu"))]
pub mod emu;
