//! Envío de trabajos a GRBL 1.1 con conteo de caracteres, comandos en tiempo
//! real y parser del reporte de estado; con un [`LinkProfile`] de Marlin o Smoothieware
//! envía en ping-pong (una línea por `ok`).
//!
//! El conteo de caracteres lleva los bytes que cada línea ocupa en el búfer de
//! recepción de la controladora y solo envía otra mientras quepa; cada `ok` o
//! `error:N` libera la línea más antigua. Así el búfer se mantiene lleno sin
//! desbordarse (a diferencia del modo "una línea por vez", que se entrecorta).
//!
//! Qué cambia entre familias (control de flujo, consulta de estado, pausa, cancelación, fin
//! del trabajo, saludo de reinicio) lo decide el [`LinkProfile`]; `Streamer::new` usa el de
//! GRBL y se comporta como siempre. Para otro firmware: [`Streamer::with_link`] o
//! [`Streamer::for_device`].

mod config;
mod events;
mod firmware;
mod incoming;
mod lines;
mod realtime;
mod run;
mod status;
mod stop;
mod tick;

#[cfg(test)]
mod tests;

pub use config::*;
pub use events::*;
pub use firmware::*;
pub use incoming::*;
pub use lines::*;
pub use realtime::*;
pub use status::*;
pub use stop::*;

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tlaku_i18n::tr;

use crate::link::LinkProfile;
use crate::serial::{LineReader, Transport};

/// Espera máxima de cada lectura; acota la latencia de pausa, cancelación y sondeo.
const TICK: Duration = Duration::from_millis(5);

/// Si escribir (más el sondeo) tarda esto o más en una vuelta, el anfitrión estuvo bloqueado y
/// no puede contar ese lapso como silencio de la controladora.
const WRITE_STALL: Duration = Duration::from_millis(250);

#[derive(Debug)]
enum Command {
    Pause,
    Resume,
    Cancel,
    Realtime(Realtime),
}

/// Mando remoto de un trabajo en curso; se puede clonar y usar desde otro hilo.
/// Los envíos a un `Streamer` ya destruido se ignoran.
#[derive(Clone)]
pub struct Control {
    tx: Sender<Command>,
}

impl Control {
    /// Feed hold y dejar de enviar líneas nuevas.
    pub fn pause(&self) {
        let _ = self.tx.send(Command::Pause);
    }

    /// Reanuda (`~`) y vuelve a enviar. No hace nada si no estaba en pausa.
    pub fn resume(&self) {
        let _ = self.tx.send(Command::Resume);
    }

    /// Feed hold, soft reset y fin del trabajo.
    pub fn cancel(&self) {
        let _ = self.tx.send(Command::Cancel);
    }

    /// Byte en tiempo real (overrides, jog cancel...). `SoftReset` también
    /// termina el trabajo como cancelado.
    pub fn realtime(&self, cmd: Realtime) {
        let _ = self.tx.send(Command::Realtime(cmd));
    }
}

/// Trabajo lanzado en su propio hilo con [`Streamer::spawn`].
pub struct Job<T: Transport> {
    pub control: Control,
    pub events: Receiver<Event>,
    handle: JoinHandle<(Streamer<T>, Outcome)>,
}

impl<T: Transport> Job<T> {
    /// Espera el fin del trabajo y devuelve el streamer (con su transporte) y el resultado.
    pub fn join(self) -> thread::Result<(Streamer<T>, Outcome)> {
        self.handle.join()
    }
}

pub struct Streamer<T: Transport> {
    transport: T,
    config: StreamerConfig,
    link: LinkProfile,
    cmd_tx: Sender<Command>,
    cmd_rx: Receiver<Command>,
    reader: LineReader,
    stop: Option<StopInfo>,
}

/// Qué pasó al detener la máquina (cancelación, `error:N` con la política `Stop`, falla de
/// Creality o reset pedido por el usuario).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StopInfo {
    /// Se vio la máquina quieta (`Hold:0`, `Idle`...) antes del soft reset.
    pub settled: bool,
    /// El reset llegó con la máquina posiblemente en movimiento (no se llegó a ver quieta) o
    /// la controladora respondió `ALARM:3` tras él: la posición puede haberse perdido.
    pub position_maybe_lost: bool,
    /// `ALARM:N` recibido durante la parada o justo después del reset.
    pub alarm: Option<u32>,
}

/// Aviso para el usuario cuando [`StopInfo::position_maybe_lost`].
pub fn position_lost_note() -> String {
    tr!("grbl.stream.position_lost_note")
}

struct RunState {
    next: usize,
    acked: usize,
    errors: usize,
    used: usize,
    /// (índice en el trabajo, bytes) de las líneas enviadas sin responder.
    pending: VecDeque<(usize, usize)>,
    paused: bool,
    idle_seen: bool,
    last_poll: Option<Instant>,
    last_rx: Instant,
    /// Último estado informado por la máquina.
    last_state: Option<MachineState>,
    /// Líneas propias del streamer al inicio del trabajo (no cuentan en el avance).
    hidden: usize,
    /// Ya se envió la orden de cierre (`M400`) y se espera su `ok`.
    sync_sent: bool,
    /// Tras un `Resend:` no se envía hasta el `ok` que lo acompaña (o hasta este instante).
    resend_until: Option<Instant>,
    /// Reenvíos seguidos sin ningún `ok` entre medias.
    resends: u32,
}

struct Prepared {
    /// Texto sin el `\n` final.
    text: String,
    /// Número de línea (desde 1) en la entrada original.
    source_line: usize,
    /// Escribe en la EEPROM: se envía sola y se espera su `ok` antes de seguir (ver
    /// [`writes_eeprom`]).
    sync: bool,
    /// Línea del propio streamer (`M110 N0` de las líneas numeradas): no cuenta en el avance.
    silent: bool,
}

/// Marca de las líneas pendientes que no son del trabajo (`M0`, `M108`, `M400` de cierre...):
/// su `ok` libera el búfer pero no cuenta como avance.
const EXTRA: usize = usize::MAX;
