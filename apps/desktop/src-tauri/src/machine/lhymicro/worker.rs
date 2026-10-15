//! El hilo dueño de la sesión: bucle (`run`), estado y solicitudes. Los movimientos y el trabajo
//! viven en los submódulos.

mod job;
mod motion;

use super::*;
use tlaku_i18n::tr;

pub(super) struct Worker {
    pub(super) session: LhySession<Box<dyn Transport>>,
    pub(super) rx: Receiver<Request>,
    pub(super) shared: Arc<Shared>,
    pub(super) sink: Arc<dyn Sink>,
    pub(super) timing: LhyTiming,
    pub(super) port: String,
    pub(super) home: HomeCorner,
    pub(super) bed: [f64; 2],
    /// Lecturas de estado seguidas que fallaron.
    pub(super) failures: u32,
    pub(super) last_state: MachineState,
    pub(super) last_pos: [f64; 2],
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
        self.shared.dead.store(true, Ordering::SeqCst);
        self.sink.emit(Emit::Connection(ConnectionEvent {
            connected: false,
            port: Some(self.port.clone()),
            error: lost,
        }));
    }

    pub(super) fn log(&self, dir: LogDir, text: &str) {
        log(self.sink.as_ref(), dir, text);
    }

    // ---- estado ------------------------------------------------------------

    /// Posición del cabezal en mm de la máquina (distancia desde el home por eje).
    pub(super) fn machine_pos(&self) -> Option<[f64; 2]> {
        self.session
            .position_mil()
            .map(|(x, y)| [mil_to_mm(x), mil_to_mm(y)])
    }

    /// Publica el estado con la posición actual y deja la marca de "posición conocida".
    pub(super) fn publish(&mut self, state: MachineState) {
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
            position_unconfirmed: None,
        };
        if let Ok(mut g) = self.shared.status.lock() {
            *g = Some(report.clone());
        }
        self.sink.emit(Emit::Status(report));
    }

    /// Un sondeo de estado en reposo. `Err` = conexión perdida (el hilo termina).
    pub(super) fn poll(&mut self) -> Result<(), String> {
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
                    self.log(LogDir::In, &tr!("desktop.lhymicro.log_low_power"));
                }
                self.publish(state);
                Ok(())
            }
            Err(SessionError::NotM2Nano { code }) => {
                Err(tr!("desktop.lhymicro.unknown_status_code", code = code))
            }
            Err(SessionError::Transport(
                e @ (TransportError::Disconnected | TransportError::Io(_)),
            )) => Err(tr!("desktop.lhymicro.link_lost_io", err = e)),
            Err(e) => {
                self.failures += 1;
                if self.failures >= self.timing.lost_after {
                    Err(tr!("desktop.lhymicro.link_lost", err = e))
                } else {
                    Ok(())
                }
            }
        }
    }

    // ---- solicitudes -------------------------------------------------------

    pub(super) fn handle(&mut self, req: Request) {
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
    pub(super) fn finish<T>(&self, reply: Reply<T>, result: Result<T, String>) {
        self.shared.busy.store(false, Ordering::SeqCst);
        let _ = reply.send(result);
    }

    pub(super) fn cancelled(&self) -> bool {
        self.shared.cancel.load(Ordering::SeqCst)
    }

    pub(super) fn home_now(&mut self) -> Result<(), String> {
        self.log(LogDir::Out, &tr!("desktop.lhymicro.log_home"));
        self.publish(MachineState::Home);
        let r = self.session.home();
        self.publish(MachineState::Idle);
        match r {
            Ok(()) => {
                self.log(LogDir::In, &tr!("desktop.lhymicro.log_home_done"));
                Ok(())
            }
            Err(e) => Err(op_error(e)),
        }
    }
}
