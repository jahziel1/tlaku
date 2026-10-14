//! Comandos de consola, tiempo real y el intercambio línea/`ok`.

use super::*;
use tlaku_i18n::tr;

impl Worker {
    pub(super) fn realtime(
        &mut self,
        t: &mut dyn Transport,
        cmd: RealtimeCommand,
    ) -> Result<(), Fail> {
        if self.link_family != LinkFamily::Grbl {
            // Marlin y Smoothieware no tienen bytes en tiempo real: pausar y cancelar son cosas
            // del anfitrión durante un trabajo (ver `stream_request`). Fuera de él no hay nada
            // que detener.
            return match cmd {
                RealtimeCommand::Status | RealtimeCommand::JogCancel => Ok(()),
                RealtimeCommand::Hold | RealtimeCommand::Resume | RealtimeCommand::Reset => {
                    Err(Fail::Msg(tr!("desktop.worker.no_realtime")))
                }
            };
        }
        let (rt, text) = match cmd {
            RealtimeCommand::Status => {
                // El reporte llega por el flujo normal de estado.
                t.write_all(&[Realtime::StatusQuery.byte()])?;
                self.last_poll = Some(Instant::now());
                return Ok(());
            }
            RealtimeCommand::Hold => (Realtime::FeedHold, tr!("desktop.worker.log_hold")),
            RealtimeCommand::Resume => (Realtime::CycleStart, tr!("desktop.worker.log_resume")),
            RealtimeCommand::Reset => (Realtime::SoftReset, tr!("desktop.worker.log_reset")),
            RealtimeCommand::JogCancel => {
                (Realtime::JogCancel, tr!("desktop.worker.log_jog_cancel"))
            }
        };
        self.log(LogDir::Out, &text);
        t.write_all(&[rt.byte()])?;
        // Un reset en pleno movimiento (o homing) pierde la posición: GRBL lanzará ALARM:3.
        if rt == Realtime::SoftReset
            && matches!(
                self.last.state,
                MachineState::Run | MachineState::Jog | MachineState::Home
            )
        {
            self.log(LogDir::In, &format!("; {}", position_lost_note()));
        }
        Ok(())
    }

    /// Envía una línea y espera `ok`, `error:N` o `ALARM:N`. Devuelve todas las líneas de
    /// respuesta, la última incluida (los errores con su explicación).
    pub(super) fn command(
        &mut self,
        t: &mut dyn Transport,
        raw: &str,
    ) -> Result<Vec<String>, Fail> {
        // GRBL ignora los espacios y se quitan para ahorrar búfer; Marlin y Smoothie los
        // necesitan para separar parámetros (por ejemplo `M3 S0 I`).
        let grbl = self.link_family == LinkFamily::Grbl;
        let line = prepare_line_with(raw, grbl, grbl).map_err(|ch| {
            Fail::Msg(tr!(
                "desktop.worker.realtime_char",
                raw = raw,
                ch = format!("{ch:?}")
            ))
        })?;
        if line.is_empty() {
            return Err(Fail::Msg(tr!("desktop.worker.line_empty")));
        }
        let max = streamer::StreamerConfig::default().rx_buffer;
        if line.len() + 1 > max {
            return Err(Fail::Msg(tr!(
                "desktop.worker.line_too_long",
                line = line,
                max = max
            )));
        }
        let timeout = if line.eq_ignore_ascii_case("$H") {
            self.timing.home_timeout
        } else {
            self.timing.command_timeout
        };
        self.exchange(t, &line, timeout)
    }

    pub(super) fn exchange(
        &mut self,
        t: &mut dyn Transport,
        line: &str,
        timeout: Duration,
    ) -> Result<Vec<String>, Fail> {
        // Durante el homing `?` puede no contestarse: se tolera el silencio hasta `home_timeout`.
        if is_homing_line(line) && self.quiet.is_none() {
            self.quiet = Some(Quiet::Homing);
        }
        let r = self.exchange_inner(t, line, timeout);
        if self.quiet == Some(Quiet::Homing) {
            self.quiet = None;
        }
        r
    }

    pub(super) fn exchange_inner(
        &mut self,
        t: &mut dyn Transport,
        line: &str,
        timeout: Duration,
    ) -> Result<Vec<String>, Fail> {
        self.log(LogDir::Out, line);
        self.reader.clear();
        t.write_all(format!("{line}\n").as_bytes())?;
        let deadline = Instant::now() + timeout;
        let mut out = Vec::new();
        loop {
            let lines = self.pump(t, self.timing.tick)?;
            self.urgent_requests(t)?;
            for l in lines {
                let shown = describe_line(self.fw, &l);
                self.log(LogDir::In, &shown);
                if is_ack(self.link_family, &l) {
                    out.push(l);
                    return Ok(out);
                }
                if l.starts_with("error") || l.starts_with("ALARM") {
                    out.push(shown);
                    return Ok(out);
                }
                out.push(l);
            }
            if Instant::now() > deadline {
                return Err(Fail::Msg(tr!(
                    "desktop.worker.no_reply",
                    line = line,
                    secs = format!("{:.0}", timeout.as_secs_f64())
                )));
            }
        }
    }

    /// Durante una espera larga (p. ej. `$H`) atiende al instante pausa, reinicio y
    /// cancelación de jog; lo demás espera su turno.
    pub(super) fn urgent_requests(&mut self, t: &mut dyn Transport) -> Result<(), Fail> {
        while let Ok(req) = self.rx.try_recv() {
            match req {
                Request::Realtime {
                    cmd:
                        cmd @ (RealtimeCommand::Hold
                        | RealtimeCommand::Reset
                        | RealtimeCommand::JogCancel
                        | RealtimeCommand::Status),
                    reply,
                } => {
                    let r = self.realtime(t, cmd);
                    let _ = reply.send(r.as_ref().map(|()| vec![]).map_err(|e| match e {
                        Fail::Msg(m) | Fail::Lost(m) => m.clone(),
                    }));
                    r?;
                }
                other => self.pending.push_back(other),
            }
        }
        Ok(())
    }
}
