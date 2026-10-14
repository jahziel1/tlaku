//! Comandos de consola, tiempo real y el intercambio línea/`ok`.

use super::*;

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
                    Err(Fail::Msg(
                        "Esta controladora no tiene comandos en tiempo real: pausa y cancela \
                         desde el trabajo en curso."
                            .into(),
                    ))
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
            RealtimeCommand::Hold => (Realtime::FeedHold, "! (pausa, feed hold)"),
            RealtimeCommand::Resume => (Realtime::CycleStart, "~ (reanudar, cycle start)"),
            RealtimeCommand::Reset => (Realtime::SoftReset, "Ctrl+X (reinicio, soft reset)"),
            RealtimeCommand::JogCancel => (Realtime::JogCancel, "0x85 (cancelar movimiento)"),
        };
        self.log(LogDir::Out, text);
        t.write_all(&[rt.byte()])?;
        // Un reset en pleno movimiento (o homing) pierde la posición: GRBL lanzará ALARM:3.
        if rt == Realtime::SoftReset
            && matches!(
                self.last.state,
                MachineState::Run | MachineState::Jog | MachineState::Home
            )
        {
            self.log(LogDir::In, &format!("; {POSITION_LOST_NOTE}"));
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
            Fail::Msg(format!(
                "'{raw}' contiene {ch:?}, que GRBL interpretaría como comando en tiempo real. \
                 Para pausar, reanudar o reiniciar usa los botones de la pestaña Máquina."
            ))
        })?;
        if line.is_empty() {
            return Err(Fail::Msg("La línea está vacía.".into()));
        }
        let max = streamer::StreamerConfig::default().rx_buffer;
        if line.len() + 1 > max {
            return Err(Fail::Msg(format!(
                "'{line}' no cabe en el búfer de la controladora ({max} bytes)."
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
                return Err(Fail::Msg(format!(
                    "La máquina no respondió a '{line}' en {:.0} s.",
                    timeout.as_secs_f64()
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
