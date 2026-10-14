//! Tipos de lo que la app lee y escribe en una controladora Ruida: información (M25 y M26),
//! documentos guardados (M23 y M24) y ajustes (M25 y M28, ola 6E). **EXPERIMENTAL: sin probar con
//! una controladora real**; ver `docs/hardware/ruida.md`.

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
    /// Versión de la placa (cadena que informa la controladora).
    pub board_version: Option<String>,
    /// Duración del trabajo anterior; la unidad no está documentada en ninguna fuente.
    pub previous_work: Option<f64>,
    /// Modo de control del sistema: `0` origen, `16384` (0x4000) sin retorno, `32768` (0x8000)
    /// origen absoluto, según EduTech.
    pub control_mode: Option<u32>,
    /// Funciones de la máquina (`MEM_MACHINE_FEATURES` de ruida-pa): foco habilitado, retorno de Z
    /// al muelle y modo del aire (0 a 3).
    pub focus: Option<bool>,
    pub z_return_docking: Option<bool>,
    pub air_assist_mode: Option<u8>,
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

// ---------------------------------------------------------------------------------------------
// Ajustes de la controladora (M25, M28). Lista cerrada: `tlaku_ruida::settings::WRITABLE`.

/// Un ajuste escribible tal como está en la máquina. Las unidades de `kind` son: `speed` µm/s,
/// `accel` µm/s², `length` y `step` µm y `raw` sin unidad documentada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct RuidaSetting {
    /// Clave estable (`x_max_speed`); la interfaz la usa para el nombre.
    pub key: String,
    /// Dirección de memoria en hexadecimal (`0x023`).
    pub addr: String,
    /// `general`, `axis_x`, `axis_y`, `axis_z`, `axis_u` o `rotary`.
    pub group: String,
    /// `speed`, `accel`, `length`, `step` o `raw`.
    pub kind: String,
    /// Valor actual; `null` si la máquina no contestó.
    pub value: Option<f64>,
    /// Rango permitido por Tlaku (cota propia, no del firmware).
    pub min: f64,
    pub max: f64,
    /// Nombre del dato en las fuentes públicas (`MEM_MAX_VELOCITY_X`).
    pub source_name: String,
}

/// Todos los ajustes escribibles, leídos de la controladora (`ruida_settings_read`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct RuidaControllerSettings {
    pub model: Option<String>,
    /// ID de tarjeta en hexadecimal (`0x65106510`).
    pub card_id: String,
    pub settings: Vec<RuidaSetting>,
}

/// Un cambio pedido a la controladora, por clave.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct RuidaSettingChange {
    pub key: String,
    pub value: f64,
}

/// Un cambio validado: valor actual junto al nuevo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct RuidaPlannedChange {
    pub key: String,
    pub addr: String,
    pub current: f64,
    pub new: f64,
}

/// Las diferencias que produciría una petición (`ruida_settings_preview`), sin escribir nada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct RuidaSettingsPlan {
    pub changes: Vec<RuidaPlannedChange>,
    /// Pedidos cuyo valor ya es el de la máquina.
    pub unchanged: u32,
    /// Avisos de lo que se pide (p. ej. el respaldo es de otro modelo).
    pub warnings: Vec<String>,
}

/// Un valor releído tras escribir.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct RuidaVerified {
    pub key: String,
    pub expected: f64,
    /// `null` si la máquina no contestó a la relectura.
    pub actual: Option<f64>,
    pub ok: bool,
}

/// Resultado de aplicar o restaurar ajustes (`ruida_settings_apply`, `ruida_settings_restore`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct RuidaApplyReport {
    pub changes: Vec<RuidaPlannedChange>,
    pub unchanged: u32,
    /// Archivo del respaldo previo (ruta); `null` si no hubo nada que escribir.
    pub backup: Option<String>,
    pub verified: Vec<RuidaVerified>,
    /// Si una escritura falló: dirección y motivo. Las siguientes no se enviaron.
    pub write_error: Option<String>,
    /// Todo coincide y no hubo errores.
    pub ok: bool,
    /// Conviene restaurar el respaldo (algo no coincide o falló).
    pub restore_advised: bool,
}

/// Un respaldo automático guardado en la carpeta de configuración.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct RuidaBackup {
    /// Nombre del archivo (`ruida-20261014-120000.tlakuruida`).
    pub name: String,
    pub path: String,
    /// Fecha UTC del respaldo (de su contenido; vacío si no se pudo leer).
    pub created: String,
    /// ID de tarjeta de la controladora de la que sale.
    pub card_id: String,
}

/// Propuesta de calibración de un eje (`ruida_calibrate`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct RuidaCalibration {
    pub key: String,
    pub current: f64,
    pub proposed: f64,
    /// Medido entre pedido.
    pub ratio: f64,
    /// Pasos por mm antes y después, si el valor está en µm (lo que indican las fuentes).
    pub steps_per_mm_before: f64,
    pub steps_per_mm_after: f64,
}

/// Resultado de borrar o ejecutar un documento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct RuidaDocResult {
    pub index: u32,
    pub name: String,
    /// Borrar: la cuenta bajó en uno. Ejecutar: la máquina empezó a moverse o a trabajar.
    pub confirmed: bool,
}
