//! Conexión con una tarjeta galvo JCZ/BJJCZ LMC (EZCAD2, USB 9588:9899). **EXPERIMENTAL: no se ha
//! probado con una tarjeta real**; sí contra el emulador de `tlaku-galvo`. Un láser de fibra es
//! clase 4 y su haz es INVISIBLE.
//!
//! Igual que Ruida y el K40, es un hilo dedicado dueño de la sesión ([`GalvoSession`]) que habla
//! con la interfaz solo por eventos `machine://*` (el mismo [`Sink`]). Lo que lo distingue:
//!
//! - **Seguridad.** Para abrir una tarjeta real (`usb:galvo:<n>`) hace falta un reconocimiento de
//!   seguridad explícito ([`SafetyAck`]: gafas con la DO correcta, recinto cerrado y paro de
//!   emergencia al alcance). Solo [`SafetyAck::token`] crea el [`LaserSafetyAck`] que exige el USB,
//!   y la interfaz lo pide en CADA conexión (no se recuerda). La máquina virtual (`virtual:galvo`)
//!   usa el emulador y no lo necesita.
//! - **Sin pausa, a propósito.** Pausar deja el espejo quieto con el haz encendido (reportes de
//!   usuarios, docs/hardware/galvo.md §5): solo se puede detener. Si detener no se confirma
//!   (`StopOutcome::Unconfirmed`, `laser_may_be_on`), el mensaje trae [`UNCONFIRMED_MARK`] y la
//!   conexión queda marcada ([`GalvoState::laser_may_be_on`]) hasta un apagado confirmado: hay que
//!   cortar la energía de la fuente.
//! - **El encuadre es solo con el puntero rojo** (el láser nunca se habilita) y repite en bucle
//!   hasta detener.
//! - **Sin ejes.** La tarjeta no mueve mesa ni rotativo: en el marcado dividido, repetido o
//!   rotativo ([`prepare`]) el hilo se detiene entre piezas, avisa a la interfaz
//!   ([`GalvoEvent::WaitMove`]) y espera que el usuario mueva el material y confirme
//!   ([`GalvoHandle::confirm_move`]).
//! - **La tarjeta no informa su posición.** Se cuenta la última orden de salto; tras cualquier
//!   trabajo se desconoce.
//!
//! Coordenadas: la posición de la máquina galvo es la del documento (mm, Y hacia arriba, origen
//! abajo a la izquierda de la cama; el campo de la lente está centrado en la cama). El origen del
//! perfil no se aplica.
//!
//! Máquina virtual: el puerto `virtual:galvo` usa el emulador de la tarjeta. Su reloj avanza con
//! cada consulta de estado, así que cada lectura duerme un poco para marcar el paso ([`Paced`]).

use super::{Emit, Sink};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tlaku_core::Point;
use tlaku_core::toolpath::Toolpath;
use tlaku_doc::{
    BufferStatus, ConnectResult, ConnectionEvent, DeviceKind, DeviceProfile, JobEvent, JobState,
    LaserSource, LogDir, LogEvent, MachineState, Origin, ProgressEvent, StatusReport,
};
use tlaku_galvo::compile::{self, FieldOrigin, LightOptions, LightShape};
use tlaku_galvo::emu::{EmuConfig, emulator};
use tlaku_galvo::session::{
    BoardInfo, GalvoSession, JobControl, JobError, JobState as GalvoJobState, SessionConfig,
    SessionError, StopOutcome,
};
use tlaku_galvo::transport::{LaserSafetyAck, Transport, TransportError, TransportInfo};
use tlaku_galvo::usb::{self, UsbTimeouts, UsbTransport};
use tlaku_pipeline::{StageMove, job_mark_sequence};

/// Puerto de la máquina virtual galvo.
pub const VIRTUAL_PORT: &str = "virtual:galvo";

/// Prefijo de los puertos USB reales: `usb:galvo:<n>` es la tarjeta número `n` de la lista.
pub const USB_PREFIX: &str = "usb:galvo:";

/// Texto que delata, dentro del mensaje de un trabajo, que detener NO se confirmó. La interfaz lo
/// busca para destacar el aviso de cortar la energía de la fuente (viene de `JobError`).
pub const UNCONFIRMED_MARK: &str = "NO hubo confirmación";

/// Aviso que acompaña a todo lo que toca una tarjeta real.
pub const EXPERIMENTAL: &str = "EXPERIMENTAL: el soporte galvo no se ha probado con ninguna \
tarjeta real. Una fibra es láser clase 4 y su haz es INVISIBLE: usa gafas con la longitud de onda \
y la DO correctas, recinto cerrado y ten el paro de emergencia al alcance. Prueba primero sin la \
fuente conectada.";

/// Mensaje cuando una operación solo existe en GRBL.
pub const GRBL_ONLY: &str = "Esta función es solo de GRBL: el galvo no usa G-code ni overrides en \
vivo.";

/// Mensaje cuando se pide pausar o reanudar: no existe, a propósito.
pub const NO_PAUSE: &str = "El galvo no tiene pausa: pausar deja el espejo quieto con el haz \
encendido. Solo se puede detener.";

/// ¿Es el puerto de la máquina virtual galvo (con o sin `@velocidad`)?
pub fn is_virtual(port: &str) -> bool {
    port.split('@').next() == Some(VIRTUAL_PORT)
}

/// `virtual:galvo@5` -> 5 (veces el tiempo real); sin velocidad, 1.
fn virtual_speed(port: &str) -> Result<f64, String> {
    match port.split_once('@') {
        None => Ok(1.0),
        Some((_, s)) => match s.parse::<f64>() {
            Ok(v) if v.is_finite() && v > 0.0 && v <= super::virtual_port::MAX_SPEED => Ok(v),
            _ => Err(format!(
                "La velocidad '{s}' de la máquina virtual debe ser un número entre 0 y {} veces \
                 el tiempo real.",
                super::virtual_port::MAX_SPEED
            )),
        },
    }
}

/// Número de tarjeta de un puerto `usb:galvo:<n>`.
pub fn usb_index(port: &str) -> Option<usize> {
    port.strip_prefix(USB_PREFIX)?.parse().ok()
}

/// Puertos USB del galvo para la lista: una entrada por dispositivo BJJCZ (9588:xxxx) encontrado
/// (si no es compatible, abrirlo explica por qué). Un error al listar el USB no estropea la lista.
pub fn usb_ports() -> Vec<String> {
    usb::list_boards()
        .unwrap_or_default()
        .iter()
        .map(|b| format!("{USB_PREFIX}{}", b.index))
        .collect()
}

// ---------------------------------------------------------------------------
// Reconocimiento de seguridad
// ---------------------------------------------------------------------------

/// Lo que el usuario confirma en el diálogo de seguridad antes de abrir una tarjeta real. Las
/// TRES casillas deben estar marcadas; no se recuerda de una conexión a otra.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafetyAck {
    /// Lleva gafas con la densidad óptica correcta para la longitud de onda de la fuente.
    pub eyewear: bool,
    /// El láser está en un recinto cerrado.
    pub enclosure: bool,
    /// Sabe dónde está el paro de emergencia.
    pub emergency_stop: bool,
}

impl SafetyAck {
    /// El único lugar de la app que crea el [`LaserSafetyAck`] del USB: falla si falta alguna
    /// confirmación.
    pub fn token(&self) -> Result<LaserSafetyAck, String> {
        if self.eyewear && self.enclosure && self.emergency_stop {
            Ok(LaserSafetyAck::i_understand_laser_safety())
        } else {
            Err(ACK_REQUIRED.into())
        }
    }
}

/// Error cuando falta el reconocimiento de seguridad para una tarjeta real.
pub const ACK_REQUIRED: &str = "Para conectar con una tarjeta galvo real hay que confirmar la \
seguridad láser: gafas con la densidad óptica correcta para la longitud de onda, recinto cerrado \
y saber dónde está el paro de emergencia. Una fibra es clase 4 y su haz es invisible. Para \
probar sin máquina usa la máquina virtual (virtual:galvo).";

// ---------------------------------------------------------------------------
// Del perfil a la sesión
// ---------------------------------------------------------------------------

/// Configuración de la sesión a partir del perfil: límites del perfil y el campo centrado en la
/// cama (convención del pipeline).
pub fn session_config(device: &DeviceProfile) -> Result<SessionConfig, String> {
    if device.kind != DeviceKind::Galvo {
        return Err("El perfil de la máquina no es de tipo galvo.".into());
    }
    // La posición del galvo es la del documento: no se aplica ningún otro origen.
    if device.origin != Origin::BottomLeft {
        return Err(
            "El origen del perfil galvo debe ser «abajo a la izquierda»: el campo de la \
                    lente se coloca con las coordenadas del documento."
                .into(),
        );
    }
    let mut cfg = SessionConfig::new(device.galvo.clone())
        .map_err(|e| format!("El perfil del galvo no es válido: {e}"))?;
    cfg.origin = FieldOrigin::CenterAt(Point::new(device.bed[0] / 2.0, device.bed[1] / 2.0));
    Ok(cfg)
}

/// La parte del perfil que usa el driver. Lo que el pipeline calcula y la sesión NO usa (marcado
/// dividido y repetido, cilindro y cono) se descarta: se ignora al comprobar que el perfil no
/// cambió desde que se conectó.
fn driver_part(g: &tlaku_doc::GalvoDevice) -> tlaku_doc::GalvoDevice {
    let mut g = g.clone();
    g.cylinder = tlaku_doc::CylinderCorrection::default();
    g.taper = tlaku_doc::TaperWarp::default();
    g.split = tlaku_doc::SplitMarking::default();
    g.repeat = tlaku_doc::RepeatMarking::default();
    g
}

/// Rectángulo del campo (cuadrado de `field_mm` centrado en la cama), como `tlaku_pipeline::field_rect`.
fn field_bounds(bed: [f64; 2], field_mm: f64) -> tlaku_core::Bounds {
    tlaku_pipeline::field_rect(&DeviceProfile {
        bed,
        galvo: tlaku_doc::GalvoDevice {
            field_mm,
            ..tlaku_doc::GalvoDevice::default()
        },
        ..DeviceProfile::default()
    })
}

// ---------------------------------------------------------------------------
// Trabajo preparado (secuencia de pasos)
// ---------------------------------------------------------------------------

/// Un paso del marcado: un tramo del diseño, con el movimiento de material que lo precede.
#[derive(Debug, Clone)]
pub struct PreparedStep {
    pub info: StepInfo,
    pub toolpath: Toolpath,
}

/// Un trabajo listo para enviar: uno o varios pasos (marcado dividido, repetido o rotativo).
#[derive(Debug, Clone)]
pub struct PreparedJob {
    pub steps: Vec<PreparedStep>,
    /// Recorrido con el láser deshabilitado y el puntero rojo (nunca se arma la fuente).
    pub red_only: bool,
    pub warnings: Vec<String>,
    pub est_seconds: f64,
}

impl PreparedJob {
    /// Pasos que exigen que el usuario mueva el material (la tarjeta no tiene ejes).
    pub fn manual_moves(&self) -> usize {
        self.steps.iter().filter(|s| s.info.needs_move()).count()
    }
}

/// Qué paso es y qué hay que hacer antes de marcarlo. Viaja a la interfaz.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StepInfo {
    /// Paso actual y total (desde 1).
    pub step: usize,
    pub steps: usize,
    /// Repetición y pieza dentro de ella (desde 1) y sus totales.
    pub repeat: u32,
    pub repeats: u32,
    pub piece: usize,
    pub pieces: usize,
    /// Cuánto debe moverse el material (la mesa) en X e Y, mm.
    pub dx: f64,
    pub dy: f64,
    /// Giro del rotativo, grados.
    pub rotary_deg: f64,
    /// Pausa tras confirmar el movimiento, ms.
    pub pause_ms: f64,
}

impl StepInfo {
    /// ¿Hay que mover el material o el rotativo antes de marcar este paso?
    pub fn needs_move(&self) -> bool {
        self.dx != 0.0 || self.dy != 0.0 || self.rotary_deg != 0.0
    }
}

/// ¿El trabajo se marca en varios pasos (dividido, repetido o rotativo)?
pub fn is_sequenced(device: &DeviceProfile) -> bool {
    let g = &device.galvo;
    g.split.enabled || g.repeat.count > 1 || device.rotary.enabled
}

/// Calcula el trabajo con el pipeline. Con marcado dividido, repetido o rotativo recorre
/// `job_mark_sequence`: un paso por pieza y repetición, cada uno con `JobOptions.tile`. Una pieza
/// sin contenido se omite y su movimiento se suma al del paso siguiente (el material no se mueve
/// dos veces). Falla en español si el diseño no cabe en el campo o no hay nada que marcar.
pub fn prepare(
    doc: &tlaku_doc::Document,
    device: &DeviceProfile,
    options: &tlaku_doc::JobOptions,
    red_only: bool,
) -> Result<PreparedJob, String> {
    if device.kind != DeviceKind::Galvo {
        return Err("El perfil de la máquina no es de tipo galvo.".into());
    }
    let mut warnings = Vec::new();
    if !is_sequenced(device) {
        let job = tlaku_pipeline::compute_at(doc, device, options, None)?;
        warnings.extend(job.preview.warnings.iter().cloned());
        return Ok(PreparedJob {
            est_seconds: job.preview.total_s,
            steps: vec![PreparedStep {
                info: StepInfo {
                    step: 1,
                    steps: 1,
                    repeat: 1,
                    repeats: 1,
                    piece: 1,
                    pieces: 1,
                    dx: 0.0,
                    dy: 0.0,
                    rotary_deg: 0.0,
                    pause_ms: 0.0,
                },
                toolpath: job.toolpath,
            }],
            red_only,
            warnings,
        });
    }
    // El diseño entero (sin tramo) da la caja que se reparte en piezas.
    let whole_options = tlaku_doc::JobOptions {
        tile: None,
        ..options.clone()
    };
    let whole = tlaku_pipeline::compute_at(doc, device, &whole_options, None)?;
    warnings.extend(whole.preview.warnings.iter().cloned());
    let plan = job_mark_sequence(&whole, device)?;
    warnings.extend(plan.warnings.iter().cloned());
    let repeats = device.galvo.repeat.count.max(1);
    let pieces = plan
        .steps
        .iter()
        .map(|s| s.piece)
        .max()
        .map_or(1, |p| p + 1);
    let mut steps: Vec<PreparedStep> = Vec::new();
    let mut est = 0.0;
    // Movimiento de piezas omitidas que se suma al del siguiente paso con contenido.
    let mut carried = StageMove::default();
    let mut carried_pause = 0.0_f64;
    for s in &plan.steps {
        let step_options = tlaku_doc::JobOptions {
            tile: s.tile.clone(),
            ..options.clone()
        };
        let job = match tlaku_pipeline::compute_at(doc, device, &step_options, None) {
            Ok(j) => j,
            Err(e) if e.starts_with("no hay nada que producir") && s.tile.is_some() => {
                warnings.push(format!(
                    "La pieza {} de la repetición {} no tiene nada que marcar: se omite.",
                    s.piece + 1,
                    s.repeat + 1
                ));
                carried = StageMove {
                    dx: carried.dx + s.stage.dx,
                    dy: carried.dy + s.stage.dy,
                    rotary_deg: carried.rotary_deg + s.stage.rotary_deg,
                };
                carried_pause = carried_pause.max(s.pause_ms);
                continue;
            }
            Err(e) => return Err(e),
        };
        warnings.extend(job.preview.warnings.iter().cloned());
        est += job.preview.total_s;
        let stage = StageMove {
            dx: carried.dx + s.stage.dx,
            dy: carried.dy + s.stage.dy,
            rotary_deg: carried.rotary_deg + s.stage.rotary_deg,
        };
        let pause_ms = carried_pause.max(s.pause_ms);
        carried = StageMove::default();
        carried_pause = 0.0;
        steps.push(PreparedStep {
            info: StepInfo {
                step: 0,
                steps: 0,
                repeat: s.repeat + 1,
                repeats,
                piece: s.piece + 1,
                pieces,
                dx: stage.dx,
                dy: stage.dy,
                rotary_deg: stage.rotary_deg,
                pause_ms,
            },
            toolpath: job.toolpath,
        });
    }
    if steps.is_empty() {
        return Err(
            "no hay nada que producir: ninguna pieza del marcado dividido tiene contenido".into(),
        );
    }
    // El primer paso se marca con la mesa ya colocada: no lleva movimiento (aunque lo hayan
    // arrastrado piezas vacías, que tampoco se marcan).
    let total = steps.len();
    for (i, st) in steps.iter_mut().enumerate() {
        st.info.step = i + 1;
        st.info.steps = total;
    }
    if let Some(first) = steps.first_mut() {
        first.info.dx = 0.0;
        first.info.dy = 0.0;
        first.info.rotary_deg = 0.0;
        first.info.pause_ms = 0.0;
    }
    Ok(PreparedJob {
        steps,
        red_only,
        warnings,
        est_seconds: est,
    })
}

/// Lo que se sabe de un trabajo galvo antes de enviarlo (`galvo_preview`); no envía nada.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GalvoPreview {
    pub steps: usize,
    pub pieces: usize,
    pub repeats: u32,
    /// Veces que habrá que mover el material (o girar el rotativo) a mano.
    pub manual_moves: usize,
    pub est_seconds: f64,
    pub warnings: Vec<String>,
    pub field_mm: f64,
    pub source: LaserSource,
    /// Potencia máxima del perfil (%).
    pub max_power_pct: f64,
}

/// Calcula el trabajo (sin tarjeta) y devuelve su resumen para el diálogo de inicio.
pub fn preview(
    doc: &tlaku_doc::Document,
    device: &DeviceProfile,
    options: &tlaku_doc::JobOptions,
) -> Result<GalvoPreview, String> {
    let job = prepare(doc, device, options, false)?;
    let first = job.steps.first().map(|s| s.info.clone());
    Ok(GalvoPreview {
        steps: job.steps.len(),
        pieces: first.as_ref().map_or(1, |i| i.pieces),
        repeats: first.as_ref().map_or(1, |i| i.repeats),
        manual_moves: job.manual_moves(),
        est_seconds: job.est_seconds,
        warnings: job.warnings,
        field_mm: device.galvo.field_mm,
        source: device.galvo.source,
        max_power_pct: device.galvo.max_power_pct,
    })
}

/// La forma del encuadre con el puntero rojo: el rectángulo del trabajo o su contorno. Con
/// marcado dividido se encuadra la primera pieza.
pub fn light_shape(
    doc: &tlaku_doc::Document,
    device: &DeviceProfile,
    options: &tlaku_doc::JobOptions,
    contour: bool,
) -> Result<LightShape, String> {
    let job = prepare(doc, device, options, true)?;
    let tp = &job
        .steps
        .first()
        .ok_or("No hay nada que encuadrar: el trabajo está vacío.")?
        .toolpath;
    let shape = if contour {
        LightShape::contour_of(tp)
    } else {
        LightShape::bounds_of(tp)
    };
    shape.ok_or_else(|| "No hay nada que encuadrar: el trabajo está vacío.".to_string())
}

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
    /// Una vuelta más del encuadre con el puntero rojo.
    Light { loops: u32 },
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

// ---------------------------------------------------------------------------
// Mando
// ---------------------------------------------------------------------------

type Reply<T> = Sender<Result<T, String>>;

enum Request {
    /// Marca los pasos. Responde cuando sale la primera tanda (o antes, si falla).
    Run {
        job: Box<PreparedJob>,
        reply: Reply<()>,
    },
    /// Encuadre con el puntero rojo en bucle. Responde al terminar la primera vuelta.
    Light {
        shape: Box<LightShape>,
        reply: Reply<()>,
    },
    RedLight {
        on: bool,
        reply: Reply<()>,
    },
    /// Salto de los espejos a un punto del documento (mm), con la fuente desarmada.
    JogTo {
        x: f64,
        y: f64,
        reply: Reply<()>,
    },
    /// Detener y apagar todo sin trabajo en marcha.
    Stop(Reply<()>),
    Disconnect,
}

/// Datos que la sonda de solo lectura y el sondeo van actualizando.
#[derive(Default)]
struct Live {
    status_raw: u16,
    busy: bool,
    ready: bool,
    axis: bool,
    inputs: u16,
    inputs_known: bool,
    position: Option<[f64; 2]>,
}

#[derive(Default)]
struct Gate {
    waiting: Option<StepInfo>,
    confirmed: bool,
}

struct Shared {
    /// El hilo terminó (desconexión pedida o perdida).
    dead: AtomicBool,
    /// El hilo está ocupado con un trabajo, un encuadre o un salto.
    busy: AtomicBool,
    /// Se pidió detener (también interrumpe la espera de un movimiento de material).
    cancel: AtomicBool,
    /// El último apagado no se confirmó.
    laser_may_be_on: AtomicBool,
    /// El puntero rojo quedó encendido en su sitio.
    red_on: AtomicBool,
    live: Mutex<Live>,
    status: Mutex<Option<StatusReport>>,
    gate: Mutex<Gate>,
    gate_cv: Condvar,
    control: JobControl,
    virtual_machine: bool,
    board: String,
    transport: TransportInfo,
    serial: String,
    version: String,
    /// Lo que se usó al conectar: si el perfil cambia, hay que reconectar.
    cfg: SessionConfig,
    bed: [f64; 2],
}

/// Mando de la conexión; se clona y se usa desde cualquier hilo. Las llamadas bloquean hasta que
/// el hilo responde (en Tauri, dentro de `spawn_blocking`).
#[derive(Clone)]
pub struct GalvoHandle {
    tx: Sender<Request>,
    shared: Arc<Shared>,
}

impl GalvoHandle {
    fn call<T>(&self, build: impl FnOnce(Reply<T>) -> Request) -> Result<T, String> {
        if self.is_closed() {
            return Err(super::NOT_CONNECTED.into());
        }
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(build(tx))
            .map_err(|_| super::NOT_CONNECTED.to_string())?;
        rx.recv()
            .map_err(|_| "Se perdió la conexión con la máquina.".to_string())?
    }

    pub fn is_closed(&self) -> bool {
        self.shared.dead.load(Ordering::SeqCst)
    }

    #[cfg(test)]
    pub fn is_virtual(&self) -> bool {
        self.shared.virtual_machine
    }

    fn working(&self) -> bool {
        self.shared.busy.load(Ordering::SeqCst)
            || self.shared.control.state() != GalvoJobState::Idle
    }

    /// Falla si el hilo está ocupado: las órdenes de movimiento y de inicio solo valen en reposo.
    fn ensure_free(&self) -> Result<(), String> {
        if self.working() {
            Err(
                "La máquina está ocupada (trabajo, encuadre o salto en curso). Espera a que \
                 termine o detenla."
                    .into(),
            )
        } else {
            Ok(())
        }
    }

    /// Posición del haz según la última orden de salto (mm del documento), si se conoce.
    pub fn work_pos(&self) -> Option<[f64; 2]> {
        self.shared.live.lock().ok()?.position
    }

    pub fn status_report(&self) -> Option<StatusReport> {
        self.shared.status.lock().ok()?.clone()
    }

    pub fn state(&self) -> GalvoState {
        let sh = &self.shared;
        let (live_status, busy, ready, axis, inputs, inputs_known, pos) =
            sh.live
                .lock()
                .map_or((0, false, false, false, 0, false, None), |l| {
                    (
                        l.status_raw,
                        l.busy,
                        l.ready,
                        l.axis,
                        l.inputs,
                        l.inputs_known,
                        l.position,
                    )
                });
        let center = [sh.bed[0] / 2.0, sh.bed[1] / 2.0];
        GalvoState {
            board: sh.board.clone(),
            vendor_id: sh.transport.vendor_id,
            product_id: sh.transport.product_id,
            virtual_machine: sh.virtual_machine,
            serial: sh.serial.clone(),
            version: sh.version.clone(),
            status_raw: live_status,
            busy,
            ready,
            axis,
            inputs,
            inputs_known,
            working: self.working(),
            red_light_on: sh.red_on.load(Ordering::SeqCst),
            laser_may_be_on: sh.laser_may_be_on.load(Ordering::SeqCst),
            position: pos.unwrap_or(center),
            position_known: pos.is_some(),
            field_mm: sh.cfg.device.field_mm,
            source: sh.cfg.device.source,
            waiting_move: sh.gate.lock().ok().and_then(|g| g.waiting.clone()),
        }
    }

    /// El perfil debe seguir siendo el de la conexión en lo que usa el driver (campo, fuente,
    /// límites, corrección, cama). El marcado dividido y repetido, el cilindro y el cono los
    /// calcula el pipeline y pueden cambiar sin reconectar.
    fn check_profile(&self, device: &DeviceProfile) -> Result<(), String> {
        if device.kind != DeviceKind::Galvo {
            return Err("El perfil de la máquina no es de tipo galvo.".into());
        }
        let now = (driver_part(&device.galvo), device.bed);
        let was = (driver_part(&self.shared.cfg.device), self.shared.bed);
        if now != was {
            return Err(
                "El perfil de la máquina cambió (campo, fuente, límites, corrección o \
                        cama) desde que te conectaste. Desconecta y vuelve a conectar para \
                        aplicarlo."
                    .into(),
            );
        }
        Ok(())
    }

    fn arm_request(&self) {
        // Un trabajo nuevo no hereda un «detener» anterior.
        self.shared.control.clear();
        self.shared.cancel.store(false, Ordering::SeqCst);
    }

    /// Calcula el trabajo y lo marca. Vuelve cuando sale la primera tanda (o falla antes); el final
    /// llega como evento. `red_only`: recorrido con el láser deshabilitado y el puntero rojo.
    pub fn start_job(
        &self,
        doc: &tlaku_doc::Document,
        device: &DeviceProfile,
        options: &tlaku_doc::JobOptions,
        red_only: bool,
    ) -> Result<(), String> {
        self.check_profile(device)?;
        self.ensure_free()?;
        let job = prepare(doc, device, options, red_only)?;
        self.arm_request();
        self.call(|reply| Request::Run {
            job: Box::new(job),
            reply,
        })
    }

    /// Encuadre con el puntero rojo (rectángulo o contorno) en bucle hasta detener. Vuelve al
    /// terminar la primera vuelta; nunca habilita el láser.
    pub fn frame_job(
        &self,
        doc: &tlaku_doc::Document,
        device: &DeviceProfile,
        options: &tlaku_doc::JobOptions,
        contour: bool,
    ) -> Result<(), String> {
        self.check_profile(device)?;
        self.ensure_free()?;
        let shape = light_shape(doc, device, options, contour)?;
        // Se valida el encuadre (campo) antes de tocar la tarjeta.
        compile::compile_light(
            &shape,
            &self.shared.cfg.device,
            &LightOptions {
                origin: self.shared.cfg.origin,
            },
        )
        .map_err(|e| e.to_string())?;
        self.arm_request();
        self.call(|reply| Request::Light {
            shape: Box::new(shape),
            reply,
        })
    }

    /// Enciende o apaga el puntero rojo en su sitio (la fuente queda deshabilitada).
    pub fn set_red_light(&self, on: bool) -> Result<(), String> {
        // Con algo en marcha no se encola (la llamada se quedaría esperando al final del trabajo y
        // borraría un «detener» en vuelo): para apagarlo todo está Detener.
        if self.working() {
            return Err(if on {
                "La máquina está ocupada (trabajo, encuadre o salto en curso). Espera a que \
                 termine o detenla."
                    .into()
            } else {
                "Hay un trabajo o un encuadre en curso: pulsa Detener, que apaga la fuente y el \
                 puntero."
                    .into()
            });
        }
        self.arm_request();
        self.call(|reply| Request::RedLight { on, reply })
    }

    /// Salto relativo del haz (mm) con la fuente desarmada. Parte de la última orden de salto, o
    /// del centro del campo si no hay ninguna.
    pub fn jog(&self, dx: f64, dy: f64, _speed_mm_min: f64) -> Result<(), String> {
        if !(dx.is_finite() && dy.is_finite()) {
            return Err("El desplazamiento no es válido.".into());
        }
        let base = self
            .work_pos()
            .unwrap_or([self.shared.bed[0] / 2.0, self.shared.bed[1] / 2.0]);
        self.jog_to(base[0] + dx, base[1] + dy, _speed_mm_min)
    }

    /// Salto absoluto del haz (mm del documento) con la fuente desarmada. Debe caer en el campo.
    pub fn jog_to(&self, x: f64, y: f64, _speed_mm_min: f64) -> Result<(), String> {
        if !(x.is_finite() && y.is_finite()) {
            return Err("La posición no es válida.".into());
        }
        self.ensure_free()?;
        let f = field_bounds(self.shared.bed, self.shared.cfg.device.field_mm);
        if x < f.min.x || x > f.max.x || y < f.min.y || y > f.max.y {
            return Err(format!(
                "El punto ({x:.2}, {y:.2}) mm queda fuera del campo del galvo (X de {:.1} a \
                 {:.1}, Y de {:.1} a {:.1} mm).",
                f.min.x, f.max.x, f.min.y, f.max.y
            ));
        }
        self.arm_request();
        self.call(|reply| Request::JogTo { x, y, reply })
    }

    /// El usuario ya movió el material (o el rotativo): el trabajo sigue con la pieza siguiente.
    pub fn confirm_move(&self) -> Result<(), String> {
        if self.is_closed() {
            return Err(super::NOT_CONNECTED.into());
        }
        let mut g = self
            .shared
            .gate
            .lock()
            .map_err(|_| "estado interno dañado")?;
        if g.waiting.is_none() {
            return Err("Ningún paso del trabajo espera que muevas el material.".into());
        }
        g.confirmed = true;
        self.shared.gate_cv.notify_all();
        Ok(())
    }

    /// Detiene el trabajo, el encuadre o la espera y apaga la fuente y el puntero. Con algo en
    /// marcha vuelve en cuanto lo pide: el resultado (y si la tarjeta confirmó el apagado) llega
    /// como evento. En reposo detiene y apaga en el acto.
    pub fn cancel(&self) -> Result<(), String> {
        if self.is_closed() {
            return Err(super::NOT_CONNECTED.into());
        }
        self.shared.cancel.store(true, Ordering::SeqCst);
        self.shared.control.request_stop();
        self.shared.gate_cv.notify_all();
        if self.working() {
            return Ok(());
        }
        let r = self.call(Request::Stop);
        self.shared.cancel.store(false, Ordering::SeqCst);
        r
    }
}

/// Conexión abierta: el mando y el hilo.
pub struct GalvoConnection {
    handle: GalvoHandle,
    thread: Option<JoinHandle<()>>,
}

impl GalvoConnection {
    pub fn handle(&self) -> GalvoHandle {
        self.handle.clone()
    }

    fn request_close(&self) {
        // Un trabajo en curso se detiene (y la fuente se apaga) antes de cerrar.
        let sh = &self.handle.shared;
        sh.cancel.store(true, Ordering::SeqCst);
        sh.control.request_stop();
        sh.gate_cv.notify_all();
        let _ = self.handle.tx.send(Request::Disconnect);
    }

    /// Cierra la conexión (deteniendo antes un trabajo en curso) y espera al hilo.
    pub fn close(mut self) {
        self.request_close();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }

    /// Abre la tarjeta del puerto (`usb:galvo:<n>`, con el reconocimiento de seguridad) o la
    /// máquina virtual (`virtual:galvo`), la sondea (solo lectura) y arranca el hilo.
    ///
    /// Con una tarjeta real, sin `ack` completo falla ANTES de tocar el USB.
    pub fn connect(
        device: &DeviceProfile,
        port: &str,
        ack: Option<&SafetyAck>,
        sink: Arc<dyn Sink>,
        timing: GalvoTimingCfg,
    ) -> Result<(Self, ConnectResult), String> {
        let mut cfg = session_config(device)?;
        if is_virtual(port) {
            let (transport, _state) = emulator(EmuConfig {
                source: device.galvo.source,
                field_mm: device.galvo.field_mm,
                ..EmuConfig::default()
            });
            // El tiempo ya lo marca el paso de cada lectura: no hace falta esperar más.
            cfg.poll_interval = Duration::ZERO;
            let step = timing.virtual_pace.div_f64(virtual_speed(port)?);
            Self::start(
                Box::new(Paced {
                    inner: transport,
                    step,
                }),
                cfg,
                device.bed,
                port,
                sink,
                timing,
                true,
            )
        } else if let Some(index) = usb_index(port) {
            // El reconocimiento de seguridad se exige antes de abrir nada.
            let token = ack.ok_or_else(|| ACK_REQUIRED.to_string())?.token()?;
            let transport = UsbTransport::open(index, UsbTimeouts::default(), token)
                .map_err(|e| format!("No se pudo abrir la tarjeta galvo: {e}"))?;
            Self::start(
                Box::new(transport),
                cfg,
                device.bed,
                port,
                sink,
                timing,
                false,
            )
        } else {
            Err(format!(
                "«{port}» no es un puerto galvo. Elige una tarjeta USB de la lista o la máquina \
                 virtual."
            ))
        }
    }

    /// Sondea la tarjeta por `transport` (solo lectura) y arranca el hilo. `label` es lo que se
    /// muestra como puerto.
    pub fn start(
        transport: Box<dyn Transport>,
        cfg: SessionConfig,
        bed: [f64; 2],
        label: &str,
        sink: Arc<dyn Sink>,
        timing: GalvoTimingCfg,
        virtual_machine: bool,
    ) -> Result<(Self, ConnectResult), String> {
        let mut session = GalvoSession::new(transport, cfg.clone()).map_err(connect_error)?;
        log(sink.as_ref(), LogDir::Out, "; sondeo galvo (solo lectura)");
        let info = session.probe().map_err(connect_error)?;
        for line in describe_info(&info) {
            log(sink.as_ref(), LogDir::In, &line);
        }
        log(sink.as_ref(), LogDir::In, &format!("; {EXPERIMENTAL}"));
        if virtual_machine {
            log(
                sink.as_ref(),
                LogDir::In,
                "; Máquina virtual (tarjeta simulada): no hay láser ni hardware reales.",
            );
        }
        let serial = words_hex(&info.serial);
        let version = words_hex(&info.version);
        let firmware = format!(
            "Galvo · {} · {:04X}:{:04X}{}",
            info.board_name,
            info.transport.vendor_id,
            info.transport.product_id,
            if virtual_machine { " · virtual" } else { "" }
        );
        let live = Live {
            status_raw: info.status.raw,
            busy: info.status.busy,
            ready: info.status.ready,
            axis: info.status.axis,
            inputs: info.inputs,
            inputs_known: true,
            position: None,
        };
        let shared = Arc::new(Shared {
            dead: AtomicBool::new(false),
            busy: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            laser_may_be_on: AtomicBool::new(false),
            red_on: AtomicBool::new(false),
            live: Mutex::new(live),
            status: Mutex::new(None),
            gate: Mutex::new(Gate::default()),
            gate_cv: Condvar::new(),
            control: session.control(),
            virtual_machine,
            board: info.board_name.clone(),
            transport: info.transport.clone(),
            serial,
            version,
            cfg: cfg.clone(),
            bed,
        });
        let (tx, rx) = mpsc::channel();
        let worker = Worker {
            session,
            rx,
            shared: shared.clone(),
            sink: sink.clone(),
            timing,
            port: label.to_string(),
            bed,
            failures: 0,
            last_state: MachineState::Idle,
        };
        sink.emit(Emit::Connection(ConnectionEvent {
            connected: true,
            port: Some(label.to_string()),
            error: None,
        }));
        let thread = thread::Builder::new()
            .name("tlaku-galvo".into())
            .spawn(move || worker.run())
            .map_err(|e| format!("No se pudo iniciar el hilo de la conexión: {e}"))?;
        Ok((
            Self {
                handle: GalvoHandle { tx, shared },
                thread: Some(thread),
            },
            ConnectResult { firmware },
        ))
    }
}

impl Drop for GalvoConnection {
    fn drop(&mut self) {
        self.request_close();
    }
}

/// Transporte que duerme `step` en cada lectura: marca el paso del reloj virtual del emulador
/// (que avanza una vez por consulta de estado) contra el reloj de pared.
struct Paced<T> {
    inner: T,
    step: Duration,
}

impl<T: Transport> Transport for Paced<T> {
    fn write(&mut self, data: &[u8]) -> Result<(), TransportError> {
        self.inner.write(data)
    }

    fn read(&mut self) -> Result<[u8; tlaku_galvo::wire::REPLY_LEN], TransportError> {
        thread::sleep(self.step);
        self.inner.read()
    }

    fn info(&self) -> TransportInfo {
        self.inner.info()
    }
}

fn words_hex(w: &[u16; 4]) -> String {
    format!("{:04X}-{:04X}-{:04X}-{:04X}", w[0], w[1], w[2], w[3])
}

fn describe_info(info: &BoardInfo) -> Vec<String> {
    let t = &info.transport;
    let mut out = vec![
        format!(
            "Tarjeta: {} (USB {:04X}:{:04X}); serie {}; versión {}",
            info.board_name,
            t.vendor_id,
            t.product_id,
            words_hex(&info.serial),
            words_hex(&info.version)
        ),
        format!(
            "Estado: {:#06x} (ocupada: {}, lista: {}); entradas: {:#06x}",
            info.status.raw,
            if info.status.busy { "sí" } else { "no" },
            if info.status.ready { "sí" } else { "no" },
            info.inputs
        ),
    ];
    out.extend(info.notes.iter().map(|n| format!("; AVISO: {n}")));
    out
}

/// Error del sondeo al conectar, con lo que conviene revisar.
fn connect_error(e: SessionError) -> String {
    match &e {
        SessionError::UnknownBoard { .. } => {
            format!("{e}. Tlaku solo habla con la LMCV4-FIBER-M (USB 9588:9899).")
        }
        _ => format!(
            "No se pudo hablar con la tarjeta galvo: {e}{}",
            usb_hint(&e)
        ),
    }
}

fn usb_hint(e: &SessionError) -> &'static str {
    match e {
        SessionError::Transport(_) | SessionError::Timeout { .. } => {
            ". Comprueba el cable USB, que la máquina esté encendida y que EZCAD, LightBurn o \
             MeerK40t no tengan la tarjeta abierta"
        }
        _ => "",
    }
}

/// Error de una operación en marcha, con la pista del USB si es un silencio.
fn op_error(e: &SessionError) -> String {
    format!("{e}{}", usb_hint(e))
}

fn job_error_text(e: &JobError) -> String {
    format!("{e}{}", usb_hint(&e.cause))
}

fn log(sink: &dyn Sink, dir: LogDir, text: &str) {
    sink.emit(Emit::Log(LogEvent {
        dir,
        text: text.to_string(),
    }));
}

fn emit_job(sink: &dyn Sink, state: JobState, message: &str) {
    sink.emit(Emit::Job(JobEvent {
        state,
        message: message.to_string(),
        // El galvo no pierde posición: no hay carro que haya quedado a medio camino.
        position_lost: false,
    }));
}

// ---------------------------------------------------------------------------
// Hilo
// ---------------------------------------------------------------------------

struct Worker {
    session: GalvoSession<Box<dyn Transport>>,
    rx: Receiver<Request>,
    shared: Arc<Shared>,
    sink: Arc<dyn Sink>,
    timing: GalvoTimingCfg,
    port: String,
    bed: [f64; 2],
    /// Lecturas de estado seguidas que fallaron.
    failures: u32,
    last_state: MachineState,
}

impl Worker {
    fn run(mut self) {
        let mut next_poll = Instant::now() + self.timing.status_interval;
        let lost = loop {
            let wait = next_poll.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(wait) {
                Ok(Request::Disconnect) | Err(RecvTimeoutError::Disconnected) => break None,
                Ok(req) => {
                    self.shared.busy.store(true, Ordering::SeqCst);
                    self.handle(req);
                    // Por si alguna rama no lo soltó antes de responder.
                    self.shared.busy.store(false, Ordering::SeqCst);
                    next_poll = Instant::now() + self.timing.status_interval;
                }
                Err(RecvTimeoutError::Timeout) => {
                    if let Err(msg) = self.poll() {
                        break Some(msg);
                    }
                    next_poll = Instant::now() + self.timing.status_interval;
                }
            }
        };
        // Al cerrar, la fuente y el puntero se apagan SIEMPRE y se informa si no se confirmó.
        let mut error = lost;
        if self.shared.red_on.load(Ordering::SeqCst)
            || self.shared.laser_may_be_on.load(Ordering::SeqCst)
            || error.is_some()
        {
            let outcome = self.session.stop();
            self.shared.red_on.store(false, Ordering::SeqCst);
            if outcome == StopOutcome::Unconfirmed {
                self.shared.laser_may_be_on.store(true, Ordering::SeqCst);
                let note = format!(
                    "{UNCONFIRMED_MARK} de que la fuente quedara apagada: puede seguir habilitada. \
                     Corta la energía de la fuente láser y usa el paro de emergencia."
                );
                error = Some(match error {
                    Some(e) => format!("{e}. {note}"),
                    None => note,
                });
            }
        }
        self.shared.dead.store(true, Ordering::SeqCst);
        self.sink.emit(Emit::Connection(ConnectionEvent {
            connected: false,
            port: Some(self.port.clone()),
            error,
        }));
    }

    fn log(&self, dir: LogDir, text: &str) {
        log(self.sink.as_ref(), dir, text);
    }

    // ---- estado ------------------------------------------------------------

    fn center(&self) -> [f64; 2] {
        [self.bed[0] / 2.0, self.bed[1] / 2.0]
    }

    fn position(&self) -> Option<[f64; 2]> {
        self.shared.live.lock().ok().and_then(|l| l.position)
    }

    fn set_position(&self, p: Option<[f64; 2]>) {
        if let Ok(mut l) = self.shared.live.lock() {
            l.position = p;
        }
    }

    /// Publica el estado con la última posición conocida (o el centro del campo).
    fn publish(&mut self, state: MachineState) {
        self.last_state = state;
        let pos = self.position().unwrap_or_else(|| self.center());
        let report = StatusReport {
            state,
            other_state: None,
            machine_pos: pos,
            work_pos: pos,
            feed: 0.0,
            spindle: 0.0,
            // La tarjeta no informa búferes: ceros (la interfaz los oculta).
            buffer: BufferStatus { planner: 0, rx: 0 },
            overrides: None,
        };
        if let Ok(mut g) = self.shared.status.lock() {
            *g = Some(report.clone());
        }
        self.sink.emit(Emit::Status(report));
    }

    /// Un sondeo de estado y entradas en reposo (solo lectura). `Err` = conexión perdida.
    fn poll(&mut self) -> Result<(), String> {
        let result = self
            .session
            .status()
            .and_then(|st| self.session.read_inputs().map(|inputs| (st, inputs)));
        match result {
            Ok((st, inputs)) => {
                self.failures = 0;
                if let Ok(mut l) = self.shared.live.lock() {
                    l.status_raw = st.raw;
                    l.busy = st.busy;
                    l.ready = st.ready;
                    l.axis = st.axis;
                    l.inputs = inputs;
                    l.inputs_known = true;
                }
                let state = if st.busy {
                    MachineState::Run
                } else {
                    MachineState::Idle
                };
                self.publish(state);
                Ok(())
            }
            Err(SessionError::Transport(
                e @ (TransportError::Disconnected | TransportError::Io(_)),
            )) => Err(format!(
                "se perdió la conexión con la tarjeta galvo: {e}. Revisa el cable USB y que la \
                 máquina siga encendida"
            )),
            Err(e) => {
                self.failures += 1;
                if self.failures >= self.timing.lost_after {
                    Err(format!(
                        "se perdió la conexión con la tarjeta galvo ({e}). Revisa el cable USB y \
                         que la máquina siga encendida"
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }

    // ---- solicitudes -------------------------------------------------------

    fn handle(&mut self, req: Request) {
        match req {
            Request::Run { job, reply } => self.run_job(*job, reply),
            Request::Light { shape, reply } => self.run_light(&shape, reply),
            Request::RedLight { on, reply } => {
                let r = self.red_light(on);
                self.finish(reply, r);
            }
            Request::JogTo { x, y, reply } => {
                let r = self.jog_to(x, y);
                self.finish(reply, r);
            }
            Request::Stop(reply) => {
                let r = self.stop_idle();
                self.finish(reply, r);
            }
            Request::Disconnect => unreachable!("se atiende en el ciclo principal"),
        }
    }

    /// Responde a la interfaz. Suelta antes la marca de ocupado: quien recibe la respuesta puede
    /// pedir lo siguiente de inmediato y no debe toparse con un «ocupado» que ya terminó.
    fn finish<T>(&self, reply: Reply<T>, result: Result<T, String>) {
        self.shared.busy.store(false, Ordering::SeqCst);
        let _ = reply.send(result);
    }

    fn cancelled(&self) -> bool {
        self.shared.cancel.load(Ordering::SeqCst) || self.shared.control.stop_requested()
    }

    /// Anota que la fuente pudo quedar habilitada y lo hace saber (registro y evento propio).
    fn note_laser_may_be_on(&self, message: &str) {
        self.shared.laser_may_be_on.store(true, Ordering::SeqCst);
        self.log(
            LogDir::In,
            "; AVISO: la fuente pudo quedar habilitada: CORTA LA ENERGÍA de la fuente láser, \
             abre la tapa o usa el paro de emergencia",
        );
        self.sink.emit(Emit::Galvo(GalvoEvent::LaserMayBeOn {
            message: message.to_string(),
        }));
    }

    /// Tras un apagado confirmado, la marca de «puede estar encendido» se borra.
    fn note_safe(&self) {
        self.shared.laser_may_be_on.store(false, Ordering::SeqCst);
    }

    // ---- puntero y saltos --------------------------------------------------

    fn red_light(&mut self, on: bool) -> Result<(), String> {
        let r = self.session.set_red_light(on);
        match r {
            Ok(()) => {
                self.shared.red_on.store(on, Ordering::SeqCst);
                self.log(
                    LogDir::Out,
                    if on {
                        "; puntero rojo encendido (la fuente sigue deshabilitada)"
                    } else {
                        "; puntero rojo apagado"
                    },
                );
                if !on {
                    self.note_safe();
                }
                Ok(())
            }
            Err(e) => {
                self.shared.red_on.store(false, Ordering::SeqCst);
                let msg = job_error_text(&e);
                self.log(LogDir::In, &format!("; {msg}"));
                if e.laser_may_be_on() {
                    self.note_laser_may_be_on(&msg);
                }
                Err(msg)
            }
        }
    }

    fn jog_to(&mut self, x: f64, y: f64) -> Result<(), String> {
        self.publish(MachineState::Jog);
        let r = self.session.jog_to(Point::new(x, y));
        self.publish(MachineState::Idle);
        match r {
            Ok(()) => {
                self.set_position(Some([x, y]));
                Ok(())
            }
            Err(e) => {
                let msg = job_error_text(&e);
                if e.laser_may_be_on() {
                    self.note_laser_may_be_on(&msg);
                }
                Err(msg)
            }
        }
    }

    /// Detiene y apaga sin trabajo en marcha (secuencia de apagado completa).
    fn stop_idle(&mut self) -> Result<(), String> {
        let outcome = self.session.stop();
        self.shared.red_on.store(false, Ordering::SeqCst);
        self.set_position(None);
        self.publish(MachineState::Idle);
        match outcome {
            StopOutcome::Acked | StopOutcome::NotNeeded => {
                self.note_safe();
                self.log(
                    LogDir::Out,
                    "; detener: la tarjeta confirmó el apagado de la fuente y del puntero",
                );
                Ok(())
            }
            StopOutcome::Unconfirmed => {
                let msg = format!(
                    "{UNCONFIRMED_MARK} de que la fuente se apagara. Corta la energía de la \
                     fuente láser y usa el paro de emergencia."
                );
                self.note_laser_may_be_on(&msg);
                Err(msg)
            }
        }
    }

    // ---- encuadre ----------------------------------------------------------

    fn run_light(&mut self, shape: &LightShape, reply: Reply<()>) {
        let ctl = self.shared.control.clone();
        self.log(
            LogDir::Out,
            "; encuadre con el puntero rojo (la fuente NO se habilita); Detener para terminar",
        );
        self.publish(MachineState::Run);
        let mut reply = Some(reply);
        let sink = self.sink.clone();
        let every = self.timing.light_interval;
        let mut last_emit: Option<Instant> = None;
        let result = self.session.light(shape, &ctl, None, &mut |p| {
            if let Some(r) = reply.take() {
                let _ = r.send(Ok(()));
            }
            if last_emit.is_none_or(|t| t.elapsed() >= every) {
                last_emit = Some(Instant::now());
                sink.emit(Emit::Galvo(GalvoEvent::Light { loops: p.loops }));
            }
        });
        self.shared.red_on.store(false, Ordering::SeqCst);
        self.set_position(None);
        self.publish(MachineState::Idle);
        match result {
            Ok(report) => {
                self.note_safe();
                self.log(
                    LogDir::In,
                    &format!(
                        "; encuadre terminado: {} vueltas; puntero rojo apagado",
                        report.loops
                    ),
                );
                if let Some(r) = reply.take() {
                    self.finish(r, Ok(()));
                } else {
                    self.shared.busy.store(false, Ordering::SeqCst);
                }
            }
            Err(e) => {
                let msg = job_error_text(&e);
                self.log(LogDir::In, &format!("; {msg}"));
                if e.laser_may_be_on() {
                    self.note_laser_may_be_on(&msg);
                } else {
                    self.note_safe();
                }
                if let Some(r) = reply.take() {
                    self.finish(r, Err(msg));
                } else {
                    self.shared.busy.store(false, Ordering::SeqCst);
                    emit_job(self.sink.as_ref(), JobState::Error, &msg);
                }
            }
        }
    }

    // ---- marcado -----------------------------------------------------------

    /// Espera que el usuario mueva el material y confirme. `Err(())` = se pidió detener.
    fn wait_move(&mut self, info: &StepInfo) -> Result<(), ()> {
        if let Ok(mut g) = self.shared.gate.lock() {
            g.waiting = Some(info.clone());
            g.confirmed = false;
        }
        self.log(
            LogDir::In,
            &format!(
                "; paso {}/{}: mueve el material (X {:+.2} mm, Y {:+.2} mm, rotativo {:+.2}°) y \
                 confirma para seguir",
                info.step, info.steps, info.dx, info.dy, info.rotary_deg
            ),
        );
        self.sink
            .emit(Emit::Galvo(GalvoEvent::WaitMove { info: info.clone() }));
        let result = loop {
            if self.cancelled() {
                break Err(());
            }
            let Ok(g) = self.shared.gate.lock() else {
                break Err(());
            };
            let (mut g, _) = self
                .shared
                .gate_cv
                .wait_timeout(g, Duration::from_millis(50))
                .unwrap_or_else(|e| e.into_inner());
            if g.confirmed {
                g.confirmed = false;
                break Ok(());
            }
        };
        if let Ok(mut g) = self.shared.gate.lock() {
            g.waiting = None;
            g.confirmed = false;
        }
        result
    }

    /// Pausa entre el movimiento del material y la marca; se interrumpe al detener.
    fn sleep_cancelable(&self, ms: f64) -> Result<(), ()> {
        let end = Instant::now() + Duration::try_from_secs_f64(ms / 1000.0).unwrap_or_default();
        while Instant::now() < end {
            if self.cancelled() {
                return Err(());
            }
            thread::sleep(Duration::from_millis(5).min(end - Instant::now()));
        }
        if self.cancelled() { Err(()) } else { Ok(()) }
    }

    fn run_job(&mut self, job: PreparedJob, reply: Reply<()>) {
        let mut reply = Some(reply);
        // Todo se compila antes de tocar la tarjeta (y antes de pedir que muevan nada).
        let mut lists = Vec::with_capacity(job.steps.len());
        for s in &job.steps {
            match self.session.compile(&s.toolpath, job.red_only) {
                Ok(l) => lists.push(l),
                Err(e) => {
                    if let Some(r) = reply.take() {
                        self.finish(r, Err(op_error(&e)));
                    }
                    return;
                }
            }
        }
        let total_batches: usize = lists.iter().map(|l| l.batches().len()).sum();
        let ctl = self.shared.control.clone();
        self.log(
            LogDir::Out,
            &format!(
                "; {} galvo: {} paso(s), {} tanda(s) de 256 comandos",
                if job.red_only {
                    "recorrido en rojo (fuente deshabilitada)"
                } else {
                    "marcado"
                },
                job.steps.len(),
                total_batches
            ),
        );
        for w in &job.warnings {
            self.log(LogDir::In, &format!("; AVISO: {w}"));
        }
        self.publish(MachineState::Run);
        let mut done_batches = 0_usize;
        let mut outcome: Result<(), Box<JobError>> = Ok(());
        let mut cancelled_between = false;
        let mut started_event = false;
        let every = self.timing.progress_interval;
        for (i, (step, list)) in job.steps.iter().zip(&lists).enumerate() {
            if self.cancelled() {
                cancelled_between = true;
                break;
            }
            if step.info.needs_move() {
                if self.wait_move(&step.info).is_err() {
                    cancelled_between = true;
                    break;
                }
                if step.info.pause_ms > 0.0 && self.sleep_cancelable(step.info.pause_ms).is_err() {
                    cancelled_between = true;
                    break;
                }
            }
            self.sink.emit(Emit::Galvo(GalvoEvent::Step {
                info: step.info.clone(),
            }));
            let sink = self.sink.clone();
            let base = done_batches;
            let mut last_emit: Option<Instant> = None;
            let mut announce = (!started_event).then_some(job.steps.len());
            let r = self.session.send_job(list, &ctl, &mut |p| {
                if let Some(r) = reply.take() {
                    let _ = r.send(Ok(()));
                }
                if let Some(n) = announce.take() {
                    emit_job(
                        sink.as_ref(),
                        JobState::Started,
                        &format!("Enviando el trabajo ({n} paso(s))."),
                    );
                }
                let due = last_emit.is_none_or(|t| t.elapsed() >= every) || p.batch == p.batches;
                if due {
                    last_emit = Some(Instant::now());
                    sink.emit(Emit::Progress(ProgressEvent {
                        acked: u32::try_from(base + p.batch).unwrap_or(u32::MAX),
                        total: u32::try_from(total_batches).unwrap_or(u32::MAX),
                    }));
                }
            });
            started_event = true;
            self.set_position(None);
            match r {
                Ok(report) => {
                    done_batches += report.batches;
                    self.log(
                        LogDir::In,
                        &format!(
                            "; paso {}/{} enviado: {} tandas, {} comandos, en {:.1} s{}",
                            i + 1,
                            job.steps.len(),
                            report.batches,
                            report.commands,
                            report.elapsed.as_secs_f64(),
                            if report.armed {
                                " (la fuente estuvo armada durante el paso)"
                            } else {
                                " (la fuente NO se armó)"
                            }
                        ),
                    );
                }
                Err(e) => {
                    outcome = Err(e);
                    break;
                }
            }
        }
        self.shared.red_on.store(false, Ordering::SeqCst);
        self.publish(MachineState::Idle);
        match outcome {
            Ok(()) if cancelled_between => {
                self.note_safe();
                self.end_job(
                    &mut reply,
                    JobState::Cancelled,
                    "Trabajo detenido: la fuente está apagada. (Se detuvo entre pasos del marcado dividido o repetido.)",
                    false,
                );
            }
            Ok(()) => {
                self.note_safe();
                self.end_job(
                    &mut reply,
                    JobState::Finished,
                    &if job.red_only {
                        "Recorrido en rojo terminado: la fuente nunca se armó.".to_string()
                    } else {
                        "Trabajo terminado: la fuente se apagó y la lista se vació.".to_string()
                    },
                    false,
                );
            }
            Err(e) => {
                let msg = job_error_text(&e);
                self.log(LogDir::In, &format!("; {msg}"));
                if e.laser_may_be_on() {
                    self.note_laser_may_be_on(&msg);
                    self.end_job(&mut reply, JobState::Error, &msg, true);
                } else if matches!(e.cause, SessionError::Cancelled) {
                    self.note_safe();
                    self.end_job(
                        &mut reply,
                        JobState::Cancelled,
                        &format!(
                            "Trabajo detenido: la fuente se apagó. (Tandas enviadas: {} de {}.)",
                            e.batches_sent, e.batches_total
                        ),
                        false,
                    );
                } else {
                    self.note_safe();
                    self.end_job(&mut reply, JobState::Error, &msg, true);
                }
            }
        }
    }

    /// Termina un trabajo: si la interfaz aún espera la respuesta de `start_job` (falló antes de
    /// la primera tanda) es un rechazo del envío; si no, un evento del trabajo.
    fn end_job(
        &mut self,
        reply: &mut Option<Reply<()>>,
        state: JobState,
        msg: &str,
        is_error: bool,
    ) {
        // Antes del evento: quien lo recibe puede pedir lo siguiente de inmediato.
        self.shared.busy.store(false, Ordering::SeqCst);
        if let Some(r) = reply.take() {
            if is_error {
                let _ = r.send(Err(msg.to_string()));
            } else {
                // Cancelado o terminado antes de la primera tanda: el envío se aceptó.
                let _ = r.send(Ok(()));
                emit_job(self.sink.as_ref(), state, msg);
            }
        } else {
            emit_job(self.sink.as_ref(), state, msg);
        }
    }
}

#[cfg(test)]
mod tests;
