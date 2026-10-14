//! Tiempos, solicitudes al hilo y el mando [`LhyHandle`].

use super::*;

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

pub(super) type Reply<T> = Sender<Result<T, String>>;

pub(super) enum Request {
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

pub(super) struct Shared {
    /// El hilo terminó (desconexión pedida o perdida).
    pub(super) dead: AtomicBool,
    /// El hilo está ocupado con un trabajo, un home o un encuadre.
    pub(super) busy: AtomicBool,
    /// Se pidió detener: interrumpe un encuadre y un arranque que aún no envió nada.
    pub(super) cancel: AtomicBool,
    /// Se conoce la posición del cabezal.
    pub(super) pos_known: AtomicBool,
    /// Último código de estado de la placa.
    pub(super) status_code: Mutex<u8>,
    pub(super) status: Mutex<Option<StatusReport>>,
    pub(super) control: JobControl,
    pub(super) sink: Arc<dyn Sink>,
    pub(super) virtual_machine: bool,
    pub(super) board: Board,
    /// Lo que se usó al conectar: si el perfil cambia, hay que reconectar.
    pub(super) cfg: SessionConfig,
    pub(super) timing: LhyTiming,
}

/// Mando de la conexión; se clona y se usa desde cualquier hilo. Las llamadas bloquean hasta que
/// el hilo responde (en Tauri, dentro de `spawn_blocking`).
#[derive(Clone)]
pub struct LhyHandle {
    pub(super) tx: Sender<Request>,
    pub(super) shared: Arc<Shared>,
}

impl LhyHandle {
    fn call<T>(&self, build: impl FnOnce(Reply<T>) -> Request) -> Result<T, String> {
        if self.is_closed() {
            return Err(crate::machine::NOT_CONNECTED.into());
        }
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(build(tx))
            .map_err(|_| crate::machine::NOT_CONNECTED.to_string())?;
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
        let corners = crate::machine::ruida::frame_corners(doc, device, options, self.work_pos())?;
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
            return Err(crate::machine::NOT_CONNECTED.into());
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
            return Err(crate::machine::NOT_CONNECTED.into());
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
            return Err(crate::machine::NOT_CONNECTED.into());
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
