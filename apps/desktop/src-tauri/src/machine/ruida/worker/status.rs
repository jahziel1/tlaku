//! Sondeo del estado y seguimiento del trabajo y del encuadre.

use super::*;
use tlaku_i18n::tr;

impl<L: PacketLink> Worker<L> {
    /// Un sondeo de estado. `Err` = conexión perdida (el hilo termina).
    pub(super) fn poll(&mut self) -> Result<(), String> {
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
            Err(SessionError::Io(e)) => Err(tr!("desktop.ruida.link_lost_io", err = e)),
            Err(e) => {
                self.failures += 1;
                if self.job.is_some() {
                    // Con un trabajo en marcha el silencio no es una pérdida: se sigue esperando.
                    if !self.silent_logged {
                        self.silent_logged = true;
                        self.log(LogDir::In, &tr!("desktop.ruida.log_silent_during_job"));
                    }
                    self.report_busy();
                    Ok(())
                } else if self.failures >= self.timing.lost_after {
                    Err(tr!("desktop.ruida.link_lost", err = e))
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Publica «ocupada» sin posición nueva (la máquina calla durante el trabajo).
    pub(super) fn report_busy(&mut self) {
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

    pub(super) fn publish(&self, report: StatusReport) {
        if let Ok(mut g) = self.shared.status.lock() {
            *g = Some(report.clone());
        }
        self.sink.emit(Emit::Status(report));
    }

    pub(super) fn on_status(&mut self, st: &MachineStatus) {
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
    pub(super) fn follow_job(&mut self, st: &MachineStatus) {
        let Some(job) = self.job.as_mut() else {
            return;
        };
        if st.running {
            job.seen_running = true;
            if st.paused != job.paused {
                job.paused = st.paused;
                let (state, msg) = if st.paused {
                    (JobState::Paused, tr!("desktop.ruida.job_paused_remote"))
                } else {
                    (JobState::Started, tr!("desktop.ruida.job_resumed_remote"))
                };
                self.job_event(state, &msg);
            }
        } else if st.is_idle()
            && (job.seen_running || job.since.elapsed() > self.timing.start_grace)
        {
            self.job = None;
            self.job_event(JobState::Finished, &tr!("desktop.worker.job_finished"));
        }
    }

    /// Avanza el encuadre: cuando el cabezal llegó a una esquina, va a la siguiente.
    pub(super) fn follow_frame(&mut self, st: &MachineStatus) {
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
                        &tr!("desktop.lhymicro.log_frame_interrupted", err = op_error(e)),
                    );
                }
            }
            None => {
                self.frame = None;
                self.log(LogDir::In, &frame_done_log());
            }
        }
    }
}
