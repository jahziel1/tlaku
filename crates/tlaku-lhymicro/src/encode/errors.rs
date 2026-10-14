//! Avisos y errores del codificador.

use crate::packet::FrameError;
use crate::text;
use std::fmt;
use tlaku_i18n::tr;

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
            Warning::PowerIgnored { block, pct } => f.write_str(&tr!(
                "lhymicro.encode.power_ignored",
                block = block,
                pct = format!("{pct:.0}")
            )),
            Warning::GrayThresholded { block } => {
                f.write_str(&tr!("lhymicro.encode.gray_thresholded", block = block))
            }
            Warning::AirAssistIgnored { block } => {
                f.write_str(&tr!("lhymicro.encode.air_ignored", block = block))
            }
            Warning::ZIgnored { block } => {
                f.write_str(&tr!("lhymicro.encode.z_ignored", block = block))
            }
            Warning::DwellSkipped { block, ms, fires } => f.write_str(&if *fires {
                tr!("lhymicro.encode.pulse_skipped", block = block, ms = ms)
            } else {
                tr!("lhymicro.encode.pause_skipped", block = block, ms = ms)
            }),
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
            EncodeError::BadConfig(w) => {
                f.write_str(&tr!("lhymicro.encode.bad_config", what = text::what(w)))
            }
            EncodeError::NonFinite { block, what } => f.write_str(&tr!(
                "lhymicro.encode.non_finite",
                block = block,
                what = text::what(what)
            )),
            EncodeError::OutOfBed {
                block,
                index,
                x_mm,
                y_mm,
            } => f.write_str(&tr!(
                "lhymicro.encode.out_of_bed",
                block = block,
                index = index,
                x = format!("{x_mm:.3}"),
                y = format!("{y_mm:.3}")
            )),
            EncodeError::Speed {
                block,
                mm_s,
                reason,
            } => f.write_str(&tr!(
                "lhymicro.encode.speed",
                block = block,
                mm_s = format!("{mm_s:.2}"),
                reason = reason
            )),
            EncodeError::Frame(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for EncodeError {}
