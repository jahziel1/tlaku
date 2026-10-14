//! Continuar un trabajo interrumpido: lo que la app guarda mientras envía (punto de control) y lo
//! que la interfaz necesita para ofrecer, mostrar y reanudar. Se serializa en `snake_case` y se
//! exporta a TypeScript con `ts-rs`. La lógica vive en `apps/desktop/src-tauri/src/machine/resume/`.

use crate::device::DeviceKind;
use crate::machine::MachineState;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Qué controladora llevaba el trabajo. Decide si se puede continuar: solo el G-code (GRBL,
/// Marlin, Smoothieware) se reanuda; los demás solo avisan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ResumeKind {
    /// G-code por línea: se puede continuar.
    Gcode,
    /// El trabajo vive en la controladora Ruida: sigue solo.
    Ruida,
    /// K40 (LHYMICRO-GL): no se puede continuar desde un paquete.
    K40,
    /// Galvo: no aplica todavía.
    Galvo,
}

/// Husillo/láser según el último `M3`/`M4`/`M5`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ResumeSpindle {
    #[default]
    Off,
    /// `M3`: potencia constante.
    M3,
    /// `M4`: potencia dinámica.
    M4,
}

/// Aire asistido según el último `M7`/`M8`/`M9`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ResumeAir {
    #[default]
    Off,
    /// `M7`.
    Mist,
    /// `M8`.
    Flood,
}

/// Estado modal del G-code que importa al reanudar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct ResumeModal {
    /// `G90` (true) o `G91`.
    pub absolute: bool,
    /// `G20` (true) o `G21`.
    pub inches: bool,
    pub spindle: ResumeSpindle,
    pub air: ResumeAir,
    /// Último `S` visto, en las unidades del firmware.
    pub s: Option<f64>,
    /// Último `F` visto (mm/min).
    pub f: Option<f64>,
    /// Z acumulada por los movimientos de Z del G-code (Tlaku los emite relativos), en mm.
    pub z: f64,
}

impl Default for ResumeModal {
    fn default() -> Self {
        Self {
            absolute: true,
            inches: false,
            spindle: ResumeSpindle::Off,
            air: ResumeAir::Off,
            s: None,
            f: None,
            z: 0.0,
        }
    }
}

/// Un trabajo que no terminó y del que queda punto de control.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct InterruptedJob {
    /// Identificador del punto de control (cambia con cada trabajo).
    pub id: String,
    pub kind: ResumeKind,
    /// Cuándo empezó el trabajo (ms desde 1970).
    #[ts(type = "number")]
    pub started_at_ms: u64,
    /// Última vez que se guardó el avance (ms desde 1970).
    #[ts(type = "number")]
    pub updated_at_ms: u64,
    pub device_id: String,
    pub device_name: String,
    pub device_kind: DeviceKind,
    /// Puerto con el que se conectó.
    pub port: String,
    /// Ruta del proyecto (o su nombre) si estaba guardado.
    pub document: Option<String>,
    /// Líneas de G-code que se envían (sin vacías ni comentarios).
    pub total_lines: u32,
    /// Líneas del trabajo original ya confirmadas con `ok` (ojo: `ok` no es «ejecutada»).
    pub done_lines: u32,
    /// 0 a 100.
    pub percent: f64,
    /// Había avance legible. Si no, `done_lines` es 0 y hay que elegir el punto a mano.
    pub progress_known: bool,
    /// Posición de máquina (MPos) del último reporte de estado.
    pub mpos: Option<[f64; 2]>,
    pub modal: Option<ResumeModal>,
    /// Se puede continuar desde la interfaz.
    pub can_resume: bool,
    /// Por qué no se puede continuar, o aclaración que el usuario debe leer (ya traducida).
    pub note: Option<String>,
    /// Este trabajo ya era una reanudación de otro.
    pub resumed: bool,
}

/// Dónde reiniciar y por qué.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct ResumePoint {
    /// Línea del G-code original (desde 0) con la que sigue el trabajo.
    pub line: u32,
    /// Índice en `ResumeView::moves` del movimiento que empieza en esa línea.
    pub move_index: u32,
    /// Punto donde se reanuda dentro de esa línea (solo si cae a mitad de un corte recto);
    /// `null` = el inicio de la línea.
    pub at: Option<[f64; 2]>,
    /// Línea que se estima en ejecución cuando se cortó (comparando la MPos con la geometría).
    pub executing_line: u32,
    /// Distancia en mm de la MPos al trazo más cercano (0 si no se pudo comparar).
    pub distance_mm: f64,
    /// La MPos no encajó con ninguna línea cercana: el punto es una estimación conservadora.
    pub mismatch: bool,
    /// Se retrocedió hasta el inicio de la fila de barrido en curso (grabado de imagen).
    pub row_restart: bool,
    /// Avance del trabajo hasta ese punto (0 a 100).
    pub percent: f64,
    /// Variación de Z (mm) que el preámbulo aplica para llegar a la altura que lleva ese punto.
    pub z_delta: f64,
}

/// Lo que dibuja el diálogo de reanudación: el recorrido del G-code guardado con el punto elegido.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct ResumeView {
    pub job: InterruptedJob,
    /// Una entrada `[tipo, x, y, potencia]` por movimiento (la primera es el punto de partida):
    /// tipo 0 = desplazamiento, 1 = corte; potencia 0 a 100 %.
    pub moves: Vec<f32>,
    /// Línea del G-code (desde 0) de cada movimiento de `moves`.
    pub move_lines: Vec<u32>,
    /// Esquinas mínima y máxima del recorrido.
    pub bounds: [[f64; 2]; 2],
    /// Movimiento de `moves` con el último `ok`.
    pub last_acked_move: u32,
    pub suggested: ResumePoint,
    /// Avisos para el usuario (Z, desfase de la MPos, desplazamientos G92...).
    pub warnings: Vec<String>,
}

/// Qué falta para poder reanudar, según la máquina conectada ahora.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct ResumeReadiness {
    /// La controladora tiene homing (`$22`); `null` si no contestó.
    pub homing: Option<bool>,
    /// Ya se hizo homing desde que la placa se reinició o perdió la posición.
    pub homed: bool,
    pub state: MachineState,
}

/// Argumentos de `continue_job`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct ResumeStart {
    pub id: String,
    /// Línea del G-code original (desde 0).
    pub line: u32,
    /// Punto de reanudación dentro de la línea (de `ResumePoint::at`); `null` = el inicio de la línea.
    pub at: Option<[f64; 2]>,
    /// Sin homing: el usuario confirma que ni la cabeza ni el material se movieron y que el
    /// origen es el mismo.
    pub confirm_unmoved: bool,
    /// El usuario encuadró el trabajo completo y vio que el material sigue en su lugar.
    pub framed_ok: bool,
    /// La lista de comprobación de siempre está completa.
    pub checklist_ok: bool,
}
