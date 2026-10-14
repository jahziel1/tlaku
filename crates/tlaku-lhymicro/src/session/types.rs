//! Errores y resultados de la sesión.

use crate::encode::{EncodeError, Warning};
use crate::packet::FrameError;
use crate::status::{Status, StatusReport};
use crate::transport::{TransportError, TransportInfo};
use std::fmt;
use std::time::Duration;
use tlaku_core::Point;

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
            SessionError::Timeout { what } => write!(f, "sin respuesta de la placa ({what})"),
            SessionError::CrcRepeated { packet } => write!(
                f,
                "la placa rechazó el paquete {} por CRC varias veces seguidas (¿cable USB \
                 defectuoso o interferencias?)",
                packet + 1
            ),
            SessionError::LowPower => write!(
                f,
                "la placa informa energía insuficiente (239): revisa el paro de emergencia, la \
                 tapa y que la fuente del láser esté encendida"
            ),
            SessionError::NotM2Nano { code } => write!(
                f,
                "esta placa responde con el código {code}, que no existe en la M2 Nano: parece \
                 una Moshiboard (mismo USB 1A86:5512). Tlaku no la soporta todavía"
            ),
            SessionError::NoStatus(s) => write!(
                f,
                "la placa no devuelve un estado utilizable ({})",
                s.describe()
            ),
            SessionError::Busy(s) => write!(f, "la placa no está en reposo ({})", s.describe()),
            SessionError::UnknownPosition => write!(
                f,
                "no se sabe dónde está el cabezal: haz home primero (la M2 Nano no informa su \
                 posición)"
            ),
            SessionError::StartMismatch { expected, found } => write!(
                f,
                "el trabajo se codificó desde {expected:?} mil y el cabezal está en {found:?}"
            ),
            SessionError::OutOfBed { x_mm, y_mm } => {
                write!(f, "({x_mm:.2}, {y_mm:.2}) mm queda fuera de la cama")
            }
            SessionError::JobActive => write!(f, "ya hay un trabajo en marcha"),
            SessionError::Encode(e) => write!(f, "{e}"),
            SessionError::Frame(e) => write!(f, "{e}"),
            SessionError::BadProgram(m) => write!(f, "{m}"),
            SessionError::Cancelled => write!(f, "cancelado por el usuario"),
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
        write!(
            f,
            "{} (paquetes confirmados: {} de {})",
            self.cause, self.packets_sent, self.packets_total
        )?;
        match self.stop {
            StopOutcome::Acked => write!(f, ". Se detuvo la placa y se apagó el láser"),
            StopOutcome::Unconfirmed => write!(
                f,
                ". Se intentó detener la placa y apagar el láser pero NO hubo confirmación: \
                 puede seguir en marcha. Usa el paro de emergencia o abre la tapa"
            ),
        }
    }
}

impl std::error::Error for JobError {}
