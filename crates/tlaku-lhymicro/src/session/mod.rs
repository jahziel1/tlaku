//! Sesión con una placa M2 Nano: sondeo, envío de un trabajo con progreso, pausa, reanudación,
//! detención, saltos y home.
//!
//! # Protocolo (fuentes: MeerK40t `lihuiyu/controller.py`, MIT; ver [`crate::status`])
//!
//! - Antes de cada paquete se consulta el estado hasta que la placa lo acepta (206, o 236 si ya
//!   terminó); se escribe el paquete (34 bytes con CRC) y se consulta hasta que la placa lo
//!   confirma. 207 es CRC malo: se reenvía el mismo paquete. 238 es "ocupada": se espera.
//! - Un `-` en el flujo obliga a esperar el estado 236 ("terminó") antes de seguir.
//! - La pausa es `PN` en tiempo real (la misma orden pausa y reanuda; inferido) y la detención es
//!   `I` seguido de `IUS1P` (cancelar y apagar el láser).
//! - La placa **no informa de su posición**: la sesión la lleva por conteo desde el home. Sin
//!   home (o tras detener o fallar un trabajo) la posición es desconocida y no se envía ningún
//!   movimiento hasta volver a hacer home o confirmar con [`LhySession::assume_at_home`].
//!
//! # Seguridad
//!
//! - **Ningún paquete sin confirmación de estado listo**: se espera a 206/236 antes de escribir.
//! - Ante un error de USB, un CRC repetido, un tiempo agotado, la falta de energía (239) o un
//!   estado inutilizable, el trabajo se aborta y se intenta apagar el láser y detener la placa
//!   (`I`, `IUS1P`). El resultado ([`StopOutcome`]) dice si la placa lo confirmó; si no, hay que
//!   usar el paro de emergencia o abrir la tapa.
//! - Los límites de la cama y de velocidad se comprueban al codificar ([`crate::encode`]) y en
//!   los saltos.
//!
//! # Uso desde otro hilo
//!
//! [`LhySession::send_job`] bloquea mientras dura el trabajo (la placa recibe los datos a
//! medida que los ejecuta). Para pausar, reanudar o detener hay que usar el [`JobControl`] que
//! da [`LhySession::control`], que se puede clonar y mover a otro hilo:
//!
//! ```ignore
//! let control = session.control();
//! std::thread::spawn(move || session.send_job(&program, &mut |p| println!("{}/{}", p.packet, p.packets)));
//! control.request_pause();
//! control.request_resume();
//! control.request_stop();
//! ```

mod config;
mod control;
mod job;
mod motion;
mod packets;
mod poll;
mod realtime;
#[cfg(test)]
mod tests;
mod types;

use crate::encode::{EncodeConfig, Program, encode, mil_to_mm};
use crate::packet::Frame;
use crate::transport::Transport;
pub use config::SessionConfig;
pub use control::{JobControl, JobState};
use std::time::Duration;
use tlaku_core::Point;
use tlaku_core::toolpath::Toolpath;
pub use types::{BoardInfo, JobError, JobReport, Progress, SessionError, StopOutcome};

/// Duración a partir de segundos, sin entrar en pánico con valores no finitos o negativos.
fn secs(x: f64, fallback: Duration) -> Duration {
    Duration::try_from_secs_f64(x).unwrap_or(fallback)
}

enum Confirm {
    Accepted,
    CrcRejected,
}

enum Step {
    Done,
    Continue,
}

pub struct LhySession<T: Transport> {
    transport: T,
    cfg: SessionConfig,
    control: JobControl,
    /// Posición del cabezal en mils (ejes de la placa) o `None` si no se sabe.
    pos: Option<(i64, i64)>,
    /// La pausa ya se mandó a la placa.
    pause_applied: bool,
    /// Último paquete confirmado, por si la placa avisa tarde de un CRC malo.
    last_confirmed: Option<Frame>,
    resends: usize,
}

impl<T: Transport> LhySession<T> {
    pub fn new(transport: T, cfg: SessionConfig) -> Self {
        LhySession {
            transport,
            cfg,
            control: JobControl::new(),
            pos: None,
            pause_applied: false,
            last_confirmed: None,
            resends: 0,
        }
    }

    pub fn config(&self) -> &SessionConfig {
        &self.cfg
    }

    /// Mando para pausar, reanudar o detener un trabajo desde otro hilo.
    pub fn control(&self) -> JobControl {
        self.control.clone()
    }

    pub fn into_transport(self) -> T {
        self.transport
    }

    /// Posición del cabezal (mils, ejes de la placa), si se sabe.
    pub fn position_mil(&self) -> Option<(i64, i64)> {
        self.pos
    }

    /// Posición del cabezal en el sistema de Tlaku (mm), si se sabe.
    pub fn position_mm(&self) -> Option<Point> {
        self.pos.map(|(x, y)| {
            self.cfg
                .home
                .from_machine_mm((mil_to_mm(x), mil_to_mm(y)), self.cfg.bed_mm)
        })
    }

    /// El usuario asegura que el cabezal está en el home (sin moverlo).
    pub fn assume_at_home(&mut self) {
        self.pos = Some((0, 0));
    }

    /// Configuración del codificador para el punto de partida actual.
    pub fn encode_config(&self) -> Result<EncodeConfig, SessionError> {
        let start = self.pos.ok_or(SessionError::UnknownPosition)?;
        Ok(EncodeConfig {
            board: self.cfg.board,
            bed_mm: self.cfg.bed_mm,
            home: self.cfg.home,
            start_mil: start,
            autolock: self.cfg.autolock,
            max_speed_mm_s: self.cfg.max_speed_mm_s,
            constant_speed: self.cfg.constant_speed,
            fix_speeds: self.cfg.fix_speeds,
            ..EncodeConfig::default()
        })
    }

    /// Codifica un trabajo desde la posición actual del cabezal.
    pub fn encode(&self, toolpath: &Toolpath) -> Result<Program, SessionError> {
        Ok(encode(toolpath, &self.encode_config()?)?)
    }
}
