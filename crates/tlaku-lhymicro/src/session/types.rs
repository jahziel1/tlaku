//! Errores y resultados de la sesión.

use crate::encode::{EncodeError, Warning};
use crate::packet::FrameError;
use crate::status::{Status, StatusReport};
use crate::text;
use crate::transport::{TransportError, TransportInfo};
use std::fmt;
use std::time::Duration;
use tlaku_core::Point;
use tlaku_i18n::{Arg, tr};

#[derive(Debug)]
pub enum SessionError {
    Transport(TransportError),
    Timeout {
        what: &'static str,
    },
    /// Tras los reenvíos, la placa sigue rechazando el paquete por CRC.
    CrcRepeated {
        packet: usize,
    },
    /// La placa informa 239: sin energía suficiente.
    LowPower,
    /// El estado delata una Moshiboard, que no es una M2 Nano.
    NotM2Nano {
        code: u8,
    },
    /// Estados inutilizables seguidos (0, truncados o desconocidos).
    NoStatus(Status),
    /// La placa no está en reposo.
    Busy(Status),
    /// No se sabe dónde está el cabezal: hay que hacer home.
    UnknownPosition,
    /// El programa se codificó para otro punto de partida o placa.
    StartMismatch {
        expected: (i64, i64),
        found: (i64, i64),
    },
    OutOfBed {
        x_mm: f64,
        y_mm: f64,
    },
    /// Ya hay un trabajo en marcha.
    JobActive,
    Encode(EncodeError),
    Frame(FrameError),
    BadProgram(&'static str),
    /// El usuario detuvo el trabajo.
    Cancelled,
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionError::Transport(e) => write!(f, "{e}"),
            SessionError::Timeout { what } => {
                f.write_str(&tr!("lhymicro.session.timeout", what = text::what(what)))
            }
            SessionError::CrcRepeated { packet } => {
                f.write_str(&tr!("lhymicro.session.crc_repeated", packet = packet + 1))
            }
            SessionError::LowPower => f.write_str(&tr!("lhymicro.session.low_power")),
            SessionError::NotM2Nano { code } => {
                f.write_str(&tr!("lhymicro.session.not_m2nano", code = code))
            }
            SessionError::NoStatus(s) => {
                f.write_str(&tr!("lhymicro.session.no_status", state = s.describe()))
            }
            SessionError::Busy(s) => {
                f.write_str(&tr!("lhymicro.session.busy", state = s.describe()))
            }
            SessionError::UnknownPosition => f.write_str(&tr!("lhymicro.session.unknown_position")),
            SessionError::StartMismatch { expected, found } => f.write_str(&tr!(
                "lhymicro.session.start_mismatch",
                expected = format!("{expected:?}"),
                found = format!("{found:?}")
            )),
            SessionError::OutOfBed { x_mm, y_mm } => f.write_str(&tr!(
                "lhymicro.session.out_of_bed",
                x = format!("{x_mm:.2}"),
                y = format!("{y_mm:.2}")
            )),
            SessionError::JobActive => f.write_str(&tr!("lhymicro.session.job_active")),
            SessionError::Encode(e) => write!(f, "{e}"),
            SessionError::Frame(e) => write!(f, "{e}"),
            SessionError::BadProgram(m) => f.write_str(&text::what(m)),
            SessionError::Cancelled => f.write_str(&tr!("lhymicro.session.cancelled")),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<TransportError> for SessionError {
    fn from(e: TransportError) -> Self {
        SessionError::Transport(e)
    }
}

impl From<EncodeError> for SessionError {
    fn from(e: EncodeError) -> Self {
        SessionError::Encode(e)
    }
}

impl From<FrameError> for SessionError {
    fn from(e: FrameError) -> Self {
        SessionError::Frame(e)
    }
}

/// Lo que se sabe de la placa tras el sondeo.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardInfo {
    pub status: StatusReport,
    pub transport: TransportInfo,
    /// Acepta paquetes ahora mismo.
    pub ready: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub packet: usize,
    pub packets: usize,
    pub bytes_sent: usize,
    pub bytes_total: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JobReport {
    pub packets: usize,
    pub bytes: usize,
    /// Paquetes reenviados por CRC.
    pub resends: usize,
    /// Se vio el estado 236 al final (el trabajo terminó con confirmación de la placa).
    pub finish_confirmed: bool,
    pub elapsed: Duration,
    pub warnings: Vec<Warning>,
    /// Posición final del cabezal (mm, ejes de Tlaku).
    pub end_mm: Point,
}

/// Qué pasó con el intento de detener la placa tras un fallo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopOutcome {
    /// La placa confirmó `I` y `IUS1P`.
    Acked,
    /// No hubo confirmación: la placa puede seguir moviéndose o con el láser encendido; usa el
    /// paro de emergencia o abre la tapa.
    Unconfirmed,
}

/// El trabajo falló o se detuvo. La posición del cabezal queda desconocida.
#[derive(Debug)]
pub struct JobError {
    pub cause: SessionError,
    pub packets_sent: usize,
    pub packets_total: usize,
    pub stop: StopOutcome,
}

impl JobError {
    /// El láser pudo quedar encendido (no se confirmó el apagado).
    pub fn laser_may_be_on(&self) -> bool {
        self.stop == StopOutcome::Unconfirmed
    }
}

impl fmt::Display for JobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key = match self.stop {
            StopOutcome::Acked => "lhymicro.session.job_failed_stopped",
            StopOutcome::Unconfirmed => "lhymicro.session.job_failed_unconfirmed",
        };
        let args = [
            ("cause", Arg::new(&self.cause)),
            ("sent", Arg::new(&self.packets_sent)),
            ("total", Arg::new(&self.packets_total)),
        ];
        f.write_str(&tlaku_i18n::translate(key, &args))
    }
}

impl std::error::Error for JobError {}
