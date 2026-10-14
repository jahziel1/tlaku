//! Conexión con una controladora Ruida (CO2) por UDP. EXPERIMENTAL: no se ha probado con una
//! máquina real (ver `docs/hardware/ruida.md`); sí contra el emulador de `tlaku-ruida`.
//!
//! Como la de GRBL, es un hilo dedicado dueño del enlace ([`RuidaSession`]) que habla con la
//! interfaz solo por eventos `machine://*` (el mismo [`Sink`]):
//!
//! - en reposo, lee el estado y la posición cada ~500 ms (`machine://status`). **Nunca durante una
//!   subida**: el hilo está dentro de `send_job`, así que no hay lecturas intercaladas;
//! - [`RuidaHandle::upload`] sube el `.rd` ya generado con progreso (`machine://progress`). Con
//!   [`SendMode::Store`] queda como documento en la memoria y no arranca nada; con
//!   [`SendMode::Run`] se ejecuta al llegar y el hilo sigue el trabajo por el estado hasta que la
//!   máquina vuelve a reposo (`machine://job` finished). Si la subida falla, el protocolo seguro
//!   es el de `RuidaSession::send_job` (Detener `D8 01` y aviso si no se confirmó);
//! - pausa, reanudar y detener (`D8 02/03/01`), saltos con el láser apagado y encuadre (cuatro
//!   esquinas, sin bloquear el hilo: las órdenes de detener siguen atendiéndose);
//! - información, lista de documentos y ajustes de la controladora. **Escribir ajustes, borrar y
//!   ejecutar documentos (ola 6E) es EXPERIMENTAL**: lista cerrada de direcciones, respaldo previo
//!   con fecha (`backup.rs`), confirmación explícita, verificación posterior y restauración;
//!   ver `worker/settings.rs` y `tlaku_ruida::session::settings`.
//!
//! Hay cosas del protocolo que no se conocen y se resolvieron con cautela:
//!
//! - Según un foro, algunas controladoras ignoran la red mientras ejecutan un trabajo
//!   ([FOR3] de ruida.md). Con un trabajo en marcha, un sondeo sin respuesta no cuenta como
//!   conexión perdida; fuera de uno, tres sondeos seguidos sin respuesta sí.
//! - El bit `0x2` del estado es "pausa" o "fin de parte" según la fuente.
//!
//! Máquina virtual: el puerto `virtual:ruida[@<velocidad>]` arranca un `UdpEmulator` en
//! `127.0.0.1` y se conecta a él por UDP real, así que se prueba el mismo camino que con una
//! máquina; el trabajo "se ejecuta" durante el tiempo estimado (dividido por la velocidad).
//!
//! Rotativo (M28): la fuente disponible (`docs/hardware/dsp-co2.md`) dice que en las DSP el
//! rotativo se activa en los ajustes de la controladora. El `.rd` no lleva nada del rotativo, así
//! que no se escala la Y: el trabajo viaja en mm de superficie y la controladora hace la
//! conversión. Tlaku lo avisa en el registro y puede escribir los dos ajustes públicos del
//! rotativo (`rotary_pulse`, `rotary_diameter`) con las protecciones de arriba; cómo se activa en
//! la controladora no está en ninguna fuente pública y se hace en su panel.

use super::{Emit, Sink};
use std::collections::VecDeque;
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tlaku_doc::{
    BufferStatus, ConnectResult, ConnectionEvent, DeviceProfile, JobEvent, JobState, LogDir,
    MachineState, ProgressEvent, RuidaFile, RuidaInfo, StatusReport,
};
use tlaku_i18n::tr;
use tlaku_ruida::emu::{EmuConfig, UdpEmulator};
use tlaku_ruida::link::{PacketLink, UdpLink};
use tlaku_ruida::packet::PacketConfig;
use tlaku_ruida::rd::HomeCorner;
use tlaku_ruida::safety::SafetyLimits;
use tlaku_ruida::session::{
    ControllerInfo, MachineStatus, ProbeInfo, RuidaSession, SendMode, SessionConfig, SessionError,
};
use tlaku_ruida::swizzle::{MAGIC_DEFAULT, Magic};

/// Puerto de la máquina virtual Ruida (`virtual:ruida` o `virtual:ruida@5`).
pub const VIRTUAL_PORT: &str = "virtual:ruida";

/// Aviso que acompaña a todo lo que toca una máquina real.
pub fn experimental() -> String {
    tr!("desktop.ruida.experimental")
}

/// Nota del rotativo (M28): la controladora lo gestiona por su cuenta.
pub fn rotary_note() -> String {
    tr!("desktop.ruida.rotary_note")
}

/// Mensaje cuando una operación solo existe en GRBL.
pub fn grbl_only() -> String {
    tr!("desktop.ruida.grbl_only")
}

/// ¿Es el puerto de la máquina virtual Ruida (con o sin `@velocidad`)?
pub fn is_virtual(port: &str) -> bool {
    port.split('@').next() == Some(VIRTUAL_PORT)
}

mod backup;
mod connection;
mod handle;
mod job;
mod sim;
mod worker;

pub use backup::{backups_dir, list as list_backups, read as read_backup};
pub use connection::*;
pub use handle::*;
pub use job::*;
pub use sim::*;

// Lo que no es público se comparte entre los submódulos (y con las pruebas) por estos `use`.
use worker::*;

#[cfg(test)]
mod tests;
