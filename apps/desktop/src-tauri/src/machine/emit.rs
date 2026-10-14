//! Salida de eventos (`Emit`, `Sink`) y tiempos (`Timing`) de la conexión GRBL.

use super::*;

/// Evento hacia la interfaz (`machine://*`).
#[derive(Debug, Clone, PartialEq)]
pub enum Emit {
    Status(StatusReport),
    Progress(ProgressEvent),
    Log(LogEvent),
    Job(JobEvent),
    Connection(ConnectionEvent),
    /// Lo propio del galvo (pasos del marcado dividido, vueltas del encuadre, aviso de energía).
    Galvo(galvo::GalvoEvent),
}

impl Emit {
    /// Nombre del evento de Tauri.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Status(_) => "machine://status",
            Self::Progress(_) => "machine://progress",
            Self::Log(_) => "machine://log",
            Self::Job(_) => "machine://job",
            Self::Connection(_) => "machine://connection",
            Self::Galvo(_) => "machine://galvo",
        }
    }
}

/// Destino de los eventos. En la app emite a la ventana; en las pruebas los guarda.
pub trait Sink: Send + Sync + 'static {
    fn emit(&self, event: Emit);
}

// ---------------------------------------------------------------------------
// Tiempos
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Timing {
    /// Cada cuánto se consulta `?` en reposo.
    pub status_interval: Duration,
    /// Sin recibir nada durante este tiempo (con el sondeo activo) la conexión se da por perdida.
    pub silence_timeout: Duration,
    /// Espera de la respuesta a un comando normal.
    pub command_timeout: Duration,
    /// Espera de la respuesta a `$H` (GRBL contesta `ok` al terminar el homing) y plazo de
    /// silencio mientras dura (`?` puede no contestarse durante el homing).
    pub home_timeout: Duration,
    /// Tras un feed hold de cancelación, cuánto se espera a que la máquina termine de frenar
    /// (`Hold:0`) antes de resetear de todos modos.
    pub hold_timeout: Duration,
    /// Cuánto se espera el saludo tras un soft reset de cancelación.
    pub reset_wait: Duration,
    /// Espera de la respuesta a `$I` al conectar.
    pub identify_timeout: Duration,
    /// Lectura máxima por vuelta del ciclo: acota la latencia de los mandos.
    pub tick: Duration,
    /// Mínimo entre dos eventos de progreso (el último siempre sale).
    pub progress_interval: Duration,
    /// Cada cuánto se guarda el avance del punto de control del trabajo (ver [`resume`]).
    pub checkpoint_interval: Duration,
    pub preflight: PreflightConfig,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            status_interval: Duration::from_millis(250),
            silence_timeout: Duration::from_secs(5),
            command_timeout: Duration::from_secs(5),
            home_timeout: Duration::from_secs(120),
            hold_timeout: Duration::from_secs(2),
            reset_wait: Duration::from_millis(500),
            identify_timeout: Duration::from_secs(3),
            tick: Duration::from_millis(20),
            progress_interval: Duration::from_millis(100),
            checkpoint_interval: resume::CHECKPOINT_INTERVAL,
            preflight: PreflightConfig::default(),
        }
    }
}
