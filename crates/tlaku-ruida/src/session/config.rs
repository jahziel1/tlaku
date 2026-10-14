//! Configuración de la sesión, direcciones de memoria y errores.

use super::types::MachineStatus;
use crate::packet::{self, PacketError, Response};
use crate::rd::HomeCorner;
use crate::safety::{SafetyError, SafetyLimits};
use crate::swizzle::Magic;
use std::time::Duration;
use std::{fmt, io};

/// Direcciones de memoria de solo lectura que usa la sesión (ruida.md sección 4.4).
pub mod addr {
    pub const CARD_ID: u16 = 0x2FE;
    pub const STATUS: u16 = 0x200;
    pub const POS_X: u16 = 0x221;
    pub const POS_Y: u16 = 0x231;
    pub const BED_X: u16 = 0x026;
    pub const BED_Y: u16 = 0x036;
    /// Velocidad del movimiento rápido (G0), µm/s según el emulador de MeerK40t.
    pub const G0_SPEED: u16 = 0x005;
    /// Velocidad de home, µm/s.
    pub const HOME_SPEED: u16 = 0x00C;
    /// Velocidad máxima de los ejes X e Y, µm/s.
    pub const AXIS_MAX_SPEED_X: u16 = 0x023;
    pub const AXIS_MAX_SPEED_Y: u16 = 0x033;
    /// Aceleración máxima de los ejes X e Y, µm/s².
    pub const AXIS_MAX_ACCEL_X: u16 = 0x025;
    pub const AXIS_MAX_ACCEL_Y: u16 = 0x035;
    /// Tiempo total encendida, tiempo total de trabajo (s), número de trabajos y tiempo de láser (s).
    pub const TOTAL_ON_TIME: u16 = 0x201;
    pub const TOTAL_WORK_TIME: u16 = 0x202;
    pub const TOTAL_JOBS: u16 = 0x203;
    pub const TOTAL_LASER_TIME: u16 = 0x211;
    /// Número de documentos guardados en la memoria de la controladora.
    pub const DOC_COUNT: u16 = 0x205;
    /// Recorrido total de los ejes X e Y (m).
    pub const TRAVEL_X: u16 = 0x223;
    pub const TRAVEL_Y: u16 = 0x233;
}

/// Tope de documentos que se listan (las direcciones `0x261..0x2C4` dejan unos 99).
pub const MAX_LISTED_DOCS: u32 = 99;

/// Modelos conocidos por su ID de tarjeta (ruida-pa).
pub fn model_name(card_id: u32) -> Option<&'static str> {
    match card_id {
        0x6510_6510 => Some("RDC6442S"),
        0x9010_9010 => Some("RDC8445S"),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Magic con el que empezar. Si la máquina contesta con otro, el sondeo lo detecta.
    pub magic: Magic,
    /// Probar los demás magics conocidos si la máquina no contesta al primero.
    pub probe_all_magics: bool,
    pub max_data_bytes: usize,
    /// Espera del ACK de un paquete. Las fuentes dan 0,25 a 1 s (MeerK40t, ruida-pa) y 4 s
    /// (EduTech, FabLab, versiones antiguas).
    pub ack_timeout: Duration,
    /// Espera de la respuesta de una lectura (tras el ACK).
    pub reply_timeout: Duration,
    /// Espera larga: movimientos que bloquean la comunicación (home). MeerK40t usa 40 s y
    /// ruida-pa 15 s.
    pub long_timeout: Duration,
    /// Reenvíos de un paquete tras un NAK (checksum rechazado: no se aplicó).
    pub nak_retries: u32,
    /// Reenvíos de un paquete de datos tras un timeout. 0 por defecto: sin números de
    /// secuencia no se distingue un paquete perdido de un ACK perdido y reenviar duplicaría
    /// comandos en el trabajo.
    pub timeout_retries: u32,
    /// Reintentos de una lectura (idempotente).
    pub query_retries: u32,
    /// Exigir la máquina en reposo antes de enviar, saltar o encuadrar.
    pub require_idle: bool,
    pub limits: SafetyLimits,
    pub home: HomeCorner,
    /// Velocidad de los saltos y del encuadre, mm/s.
    pub jog_speed_mm_s: f64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        SessionConfig {
            magic: Magic::default(),
            probe_all_magics: true,
            max_data_bytes: packet::DEFAULT_MAX_DATA_BYTES,
            ack_timeout: Duration::from_secs(1),
            reply_timeout: Duration::from_secs(1),
            long_timeout: Duration::from_secs(15),
            nak_retries: 2,
            timeout_retries: 0,
            query_retries: 2,
            require_idle: true,
            limits: SafetyLimits::default(),
            home: HomeCorner::default(),
            jog_speed_mm_s: 50.0,
        }
    }
}

#[derive(Debug)]
pub enum SessionError {
    Io(io::Error),
    /// No llegó la respuesta a tiempo.
    Timeout {
        what: &'static str,
    },
    /// La máquina rechazó el paquete (NAK repetido o ERR).
    Rejected {
        response: Response,
        packet: usize,
    },
    /// Respuesta que no corresponde a lo esperado (p. ej. un magic equivocado).
    Protocol(String),
    Safety(SafetyError),
    Packet(PacketError),
    /// La máquina no está en reposo.
    Busy(MachineStatus),
    /// El usuario canceló.
    Cancelled,
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionError::Io(e) => write!(f, "error de red: {e}"),
            SessionError::Timeout { what } => write!(f, "sin respuesta de la máquina ({what})"),
            SessionError::Rejected { response, packet } => write!(
                f,
                "la máquina rechazó el paquete {} ({})",
                packet + 1,
                match response {
                    Response::Nak => "checksum rechazado",
                    Response::Err => "error de mensaje",
                    _ => "respuesta inesperada",
                }
            ),
            SessionError::Protocol(m) => write!(f, "{m}"),
            SessionError::Safety(e) => write!(f, "{e}"),
            SessionError::Packet(e) => write!(f, "{e}"),
            SessionError::Busy(s) => {
                write!(f, "la máquina no está en reposo ({})", s.describe_state())
            }
            SessionError::Cancelled => write!(f, "cancelado por el usuario"),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<io::Error> for SessionError {
    fn from(e: io::Error) -> Self {
        SessionError::Io(e)
    }
}

impl From<SafetyError> for SessionError {
    fn from(e: SafetyError) -> Self {
        SessionError::Safety(e)
    }
}

impl From<PacketError> for SessionError {
    fn from(e: PacketError) -> Self {
        SessionError::Packet(e)
    }
}
