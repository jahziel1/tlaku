//! Tiempos, la subida ([`Upload`]), solicitudes al hilo y el mando [`RuidaHandle`].

use super::*;
use tlaku_i18n::tr;

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

pub(super) type Reply<T> = Sender<Result<T, String>>;

pub(super) enum Request {
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
pub(super) struct Shared {
    /// El hilo terminó (desconexión pedida o perdida).
    pub(super) dead: AtomicBool,
    /// Se pidió cancelar: la subida en curso se interrumpe entre paquetes.
    pub(super) cancel: AtomicBool,
    pub(super) status: Mutex<Option<StatusReport>>,
    pub(super) virtual_machine: AtomicBool,
}

/// Mando de la conexión; se clona y se usa desde cualquier hilo. Las llamadas bloquean hasta que
/// el hilo responde (en Tauri, dentro de `spawn_blocking`).
#[derive(Clone)]
pub struct RuidaHandle {
    pub(super) tx: Sender<Request>,
    pub(super) shared: Arc<Shared>,
}

impl RuidaHandle {
    fn call<T>(&self, build: impl FnOnce(Reply<T>) -> Request) -> Result<T, String> {
        if self.is_closed() {
            return Err(crate::machine::not_connected());
        }
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(build(tx))
            .map_err(|_| crate::machine::not_connected())?;
        rx.recv()
            .map_err(|_| tr!("desktop.machine.connection_lost"))?
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
            return Err(tr!("desktop.common.frame_speed"));
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
