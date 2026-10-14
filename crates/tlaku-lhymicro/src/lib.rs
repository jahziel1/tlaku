//! Soporte experimental para K40 con placa Lihuiyu M2 Nano (protocolo LHYMICRO-GL por USB).
//! LightBurn no soporta esta placa; Tlaku sí. **No se ha probado con una placa real**.
//!
//! Módulos, de abajo arriba:
//! - [`crc`]: CRC-8 de los paquetes y su forma en el cable (CH341 en modo EPP): 30 bytes de carga,
//!   32 lógicos, 34 con los `0xA6`.
//! - [`status`]: consulta de estado (`0xA0`, 6 bytes) y sus códigos (206, 238, 207, 236, 239...).
//! - [`speed`]: ecuaciones de velocidad por placa (A, B, B1, B2, M, M1, M2, M3) y códigos `CV...`.
//! - [`decode`]: intérprete del lenguaje; reconstruye movimientos y estado del láser.
//! - [`encode`]: de `Toolpath` a LHYMICRO-GL (saltos, modo compacto, láser, velocidad, límites).
//! - [`packet`]: del flujo de comandos a paquetes de 30 bytes y las órdenes en tiempo real.
//! - [`transport`]: el trait [`transport::Transport`] (dos endpoints de bulto) y
//!   [`transport::MemoryTransport`]; `usb` (feature `usb`, `nusb`) es el CH341 real.
//! - [`session`]: sondeo, envío con progreso, pausa/reanudar/detener, home y saltos, con las
//!   reglas de seguridad.
//! - `emu` (feature `emu` o pruebas): placa simulada que verifica el CRC, contesta estados,
//!   decodifica y ejecuta en un reloj virtual, y registra dónde estuvo encendido el láser.
//!
//! # Uso desde la app
//!
//! ```ignore
//! use tlaku_lhymicro::session::{LhySession, SessionConfig};
//! use tlaku_lhymicro::usb::{UsbTransport, UsbTimeouts};
//!
//! let transport = UsbTransport::open(0, UsbTimeouts::default())?;
//! let mut session = LhySession::new(transport, SessionConfig { bed_mm: [300.0, 200.0], ..Default::default() });
//! let info = session.probe()?;            // solo lectura; detecta una Moshiboard
//! session.home()?;                        // la placa no informa su posición: se cuenta desde el home
//! let program = session.encode(&toolpath)?; // avisos en `program.warnings` (potencia ignorada...)
//! let control = session.control();        // mando clonable: pausa, reanudar, detener
//! let report = session.send_job(&program, &mut |p| println!("{}/{}", p.packet, p.packets))?;
//! ```
//!
//! La forma sigue a `tlaku-ruida::session::RuidaSession` (`probe`, `status`, `send_job` con
//! progreso, `home`, `jog_to`, `jog_by`, `frame`, `stop`); la diferencia es que aquí el trabajo
//! se transmite mientras se ejecuta, así que `send_job` bloquea y la pausa, la reanudación y la
//! detención van por el [`session::JobControl`] (en lugar del `AtomicBool` de cancelar).
//!
//! # Lo que se sabe y lo que no
//!
//! Verificado contra el código de MeerK40t (MIT) y sus vectores de prueba: paquete de 30 bytes
//! con CRC, 34 en el cable, códigos de estado, ecuaciones de velocidad (más de 1 500 vectores)
//! y el flujo de un rectángulo byte a byte. Inferido o sin hardware: ver la documentación de
//! cada módulo (marcado "inferido" o "no verificado").

pub mod crc;
pub mod decode;
#[cfg(any(test, feature = "emu"))]
pub mod emu;
pub mod encode;
pub mod packet;
pub mod session;
pub mod speed;
pub mod status;
mod text;
pub mod transport;
#[cfg(feature = "usb")]
pub mod usb;
