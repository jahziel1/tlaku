//! Comandos en tiempo real, informe de estado, alarmas y reinicios.

use std::time::Duration;

use crate::model::{Action, Gc, StateKind};
use crate::profile::{Protocol, format_value};
use crate::trace::SpindleMode;

use super::*;

impl Machine {
    // -----------------------------------------------------------------------
    // Tiempo real
    // -----------------------------------------------------------------------

    pub(super) fn realtime(&mut self, b: u8) {
        self.log.push(LogEntry {
            at: self.now,
            what: Received::Realtime(b),
        });
        if self.profile.protocol == Protocol::PingPong {
            if b == b'?' {
                self.report_smoothie();
            }
            return;
        }
        let ext = self.profile.extended_realtime;
        let b = match b {
            0x80 if ext => b'?',
            0x81 if ext => b'~',
            0x82 if ext => b'!',
            x => x,
        };
        match b {
            b'?' => self.report_status(false),
            0x87 if ext => self.report_status(true),
            0x83 if ext => {
                let l = format!("[GC:{}]", self.parser_state());
                self.emit_line(&l);
            }
            b'!' => self.feed_hold(),
            b'~' => self.cycle_start(),
            0x18 => self.do_reset(ResetKind::FromHost),
            0x84 => self.safety_door(),
            0x85 => self.jog_cancel_cmd(),
            0x90 => self.ovr_feed = 100,
            0x91 => self.ovr_feed = (self.ovr_feed + 10).min(200),
            0x92 => self.ovr_feed = self.ovr_feed.saturating_sub(10).max(10),
            0x93 => self.ovr_feed = (self.ovr_feed + 1).min(200),
            0x94 => self.ovr_feed = self.ovr_feed.saturating_sub(1).max(10),
            0x95 => self.ovr_rapid = 100,
            0x96 => self.ovr_rapid = 50,
            0x97 => self.ovr_rapid = 25,
            0x99 => self.ovr_spindle = 100,
            0x9A => self.ovr_spindle = (self.ovr_spindle + 10).min(200),
            0x9B => self.ovr_spindle = self.ovr_spindle.saturating_sub(10).max(10),
            0x9C => self.ovr_spindle = (self.ovr_spindle + 1).min(200),
            0x9D => self.ovr_spindle = self.ovr_spindle.saturating_sub(1).max(10),
            0x9E => {
                if self.hold == Some(true) {
                    self.spindle_stop_ovr = !self.spindle_stop_ovr;
                }
            }
            0xA0 => {
                if matches!(
                    self.state(),
                    StateKind::Idle | StateKind::Run | StateKind::Hold(_)
                ) {
                    self.gc.flood = !self.gc.flood;
                }
            }
            0xA1 => {
                if self.profile.m7_enabled
                    && matches!(
                        self.state(),
                        StateKind::Idle | StateKind::Run | StateKind::Hold(_)
                    )
                {
                    self.gc.mist = !self.gc.mist;
                }
            }
            _ => {}
        }
        if (0x90..=0xA1).contains(&b) {
            self.rep.ov_counter = 0;
        }
    }

    fn feed_hold(&mut self) {
        match self.state() {
            StateKind::Idle | StateKind::Run => {
                if self.hold.is_none() {
                    self.hold = Some(self.v <= 1e-9);
                }
            }
            StateKind::Jog => self.jog_cancel_cmd(),
            _ => {}
        }
    }

    pub(super) fn cycle_start(&mut self) {
        if self.door == Some(0) {
            self.door = Some(3);
            self.door_resume_at = Some(self.now + Duration::from_millis(100));
            return;
        }
        if self.door.is_some() {
            return;
        }
        if self.hold == Some(true) {
            self.hold = None;
            self.spindle_stop_ovr = false;
            self.resolve_awaiting();
        } else if self.tool_pending {
            self.tool_pending = false;
            self.resolve_awaiting();
        }
    }

    fn resolve_awaiting(&mut self) {
        if let Some(p) = self.pending.as_mut()
            && p.awaiting_resume
        {
            p.awaiting_resume = false;
            p.actions.pop_front();
        }
        self.tool_pending = false;
    }

    pub(super) fn safety_door(&mut self) {
        if matches!(
            self.state(),
            StateKind::Idle
                | StateKind::Run
                | StateKind::Hold(_)
                | StateKind::Home
                | StateKind::Jog
        ) {
            self.door_open = true;
            self.door = Some(2);
            self.emit_line("[MSG:Check Door]");
        }
    }

    fn jog_cancel_cmd(&mut self) {
        if self.state() == StateKind::Jog {
            self.jog_cancel = true;
            self.planner_clear();
        }
    }

    fn report_status(&mut self, full: bool) {
        if self.limit_lock {
            return;
        }
        if self.homing.is_some() && !self.profile.status_during_home {
            return;
        }
        let mut counters = self.rep;
        let line = self.status_with(&mut counters, full, false);
        self.rep = counters;
        self.emit_line(&line);
    }

    pub(super) fn status_with(&self, counters: &mut ReportState, full: bool, peek: bool) -> String {
        let st = self.state();
        let mask = self.setting(10) as u32;
        let hal = self.profile.hal_status_mask;
        let show_bf = mask & 2 != 0;
        let show_ln = hal && mask & 4 != 0;
        let show_fs = !hal || mask & 8 != 0;
        let show_pn = !hal || mask & 16 != 0;
        let show_wco = !hal || mask & 32 != 0;
        let show_ov = if hal {
            mask & 64 != 0
        } else {
            self.profile.overrides_in_status
        };
        let mpos = mask & 1 != 0;
        let inches = self.setting(13) >= 1.0;
        let (div, dec) = if inches { (25.4, 4) } else { (1.0, 3) };
        let wco = self.wco();
        let p = if mpos {
            self.pos
        } else {
            [
                self.pos[0] - wco[0],
                self.pos[1] - wco[1],
                self.pos[2] - wco[2],
            ]
        };
        let coords = |v: [f64; 3]| {
            format!(
                "{},{},{}",
                format_value(v[0] / div, dec),
                format_value(v[1] / div, dec),
                format_value(v[2] / div, dec)
            )
        };
        let mut s = String::from("<");
        s.push_str(st.name());
        match st {
            StateKind::Hold(n) | StateKind::Door(n) => s.push_str(&format!(":{n}")),
            StateKind::Alarm(c) if self.profile.alarm_substate && c > 0 => {
                s.push_str(&format!(":{c}"));
            }
            _ => {}
        }
        s.push_str(&format!(
            "|{}:{}",
            if mpos { "MPos" } else { "WPos" },
            coords(p)
        ));
        if show_bf {
            let planner_free = self
                .profile
                .planner_blocks
                .saturating_sub(self.planner_used());
            let rx_free = self.rx_capacity().saturating_sub(self.rx.len());
            s.push_str(&format!("|Bf:{planner_free},{rx_free}"));
        }
        if show_ln && let Some(n) = self.current.as_ref().and_then(|e| e.block.n) {
            s.push_str(&format!("|Ln:{n}"));
        }
        if show_fs {
            let feed = (self.v * 60.0).round() as i64;
            if self.profile.fs_three_values {
                s.push_str(&format!(
                    "|FS:{feed},{},{}",
                    self.exec_spindle.1.round() as i64,
                    self.current_power().round() as i64
                ));
            } else {
                s.push_str(&format!(
                    "|FS:{feed},{}",
                    self.current_power().round() as i64
                ));
            }
        }
        if show_pn {
            let mut pins = String::new();
            if self.profile.has_limit_switches && self.homed {
                if self.pos[0].abs() < 0.5 {
                    pins.push('X');
                }
                if self.pos[1].abs() < 0.5 {
                    pins.push('Y');
                }
            }
            if self.probe_pin() {
                pins.push('P');
            }
            if self.door_open {
                pins.push('D');
            }
            if !pins.is_empty() {
                s.push_str(&format!("|Pn:{pins}"));
            }
        }
        let busy = st.is_moving();
        if show_wco && (full || (!peek && counters.wco_counter == 0)) {
            let w = if self.nan == Some(true) {
                "-nan,0.000,0.000".to_string()
            } else {
                coords(wco)
            };
            s.push_str(&format!("|WCO:{w}"));
            if !peek {
                counters.wco_counter = if busy { 10 } else { 30 };
            }
        } else if !peek {
            counters.wco_counter = counters.wco_counter.saturating_sub(1);
        }
        if show_ov && (full || (!peek && counters.ov_counter == 0)) {
            s.push_str(&format!(
                "|Ov:{},{},{}",
                self.ovr_feed, self.ovr_rapid, self.ovr_spindle
            ));
            let mut acc = String::new();
            match self.exec_spindle.0 {
                SpindleMode::Cw if !self.spindle_stop_ovr => acc.push('S'),
                SpindleMode::Ccw if !self.spindle_stop_ovr => acc.push('C'),
                _ => {}
            }
            if self.gc.flood {
                acc.push('F');
            }
            if self.gc.mist {
                acc.push('M');
            }
            if !acc.is_empty() {
                s.push_str(&format!("|A:{acc}"));
            }
            if !peek {
                counters.ov_counter = if busy { 20 } else { 10 };
            }
        } else if !peek {
            counters.ov_counter = counters.ov_counter.saturating_sub(1);
        }
        if self.profile.extra_status_fields {
            s.push_str(&format!(
                "|WCS:G{}|H:{}",
                54 + self.gc.wcs,
                u8::from(self.homed)
            ));
        }
        s.push('>');
        s
    }

    // -----------------------------------------------------------------------
    // Alarmas y reinicios
    // -----------------------------------------------------------------------

    pub(super) fn stop_motion(&mut self) {
        self.planner_clear();
        self.current = None;
        self.v = 0.0;
        self.hold = None;
        self.jog_cancel = false;
        self.homing = None;
        self.pending = None;
        self.exec_spindle = (SpindleMode::Off, self.exec_spindle.1);
        self.gc.position = self.pos;
    }

    pub(super) fn raise_alarm(&mut self, code: u32) {
        self.stop_motion();
        self.alarm = Some(code);
        self.line_ctx.bytes = 0;
        self.emit_line(&format!("ALARM:{code}"));
        if matches!(code, 1 | 2) {
            self.emit_line("[MSG:Reset to continue]");
        }
        if code == 1 {
            self.limit_lock = true;
        }
    }

    pub(super) fn do_reset(&mut self, kind: ResetKind) {
        let st = self.state();
        let homing = self.homing.is_some();
        let moving = match kind {
            ResetKind::FromHost => st.is_moving(),
            ResetKind::Motion => true,
            ResetKind::Quiet => false,
        };
        let was_sleeping = self.sleeping;
        self.rx.clear();
        self.line_buf.clear();
        self.line_bytes = 0;
        self.in_flight = 0;
        self.line_ctx.bytes = 0;
        self.pending = None;
        self.planner_clear();
        self.current = None;
        self.v = 0.0;
        self.hold = None;
        self.jog_cancel = false;
        self.homing = None;
        self.tool_pending = false;
        self.halted = false;
        self.limit_lock = false;
        self.last_error = None;
        self.next_reply = None;
        self.sleeping = false;
        self.check = false;
        self.exec_spindle = (SpindleMode::Off, 0.0);
        self.spindle_stop_ovr = false;
        self.ovr_feed = 100;
        self.ovr_rapid = 100;
        self.ovr_spindle = 100;
        self.g92 = [0.0; 3];
        self.gc = Gc::new(self.pos);
        self.door = self.door_open.then_some(1);
        self.door_resume_at = None;
        self.rep = ReportState::default();
        self.open_seg = None;
        self.resets += 1;
        self.fans.clear();
        self.inline = false;
        self.last_line = 0;
        self.corrupt_next = false;
        self.ep_buf.clear();
        let mut alarm = None;
        if homing && kind == ResetKind::FromHost {
            alarm = Some(6);
        } else if moving {
            alarm = Some(3);
        }
        if let Some(c) = alarm {
            self.position_lost = true;
            self.homed = false;
            self.emit_line(&format!("ALARM:{c}"));
        }
        let banner = self.profile.banner_reset.clone();
        for l in banner {
            self.emit_line(&l);
        }
        self.alarm = alarm.or_else(|| (self.homing_lock() || was_sleeping).then_some(0));
        if self.alarm.is_some() {
            if self.profile.unlock_message {
                self.emit_line("[MSG:'$H'|'$X' to unlock]");
            }
        } else {
            self.run_startup();
        }
    }

    fn run_startup(&mut self) {
        for i in 0..2 {
            let line = self.startup[i].clone();
            if line.is_empty() {
                continue;
            }
            match self.parse_gcode(&line) {
                Ok(parsed) => {
                    self.gc = parsed.gc;
                    for a in parsed.actions {
                        match a {
                            Action::Move(b) => self.planner_push(*b),
                            Action::ApplySpindle(m, s) => self.exec_spindle = (m, s),
                            _ => {}
                        }
                    }
                    self.emit_line(&format!(">{line}:ok"));
                }
                Err(c) => self.emit_line(&format!(">{line}:error:{c}")),
            }
        }
    }
}
