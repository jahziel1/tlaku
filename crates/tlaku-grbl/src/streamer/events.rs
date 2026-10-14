//! Eventos que el [`Streamer`] informa y resultado final del trabajo.

use super::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Una línea fue respondida. `sent` incluye las aún sin confirmar y
    /// `buffer_used` los bytes que se cree ocupados en la controladora.
    Progress {
        acked: usize,
        sent: usize,
        total: usize,
        buffer_used: usize,
    },
    Status(Box<StatusReport>),
    /// `error:N` de GRBL. `source_line` y `text` son `None` si no hay línea pendiente a la que atribuirlo.
    Error {
        code: u32,
        source_line: Option<usize>,
        text: Option<String>,
    },
    Alarm(u32),
    /// Mensaje `[...]` u otra línea no reconocida.
    Message(String),
    /// La controladora se reinició (saludo `Grbl ...`).
    Reset(String),
    Finished(Outcome),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Todas las líneas se respondieron; `errors` cuenta los `error:N` tolerados.
    Completed {
        errors: usize,
    },
    /// Cancelado: feed hold, espera a que frene y soft reset (ver [`Streamer::last_stop`]).
    Cancelled,
    /// `error:N` con la política `Stop`. Antes de terminar se detuvo la máquina (feed hold,
    /// espera y soft reset).
    LineError {
        code: u32,
        source_line: usize,
    },
    /// Falla propia de Creality (`ERROR:NN`, p. ej. llama o inclinación). No es una línea
    /// rechazada: la máquina se detuvo por seguridad. El trabajo se detuvo igual que con
    /// `LineError`. El significado está en [`creality_error_message`].
    MachineFault(u32),
    Alarm(u32),
    /// La controladora se reinició a mitad del trabajo.
    Reset(String),
    /// Fallo de E/S o silencio prolongado.
    Disconnected(String),
}
