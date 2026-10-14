//! Envío de un trabajo y su registro.

use super::*;

impl Worker {
    pub(super) fn start_job(&mut self, toolpath: Toolpath, reply: Reply<()>) {
        self.shared.cancel.store(false, Ordering::SeqCst);
        // La placa no informa dónde está: sin posición conocida se hace home antes.
        if self.machine_pos().is_none() {
            self.log(
                LogDir::Out,
                "; la posición del cabezal se desconoce: home antes de empezar",
            );
            if let Err(e) = self.home_now() {
                self.finish(reply, Err(e));
                return;
            }
        }
        if self.cancelled() {
            emit_job(
                self.sink.as_ref(),
                JobState::Cancelled,
                "Envío cancelado antes de empezar.",
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
                    &format!("Enviando el trabajo ({} paquetes).", p.packets),
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
                    &format!(
                        "; enviado: {} paquetes, {} bytes, {} reenvíos por CRC, en {:.1} s",
                        report.packets,
                        report.bytes,
                        report.resends,
                        report.elapsed.as_secs_f64()
                    ),
                );
                emit_job(
                    self.sink.as_ref(),
                    JobState::Finished,
                    &if report.finish_confirmed {
                        "Trabajo terminado: la placa confirmó el final.".to_string()
                    } else {
                        "Trabajo terminado, pero la placa no confirmó el final (estado 236)."
                            .to_string()
                    },
                    false,
                );
            }
            Err(e) => {
                let msg = e.to_string();
                self.log(LogDir::In, &format!("; {msg}"));
                if e.laser_may_be_on() {
                    self.log(
                        LogDir::In,
                        "; AVISO: el láser pudo quedar encendido o la placa en marcha: usa el \
                         paro de emergencia o abre la tapa",
                    );
                }
                if let Some(r) = reply.take() {
                    // Falló antes del primer paquete confirmado: es un rechazo del envío.
                    self.finish(r, Err(msg));
                } else if matches!(e.cause, SessionError::Cancelled) && !e.laser_may_be_on() {
                    emit_job(
                        self.sink.as_ref(),
                        JobState::Cancelled,
                        &format!(
                            "Trabajo detenido: la placa se detuvo y el láser se apagó. La \
                             posición del cabezal se perdió: haz home antes de seguir. \
                             (Paquetes confirmados: {} de {}.)",
                            e.packets_sent, e.packets_total
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
            &format!(
                "; trabajo LHYMICRO-GL (placa {}): {} bytes, {:.1} mm grabados, {:.1} mm de \
                 viajes, tiempo estimado {:.0} s",
                program.board,
                program.stream.len(),
                program.stats.burn_mm,
                program.stats.travel_mm,
                program.stats.est_seconds
            ),
        );
        for w in &program.warnings {
            self.log(LogDir::In, &format!("; AVISO: {w}"));
        }
    }
}
