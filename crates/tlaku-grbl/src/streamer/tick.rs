//! Una vuelta del ciclo de envío: sondeo, lectura, fin de trabajo y líneas auxiliares.

use std::io;
use std::time::{Duration, Instant};

use crate::link::{ControlAction, FinishDetection, FlowControl};
use crate::serial::Transport;

use super::*;

impl<T: Transport> Streamer<T> {
    /// Intervalo del sondeo `?` durante el trabajo: solo si el enlace consulta con un byte en
    /// tiempo real (GRBL). En Marlin y Smoothie no hay sondeo.
    fn polling(&self) -> Option<Duration> {
        if self.link.polls_during_job() {
            self.config.status_interval
        } else {
            None
        }
    }

    /// Una vuelta del ciclo: mandos, envío, sondeo, lectura y comprobaciones de fin.
    pub(super) fn tick(
        &mut self,
        job: &[Prepared],
        st: &mut RunState,
        emit: &mut dyn FnMut(Event),
    ) -> io::Result<Option<Outcome>> {
        while let Ok(cmd) = self.cmd_rx.try_recv() {
            if let Some(outcome) = self.handle_command(cmd, st, emit)? {
                return Ok(Some(outcome));
            }
        }

        // Tras un `Resend:` se espera el `ok` que lo acompaña (con un tope de tiempo).
        if st.resend_until.is_some_and(|t| Instant::now() >= t) {
            st.resend_until = None;
        }
        // Tras una línea que escribe en la EEPROM no se envía nada hasta su `ok`.
        let blocked = st
            .pending
            .back()
            .is_some_and(|&(i, _)| job.get(i).is_some_and(|p| p.sync));
        if !st.paused && !blocked && st.resend_until.is_none() && st.next < job.len() {
            let mut batch = Vec::new();
            while let Some(line) = job.get(st.next) {
                let wire = self.wire_text_for(st.next, &line.text);
                let len = wire.len() + 1;
                if line.sync && !st.pending.is_empty() {
                    break;
                }
                let fits = match self.link.flow {
                    FlowControl::CharCounting { rx_buffer } => st.used + len <= rx_buffer,
                    FlowControl::Window { max_lines, .. } => st.pending.len() < max_lines.max(1),
                };
                if !fits {
                    break;
                }
                batch.extend_from_slice(wire.as_bytes());
                batch.push(b'\n');
                st.pending.push_back((st.next, len));
                st.used += len;
                st.next += 1;
                if line.sync {
                    break;
                }
            }
            if !batch.is_empty() {
                self.transport.write_all(&batch)?;
            }
        }

        let polling = self.polling();
        if let Some(interval) = polling
            && st.last_poll.is_none_or(|t| t.elapsed() >= interval)
        {
            self.transport.write_all(&[Realtime::StatusQuery.byte()])?;
            st.last_poll = Some(Instant::now());
        }

        let mut buf = [0u8; 256];
        let n = self.transport.read_timeout(&mut buf, TICK)?;
        if n > 0 {
            st.last_rx = Instant::now();
            self.reader.feed(&buf[..n]);
        }
        while let Some(line) = self.reader.next_line() {
            if let Some(outcome) = self.handle_line(&line, job, st, emit)? {
                return Ok(Some(outcome));
            }
        }

        if let Some(outcome) = self.check_finished(job, st, polling.is_some())? {
            return Ok(Some(outcome));
        }
        // `?` no se contesta durante el homing: con una línea `$H` sin responder se espera
        // `home_timeout` en lugar de `silence_timeout`.
        let homing = st
            .pending
            .iter()
            .any(|&(i, _)| job.get(i).is_some_and(|p| is_homing_line(&p.text)));
        let allowance = if homing {
            self.config.home_timeout.max(self.config.silence_timeout)
        } else {
            self.config.silence_timeout
        };
        if polling.is_some() && st.last_rx.elapsed() > allowance {
            return Ok(Some(Outcome::Disconnected(
                "la controladora dejó de responder".into(),
            )));
        }
        Ok(None)
    }

    /// ¿Terminó el trabajo? GRBL: todas las líneas respondidas y, con sondeo, estado `Idle`.
    /// Marlin y Smoothie: todas respondidas y el `ok` de `M400` (los `ok` de los movimientos
    /// llegan al encolarlos, no al ejecutarlos).
    fn check_finished(
        &mut self,
        job: &[Prepared],
        st: &mut RunState,
        polling: bool,
    ) -> io::Result<Option<Outcome>> {
        if st.acked != job.len() {
            return Ok(None);
        }
        match &self.link.finish {
            FinishDetection::StatusIdle => {
                if st.idle_seen || !(self.config.wait_idle && polling) {
                    return Ok(Some(Outcome::Completed { errors: st.errors }));
                }
            }
            FinishDetection::SyncCommand(cmd) => {
                if !self.config.wait_idle
                    || job.last().is_some_and(|l| l.text.eq_ignore_ascii_case(cmd))
                {
                    // O no se pide esperar el fin, o la última línea ya es la sincronización.
                    return Ok(Some(Outcome::Completed { errors: st.errors }));
                }
                if !st.sync_sent {
                    let cmd = cmd.clone();
                    self.send_extra(&cmd, st)?;
                    st.sync_sent = true;
                } else if st.pending.is_empty() {
                    return Ok(Some(Outcome::Completed { errors: st.errors }));
                }
            }
        }
        Ok(None)
    }

    /// Envía una línea que no es del trabajo; su `ok` solo libera el búfer.
    fn send_extra(&mut self, line: &str, st: &mut RunState) -> io::Result<()> {
        let len = line.len() + 1;
        self.transport.write_all(format!("{line}\n").as_bytes())?;
        st.pending.push_back((EXTRA, len));
        st.used += len;
        Ok(())
    }

    /// Ejecuta pausa o reanudación del enlace: bytes en tiempo real, líneas auxiliares o
    /// "dejar de enviar" (que ya hace `st.paused`).
    pub(super) fn run_actions(
        &mut self,
        actions: &[ControlAction],
        st: &mut RunState,
    ) -> io::Result<()> {
        for a in actions {
            match a {
                ControlAction::Realtime(b) => self.transport.write_all(&[*b])?,
                ControlAction::Line(l) => self.send_extra(l, st)?,
                ControlAction::HostSide => {}
            }
        }
        Ok(())
    }
}
