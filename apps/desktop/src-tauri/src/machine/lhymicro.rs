//! Conexión con un K40 de placa Lihuiyu M2 Nano (LHYMICRO-GL por USB). EXPERIMENTAL: no se ha
//! probado con una placa real; sí contra el emulador de `tlaku-lhymicro`.
//!
//! Igual que las demás, es un hilo dedicado dueño de la sesión ([`LhySession`]) que habla con la
//! interfaz solo por eventos `machine://*` (el mismo [`Sink`]). Lo que la distingue:
//!
//! - **`send_job` bloquea** mientras dura el trabajo, porque la placa recibe los datos a medida
//!   que los ejecuta. Por eso el hilo reparte el trabajo así: responde a la interfaz en cuanto
//!   sale el primer paquete y sigue dentro de `send_job` hasta el final. La pausa, la reanudación
//!   y la detención no pasan por el hilo: usan el [`JobControl`] clonable, que `send_job`
//!   atiende entre paquetes y en cada consulta de estado.
//! - **La placa no informa su posición.** La sesión la cuenta desde el home. Mientras no se haya
//!   hecho home (o tras detener o fallar un trabajo) se desconoce y no se mueve nada; empezar un
//!   trabajo en ese caso hace home primero.
//! - **La potencia no se controla por software** (se ajusta con la perilla del K40): el
//!   codificador avisa ([`preview`]) y la interfaz lo muestra antes de enviar.
//! - Los saltos y el encuadre van a la velocidad de salto de la placa; la velocidad de la
//!   interfaz no se usa.
//!
//! Si detener no se confirma (`StopOutcome::Unconfirmed`) el mensaje del trabajo trae
//! [`UNCONFIRMED_MARK`]: la interfaz lo busca para mostrar el aviso del paro de emergencia.
//!
//! Máquina virtual: el puerto `virtual:k40[@<velocidad>]` usa el emulador de la placa. El reloj
//! del emulador avanza con cada consulta de estado, así que se marca el paso durmiendo un poco en
//! cada lectura (velocidad 1 = tiempo real aproximado).

use super::{Emit, Sink};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tlaku_core::Point;
use tlaku_core::toolpath::Toolpath;
use tlaku_doc::{
    BufferStatus, ConnectResult, ConnectionEvent, DeviceProfile, JobEvent, JobState, LogDir,
    LogEvent, MachineState, Origin, ProgressEvent, StatusReport,
};
use tlaku_lhymicro::emu::{EmuConfig, emulator};
use tlaku_lhymicro::encode::{EncodeConfig, HomeCorner, Program, Warning, encode, mil_to_mm};
use tlaku_lhymicro::session::{
    BoardInfo, JobControl, JobState as LhyJobState, LhySession, SessionConfig, SessionError,
    StopOutcome,
};
use tlaku_lhymicro::speed::Board;
use tlaku_lhymicro::status::Status;
use tlaku_lhymicro::transport::{Transport, TransportError, TransportInfo};
use tlaku_lhymicro::usb::{self, UsbTimeouts, UsbTransport};

/// Puerto de la máquina virtual K40 (`virtual:k40` o `virtual:k40@5`).
pub const VIRTUAL_PORT: &str = "virtual:k40";

/// Prefijo de los puertos USB reales: `usb:k40:<n>` es la placa número `n` de la lista.
pub const USB_PREFIX: &str = "usb:k40:";

/// Texto que delata, dentro del mensaje de un trabajo, que detener NO se confirmó. La interfaz
/// lo busca para destacar el aviso del paro de emergencia (viene de `JobError`).
pub const UNCONFIRMED_MARK: &str = "NO hubo confirmación";

/// Aviso que acompaña a todo lo que toca una placa real.
pub const EXPERIMENTAL: &str = "EXPERIMENTAL: el soporte del K40 (M2 Nano) no se ha probado con \
una placa real. La M2 Nano de serie no controla la potencia por software: se ajusta con la perilla \
del K40. Ten el paro de emergencia al alcance y prueba primero con el láser deshabilitado o muy \
débil.";

/// Mensaje cuando una operación solo existe en GRBL.
pub const GRBL_ONLY: &str = "Esta función es solo de GRBL: la placa M2 Nano del K40 no usa G-code \
ni overrides en vivo.";

/// Mensaje cuando no se sabe dónde está el cabezal.
pub const UNKNOWN_POSITION: &str = "No se sabe dónde está el cabezal (la M2 Nano no informa su \
posición, se cuenta desde el home). Haz home primero.";

/// ¿Es el puerto de la máquina virtual K40 (con o sin `@velocidad`)?
pub fn is_virtual(port: &str) -> bool {
    port.split('@').next() == Some(VIRTUAL_PORT)
}

/// Número de placa de un puerto `usb:k40:<n>`.
pub fn usb_index(port: &str) -> Option<usize> {
    port.strip_prefix(USB_PREFIX)?.parse().ok()
}

/// Puertos USB del K40 para la lista: una entrada por placa encontrada. Un error al listar el USB
/// (permisos, sin bus) no estropea la lista: solo no hay placas.
pub fn usb_ports() -> Vec<String> {
    usb::list_boards()
        .unwrap_or_default()
        .iter()
        .map(|b| format!("{USB_PREFIX}{}", b.index))
        .collect()
}

// ---------------------------------------------------------------------------
// Del perfil a la sesión
// ---------------------------------------------------------------------------

/// Esquina del home a partir del origen del perfil: el home de un K40 de serie es arriba a la
/// izquierda. El origen del perfil es donde la máquina pone su (0, 0), o sea, el home.
pub fn home_for_origin(origin: Origin) -> HomeCorner {
    match origin {
        Origin::TopLeft => HomeCorner::TopLeft,
        Origin::TopRight => HomeCorner::TopRight,
        Origin::BottomLeft => HomeCorner::BottomLeft,
        Origin::BottomRight => HomeCorner::BottomRight,
    }
}

/// Modelo de placa del perfil.
pub fn board_of(device: &DeviceProfile) -> Result<Board, String> {
    Board::parse(&device.lhymicro.board).ok_or_else(|| {
        format!(
            "El modelo de placa «{}» no existe. Usa A, B, B1, B2, M, M1, M2 (la de serie) o M3.",
            device.lhymicro.board
        )
    })
}

/// Velocidad máxima de corte (mm/s) a partir del tope del perfil (mm/min); 140 mm/s si el perfil
/// no tiene uno utilizable.
fn max_speed_mm_s(device: &DeviceProfile) -> f64 {
    let rate = device.max_rate[0].max(device.max_rate[1]);
    if rate.is_finite() && rate > 0.0 {
        rate / 60.0
    } else {
        EncodeConfig::default().max_speed_mm_s
    }
}

/// Configuración de la sesión a partir del perfil: placa, cama, home y tope de velocidad.
pub fn session_config(device: &DeviceProfile) -> Result<SessionConfig, String> {
    Ok(SessionConfig {
        board: board_of(device)?,
        bed_mm: device.bed,
        home: home_for_origin(device.origin),
        max_speed_mm_s: max_speed_mm_s(device),
        ..SessionConfig::default()
    })
}

/// Configuración del codificador con la que se calcula el aviso previo; `start_mm` es dónde está
/// el cabezal (en mm de la máquina) o `None` si se hará home primero.
fn encode_config(
    device: &DeviceProfile,
    start_mm: Option<[f64; 2]>,
) -> Result<EncodeConfig, String> {
    let cfg = session_config(device)?;
    let mil = |v: f64| tlaku_lhymicro::encode::mm_to_mil(v).max(0);
    Ok(EncodeConfig {
        board: cfg.board,
        bed_mm: cfg.bed_mm,
        home: cfg.home,
        autolock: cfg.autolock,
        max_speed_mm_s: cfg.max_speed_mm_s,
        constant_speed: cfg.constant_speed,
        fix_speeds: cfg.fix_speeds,
        start_mil: start_mm.map_or((0, 0), |[x, y]| (mil(x), mil(y))),
        ..EncodeConfig::default()
    })
}

// ---------------------------------------------------------------------------
// Aviso previo al envío
// ---------------------------------------------------------------------------

/// Un aviso del codificador, con lo justo para que la interfaz lo escriba en su idioma.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LhyWarning {
    /// `power_ignored`, `gray_thresholded`, `air_assist_ignored`, `z_ignored` o `dwell_skipped`.
    pub kind: &'static str,
    /// Número del bloque del trabajo (desde 0); es el mismo orden de las capas calculadas.
    pub block: usize,
    /// Nombre de la capa del bloque, si se conoce.
    pub layer: Option<String>,
    /// Potencia pedida (%) en `power_ignored`; milisegundos en `dwell_skipped`.
    pub value: Option<f64>,
    /// Solo en `dwell_skipped`: era un pulso (el láser disparaba) y no una pausa.
    pub fires: bool,
}

/// Lo que se sabe de un trabajo antes de enviarlo a un K40.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LhyPreview {
    pub board: String,
    pub warnings: Vec<LhyWarning>,
    pub packets: usize,
    pub bytes: usize,
    /// Tiempo estimado por exceso (s), con saltos rápidos a 40 mm/s.
    pub est_seconds: f64,
    pub burn_mm: f64,
    pub travel_mm: f64,
    /// La posición del cabezal se desconoce: el envío hará home primero.
    pub homes_first: bool,
}

fn to_warning(w: &Warning, layers: &[String]) -> LhyWarning {
    let (kind, block, value, fires) = match *w {
        Warning::PowerIgnored { block, pct } => ("power_ignored", block, Some(pct), false),
        Warning::GrayThresholded { block } => ("gray_thresholded", block, None, false),
        Warning::AirAssistIgnored { block } => ("air_assist_ignored", block, None, false),
        Warning::ZIgnored { block } => ("z_ignored", block, None, false),
        Warning::DwellSkipped { block, ms, fires } => ("dwell_skipped", block, Some(ms), fires),
    };
    LhyWarning {
        kind,
        block,
        layer: layers.get(block).cloned(),
        value,
        fires,
    }
}

/// Calcula el trabajo, lo codifica (sin enviar nada) y devuelve sus avisos. `head` es la posición
/// del cabezal en mm de la máquina si se conoce.
pub fn preview(
    doc: &tlaku_doc::Document,
    device: &DeviceProfile,
    options: &tlaku_doc::JobOptions,
    head: Option<[f64; 2]>,
) -> Result<LhyPreview, String> {
    let at = head.map(|[x, y]| Point::new(x, y));
    let job = tlaku_pipeline::compute_at(doc, device, options, at)?;
    let cfg = encode_config(device, head)?;
    let program = encode(&job.toolpath, &cfg).map_err(|e| e.to_string())?;
    let frames = program.frames().map_err(|e| e.to_string())?;
    let layers: Vec<String> = job.layers.iter().map(|l| l.name.clone()).collect();
    Ok(LhyPreview {
        board: program.board.name().to_string(),
        warnings: program
            .warnings
            .iter()
            .map(|w| to_warning(w, &layers))
            .collect(),
        packets: frames.len(),
        bytes: program.stream.len(),
        est_seconds: program.stats.est_seconds,
        burn_mm: program.stats.burn_mm,
        travel_mm: program.stats.travel_mm,
        homes_first: head.is_none(),
    })
}

/// Estado de la conexión para el panel (lo que la interfaz no puede deducir de los eventos).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LhyState {
    pub board: String,
    pub virtual_machine: bool,
    /// Se conoce la posición del cabezal (se cuenta desde el último home).
    pub position_known: bool,
    /// Último código de estado de la placa (206 lista, 238 ocupada, 236 terminó, 239 sin
    /// energía...); 0 si aún no hay lectura.
    pub status_code: u8,
}

// ---------------------------------------------------------------------------
// Tiempos
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LhyTiming {
    /// Cada cuánto se lee el estado en reposo.
    pub status_interval: Duration,
    /// Lecturas seguidas sin respuesta tras las cuales se da por perdida la conexión.
    pub lost_after: u32,
    /// Cuánto espera la pausa o la reanudación a que la placa las confirme.
    pub pause_wait: Duration,
    /// Mínimo entre dos eventos de progreso (el último siempre sale).
    pub progress_interval: Duration,
}

impl Default for LhyTiming {
    fn default() -> Self {
        Self {
            status_interval: Duration::from_millis(500),
            lost_after: 3,
            pause_wait: Duration::from_secs(3),
            progress_interval: Duration::from_millis(100),
        }
    }
}

// ---------------------------------------------------------------------------
// Mando
// ---------------------------------------------------------------------------

type Reply<T> = Sender<Result<T, String>>;

enum Request {
    /// Codifica y envía. Responde cuando sale el primer paquete (o antes, si falla).
    Start {
        toolpath: Box<Toolpath>,
        reply: Reply<()>,
    },
    /// Recorre las esquinas (mm de la máquina) con el láser apagado. Responde al empezar.
    Frame {
        corners: Vec<(f64, f64)>,
        reply: Reply<()>,
    },
    Home(Reply<()>),
    Jog {
        dx: f64,
        dy: f64,
        reply: Reply<()>,
    },
    JogTo {
        x: f64,
        y: f64,
        reply: Reply<()>,
    },
    /// Detener sin trabajo en marcha (`I` e `IUS1P`).
    Stop(Reply<()>),
    Disconnect,
}

struct Shared {
    /// El hilo terminó (desconexión pedida o perdida).
    dead: AtomicBool,
    /// El hilo está ocupado con un trabajo, un home o un encuadre.
    busy: AtomicBool,
    /// Se pidió detener: interrumpe un encuadre y un arranque que aún no envió nada.
    cancel: AtomicBool,
    /// Se conoce la posición del cabezal.
    pos_known: AtomicBool,
    /// Último código de estado de la placa.
    status_code: Mutex<u8>,
    status: Mutex<Option<StatusReport>>,
    control: JobControl,
    sink: Arc<dyn Sink>,
    virtual_machine: bool,
    board: Board,
    /// Lo que se usó al conectar: si el perfil cambia, hay que reconectar.
    cfg: SessionConfig,
    timing: LhyTiming,
}

/// Mando de la conexión; se clona y se usa desde cualquier hilo. Las llamadas bloquean hasta que
/// el hilo responde (en Tauri, dentro de `spawn_blocking`).
#[derive(Clone)]
pub struct LhyHandle {
    tx: Sender<Request>,
    shared: Arc<Shared>,
}

impl LhyHandle {
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

    /// Hay un trabajo en marcha (enviándose o en pausa).
    fn job_running(&self) -> bool {
        self.shared.control.state() != LhyJobState::Idle
    }

    /// Falla si el hilo está ocupado: las órdenes de movimiento solo valen en reposo.
    fn ensure_free(&self) -> Result<(), String> {
        if self.shared.busy.load(Ordering::SeqCst) || self.job_running() {
            Err("La máquina está ocupada (trabajo, home o encuadre en curso). Espera a que termine o detenla.".into())
        } else {
            Ok(())
        }
    }

    /// Posición del cabezal en mm del sistema de la máquina, si se conoce.
    pub fn work_pos(&self) -> Option<[f64; 2]> {
        if !self.shared.pos_known.load(Ordering::SeqCst) {
            return None;
        }
        self.shared.status.lock().ok()?.as_ref().map(|s| s.work_pos)
    }

    pub fn status_report(&self) -> Option<StatusReport> {
        self.shared.status.lock().ok()?.clone()
    }

    pub fn state(&self) -> LhyState {
        LhyState {
            board: self.shared.board.name().to_string(),
            virtual_machine: self.shared.virtual_machine,
            position_known: self.shared.pos_known.load(Ordering::SeqCst),
            status_code: self.shared.status_code.lock().map_or(0, |g| *g),
        }
    }

    /// El perfil debe seguir siendo el de la conexión (placa, cama, home y tope de velocidad).
    fn check_profile(&self, device: &DeviceProfile) -> Result<(), String> {
        let now = session_config(device)?;
        let was = &self.shared.cfg;
        if now.board != was.board
            || now.bed_mm != was.bed_mm
            || now.home != was.home
            || (now.max_speed_mm_s - was.max_speed_mm_s).abs() > 1e-9
        {
            return Err(
                "El perfil de la máquina cambió (placa, cama, origen o velocidad máxima) \
                        desde que te conectaste. Desconecta y vuelve a conectar para aplicarlo."
                    .into(),
            );
        }
        Ok(())
    }

    /// Calcula el trabajo, lo codifica y lo envía. Vuelve cuando sale el primer paquete; el final
    /// del trabajo llega como evento. Si no se conoce la posición, hace home antes.
    pub fn start_job(
        &self,
        doc: &tlaku_doc::Document,
        device: &DeviceProfile,
        options: &tlaku_doc::JobOptions,
    ) -> Result<(), String> {
        self.check_profile(device)?;
        self.ensure_free()?;
        let at = self.work_pos().map(|[x, y]| Point::new(x, y));
        let job = tlaku_pipeline::compute_at(doc, device, options, at)?;
        self.call(|reply| Request::Start {
            toolpath: Box::new(job.toolpath),
            reply,
        })
    }

    /// Encuadre del trabajo: recorre su rectángulo con el láser apagado. Vuelve al empezar.
    pub fn frame_job(
        &self,
        doc: &tlaku_doc::Document,
        device: &DeviceProfile,
        options: &tlaku_doc::JobOptions,
    ) -> Result<(), String> {
        self.check_profile(device)?;
        self.ensure_free()?;
        let corners = super::ruida::frame_corners(doc, device, options, self.work_pos())?;
        self.call(|reply| Request::Frame { corners, reply })
    }

    /// Lleva el cabezal al home (finales de carrera). Vuelve al terminar.
    pub fn home(&self) -> Result<(), String> {
        self.ensure_free()?;
        self.call(Request::Home)
    }

    /// Movimiento relativo (mm del sistema de la máquina) con el láser apagado. La velocidad de
    /// la interfaz no se usa: la placa salta a su velocidad de salto.
    pub fn jog(&self, dx: f64, dy: f64, _speed_mm_min: f64) -> Result<(), String> {
        self.ensure_free()?;
        self.call(|reply| Request::Jog { dx, dy, reply })
    }

    /// Movimiento absoluto (mm del sistema de la máquina) con el láser apagado.
    pub fn jog_to(&self, x: f64, y: f64, _speed_mm_min: f64) -> Result<(), String> {
        self.ensure_free()?;
        self.call(|reply| Request::JogTo { x, y, reply })
    }

    /// Espera a que el trabajo llegue a `target`, o a que termine, hasta `wait`.
    fn wait_job_state(&self, target: LhyJobState) -> Result<(), ()> {
        let end = Instant::now() + self.shared.timing.pause_wait;
        loop {
            match self.shared.control.state() {
                s if s == target => return Ok(()),
                LhyJobState::Idle => return Err(()),
                _ => {}
            }
            if Instant::now() >= end {
                return Err(());
            }
            thread::sleep(Duration::from_millis(2));
        }
    }

    pub fn pause(&self) -> Result<(), String> {
        if self.is_closed() {
            return Err(super::NOT_CONNECTED.into());
        }
        if !self.job_running() {
            return Err("No hay un trabajo en marcha que pausar.".into());
        }
        self.shared.control.request_pause();
        match self.wait_job_state(LhyJobState::Paused) {
            Ok(()) => {
                emit_job(
                    self.shared.sink.as_ref(),
                    JobState::Paused,
                    "Trabajo en pausa (PN).",
                    false,
                );
                Ok(())
            }
            Err(()) => {
                self.shared.control.request_resume();
                Err("La placa no confirmó la pausa a tiempo (o el trabajo ya terminó).".into())
            }
        }
    }

    pub fn resume(&self) -> Result<(), String> {
        if self.is_closed() {
            return Err(super::NOT_CONNECTED.into());
        }
        if self.shared.control.state() != LhyJobState::Paused {
            return Err("El trabajo no está en pausa.".into());
        }
        self.shared.control.request_resume();
        match self.wait_job_state(LhyJobState::Running) {
            Ok(()) => {
                emit_job(
                    self.shared.sink.as_ref(),
                    JobState::Started,
                    "Trabajo reanudado.",
                    false,
                );
                Ok(())
            }
            Err(()) => Err(
                "La placa no confirmó la reanudación a tiempo (o el trabajo ya terminó).".into(),
            ),
        }
    }

    /// Detiene el trabajo (o el encuadre) y apaga el láser. Con un trabajo, vuelve en cuanto lo
    /// pide: el resultado (y si la placa lo confirmó) llega como evento del trabajo.
    pub fn cancel(&self) -> Result<(), String> {
        if self.is_closed() {
            return Err(super::NOT_CONNECTED.into());
        }
        self.shared.cancel.store(true, Ordering::SeqCst);
        if self.job_running() {
            self.shared.control.request_stop();
            return Ok(());
        }
        if self.shared.busy.load(Ordering::SeqCst) {
            // Un encuadre o un arranque: la bandera los interrumpe y el hilo detiene la placa.
            return Ok(());
        }
        let r = self.call(Request::Stop);
        self.shared.cancel.store(false, Ordering::SeqCst);
        r
    }
}

/// Conexión abierta: el mando y el hilo.
pub struct LhyConnection {
    handle: LhyHandle,
    thread: Option<JoinHandle<()>>,
}

impl LhyConnection {
    pub fn handle(&self) -> LhyHandle {
        self.handle.clone()
    }

    fn request_close(&self) {
        // Un trabajo en curso se detiene (y la placa también) antes de cerrar.
        self.handle.shared.cancel.store(true, Ordering::SeqCst);
        self.handle.shared.control.request_stop();
        let _ = self.handle.tx.send(Request::Disconnect);
    }

    /// Cierra la conexión (deteniendo antes un trabajo en curso) y espera al hilo.
    pub fn close(mut self) {
        self.request_close();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }

    /// Abre la placa del puerto (`usb:k40:<n>`) o la máquina virtual (`virtual:k40`), la sondea
    /// (solo lectura) y arranca el hilo.
    pub fn connect(
        device: &DeviceProfile,
        port: &str,
        sink: Arc<dyn Sink>,
        timing: LhyTiming,
    ) -> Result<(Self, ConnectResult), String> {
        let mut cfg = session_config(device)?;
        if is_virtual(port) {
            let speed = virtual_speed(port)?;
            let emu = EmuConfig {
                board: cfg.board,
                ..EmuConfig::default()
            };
            let step = Duration::from_secs_f64(emu.poll_dt / speed);
            let (transport, _board) = emulator(emu);
            // El tiempo ya lo marca el paso de cada lectura: no hace falta esperar más.
            cfg.poll_backoff_max = Duration::ZERO;
            Self::start(
                Box::new(Paced {
                    inner: transport,
                    step,
                }),
                cfg,
                port,
                sink,
                timing,
                true,
            )
        } else if let Some(index) = usb_index(port) {
            let transport = UsbTransport::open(index, UsbTimeouts::default())
                .map_err(|e| format!("No se pudo abrir la placa del K40: {e}"))?;
            Self::start(Box::new(transport), cfg, port, sink, timing, false)
        } else {
            Err(format!(
                "«{port}» no es un puerto de K40. Elige una placa USB de la lista o la máquina \
                 virtual."
            ))
        }
    }

    /// Sondea la placa por `transport` y arranca el hilo. `label` es lo que se muestra como puerto.
    pub fn start(
        transport: Box<dyn Transport>,
        cfg: SessionConfig,
        label: &str,
        sink: Arc<dyn Sink>,
        timing: LhyTiming,
        virtual_machine: bool,
    ) -> Result<(Self, ConnectResult), String> {
        let mut session = LhySession::new(transport, cfg.clone());
        log(sink.as_ref(), LogDir::Out, "; sondeo K40 (solo lectura)");
        let info = session.probe().map_err(connect_error)?;
        for line in describe_info(&info) {
            log(sink.as_ref(), LogDir::In, &line);
        }
        if matches!(info.status.status, Status::NoReply) {
            // El USB abre pero la placa no contesta: no hay con qué hablar.
            return Err(
                "La placa del K40 no devuelve ningún estado (código 0). Comprueba que la \
                        máquina esté encendida, el cable USB y que ningún otro programa (K40 \
                        Whisperer, MeerK40t, LaserDRW) tenga la placa abierta."
                    .into(),
            );
        }
        log(sink.as_ref(), LogDir::In, &format!("; {EXPERIMENTAL}"));
        log(
            sink.as_ref(),
            LogDir::In,
            "; La placa no informa su posición: se cuenta desde el home. Haz home antes de mover \
             o enviar (si no, el envío lo hace primero).",
        );
        if virtual_machine {
            log(
                sink.as_ref(),
                LogDir::In,
                "; Máquina virtual (emulador de la M2 Nano): no hay láser ni hardware reales.",
            );
        }
        let firmware = format!(
            "K40 · placa {} · {} · {}",
            cfg.board.name(),
            if info.transport.name.is_empty() {
                "CH341"
            } else {
                &info.transport.name
            },
            info.status.status.describe()
        );

        let shared = Arc::new(Shared {
            dead: AtomicBool::new(false),
            busy: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            pos_known: AtomicBool::new(false),
            status_code: Mutex::new(info.status.code()),
            status: Mutex::new(None),
            control: session.control(),
            sink: sink.clone(),
            virtual_machine,
            board: cfg.board,
            cfg: cfg.clone(),
            timing: timing.clone(),
        });
        let (tx, rx) = mpsc::channel();
        let worker = Worker {
            session,
            rx,
            shared: shared.clone(),
            sink: sink.clone(),
            timing,
            port: label.to_string(),
            home: cfg.home,
            bed: cfg.bed_mm,
            failures: 0,
            last_state: MachineState::Idle,
            last_pos: [0.0, 0.0],
        };
        sink.emit(Emit::Connection(ConnectionEvent {
            connected: true,
            port: Some(label.to_string()),
            error: None,
        }));
        let thread = thread::Builder::new()
            .name("tlaku-lhymicro".into())
            .spawn(move || worker.run())
            .map_err(|e| format!("No se pudo iniciar el hilo de la conexión: {e}"))?;
        Ok((
            Self {
                handle: LhyHandle { tx, shared },
                thread: Some(thread),
            },
            ConnectResult { firmware },
        ))
    }
}

impl Drop for LhyConnection {
    fn drop(&mut self) {
        self.request_close();
    }
}

/// `virtual:k40@5` -> 5 (veces el tiempo real).
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

    fn read(&mut self, len: usize) -> Result<Vec<u8>, TransportError> {
        thread::sleep(self.step);
        self.inner.read(len)
    }

    fn info(&self) -> TransportInfo {
        self.inner.info()
    }
}

fn describe_info(info: &BoardInfo) -> Vec<String> {
    let t = &info.transport;
    let mut out = vec![format!(
        "Placa: {} (USB {:04X}:{:04X}{}); estado: {}",
        if t.name.is_empty() { "CH341" } else { &t.name },
        t.vendor_id,
        t.product_id,
        t.chip_version
            .map(|v| format!(", chip CH341 versión 0x{v:02X}"))
            .unwrap_or_default(),
        info.status.status.describe()
    )];
    out.extend(info.notes.iter().map(|n| format!("; AVISO: {n}")));
    out
}

/// Error del sondeo al conectar, con lo que conviene revisar.
fn connect_error(e: SessionError) -> String {
    match &e {
        SessionError::NotM2Nano { .. } => format!(
            "{e}. Una Moshiboard no es compatible con este soporte: Tlaku solo habla con la M2 \
             Nano y sus hermanas (A, B, M...)."
        ),
        SessionError::Transport(_) | SessionError::Timeout { .. } => format!(
            "No se pudo hablar con la placa del K40: {e}. Comprueba el cable USB, que la máquina \
             esté encendida y que ningún otro programa (K40 Whisperer, MeerK40t, LaserDRW) tenga \
             la placa abierta."
        ),
        _ => format!("No se pudo hablar con la placa del K40: {e}"),
    }
}

/// Error de una operación en marcha, con la pista del USB si es un silencio.
fn op_error(e: SessionError) -> String {
    match &e {
        SessionError::UnknownPosition => UNKNOWN_POSITION.to_string(),
        SessionError::Transport(_) | SessionError::Timeout { .. } => format!(
            "{e}. Comprueba el cable USB y que ningún otro programa tenga la placa abierta."
        ),
        SessionError::Busy(_) => format!("{e}. Espera a que termine o pulsa Detener."),
        _ => e.to_string(),
    }
}

fn log(sink: &dyn Sink, dir: LogDir, text: &str) {
    sink.emit(Emit::Log(LogEvent {
        dir,
        text: text.to_string(),
    }));
}

fn emit_job(sink: &dyn Sink, state: JobState, message: &str, position_lost: bool) {
    sink.emit(Emit::Job(JobEvent {
        state,
        message: message.to_string(),
        position_lost,
    }));
}

// ---------------------------------------------------------------------------
// Hilo
// ---------------------------------------------------------------------------

struct Worker {
    session: LhySession<Box<dyn Transport>>,
    rx: Receiver<Request>,
    shared: Arc<Shared>,
    sink: Arc<dyn Sink>,
    timing: LhyTiming,
    port: String,
    home: HomeCorner,
    bed: [f64; 2],
    /// Lecturas de estado seguidas que fallaron.
    failures: u32,
    last_state: MachineState,
    last_pos: [f64; 2],
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
        self.shared.dead.store(true, Ordering::SeqCst);
        self.sink.emit(Emit::Connection(ConnectionEvent {
            connected: false,
            port: Some(self.port.clone()),
            error: lost,
        }));
    }

    fn log(&self, dir: LogDir, text: &str) {
        log(self.sink.as_ref(), dir, text);
    }

    // ---- estado ------------------------------------------------------------

    /// Posición del cabezal en mm de la máquina (distancia desde el home por eje).
    fn machine_pos(&self) -> Option<[f64; 2]> {
        self.session
            .position_mil()
            .map(|(x, y)| [mil_to_mm(x), mil_to_mm(y)])
    }

    /// Publica el estado con la posición actual y deja la marca de "posición conocida".
    fn publish(&mut self, state: MachineState) {
        let known = match self.machine_pos() {
            Some(p) => {
                self.last_pos = p;
                true
            }
            None => false,
        };
        self.shared.pos_known.store(known, Ordering::SeqCst);
        self.last_state = state;
        let report = StatusReport {
            state,
            other_state: None,
            machine_pos: self.last_pos,
            work_pos: self.last_pos,
            feed: 0.0,
            spindle: 0.0,
            // La placa no informa búferes: ceros (la interfaz los oculta).
            buffer: BufferStatus { planner: 0, rx: 0 },
            overrides: None,
        };
        if let Ok(mut g) = self.shared.status.lock() {
            *g = Some(report.clone());
        }
        self.sink.emit(Emit::Status(report));
    }

    /// Un sondeo de estado en reposo. `Err` = conexión perdida (el hilo termina).
    fn poll(&mut self) -> Result<(), String> {
        match self.session.status() {
            Ok(r) => {
                self.failures = 0;
                if let Ok(mut g) = self.shared.status_code.lock() {
                    *g = r.code();
                }
                let state = match r.status {
                    Status::Busy => MachineState::Run,
                    Status::LowPower => MachineState::Door,
                    _ => MachineState::Idle,
                };
                if state == MachineState::Door && self.last_state != MachineState::Door {
                    self.log(
                        LogDir::In,
                        "; AVISO: la placa informa energía insuficiente (239): paro de \
                         emergencia pulsado, tapa abierta o fuente del láser apagada",
                    );
                }
                self.publish(state);
                Ok(())
            }
            Err(SessionError::NotM2Nano { code }) => Err(format!(
                "la placa respondió con el código {code}, que no existe en la M2 Nano (¿una \
                 Moshiboard?)"
            )),
            Err(SessionError::Transport(
                e @ (TransportError::Disconnected | TransportError::Io(_)),
            )) => Err(format!(
                "se perdió la conexión con la placa del K40: {e}. Revisa el cable USB y que la \
                 máquina siga encendida."
            )),
            Err(e) => {
                self.failures += 1;
                if self.failures >= self.timing.lost_after {
                    Err(format!(
                        "se perdió la conexión con la placa del K40 ({e}). Revisa el cable USB y \
                         que la máquina siga encendida."
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
            Request::Start { toolpath, reply } => self.start_job(*toolpath, reply),
            Request::Frame { corners, reply } => self.frame(corners, reply),
            Request::Home(reply) => {
                self.shared.cancel.store(false, Ordering::SeqCst);
                let r = self.home_now();
                self.finish(reply, r);
            }
            Request::Jog { dx, dy, reply } => {
                let r = self.jog(dx, dy);
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
    /// pedir lo siguiente de inmediato y no debe toparse con un "ocupado" que ya terminó.
    fn finish<T>(&self, reply: Reply<T>, result: Result<T, String>) {
        self.shared.busy.store(false, Ordering::SeqCst);
        let _ = reply.send(result);
    }

    fn cancelled(&self) -> bool {
        self.shared.cancel.load(Ordering::SeqCst)
    }

    fn home_now(&mut self) -> Result<(), String> {
        self.log(
            LogDir::Out,
            "; home (IPP): el cabezal va a los finales de carrera",
        );
        self.publish(MachineState::Home);
        let r = self.session.home();
        self.publish(MachineState::Idle);
        match r {
            Ok(()) => {
                self.log(LogDir::In, "; home hecho: el cabezal está en (0, 0)");
                Ok(())
            }
            Err(e) => Err(op_error(e)),
        }
    }

    fn jog(&mut self, dx: f64, dy: f64) -> Result<(), String> {
        if !(dx.is_finite() && dy.is_finite()) {
            return Err("El desplazamiento no es válido.".into());
        }
        let Some([x, y]) = self.machine_pos() else {
            return Err(UNKNOWN_POSITION.into());
        };
        self.jog_to(x + dx, y + dy)
    }

    /// Salto absoluto a un punto en mm del sistema de la máquina.
    fn jog_to(&mut self, x: f64, y: f64) -> Result<(), String> {
        if !(x.is_finite() && y.is_finite()) {
            return Err("La posición no es válida.".into());
        }
        let target = self.home.from_machine_mm((x, y), self.bed);
        self.publish(MachineState::Jog);
        let r = self.session.jog_to(target);
        self.publish(MachineState::Idle);
        if r.is_err() && self.machine_pos().is_none() {
            self.log(
                LogDir::In,
                "; AVISO: el salto falló y la posición del cabezal quedó desconocida: haz home",
            );
        }
        r.map_err(op_error)
    }

    fn frame(&mut self, corners: Vec<(f64, f64)>, reply: Reply<()>) {
        self.shared.cancel.store(false, Ordering::SeqCst);
        if self.machine_pos().is_none() {
            self.finish(reply, Err(UNKNOWN_POSITION.into()));
            return;
        }
        if corners.is_empty() {
            self.finish(
                reply,
                Err("No hay nada que encuadrar: el trabajo está vacío.".into()),
            );
            return;
        }
        // Se valida todo el recorrido antes de mover nada.
        let eps = 0.05;
        if let Some(&(x, y)) = corners.iter().find(|&&(x, y)| {
            !(x.is_finite() && y.is_finite())
                || x < -eps
                || y < -eps
                || x > self.bed[0] + eps
                || y > self.bed[1] + eps
        }) {
            self.finish(
                reply,
                Err(format!(
                    "El encuadre pasa por ({x:.2}, {y:.2}) mm, que queda fuera de la cama."
                )),
            );
            return;
        }
        self.log(
            LogDir::Out,
            "; encuadre: recorrido del rectángulo con el láser apagado",
        );
        let _ = reply.send(Ok(()));
        self.publish(MachineState::Run);
        let mut aborted = false;
        for &(x, y) in &corners {
            if self.cancelled() {
                aborted = true;
                break;
            }
            let target = self.home.from_machine_mm((x, y), self.bed);
            if let Err(e) = self.session.jog_to(target) {
                self.log(
                    LogDir::In,
                    &format!("; encuadre interrumpido: {}", op_error(e)),
                );
                aborted = true;
                break;
            }
        }
        if self.cancelled() {
            self.log(LogDir::In, "; encuadre detenido por el usuario");
            let _ = self.stop_idle();
        } else if aborted {
            self.publish(MachineState::Idle);
        } else {
            self.publish(MachineState::Idle);
            self.log(LogDir::In, "; encuadre terminado");
        }
        self.shared.cancel.store(false, Ordering::SeqCst);
    }

    /// Detiene la placa sin trabajo en marcha (`I` e `IUS1P`). La posición queda desconocida.
    fn stop_idle(&mut self) -> Result<(), String> {
        let outcome = self.session.stop();
        self.publish(MachineState::Idle);
        match outcome {
            StopOutcome::Acked => {
                self.log(
                    LogDir::Out,
                    "; detener (I, IUS1P): la placa lo confirmó y el láser quedó apagado. La \
                     posición del cabezal se perdió: haz home antes de seguir",
                );
                Ok(())
            }
            StopOutcome::Unconfirmed => {
                self.log(
                    LogDir::In,
                    "; AVISO: la placa no confirmó detenerse ni apagar el láser: usa el paro de \
                     emergencia o abre la tapa",
                );
                Err(format!(
                    "{UNCONFIRMED_MARK} de que la placa se detuviera ni de que el láser se \
                     apagara. Usa el paro de emergencia o abre la tapa."
                ))
            }
        }
    }

    fn start_job(&mut self, toolpath: Toolpath, reply: Reply<()>) {
        self.shared.cancel.store(false, Ordering::SeqCst);
        // La placa no informa dónde está: sin posición conocida se hace home antes.
        if self.machine_pos().is_none() {
            self.log(
                LogDir::Out,
                "; la posición del cabezal se desconoce: home antes de empezar",
            );
            if let Err(e) = self.home_now() {
                self.finish(reply, Err(e));
                return;
            }
        }
        if self.cancelled() {
            emit_job(
                self.sink.as_ref(),
                JobState::Cancelled,
                "Envío cancelado antes de empezar.",
                false,
            );
            self.shared.cancel.store(false, Ordering::SeqCst);
            self.finish(reply, Ok(()));
            return;
        }
        let program = match self.session.encode(&toolpath) {
            Ok(p) => p,
            Err(e) => {
                self.finish(reply, Err(op_error(e)));
                return;
            }
        };
        self.log_program(&program);

        self.publish(MachineState::Run);
        let mut reply = Some(reply);
        let sink = self.sink.clone();
        let every = self.timing.progress_interval;
        let mut last_emit: Option<Instant> = None;
        let result = self.session.send_job(&program, &mut |p| {
            if let Some(r) = reply.take() {
                emit_job(
                    sink.as_ref(),
                    JobState::Started,
                    &format!("Enviando el trabajo ({} paquetes).", p.packets),
                    false,
                );
                let _ = r.send(Ok(()));
            }
            let due = last_emit.is_none_or(|t| t.elapsed() >= every) || p.packet == p.packets;
            if due {
                last_emit = Some(Instant::now());
                sink.emit(Emit::Progress(ProgressEvent {
                    acked: u32::try_from(p.packet).unwrap_or(u32::MAX),
                    total: u32::try_from(p.packets).unwrap_or(u32::MAX),
                }));
            }
        });
        self.shared.cancel.store(false, Ordering::SeqCst);
        self.publish(MachineState::Idle);
        match result {
            Ok(report) => {
                self.log(
                    LogDir::In,
                    &format!(
                        "; enviado: {} paquetes, {} bytes, {} reenvíos por CRC, en {:.1} s",
                        report.packets,
                        report.bytes,
                        report.resends,
                        report.elapsed.as_secs_f64()
                    ),
                );
                emit_job(
                    self.sink.as_ref(),
                    JobState::Finished,
                    &if report.finish_confirmed {
                        "Trabajo terminado: la placa confirmó el final.".to_string()
                    } else {
                        "Trabajo terminado, pero la placa no confirmó el final (estado 236)."
                            .to_string()
                    },
                    false,
                );
            }
            Err(e) => {
                let msg = e.to_string();
                self.log(LogDir::In, &format!("; {msg}"));
                if e.laser_may_be_on() {
                    self.log(
                        LogDir::In,
                        "; AVISO: el láser pudo quedar encendido o la placa en marcha: usa el \
                         paro de emergencia o abre la tapa",
                    );
                }
                if let Some(r) = reply.take() {
                    // Falló antes del primer paquete confirmado: es un rechazo del envío.
                    self.finish(r, Err(msg));
                } else if matches!(e.cause, SessionError::Cancelled) && !e.laser_may_be_on() {
                    emit_job(
                        self.sink.as_ref(),
                        JobState::Cancelled,
                        &format!(
                            "Trabajo detenido: la placa se detuvo y el láser se apagó. La \
                             posición del cabezal se perdió: haz home antes de seguir. \
                             (Paquetes confirmados: {} de {}.)",
                            e.packets_sent, e.packets_total
                        ),
                        true,
                    );
                } else {
                    emit_job(self.sink.as_ref(), JobState::Error, &msg, true);
                }
            }
        }
    }

    fn log_program(&self, program: &Program) {
        self.log(
            LogDir::Out,
            &format!(
                "; trabajo LHYMICRO-GL (placa {}): {} bytes, {:.1} mm grabados, {:.1} mm de \
                 viajes, tiempo estimado {:.0} s",
                program.board,
                program.stream.len(),
                program.stats.burn_mm,
                program.stats.travel_mm,
                program.stats.est_seconds
            ),
        );
        for w in &program.warnings {
            self.log(LogDir::In, &format!("; AVISO: {w}"));
        }
    }
}

#[cfg(test)]
mod tests;
