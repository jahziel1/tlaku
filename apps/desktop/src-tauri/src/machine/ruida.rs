//! Conexión con una controladora Ruida (CO2) por UDP. EXPERIMENTAL: no se ha probado con una
//! máquina real (ver `docs/hardware/ruida.md`); sí contra el emulador de `tlaku-ruida`.
//!
//! Como la de GRBL, es un hilo dedicado dueño del enlace ([`RuidaSession`]) que habla con la
//! interfaz solo por eventos `machine://*` (el mismo [`Sink`]):
//!
//! - en reposo, lee el estado y la posición cada ~500 ms (`machine://status`). **Nunca durante una
//!   subida**: el hilo está dentro de `send_job`, así que no hay lecturas intercaladas;
//! - [`RuidaHandle::upload`] sube el `.rd` ya generado con progreso (`machine://progress`). Con
//!   [`SendMode::Store`] queda como documento en la memoria y no arranca nada; con
//!   [`SendMode::Run`] se ejecuta al llegar y el hilo sigue el trabajo por el estado hasta que la
//!   máquina vuelve a reposo (`machine://job` finished). Si la subida falla, el protocolo seguro
//!   es el de `RuidaSession::send_job` (Detener `D8 01` y aviso si no se confirmó);
//! - pausa, reanudar y detener (`D8 02/03/01`), saltos con el láser apagado y encuadre (cuatro
//!   esquinas, sin bloquear el hilo: las órdenes de detener siguen atendiéndose);
//! - información y lista de documentos de la controladora, solo lectura.
//!
//! Hay cosas del protocolo que no se conocen y se resolvieron con cautela:
//!
//! - Según un foro, algunas controladoras ignoran la red mientras ejecutan un trabajo
//!   ([FOR3] de ruida.md). Con un trabajo en marcha, un sondeo sin respuesta no cuenta como
//!   conexión perdida; fuera de uno, tres sondeos seguidos sin respuesta sí.
//! - El bit `0x2` del estado es "pausa" o "fin de parte" según la fuente.
//!
//! Máquina virtual: el puerto `virtual:ruida[@<velocidad>]` arranca un `UdpEmulator` en
//! `127.0.0.1` y se conecta a él por UDP real, así que se prueba el mismo camino que con una
//! máquina; el trabajo "se ejecuta" durante el tiempo estimado (dividido por la velocidad).
//!
//! Rotativo (M28): la fuente disponible (`docs/hardware/dsp-co2.md`) dice que en las DSP el
//! rotativo se activa en los ajustes de la controladora. El `.rd` no lleva nada del rotativo, así
//! que no se escala la Y: el trabajo viaja en mm de superficie y la controladora hace la
//! conversión. Tlaku solo lo avisa en el registro.

use super::{Emit, Sink};
use std::collections::VecDeque;
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tlaku_doc::{
    BufferStatus, ConnectResult, ConnectionEvent, DeviceProfile, JobEvent, JobState, LogDir,
    LogEvent, MachineState, ProgressEvent, RuidaFile, RuidaInfo, StatusReport,
};
use tlaku_ruida::emu::{EmuConfig, UdpEmulator};
use tlaku_ruida::link::{PacketLink, UdpLink};
use tlaku_ruida::packet::PacketConfig;
use tlaku_ruida::rd::HomeCorner;
use tlaku_ruida::safety::SafetyLimits;
use tlaku_ruida::session::{
    ControllerInfo, MachineStatus, ProbeInfo, RuidaSession, SendMode, SessionConfig, SessionError,
};
use tlaku_ruida::swizzle::{MAGIC_DEFAULT, Magic};

/// Puerto de la máquina virtual Ruida (`virtual:ruida` o `virtual:ruida@5`).
pub const VIRTUAL_PORT: &str = "virtual:ruida";

/// Aviso que acompaña a todo lo que toca una máquina real.
pub const EXPERIMENTAL: &str = "EXPERIMENTAL: el soporte Ruida no se ha probado con una máquina \
real (docs/hardware/ruida.md). Prueba primero con el láser deshabilitado o a potencia 0.";

/// Nota del rotativo (M28): la controladora lo gestiona por su cuenta.
pub const ROTARY_NOTE: &str = "Rotativo: Ruida lo activa en los ajustes de la propia \
controladora; Tlaku envía la Y en mm de superficie, sin escalarla ni reflejarla.";

/// Mensaje cuando una operación solo existe en GRBL.
pub const GRBL_ONLY: &str = "Esta función es solo de GRBL: la controladora Ruida no usa G-code \
ni overrides en vivo.";

/// ¿Es el puerto de la máquina virtual Ruida (con o sin `@velocidad`)?
pub fn is_virtual(port: &str) -> bool {
    port.split('@').next() == Some(VIRTUAL_PORT)
}

// ---------------------------------------------------------------------------
// Tiempos
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RuidaTiming {
    /// Cada cuánto se lee el estado en reposo.
    pub status_interval: Duration,
    /// Lo mismo mientras se recorre un encuadre.
    pub frame_interval: Duration,
    /// Espera del ACK y de la respuesta de una lectura al sondear el estado. Corta: si no
    /// contesta, el siguiente sondeo ya la intenta de nuevo.
    pub poll_timeout: Duration,
    /// Espera del ACK de cada paquete al subir un trabajo.
    pub upload_ack: Duration,
    /// Sondeos seguidos sin respuesta (fuera de un trabajo) tras los cuales se da por perdida
    /// la conexión.
    pub lost_after: u32,
    /// Tras subir un trabajo en modo ejecutar, cuánto se espera a ver la máquina en marcha antes
    /// de darlo por terminado si ya está en reposo (un trabajo muy corto).
    pub start_grace: Duration,
}

impl Default for RuidaTiming {
    fn default() -> Self {
        Self {
            status_interval: Duration::from_millis(500),
            frame_interval: Duration::from_millis(200),
            poll_timeout: Duration::from_millis(400),
            upload_ack: Duration::from_secs(1),
            lost_after: 3,
            start_grace: Duration::from_secs(5),
        }
    }
}

// ---------------------------------------------------------------------------
// Mando
// ---------------------------------------------------------------------------

/// Lo que hace falta para subir un trabajo.
pub struct Upload {
    /// Flujo `.rd` en claro (`tlaku_pipeline::export_rd_for_device`).
    pub rd: Vec<u8>,
    /// Ejecutar al llegar o guardar como documento.
    pub mode: SendMode,
    /// Cama y tope de velocidad del perfil (los mismos con que se generó el archivo).
    pub limits: SafetyLimits,
    /// Esquina del home con que se generó el archivo.
    pub home: HomeCorner,
    /// Duración estimada del trabajo; solo la usa la máquina virtual para simularlo.
    pub estimate_s: f64,
    /// Avisos para el registro antes de enviar (p. ej. el rotativo).
    pub notes: Vec<String>,
}

type Reply<T> = Sender<Result<T, String>>;

enum Request {
    Upload(Box<Upload>, Reply<()>),
    /// Recorre el rectángulo con el láser apagado. Esquinas en mm del sistema de la máquina.
    Frame {
        corners: Vec<(f64, f64)>,
        speed_mm_min: f64,
        reply: Reply<()>,
    },
    Jog {
        dx: f64,
        dy: f64,
        speed_mm_min: f64,
        reply: Reply<()>,
    },
    JogTo {
        x: f64,
        y: f64,
        speed_mm_min: f64,
        reply: Reply<()>,
    },
    Pause(Reply<()>),
    Resume(Reply<()>),
    Stop(Reply<()>),
    Info(Reply<Box<RuidaInfo>>),
    Files(Reply<Vec<RuidaFile>>),
    Disconnect,
}

#[derive(Default)]
struct Shared {
    /// El hilo terminó (desconexión pedida o perdida).
    dead: AtomicBool,
    /// Se pidió cancelar: la subida en curso se interrumpe entre paquetes.
    cancel: AtomicBool,
    status: Mutex<Option<StatusReport>>,
    virtual_machine: AtomicBool,
}

/// Mando de la conexión; se clona y se usa desde cualquier hilo. Las llamadas bloquean hasta que
/// el hilo responde (en Tauri, dentro de `spawn_blocking`).
#[derive(Clone)]
pub struct RuidaHandle {
    tx: Sender<Request>,
    shared: Arc<Shared>,
}

impl RuidaHandle {
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
        self.shared.virtual_machine.load(Ordering::SeqCst)
    }

    /// Posición del cabezal en mm del sistema de la máquina, del último sondeo.
    pub fn work_pos(&self) -> Option<[f64; 2]> {
        self.shared.status.lock().ok()?.as_ref().map(|s| s.work_pos)
    }

    /// Último reporte de estado completo (para la automatización: `STATUS`).
    pub fn status_report(&self) -> Option<StatusReport> {
        self.shared.status.lock().ok()?.clone()
    }

    /// Calcula el trabajo, genera el `.rd` y lo sube: ejecuta o guarda según el perfil
    /// (`ruida.run_after_upload`).
    pub fn start_job(
        &self,
        doc: &tlaku_doc::Document,
        device: &DeviceProfile,
        options: &tlaku_doc::JobOptions,
    ) -> Result<(), String> {
        let up = build_upload(doc, device, options, self.work_pos())?;
        self.upload(up)
    }

    /// Encuadre del trabajo (rectángulo de todos sus movimientos) con el láser apagado, a
    /// `speed` mm/min.
    pub fn frame_job(
        &self,
        doc: &tlaku_doc::Document,
        device: &DeviceProfile,
        options: &tlaku_doc::JobOptions,
        speed: f64,
    ) -> Result<(), String> {
        if !(speed.is_finite() && speed > 0.0) {
            return Err("La velocidad del encuadre debe ser mayor que 0.".into());
        }
        let corners = frame_corners(doc, device, options, self.work_pos())?;
        self.frame(corners, speed)
    }

    /// Sube el trabajo. Vuelve cuando terminó de subirse (con `Run` el trabajo sigue en la
    /// máquina; su final llega como evento).
    pub fn upload(&self, up: Upload) -> Result<(), String> {
        self.call(|reply| Request::Upload(Box::new(up), reply))
    }

    /// Recorre el rectángulo de `corners` (mm del sistema de la máquina) sin disparar el láser.
    /// Vuelve en cuanto empieza; el hilo sigue el recorrido.
    pub fn frame(&self, corners: Vec<(f64, f64)>, speed_mm_min: f64) -> Result<(), String> {
        self.call(|reply| Request::Frame {
            corners,
            speed_mm_min,
            reply,
        })
    }

    /// Movimiento relativo (mm del sistema de la máquina) con el láser apagado.
    pub fn jog(&self, dx: f64, dy: f64, speed_mm_min: f64) -> Result<(), String> {
        self.call(|reply| Request::Jog {
            dx,
            dy,
            speed_mm_min,
            reply,
        })
    }

    /// Movimiento absoluto (mm del sistema de la máquina) con el láser apagado.
    pub fn jog_to(&self, x: f64, y: f64, speed_mm_min: f64) -> Result<(), String> {
        self.call(|reply| Request::JogTo {
            x,
            y,
            speed_mm_min,
            reply,
        })
    }

    pub fn pause(&self) -> Result<(), String> {
        self.call(Request::Pause)
    }

    pub fn resume(&self) -> Result<(), String> {
        self.call(Request::Resume)
    }

    /// Detiene el trabajo (`D8 01`); durante una subida, la interrumpe antes.
    pub fn cancel(&self) -> Result<(), String> {
        self.shared.cancel.store(true, Ordering::SeqCst);
        let r = self.call(Request::Stop);
        // Si la conexión ya no existe, la bandera no importa; si la hay, el hilo la apaga.
        if r.is_err() {
            self.shared.cancel.store(false, Ordering::SeqCst);
        }
        r
    }

    /// Información de la controladora (M25 y M26), solo lectura.
    pub fn info(&self) -> Result<RuidaInfo, String> {
        self.call(Request::Info).map(|b| *b)
    }

    /// Documentos guardados en la memoria (M24), solo lectura y experimental.
    pub fn files(&self) -> Result<Vec<RuidaFile>, String> {
        self.call(Request::Files)
    }
}

/// Conexión abierta: el mando y el hilo.
pub struct RuidaConnection {
    handle: RuidaHandle,
    thread: Option<JoinHandle<()>>,
}

impl RuidaConnection {
    pub fn handle(&self) -> RuidaHandle {
        self.handle.clone()
    }

    /// Cierra la conexión (deteniendo antes un trabajo en curso) y espera al hilo.
    pub fn close(mut self) {
        let _ = self.handle.tx.send(Request::Disconnect);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }

    /// Abre la conexión del perfil: la IP de `device.connection` por UDP o, con el puerto
    /// `virtual:ruida`, un emulador local. Sondea (solo lectura) y arranca el hilo.
    pub fn connect(
        device: &DeviceProfile,
        port: &str,
        sink: Arc<dyn Sink>,
        timing: RuidaTiming,
    ) -> Result<(Self, ConnectResult), String> {
        let cfg = session_config(device, &timing);
        if is_virtual(port) {
            let speed = virtual_speed(port)?;
            let server = UdpEmulator::spawn(EmuConfig {
                packets: PacketConfig {
                    magic: cfg.magic,
                    ..PacketConfig::default()
                },
                bed_um: [
                    (device.bed[0] * 1000.0).round() as i64,
                    (device.bed[1] * 1000.0).round() as i64,
                ],
                ..EmuConfig::default()
            })
            .map_err(|e| format!("No se pudo iniciar la máquina virtual Ruida: {e}"))?;
            let addr = server.addr();
            let link = UdpLink::to_controller_ports(addr.ip(), addr.port(), 0).map_err(|e| {
                format!("No se pudo abrir el enlace local de la máquina virtual: {e}")
            })?;
            let sim = VirtualSim::new(server, speed);
            Self::start(link, cfg, port, sink, timing, Some(sim), device)
        } else {
            let ip = host_of(device)?;
            let link = UdpLink::to_controller(ip).map_err(|e| {
                format!(
                    "No se pudo abrir el puerto UDP local 40200: {e}. Cierra cualquier otro \
                     programa de láser que lo use."
                )
            })?;
            Self::start(
                link,
                cfg,
                &format!("udp://{ip}"),
                sink,
                timing,
                None,
                device,
            )
        }
    }

    /// Sondea la máquina por `link` y arranca el hilo. `label` es lo que se muestra como puerto.
    pub fn start<L: PacketLink + 'static>(
        link: L,
        cfg: SessionConfig,
        label: &str,
        sink: Arc<dyn Sink>,
        timing: RuidaTiming,
        sim: Option<VirtualSim>,
        device: &DeviceProfile,
    ) -> Result<(Self, ConnectResult), String> {
        let auto_magic = device.ruida.magic.is_none();
        let mut session = RuidaSession::new(link, cfg);
        let probe = session.probe().map_err(connect_error)?;
        // Ya se conoce el magic: el resto de las lecturas no lo vuelven a buscar.
        {
            let c = session.config_mut();
            c.probe_all_magics = false;
            c.ack_timeout = timing.poll_timeout;
            c.reply_timeout = timing.poll_timeout;
            c.query_retries = 1;
        }
        log(sink.as_ref(), LogDir::Out, "; sondeo Ruida (solo lectura)");
        log(sink.as_ref(), LogDir::In, &describe_probe(&probe));
        if probe.magic_changed || auto_magic {
            log(
                sink.as_ref(),
                LogDir::In,
                &format!(
                    "; magic 0x{:02X} detectado: puedes fijarlo en el perfil de la máquina",
                    probe.magic.0
                ),
            );
        }
        if let Some([w, h]) = probe.bed_mm
            && ((w - device.bed[0]).abs() > 1.0 || (h - device.bed[1]).abs() > 1.0)
        {
            log(
                sink.as_ref(),
                LogDir::In,
                &format!(
                    "; AVISO: la cama que informa la máquina ({w:.0} × {h:.0} mm) no coincide \
                     con la del perfil ({:.0} × {:.0} mm); no se enviarán trabajos hasta que la \
                     corrijas, porque el origen del archivo depende de ella",
                    device.bed[0], device.bed[1]
                ),
            );
        }
        if device.rotary.enabled {
            log(sink.as_ref(), LogDir::In, &format!("; {ROTARY_NOTE}"));
        }
        log(sink.as_ref(), LogDir::In, &format!("; {EXPERIMENTAL}"));
        if sim.is_some() {
            log(
                sink.as_ref(),
                LogDir::In,
                "; Máquina virtual (emulador Ruida): no hay láser ni hardware reales.",
            );
        }
        let firmware = format!(
            "Ruida {} · tarjeta 0x{:08X}",
            probe.model.unwrap_or("modelo desconocido"),
            probe.card_id
        );

        let shared = Arc::new(Shared::default());
        shared
            .virtual_machine
            .store(sim.is_some(), Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        let worker = Worker {
            session,
            rx,
            sink: sink.clone(),
            shared: shared.clone(),
            timing,
            port: label.to_string(),
            bed_mm: probe.bed_mm,
            job: None,
            frame: None,
            failures: 0,
            silent_logged: false,
            sim,
        };
        sink.emit(Emit::Connection(ConnectionEvent {
            connected: true,
            port: Some(label.to_string()),
            error: None,
        }));
        let thread = thread::Builder::new()
            .name("tlaku-ruida".into())
            .spawn(move || worker.run())
            .map_err(|e| format!("No se pudo iniciar el hilo de la conexión: {e}"))?;
        Ok((
            Self {
                handle: RuidaHandle { tx, shared },
                thread: Some(thread),
            },
            ConnectResult { firmware },
        ))
    }
}

impl Drop for RuidaConnection {
    fn drop(&mut self) {
        let _ = self.handle.tx.send(Request::Disconnect);
    }
}

/// Configuración de la sesión a partir del perfil: magic (o búsqueda automática), cama y home.
pub fn session_config(device: &DeviceProfile, timing: &RuidaTiming) -> SessionConfig {
    SessionConfig {
        magic: Magic(device.ruida.magic.unwrap_or(MAGIC_DEFAULT)),
        // Con el magic fijado en el perfil no se prueban otros: el usuario sabe cuál es.
        probe_all_magics: device.ruida.magic.is_none(),
        ack_timeout: timing.upload_ack,
        reply_timeout: timing.upload_ack,
        limits: SafetyLimits {
            bed_mm: device.bed,
            ..SafetyLimits::default()
        },
        home: tlaku_pipeline::home_for_origin(device.origin),
        ..SessionConfig::default()
    }
}

/// IP de la controladora del perfil (`connection = udp`). Acepta una IP o un nombre.
fn host_of(device: &DeviceProfile) -> Result<IpAddr, String> {
    let host = match &device.connection {
        tlaku_doc::Connection::Udp { host } if !host.trim().is_empty() => host.trim(),
        _ => {
            return Err(
                "El perfil Ruida necesita la IP de la controladora (conexión UDP). Escríbela \
                 en el perfil de la máquina."
                    .into(),
            );
        }
    };
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(ip);
    }
    (host, 0)
        .to_socket_addrs()
        .ok()
        .and_then(|mut it| it.find(|a| a.is_ipv4()))
        .map(|a| a.ip())
        .ok_or_else(|| format!("«{host}» no es una IP ni un nombre que se pueda resolver."))
}

/// `virtual:ruida@5` -> 5 (veces el tiempo real).
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

fn describe_probe(info: &ProbeInfo) -> String {
    format!(
        "Ruida {} (tarjeta 0x{:08X}), magic 0x{:02X}{}, cama {}",
        info.model.unwrap_or("modelo desconocido"),
        info.card_id,
        info.magic.0,
        if info.magic_changed {
            " (detectado)"
        } else {
            ""
        },
        match info.bed_mm {
            Some([w, h]) => format!("{w:.1} × {h:.1} mm"),
            None => "no informada".into(),
        }
    )
}

/// Error del sondeo al conectar, con lo que conviene revisar.
fn connect_error(e: SessionError) -> String {
    match &e {
        SessionError::Timeout { .. } => format!(
            "La controladora Ruida no contesta ({e}). Revisa la IP, que el equipo esté en la \
             misma red /24 que la máquina, que ningún otro programa de láser tenga abierto el \
             puerto UDP 40200 y el magic del perfil."
        ),
        _ => format!("No se pudo hablar con la controladora Ruida: {e}"),
    }
}

/// Error de una operación en marcha, con la pista de la red si es un silencio.
fn op_error(e: SessionError) -> String {
    match &e {
        SessionError::Timeout { .. } => format!(
            "{e}. Algunas controladoras ignoran la red mientras ejecutan un trabajo: usa el panel \
             de la máquina o el paro de emergencia."
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

// ---------------------------------------------------------------------------
// Máquina virtual
// ---------------------------------------------------------------------------

/// Emulador local con el que "se ejecuta" un trabajo durante su tiempo estimado.
pub struct VirtualSim {
    /// Mantiene vivo el emulador UDP (se detiene al soltarlo).
    server: UdpEmulator,
    speed: f64,
    /// Segundos simulados que le quedan al trabajo en marcha.
    remaining: Option<f64>,
    last_tick: Instant,
}

impl VirtualSim {
    pub fn new(server: UdpEmulator, speed: f64) -> Self {
        Self {
            server,
            speed,
            remaining: None,
            last_tick: Instant::now(),
        }
    }

    fn start(&mut self, estimate_s: f64) {
        let s = if estimate_s.is_finite() {
            estimate_s.max(1.0)
        } else {
            1.0
        };
        self.remaining = Some(s);
        self.last_tick = Instant::now();
    }

    /// Hace avanzar el reloj del trabajo; al agotarse, la máquina lo da por terminado.
    fn tick(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f64() * self.speed;
        self.last_tick = now;
        let Some(rem) = self.remaining.as_mut() else {
            return;
        };
        let mut emu = self.server.emulator();
        if !emu.running {
            // Se detuvo desde fuera (Detener).
            self.remaining = None;
        } else if !emu.paused {
            *rem -= dt;
            if *rem <= 0.0 {
                emu.finish_job();
                self.remaining = None;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Hilo
// ---------------------------------------------------------------------------

/// Un trabajo ejecutándose en la máquina (modo `Run`).
struct ActiveJob {
    since: Instant,
    /// Ya se vio la máquina en marcha.
    seen_running: bool,
    paused: bool,
}

/// Recorrido de un encuadre: esquinas pendientes (mm del sistema de la máquina).
struct FrameRun {
    corners: VecDeque<(f64, f64)>,
}

struct Worker<L: PacketLink> {
    session: RuidaSession<L>,
    rx: Receiver<Request>,
    sink: Arc<dyn Sink>,
    shared: Arc<Shared>,
    timing: RuidaTiming,
    port: String,
    /// Cama que informó la máquina al conectar.
    bed_mm: Option<[f64; 2]>,
    job: Option<ActiveJob>,
    frame: Option<FrameRun>,
    /// Sondeos seguidos sin respuesta.
    failures: u32,
    silent_logged: bool,
    sim: Option<VirtualSim>,
}

impl<L: PacketLink> Worker<L> {
    fn run(mut self) {
        let mut next_poll = Instant::now() + self.timing.status_interval;
        let lost = loop {
            let wait = next_poll.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(wait) {
                Ok(Request::Disconnect) | Err(RecvTimeoutError::Disconnected) => {
                    self.shutdown();
                    break None;
                }
                Ok(req) => self.handle(req),
                Err(RecvTimeoutError::Timeout) => {
                    if let Err(msg) = self.poll() {
                        break Some(msg);
                    }
                    let every = if self.frame.is_some() {
                        self.timing.frame_interval
                    } else {
                        self.timing.status_interval
                    };
                    next_poll = Instant::now() + every;
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

    fn job_event(&self, state: JobState, message: &str) {
        self.sink.emit(Emit::Job(JobEvent {
            state,
            message: message.to_string(),
            position_lost: false,
        }));
    }

    /// Al desconectar, un trabajo en marcha se detiene (igual que con GRBL).
    fn shutdown(&mut self) {
        if self.job.take().is_some() {
            self.log(
                LogDir::Out,
                "; desconexión: se detiene el trabajo en marcha (D8 01)",
            );
            match self.session.stop() {
                Ok(()) => self.job_event(JobState::Cancelled, "Trabajo detenido al desconectar."),
                Err(e) => self.log(
                    LogDir::In,
                    &format!(
                        "; la máquina no confirmó Detener ({e}): puede seguir ejecutando; usa el \
                         panel de la máquina o el paro de emergencia"
                    ),
                ),
            }
        }
    }

    // ---- estado ------------------------------------------------------------

    /// Un sondeo de estado. `Err` = conexión perdida (el hilo termina).
    fn poll(&mut self) -> Result<(), String> {
        if let Some(sim) = self.sim.as_mut() {
            sim.tick();
        }
        match self.session.status() {
            Ok(st) => {
                self.failures = 0;
                self.silent_logged = false;
                self.on_status(&st);
                Ok(())
            }
            Err(SessionError::Io(e)) => Err(format!(
                "se perdió la conexión con la controladora: error de red ({e})"
            )),
            Err(e) => {
                self.failures += 1;
                if self.job.is_some() {
                    // Con un trabajo en marcha el silencio no es una pérdida: se sigue esperando.
                    if !self.silent_logged {
                        self.silent_logged = true;
                        self.log(
                            LogDir::In,
                            "; la controladora no contesta mientras ejecuta el trabajo (algunas \
                             ignoran la red); se sigue esperando",
                        );
                    }
                    self.report_busy();
                    Ok(())
                } else if self.failures >= self.timing.lost_after {
                    Err(format!(
                        "se perdió la conexión con la controladora ({e}). Revisa el cable de red \
                         y que la máquina siga encendida."
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Publica «ocupada» sin posición nueva (la máquina calla durante el trabajo).
    fn report_busy(&mut self) {
        let mut report = self
            .shared
            .status
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .unwrap_or_else(idle_report);
        report.state = MachineState::Run;
        self.publish(report);
    }

    fn publish(&self, report: StatusReport) {
        if let Ok(mut g) = self.shared.status.lock() {
            *g = Some(report.clone());
        }
        self.sink.emit(Emit::Status(report));
    }

    fn on_status(&mut self, st: &MachineStatus) {
        let mut state = match (st.running, st.paused, st.moving) {
            (true, true, _) => MachineState::Hold,
            (true, false, _) => MachineState::Run,
            (false, _, true) => MachineState::Jog,
            _ => MachineState::Idle,
        };
        let waiting_to_start = self.job.as_ref().is_some_and(|j| !j.seen_running);
        if state != MachineState::Hold
            && (self.frame.is_some() || (waiting_to_start && st.is_idle()))
        {
            state = MachineState::Run;
        }
        let pos = [st.pos_um.0 as f64 / 1000.0, st.pos_um.1 as f64 / 1000.0];
        let mut report = idle_report();
        report.state = state;
        report.machine_pos = pos;
        report.work_pos = pos;
        self.publish(report);
        self.follow_job(st);
        self.follow_frame(st);
    }

    /// Sigue un trabajo ejecutándose: pausas hechas desde el panel y final.
    fn follow_job(&mut self, st: &MachineStatus) {
        let Some(job) = self.job.as_mut() else {
            return;
        };
        if st.running {
            job.seen_running = true;
            if st.paused != job.paused {
                job.paused = st.paused;
                let (state, msg) = if st.paused {
                    (JobState::Paused, "Trabajo en pausa (desde la máquina).")
                } else {
                    (JobState::Started, "Trabajo reanudado (desde la máquina).")
                };
                self.job_event(state, msg);
            }
        } else if st.is_idle()
            && (job.seen_running || job.since.elapsed() > self.timing.start_grace)
        {
            self.job = None;
            self.job_event(JobState::Finished, "Trabajo terminado.");
        }
    }

    /// Avanza el encuadre: cuando el cabezal llegó a una esquina, va a la siguiente.
    fn follow_frame(&mut self, st: &MachineStatus) {
        if self.frame.is_none() || !st.is_idle() {
            return;
        }
        let next = self.frame.as_mut().and_then(|f| f.corners.pop_front());
        match next {
            Some((x, y)) => {
                if let Err(e) = self.session.jog_machine(x, y) {
                    self.frame = None;
                    self.log(
                        LogDir::In,
                        &format!("; encuadre interrumpido: {}", op_error(e)),
                    );
                }
            }
            None => {
                self.frame = None;
                self.log(LogDir::In, "; encuadre terminado");
            }
        }
    }

    // ---- solicitudes -------------------------------------------------------

    fn handle(&mut self, req: Request) {
        match req {
            Request::Upload(up, reply) => {
                let r = self.upload(*up);
                let _ = reply.send(r);
            }
            Request::Frame {
                corners,
                speed_mm_min,
                reply,
            } => {
                let r = self.start_frame(corners, speed_mm_min);
                let _ = reply.send(r);
            }
            Request::Jog {
                dx,
                dy,
                speed_mm_min,
                reply,
            } => {
                let r = self.jog(dx, dy, speed_mm_min);
                let _ = reply.send(r);
            }
            Request::JogTo {
                x,
                y,
                speed_mm_min,
                reply,
            } => {
                let r = self.jog_to(x, y, speed_mm_min);
                let _ = reply.send(r);
            }
            Request::Pause(reply) => {
                let r = self.session.pause().map_err(op_error);
                if r.is_ok()
                    && let Some(j) = self.job.as_mut()
                    && !j.paused
                {
                    j.paused = true;
                    self.job_event(JobState::Paused, "Trabajo en pausa.");
                }
                let _ = reply.send(r);
            }
            Request::Resume(reply) => {
                let r = self.session.resume().map_err(op_error);
                if r.is_ok()
                    && let Some(j) = self.job.as_mut()
                    && j.paused
                {
                    j.paused = false;
                    self.job_event(JobState::Started, "Trabajo reanudado.");
                }
                let _ = reply.send(r);
            }
            Request::Stop(reply) => {
                let r = self.stop();
                let _ = reply.send(r);
            }
            Request::Info(reply) => {
                let r = self.info();
                let _ = reply.send(r.map(Box::new));
            }
            Request::Files(reply) => {
                let r = self.files();
                let _ = reply.send(r);
            }
            Request::Disconnect => unreachable!("se atiende en el ciclo principal"),
        }
    }

    fn busy_error(&self) -> Option<String> {
        if self.job.is_some() {
            Some("Hay un trabajo en curso. Espera a que termine o detenlo.".into())
        } else if self.frame.is_some() {
            Some("Se está recorriendo un encuadre. Espera a que termine o detenlo.".into())
        } else {
            None
        }
    }

    fn set_jog_speed(&mut self, speed_mm_min: f64) {
        if speed_mm_min.is_finite() && speed_mm_min > 0.0 {
            self.session.config_mut().jog_speed_mm_s = speed_mm_min / 60.0;
        }
    }

    fn jog(&mut self, dx: f64, dy: f64, speed_mm_min: f64) -> Result<(), String> {
        if let Some(e) = self.busy_error() {
            return Err(e);
        }
        if !(dx.is_finite() && dy.is_finite()) {
            return Err("El desplazamiento no es válido.".into());
        }
        self.set_jog_speed(speed_mm_min);
        let st = self.session.status().map_err(op_error)?;
        let (x, y) = (
            st.pos_um.0 as f64 / 1000.0 + dx,
            st.pos_um.1 as f64 / 1000.0 + dy,
        );
        self.session.jog_machine(x, y).map_err(op_error)
    }

    fn jog_to(&mut self, x: f64, y: f64, speed_mm_min: f64) -> Result<(), String> {
        if let Some(e) = self.busy_error() {
            return Err(e);
        }
        if !(x.is_finite() && y.is_finite()) {
            return Err("La posición no es válida.".into());
        }
        self.set_jog_speed(speed_mm_min);
        self.session.jog_machine(x, y).map_err(op_error)
    }

    fn start_frame(&mut self, corners: Vec<(f64, f64)>, speed_mm_min: f64) -> Result<(), String> {
        if let Some(e) = self.busy_error() {
            return Err(e);
        }
        let mut queue: VecDeque<(f64, f64)> = corners.into();
        let Some((x, y)) = queue.pop_front() else {
            return Err("No hay nada que encuadrar: el trabajo está vacío.".into());
        };
        self.set_jog_speed(speed_mm_min);
        // La primera esquina comprueba que la máquina está en reposo y dentro de la cama.
        self.session.jog_machine(x, y).map_err(op_error)?;
        self.log(
            LogDir::Out,
            "; encuadre: recorrido del rectángulo con el láser apagado",
        );
        self.frame = Some(FrameRun { corners: queue });
        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        self.frame = None;
        let r = self.session.stop().map_err(op_error);
        // La subida (si la había) ya terminó: la orden de cancelar quedó atendida.
        self.shared.cancel.store(false, Ordering::SeqCst);
        if r.is_ok() {
            if let Some(sim) = self.sim.as_mut() {
                sim.remaining = None;
            }
            if self.job.take().is_some() {
                self.job_event(
                    JobState::Cancelled,
                    "Trabajo detenido (D8 01): el cabezal frena y el láser se apaga.",
                );
            } else {
                self.log(LogDir::Out, "; detener (D8 01)");
            }
        }
        r
    }

    fn info(&mut self) -> Result<RuidaInfo, String> {
        if self.job.is_some() {
            return Err("Espera a que termine el trabajo para leer la controladora.".into());
        }
        self.log(
            LogDir::Out,
            "; información de la controladora (solo lectura)",
        );
        self.session
            .controller_info()
            .map(to_info)
            .map_err(op_error)
    }

    fn files(&mut self) -> Result<Vec<RuidaFile>, String> {
        if self.job.is_some() {
            return Err("Espera a que termine el trabajo para leer la controladora.".into());
        }
        self.log(
            LogDir::Out,
            "; lista de documentos de la controladora (solo lectura, experimental)",
        );
        self.session
            .list_documents()
            .map(|v| {
                v.into_iter()
                    .map(|f| RuidaFile {
                        index: f.index,
                        name: f.name,
                    })
                    .collect()
            })
            .map_err(op_error)
    }

    fn upload(&mut self, up: Upload) -> Result<(), String> {
        if let Some(e) = self.busy_error() {
            return Err(e);
        }
        // El origen del archivo depende de la cama: si la máquina informa otra, no se envía.
        if let Some([w, h]) = self.bed_mm
            && ((w - up.limits.bed_mm[0]).abs() > 1.0 || (h - up.limits.bed_mm[1]).abs() > 1.0)
        {
            return Err(format!(
                "La cama que informa la máquina ({w:.0} × {h:.0} mm) no coincide con la del \
                 perfil ({:.0} × {:.0} mm): el origen del archivo depende de ella. Corrige el \
                 tamaño de la cama en el perfil.",
                up.limits.bed_mm[0], up.limits.bed_mm[1]
            ));
        }
        {
            let c = self.session.config_mut();
            c.limits = up.limits;
            c.home = up.home;
            c.ack_timeout = self.timing.upload_ack;
        }
        for note in &up.notes {
            self.log(LogDir::In, &format!("; {note}"));
        }
        let name = match &up.mode {
            SendMode::Run => "ejecutar al llegar".to_string(),
            SendMode::Store { name } => format!("guardar como «{name}»"),
        };
        self.log(
            LogDir::Out,
            &format!("; trabajo Ruida: {} bytes, {name}", up.rd.len()),
        );
        let sink = self.sink.clone();
        let mut started = false;
        let result = self
            .session
            .send_job(&up.rd, &up.mode, &self.shared.cancel, &mut |p| {
                if !started {
                    started = true;
                    sink.emit(Emit::Job(JobEvent {
                        state: JobState::Started,
                        message: format!("Enviando el trabajo ({} paquetes).", p.packets),
                        position_lost: false,
                    }));
                }
                sink.emit(Emit::Progress(ProgressEvent {
                    acked: u32::try_from(p.packet).unwrap_or(u32::MAX),
                    total: u32::try_from(p.packets).unwrap_or(u32::MAX),
                }));
            });
        self.session.config_mut().ack_timeout = self.timing.poll_timeout;
        match result {
            Ok(report) => {
                self.log(
                    LogDir::In,
                    &format!(
                        "; subido: {} paquetes, {} bytes, {} reenvíos",
                        report.packets, report.bytes, report.resends
                    ),
                );
                match up.mode {
                    SendMode::Store { name } => self.job_event(
                        JobState::Finished,
                        &format!(
                            "Documento «{name}» guardado en la memoria de la controladora. No se \
                             ejecutó: selecciónalo e inícialo desde el panel de la máquina."
                        ),
                    ),
                    SendMode::Run => {
                        if let Some(sim) = self.sim.as_mut() {
                            sim.start(up.estimate_s);
                        }
                        self.job = Some(ActiveJob {
                            since: Instant::now(),
                            seen_running: false,
                            paused: false,
                        });
                        self.log(LogDir::In, "; el trabajo se está ejecutando en la máquina");
                    }
                }
                Ok(())
            }
            Err(e) => {
                let msg = e.to_string();
                if e.machine_may_be_running {
                    self.log(
                        LogDir::In,
                        "; AVISO: la máquina puede estar ejecutando el trabajo incompleto: pulsa \
                         Stop en su panel o usa el paro de emergencia",
                    );
                }
                if matches!(e.cause, SessionError::Cancelled) {
                    self.job_event(JobState::Cancelled, &format!("Envío cancelado. {msg}"));
                    Ok(())
                } else if !started {
                    // Nada llegó a salir (trabajo inválido, máquina ocupada, sin respuesta).
                    Err(op_error_text(&e.cause, &msg))
                } else {
                    self.job_event(JobState::Error, &format!("Falló el envío: {msg}"));
                    Ok(())
                }
            }
        }
    }
}

/// Texto de un fallo de subida antes del primer paquete: el de la causa con su pista.
fn op_error_text(cause: &SessionError, full: &str) -> String {
    match cause {
        SessionError::Timeout { .. } | SessionError::Busy(_) => {
            // `UploadError` ya añade el aviso sobre el documento incompleto, que aquí no aplica.
            let base = full.split(" (paquetes confirmados").next().unwrap_or(full);
            match cause {
                SessionError::Timeout { .. } => format!(
                    "{base}. Comprueba la IP, la red y que la máquina no esté ejecutando otro \
                     trabajo."
                ),
                _ => format!("{base}. Espera a que termine o pulsa Detener en la máquina."),
            }
        }
        _ => full
            .split(" (paquetes confirmados")
            .next()
            .unwrap_or(full)
            .to_string(),
    }
}

fn idle_report() -> StatusReport {
    StatusReport {
        state: MachineState::Idle,
        other_state: None,
        machine_pos: [0.0, 0.0],
        work_pos: [0.0, 0.0],
        feed: 0.0,
        spindle: 0.0,
        // Ruida no informa búferes: ceros (la interfaz los oculta con Ruida).
        buffer: BufferStatus { planner: 0, rx: 0 },
        overrides: None,
    }
}

/// Información de la sesión a los tipos del contrato con la interfaz.
fn to_info(i: ControllerInfo) -> RuidaInfo {
    let secs = |v: Option<u64>| v.map(|v| v as f64);
    RuidaInfo {
        model: i.probe.model.map(str::to_string),
        card_id: format!("0x{:08X}", i.probe.card_id),
        magic: i.probe.magic.0,
        bed: i.probe.bed_mm,
        rapid_mm_s: i.rapid_mm_s,
        home_mm_s: i.home_mm_s,
        axis_max_mm_s: i.axis_max_mm_s,
        axis_accel_mm_s2: i.axis_accel_mm_s2,
        total_on_s: secs(i.total_on_s),
        total_work_s: secs(i.total_work_s),
        total_laser_s: secs(i.total_laser_s),
        total_jobs: i.total_jobs.map(|v| u32::try_from(v).unwrap_or(u32::MAX)),
        travel_m: [secs(i.travel_m[0]), secs(i.travel_m[1])],
        documents: i.documents,
    }
}

// ---------------------------------------------------------------------------
// Del documento a la controladora
// ---------------------------------------------------------------------------

/// Nombre del documento que se guarda en la memoria: `TLAKU` y tres cifras del reloj, para no
/// repetirlo entre envíos seguidos (ASCII y corto, como los que escribe RDWorks).
fn store_name() -> String {
    let minutes = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 60);
    format!("TLAKU{:03}", minutes % 1000)
}

/// Calcula el trabajo y lo convierte en un flujo `.rd` listo para subir. `head` es la posición
/// del cabezal en mm del sistema de la máquina (para empezar desde la posición actual).
pub fn build_upload(
    doc: &tlaku_doc::Document,
    device: &DeviceProfile,
    options: &tlaku_doc::JobOptions,
    head: Option<[f64; 2]>,
) -> Result<Upload, String> {
    let at = head.map(|[x, y]| tlaku_core::Point::new(x, y));
    let job = tlaku_pipeline::compute_at(doc, device, options, at)?;
    let rd = tlaku_pipeline::export_rd_for_device(&job, device)?;
    Ok(Upload {
        rd,
        mode: if device.ruida.run_after_upload {
            SendMode::Run
        } else {
            SendMode::Store { name: store_name() }
        },
        limits: tlaku_pipeline::limits_for_device(device),
        home: tlaku_pipeline::home_for_origin(device.origin),
        estimate_s: job.preview.total_s,
        notes: if device.rotary.enabled {
            vec![ROTARY_NOTE.to_string()]
        } else {
            Vec::new()
        },
    })
}

/// Esquinas del encuadre (rectángulo de todos los movimientos, overscan incluido) en mm del
/// sistema de la máquina, cerrando el recorrido en la primera.
pub fn frame_corners(
    doc: &tlaku_doc::Document,
    device: &DeviceProfile,
    options: &tlaku_doc::JobOptions,
    head: Option<[f64; 2]>,
) -> Result<Vec<(f64, f64)>, String> {
    let at = head.map(|[x, y]| tlaku_core::Point::new(x, y));
    let job = tlaku_pipeline::compute_at(doc, device, options, at)?;
    let b = job
        .bounds()
        .ok_or("No hay nada que encuadrar: el trabajo está vacío.")?;
    let home = tlaku_pipeline::home_for_origin(device.origin);
    let (ax, ay) = home.to_machine(b.min, device.bed);
    let (cx, cy) = home.to_machine(b.max, device.bed);
    let (x0, x1, y0, y1) = (ax.min(cx), ax.max(cx), ay.min(cy), ay.max(cy));
    Ok(vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)])
}

#[cfg(test)]
mod tests;
