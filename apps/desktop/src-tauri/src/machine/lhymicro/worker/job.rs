//! Envío de un trabajo y su registro.

use super::*;
use tlaku_i18n::tr;

impl Worker {
    pub(super) fn start_job(&mut self, toolpath: Toolpath, reply: Reply<()>) {
        self.shared.cancel.store(false, Ordering::SeqCst);
        // La placa no informa dónde está: sin posición conocida se hace home antes.
        if self.machine_pos().is_none() {
            self.log(LogDir::Out, &tr!("desktop.lhymicro.log_home_first"));
            if let Err(e) = self.home_now() {
                self.finish(reply, Err(e));
                return;
            }
        }
        if self.cancelled() {
            emit_job(
                self.sink.as_ref(),
                JobState::Cancelled,
                &tr!("desktop.lhymicro.send_cancelled_early"),
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
                    &tr!("desktop.lhymicro.job_sending", packets = p.packets),
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
                    &tr!(
                        "desktop.lhymicro.log_sent",
                        packets = report.packets,
                        bytes = report.bytes,
                        resends = report.resends,
                        secs = format!("{:.1}", report.elapsed.as_secs_f64())
                    ),
                );
                emit_job(
                    self.sink.as_ref(),
                    JobState::Finished,
                    &if report.finish_confirmed {
                        tr!("desktop.lhymicro.job_finished")
                    } else {
                        tr!("desktop.lhymicro.job_finished_unconfirmed")
                    },
                    false,
                );
            }
            Err(e) => {
                // Si no se confirmó el apagado, el mensaje lleva un código estable para que la
                // interfaz destaque el aviso del paro de emergencia en cualquier idioma.
                let msg = job_error_text(&e);
                self.log(LogDir::In, &format!("; {}", tlaku_i18n::split_code(&msg).1));
                if e.laser_may_be_on() {
                    self.log(LogDir::In, &tr!("desktop.lhymicro.log_laser_may_be_on"));
                }
                if let Some(r) = reply.take() {
                    // Falló antes del primer paquete confirmado: es un rechazo del envío.
                    self.finish(r, Err(msg));
                } else if matches!(e.cause, SessionError::Cancelled) && !e.laser_may_be_on() {
                    emit_job(
                        self.sink.as_ref(),
                        JobState::Cancelled,
                        &tr!(
                            "desktop.lhymicro.job_stopped",
                            sent = e.packets_sent,
                            total = e.packets_total
                        ),
                        true,
                    );
                } else {
                    emit_job(self.sink.as_ref(), JobState::Error, &msg, true);
                }
            }
        }
    }

    pub(super) fn log_program(&self, program: &Program) {
        self.log(
            LogDir::Out,
            &tr!(
                "desktop.lhymicro.log_program",
                board = program.board,
                bytes = program.stream.len(),
                burn = format!("{:.1}", program.stats.burn_mm),
                travel = format!("{:.1}", program.stats.travel_mm),
                secs = format!("{:.0}", program.stats.est_seconds)
            ),
        );
        for w in &program.warnings {
            self.log(LogDir::In, &tr!("desktop.lhymicro.log_note", note = w));
        }
    }
}
