//! Intérprete de G-code: de una línea ya limpia (mayúsculas, sin espacios ni comentarios) a las
//! acciones que ejecuta la controladora, o al `error:N` que GRBL daría.
//!
//! Subconjunto soportado (el resto da `error:20`):
//! `G0 G1 G2 G3` (arcos solo en el plano XY, se trocean en segmentos), `G4`, `G10 L2/L20`,
//! `G17-G19`, `G20/G21`, `G28/G28.1/G30/G30.1`, `G40`, `G43.1/G49`, `G53`, `G54-G59`, `G61`,
//! `G80`, `G90/G91`, `G91.1`, `G92/G92.1`, `G93/G94`; `M0 M1 M2 M30 M3 M4 M5 M7 M8 M9` (y `M6`
//! en grblHAL), `G38.2` a `G38.5` (sondas, solo GRBL); palabras `F I J K L N P R S T X Y Z`.
//! No se simulan ejes A/B/C ni compensaciones de herramienta.

use crate::machine::Machine;
use crate::model::{Action, Block, Effect, Gc, MotionMode, Parsed};
use crate::profile::Protocol;
use crate::trace::{MoveKind, SpindleMode};

/// Palabra de G-code: letra y valor.
struct Word {
    letter: u8,
    value: f64,
}

fn tokenize(text: &str) -> Result<Vec<Word>, u32> {
    let b = text.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let letter = b[i];
        if !letter.is_ascii_uppercase() {
            // Se esperaba la letra de una palabra.
            return Err(1);
        }
        i += 1;
        let start = i;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let mut digits = 0;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            digits += 1;
        }
        if i < b.len() && b[i] == b'.' {
            i += 1;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
                digits += 1;
            }
        }
        if digits == 0 {
            // Letra sin valor numérico.
            return Err(2);
        }
        let value: f64 = text[start..i].parse().map_err(|_| 2u32)?;
        out.push(Word { letter, value });
    }
    Ok(out)
}

/// Palabras `letra + valor` de una línea ya limpia (para las órdenes RepRap con parámetros).
pub(crate) fn parse_words(text: &str) -> Result<Vec<(u8, f64)>, u32> {
    Ok(tokenize(text)?
        .into_iter()
        .map(|w| (w.letter, w.value))
        .collect())
}

fn idx(letter: u8) -> usize {
    usize::from(letter - b'A')
}

/// Valor entero de un código G/M o `Err(23)` si no lo es.
fn code10(value: f64) -> Result<i32, u32> {
    let v10 = (value * 10.0).round();
    if (value * 10.0 - v10).abs() > 1e-4 || v10.abs() > 9999.0 {
        return Err(23);
    }
    Ok(v10 as i32)
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[derive(Default)]
struct Groups {
    nonmodal: Option<i32>,
    motion: Option<i32>,
    plane: Option<i32>,
    distance: Option<i32>,
    feed_mode: Option<i32>,
    units: Option<i32>,
    wcs: Option<i32>,
    tlo: Option<i32>,
    stop: Option<i32>,
    spindle: Option<i32>,
    coolant: Option<i32>,
    tool: bool,
}

fn put(slot: &mut Option<i32>, v: i32) -> Result<(), u32> {
    if slot.is_some() {
        // Dos comandos del mismo grupo modal.
        return Err(21);
    }
    *slot = Some(v);
    Ok(())
}

impl Machine {
    fn classify_g(&self, v10: i32, g: &mut Groups) -> Result<(), u32> {
        match v10 {
            40 | 100 | 280 | 281 | 300 | 301 | 530 | 920 | 921 => put(&mut g.nonmodal, v10),
            0 | 10 | 20 | 30 | 800 => put(&mut g.motion, v10),
            // Las sondas son de GRBL; Marlin y Smoothie no las tienen en este emulador.
            382..=385 if self.profile.protocol != Protocol::PingPong => put(&mut g.motion, v10),
            170 | 180 | 190 => put(&mut g.plane, v10),
            900 | 910 => put(&mut g.distance, v10),
            911 | 400 | 610 => Ok(()),
            930 | 940 => put(&mut g.feed_mode, v10),
            200 | 210 => put(&mut g.units, v10),
            431 | 490 => put(&mut g.tlo, v10),
            540 | 550 | 560 | 570 | 580 | 590 => put(&mut g.wcs, v10),
            _ => Err(20),
        }
    }

    fn classify_m(&self, v10: i32, g: &mut Groups) -> Result<(), u32> {
        if v10 % 10 != 0 {
            return Err(23);
        }
        match v10 / 10 {
            m @ (0 | 1 | 2 | 30) => put(&mut g.stop, m),
            m @ (3..=5) => put(&mut g.spindle, m),
            7 if !self.profile.m7_enabled => Err(20),
            m @ (7..=9) => put(&mut g.coolant, m),
            6 if self.profile.tool_change_state => {
                if g.tool {
                    return Err(21);
                }
                g.tool = true;
                Ok(())
            }
            _ => Err(20),
        }
    }

    /// Interpreta una línea de G-code. No modifica la máquina: devuelve el estado modal
    /// resultante y las acciones por ejecutar.
    pub(crate) fn parse_gcode(&self, text: &str) -> Result<Parsed, u32> {
        let words = tokenize(text)?;
        let old = self.gc.clone();
        let mut gc = old.clone();
        let mut g = Groups::default();
        let mut vals: [Option<f64>; 26] = [None; 26];
        for w in &words {
            match w.letter {
                b'G' => {
                    let v10 = code10(w.value)?;
                    if v10 % 10 != 0 && !matches!(v10, 281 | 301 | 382..=385 | 431 | 911 | 921) {
                        return Err(23);
                    }
                    self.classify_g(v10, &mut g)?;
                }
                b'M' => {
                    let v10 = code10(w.value)?;
                    self.classify_m(v10, &mut g)?;
                }
                b'F' | b'I' | b'J' | b'K' | b'L' | b'N' | b'P' | b'R' | b'S' | b'T' | b'X'
                | b'Y' | b'Z' => {
                    let slot = &mut vals[idx(w.letter)];
                    if slot.is_some() {
                        return Err(25);
                    }
                    if matches!(w.letter, b'F' | b'L' | b'N' | b'P' | b'S' | b'T') && w.value < 0.0
                    {
                        return Err(4);
                    }
                    if matches!(w.letter, b'L' | b'N' | b'T') && w.value.fract() != 0.0 {
                        return Err(23);
                    }
                    if w.letter == b'N' && w.value > 9_999_999.0 {
                        return Err(27);
                    }
                    *slot = Some(w.value);
                }
                _ => return Err(20),
            }
        }
        let val = |c: u8| vals[idx(c)];

        // Modos.
        if let Some(u) = g.units {
            gc.inches = u == 200;
        }
        if let Some(p) = g.plane {
            gc.plane = (p / 10) as u8;
        }
        if let Some(d) = g.distance {
            gc.absolute = d == 900;
        }
        if let Some(f) = g.feed_mode {
            gc.inverse_time = f == 930;
        }
        if let Some(w) = g.wcs {
            gc.wcs = ((w - 540) / 10) as usize;
        }
        let scale = if gc.inches { 25.4 } else { 1.0 };
        let inv_f = if gc.inverse_time { val(b'F') } else { None };
        if !gc.inverse_time
            && let Some(f) = val(b'F')
        {
            gc.feed = f * scale;
        }
        if let Some(s) = val(b'S') {
            gc.s = s;
        }
        if let Some(t) = val(b'T') {
            gc.tool = t as u32;
        }
        match g.spindle {
            Some(3) => gc.spindle = SpindleMode::Cw,
            Some(4) => gc.spindle = SpindleMode::Ccw,
            Some(5) => gc.spindle = SpindleMode::Off,
            _ => {}
        }
        match g.coolant {
            Some(7) => gc.mist = true,
            Some(8) => gc.flood = true,
            Some(9) => {
                gc.mist = false;
                gc.flood = false;
            }
            _ => {}
        }

        let axis = [val(b'X'), val(b'Y'), val(b'Z')];
        let has_axis = axis.iter().any(Option::is_some);
        let wco = self.wco_for(gc.wcs);
        let mut effects = Vec::new();
        let mut motion_actions: Vec<Action> = Vec::new();
        let mut used_p = false;
        let mut used_l = false;
        let mut used_ijkr = false;
        let mut axis_used = false;
        let mut dwell: Option<f64> = None;
        let mut moves = false;
        let mut soft_alarm = false;

        // Comando no modal.
        let mode = match g.motion {
            Some(0) => Some(MotionMode::Rapid),
            Some(10) => Some(MotionMode::Linear),
            Some(20) => Some(MotionMode::ArcCw),
            Some(30) => Some(MotionMode::ArcCcw),
            Some(800) => Some(MotionMode::None),
            Some(m @ 382..=385) => Some(MotionMode::Probe((m - 380) as u8)),
            _ => None,
        };
        let eff_mode = mode.unwrap_or(gc.motion);
        let mut motion_axis_cmd = true;
        match g.nonmodal {
            Some(40) => {
                let p = val(b'P').ok_or(28u32)?;
                used_p = true;
                dwell = Some(p);
            }
            Some(100) => {
                let l = val(b'L').ok_or(28u32)?;
                let p = val(b'P').ok_or(28u32)?;
                used_l = true;
                used_p = true;
                if !has_axis {
                    return Err(26);
                }
                if g.motion.is_some() {
                    return Err(24);
                }
                if !(0.0..=6.0).contains(&p) || p.fract() != 0.0 {
                    return Err(29);
                }
                let wi = if p == 0.0 { gc.wcs } else { p as usize - 1 };
                let mut off = self.wcs[wi];
                for i in 0..3 {
                    if let Some(v) = axis[i] {
                        off[i] = match l as i32 {
                            2 => v * scale,
                            20 => gc.position[i] - self.g92[i] - v * scale,
                            _ => return Err(20),
                        };
                    }
                }
                effects.push(Effect::SetWcs(wi, off));
                axis_used = true;
                motion_axis_cmd = false;
            }
            Some(280 | 300) => {
                let store_is_30 = g.nonmodal == Some(300);
                if g.motion.is_some() && has_axis {
                    return Err(24);
                }
                let dest = if store_is_30 { self.g30 } else { self.g28 };
                let mut at = gc.position;
                if has_axis {
                    for i in 0..3 {
                        if let Some(v) = axis[i] {
                            at[i] = self.axis_target(
                                gc.absolute,
                                false,
                                gc.position[i],
                                v * scale,
                                wco[i],
                            );
                        }
                    }
                    motion_actions.extend(self.rapid_blocks(gc.position, at, &gc));
                }
                motion_actions.extend(self.rapid_blocks(at, dest, &gc));
                gc.position = dest;
                axis_used = true;
                moves = true;
                motion_axis_cmd = false;
            }
            Some(281 | 301) => {
                if g.nonmodal == Some(301) {
                    effects.push(Effect::StoreG30(gc.position));
                } else {
                    effects.push(Effect::StoreG28(gc.position));
                }
                motion_axis_cmd = !has_axis;
            }
            Some(920) => {
                if !has_axis {
                    return Err(26);
                }
                if g.motion.is_some() {
                    return Err(24);
                }
                let mut off = self.g92;
                for i in 0..3 {
                    if let Some(v) = axis[i] {
                        off[i] = gc.position[i] - self.wcs[gc.wcs][i] - v * scale;
                    }
                }
                effects.push(Effect::SetG92(off));
                axis_used = true;
                motion_axis_cmd = false;
            }
            Some(921) => {
                effects.push(Effect::SetG92([0.0; 3]));
                motion_axis_cmd = !has_axis;
            }
            _ => {}
        }
        if g.tlo == Some(431) {
            // G43.1 solo admite el eje Z.
            if axis[0].is_some() || axis[1].is_some() {
                return Err(37);
            }
            axis_used = true;
            motion_axis_cmd = false;
        }
        let g53 = g.nonmodal == Some(530);
        if g53 && !matches!(eff_mode, MotionMode::Rapid | MotionMode::Linear) {
            return Err(30);
        }

        // Movimiento.
        if motion_axis_cmd && (has_axis || mode.is_some()) {
            if let Some(m) = mode {
                gc.motion = m;
            }
            if has_axis && eff_mode == MotionMode::None {
                // Palabras de eje sin uso con G80.
                return Err(31);
            }
            let mut target = gc.position;
            for i in 0..3 {
                if let Some(v) = axis[i] {
                    target[i] =
                        self.axis_target(gc.absolute, g53, gc.position[i], v * scale, wco[i]);
                }
            }
            match eff_mode {
                MotionMode::Rapid | MotionMode::Linear if has_axis => {
                    let rapid = eff_mode == MotionMode::Rapid;
                    let feed = if rapid {
                        0.0
                    } else if gc.inverse_time {
                        let f = inv_f.ok_or(22u32)?;
                        if f <= 0.0 {
                            return Err(22);
                        }
                        dist(gc.position, target) * f
                    } else {
                        if gc.feed <= 0.0 {
                            return Err(22);
                        }
                        gc.feed
                    };
                    if self.outside_travel(target) {
                        soft_alarm = true;
                    } else {
                        let kind = if rapid {
                            MoveKind::Rapid
                        } else {
                            MoveKind::Feed
                        };
                        if let Some(b) =
                            self.make_block(kind, gc.position, target, feed, &gc, val(b'N'))
                        {
                            motion_actions.push(Action::Move(Box::new(b)));
                        }
                    }
                    gc.position = target;
                    axis_used = true;
                    moves = true;
                }
                MotionMode::Probe(n) => {
                    if !has_axis {
                        return Err(26);
                    }
                    // Sin avance o con tiempo inverso no hay una velocidad de sondeo definida.
                    if gc.inverse_time || gc.feed <= 0.0 {
                        return Err(22);
                    }
                    if dist(gc.position, target) < 1e-9 {
                        return Err(33);
                    }
                    if self.outside_travel(target) {
                        soft_alarm = true;
                    } else {
                        let cycle = self.probe_cycle(&gc, target, n, val(b'N'));
                        gc.position = cycle.stop;
                        motion_actions.extend(cycle.actions);
                    }
                    axis_used = true;
                    moves = true;
                }
                MotionMode::ArcCw | MotionMode::ArcCcw => {
                    if gc.plane != 17 {
                        return Err(20);
                    }
                    if axis[0].is_none() && axis[1].is_none() {
                        return Err(32);
                    }
                    let start = gc.position;
                    let feed = if gc.inverse_time {
                        inv_f.ok_or(22u32)?
                    } else {
                        gc.feed
                    };
                    if feed <= 0.0 {
                        return Err(22);
                    }
                    let cw = eff_mode == MotionMode::ArcCw;
                    let center = if let Some(r) = val(b'R') {
                        used_ijkr = true;
                        let (cx, cy) = (target[0] - start[0], target[1] - start[1]);
                        let d = cx.hypot(cy);
                        let r = r * scale;
                        if d < 1e-9 {
                            return Err(33);
                        }
                        let h2 = r * r - d * d / 4.0;
                        if h2 < -1e-9 {
                            return Err(34);
                        }
                        let h = h2.max(0.0).sqrt();
                        // Normal a la izquierda del sentido de avance de la cuerda.
                        let (nx, ny) = (-cy / d, cx / d);
                        let mut side = if cw { -1.0 } else { 1.0 };
                        if r < 0.0 {
                            side = -side;
                        }
                        [
                            start[0] + cx / 2.0 + side * nx * h,
                            start[1] + cy / 2.0 + side * ny * h,
                        ]
                    } else {
                        let (i, j) = (val(b'I'), val(b'J'));
                        if i.is_none() && j.is_none() {
                            return Err(35);
                        }
                        used_ijkr = true;
                        let c = [
                            start[0] + i.unwrap_or(0.0) * scale,
                            start[1] + j.unwrap_or(0.0) * scale,
                        ];
                        let r0 = (start[0] - c[0]).hypot(start[1] - c[1]);
                        let r1 = (target[0] - c[0]).hypot(target[1] - c[1]);
                        let dr = (r1 - r0).abs();
                        if dr > 0.5 || (dr > 0.005 && dr > 0.001 * r0) {
                            return Err(33);
                        }
                        c
                    };
                    if val(b'K').is_some() {
                        used_ijkr = true;
                    }
                    let pts = arc_points(start, target, center, cw);
                    if self.outside_travel(target) {
                        soft_alarm = true;
                    } else {
                        let mut prev = start;
                        for p in pts {
                            if let Some(b) =
                                self.make_block(MoveKind::Feed, prev, p, feed, &gc, val(b'N'))
                            {
                                motion_actions.push(Action::Move(Box::new(b)));
                            }
                            prev = p;
                        }
                    }
                    gc.position = target;
                    axis_used = true;
                    moves = true;
                }
                _ => {}
            }
        }
        if has_axis && !axis_used && !moves {
            return Err(36);
        }
        if (val(b'I').is_some()
            || val(b'J').is_some()
            || val(b'K').is_some()
            || val(b'R').is_some())
            && !used_ijkr
        {
            return Err(36);
        }
        if val(b'P').is_some() && !used_p {
            return Err(36);
        }
        if val(b'L').is_some() && !used_l {
            return Err(36);
        }

        // Acciones en orden de ejecución.
        let laser = self.laser_mode();
        let spindle_changed = gc.spindle != old.spindle;
        let s_changed = (gc.s - old.s).abs() > f64::EPSILON;
        let sync_spindle = if laser {
            spindle_changed || (gc.spindle == SpindleMode::Cw && s_changed && !moves)
        } else {
            spindle_changed || (s_changed && gc.spindle != SpindleMode::Off)
        };
        let mut actions = Vec::new();
        if sync_spindle {
            actions.push(Action::Sync);
            actions.push(Action::ApplySpindle(gc.spindle, gc.s));
        }
        if gc.flood != old.flood || gc.mist != old.mist {
            actions.push(Action::Sync);
        }
        if g.tool {
            actions.push(Action::Sync);
            actions.push(Action::ToolChange);
        }
        if let Some(d) = dwell {
            actions.push(Action::Sync);
            actions.push(Action::Dwell(d));
        }
        if soft_alarm {
            actions.push(Action::SoftLimitAlarm);
        } else {
            actions.extend(motion_actions);
        }
        match g.stop {
            Some(0) => {
                actions.push(Action::Sync);
                actions.push(Action::Pause);
            }
            Some(2 | 30) => {
                actions.push(Action::Sync);
                actions.push(Action::ApplySpindle(SpindleMode::Off, gc.s));
                actions.push(Action::ProgramEnd);
                gc.motion = MotionMode::Linear;
                gc.plane = 17;
                gc.absolute = true;
                gc.inverse_time = false;
                gc.wcs = 0;
                gc.spindle = SpindleMode::Off;
                gc.flood = false;
                gc.mist = false;
            }
            _ => {}
        }
        Ok(Parsed {
            gc,
            effects,
            actions,
        })
    }

    /// Interpreta `$J=...` (el texto recibido es lo que sigue al `=`). El jog no modifica el
    /// estado del parser.
    pub(crate) fn parse_jog(&self, text: &str) -> Result<Block, u32> {
        let words = tokenize(text).map_err(|e| if e == 20 { 16 } else { e })?;
        let mut absolute = self.gc.absolute;
        let mut inches = self.gc.inches;
        let mut g53 = false;
        let mut feed: Option<f64> = None;
        let mut n: Option<u32> = None;
        let mut axis = [None; 3];
        for w in &words {
            match w.letter {
                b'G' => match code10(w.value)? {
                    900 => absolute = true,
                    910 => absolute = false,
                    200 => inches = true,
                    210 => inches = false,
                    530 => g53 = true,
                    // G93 se ignora; cualquier otro G está prohibido.
                    930 => {}
                    _ => return Err(16),
                },
                b'F' => feed = Some(w.value),
                b'N' => n = Some(w.value as u32),
                b'X' => axis[0] = Some(w.value),
                b'Y' => axis[1] = Some(w.value),
                b'Z' => axis[2] = Some(w.value),
                _ => return Err(16),
            }
        }
        let scale = if inches { 25.4 } else { 1.0 };
        let feed = feed.ok_or(22u32)? * scale;
        if feed <= 0.0 {
            return Err(22);
        }
        if axis.iter().all(Option::is_none) {
            return Err(16);
        }
        let wco = self.wco_for(self.gc.wcs);
        let from = self.gc.position;
        let mut to = from;
        for i in 0..3 {
            if let Some(v) = axis[i] {
                to[i] = self.axis_target(absolute, g53, from[i], v * scale, wco[i]);
            }
        }
        if self.soft_limits_on() && self.outside_travel(to) {
            return Err(15);
        }
        Ok(Block {
            kind: MoveKind::Jog,
            from,
            to,
            len: dist(from, to),
            feed,
            spindle: SpindleMode::Off,
            s: 0.0,
            line: self.lines_seen,
            n,
        })
    }

    fn axis_target(&self, absolute: bool, g53: bool, current: f64, v: f64, wco: f64) -> f64 {
        if absolute {
            if g53 { v } else { v + wco }
        } else {
            current + v
        }
    }

    fn make_block(
        &self,
        kind: MoveKind,
        from: [f64; 3],
        to: [f64; 3],
        feed: f64,
        gc: &Gc,
        n: Option<f64>,
    ) -> Option<Block> {
        let len = dist(from, to);
        if len < 1e-9 {
            return None;
        }
        Some(Block {
            kind,
            from,
            to,
            len,
            feed,
            spindle: gc.spindle,
            s: gc.s,
            line: self.lines_seen,
            n: n.map(|v| v as u32),
        })
    }

    /// Acciones de un ciclo de sonda de `gc.position` hacia `target` y dónde queda la cabeza.
    /// `n` es el decimal del código: 2 y 3 hacia la pieza, 4 y 5 alejándose; 2 y 4 dan alarma
    /// si no hay cambio de estado.
    fn probe_cycle(&self, gc: &Gc, target: [f64; 3], n: u8, line_n: Option<f64>) -> ProbeCycle {
        let away = n >= 4;
        let from = gc.position;
        let hit_at = self
            .probe_surface()
            .and_then(|s| s.transition(from, target, away));
        let stop = hit_at.unwrap_or(target);
        let mut actions = vec![Action::Sync, Action::ProbeStart { away }];
        // El láser nunca dispara en una sonda: el bloque va con el husillo apagado.
        let off = Gc {
            spindle: SpindleMode::Off,
            s: 0.0,
            ..gc.clone()
        };
        if let Some(b) = self.make_block(MoveKind::Probe, from, stop, gc.feed, &off, line_n) {
            actions.push(Action::Move(Box::new(b)));
        }
        actions.push(Action::Sync);
        actions.push(Action::ProbeEnd {
            stop,
            hit: hit_at.is_some(),
            error_on_miss: n == 2 || n == 4,
        });
        ProbeCycle { stop, actions }
    }

    fn rapid_blocks(&self, from: [f64; 3], to: [f64; 3], gc: &Gc) -> Option<Action> {
        self.make_block(MoveKind::Rapid, from, to, 0.0, gc, None)
            .map(|b| Action::Move(Box::new(b)))
    }
}

/// Resultado de interpretar un `G38.x`: acciones y posición programada al terminar.
struct ProbeCycle {
    stop: [f64; 3],
    actions: Vec<Action>,
}

/// Puntos de un arco en XY (el último es el destino exacto), con Z interpolada.
fn arc_points(start: [f64; 3], target: [f64; 3], c: [f64; 2], cw: bool) -> Vec<[f64; 3]> {
    use std::f64::consts::TAU;
    let r = (start[0] - c[0]).hypot(start[1] - c[1]);
    let a0 = (start[1] - c[1]).atan2(start[0] - c[0]);
    let a1 = (target[1] - c[1]).atan2(target[0] - c[0]);
    let mut sweep = if cw { a0 - a1 } else { a1 - a0 };
    if sweep <= 1e-9 {
        sweep += TAU;
    }
    let arc_len = sweep * r;
    let n = ((arc_len / 0.5).ceil() as usize).clamp(2, 2000);
    let mut pts = Vec::with_capacity(n);
    for k in 1..=n {
        if k == n {
            pts.push(target);
            break;
        }
        let f = k as f64 / n as f64;
        let a = if cw { a0 - sweep * f } else { a0 + sweep * f };
        pts.push([
            c[0] + r * a.cos(),
            c[1] + r * a.sin(),
            start[2] + (target[2] - start[2]) * f,
        ]);
    }
    pts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizer_errors() {
        assert!(tokenize("G1X10.5Y-2F1000").is_ok());
        assert_eq!(tokenize("1G").err(), Some(1));
        assert_eq!(tokenize("G1X").err(), Some(2));
        assert_eq!(tokenize("XYZZY").err(), Some(2));
        assert_eq!(code10(1.5).err(), None);
        assert_eq!(code10(92.1).unwrap(), 921);
    }

    #[test]
    fn arc_ends_at_target() {
        let pts = arc_points([10.0, 0.0, 0.0], [0.0, 10.0, 0.0], [0.0, 0.0], false);
        let last = pts.last().unwrap();
        assert_eq!(*last, [0.0, 10.0, 0.0]);
        for p in &pts {
            assert!((p[0].hypot(p[1]) - 10.0).abs() < 1e-9);
        }
    }
}
