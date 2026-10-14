//! Sonda de contacto con `G38.3` (M48): tocar un borde con el cabezal y devolver dónde pasó.

use super::*;
use crate::machine::probe::{ProbeTouch, check_args};
use tlaku_i18n::tr;

/// `[PRB:x,y,z:s]` -> (x, y, éxito).
fn parse_prb(line: &str) -> Option<([f64; 2], bool)> {
    let body = line.strip_prefix("[PRB:")?.strip_suffix(']')?;
    let (pos, ok) = body.rsplit_once(':')?;
    let mut it = pos.split(',').map(|v| v.trim().parse::<f64>());
    let x = it.next()?.ok()?;
    let y = it.next()?.ok()?;
    Some(([x, y], ok.trim() == "1"))
}

impl Worker {
    /// Pide un reporte `?` nuevo y espera a que llegue: el último guardado puede ser anterior a
    /// un movimiento que acaba de terminar (o de empezar).
    fn refresh_status(&mut self, t: &mut dyn Transport) -> Result<(), Fail> {
        let seen = self.statuses;
        self.last_poll = None;
        let deadline = Instant::now() + self.timing.command_timeout;
        while self.statuses == seen {
            for l in self.pump(t, self.timing.tick)? {
                let shown = describe_line(self.fw, &l);
                self.log(LogDir::In, &shown);
            }
            if Instant::now() > deadline {
                return Err(Fail::Msg(tr!("desktop.worker.probe_no_status")));
            }
        }
        Ok(())
    }

    /// Un toque: láser apagado, `G91`, `G38.3` hacia `(dx, dy)` a `feed` mm/min y, si tocó,
    /// retrocede `retract` mm por el mismo camino. Siempre vuelve a `G90`.
    ///
    /// Se usa `G38.3` y no `G38.2` para que no tocar nada en el recorrido sea un resultado
    /// (`hit: false`) y no una alarma que bloquee la máquina; el recorrido y la velocidad están
    /// acotados aquí.
    pub(super) fn probe(
        &mut self,
        t: &mut dyn Transport,
        dx: f64,
        dy: f64,
        feed: f64,
        retract: f64,
    ) -> Result<ProbeTouch, Fail> {
        let travel = check_args(dx, dy, feed, retract).map_err(Fail::Msg)?;
        if self.link_family != LinkFamily::Grbl {
            return Err(Fail::Msg(tr!("desktop.worker.probe_grbl_only")));
        }
        self.refresh_status(t)?;
        if self.last.state != MachineState::Idle {
            return Err(Fail::Msg(tr!("desktop.worker.probe_not_idle")));
        }
        // El láser apagado antes de cualquier movimiento; `G38.x` nunca lo enciende, pero una
        // orden anterior de la consola podría haberlo dejado en `M3`.
        let off = self.exchange(t, "M5", self.timing.command_timeout)?;
        if let Some(e) = off.last().filter(|l| !is_ack(self.link_family, l)) {
            return Err(Fail::Msg(jog_error(e)));
        }
        let rel = self.exchange(t, "G91", self.timing.command_timeout)?;
        if let Some(e) = rel.last().filter(|l| !is_ack(self.link_family, l)) {
            return Err(Fail::Msg(jog_error(e)));
        }
        let line = format!("G38.3 X{} Y{} F{}", num(dx), num(dy), num(feed));
        let wait = Duration::from_secs_f64(travel / feed * 60.0) + self.timing.command_timeout;
        let probed = self.exchange(t, &line, wait);
        let result = match probed {
            Ok(reply) => self.finish_probe(t, &reply, dx, dy, feed, retract, travel),
            Err(e) => Err(e),
        };
        // Pase lo que pase, de vuelta a coordenadas absolutas.
        let back = self.exchange(t, "G90", self.timing.command_timeout);
        let touch = result?;
        back?;
        Ok(touch)
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_probe(
        &mut self,
        t: &mut dyn Transport,
        reply: &[String],
        dx: f64,
        dy: f64,
        feed: f64,
        retract: f64,
        travel: f64,
    ) -> Result<ProbeTouch, Fail> {
        if let Some(e) = reply.last().filter(|l| !is_ack(self.link_family, l)) {
            return Err(Fail::Msg(if e.starts_with("ALARM:4") {
                tr!("desktop.worker.probe_already_touching", reply = e)
            } else {
                jog_error(e)
            }));
        }
        let Some((machine, hit)) = reply.iter().find_map(|l| parse_prb(l)) else {
            // Sin `[PRB:...]`: si la controladora levantó una alarma después del `ok` es porque
            // la sonda ya estaba activa al empezar (`ALARM:4`).
            self.refresh_status(t)?;
            return Err(Fail::Msg(if self.last.state == MachineState::Alarm {
                tr!("desktop.worker.probe_already_touching", reply = "ALARM:4")
            } else {
                tr!("desktop.worker.probe_no_prb")
            }));
        };
        let touch = ProbeTouch {
            hit,
            machine_pos: machine,
            work_pos: [machine[0] - self.wco[0], machine[1] - self.wco[1]],
        };
        self.log(
            LogDir::In,
            &tr!(
                "desktop.worker.log_probe",
                result = if hit { "1" } else { "0" },
                x = num(touch.work_pos[0]),
                y = num(touch.work_pos[1])
            ),
        );
        if hit && retract > 0.0 {
            // Hacia atrás por el mismo camino, a la misma velocidad baja y sin pasar de lo
            // recorrido; el láser sigue apagado (`M5`).
            let k = retract.min(travel) / travel;
            let line = format!("G1 X{} Y{} F{}", num(-dx * k), num(-dy * k), num(feed));
            let back = self.exchange(t, &line, self.timing.command_timeout)?;
            if let Some(e) = back.last().filter(|l| !is_ack(self.link_family, l)) {
                return Err(Fail::Msg(jog_error(e)));
            }
        }
        Ok(touch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prb_lines_parse() {
        assert_eq!(
            parse_prb("[PRB:10.000,-2.500,0.000:1]"),
            Some(([10.0, -2.5], true))
        );
        assert_eq!(
            parse_prb("[PRB:0.000,0.000,0.000:0]"),
            Some(([0.0, 0.0], false))
        );
        assert_eq!(parse_prb("ok"), None);
    }
}
