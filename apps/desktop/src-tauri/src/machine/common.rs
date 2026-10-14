//! Piezas que repiten los controladores de la app (Ruida, K40 y galvo): el registro de la
//! consola, la velocidad de las máquinas virtuales y el transporte con ritmo.

use std::thread;
use std::time::Duration;

use tlaku_doc::{LogDir, LogEvent};

use super::{Emit, Sink};

/// Anota una línea en el registro de la consola (`machine://log`).
pub(super) fn log(sink: &dyn Sink, dir: LogDir, text: &str) {
    sink.emit(Emit::Log(LogEvent {
        dir,
        text: text.to_string(),
    }));
}

/// `virtual:k40@5` -> 5 (veces el tiempo real); sin `@velocidad`, 1. Es un error que no sea un
/// número entre 0 (excluido) y [`virtual_port::MAX_SPEED`](super::virtual_port::MAX_SPEED).
pub(super) fn virtual_speed(port: &str) -> Result<f64, String> {
    match port.split_once('@') {
        None => Ok(1.0),
        Some((_, s)) => match s.parse::<f64>() {
            Ok(v) if v.is_finite() && v > 0.0 && v <= super::virtual_port::MAX_SPEED => Ok(v),
            _ => Err(format!(
                "La velocidad '{s}' de la máquina virtual debe ser un número entre 0 y {} veces \
                 el tiempo real.",
                super::virtual_port::MAX_SPEED
            )),
        },
    }
}

/// Transporte que duerme `step` en cada lectura: marca el paso del reloj virtual del emulador
/// (que avanza una vez por consulta de estado) contra el reloj de pared. Cada controlador
/// implementa su propio `Transport` sobre este tipo y llama a [`Paced::pace`] antes de leer
/// (los `Transport` de K40 y galvo son traits distintos).
pub(super) struct Paced<T> {
    pub(super) inner: T,
    pub(super) step: Duration,
}

impl<T> Paced<T> {
    /// Espera un paso del reloj virtual.
    pub(super) fn pace(&self) {
        thread::sleep(self.step);
    }
}
