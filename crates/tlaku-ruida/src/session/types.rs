//! Tipos de resultado de la sesión: sondeo, estado, información del controlador, envío.

use super::config::SessionError;
use crate::swizzle::Magic;
use std::fmt;
use tlaku_core::Point;
use tlaku_i18n::{Arg, tr};

/// Bits de `0x200`: ruida-pa y MeerK40t coinciden en el movimiento (`0x01000000`) y el trabajo
/// (`0x1`); el bit `0x2` es "pausa" para ruida-pa y "fin de parte" para MeerK40t.
pub(super) const BIT_RUNNING: u64 = 0x0000_0001;
pub(super) const BIT_PAUSED: u64 = 0x0000_0002;
pub(super) const BIT_MOVING: u64 = 0x0100_0000;

/// Resultado del sondeo.
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeInfo {
    pub magic: Magic,
    /// El magic detectado no es el configurado.
    pub magic_changed: bool,
    /// ID de tarjeta de 32 bits (`0x65106510` en una RDC6442S).
    pub card_id: u32,
    pub model: Option<&'static str>,
    /// Tamaño de la cama que informa la máquina (mm), si contestó.
    pub bed_mm: Option<[f64; 2]>,
}

/// Estado leído de la máquina.
#[derive(Debug, Clone, PartialEq)]
pub struct MachineStatus {
    pub raw: u64,
    pub running: bool,
    /// Bit `0x2`: pausa o fin de parte según la fuente.
    pub paused: bool,
    pub moving: bool,
    /// Posición en el sistema de la máquina (µm desde el home).
    pub pos_um: (i64, i64),
    /// Posición en el sistema de Tlaku (mm, abajo-izquierda, Y arriba).
    pub pos_mm: Point,
}

impl MachineStatus {
    pub fn is_idle(&self) -> bool {
        !self.running && !self.moving
    }

    pub fn describe_state(&self) -> String {
        match (self.running, self.paused, self.moving) {
            (true, true, _) => tr!("ruida.session.state_paused"),
            (true, false, _) => tr!("ruida.session.state_running"),
            (false, _, true) => tr!("ruida.session.state_moving"),
            _ => tr!("ruida.session.state_idle"),
        }
    }
}

/// Información de la controladora (solo lectura). Los valores que la máquina no informa (o
/// informa como 0) quedan en `None`. Direcciones y unidades de ruida.md sección 4.4 y del
/// emulador de MeerK40t (MIT); **sin verificar con hardware real**.
#[derive(Debug, Clone, PartialEq)]
pub struct ControllerInfo {
    pub probe: ProbeInfo,
    /// Velocidad del movimiento rápido (G0), mm/s.
    pub rapid_mm_s: Option<f64>,
    /// Velocidad de home, mm/s.
    pub home_mm_s: Option<f64>,
    /// Velocidad máxima de los ejes X e Y, mm/s.
    pub axis_max_mm_s: [Option<f64>; 2],
    /// Aceleración máxima de los ejes X e Y, mm/s².
    pub axis_accel_mm_s2: [Option<f64>; 2],
    /// Segundos encendida, segundos de trabajo, trabajos hechos y segundos de láser.
    pub total_on_s: Option<u64>,
    pub total_work_s: Option<u64>,
    pub total_jobs: Option<u64>,
    pub total_laser_s: Option<u64>,
    /// Recorrido total de los ejes X e Y, metros.
    pub travel_m: [Option<u64>; 2],
    /// Documentos guardados en la memoria.
    pub documents: Option<u32>,
    /// Versión de la placa (cadena), si la máquina la informa.
    pub board_version: Option<String>,
    /// Duración del trabajo anterior; unidad sin documentar.
    pub previous_work: Option<u64>,
    /// Modo de control del sistema (`0x0000` origen, `0x4000` sin retorno, `0x8000` origen
    /// absoluto según EduTech).
    pub control_mode: Option<u64>,
    /// Funciones de la máquina, decodificadas del valor de `0x18F`.
    pub features: Option<MachineFeatures>,
}

/// Bits de `MEM_MACHINE_FEATURES` (ruida-pa, `ruida_protocol.py`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachineFeatures {
    pub raw: u64,
    /// Bit 0: foco habilitado.
    pub focus: bool,
    /// Bit 3: retorno de Z al muelle.
    pub z_return_docking: bool,
    /// Bits 9 y 10: modo del aire (0 a 3; ruida-pa no nombra los modos).
    pub air_assist_mode: u8,
}

impl MachineFeatures {
    pub fn from_raw(raw: u64) -> Self {
        MachineFeatures {
            raw,
            focus: raw & 0x1 != 0,
            z_return_docking: raw & 0x8 != 0,
            air_assist_mode: ((raw >> 9) & 0x3) as u8,
        }
    }
}

/// Documento guardado en la memoria de la controladora.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerFile {
    /// Número del documento (1 en adelante, como lo numera `E8 01`).
    pub index: u32,
    /// Nombre; `None` si la máquina no contestó a la consulta de ese documento.
    pub name: Option<String>,
}

/// Cómo enviar el trabajo.
#[derive(Debug, Clone, PartialEq)]
pub enum SendMode {
    /// Flujo directo: el trabajo empieza a ejecutarse al llegar su `D8 00` (como LightBurn).
    Run,
    /// Se guarda como documento con ese nombre y no arranca; se inicia desde el panel.
    Store { name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub packet: usize,
    pub packets: usize,
    pub bytes_sent: usize,
    pub bytes_total: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UploadReport {
    pub packets: usize,
    pub bytes: usize,
    /// NAK que se resolvieron reenviando.
    pub resends: usize,
    /// El trabajo ya está ejecutándose en la máquina (modo `Run`).
    pub running: bool,
}

/// Qué pasó con el intento de dejar la máquina en estado seguro tras un fallo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopOutcome {
    /// No hacía falta (modo `Store`: nada se ejecuta).
    NotNeeded,
    /// La máquina confirmó el `D8 01`.
    Acked,
    /// No hubo confirmación: puede seguir en marcha; usar el panel o el paro de emergencia.
    Unconfirmed,
}

/// El envío falló. `machine_may_be_running` es verdadero si pudo llegar el `D8 00` y no hay
/// confirmación de que la máquina esté parada.
#[derive(Debug)]
pub struct UploadError {
    pub cause: SessionError,
    pub packets_sent: usize,
    pub packets_total: usize,
    pub stop: StopOutcome,
    pub machine_may_be_running: bool,
}

impl fmt::Display for UploadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key = match self.stop {
            StopOutcome::NotNeeded => "ruida.session.upload_failed_incomplete",
            StopOutcome::Acked => "ruida.session.upload_failed_stopped",
            StopOutcome::Unconfirmed => "ruida.session.upload_failed_unconfirmed",
        };
        let args = [
            ("cause", Arg::new(&self.cause)),
            ("sent", Arg::new(&self.packets_sent)),
            ("total", Arg::new(&self.packets_total)),
        ];
        f.write_str(&tlaku_i18n::translate(key, &args))
    }
}

impl std::error::Error for UploadError {}
