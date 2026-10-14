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
//! Regla del silencio ("conexión perdida"). Con el sondeo activo, sin recibir ni un byte durante
//! `silence_timeout` (5 s) la conexión se da por perdida, porque en funcionamiento normal GRBL
//! contesta `?` en 5 a 20 ms. Pero hay dos casos en que el silencio es legítimo, y en ellos
//! no se declara la pérdida:
//!
//! - **Homing (`$H`)**: `?` puede no contestarse y el `ok` llega al terminar el ciclo (un
//!   minuto o más). Mientras haya un `$H` sin responder, el plazo es `home_timeout` (120 s).
//!   En el trabajo (`Streamer`) rige la misma regla si una de sus líneas es un `$H`.
//! - **Bloqueo crítico** tras `ALARM:1`/`ALARM:2` o `[MSG:Reset to continue]`: GRBL deja de
//!   atender `?` hasta un soft reset. No hay plazo: solo se sale al recibir un `ok` o el
//!   saludo (`Grbl ...`) tras el reset, y entretanto `?` se manda cada 2 s en vez de cada
//!   250 ms. Un cable arrancado sigue detectándose, porque la lectura o la escritura
//!   fallan con error de E/S (eso no depende del silencio).
//!
//! Fuera de esos dos casos el plazo no cambia.
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
    MachineState, OverrideKind, Overrides, ProgressEvent, RealtimeCommand, SimulatedFault,
    StatusReport,
};
use tlaku_grbl::link::{LinkFamily, LinkProfile};
use tlaku_grbl::preflight::{
    Advice, JobExpect, PreflightConfig, ReprapInfo, outcome_result_for, preflight_link,
    validate_gcode_for,
};
use tlaku_grbl::serial::{Grbl, LineReader, Transport};
use tlaku_grbl::streamer::{
    self, Event, Firmware, Job, Outcome, POSITION_LOST_NOTE, Realtime, Streamer, StreamerConfig,
    alarm_message_for, creality_error_message, error_message_for, is_greeting, is_homing_line,
    override_plan, parse_creality_error, prepare_line_with,
};

pub mod active;
pub mod extras;
pub mod frame;
pub mod galvo;
pub mod lhymicro;
pub mod ruida;
pub mod virtual_port;

mod connection;
mod emit;
mod handle;
mod worker;

/// Mensaje cuando no hay conexión (o se perdió).
pub const NOT_CONNECTED: &str = "No hay máquina conectada. Conéctala desde la pestaña Máquina.";

pub use connection::Connection;
pub use emit::{Emit, Sink, Timing};
pub use handle::Handle;

// Lo que no es público se comparte entre los submódulos (y con `extras` y las pruebas) por estos
// `use`, de modo que `super::nombre` sigue resolviéndose igual que antes de partir el archivo.
use connection::*;
use handle::*;
use worker::*;

#[cfg(test)]
mod tests;
