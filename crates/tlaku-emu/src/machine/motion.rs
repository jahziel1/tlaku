//! Avance del tiempo simulado, cinemática trapezoidal, láser y límites.

use std::time::Duration;

use crate::fault::Trigger;
use crate::model::Block;
use crate::profile::Family;
use crate::trace::{LaserSegment, MoveKind, SpindleMode};

use super::*;

impl Machine {
    // -----------------------------------------------------------------------
    // Avance del tiempo
    // -----------------------------------------------------------------------

    pub(super) fn homing_lock(&self) -> bool {
        self.setting(22) >= 1.0
    }

    fn quiescent(&self) -> bool {
        self.current.is_none()
            && self.planner.is_empty()
            && self.pending.is_none()
            && self.homing.is_none()
            && self.door_resume_at.is_none()
            && self.hold != Some(false)
            && self.door != Some(2)
            && !self.jog_cancel
            && self.v == 0.0
            && self.laser_sample().is_none()
    }

    fn next_event(&self) -> Option<Duration> {
        let mut t: Option<Duration> = None;
        let mut take = |x: Duration| t = Some(t.map_or(x, |c| c.min(x)));
        for f in &self.faults {
            if let Trigger::At(at) = f.trigger {
                take(at);
            }
        }
        if let Some((until, _)) = self.silence {
            take(until);
        }
        t
    }

    /// Hace avanzar la simulación hasta el instante `target` (nunca retrocede).
    pub fn advance_to(&mut self, target: Duration) {
        while self.now < target {
            if self.quiescent() {
                let horizon = self
                    .next_event()
                    .filter(|&e| e > self.now)
                    .map_or(target, |e| e.min(target));
                self.now = horizon;
                self.fire_due_faults();
                self.flush_silence();
                continue;
            }
            let step = (target - self.now).min(self.opts.step.max(Duration::from_micros(100)));
            self.now += step;
            self.tick(step.as_secs_f64());
        }
    }

    fn tick(&mut self, dt: f64) {
        self.fire_due_faults();
        self.flush_silence();
        self.tick_door_hold();
        self.tick_homing();
        let prev = self.pos;
        self.step_motion(dt);
        self.update_laser(prev, dt);
        if self.block_done {
            self.block_done = false;
            self.current = None;
            if self.planner.is_empty() {
                self.v = 0.0;
            }
        }
        self.pump();
    }

    fn flush_silence(&mut self) {
        if let Some((until, _)) = self.silence
            && self.now >= until
        {
            self.silence = None;
            let held = std::mem::take(&mut self.held);
            self.out.extend(held);
        }
    }

    fn tick_door_hold(&mut self) {
        if self.hold == Some(false) && self.v <= 1e-9 {
            self.hold = Some(true);
        }
        if self.door == Some(2) && self.v <= 1e-9 {
            self.door = Some(if self.door_open { 1 } else { 0 });
        }
        if let Some(t) = self.door_resume_at
            && self.now >= t
        {
            self.door_resume_at = None;
            self.door = None;
        }
    }

    fn tick_homing(&mut self) {
        let Some(h) = &self.homing else { return };
        let total = h.end.saturating_sub(h.start).as_secs_f64().max(1e-9);
        let f = (self.now.saturating_sub(h.start).as_secs_f64() / total).clamp(0.0, 1.0);
        let from = h.from;
        let done = self.now >= h.end;
        for (p, f0) in self.pos.iter_mut().zip(from) {
            *p = f0 * (1.0 - f);
        }
        self.v = self.setting(25).max(1.0) / 60.0;
        if done {
            self.homing = None;
            self.pos = [0.0; 3];
            self.v = 0.0;
            self.gc.position = self.pos;
            self.alarm = None;
            self.homed = true;
            self.position_lost = false;
            self.limit_lock = false;
        }
    }

    // -----------------------------------------------------------------------
    // Movimiento
    // -----------------------------------------------------------------------

    fn accel(&self) -> f64 {
        self.setting(120).min(self.setting(121)).max(10.0)
    }

    fn max_rate_mm_s(&self) -> f64 {
        let r = self.setting(110).min(self.setting(111));
        (if r > 0.0 { r } else { 6000.0 }) / 60.0
    }

    fn cruise_speed(&self, b: &Block) -> f64 {
        let max = self.max_rate_mm_s();
        let v = match b.kind {
            // Marlin: G0 y G1 comparten `F`.
            MoveKind::Rapid if b.feed > 0.0 => {
                (b.feed / 60.0).min(max) * f64::from(self.ovr_rapid) / 100.0
            }
            MoveKind::Rapid => max * f64::from(self.ovr_rapid) / 100.0,
            MoveKind::Jog => (b.feed / 60.0).min(max),
            _ => ((b.feed / 60.0) * f64::from(self.ovr_feed) / 100.0).min(max),
        };
        v.max(0.1)
    }

    fn junction_speed(&self, cur: &Block, next: &Block) -> f64 {
        let c = self.cruise_speed(cur).min(self.cruise_speed(next));
        let (a, b) = (cur.dir(), next.dir());
        let cos = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        if cos >= 0.999 {
            return c;
        }
        (c * cos.max(0.0).powi(8)).max(MIN_JUNCTION).min(c)
    }

    fn stopping(&self) -> bool {
        self.hold.is_some() || self.door.is_some() || self.jog_cancel || self.tool_pending
    }

    fn step_motion(&mut self, dt: f64) {
        if self.homing.is_some() {
            return;
        }
        if self.alarm.is_some() || self.check || self.sleeping {
            self.v = 0.0;
            return;
        }
        if self.current.is_none() {
            if self.stopping() {
                self.v = 0.0;
                if self.jog_cancel {
                    self.finish_jog_cancel();
                }
                return;
            }
            match self.planner.pop_front() {
                Some(b) => {
                    // Con el láser en un ventilador, lo manda `M106`/`M107`, no el bloque.
                    if self.profile.laser_fan.is_none() {
                        self.exec_spindle = (b.spindle, b.s);
                    }
                    self.block_seq += 1;
                    self.current = Some(Exec {
                        block: b,
                        done: 0.0,
                    });
                }
                None => {
                    self.v = 0.0;
                    return;
                }
            }
        }
        let stopping = self.stopping();
        let accel = self.accel();
        let (block, done) = {
            let e = self.current.as_ref().expect("bloque en curso");
            (e.block.clone(), e.done)
        };
        let cruise = self.cruise_speed(&block);
        let remaining = (block.len - done).max(0.0);
        let end_speed = if stopping {
            0.0
        } else {
            self.planner
                .front()
                .map_or(0.0, |n| self.junction_speed(&block, n))
        };
        let limit = (end_speed * end_speed + 2.0 * accel * remaining).sqrt();
        let target = if stopping { 0.0 } else { cruise.min(limit) };
        let dv = accel * dt;
        if self.v < target {
            self.v = (self.v + dv).min(target);
        } else {
            self.v = (self.v - dv).max(target);
        }
        let eff = if stopping {
            self.v
        } else {
            self.v.max(MIN_SPEED)
        };
        let new_done = done + eff * dt;
        if new_done >= block.len - 1e-9 {
            // El bloque se retira tras contabilizar el láser de este último paso.
            self.pos = block.to;
            self.block_done = true;
        } else {
            let f = new_done / block.len;
            for i in 0..3 {
                self.pos[i] = block.from[i] + (block.to[i] - block.from[i]) * f;
            }
            if let Some(e) = self.current.as_mut() {
                e.done = new_done;
            }
        }
        if self.jog_cancel && self.v <= 1e-9 {
            self.finish_jog_cancel();
        }
    }

    fn finish_jog_cancel(&mut self) {
        self.current = None;
        self.planner.clear();
        self.v = 0.0;
        self.jog_cancel = false;
        self.gc.position = self.pos;
        // Un jog bloqueado esperando sitio queda cancelado también.
        if self.pending.is_some() {
            self.pending = None;
            self.reply(Ok(()));
        }
    }

    pub(crate) fn laser_mode(&self) -> bool {
        match self.profile.family {
            // Con el láser en el ventilador, G0 no lo apaga: se comporta como `$32=0`.
            Family::Marlin if self.profile.laser_fan.is_some() => false,
            Family::Marlin | Family::Smoothie => true,
            _ => (self.setting(32) - 1.0).abs() < f64::EPSILON,
        }
    }

    /// ¿Puede quemar con la cabeza parada y el husillo en `M3`? Marlin en modo inline y
    /// Smoothie solo disparan durante un movimiento `G1`.
    fn fires_while_stationary(&self) -> bool {
        match self.profile.family {
            Family::Marlin => !self.inline,
            Family::Smoothie => false,
            _ => true,
        }
    }

    /// Potencia efectiva, tipo de movimiento y modo si el láser está encendido.
    pub(super) fn laser_sample(&self) -> Option<(f64, MoveKind, SpindleMode, f64)> {
        if self.check
            || self.alarm.is_some()
            || self.sleeping
            || self.homing.is_some()
            || self.door.is_some()
            || self.halted
        {
            return None;
        }
        let (mode, s) = self.exec_spindle;
        if mode == SpindleMode::Off || s <= 0.0 || self.spindle_stop_ovr {
            return None;
        }
        let smax = self.setting(30);
        let mut eff = s * f64::from(self.ovr_spindle) / 100.0;
        if smax > 0.0 {
            eff = eff.min(smax);
        }
        if !self.laser_mode() {
            let kind = self
                .current
                .as_ref()
                .map_or(MoveKind::Stationary, |e| e.block.kind);
            return Some((eff, kind, mode, s));
        }
        match &self.current {
            Some(e) => match e.block.kind {
                MoveKind::Rapid | MoveKind::Jog | MoveKind::Stationary => None,
                MoveKind::Feed => match mode {
                    SpindleMode::Cw if self.hold.is_some() && self.profile.laser_off_in_hold => {
                        None
                    }
                    SpindleMode::Cw => Some((eff, MoveKind::Feed, mode, s)),
                    _ => {
                        if self.v <= 1e-9 {
                            return None;
                        }
                        let nominal = self.cruise_speed(&e.block);
                        let ratio = (self.v / nominal).clamp(0.0, 1.0);
                        Some((eff * ratio, MoveKind::Feed, mode, s))
                    }
                },
            },
            None => match mode {
                SpindleMode::Cw
                    if !(self.hold.is_some() && self.profile.laser_off_in_hold)
                        && self.fires_while_stationary() =>
                {
                    Some((eff, MoveKind::Stationary, mode, s))
                }
                _ => None,
            },
        }
    }

    fn update_laser(&mut self, prev: [f64; 3], dt: f64) {
        let sample = self.laser_sample();
        let Some((power, kind, mode, s)) = sample else {
            self.open_seg = None;
            return;
        };
        let in_block = self.current.is_some();
        let key = (self.block_seq, in_block);
        let extend = matches!(self.open_seg, Some((_, seq, ib)) if (seq, ib) == key);
        if extend && let Some((idx, _, _)) = self.open_seg {
            let (now, pos) = (self.now, self.pos);
            if let Some(seg) = self.trace.last_mut(idx) {
                seg.end = pos;
                seg.t1 = now;
                seg.energy += power * dt;
                seg.s_peak = seg.s_peak.max(power);
            }
            return;
        }
        let line = self.current.as_ref().map_or(0, |e| e.block.line);
        let idx = self.trace.push(LaserSegment {
            start: prev,
            end: self.pos,
            t0: self.now.saturating_sub(Duration::from_secs_f64(dt)),
            t1: self.now,
            kind,
            mode,
            s_programmed: s,
            s_peak: power,
            energy: power * dt,
            line,
        });
        self.open_seg = Some((idx, key.0, key.1));
    }

    pub(super) fn quiet(&self) -> bool {
        self.planner.is_empty() && self.current.is_none() && self.v <= 1e-9
    }

    pub(crate) fn soft_limits_on(&self) -> bool {
        self.setting(20) >= 1.0
    }

    pub(crate) fn outside_travel(&self, p: [f64; 3]) -> bool {
        if !self.soft_limits_on() {
            return false;
        }
        (0..3).any(|i| {
            let t = self.setting(130 + i as u32);
            if t <= 0.0 {
                return false;
            }
            let positive = (self.setting(23) as u32 >> i) & 1 == 1;
            let (lo, hi) = if positive { (0.0, t) } else { (-t, 0.0) };
            p[i] < lo - 1e-6 || p[i] > hi + 1e-6
        })
    }
}
