//! Error de un generador: un mensaje ya traducido y, si aplica, el parámetro que lo causa.

use serde::Serialize;
use std::fmt;
use ts_rs::TS;

/// Parámetros inválidos. Durante la edición en vivo es un estado normal (el usuario teclea un
/// valor a medias), por eso viaja como dato y no como fallo del comando.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct PartsError {
    /// Clave del parámetro (`thickness_mm`...) para marcar el campo; `None` si es del conjunto.
    #[ts(optional = nullable)]
    pub field: Option<String>,
    /// Texto en el idioma activo.
    pub message: String,
}

impl PartsError {
    /// Error de un parámetro concreto.
    pub fn field(field: &str, message: String) -> Self {
        Self {
            field: Some(field.to_string()),
            message,
        }
    }

    /// Error que no se debe a un solo parámetro.
    pub fn general(message: String) -> Self {
        Self {
            field: None,
            message,
        }
    }
}

impl fmt::Display for PartsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PartsError {}
