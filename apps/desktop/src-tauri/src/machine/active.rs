//! Despacho entre las conexiones posibles con una máquina.
//!
//! La app tiene **una** conexión activa ([`ActiveMachine`]) y todas hablan con la interfaz igual:
//! los eventos `machine://*` ([`super::Sink`]) y las mismas operaciones de control. Hoy hay dos
//! respaldos:
//!
//! - GRBL y derivados ([`super::Connection`], trabajador sobre un puerto serie o virtual);
//! - Ruida por UDP ([`super::ruida::RuidaConnection`]);
//! - el K40 con placa M2 Nano por USB ([`super::lhymicro::LhyConnection`]).
//!
//! # Cómo se agrega otro respaldo (así se hizo con el K40, LHYMICRO)
//!
//! 1. Una variante en [`ActiveMachine`] (dueña del hilo, con `close`) y otra en [`Machine`] (el
//!    mando clonable, `Send`).
//! 2. El compilador marca cada `match` incompleto de este archivo: es la lista de operaciones
//!    comunes que el respaldo debe resolver (`is_closed`, `work_pos`, `jog`,
//!    `jog_to`, pausa, reanudar, cancelar y `realtime` si tiene equivalentes). Lo que no tenga
//!    sentido devuelve un error en español con el motivo, como hace Ruida con la consola de G-code.
//! 3. En `commands.rs`, `connect` crea la variante nueva según `device.kind`. Enviar y encuadrar
//!    un trabajo (`start_job`, `frame_job`, `frame_hull`) calculan cosas distintas en cada
//!    respaldo (G-code, `.rd`, LHYMICRO): ahí se despacha con `if let Machine::Ruida(r)` antes de
//!    la ruta GRBL, y el K40 agrega la suya igual (`RuidaHandle::start_job` es el modelo).
//! 4. Las operaciones propias del respaldo (como la lista de archivos de Ruida) se piden con un
//!    accesor tipado ([`Machine::ruida`]) que falla con un mensaje claro si la conexión es otra.
//!
//! Las operaciones que solo existen en GRBL (consola, overrides, ajustes `$`, macros) usan
//! [`Machine::grbl`] y no se tocan.

use super::lhymicro::{self, LhyConnection, LhyHandle};
use super::ruida::{self, RuidaConnection, RuidaHandle};
use super::{Connection, Handle};
use tlaku_doc::{RealtimeCommand, StatusReport};

/// La conexión activa de la app, dueña del hilo del respaldo.
pub enum ActiveMachine {
    Grbl(Connection),
    Ruida(RuidaConnection),
    Lhy(LhyConnection),
}

impl ActiveMachine {
    /// Mando clonable de la conexión.
    pub fn handle(&self) -> Machine {
        match self {
            Self::Grbl(c) => Machine::Grbl(c.handle()),
            Self::Ruida(c) => Machine::Ruida(c.handle()),
            Self::Lhy(c) => Machine::Lhy(c.handle()),
        }
    }

    /// Cierra la conexión (cancelando antes un trabajo en curso) y espera al hilo.
    pub fn close(self) {
        match self {
            Self::Grbl(c) => c.close(),
            Self::Ruida(c) => c.close(),
            Self::Lhy(c) => c.close(),
        }
    }
}

/// Mando de la conexión activa: lo que usan los comandos de Tauri.
#[derive(Clone)]
pub enum Machine {
    Grbl(Handle),
    Ruida(RuidaHandle),
    Lhy(LhyHandle),
}

impl Machine {
    /// `true` si la conexión terminó (a petición o por un fallo).
    pub fn is_closed(&self) -> bool {
        match self {
            Self::Grbl(h) => h.is_closed(),
            Self::Ruida(h) => h.is_closed(),
            Self::Lhy(h) => h.is_closed(),
        }
    }

    /// Posición del cabezal del último reporte, en coordenadas de la máquina del perfil.
    pub fn work_pos(&self) -> Option<[f64; 2]> {
        match self {
            Self::Grbl(h) => h.work_pos(),
            Self::Ruida(h) => h.work_pos(),
            Self::Lhy(h) => h.work_pos(),
        }
    }

    /// Último reporte de estado completo, de cualquier respaldo.
    pub fn status_report(&self) -> Option<StatusReport> {
        match self {
            Self::Grbl(h) => h.status_report(),
            Self::Ruida(h) => h.status_report(),
            Self::Lhy(h) => h.status_report(),
        }
    }

    /// El mando GRBL, o un error que explica por qué la operación no aplica a esta máquina.
    pub fn grbl(&self) -> Result<&Handle, String> {
        match self {
            Self::Grbl(h) => Ok(h),
            Self::Ruida(_) => Err(ruida::GRBL_ONLY.into()),
            Self::Lhy(_) => Err(lhymicro::GRBL_ONLY.into()),
        }
    }

    /// El mando Ruida, o un error si la conexión activa es otra.
    pub fn ruida(&self) -> Result<&RuidaHandle, String> {
        match self {
            Self::Ruida(h) => Ok(h),
            Self::Grbl(_) | Self::Lhy(_) => {
                Err("Esta función es solo de las controladoras Ruida.".into())
            }
        }
    }

    /// El mando del K40 (M2 Nano), o un error si la conexión activa es otra.
    pub fn lhy(&self) -> Result<&LhyHandle, String> {
        match self {
            Self::Lhy(h) => Ok(h),
            Self::Grbl(_) | Self::Ruida(_) => {
                Err("Esta función es solo del K40 con placa M2 Nano.".into())
            }
        }
    }

    /// Movimiento relativo en mm a `speed` mm/min (láser apagado).
    pub fn jog(&self, dx: f64, dy: f64, speed: f64) -> Result<(), String> {
        match self {
            Self::Grbl(h) => h.jog(dx, dy, speed),
            Self::Ruida(h) => h.jog(dx, dy, speed),
            Self::Lhy(h) => h.jog(dx, dy, speed),
        }
    }

    /// Movimiento absoluto en coordenadas de la máquina del perfil (láser apagado).
    pub fn jog_to(&self, x: f64, y: f64, speed: f64) -> Result<(), String> {
        match self {
            Self::Grbl(h) => h.jog_to(x, y, speed),
            Self::Ruida(h) => h.jog_to(x, y, speed),
            Self::Lhy(h) => h.jog_to(x, y, speed),
        }
    }

    pub fn pause(&self) -> Result<(), String> {
        match self {
            Self::Grbl(h) => h.pause(),
            Self::Ruida(h) => h.pause(),
            Self::Lhy(h) => h.pause(),
        }
    }

    pub fn resume(&self) -> Result<(), String> {
        match self {
            Self::Grbl(h) => h.resume(),
            Self::Ruida(h) => h.resume(),
            Self::Lhy(h) => h.resume(),
        }
    }

    /// Cancela el trabajo (o el movimiento en curso).
    pub fn cancel(&self) -> Result<(), String> {
        match self {
            Self::Grbl(h) => h.cancel(),
            Self::Ruida(h) => h.cancel(),
            Self::Lhy(h) => h.cancel(),
        }
    }

    /// Comandos en tiempo real. GRBL los manda tal cual; en Ruida los que tienen equivalente
    /// (pausa, reanudar, detener) se traducen y el resto no aplica.
    pub fn realtime(&self, cmd: RealtimeCommand) -> Result<(), String> {
        match self {
            Self::Grbl(h) => h.realtime(cmd),
            Self::Ruida(h) => match cmd {
                RealtimeCommand::Hold => h.pause(),
                RealtimeCommand::Resume => h.resume(),
                RealtimeCommand::Reset | RealtimeCommand::JogCancel => h.cancel(),
                // Ruida no tiene un reporte a petición: el estado llega solo cada ~500 ms.
                RealtimeCommand::Status => Ok(()),
            },
            // El K40 traduce lo mismo que Ruida; la placa se consulta sola cada ~500 ms. Cancelar un
            // jog no existe: los saltos son cortos y detener la placa le haría perder la posición.
            Self::Lhy(h) => match cmd {
                RealtimeCommand::Hold => h.pause(),
                RealtimeCommand::Resume => h.resume(),
                RealtimeCommand::Reset => h.cancel(),
                RealtimeCommand::JogCancel | RealtimeCommand::Status => Ok(()),
            },
        }
    }
}
