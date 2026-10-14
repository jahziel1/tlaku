//! Comandos `$` (ajustes, jog, homing, `$#`, `$G`) de GRBL.

use std::collections::VecDeque;

use crate::model::{Action, MotionMode, StateKind};
use crate::trace::SpindleMode;

use super::*;

impl Machine {
    // -----------------------------------------------------------------------
    // Comandos `$`
    // -----------------------------------------------------------------------

    pub(super) fn reply_ok_with(&mut self, lines: &[String]) {
        for l in lines {
            self.emit_line(l);
        }
        self.reply(Ok(()));
    }

    pub(super) fn exec_dollar(&mut self, raw: &str, text: &str) {
        let body = &text[1..];
        if body.is_empty() {
            let help = "[HLP:$$ $# $G $I $N $x=val $Nx=line $J=line $C $X $H ~ ! ? ctrl-x]";
            self.reply_ok_with(&[help.to_string()]);
            return;
        }
        if body.starts_with('J') {
            self.exec_jog(text);
            return;
        }
        if !matches!(
            self.state(),
            StateKind::Idle | StateKind::Alarm(_) | StateKind::Check
        ) {
            self.reply(Err(8));
            return;
        }
        match body {
            "$" => {
                let lines: Vec<String> = self
                    .settings
                    .values()
                    .map(|s| format!("${}={}", s.id, s.format()))
                    .collect();
                self.reply_ok_with(&lines);
            }
            "#" => {
                let lines = self.parameter_lines();
                self.reply_ok_with(&lines);
            }
            "G" => {
                let l = format!("[GC:{}]", self.parser_state());
                self.reply_ok_with(&[l]);
            }
            "I" => {
                let mut lines = self.profile.info.clone();
                if !self.user_info.is_empty() && !lines.is_empty() {
                    let first = lines[0].trim_end_matches(":]").to_string();
                    lines[0] = format!("{first}:{}]", self.user_info);
                }
                self.reply_ok_with(&lines);
            }
            "N" => {
                let lines = vec![
                    format!("$N0={}", self.startup[0]),
                    format!("$N1={}", self.startup[1]),
                ];
                self.reply_ok_with(&lines);
            }
            "C" => self.toggle_check(),
            "X" => {
                if self.alarm.is_some() && !self.halted {
                    self.alarm = None;
                    self.emit_line("[MSG:Caution: Unlocked]");
                }
                self.reply(Ok(()));
            }
            "H" => self.start_homing(),
            "SLP" => {
                self.stop_motion();
                self.sleeping = true;
                self.emit_line("[MSG:Sleeping]");
                self.reply(Ok(()));
            }
            _ if body.starts_with("RST=") => self.restore(&body[4..]),
            _ if body.starts_with("I=") => {
                // Cadena de usuario: se conserva tal como llegó.
                let v = raw.split_once('=').map_or("", |x| x.1).trim();
                if v.len() >= 80 {
                    self.reply(Err(14));
                } else {
                    self.user_info = v.to_string();
                    self.reply(Ok(()));
                }
            }
            _ if body.starts_with('N') && body.contains('=') => {
                let (k, v) = body.split_once('=').unwrap_or_default();
                match k {
                    "N0" | "N1" => {
                        let i = usize::from(k == "N1");
                        // Se prueba la línea antes de guardarla, como hace GRBL.
                        match self.parse_gcode(v) {
                            Ok(_) => {
                                self.startup[i] = v.to_string();
                                self.reply(Ok(()));
                            }
                            Err(c) => self.reply(Err(c)),
                        }
                    }
                    _ => self.reply(Err(3)),
                }
            }
            _ => match body.split_once('=') {
                Some((k, v)) if k.bytes().all(|b| b.is_ascii_digit()) && !k.is_empty() => {
                    let r = k
                        .parse::<u32>()
                        .map_err(|_| 3u32)
                        .and_then(|id| self.write_setting(id, v));
                    self.reply(r);
                }
                _ => self.reply(Err(3)),
            },
        }
    }

    fn write_setting(&mut self, id: u32, text: &str) -> Result<(), u32> {
        if !self.settings.contains_key(&id) {
            return Err(3);
        }
        let v: f64 = text.parse().map_err(|_| 2u32)?;
        if !v.is_finite() {
            return Err(2);
        }
        if v < 0.0 {
            return Err(4);
        }
        match id {
            0 if v < 3.0 => return Err(6),
            20 if v >= 1.0 && self.setting(22) < 1.0 => return Err(10),
            _ => {}
        }
        self.set_setting(id, v);
        Ok(())
    }

    fn restore(&mut self, what: &str) {
        match what {
            "$" | "#" | "*" => {
                if what != "#" {
                    self.settings = self
                        .profile
                        .settings
                        .iter()
                        .map(|s| (s.id, s.clone()))
                        .collect();
                }
                if what != "$" {
                    self.wcs = [[0.0; 3]; 6];
                    self.g28 = [0.0; 3];
                    self.g30 = [0.0; 3];
                }
                if what == "*" {
                    self.user_info.clear();
                    self.startup = [String::new(), String::new()];
                }
                self.emit_line("[MSG:Restoring defaults]");
                self.reply(Ok(()));
                self.do_reset(ResetKind::Quiet);
            }
            _ => self.reply(Err(3)),
        }
    }

    fn exec_jog(&mut self, text: &str) {
        if self.alarm.is_some() {
            self.reply(Err(9));
            return;
        }
        if !matches!(self.state(), StateKind::Idle | StateKind::Jog) {
            self.reply(Err(8));
            return;
        }
        let Some(rest) = text.strip_prefix("$J=") else {
            self.reply(Err(16));
            return;
        };
        match self.parse_jog(rest) {
            Err(c) => self.reply(Err(c)),
            Ok(b) => {
                self.gc.position = b.to;
                self.pending = Some(Pending {
                    actions: VecDeque::from([Action::Move(Box::new(b))]),
                    dwell_until: None,
                    awaiting_resume: false,
                    homing: false,
                });
            }
        }
    }

    fn toggle_check(&mut self) {
        if self.check {
            self.check = false;
            self.emit_line("[MSG:Disabled]");
            self.reply(Ok(()));
            self.do_reset(ResetKind::Quiet);
        } else {
            self.check = true;
            self.emit_line("[MSG:Enabled]");
            self.reply(Ok(()));
        }
    }

    fn start_homing(&mut self) {
        if self.setting(22) < 1.0 {
            self.reply(Err(5));
            return;
        }
        let end = self.now + self.profile.homing_duration;
        self.alarm = None;
        self.homing = Some(Homing {
            start: self.now,
            end,
            from: self.pos,
        });
        self.pending = Some(Pending {
            actions: VecDeque::new(),
            dwell_until: None,
            awaiting_resume: false,
            homing: true,
        });
    }

    fn parameter_lines(&self) -> Vec<String> {
        let f3 = |v: [f64; 3]| format!("{},{},{}", fmt3(v[0]), fmt3(v[1]), fmt3(v[2]));
        let mut lines = Vec::new();
        for (i, o) in self.wcs.iter().enumerate() {
            if i == 0 && self.nan.is_some() {
                lines.push("[G54:-nan,1.579,1.619]".to_string());
            } else {
                lines.push(format!("[G{}:{}]", 54 + i, f3(*o)));
            }
        }
        lines.push(format!("[G28:{}]", f3(self.g28)));
        lines.push(format!("[G30:{}]", f3(self.g30)));
        lines.push(format!("[G92:{}]", f3(self.g92)));
        lines.push("[TLO:0.000]".to_string());
        let (probe, hit) = self.probe_last;
        lines.push(format!("[PRB:{}:{}]", f3(probe), u8::from(hit)));
        lines
    }

    pub(super) fn parser_state(&self) -> String {
        let g = &self.gc;
        let motion = match g.motion {
            MotionMode::Rapid => "G0",
            MotionMode::Linear => "G1",
            MotionMode::ArcCw => "G2",
            MotionMode::ArcCcw => "G3",
            MotionMode::None => "G80",
            MotionMode::Probe(3) => "G38.3",
            MotionMode::Probe(4) => "G38.4",
            MotionMode::Probe(5) => "G38.5",
            MotionMode::Probe(_) => "G38.2",
        };
        let spindle = match g.spindle {
            SpindleMode::Off => "M5",
            SpindleMode::Cw => "M3",
            SpindleMode::Ccw => "M4",
        };
        let coolant = match (g.mist, g.flood) {
            (false, false) => "M9".to_string(),
            (true, false) => "M7".to_string(),
            (false, true) => "M8".to_string(),
            (true, true) => "M7 M8".to_string(),
        };
        let feed = if g.feed == 0.0 {
            "0.0".to_string()
        } else {
            fmt_trim(g.feed)
        };
        format!(
            "{motion} G{} G{} G{} G{} G{} {spindle} {coolant} T{} F{feed} S{}",
            54 + g.wcs,
            g.plane,
            if g.inches { 20 } else { 21 },
            if g.absolute { 90 } else { 91 },
            if g.inverse_time { 93 } else { 94 },
            g.tool,
            fmt_trim(g.s),
        )
    }
}
