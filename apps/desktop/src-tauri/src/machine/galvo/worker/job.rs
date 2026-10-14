//! Marcado: espera de movimiento entre piezas, `run_job` y su cierre.

use super::*;

impl Worker {
    /// Espera que el usuario mueva el material y confirme. `Err(())` = se pidió detener.
    pub(super) fn wait_move(&mut self, info: &StepInfo) -> Result<(), ()> {
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
    pub(super) fn sleep_cancelable(&self, ms: f64) -> Result<(), ()> {
        let end = Instant::now() + Duration::try_from_secs_f64(ms / 1000.0).unwrap_or_default();
        while Instant::now() < end {
            if self.cancelled() {
                return Err(());
            }
            thread::sleep(Duration::from_millis(5).min(end - Instant::now()));
        }
        if self.cancelled() { Err(()) } else { Ok(()) }
    }

    pub(super) fn run_job(&mut self, job: PreparedJob, reply: Reply<()>) {
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
    pub(super) fn end_job(
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
