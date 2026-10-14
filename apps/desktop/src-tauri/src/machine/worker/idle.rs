//! Reposo: el bucle de solicitudes con el puerto en manos del trabajador.

use super::*;

impl Worker {
    pub(super) fn idle_step(&mut self, mut t: Box<dyn Transport>) -> Result<Phase, Fail> {
        while let Some(req) = self.next_request()? {
            match req {
                Request::Disconnect => return Ok(Phase::Done),
                Request::Start {
                    lines,
                    link,
                    s_max,
                    kind,
                    reply,
                } => match self.try_start(t, lines, *link, s_max, kind) {
                    Ok(run) => {
                        let _ = reply.send(Ok(Vec::new()));
                        return Ok(Phase::Streaming(run));
                    }
                    Err(StartFail::Refused(back, msg)) => {
                        let _ = reply.send(Err(msg));
                        t = back;
                    }
                    Err(StartFail::Lost(msg)) => {
                        let _ = reply.send(Err(msg.clone()));
                        return Err(Fail::Lost(msg));
                    }
                },
                other => self.simple_request(&mut *t, other)?,
            }
        }
        if self.link_family != LinkFamily::Grbl
            && self
                .last_poll
                .is_none_or(|p| p.elapsed() >= REPRAP_POLL.max(self.timing.status_interval))
        {
            self.poll_reprap(&mut *t)?;
        }
        for line in self.pump(&mut *t, self.timing.tick)? {
            self.log(LogDir::In, &describe_line(self.fw, &line));
        }
        Ok(Phase::Idle(t))
    }

    /// Marlin y Smoothieware no tienen `?`: la posición se pide con `M114` (sin registrar la
    /// consulta) y se informa como estado en reposo. Si no contesta en el plazo largo
    /// (`home_timeout`), la conexión se da por perdida.
    pub(super) fn poll_reprap(&mut self, t: &mut dyn Transport) -> Result<(), Fail> {
        self.reader.clear();
        t.write_all(b"M114\n")?;
        self.last_poll = Some(Instant::now());
        // Marlin contesta `M114` en su turno, detrás de los movimientos que aún tenga en cola
        // (p. ej. tras cancelar un trabajo largo): el plazo es el largo, como en el homing.
        let deadline = Instant::now() + self.timing.silence_timeout.max(self.timing.home_timeout);
        let mut pos = None;
        loop {
            for l in self.pump(t, self.timing.tick)? {
                if is_ack(self.link_family, &l) {
                    if let Some(p) = pos {
                        self.report_position(p);
                    }
                    return Ok(());
                }
                match parse_m114(&l) {
                    Some(p) => pos = Some(p),
                    None => self.log(LogDir::In, &l),
                }
            }
            if Instant::now() > deadline {
                return Err(Fail::Lost(
                    "la controladora dejó de responder (¿se desconectó el cable USB o la red, o se \
                     apagó la máquina?)"
                        .into(),
                ));
            }
        }
    }

    /// Estado de una controladora sin `?`: la posición de `M114` y el estado que sabe el
    /// anfitrión (en marcha, en pausa o en reposo).
    pub(super) fn report_position(&mut self, p: [f64; 2]) {
        self.last.machine_pos = p;
        self.last.work_pos = p;
        self.publish_last();
    }

    /// Cambia el estado informado (solo para controladoras sin `?`) y lo emite.
    pub(super) fn set_reported_state(&mut self, state: MachineState) {
        if self.link_family != LinkFamily::Grbl {
            self.last.state = state;
            self.publish_last();
        }
    }

    pub(super) fn publish_last(&mut self) {
        let s = self.last.clone();
        if let Ok(mut g) = self.shared.status.lock() {
            *g = Some(s.clone());
        }
        self.sink.emit(Emit::Status(s));
    }

    /// Solicitudes que no cambian de fase. Responde siempre; solo devuelve `Err` si se perdió
    /// la conexión.
    pub(super) fn simple_request(
        &mut self,
        t: &mut dyn Transport,
        req: Request,
    ) -> Result<(), Fail> {
        let (reply, result) = match req {
            Request::Command { line, reply } => (reply, self.command(t, &line)),
            Request::Realtime { cmd, reply } => (reply, self.realtime(t, cmd).map(|()| vec![])),
            Request::Jog {
                dx,
                dy,
                speed,
                reply,
            } => (reply, self.jog(t, dx, dy, speed).map(|()| vec![])),
            Request::JogTo { x, y, speed, reply } => {
                (reply, self.jog_to(t, x, y, speed).map(|()| vec![]))
            }
            Request::Override {
                kind,
                percent,
                reply,
            } => (reply, self.override_idle(t, kind, percent).map(|()| vec![])),
            // Sin trabajo, pausar o reanudar no hace nada; cancelar apaga un jog en curso.
            Request::Pause(reply) | Request::Resume(reply) => (reply, Ok(vec![])),
            Request::Cancel(reply) => (
                reply,
                self.realtime(t, RealtimeCommand::JogCancel)
                    .map(|()| vec![]),
            ),
            Request::Start { .. } | Request::Disconnect => unreachable!("se atienden antes"),
        };
        match result {
            Ok(v) => {
                let _ = reply.send(Ok(v));
                Ok(())
            }
            Err(Fail::Msg(m)) => {
                let _ = reply.send(Err(m));
                Ok(())
            }
            Err(Fail::Lost(m)) => {
                let _ = reply.send(Err(m.clone()));
                Err(Fail::Lost(m))
            }
        }
    }
}
