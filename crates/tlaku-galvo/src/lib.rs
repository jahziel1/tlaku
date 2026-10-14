//! Soporte **EXPERIMENTAL** para tarjetas galvo JCZ/BJJCZ LMC (EZCAD2) por USB: láseres de fibra,
//! UV y CO2 galvo. **No se ha probado con una tarjeta real.** Un láser de fibra es clase 4 y su
//! haz es invisible: lee `docs/hardware/galvo.md` y haz primero la prueba sin fuente conectada
//! (sección 9) antes de conectar un láser.
//!
//! Módulos, de abajo arriba:
//! - [`wire`]: comandos de 12 bytes (seis enteros de 16 bits little-endian), lista frente a
//!   inmediato, tandas de 256 comandos con relleno.
//! - [`codes`]: tabla de todos los códigos conocidos con fuente, confianza y política de envío.
//! - [`cmd`]: comandos tipados; solo existen los que Tlaku envía.
//! - [`params`]: unidades de la tarjeta (velocidad, frecuencia, potencia, retardos).
//! - [`calib`]: milímetros a 16 bits centrados en `0x8000`, corrección paramétrica del campo y
//!   tabla del `.cor`.
//! - [`cor`]: lectura de un `.cor` de EZCAD y su tabla de 65 x 65 desplazamientos, que `calib`
//!   aplica en el anfitrión (nunca se escribe a la tarjeta).
//! - [`compile`]: de `Toolpath` y `GalvoDevice` a la lista de comandos; encuadres.
//! - [`safety`]: límites del perfil, validación de listas, lista blanca de inmediatos y
//!   secuencia de apagado.
//! - [`transport`]: el trait `Transport`, transporte en memoria y tabla de tarjetas conocidas;
//!   `usb` (feature `usb`, `nusb`) es la tarjeta real.
//! - [`emu`]: tarjeta simulada con rastro del haz y fallos inyectables.
//! - [`session`]: conexión, encuadre con luz roja, marcado con progreso, detener.
//!
//! # Uso desde la app o el pipeline
//!
//! ```ignore
//! use tlaku_galvo::session::{GalvoSession, JobControl, SessionConfig};
//! use tlaku_galvo::transport::LaserSafetyAck;
//! use tlaku_galvo::usb::{UsbTimeouts, UsbTransport};
//!
//! // El pipeline llena `block.galvo` (GalvoParams) en cada bloque del Toolpath.
//! let ack = LaserSafetyAck::i_understand_laser_safety(); // solo tras la confirmación del usuario
//! let transport = UsbTransport::open(0, UsbTimeouts::default(), ack)?;
//! let mut session = GalvoSession::new(transport, SessionConfig::new(device.galvo.clone())?)?;
//! let info = session.probe()?;                         // solo lectura
//! let ctl = session.control();                         // clonable: ctl.request_stop()
//! session.frame(min, max, &ctl, None, &mut |p| println!("{} vueltas", p.loops))?; // luz roja
//! let report = session.mark(&toolpath, false, &ctl, &mut |p| println!("{}/{}", p.batch, p.batches))?;
//! ```
//!
//! # Lo que se sabe y lo que no
//!
//! Formato de 12 bytes, tandas de 256, códigos y sus parámetros: descritos por galvoplotter y
//! MeerK40t `balormk` (ambos MIT, ver THIRD_PARTY.md) y por la prosa pública de
//! docs/hardware/galvo.md. **Nada se ha verificado con una tarjeta real**: unidades, bits de
//! estado, secuencia de arranque, qué hace la tarjeta si se cae el USB. Cada módulo marca
//! "inferido" o "sin verificar" lo que corresponde.
//!
//! # Lo que no se hace, a propósito
//!
//! Escribir EEPROM o firmware, escribir la tabla de corrección (el `.cor` se aplica en el
//! anfitrión) o los parámetros de configuración de la tarjeta, `LaserSignalOn`, pausa/reanudar, escribir salidas arbitrarias, marcado al vuelo,
//! ejes y Z, y tarjetas EZCAD3/DLC.

pub mod calib;
pub mod cmd;
pub mod codes;
pub mod compile;
pub mod cor;
pub mod emu;
pub mod params;
pub mod safety;
pub mod session;
pub mod transport;
#[cfg(feature = "usb")]
pub mod usb;
pub mod wire;
