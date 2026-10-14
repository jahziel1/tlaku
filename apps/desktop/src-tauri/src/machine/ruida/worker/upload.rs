//! Subida del trabajo a la controladora.

use super::*;
use tlaku_i18n::tr;

impl<L: PacketLink> Worker<L> {
    pub(super) fn upload(&mut self, up: Upload) -> Result<(), String> {
        if let Some(e) = self.busy_error() {
            return Err(e);
        }
        // El origen del archivo depende de la cama: si la máquina informa otra, no se envía.
        if let Some([w, h]) = self.bed_mm
            && ((w - up.limits.bed_mm[0]).abs() > 1.0 || (h - up.limits.bed_mm[1]).abs() > 1.0)
        {
            return Err(tr!(
                "desktop.ruida.bed_mismatch",
                w = format!("{w:.0}"),
                h = format!("{h:.0}"),
                pw = format!("{:.0}", up.limits.bed_mm[0]),
                ph = format!("{:.0}", up.limits.bed_mm[1])
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
        let log_line = match &up.mode {
            SendMode::Run => tr!("desktop.ruida.log_job_run", bytes = up.rd.len()),
            SendMode::Store { name } => {
                tr!(
                    "desktop.ruida.log_job_store",
                    bytes = up.rd.len(),
                    name = name
                )
            }
        };
        self.log(LogDir::Out, &log_line);
        let sink = self.sink.clone();
        let mut started = false;
        let result = self
            .session
            .send_job(&up.rd, &up.mode, &self.shared.cancel, &mut |p| {
                if !started {
                    started = true;
                    sink.emit(Emit::Job(JobEvent {
                        state: JobState::Started,
                        message: tr!("desktop.lhymicro.job_sending", packets = p.packets),
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
                    &tr!(
                        "desktop.ruida.log_uploaded",
                        packets = report.packets,
                        bytes = report.bytes,
                        resends = report.resends
                    ),
                );
                match up.mode {
                    SendMode::Store { name } => self.job_event(
                        JobState::Finished,
                        &tr!("desktop.ruida.job_stored", name = name),
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
                        self.log(LogDir::In, &tr!("desktop.ruida.log_running"));
                    }
                }
                Ok(())
            }
            Err(e) => {
                let msg = e.to_string();
                if e.machine_may_be_running {
                    self.log(LogDir::In, &tr!("desktop.ruida.log_may_be_running"));
                }
                if matches!(e.cause, SessionError::Cancelled) {
                    self.job_event(
                        JobState::Cancelled,
                        &tr!("desktop.ruida.send_cancelled", err = msg),
                    );
                    Ok(())
                } else if !started {
                    // Nada llegó a salir (trabajo inválido, máquina ocupada, sin respuesta).
                    Err(op_error_text(&e.cause))
                } else {
                    self.job_event(
                        JobState::Error,
                        &tr!("desktop.ruida.send_failed", err = msg),
                    );
                    Ok(())
                }
            }
        }
    }
}
