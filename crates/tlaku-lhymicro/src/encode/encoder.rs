//! Máquina de estados que emite el texto LHYMICRO-GL (modo normal y modo compacto).

use super::{
    EncodeConfig, ProgramStats, RAPID_ESTIMATE_MM_S, distance, line_runs, mil_to_mm, rapid_text,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    /// Modo normal: saltos con `S1P`.
    Rapid,
    /// Modo compacto: movimientos con la velocidad del programa.
    Program,
}

pub(super) struct Encoder<'a> {
    pub(super) cfg: &'a EncodeConfig,
    pub(super) out: Vec<u8>,
    pub(super) pos: (i64, i64),
    pub(super) mode: Mode,
    /// Destino de un viaje diferido: se ejecuta cuando haga falta (al cortar o al terminar).
    pub(super) pending: Option<(i64, i64)>,
    pub(super) code: Option<String>,
    pub(super) leftward: bool,
    pub(super) topward: bool,
    pub(super) x_engaged: bool,
    pub(super) y_engaged: bool,
    pub(super) laser: bool,
    pub(super) stats: ProgramStats,
}

impl<'a> Encoder<'a> {
    pub(super) fn new(cfg: &'a EncodeConfig) -> Self {
        Encoder {
            cfg,
            out: Vec::new(),
            pos: cfg.start_mil,
            mode: Mode::Rapid,
            pending: None,
            code: None,
            leftward: false,
            topward: false,
            x_engaged: false,
            y_engaged: false,
            laser: false,
            stats: ProgramStats::default(),
        }
    }

    fn emit(&mut self, s: &str) {
        self.out.extend_from_slice(s.as_bytes());
    }

    fn is_right(&self) -> bool {
        self.x_engaged && !self.y_engaged && !self.leftward
    }
    fn is_left(&self) -> bool {
        self.x_engaged && !self.y_engaged && self.leftward
    }
    fn is_bottom(&self) -> bool {
        !self.x_engaged && self.y_engaged && !self.topward
    }
    fn is_top(&self) -> bool {
        !self.x_engaged && self.y_engaged && self.topward
    }

    fn set_laser(&mut self, on: bool) {
        if on && !self.laser {
            self.emit("D");
            self.laser = true;
        } else if !on && self.laser {
            self.emit("U");
            self.laser = false;
        }
    }

    /// Salto rápido con el láser apagado (modo normal).
    fn rapid_to(&mut self, to: (i64, i64)) {
        debug_assert_eq!(self.mode, Mode::Rapid);
        let (dx, dy) = (to.0 - self.pos.0, to.1 - self.pos.1);
        if dx == 0 && dy == 0 {
            return;
        }
        let text = rapid_text(dx, dy, self.cfg.autolock);
        self.emit(&text);
        let len = mil_to_mm(dx.abs().max(dy.abs()));
        self.stats.rapid_mm += len;
        self.stats.rapids += 1;
        self.stats.est_seconds += len / RAPID_ESTIMATE_MM_S;
        self.pos = to;
    }

    pub(super) fn flush_pending(&mut self) {
        if let Some(t) = self.pending.take() {
            self.rapid_to(t);
        }
    }

    /// Entra al modo compacto declarando la dirección del primer tramo hacia `target`.
    fn enter_program(&mut self, code: &str, target: (i64, i64)) {
        debug_assert_eq!(self.mode, Mode::Rapid);
        let (dx, dy) = (target.0 - self.pos.0, target.1 - self.pos.1);
        let first = line_runs(dx, dy).first().copied().unwrap_or((0, 0));
        self.leftward = first.0 <= 0;
        self.topward = first.1 <= 0;
        let horizontal_major = dx.abs() >= dy.abs();
        self.emit("I");
        self.emit(code);
        self.emit("N");
        let x_dir = if self.leftward { "T" } else { "B" };
        let y_dir = if self.topward { "L" } else { "R" };
        if horizontal_major {
            self.emit(y_dir);
            self.emit(x_dir);
            self.x_engaged = true;
            self.y_engaged = false;
        } else {
            self.emit(x_dir);
            self.emit(y_dir);
            self.x_engaged = false;
            self.y_engaged = true;
        }
        self.emit("S1E");
        self.mode = Mode::Program;
        self.laser = false;
        self.code = Some(code.to_string());
        self.stats.programs += 1;
    }

    /// Sale del modo compacto: láser apagado, `FNSE` y espera a que la placa termine.
    pub(super) fn exit_program(&mut self) {
        if self.mode != Mode::Program {
            return;
        }
        // La `U` explícita no la escribe MeerK40t (el `N` ya apaga el láser); se añade por
        // seguridad y es un comando válido en el modo compacto.
        self.set_laser(false);
        self.emit("FNSE-\n");
        self.mode = Mode::Rapid;
        self.code = None;
    }

    fn octant(&mut self, dx: i64, dy: i64, on: bool) {
        if dx == 0 && dy == 0 {
            return;
        }
        if dx.abs() == dy.abs() {
            self.x_engaged = true;
            self.y_engaged = true;
            if dx > 0 {
                if self.leftward {
                    self.emit("B");
                    self.leftward = false;
                }
            } else if !self.leftward {
                self.emit("T");
                self.leftward = true;
            }
            if dy > 0 {
                if self.topward {
                    self.emit("R");
                    self.topward = false;
                }
            } else if !self.topward {
                self.emit("L");
                self.topward = true;
            }
            self.emit("M");
            self.set_laser(on);
            let d = distance(dy.unsigned_abs());
            self.emit(&d);
        } else if dx != 0 {
            if dx > 0 {
                if !self.is_right() {
                    self.emit("B");
                    self.leftward = false;
                }
            } else if !self.is_left() {
                self.emit("T");
                self.leftward = true;
            }
            self.x_engaged = true;
            self.y_engaged = false;
            self.set_laser(on);
            let d = distance(dx.unsigned_abs());
            self.emit(&d);
        } else {
            if dy > 0 {
                if !self.is_bottom() {
                    self.emit("R");
                    self.topward = false;
                }
            } else if !self.is_top() {
                self.emit("L");
                self.topward = true;
            }
            self.x_engaged = false;
            self.y_engaged = true;
            self.set_laser(on);
            let d = distance(dy.unsigned_abs());
            self.emit(&d);
        }
    }

    /// Movimiento dentro del programa (compacto) hasta `to`.
    fn program_move(&mut self, to: (i64, i64), on: bool) {
        let (dx, dy) = (to.0 - self.pos.0, to.1 - self.pos.1);
        let len_mm = mil_to_mm(((dx * dx + dy * dy) as f64).sqrt().round() as i64);
        for (rx, ry) in line_runs(dx, dy) {
            self.octant(rx, ry, on);
        }
        if on {
            self.stats.burn_mm += len_mm;
        } else {
            self.stats.travel_mm += len_mm;
        }
        self.pos = to;
    }

    pub(super) fn cut(&mut self, to: (i64, i64), on: bool, code: &str) {
        if to == self.pos && self.pending.is_none() {
            return;
        }
        if self.mode == Mode::Rapid {
            self.flush_pending();
            if to == self.pos {
                return;
            }
            self.enter_program(code, to);
        }
        self.program_move(to, on);
    }

    pub(super) fn travel(&mut self, to: (i64, i64)) {
        let from = self.pending.unwrap_or(self.pos);
        if to == from {
            return;
        }
        if self.mode == Mode::Program {
            let (dx, dy) = ((to.0 - self.pos.0) as f64, (to.1 - self.pos.1) as f64);
            if (dx * dx + dy * dy).sqrt() < self.cfg.min_jog_mil as f64 {
                self.program_move(to, false);
                return;
            }
            self.exit_program();
        }
        self.pending = Some(to);
    }
}
