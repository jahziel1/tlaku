//! Puntero rojo, saltos y encuadre.

use super::*;
use tlaku_i18n::tr;

impl Worker {
    pub(super) fn red_light(&mut self, on: bool) -> Result<(), String> {
        let r = self.session.set_red_light(on);
        match r {
            Ok(()) => {
                self.shared.red_on.store(on, Ordering::SeqCst);
                self.log(
                    LogDir::Out,
                    &if on {
                        tr!("desktop.galvo.log_red_on")
                    } else {
                        tr!("desktop.galvo.log_red_off")
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
                self.log(LogDir::In, &format!("; {}", tlaku_i18n::split_code(&msg).1));
                if e.laser_may_be_on() {
                    self.note_laser_may_be_on(&msg);
                }
                Err(msg)
            }
        }
    }

    pub(super) fn jog_to(&mut self, x: f64, y: f64) -> Result<(), String> {
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
    pub(super) fn stop_idle(&mut self) -> Result<(), String> {
        let outcome = self.session.stop();
        self.shared.red_on.store(false, Ordering::SeqCst);
        self.set_position(None);
        self.publish(MachineState::Idle);
        match outcome {
            StopOutcome::Acked | StopOutcome::NotNeeded => {
                self.note_safe();
                self.log(LogDir::Out, &tr!("desktop.galvo.log_stop_acked"));
                Ok(())
            }
            StopOutcome::Unconfirmed => {
                let msg =
                    tlaku_i18n::coded(STOP_UNCONFIRMED, tr!("desktop.galvo.stop_unconfirmed"));
                self.note_laser_may_be_on(&msg);
                Err(msg)
            }
        }
    }

    // ---- encuadre ----------------------------------------------------------

    pub(super) fn run_light(&mut self, shape: &LightShape, loops: Option<u32>, reply: Reply<()>) {
        let ctl = self.shared.control.clone();
        self.log(LogDir::Out, &tr!("desktop.galvo.log_light_start"));
        self.publish(MachineState::Run);
        self.shared.framing.store(true, Ordering::SeqCst);
        let mut reply = Some(reply);
        let sink = self.sink.clone();
        let every = self.timing.light_interval;
        let mut last_emit: Option<Instant> = None;
        let result = self.session.light(shape, &ctl, loops, &mut |p| {
            if let Some(r) = reply.take() {
                let _ = r.send(Ok(()));
            }
            if last_emit.is_none_or(|t| t.elapsed() >= every) {
                last_emit = Some(Instant::now());
                sink.emit(Emit::Galvo(GalvoEvent::Light {
                    loops: p.loops,
                    adjust_gen: p.adjust_gen,
                    adjust_rejected: p.adjust_rejected,
                }));
            }
        });
        self.shared.framing.store(false, Ordering::SeqCst);
        self.shared.red_on.store(false, Ordering::SeqCst);
        self.set_position(None);
        self.publish(MachineState::Idle);
        match result {
            Ok(report) => {
                self.note_safe();
                self.log(
                    LogDir::In,
                    &tr!("desktop.galvo.log_light_done", loops = report.loops),
                );
                if let Some(r) = reply.take() {
                    self.finish(r, Ok(()));
                } else {
                    self.shared.busy.store(false, Ordering::SeqCst);
                }
            }
            Err(e) => {
                let msg = job_error_text(&e);
                self.log(LogDir::In, &format!("; {}", tlaku_i18n::split_code(&msg).1));
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
}
