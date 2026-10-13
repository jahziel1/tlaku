//! Tipos de lo que la app lee de una controladora Ruida: información (M25 y M26) y lista de
//! documentos guardados (M24). Todo es de solo lectura; ver `docs/hardware/ruida.md`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Información de la controladora Ruida (`ruida_info`). Un dato que la máquina no informa (o
/// informa como 0) viene como `null`. Direcciones y unidades sin verificar con hardware real.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct RuidaInfo {
    /// Modelo conocido por el ID de tarjeta (p. ej. `RDC6442S`).
    pub model: Option<String>,
    /// ID de tarjeta en hexadecimal (`0x65106510`).
    pub card_id: String,
    /// Byte de ofuscación (swizzle) con el que habla la máquina.
    pub magic: u8,
    /// Tamaño de la cama que informa la máquina, mm.
    pub bed: Option<[f64; 2]>,
    /// Velocidad del movimiento rápido, mm/s.
    pub rapid_mm_s: Option<f64>,
    /// Velocidad de home, mm/s.
    pub home_mm_s: Option<f64>,
    /// Velocidad máxima de los ejes X e Y, mm/s.
    pub axis_max_mm_s: [Option<f64>; 2],
    /// Aceleración máxima de los ejes X e Y, mm/s².
    pub axis_accel_mm_s2: [Option<f64>; 2],
    /// Segundos encendida, de trabajo y de láser.
    pub total_on_s: Option<f64>,
    pub total_work_s: Option<f64>,
    pub total_laser_s: Option<f64>,
    /// Trabajos hechos.
    pub total_jobs: Option<u32>,
    /// Recorrido total de los ejes X e Y, metros.
    pub travel_m: [Option<f64>; 2],
    /// Documentos guardados en la memoria.
    pub documents: Option<u32>,
}

/// Un documento guardado en la memoria de la controladora.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct RuidaFile {
    /// Número del documento (desde 1).
    pub index: u32,
    /// Nombre; `null` si la máquina no lo contestó.
    pub name: Option<String>,
}
