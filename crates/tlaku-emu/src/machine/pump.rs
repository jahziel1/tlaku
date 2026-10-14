//! Bombeo de líneas: del RX al intérprete, ejecución de G-code y respuestas.

use std::time::Duration;

use crate::model::{Action, Effect, StateKind};
use crate::profile::{Family, Protocol};

use super::*;

impl Machine {
    // -----------------------------------------------------------------------
    // Bombeo de líneas
    // -----------------------------------------------------------------------

    /// Avanza lo que no depende del tiempo: acciones pendientes y líneas del RX.
    pub(super) fn pump(&mut self) {
        loop {
            let a = self.run_pending();
            let b = self.process_rx();
            if !a && !b {
                break;
            }
        }
    }

    fn process_rx(&mut self) -> bool {
        let mut progressed = false;
        while self.pending.is_none() {
            let Some(b) = self.rx.pop_front() else { break };
            progressed = true;
            self.line_bytes += 1;
            if b == b'\n' || b == b'\r' {
                let raw = String::from_utf8_lossy(&self.line_buf).trim().to_string();
                let overflow = self.line_buf.len() >= MAX_RAW_LINE;
                self.line_buf.clear();
                let bytes = std::mem::take(&mut self.line_bytes);
                self.exec_line(raw, bytes, overflow);
            } else if self.line_buf.len() < MAX_RAW_LINE {
                self.line_buf.push(b);
            }
        }
        progressed
    }

    pub(super) fn reply(&mut self, result: Result<(), u32>) {
        let ctx = self.line_ctx;
        self.in_flight = self.in_flight.saturating_sub(ctx.bytes);
        self.line_ctx.bytes = 0;
        match (ctx.reply, result) {
            (Reply::Drop, _) => {}
            (Reply::Error(c), _) => self.emit_line(&format!("error:{c}")),
            (Reply::Normal, Ok(())) => self.emit_line("ok"),
            (Reply::Normal, Err(c)) => self.emit_line(&format!("error:{c}")),
        }
    }

    fn exec_line(&mut self, raw: String, bytes: usize, overflow: bool) {
        self.line_ctx = LineCtx {
            bytes,
            reply: Reply::Normal,
        };
        if self.halted && self.profile.family == Family::Smoothie && clean_line(&raw) == "M999" {
            // Smoothie sale del paro con M999; Marlin tras `M112` solo con un reinicio.
            self.log.push(LogEntry {
                at: self.now,
                what: Received::Line(raw),
            });
            self.halted = false;
            self.alarm = None;
            self.reply(Ok(()));
            return;
        }
        if self.halted {
            // Detenida por una falla de Creality: no contesta líneas hasta el reset.
            self.log.push(LogEntry {
                at: self.now,
                what: Received::Line(raw),
            });
            return;
        }
        let text = clean_line(&raw);
        self.log.push(LogEntry {
            at: self.now,
            what: Received::Line(raw.clone()),
        });
        if text.is_empty() {
            self.last_error = None;
            self.reply(Ok(()));
            return;
        }
        self.lines_seen += 1;
        self.fire_line_faults();
        if let Some(r) = self.next_reply.take() {
            self.line_ctx.reply = r;
            if let Reply::Error(_) = r {
                self.reply(Ok(()));
                return;
            }
        }
        if self.halted {
            return;
        }
        if overflow || self.profile.max_line_len.is_some_and(|m| text.len() > m) {
            self.reply(Err(11));
            return;
        }
        if self.profile.protocol == Protocol::PingPong {
            self.exec_pingpong(&raw, &text);
        } else if text.starts_with('$') {
            self.last_error = None;
            self.exec_dollar(&raw, &text);
        } else if let Some(c) = self.last_error.filter(|_| self.profile.persistent_error) {
            self.reply(Err(c));
        } else {
            self.exec_gcode(&text);
        }
    }

    fn exec_gcode(&mut self, text: &str) {
        if self.alarm.is_some() || self.sleeping || self.state() == StateKind::Jog {
            self.reply(Err(9));
            return;
        }
        match self.parse_gcode(text) {
            Err(c) => {
                if self.profile.persistent_error {
                    self.last_error = Some(c);
                }
                self.reply(Err(c));
            }
            Ok(parsed) => {
                let before = self.wco();
                self.gc = parsed.gc;
                for e in parsed.effects {
                    match e {
                        Effect::SetWcs(i, v) => self.wcs[i] = v,
                        Effect::SetG92(v) => self.g92 = v,
                        Effect::StoreG28(v) => self.g28 = v,
                        Effect::StoreG30(v) => self.g30 = v,
                    }
                }
                if self.wco() != before {
                    // El desplazamiento cambió: el próximo reporte lo incluye.
                    self.rep.wco_counter = 0;
                }
                if self.check {
                    self.reply(Ok(()));
                    return;
                }
                self.pending = Some(Pending {
                    actions: parsed.actions.into(),
                    dwell_until: None,
                    awaiting_resume: false,
                    homing: false,
                });
            }
        }
    }

    fn run_pending(&mut self) -> bool {
        let Some(mut p) = self.pending.take() else {
            return false;
        };
        let mut progressed = false;
        loop {
            if p.homing {
                if self.homing.is_some() {
                    break;
                }
                self.reply(Ok(()));
                return true;
            }
            let Some(front) = p.actions.front() else {
                self.reply(Ok(()));
                return true;
            };
            match front {
                Action::Sync => {
                    if !self.quiet() {
                        break;
                    }
                }
                Action::ApplySpindle(m, s) => {
                    let (m, s) = (*m, *s);
                    self.exec_spindle = (m, s);
                }
                Action::Move(_) => {
                    if self.planner_used() >= self.profile.planner_blocks.max(1) {
                        break;
                    }
                    if let Some(Action::Move(b)) = p.actions.pop_front() {
                        self.planner_push(*b);
                    }
                    progressed = true;
                    continue;
                }
                Action::Dwell(secs) => {
                    let secs = *secs;
                    let until = *p
                        .dwell_until
                        .get_or_insert(self.now + Duration::from_secs_f64(secs.max(0.0)));
                    if self.now < until {
                        break;
                    }
                    p.dwell_until = None;
                }
                Action::ProgramEnd => self.emit_line("[MSG:Pgm End]"),
                Action::Pause => {
                    if !p.awaiting_resume {
                        p.awaiting_resume = true;
                        if self.hold.is_none() {
                            self.hold = Some(true);
                        }
                    }
                    break;
                }
                Action::ToolChange => {
                    if !p.awaiting_resume {
                        p.awaiting_resume = true;
                        self.tool_pending = true;
                    }
                    break;
                }
                Action::SoftLimitAlarm => {
                    self.reply(Ok(()));
                    self.raise_alarm(2);
                    return true;
                }
            }
            p.actions.pop_front();
            progressed = true;
        }
        self.pending = Some(p);
        progressed
    }
}
