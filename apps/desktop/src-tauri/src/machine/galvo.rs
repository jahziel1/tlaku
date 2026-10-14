//! Conexión con una tarjeta galvo JCZ/BJJCZ LMC (EZCAD2, USB 9588:9899). **EXPERIMENTAL: no se ha
//! probado con una tarjeta real**; sí contra el emulador de `tlaku-galvo`. Un láser de fibra es
//! clase 4 y su haz es INVISIBLE.
//!
//! Igual que Ruida y el K40, es un hilo dedicado dueño de la sesión ([`GalvoSession`]) que habla
//! con la interfaz solo por eventos `machine://*` (el mismo [`Sink`]). Lo que lo distingue:
//!
//! - **Seguridad.** Para abrir una tarjeta real (`usb:galvo:<n>`) hace falta un reconocimiento de
//!   seguridad explícito ([`SafetyAck`]: gafas con la DO correcta, recinto cerrado y paro de
//!   emergencia al alcance). Solo [`SafetyAck::token`] crea el [`LaserSafetyAck`] que exige el USB,
//!   y la interfaz lo pide en CADA conexión (no se recuerda). La máquina virtual (`virtual:galvo`)
//!   usa el emulador y no lo necesita.
//! - **Sin pausa, a propósito.** Pausar deja el espejo quieto con el haz encendido (reportes de
//!   usuarios, docs/hardware/galvo.md §5): solo se puede detener. Si detener no se confirma
//!   (`StopOutcome::Unconfirmed`, `laser_may_be_on`), el mensaje trae [`UNCONFIRMED_MARK`] y la
//!   conexión queda marcada ([`GalvoState::laser_may_be_on`]) hasta un apagado confirmado: hay que
//!   cortar la energía de la fuente.
//! - **El encuadre es solo con el puntero rojo** (el láser nunca se habilita) y repite en bucle
//!   hasta detener.
//! - **Sin ejes.** La tarjeta no mueve mesa ni rotativo: en el marcado dividido, repetido o
//!   rotativo ([`prepare`]) el hilo se detiene entre piezas, avisa a la interfaz
//!   ([`GalvoEvent::WaitMove`]) y espera que el usuario mueva el material y confirme
//!   ([`GalvoHandle::confirm_move`]).
//! - **La tarjeta no informa su posición.** Se cuenta la última orden de salto; tras cualquier
//!   trabajo se desconoce.
//!
//! Coordenadas: la posición de la máquina galvo es la del documento (mm, Y hacia arriba, origen
//! abajo a la izquierda de la cama; el campo de la lente está centrado en la cama). El origen del
//! perfil no se aplica.
//!
//! Máquina virtual: el puerto `virtual:galvo` usa el emulador de la tarjeta. Su reloj avanza con
//! cada consulta de estado, así que cada lectura duerme un poco para marcar el paso ([`Paced`]).

use super::{Emit, Sink};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tlaku_core::Point;
use tlaku_core::toolpath::Toolpath;
use tlaku_doc::{
    BufferStatus, ConnectResult, ConnectionEvent, DeviceKind, DeviceProfile, JobEvent, JobState,
    LaserSource, LogDir, MachineState, Origin, ProgressEvent, StatusReport,
};
use tlaku_galvo::compile::{self, FieldOrigin, LightAdjust, LightOptions, LightShape};
use tlaku_galvo::emu::{EmuConfig, emulator};
use tlaku_galvo::session::{
    BoardInfo, GalvoSession, JobControl, JobError, JobState as GalvoJobState, SessionConfig,
    SessionError, StopOutcome,
};
use tlaku_galvo::transport::{LaserSafetyAck, Transport, TransportError, TransportInfo};
use tlaku_galvo::usb::{self, UsbTimeouts, UsbTransport};
use tlaku_i18n::tr;
use tlaku_pipeline::{StageMove, job_mark_sequence};

/// Puerto de la máquina virtual galvo.
pub const VIRTUAL_PORT: &str = "virtual:galvo";

/// Prefijo de los puertos USB reales: `usb:galvo:<n>` es la tarjeta número `n` de la lista.
pub const USB_PREFIX: &str = "usb:galvo:";

/// Código estable (`tlaku_i18n::coded`) del mensaje de un trabajo o de una conexión cuando detener
/// NO se confirmó. La interfaz lo reconoce con `errorCode` (`ENGINE_ERROR.galvoStopUnconfirmed`)
/// para destacar el aviso de cortar la energía de la fuente; el texto cambia con el idioma, el
/// código no. Es el mismo que antepone `JobError::code()` en `tlaku-galvo`.
pub const STOP_UNCONFIRMED: &str = tlaku_galvo::session::STOP_UNCONFIRMED_CODE;

/// Aviso que acompaña a todo lo que toca una tarjeta real.
pub fn experimental() -> String {
    tr!("desktop.galvo.experimental")
}

/// Mensaje cuando una operación solo existe en GRBL.
pub fn grbl_only() -> String {
    tr!("desktop.galvo.grbl_only")
}

/// Mensaje cuando se pide pausar o reanudar: no existe, a propósito.
pub fn no_pause() -> String {
    tr!("desktop.galvo.no_pause")
}

/// ¿Es el puerto de la máquina virtual galvo (con o sin `@velocidad`)?
pub fn is_virtual(port: &str) -> bool {
    port.split('@').next() == Some(VIRTUAL_PORT)
}

/// Número de tarjeta de un puerto `usb:galvo:<n>`.
pub fn usb_index(port: &str) -> Option<usize> {
    port.strip_prefix(USB_PREFIX)?.parse().ok()
}

/// Puertos USB del galvo para la lista: una entrada por dispositivo BJJCZ (9588:xxxx) encontrado
/// (si no es compatible, abrirlo explica por qué). Un error al listar el USB no estropea la lista.
pub fn usb_ports() -> Vec<String> {
    usb::list_boards()
        .unwrap_or_default()
        .iter()
        .map(|b| format!("{USB_PREFIX}{}", b.index))
        .collect()
}

mod connection;
mod events;
mod handle;
mod prepare;
mod safety;
mod worker;

pub use connection::{GalvoConnection, GalvoVerifyReport, verify};
pub use events::{GalvoEvent, GalvoInterlock, GalvoState, GalvoTimingCfg};
pub use handle::GalvoHandle;
pub use prepare::*;
pub use safety::*;

// Lo que no es público se comparte entre los submódulos (y con las pruebas) por estos `use`.
use connection::*;
use handle::*;
use worker::*;

#[cfg(test)]
mod tests;
