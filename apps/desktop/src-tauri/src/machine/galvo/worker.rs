//! El hilo dueño de la sesión: bucle (`run`), estado y solicitudes. El puntero, el encuadre y el
//! marcado viven en los submódulos.

mod job;
mod light;

use super::*;

// ---------------------------------------------------------------------------
// Hilo
// ---------------------------------------------------------------------------

pub(super) struct Worker {
    pub(super) session: GalvoSession<Box<dyn Transport>>,
    pub(super) rx: Receiver<Request>,
    pub(super) shared: Arc<Shared>,
    pub(super) sink: Arc<dyn Sink>,
    pub(super) timing: GalvoTimingCfg,
    pub(super) port: String,
    pub(super) bed: [f64; 2],
    /// Lecturas de estado seguidas que fallaron.
    pub(super) failures: u32,
    pub(super) last_state: MachineState,
}

impl Worker {
    pub(super) fn run(mut self) {
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
}
