//! Conexión con la máquina GRBL. (La de Ruida vive en [`ruida`] y el despacho entre respaldos, en
//! [`active`].)
//!
//! Un hilo dedicado ("trabajador") es el único dueño del puerto serie. Los comandos de la
//! interfaz llegan por un canal ([`Handle`]) y el trabajador:
//!
//! - en reposo, sondea `?` cada ~250 ms, emite `machine://status` y registra lo que llegue;
//! - al empezar un trabajo, comprueba el estado (misma lógica que `tlaku send`) y le
//!   entrega el puerto a un `Streamer` (conteo de caracteres) en su propio hilo; mientras tanto
//!   traduce sus eventos a progreso, estado y registro, y atiende pausa, reanudación y cancelación;
//! - al terminar el trabajo recupera el puerto y vuelve a sondear;
//! - si el puerto falla o la controladora calla, emite `machine://connection` con el error y termina.
//!
//! Los reportes de estado (`?`) no se registran en `machine://log`: solo llegan como estado.
//!
//! Máquinas virtuales ([`virtual_port`]). Un puerto `virtual:<perfil>` abre un
//! `tlaku_emu::VirtualMachine` con reloj de pared en lugar del puerto serie; el resto (estado,
//! envío, pausa, jog, consola) corre exactamente igual. [`Handle::simulate_fault`] provoca fallas
//! (llama, alarma 3, desconexión) y solo existe con una máquina virtual.
//!
//! Regla del silencio. **El silencio de la placa nunca da la conexión por perdida**, ni en reposo
//! ni durante un trabajo (igual que LightBurn, que simplemente espera): la Falcon2 se para 2–20 s
//! detrás de un concentrador USB y sigue sola, y declarar la pérdida no detiene la máquina (con la
//! placa trabada no se le puede mandar nada), solo desconecta y molesta. La conexión solo se
//! pierde por un error real de E/S del sistema (puerto desaparecido, cable, máquina apagada: el
//! error de lectura o escritura del SO) o porque el usuario desconecta. Mientras no contesta:
//!
//! - A los 2 s (`Timing::silence_notice`) el trabajador emite `machine://silence`
//!   (`silence_ms`, cada segundo entero; `0` = volvió a contestar) y la interfaz muestra «la
//!   máquina no contesta desde hace N s…» con Detener y Desconectar. Durante un trabajo lo mide
//!   el `Streamer` y el trabajador solo lo reenvía. No se avisa en el homing (`$H`: `?` puede no
//!   contestarse y el `ok` llega al terminar el ciclo, un minuto o más) ni tras `ALARM:1`/`ALARM:2`
//!   o `[MSG:Reset to continue]` (GRBL deja de atender `?` hasta un soft reset; entonces `?` se
//!   manda cada 2 s en vez de cada 250 ms).
//! - Nada se manda solo por el silencio (ni `!`): detener es decisión del usuario (feed hold y
//!   reset, como siempre). Las escrituras al puerto serie esperan sin límite mientras no haya un
//!   error real; Detener y Desconectar las abandonan al instante (`Transport::set_abort`).
//!
//! Nada aquí depende de Tauri: los eventos salen por [`Sink`], de modo que se prueba con
//! `memory_pair` sin hardware.

use std::collections::VecDeque;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tlaku_doc::{
    BufferStatus, ConnectResult, ConnectionEvent, JobEvent, JobState, LogDir, LogEvent,
    MachineState, OverrideKind, Overrides, ProgressEvent, RealtimeCommand, SilenceEvent,
    SimulatedFault, StatusReport,
};
use tlaku_grbl::link::{LinkFamily, LinkProfile};
use tlaku_grbl::preflight::{
    Advice, JobExpect, LinkLoad, PreflightConfig, ReprapInfo, outcome_result_for,
    preflight_link_with_load, validate_gcode_for,
};
use tlaku_grbl::serial::{Grbl, LineReader, Transport};
use tlaku_grbl::streamer::{
    self, Event, Firmware, Job, Outcome, Realtime, Streamer, StreamerConfig, alarm_message_for,
    creality_error_message, error_message_for, is_boot_banner, is_greeting, is_homing_line,
    override_plan, parse_creality_error, position_lost_note, prepare_line_with,
};

pub mod active;
pub mod extras;
pub mod frame;
pub mod galvo;
pub mod lhymicro;
pub mod probe;
pub mod resume;
pub mod ruida;
pub mod virtual_port;

mod common;
mod connection;
mod emit;
mod handle;
mod worker;

/// Mensaje cuando no hay conexión (o se perdió).
pub fn not_connected() -> String {
    tlaku_i18n::tr!("desktop.machine.not_connected")
}

pub use connection::Connection;
pub use emit::{Emit, Sink, Timing};
pub use handle::Handle;

// Lo que no es público se comparte entre los submódulos (y con `extras` y las pruebas) por estos
// `use`, de modo que `super::nombre` sigue resolviéndose igual que antes de partir el archivo.
use common::{frame_cancelled_log, frame_done_log};
use connection::*;
use handle::*;
use worker::*;

#[cfg(test)]
mod tests;
