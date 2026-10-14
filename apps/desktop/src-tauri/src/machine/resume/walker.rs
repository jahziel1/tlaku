//! Lectura del G-code de un trabajo: recorre las líneas siguiendo el estado modal (distancia,
//! unidades, husillo, `S`, `F`, aire y Z) y saca de cada línea con movimiento un [`Step`].
//!
//! Solo entiende lo que genera Tlaku (`G0`/`G1`, también `G2`/`G3` como cuerda hasta su destino,
//! `G90`/`G91`, `G20`/`G21`, `M3`/`M4`/`M5`, `M7`/`M8`/`M9`, `S`, `F`, `G4`). Lo que no entiende no
//! lo inventa: marca la bandera correspondiente en [`Flags`] para que el aviso lo diga.

use tlaku_doc::{ResumeAir, ResumeModal, ResumeSpindle};
use tlaku_grbl::streamer::prepare_line_with;

/// Qué clase de movimiento es un paso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    /// `G0`: desplazamiento con el láser apagado.
    Rapid,
    /// `G1`/`G2`/`G3` sin láser (apagado, `S0`: el overscan).
    Feed,
    /// `G1`/`G2`/`G3` con el láser encendido.
    Cut,
}

/// Una línea con movimiento en el plano XY.
#[derive(Debug, Clone, Copy)]
pub struct Step {
    /// Línea del G-code (desde 0).
    pub line: u32,
    pub from: [f64; 2],
    pub to: [f64; 2],
    pub kind: StepKind,
    /// `S` vigente en el paso (unidades del firmware).
    pub s: f32,
    /// Recto y en coordenadas absolutas: se puede empezar a mitad.
    pub splittable: bool,
}

impl Step {
    pub fn length(&self) -> f64 {
        (self.to[0] - self.from[0]).hypot(self.to[1] - self.from[1])
    }
}

/// Cosas del G-code que la reanudación no sabe reproducir.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flags {
    /// `G92`, `G10`, `G28.1`, `G30.1`: cambian el sistema de coordenadas; al empezar a mitad se
    /// perderían.
    pub offsets: bool,
    /// `G28`, `G30`, `G53`: movimientos a posiciones de la máquina que no se siguen.
    pub machine_moves: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Motion {
    Rapid,
    Linear,
    Arc,
    Other,
}

/// Recorre líneas de G-code llevando el estado modal.
pub struct Walker {
    motion: Motion,
    absolute: bool,
    inches: bool,
    spindle: ResumeSpindle,
    air: ResumeAir,
    s: Option<f64>,
    f: Option<f64>,
    /// Posición actual en mm (X, Y); `None` mientras no haya habido movimiento.
    pos: Option<[f64; 2]>,
    z: f64,
    pub flags: Flags,
    words: Vec<(u8, f64, bool)>,
}

impl Default for Walker {
    fn default() -> Self {
        Self::new()
    }
}

impl Walker {
    pub fn new() -> Self {
        Self {
            motion: Motion::Rapid,
            absolute: true,
            inches: false,
            spindle: ResumeSpindle::Off,
            air: ResumeAir::Off,
            s: None,
            f: None,
            pos: None,
            z: 0.0,
            flags: Flags::default(),
            words: Vec::with_capacity(8),
        }
    }

    /// Estado modal tras las líneas leídas hasta ahora.
    pub fn modal(&self) -> ResumeModal {
        ResumeModal {
            absolute: self.absolute,
            inches: self.inches,
            spindle: self.spindle,
            air: self.air,
            s: self.s,
            f: self.f,
            z: self.z,
        }
    }

    /// Posición (X, Y) en mm tras las líneas leídas, si hubo algún movimiento.
    pub fn position(&self) -> Option<[f64; 2]> {
        self.pos
    }

    /// Lee la línea `index`. Devuelve el paso si movió la cabeza en el plano.
    pub fn feed(&mut self, index: u32, raw: &str) -> Option<Step> {
        self.tokenize(raw);
        let unit = |inches: bool| if inches { 25.4 } else { 1.0 };
        let mut axis: [Option<f64>; 3] = [None; 3];
        let mut g53 = false;
        let mut dwell = false;
        // Primero el estado modal de la línea (el orden de las palabras no importa en G-code).
        for i in 0..self.words.len() {
            let (letter, v, _) = self.words[i];
            match letter {
                b'G' => match (v * 10.0).round() as i64 {
                    0 => self.motion = Motion::Rapid,
                    10 => self.motion = Motion::Linear,
                    20 | 30 => self.motion = Motion::Arc,
                    40 => dwell = true,
                    200 => self.inches = true,
                    210 => self.inches = false,
                    900 => self.absolute = true,
                    910 => self.absolute = false,
                    100 | 281 | 301 | 920..=923 => self.flags.offsets = true,
                    280 | 300 => self.flags.machine_moves = true,
                    530 => g53 = true,
                    382..=385 | 800 => self.motion = Motion::Other,
                    _ => {}
                },
                b'M' => match v.round() as i64 {
                    3 => self.spindle = ResumeSpindle::M3,
                    4 => self.spindle = ResumeSpindle::M4,
                    5 => self.spindle = ResumeSpindle::Off,
                    7 => self.air = ResumeAir::Mist,
                    8 => self.air = ResumeAir::Flood,
                    9 => self.air = ResumeAir::Off,
                    _ => {}
                },
                b'S' => self.s = Some(v),
                b'F' => self.f = Some(v),
                b'X' => axis[0] = Some(v),
                b'Y' => axis[1] = Some(v),
                b'Z' => axis[2] = Some(v),
                _ => {}
            }
        }
        if dwell || g53 {
            if g53 && (axis[0].is_some() || axis[1].is_some()) {
                self.flags.machine_moves = true;
            }
            return None;
        }
        if self.motion == Motion::Other {
            return None;
        }
        let k = unit(self.inches);
        if let Some(dz) = axis[2] {
            if self.absolute {
                self.z = dz * k;
            } else {
                self.z += dz * k;
            }
        }
        if axis[0].is_none() && axis[1].is_none() {
            return None;
        }
        let from = self.pos.unwrap_or([0.0, 0.0]);
        let mut to = from;
        for i in 0..2 {
            if let Some(v) = axis[i] {
                to[i] = if self.absolute {
                    v * k
                } else {
                    from[i] + v * k
                };
            }
        }
        self.pos = Some(to);
        let s = self.s.unwrap_or(0.0);
        let kind = match self.motion {
            // En modo láser de GRBL el G0 apaga el láser; en los demás dialectos el generador
            // escribe `S0` o `M5` antes, así que también es un desplazamiento.
            Motion::Rapid => StepKind::Rapid,
            _ if self.spindle != ResumeSpindle::Off && s > 0.0 => StepKind::Cut,
            _ => StepKind::Feed,
        };
        Some(Step {
            line: index,
            from,
            to,
            kind,
            s: s as f32,
            splittable: self.absolute && self.motion != Motion::Arc,
        })
    }

    /// Separa la línea en palabras `(letra, valor, ¿tenía parte decimal?)`. Ignora comentarios,
    /// espacios, números de línea (`N`) y lo que no sea letra + número.
    fn tokenize(&mut self, raw: &str) {
        self.words.clear();
        let b = raw.as_bytes();
        let mut i = 0;
        let mut in_paren = false;
        while i < b.len() {
            let c = b[i];
            if in_paren {
                if c == b')' {
                    in_paren = false;
                }
                i += 1;
                continue;
            }
            match c {
                b';' => break,
                b'(' => {
                    in_paren = true;
                    i += 1;
                }
                c if c.is_ascii_alphabetic() => {
                    let letter = c.to_ascii_uppercase();
                    let start = i + 1;
                    let mut end = start;
                    while end < b.len()
                        && (b[end].is_ascii_digit() || matches!(b[end], b'.' | b'-' | b'+'))
                    {
                        end += 1;
                    }
                    let text = raw[start..end].trim();
                    if letter != b'N'
                        && let Ok(v) = text.parse::<f64>()
                        && v.is_finite()
                    {
                        self.words.push((letter, v, text.contains('.')));
                    }
                    i = end.max(i + 1);
                }
                _ => i += 1,
            }
        }
    }
}

/// ¿La línea se envía a la máquina? Las vacías y las de solo comentario no, y no cuentan en el
/// avance del `Streamer`.
pub fn is_sendable(raw: &str) -> bool {
    prepare_line_with(raw, false, false).is_ok_and(|t| !t.is_empty())
}

/// Resultado de leer un trabajo entero.
pub struct Parsed {
    pub steps: Vec<Step>,
    /// Línea del G-code de cada línea enviable, en orden: `sendable[n - 1]` es la línea que
    /// recibió el `n`-ésimo `ok`.
    pub sendable: Vec<u32>,
    pub flags: Flags,
}

impl Parsed {
    pub fn parse(lines: &[String]) -> Self {
        let mut w = Walker::new();
        let mut steps = Vec::new();
        let mut sendable = Vec::new();
        for (i, raw) in lines.iter().enumerate() {
            let i = u32::try_from(i).unwrap_or(u32::MAX);
            if is_sendable(raw) {
                sendable.push(i);
            }
            if let Some(s) = w.feed(i, raw) {
                steps.push(s);
            }
        }
        Self {
            steps,
            sendable,
            flags: w.flags,
        }
    }

    /// Cuántas líneas enviables hay antes de la línea `line` (sin contarla).
    pub fn sendable_before(&self, line: u32) -> usize {
        self.sendable.partition_point(|&l| l < line)
    }

    /// Índice del primer paso cuya línea es `>= line`.
    pub fn step_at_or_after(&self, line: u32) -> usize {
        self.steps.partition_point(|s| s.line < line)
    }

    /// Caja de los pasos con láser; si no hay ninguno, de todos.
    pub fn bounds(&self) -> Option<[[f64; 2]; 2]> {
        let boxed = |keep: &dyn Fn(&Step) -> bool| {
            let mut it = self.steps.iter().filter(|s| keep(s));
            let first = it.next()?;
            let mut b = [first.from, first.from];
            for s in std::iter::once(first).chain(it) {
                for p in [s.from, s.to] {
                    b[0][0] = b[0][0].min(p[0]);
                    b[0][1] = b[0][1].min(p[1]);
                    b[1][0] = b[1][0].max(p[0]);
                    b[1][1] = b[1][1].max(p[1]);
                }
            }
            Some(b)
        };
        boxed(&|s| s.kind == StepKind::Cut).or_else(|| boxed(&|_| true))
    }
}

/// Estado modal y posición justo antes de ejecutar la línea `before` (o después de ella con
/// `inclusive`).
pub fn modal_at(
    lines: &[String],
    before: usize,
    inclusive: bool,
) -> (ResumeModal, Option<[f64; 2]>) {
    let end = (before + usize::from(inclusive)).min(lines.len());
    let mut w = Walker::new();
    for (i, raw) in lines[..end].iter().enumerate() {
        let _ = w.feed(u32::try_from(i).unwrap_or(u32::MAX), raw);
    }
    (w.modal(), w.position())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_string).collect()
    }

    #[test]
    fn follows_modal_state_and_classifies_steps() {
        let l = lines(
            "; encabezado\nG90\nG21\nM4 S0\nG0 X10 Y5\nG1 X12.3 S500 F6000\nG1 X15\nG1 X20 S0\nG0 Y5.1\nM5\nM9",
        );
        let p = Parsed::parse(&l);
        let kinds: Vec<_> = p.steps.iter().map(|s| s.kind).collect();
        assert_eq!(
            kinds,
            [
                StepKind::Rapid,
                StepKind::Cut,
                StepKind::Cut,
                StepKind::Feed,
                StepKind::Rapid
            ]
        );
        assert_eq!(p.steps[1].from, [10.0, 5.0]);
        assert_eq!(p.steps[1].to, [12.3, 5.0]);
        assert_eq!(p.steps[4].to, [20.0, 5.1]);
        let (m, pos) = modal_at(&l, 7, false);
        assert_eq!(m.spindle, ResumeSpindle::M4);
        assert_eq!((m.s, m.f), (Some(500.0), Some(6000.0)));
        assert_eq!(pos, Some([15.0, 5.0]));
        // La línea 0 es un comentario: no se envía.
        assert_eq!(p.sendable[0], 1);
        assert_eq!(p.sendable_before(4), 3);
    }

    #[test]
    fn relative_z_accumulates_and_relative_xy_moves_from_the_last_point() {
        let l = lines("G90\nG0 X1 Y1\nG91\nG0 Z-1.5\nG1 X2 Y0 S100\nG90\nG91\nG0 Z1.5\nG90");
        let (m, _) = modal_at(&l, l.len(), false);
        assert!((m.z - 0.0).abs() < 1e-9);
        let (m, pos) = modal_at(&l, 5, false);
        assert!((m.z + 1.5).abs() < 1e-9);
        assert!(!m.absolute);
        assert_eq!(pos, Some([3.0, 1.0]));
    }

    #[test]
    fn air_and_offsets_are_tracked() {
        let l = lines("M8\nG0 X1 Y1\nM9\nG92 X0");
        let (m, _) = modal_at(&l, 2, false);
        assert_eq!(m.air, ResumeAir::Flood);
        let p = Parsed::parse(&l);
        assert!(p.flags.offsets);
    }

    #[test]
    fn tokenizer_ignores_comments_and_line_numbers() {
        let l = lines("N10 G1 X5 (mueve) Y2 ; resto S9");
        let p = Parsed::parse(&l);
        assert_eq!(p.steps[0].to, [5.0, 2.0]);
        assert_eq!(p.steps[0].s, 0.0);
    }
}
