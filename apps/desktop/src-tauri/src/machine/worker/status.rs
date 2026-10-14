//! Estado del trabajador: reportes `?`, silencio y lectura del puerto.

use super::*;
use tlaku_i18n::tr;

impl Worker {
    pub(super) fn on_status(&mut self, r: &streamer::StatusReport) {
        if let Some(w) = &r.wco {
            self.wco = [w.x, w.y];
        }
        let mut s = self.last.clone();
        s.state = convert_state(r.state);
        s.other_state = r
            .other_state
            .clone()
            .filter(|_| s.state == MachineState::Other);
        match (&r.mpos, &r.wpos) {
            (Some(m), _) => {
                s.machine_pos = [m.x, m.y];
                s.work_pos = match &r.wpos {
                    Some(w) => [w.x, w.y],
                    None => [m.x - self.wco[0], m.y - self.wco[1]],
                };
            }
            (None, Some(w)) => {
                s.work_pos = [w.x, w.y];
                s.machine_pos = [w.x + self.wco[0], w.y + self.wco[1]];
            }
            (None, None) => {}
        }
        s.feed = r.feed.unwrap_or(0.0);
        s.spindle = r.spindle.unwrap_or(0.0);
        if let Some(b) = r.buffer {
            s.buffer = BufferStatus {
                planner: b.planner_blocks,
                rx: b.rx_bytes,
            };
        }
        // GRBL manda `Ov:` solo de vez en cuando: se conserva el último valor visto.
        if let Some(o) = r.overrides {
            s.overrides = Some(Overrides {
                feed: o.feed,
                rapid: o.rapid,
                spindle: o.spindle,
            });
        }
        self.last = s.clone();
        if let Ok(mut g) = self.shared.status.lock() {
            *g = Some(s.clone());
        }
        self.sink.emit(Emit::Status(s));
    }

    /// ¿Se superó el silencio tolerado? Ver la regla en la cabecera del módulo.
    pub(super) fn silence_exceeded(&self) -> bool {
        // Sin `?` no hay sondeo que contestar: la pérdida se detecta con `M114` y con los
        // errores de E/S.
        if self.link_family != LinkFamily::Grbl {
            return false;
        }
        let elapsed = self.last_rx.elapsed();
        match self.quiet {
            None => elapsed > self.timing.silence_timeout,
            Some(Quiet::Homing) => {
                elapsed > self.timing.silence_timeout.max(self.timing.home_timeout)
            }
            Some(Quiet::CriticalAlarm) => false,
        }
    }

    /// Actualiza el silencio tolerado y la familia del firmware con una línea recibida.
    pub(super) fn note_line(&mut self, line: &str) {
        if is_critical_alarm(line) {
            self.quiet = Some(Quiet::CriticalAlarm);
        } else if self.quiet == Some(Quiet::CriticalAlarm) && (line == "ok" || is_greeting(line)) {
            self.quiet = None;
        }
        if let Some(fw) = Firmware::from_banner(line) {
            self.fw = fw;
        }
    }

    /// Una vuelta de lectura: sondea `?` si toca, lee hasta `wait` y devuelve las líneas que no
    /// son reportes de estado (esos se emiten y no se registran).
    pub(super) fn pump(
        &mut self,
        t: &mut dyn Transport,
        wait: Duration,
    ) -> Result<Vec<String>, Fail> {
        let interval = if self.quiet == Some(Quiet::CriticalAlarm) {
            self.timing.status_interval.max(CRITICAL_POLL)
        } else {
            self.timing.status_interval
        };
        if self.link_family == LinkFamily::Grbl
            && self.last_poll.is_none_or(|p| p.elapsed() >= interval)
        {
            t.write_all(&[Realtime::StatusQuery.byte()])?;
            self.last_poll = Some(Instant::now());
        }
        let mut buf = [0u8; 256];
        let n = t.read_timeout(&mut buf, wait)?;
        if n > 0 {
            self.last_rx = Instant::now();
            self.reader.feed(&buf[..n]);
        } else if self.silence_exceeded() {
            return Err(Fail::Lost(tr!("desktop.worker.controller_silent")));
        }
        let mut others = Vec::new();
        while let Some(line) = self.reader.next_line() {
            match streamer::StatusReport::parse(&line) {
                Some(r) => self.on_status(&r),
                None => {
                    self.note_line(&line);
                    others.push(line);
                }
            }
        }
        Ok(others)
    }
}
