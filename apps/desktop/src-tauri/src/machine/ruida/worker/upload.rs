//! Subida del trabajo a la controladora.

use super::*;

impl<L: PacketLink> Worker<L> {
    pub(super) fn upload(&mut self, up: Upload) -> Result<(), String> {
        if let Some(e) = self.busy_error() {
            return Err(e);
        }
        // El origen del archivo depende de la cama: si la máquina informa otra, no se envía.
        if let Some([w, h]) = self.bed_mm
            && ((w - up.limits.bed_mm[0]).abs() > 1.0 || (h - up.limits.bed_mm[1]).abs() > 1.0)
        {
            return Err(format!(
                "La cama que informa la máquina ({w:.0} × {h:.0} mm) no coincide con la del \
                 perfil ({:.0} × {:.0} mm): el origen del archivo depende de ella. Corrige el \
                 tamaño de la cama en el perfil.",
                up.limits.bed_mm[0], up.limits.bed_mm[1]
            ));
        }
        {
            let c = self.session.config_mut();
            c.limits = up.limits;
            c.home = up.home;
            c.ack_timeout = self.timing.upload_ack;
        }
        for note in &up.notes {
            self.log(LogDir::In, &format!("; {note}"));
        }
        let name = match &up.mode {
            SendMode::Run => "ejecutar al llegar".to_string(),
            SendMode::Store { name } => format!("guardar como «{name}»"),
        };
        self.log(
            LogDir::Out,
            &format!("; trabajo Ruida: {} bytes, {name}", up.rd.len()),
        );
        let sink = self.sink.clone();
        let mut started = false;
        let result = self
            .session
            .send_job(&up.rd, &up.mode, &self.shared.cancel, &mut |p| {
                if !started {
                    started = true;
                    sink.emit(Emit::Job(JobEvent {
                        state: JobState::Started,
                        message: format!("Enviando el trabajo ({} paquetes).", p.packets),
                        position_lost: false,
                    }));
                }
                sink.emit(Emit::Progress(ProgressEvent {
                    acked: u32::try_from(p.packet).unwrap_or(u32::MAX),
                    total: u32::try_from(p.packets).unwrap_or(u32::MAX),
                }));
            });
        self.session.config_mut().ack_timeout = self.timing.poll_timeout;
        match result {
            Ok(report) => {
                self.log(
                    LogDir::In,
                    &format!(
                        "; subido: {} paquetes, {} bytes, {} reenvíos",
                        report.packets, report.bytes, report.resends
                    ),
                );
                match up.mode {
                    SendMode::Store { name } => self.job_event(
                        JobState::Finished,
                        &format!(
                            "Documento «{name}» guardado en la memoria de la controladora. No se \
                             ejecutó: selecciónalo e inícialo desde el panel de la máquina."
                        ),
                    ),
                    SendMode::Run => {
                        if let Some(sim) = self.sim.as_mut() {
                            sim.start(up.estimate_s);
                        }
                        self.job = Some(ActiveJob {
                            since: Instant::now(),
                            seen_running: false,
                            paused: false,
                        });
                        self.log(LogDir::In, "; el trabajo se está ejecutando en la máquina");
                    }
                }
                Ok(())
            }
            Err(e) => {
                let msg = e.to_string();
                if e.machine_may_be_running {
                    self.log(
                        LogDir::In,
                        "; AVISO: la máquina puede estar ejecutando el trabajo incompleto: pulsa \
                         Stop en su panel o usa el paro de emergencia",
                    );
                }
                if matches!(e.cause, SessionError::Cancelled) {
                    self.job_event(JobState::Cancelled, &format!("Envío cancelado. {msg}"));
                    Ok(())
                } else if !started {
                    // Nada llegó a salir (trabajo inválido, máquina ocupada, sin respuesta).
                    Err(op_error_text(&e.cause, &msg))
                } else {
                    self.job_event(JobState::Error, &format!("Falló el envío: {msg}"));
                    Ok(())
                }
            }
        }
    }
}
