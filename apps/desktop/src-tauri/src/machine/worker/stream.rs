//! Transmisión: el trabajador mientras el `Streamer` tiene el puerto, y el cierre del trabajo.

use super::*;
use tlaku_i18n::tr;

impl Worker {
    pub(super) fn stream_step(&mut self, mut run: Run) -> Result<Phase, Fail> {
        while let Some(req) = self.next_request()? {
            match req {
                Request::Disconnect => {
                    run.job.control.cancel();
                    self.wait_finished(run);
                    return Ok(Phase::Done);
                }
                other => self.stream_request(&mut run, other),
            }
        }
        match run.job.events.recv_timeout(self.timing.tick) {
            Ok(Event::Finished(_)) | Err(RecvTimeoutError::Disconnected) => self.finish(run),
            Ok(ev) => {
                self.stream_event(&mut run, ev);
                Ok(Phase::Streaming(run))
            }
            Err(RecvTimeoutError::Timeout) => Ok(Phase::Streaming(run)),
        }
    }

    pub(super) fn stream_request(&mut self, run: &mut Run, req: Request) {
        let ok = |reply: Reply| {
            let _ = reply.send(Ok(Vec::new()));
        };
        match req {
            Request::Pause(reply) => {
                self.pause(run);
                ok(reply);
            }
            Request::Resume(reply) => {
                self.resume(run);
                ok(reply);
            }
            Request::Cancel(reply) => {
                self.cancel(run);
                ok(reply);
            }
            Request::Realtime { cmd, reply } => {
                match cmd {
                    RealtimeCommand::Status => {}
                    RealtimeCommand::Hold => self.pause(run),
                    RealtimeCommand::Resume => self.resume(run),
                    // Marlin y Smoothieware ignoran los bytes en tiempo real: se cancela el trabajo.
                    RealtimeCommand::Reset if self.link_family != LinkFamily::Grbl => {
                        self.cancel(run);
                    }
                    RealtimeCommand::Reset => {
                        run.cancelling = true;
                        run.job.control.realtime(Realtime::SoftReset);
                    }
                    RealtimeCommand::JogCancel if self.link_family != LinkFamily::Grbl => {}
                    RealtimeCommand::JogCancel => run.job.control.realtime(Realtime::JogCancel),
                }
                ok(reply);
            }
            // Los overrides son justo para esto: se mandan por el mando del trabajo en curso.
            Request::Override {
                kind,
                percent,
                reply,
            } => {
                let r = self.override_bytes(kind, percent).map(|plan| {
                    for r in plan {
                        run.job.control.realtime(r);
                    }
                    self.note_override(kind, percent);
                });
                let _ = reply.send(match r {
                    Ok(()) => Ok(Vec::new()),
                    Err(Fail::Msg(m) | Fail::Lost(m)) => Err(m),
                });
            }
            Request::Command { reply, .. }
            | Request::Jog { reply, .. }
            | Request::JogTo { reply, .. }
            | Request::Probe { reply, .. }
            | Request::Start { reply, .. } => {
                let _ = reply.send(Err(tr!("desktop.worker.job_running")));
            }
            Request::Disconnect => unreachable!("se atiende antes"),
        }
    }

    pub(super) fn pause(&mut self, run: &Run) {
        run.job.control.pause();
        self.set_reported_state(MachineState::Hold);
        if run.kind == RunKind::Job {
            self.job_event(JobState::Paused, &tr!("desktop.worker.job_paused"));
        }
    }

    pub(super) fn resume(&mut self, run: &Run) {
        run.job.control.resume();
        self.set_reported_state(MachineState::Run);
        if run.kind == RunKind::Job {
            self.job_event(JobState::Started, &tr!("desktop.worker.job_resumed"));
        }
    }

    pub(super) fn cancel(&self, run: &mut Run) {
        run.cancelling = true;
        run.job.control.cancel();
    }

    pub(super) fn stream_event(&mut self, run: &mut Run, ev: Event) {
        match ev {
            Event::Progress {
                acked, sent, total, ..
            } => {
                if let Some(c) = &run.checkpoint {
                    c.writer.note_acked(acked);
                    c.writer.note_sent(sent);
                    if let Some(e) = c.writer.take_error() {
                        self.log(
                            LogDir::In,
                            &tr!("desktop.resume.checkpoint_write_failed", err = e),
                        );
                    }
                }
                if run.kind != RunKind::Job {
                    return;
                }
                let due = run
                    .last_progress
                    .is_none_or(|t| t.elapsed() >= self.timing.progress_interval);
                if due || acked == total {
                    run.last_progress = Some(Instant::now());
                    self.sink.emit(Emit::Progress(ProgressEvent {
                        acked: acked.try_into().unwrap_or(u32::MAX),
                        total: total.try_into().unwrap_or(u32::MAX),
                    }));
                }
            }
            Event::Status(r) => {
                self.on_status(&r);
                if let Some(c) = &run.checkpoint {
                    c.writer.note_status(resume::StatusSnap {
                        mpos: self.last.machine_pos,
                        wpos: self.last.work_pos,
                        wco: self.wco,
                        planner_free: self.last.buffer.planner,
                        rx_free: self.last.buffer.rx,
                    });
                }
            }
            Event::Error {
                code,
                source_line,
                text,
            } => {
                let msg = error_message_for(self.fw, code);
                let shown = match (source_line, text) {
                    (Some(l), Some(t)) => tr!(
                        "desktop.worker.log_error_at_line",
                        code = code,
                        line = l,
                        text = t,
                        msg = msg
                    ),
                    _ => format!("error:{code}: {msg}"),
                };
                self.log(LogDir::In, &shown);
            }
            Event::Alarm(code) => {
                self.log(
                    LogDir::In,
                    &format!("ALARM:{code}: {}", alarm_message_for(self.fw, code)),
                );
            }
            Event::Reset(m) => {
                // Reinicio por su cuenta: la referencia del homing ya no vale.
                self.shared.homed.store(false, Ordering::SeqCst);
                self.note_line(&m);
                self.log(LogDir::In, &m);
            }
            Event::Message(m) => {
                self.note_line(&m);
                self.log(LogDir::In, &m);
            }
            Event::Finished(_) => {}
        }
    }

    /// Espera el fin del envío tras pedir la cancelación.
    pub(super) fn wait_finished(&mut self, run: Run) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            match run.job.events.recv_timeout(Duration::from_millis(50)) {
                Ok(Event::Finished(_)) | Err(RecvTimeoutError::Disconnected) => break,
                Ok(_) | Err(RecvTimeoutError::Timeout) => {}
            }
        }
        self.shared.running.store(false, Ordering::SeqCst);
        self.shared.streaming.store(false, Ordering::SeqCst);
        // Desconectar con un trabajo en curso lo detiene, pero no es «cancelar a propósito»: el
        // punto de control se queda para poder continuarlo (se borra al pasar a otra máquina).
        if let Some(c) = run.checkpoint {
            c.writer.finish(resume::Finish::Keep, &c.store);
        }
        if run.kind == RunKind::Job {
            self.job_event(
                JobState::Cancelled,
                &tr!("desktop.worker.job_cancelled_disconnect"),
            );
        }
        let _ = run.job.join();
    }

    /// El `Streamer` terminó: recupera el puerto y comunica el resultado.
    pub(super) fn finish(&mut self, run: Run) -> Result<Phase, Fail> {
        let kind = run.kind;
        let checkpoint = run.checkpoint;
        self.shared.running.store(false, Ordering::SeqCst);
        self.shared.streaming.store(false, Ordering::SeqCst);
        let (streamer, outcome) = run
            .job
            .join()
            .map_err(|_| Fail::Lost(tr!("desktop.worker.stream_thread_died")))?;
        // Terminó bien o se canceló a propósito: el punto de control se borra (antes del evento,
        // para que la interfaz ya lo vea borrado). Cualquier otro final lo conserva.
        if let Some(c) = checkpoint {
            let how = match outcome {
                Outcome::Completed { .. } | Outcome::Cancelled => resume::Finish::Discard,
                _ => resume::Finish::Keep,
            };
            if let Some(e) = c.writer.take_error() {
                self.log(
                    LogDir::In,
                    &tr!("desktop.resume.checkpoint_write_failed", err = e),
                );
            }
            c.writer.finish(how, &c.store);
        }
        // Si la máquina se detuvo (cancelación, error, falla de Creality), cómo quedó.
        // `ALARM:3` es justo eso: reinicio con la máquina en movimiento.
        let position_lost = streamer.last_stop().is_some_and(|s| s.position_maybe_lost)
            || matches!(outcome, Outcome::Alarm(3));
        // Si el streamer mandó un reinicio (cancelar, error), el saludo que sigue es esperado.
        if streamer.last_stop().is_some() {
            self.note_own_reset();
        }
        if position_lost {
            self.shared.homed.store(false, Ordering::SeqCst);
        }
        // La placa se reinició por su cuenta a mitad del trabajo: posición sin confirmar.
        if matches!(outcome, Outcome::Reset(_)) {
            self.mark_unconfirmed();
        }
        let t = streamer.into_transport();
        self.reader.clear();
        self.last_poll = None;
        self.last_rx = Instant::now();
        // Un ALARM:1/2 que terminó el trabajo deja la placa bloqueada y callada hasta un
        // reset; tras cualquier otro final ya no hay bloqueo que esperar.
        self.quiet = matches!(outcome, Outcome::Alarm(1 | 2)).then_some(Quiet::CriticalAlarm);
        self.set_reported_state(MachineState::Idle);
        match (&outcome, kind) {
            (Outcome::Completed { errors: 0 }, RunKind::Job) => {
                self.job_event(JobState::Finished, &tr!("desktop.worker.job_finished"));
            }
            (Outcome::Completed { errors: 0 }, RunKind::Frame) => {
                self.log(LogDir::In, &frame_done_log());
            }
            (Outcome::Cancelled, RunKind::Job) => {
                let mut msg = tr!("desktop.worker.job_cancelled");
                if position_lost {
                    msg.push(' ');
                    msg.push_str(&position_lost_note());
                }
                self.job_event_at(JobState::Cancelled, &msg, position_lost);
            }
            (Outcome::Cancelled, RunKind::Frame) => {
                self.log(LogDir::In, &frame_cancelled_log());
                if position_lost {
                    self.log(LogDir::In, &format!("; {}", position_lost_note()));
                }
            }
            _ => {
                let mut msg = outcome_result_for(self.fw, &outcome)
                    .err()
                    .unwrap_or_else(|| tr!("desktop.worker.stream_errors"));
                if position_lost {
                    msg.push(' ');
                    msg.push_str(&position_lost_note());
                }
                self.log(LogDir::In, &first_line(&msg));
                self.job_event_at(JobState::Error, &msg, position_lost);
            }
        }
        if let Outcome::Disconnected(m) = outcome {
            return Err(Fail::Lost(tr!(
                "desktop.worker.connection_lost_io",
                err = m
            )));
        }
        Ok(Phase::Idle(t))
    }
}
