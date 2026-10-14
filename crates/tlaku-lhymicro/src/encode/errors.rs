//! Avisos y errores del codificador.

use crate::packet::FrameError;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Warning {
    /// La capa pedía potencia `pct` y la placa no la controla.
    PowerIgnored {
        block: usize,
        pct: f64,
    },
    /// El bloque tiene niveles de potencia intermedios: se graba en blanco y negro.
    GrayThresholded {
        block: usize,
    },
    AirAssistIgnored {
        block: usize,
    },
    ZIgnored {
        block: usize,
    },
    /// Pausa o pulso en el sitio que no se puede hacer con seguridad.
    DwellSkipped {
        block: usize,
        ms: f64,
        fires: bool,
    },
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Warning::PowerIgnored { block, pct } => write!(
                f,
                "bloque {block}: la potencia de {pct:.0} % se ignora; la M2 Nano no la controla \
                 por software (se ajusta en el panel o el potenciómetro de la máquina)"
            ),
            Warning::GrayThresholded { block } => write!(
                f,
                "bloque {block}: niveles de gris binarizados (cualquier potencia mayor que 0 \
                 quema); usa un tramado para simular grises"
            ),
            Warning::AirAssistIgnored { block } => write!(
                f,
                "bloque {block}: la asistencia de aire no se controla desde la M2 Nano"
            ),
            Warning::ZIgnored { block } => {
                write!(
                    f,
                    "bloque {block}: el eje Z no existe en la M2 Nano y se ignora"
                )
            }
            Warning::DwellSkipped { block, ms, fires } => write!(
                f,
                "bloque {block}: {} de {ms} ms omitida (la placa no hace pulsos en el sitio)",
                if *fires { "pulso" } else { "pausa" }
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EncodeError {
    BadConfig(&'static str),
    NonFinite {
        block: usize,
        what: &'static str,
    },
    OutOfBed {
        block: usize,
        index: usize,
        x_mm: f64,
        y_mm: f64,
    },
    Speed {
        block: usize,
        mm_s: f64,
        reason: String,
    },
    Frame(FrameError),
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EncodeError::BadConfig(w) => write!(f, "configuración inválida: {w}"),
            EncodeError::NonFinite { block, what } => {
                write!(f, "bloque {block}: {what} no es un número finito")
            }
            EncodeError::OutOfBed {
                block,
                index,
                x_mm,
                y_mm,
            } => write!(
                f,
                "bloque {block}, movimiento {index}: ({x_mm:.3}, {y_mm:.3}) mm queda fuera de la cama"
            ),
            EncodeError::Speed {
                block,
                mm_s,
                reason,
            } => write!(f, "bloque {block}: velocidad {mm_s:.2} mm/s: {reason}"),
            EncodeError::Frame(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for EncodeError {}
