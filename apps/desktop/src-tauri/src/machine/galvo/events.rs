//! Eventos del galvo, su estado publicado y la temporización.

use super::*;

// ---------------------------------------------------------------------------
// Eventos y estado propios
// ---------------------------------------------------------------------------

/// Evento `machine://galvo`: lo propio del galvo que los eventos comunes no dicen.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GalvoEvent {
    /// Empieza a marcarse un paso.
    Step { info: StepInfo },
    /// Hay que mover el material (o girar el rotativo) a mano y confirmar para seguir. El trabajo
    /// espera hasta `galvo_confirm_move` o hasta detener.
    WaitMove { info: StepInfo },
    /// Una vuelta más del encuadre con el puntero rojo. `adjust_gen` es la versión del último
    /// ajuste en vivo aplicado (0 = ninguno) y `adjust_rejected` avisa que el último ajuste
    /// pedido no cabía en el campo y se descartó.
    Light {
        loops: u32,
        adjust_gen: u32,
        adjust_rejected: bool,
    },
    /// No se pudo confirmar el apagado: la fuente puede seguir habilitada. Cortar la energía.
    LaserMayBeOn { message: String },
}

/// Estado de la conexión para el panel.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GalvoState {
    pub board: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub virtual_machine: bool,
    /// Serie y versión de la tarjeta, en hexadecimal.
    pub serial: String,
    pub version: String,
    /// Bits del estado de la tarjeta (palabra 3 de `GetVersion`; sin verificar).
    pub status_raw: u16,
    pub busy: bool,
    pub ready: bool,
    pub axis: bool,
    /// Entradas (palabra 1 de `ReadPort`). Solo lectura.
    pub inputs: u16,
    pub inputs_known: bool,
    /// Hay un trabajo, un encuadre o una espera en curso.
    pub working: bool,
    /// El puntero rojo está encendido en su sitio.
    pub red_light_on: bool,
    /// El último apagado NO se confirmó: la fuente puede seguir habilitada. Se mantiene hasta un
    /// apagado confirmado.
    pub laser_may_be_on: bool,
    /// Posición del haz según la última orden de salto (mm del documento); ver `position_known`.
    pub position: [f64; 2],
    pub position_known: bool,
    pub field_mm: f64,
    pub source: LaserSource,
    /// El trabajo espera que muevas el material y confirmes.
    pub waiting_move: Option<StepInfo>,
    /// Hay un encuadre con el puntero en curso (se puede mover y escalar en vivo).
    pub framing: bool,
    /// Enclavamiento por entrada del perfil y si ahora se cumple (M45); `None` = sin enclavamiento.
    pub interlock: Option<GalvoInterlock>,
    /// Desfase del puntero rojo con el que se conectó (mm) y overscan del perfil.
    pub pointer_offset: [f64; 2],
    pub overscan_mm: f64,
}

/// La condición `(entradas & mask) == value` que el perfil exige antes de cada trabajo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct GalvoInterlock {
    pub mask: u16,
    pub value: u16,
    /// Se cumple con las últimas entradas leídas.
    pub met: bool,
}

// ---------------------------------------------------------------------------
// Tiempos
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct GalvoTimingCfg {
    /// Cada cuánto se lee el estado en reposo.
    pub status_interval: Duration,
    /// Lecturas seguidas sin respuesta tras las cuales se da por perdida la conexión.
    pub lost_after: u32,
    /// Mínimo entre dos eventos de progreso (el último siempre sale).
    pub progress_interval: Duration,
    /// Mínimo entre dos eventos de vuelta del encuadre.
    pub light_interval: Duration,
    /// Cuánto duerme cada lectura de la máquina virtual (marca el paso del reloj del emulador).
    pub virtual_pace: Duration,
}

impl Default for GalvoTimingCfg {
    fn default() -> Self {
        Self {
            status_interval: Duration::from_millis(500),
            lost_after: 3,
            progress_interval: Duration::from_millis(100),
            light_interval: Duration::from_millis(150),
            virtual_pace: Duration::from_millis(4),
        }
    }
}
