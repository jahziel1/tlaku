//! Tipos internos del emulador: estado modal del parser, bloques del planificador y acciones.

use crate::trace::{MoveKind, SpindleMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MotionMode {
    /// G0.
    Rapid,
    /// G1.
    Linear,
    /// G2.
    ArcCw,
    /// G3.
    ArcCcw,
    /// G80: sin modo de movimiento.
    None,
}

/// Estado modal del intérprete de G-code (lo que imprime `$G`).
#[derive(Debug, Clone)]
pub(crate) struct Gc {
    pub motion: MotionMode,
    /// Índice del sistema de coordenadas activo (0 = G54 ... 5 = G59).
    pub wcs: usize,
    pub plane: u8,
    pub inches: bool,
    pub absolute: bool,
    pub inverse_time: bool,
    pub spindle: SpindleMode,
    pub flood: bool,
    pub mist: bool,
    /// Avance en mm/min.
    pub feed: f64,
    pub s: f64,
    pub tool: u32,
    /// Posición de máquina a la que llega lo ya programado (el final del último bloque).
    pub position: [f64; 3],
}

impl Gc {
    pub fn new(position: [f64; 3]) -> Self {
        Self {
            motion: MotionMode::Rapid,
            wcs: 0,
            plane: 17,
            inches: false,
            absolute: true,
            inverse_time: false,
            spindle: SpindleMode::Off,
            flood: false,
            mist: false,
            feed: 0.0,
            s: 0.0,
            tool: 0,
            position,
        }
    }
}

/// Segmento de movimiento en el planificador.
#[derive(Debug, Clone)]
pub(crate) struct Block {
    pub kind: MoveKind,
    pub from: [f64; 3],
    pub to: [f64; 3],
    pub len: f64,
    /// mm/min programados (ignorado en rápidos).
    pub feed: f64,
    pub spindle: SpindleMode,
    pub s: f64,
    /// Número de línea (no vacía) que lo originó.
    pub line: usize,
    /// Palabra `N` de la línea.
    pub n: Option<u32>,
}

impl Block {
    pub fn dir(&self) -> [f64; 3] {
        if self.len <= 0.0 {
            return [0.0; 3];
        }
        [
            (self.to[0] - self.from[0]) / self.len,
            (self.to[1] - self.from[1]) / self.len,
            (self.to[2] - self.from[2]) / self.len,
        ]
    }
}

/// Paso de ejecución de una línea. Se ejecutan en orden; uno puede bloquear a los siguientes.
#[derive(Debug, Clone)]
pub(crate) enum Action {
    /// Espera a que el planificador se vacíe (y la cabeza esté libre de hold).
    Sync,
    /// Fija el estado efectivo del husillo (tras una espera).
    ApplySpindle(SpindleMode, f64),
    /// Encola un bloque; espera si el planificador está lleno.
    Move(Box<Block>),
    /// Espera `s` segundos.
    Dwell(f64),
    /// M2/M30: fin de programa.
    ProgramEnd,
    /// M0: pausa del programa hasta `~`.
    Pause,
    /// M6 de grblHAL: cambio de herramienta hasta `~`.
    ToolChange,
    /// La línea excede los límites blandos: se responde `ok` y salta `ALARM:2`.
    SoftLimitAlarm,
}

/// Efectos inmediatos sobre parámetros del controlador.
#[derive(Debug, Clone)]
pub(crate) enum Effect {
    SetWcs(usize, [f64; 3]),
    SetG92([f64; 3]),
    StoreG28([f64; 3]),
    StoreG30([f64; 3]),
}

/// Resultado de interpretar una línea de G-code.
pub(crate) struct Parsed {
    pub gc: Gc,
    pub effects: Vec<Effect>,
    pub actions: Vec<Action>,
}

/// Estado informado por `?`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateKind {
    Idle,
    Run,
    /// 1 = deteniéndose, 0 = detenida y lista para reanudar.
    Hold(u8),
    Jog,
    /// Código de la alarma; 0 si es un bloqueo sin código (arranque con homing obligatorio).
    Alarm(u32),
    Door(u8),
    Check,
    Home,
    Sleep,
    /// Cambio de herramienta pendiente (grblHAL).
    Tool,
}

impl StateKind {
    /// Nombre base sin subestado.
    pub fn name(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Run => "Run",
            Self::Hold(_) => "Hold",
            Self::Jog => "Jog",
            Self::Alarm(_) => "Alarm",
            Self::Door(_) => "Door",
            Self::Check => "Check",
            Self::Home => "Home",
            Self::Sleep => "Sleep",
            Self::Tool => "Tool",
        }
    }

    /// `true` si hay (o se está terminando) un movimiento: un reset aquí dispara `ALARM:3`.
    pub fn is_moving(self) -> bool {
        matches!(
            self,
            Self::Run | Self::Jog | Self::Hold(1) | Self::Door(2 | 3)
        )
    }
}
