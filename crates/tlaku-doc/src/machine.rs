//! Tipos de la máquina conectada: reporte de estado, eventos `machine://*` y resultados de
//! comandos. Se serializan en `snake_case` y se exportan a TypeScript con `ts-rs`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Estado de la controladora (`Idle`, `Run`, `Hold`...). En minúsculas en el JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MachineState {
    Idle,
    Run,
    Hold,
    Jog,
    Alarm,
    Door,
    Check,
    Home,
    Sleep,
    /// Estado que Tlaku no conoce (p. ej. `Tool` de grblHAL). La máquina está ocupada, no en
    /// reposo; el nombre original viaja en [`StatusReport::other_state`].
    Other,
}

/// Espacio libre en la controladora.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct BufferStatus {
    /// Bloques libres del planificador de movimiento.
    pub planner: u32,
    /// Bytes libres del búfer de recepción serie.
    pub rx: u32,
}

/// Evento `machine://status`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct StatusReport {
    pub state: MachineState,
    /// Nombre original del estado cuando `state` es `other` (p. ej. `Tool`), tal como lo
    /// informó la controladora.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub other_state: Option<String>,
    /// Posición de máquina [x, y], mm.
    pub machine_pos: [f64; 2],
    /// Posición de trabajo [x, y], mm (la misma que usa `jog`).
    pub work_pos: [f64; 2],
    /// F: velocidad actual, mm/min.
    pub feed: f64,
    /// S: potencia actual (0..s_max).
    pub spindle: f64,
    pub buffer: BufferStatus,
}

/// Evento `machine://progress`: líneas confirmadas por la controladora.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct ProgressEvent {
    pub acked: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum LogDir {
    /// Lo que la máquina nos envía.
    #[serde(rename = "in")]
    In,
    /// Lo que enviamos a la máquina.
    #[serde(rename = "out")]
    Out,
}

/// Evento `machine://log`. Los reportes de estado (`?`) no se registran.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct LogEvent {
    pub dir: LogDir,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum JobState {
    Started,
    Paused,
    Finished,
    Cancelled,
    Error,
}

/// Evento `machine://job`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct JobEvent {
    pub state: JobState,
    /// Texto en español para mostrar al usuario.
    pub message: String,
    /// El trabajo terminó con un reset (cancelación, error, falla) con la máquina posiblemente
    /// en movimiento: la posición puede haberse perdido y conviene hacer homing o fijar el
    /// origen de nuevo.
    pub position_lost: bool,
}

/// Evento `machine://connection`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct ConnectionEvent {
    pub connected: bool,
    pub port: Option<String>,
    /// Por qué se cerró la conexión, si no fue a petición del usuario.
    pub error: Option<String>,
}

/// Comandos en tiempo real que expone `machine_realtime`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RealtimeCommand {
    /// `?`: pide un reporte de estado.
    Status,
    /// `!`: feed hold.
    Hold,
    /// `~`: cycle start.
    Resume,
    /// Ctrl+X: soft reset.
    Reset,
    /// 0x85: cancela el jog en curso (la cabeza se detiene sin perder la posición).
    JogCancel,
}

/// Falla que se puede provocar en una máquina virtual (`simulate_fault`) para ver cómo reacciona
/// la interfaz. En una máquina real no existe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SimulatedFault {
    /// `ERROR:05` de Creality: llama detectada; la máquina se detiene.
    Flame,
    /// `ERROR:01` de Creality: ángulo anormal (inclinación o vibración).
    Tilt,
    /// `ALARM:3`: reinicio durante un movimiento, posición perdida.
    Alarm3,
    /// El cable se arranca: la conexión se pierde.
    Disconnect,
}

/// Salida de `connect`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct ConnectResult {
    /// Texto de versión de la controladora (p. ej. `Grbl 1.1h`).
    pub firmware: String,
}

/// Salida de `export_gcode`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct ExportResult {
    /// Líneas con contenido del archivo escrito.
    pub lines: u32,
    pub estimate_s: f64,
    pub warnings: Vec<String>,
}
