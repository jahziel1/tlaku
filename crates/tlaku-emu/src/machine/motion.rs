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
                self.carry = 0.0;
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

    /// Límite por eje (`ids` = ajustes X, Y, Z) proyectado sobre la dirección `dir`: el eje que
    /// primero llega a su tope manda, como en GRBL (en diagonal se puede ir más deprisa que el
    /// tope de un eje). Un ajuste no positivo usa `fallback`.
    fn along(&self, ids: [u32; 3], dir: [f64; 3], fallback: f64) -> f64 {
        let mut best = f64::INFINITY;
        for (id, u) in ids.into_iter().zip(dir) {
            let u = u.abs();
            if u > 1e-9 {
                let v = self.setting(id);
                best = best.min(if v > 0.0 { v } else { fallback } / u);
            }
        }
        if best.is_finite() { best } else { fallback }
    }

    /// Aceleración (mm/s²) al moverse en la dirección `dir`.
    fn accel_along(&self, dir: [f64; 3]) -> f64 {
        self.along([120, 121, 122], dir, 10.0).max(10.0)
    }

    /// Velocidad máxima (mm/s) al moverse en la dirección `dir`.
    fn max_rate_along(&self, dir: [f64; 3]) -> f64 {
        self.along([110, 111, 112], dir, 6000.0) / 60.0
    }

    fn cruise_speed(&self, b: &Block) -> f64 {
        let max = self.max_rate_along(b.dir());
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

    /// Velocidad máxima (mm/s) con la que se puede pasar de `cur` a `next` sin detenerse, por
    /// desviación de unión (`$11`).
    ///
    /// Es el modelo que describe la documentación pública de GRBL ("Improving grbl: cornering
    /// algorithm", S. Jeon, wiki de gnea/grbl y la descripción de `$11` en su `settings.md`):
    /// la esquina se sustituye por un arco tangente a ambos tramos que se aparta del vértice
    /// como mucho `δ = $11`. Con `θ` el ángulo entre el tramo anterior invertido y el siguiente
    /// (180 grados = recta, 0 = media vuelta), el radio del arco es `R = δ·sin(θ/2) /
    /// (1 - sin(θ/2))` y la velocidad `v = sqrt(a·R)`, con `a` la aceleración. Una recta no
    /// limita (solo las velocidades de los bloques) y una media vuelta obliga a detenerse. Está
    /// escrito desde esa fórmula, no desde el código de GRBL.
    fn junction_speed(&self, cur: &Block, next: &Block) -> f64 {
        let cap = self.cruise_speed(cur).min(self.cruise_speed(next));
        if cur.len <= 1e-9 || next.len <= 1e-9 {
            return cap;
        }
        let (a, b) = (cur.dir(), next.dir());
        // Coseno de θ: -1 en una recta, +1 en una media vuelta.
        let cos_theta = -(a[0] * b[0] + a[1] * b[1] + a[2] * b[2]);
        if cos_theta <= -0.999_999 {
            return cap;
        }
        if cos_theta >= 0.999_999 {
            return 0.0;
        }
        let sin_half = (0.5 * (1.0 - cos_theta)).max(0.0).sqrt();
        let accel = self.accel_along(a).min(self.accel_along(b));
        let radius = self.setting(11).max(0.0) * sin_half / (1.0 - sin_half);
        (accel * radius).sqrt().min(cap)
    }

    /// Velocidad (mm/s) con la que el bloque en curso debe llegar a su final: la que deja el
    /// planificador con los bloques del búfer (ver [`Profile::lookahead_blocks`]). Se recorre
    /// de atrás adelante: tras el último bloque de la ventana la cabeza debe poder detenerse, y
    /// cada bloque entra como mucho a la velocidad de su unión y a la desde la que aún puede
    /// frenar hasta la entrada del siguiente. El resultado se guarda mientras el búfer no
    /// cambie.
    fn planned_exit_speed(&mut self, cur: &Block) -> f64 {
        let key = (
            self.plan_gen,
            self.ovr_feed,
            self.ovr_rapid,
            [11, 110, 111, 112, 120, 121, 122].map(|id| self.setting(id).to_bits()),
        );
        if let Some((k, v)) = self.plan_cache
            && k == key
        {
            return v;
        }
        let window = self
            .profile
            .lookahead_blocks
            .unwrap_or(self.profile.planner_blocks)
            .max(1);
        let mut entry = 0.0f64;
        for k in (0..self.planner.len().min(window - 1)).rev() {
            let prev = if k == 0 { cur } else { &self.planner[k - 1] };
            let blk = &self.planner[k];
            let reach = (entry * entry + 2.0 * self.accel_along(blk.dir()) * blk.len).sqrt();
            entry = self.junction_speed(prev, blk).min(reach);
        }
        self.plan_cache = Some((key, entry));
        entry
    }

    /// Encola un bloque en el planificador.
    pub(super) fn planner_push(&mut self, b: Block) {
        self.planner.push_back(b);
        self.plan_gen += 1;
    }

    /// Vacía el planificador.
    pub(super) fn planner_clear(&mut self) {
        self.planner.clear();
        self.plan_gen += 1;
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
                    self.plan_gen += 1;
                    // Con el láser en un ventilador, lo manda `M106`/`M107`, no el bloque.
                    if self.profile.laser_fan.is_none() {
                        self.exec_spindle = (b.spindle, b.s);
                    }
                    self.block_seq += 1;
                    // Lo que el bloque anterior recorrió de más en el último paso cuenta aquí.
                    let done = std::mem::take(&mut self.carry).min(b.len);
                    self.current = Some(Exec { block: b, done });
                }
                None => {
                    self.v = 0.0;
                    return;
                }
            }
        }
        let stopping = self.stopping();
        let (block, done) = {
            let e = self.current.as_ref().expect("bloque en curso");
            (e.block.clone(), e.done)
        };
        let accel = self.accel_along(block.dir());
        let cruise = self.cruise_speed(&block);
        let remaining = (block.len - done).max(0.0);
        let end_speed = if stopping {
            0.0
        } else {
            self.planned_exit_speed(&block)
        };
        let dv = accel * dt;
        let v0 = self.v;
        // Mayor velocidad al final del paso desde la que aún se frena hasta `end_speed` en lo
        // que queda del bloque: con recorrido medio `(v0 + v1) dt / 2` es la raíz de
        // `v1^2 + a dt v1 + a dt v0 - end^2 - 2 a remaining = 0`.
        let disc = dv * dv - 4.0 * (dv * v0 - end_speed * end_speed - 2.0 * accel * remaining);
        let limit = if disc > 0.0 {
            ((disc.sqrt() - dv) / 2.0).max(end_speed.min(v0))
        } else {
            end_speed.min(v0)
        };
        let target = if stopping { 0.0 } else { cruise.min(limit) };
        if self.v < target {
            self.v = (self.v + dv).min(target);
        } else {
            self.v = (self.v - dv).max(target);
        }
        // Con aceleración constante el recorrido del paso es la velocidad media.
        let step = 0.5 * (v0 + self.v) * dt;
        let step = if stopping {
            step
        } else {
            step.max(MIN_SPEED * dt)
        };
        let new_done = done + step;
        if new_done >= block.len - 1e-9 {
            // El bloque se retira tras contabilizar el láser de este último paso.
            self.pos = block.to;
            self.block_done = true;
            self.carry = (new_done - block.len).max(0.0);
            // La rampa discreta llega al final con algo de velocidad de más: la unión se pasa a
            // la velocidad que planificó el planificador (0 en una media vuelta).
            self.v = self.v.min(end_speed);
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
        self.planner_clear();
        self.v = 0.0;
        self.carry = 0.0;
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
