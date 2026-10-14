//! Conexión con un K40 de placa Lihuiyu M2 Nano (LHYMICRO-GL por USB). EXPERIMENTAL: no se ha
//! probado con una placa real; sí contra el emulador de `tlaku-lhymicro`.
//!
//! Igual que las demás, es un hilo dedicado dueño de la sesión ([`LhySession`]) que habla con la
//! interfaz solo por eventos `machine://*` (el mismo [`Sink`]). Lo que la distingue:
//!
//! - **`send_job` bloquea** mientras dura el trabajo, porque la placa recibe los datos a medida
//!   que los ejecuta. Por eso el hilo reparte el trabajo así: responde a la interfaz en cuanto
//!   sale el primer paquete y sigue dentro de `send_job` hasta el final. La pausa, la reanudación
//!   y la detención no pasan por el hilo: usan el [`JobControl`] clonable, que `send_job`
//!   atiende entre paquetes y en cada consulta de estado.
//! - **La placa no informa su posición.** La sesión la cuenta desde el home. Mientras no se haya
//!   hecho home (o tras detener o fallar un trabajo) se desconoce y no se mueve nada; empezar un
//!   trabajo en ese caso hace home primero.
//! - **La potencia no se controla por software** (se ajusta con la perilla del K40): el
//!   codificador avisa ([`preview`]) y la interfaz lo muestra antes de enviar.
//! - Los saltos y el encuadre van a la velocidad de salto de la placa; la velocidad de la
//!   interfaz no se usa.
//!
//! Si detener no se confirma (`StopOutcome::Unconfirmed`) el mensaje del trabajo trae
//! [`UNCONFIRMED_MARK`]: la interfaz lo busca para mostrar el aviso del paro de emergencia.
//!
//! Máquina virtual: el puerto `virtual:k40[@<velocidad>]` usa el emulador de la placa. El reloj
//! del emulador avanza con cada consulta de estado, así que se marca el paso durmiendo un poco en
//! cada lectura (velocidad 1 = tiempo real aproximado).

use super::{Emit, Sink};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tlaku_core::Point;
use tlaku_core::toolpath::Toolpath;
use tlaku_doc::{
    BufferStatus, ConnectResult, ConnectionEvent, DeviceProfile, JobEvent, JobState, LogDir,
    MachineState, Origin, ProgressEvent, StatusReport,
};
use tlaku_lhymicro::emu::{EmuConfig, emulator};
use tlaku_lhymicro::encode::{EncodeConfig, HomeCorner, Program, Warning, encode, mil_to_mm};
use tlaku_lhymicro::session::{
    BoardInfo, JobControl, JobState as LhyJobState, LhySession, SessionConfig, SessionError,
    StopOutcome,
};
use tlaku_lhymicro::speed::Board;
use tlaku_lhymicro::status::Status;
use tlaku_lhymicro::transport::{Transport, TransportError, TransportInfo};
use tlaku_lhymicro::usb::{self, UsbTimeouts, UsbTransport};

/// Puerto de la máquina virtual K40 (`virtual:k40` o `virtual:k40@5`).
pub const VIRTUAL_PORT: &str = "virtual:k40";

/// Prefijo de los puertos USB reales: `usb:k40:<n>` es la placa número `n` de la lista.
pub const USB_PREFIX: &str = "usb:k40:";

/// Texto que delata, dentro del mensaje de un trabajo, que detener NO se confirmó. La interfaz
/// lo busca para destacar el aviso del paro de emergencia (viene de `JobError`).
pub const UNCONFIRMED_MARK: &str = "NO hubo confirmación";

/// Aviso que acompaña a todo lo que toca una placa real.
pub const EXPERIMENTAL: &str = "EXPERIMENTAL: el soporte del K40 (M2 Nano) no se ha probado con \
una placa real. La M2 Nano de serie no controla la potencia por software: se ajusta con la perilla \
del K40. Ten el paro de emergencia al alcance y prueba primero con el láser deshabilitado o muy \
débil.";

/// Mensaje cuando una operación solo existe en GRBL.
pub const GRBL_ONLY: &str = "Esta función es solo de GRBL: la placa M2 Nano del K40 no usa G-code \
ni overrides en vivo.";

/// Mensaje cuando no se sabe dónde está el cabezal.
pub const UNKNOWN_POSITION: &str = "No se sabe dónde está el cabezal (la M2 Nano no informa su \
posición, se cuenta desde el home). Haz home primero.";

/// ¿Es el puerto de la máquina virtual K40 (con o sin `@velocidad`)?
pub fn is_virtual(port: &str) -> bool {
    port.split('@').next() == Some(VIRTUAL_PORT)
}

/// Número de placa de un puerto `usb:k40:<n>`.
pub fn usb_index(port: &str) -> Option<usize> {
    port.strip_prefix(USB_PREFIX)?.parse().ok()
}

/// Puertos USB del K40 para la lista: una entrada por placa encontrada. Un error al listar el USB
/// (permisos, sin bus) no estropea la lista: solo no hay placas.
pub fn usb_ports() -> Vec<String> {
    usb::list_boards()
        .unwrap_or_default()
        .iter()
        .map(|b| format!("{USB_PREFIX}{}", b.index))
        .collect()
}

mod config;
mod connection;
mod handle;
mod worker;

pub use config::*;
pub use connection::*;
pub use handle::*;

// Lo que no es público se comparte entre los submódulos (y con las pruebas) por estos `use`.
use worker::*;

#[cfg(test)]
mod tests;
