//! Estado del trabajador: reportes `?`, silencio y lectura del puerto.

use super::*;

impl Worker {
    pub(super) fn on_status(&mut self, r: &streamer::StatusReport) {
        self.statuses += 1;
        if let Some(w) = &r.wco {
            self.wco = [w.x, w.y];
        }
        let mut s = self.last.clone();
        s.state = convert_state(r.state);
        if s.state == MachineState::Alarm {
            self.shared.homed.store(false, Ordering::SeqCst);
        }
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
        self.learn_space(s.state, s.machine_pos);
        self.last = s.clone();
        if let Ok(mut g) = self.shared.status.lock() {
            *g = Some(s.clone());
        }
        self.sink.emit(Emit::Status(s));
    }

    /// Aviso temprano a la interfaz: la placa no contesta desde hace `silence_notice` (2 s). Se
    /// repite cada segundo entero y se retira (`silence_ms: 0`) al volver a contestar. No cambia
    /// la conexión (nunca se pierde por silencio). No avisa donde el silencio es lo esperado (homing,
    /// bloqueo tras una alarma) ni sin `?` (Marlin, Smoothieware).
    pub(super) fn report_silence(&mut self) {
        let notice = self.timing.silence_notice;
        let elapsed = self.last_rx.elapsed();
        let watch = self.link_family == LinkFamily::Grbl && self.quiet.is_none();
        if watch && !notice.is_zero() && elapsed >= notice {
            if self.silence_shown != Some(elapsed.as_secs()) {
                self.emit_silence(u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX));
            }
        } else if self.silence_shown.is_some() {
            self.emit_silence(0);
        }
    }

    /// Emite `machine://silence` y recuerda lo avisado (`ms: 0` = volvió a contestar).
    pub(super) fn emit_silence(&mut self, ms: u64) {
        self.silence_shown = (ms > 0).then_some(ms / 1000);
        self.sink.emit(Emit::Silence(SilenceEvent {
            silence_ms: u32::try_from(ms).unwrap_or(u32::MAX),
        }));
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
        }
        // Sin respuesta no pasa nada más que el aviso: la conexión solo la pierde un error real de
        // E/S del sistema (arriba, con `?`) o que el usuario desconecte.
        self.report_silence();
        let mut others = Vec::new();
        while let Some(line) = self.reader.next_line() {
            match streamer::StatusReport::parse(&line) {
                Some(r) => self.on_status(&r),
                None => {
                    self.note_line(&line);
                    self.detect_reboot(&line);
                    others.push(line);
                }
            }
        }
        Ok(others)
    }
}
