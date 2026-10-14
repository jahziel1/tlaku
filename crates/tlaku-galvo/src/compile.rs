//! De `Toolpath` (con `GalvoParams`) y `GalvoDevice` a una lista de comandos de la tarjeta.
//!
//! Traducción (docs/hardware/galvo.md, secciones 2.3 y 8):
//!
//! - `Move::Travel` es un **salto** (`listJumpTo`, láser apagado) y `Move::Cut` es una **marca**
//!   (`listMarkTo`). Una marca con potencia efectiva 0 (overscan) se emite como salto: nunca se
//!   manda `MarkTo` sin potencia.
//! - `Move::Dwell` con potencia es un disparo en un punto (`listLaserOnPoint`, en trozos de
//!   600 ms); sin potencia, una espera.
//! - Antes de marcar se fijan velocidades, frecuencia, ancho de pulso (MOPA), retardos y potencia;
//!   solo se repite un comando si su valor cambia.
//! - El retardo de salto se interpola entre el mínimo y el máximo del perfil según la distancia del
//!   salto y la distancia límite; el retardo final se inserta al terminar cada trazo.
//! - `repeat` expande el bloque ese número de veces (no se conoce ningún comando de bucle).
//! - Las coordenadas pasan por la corrección del campo (`calib`) y se cuantizan a 16 bits con el
//!   centro en `0x8000`. Un punto fuera del campo es un error, no se recorta.
//! - El resultado se vuelve a validar con `safety::validate_list`.
//!
//! Sin verificar con hardware: las unidades de `params`. Ignorados con aviso: asistencia de aire,
//! Z y el archivo `.cor` (su semántica no está documentada; ver `cor`).

use crate::calib::{CalibError, FieldMap, units_per_mm};
use crate::cmd::{Delay, ListOp};
use crate::params;
use crate::safety::{self, ListKind, SafetyError, SafetyLimits};
use crate::wire::{self, Raw};
use std::fmt;
use tlaku_core::Point;
use tlaku_core::toolpath::{GalvoParams, Move, Toolpath, ToolpathBlock};
use tlaku_doc::{GalvoDevice, LaserSource};

/// Qué punto de las coordenadas del `Toolpath` cae en el centro del campo (`0x8000`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FieldOrigin {
    /// El (0, 0): el sistema natural de la tarjeta.
    #[default]
    Center,
    /// La esquina inferior izquierda del campo es el (0, 0), como en la cama de las demás
    /// máquinas: el centro es `(field_mm / 2, field_mm / 2)`.
    BottomLeft,
    /// Este punto del `Toolpath` es el centro del campo. Es la convención del pipeline: el campo
    /// de la lente está centrado en la cama, así que el punto es `bed / 2`.
    CenterAt(Point),
}

impl FieldOrigin {
    /// Milímetros desde el centro del campo para un punto del `Toolpath`.
    pub fn to_centered(&self, p: Point, field_mm: f64) -> (f64, f64) {
        match *self {
            FieldOrigin::Center => (p.x, p.y),
            FieldOrigin::BottomLeft => (p.x - field_mm / 2.0, p.y - field_mm / 2.0),
            FieldOrigin::CenterAt(c) => (p.x - c.x, p.y - c.y),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompileOptions {
    pub origin: FieldOrigin,
    /// `Mark` (marcado real) o `DryRun` (el mismo trabajo con el láser deshabilitado y el
    /// puntero rojo). `Light` no se compila aquí: ver [`compile_light`].
    pub kind: ListKind,
    /// Espera tras abrir el MO de la fibra, ms (galvoplotter usa 8). 0 = sin espera.
    pub mo_delay_ms: f64,
    /// Máximo de comandos de la lista.
    pub max_commands: usize,
    /// Posición de los espejos al empezar, para calcular el primer salto.
    pub start_units: (u16, u16),
}

impl Default for CompileOptions {
    fn default() -> Self {
        CompileOptions {
            origin: FieldOrigin::Center,
            kind: ListKind::Mark,
            mo_delay_ms: 8.0,
            max_commands: safety::DEFAULT_MAX_COMMANDS,
            start_units: (0x8000, 0x8000),
        }
    }
}

/// Resumen de una lista compilada.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ListStats {
    pub commands: usize,
    pub batches: usize,
    pub marks: usize,
    pub jumps: usize,
    pub dots: usize,
    pub mark_mm: f64,
    pub jump_mm: f64,
    /// Marcas con potencia 0 emitidas como saltos.
    pub zero_power_cuts: usize,
    /// Estimación de duración, s (movimientos y esperas; sin los retardos de láser).
    pub est_seconds: f64,
    /// Caja de las marcas en el sistema del `Toolpath`.
    pub mark_bounds: Option<[Point; 2]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledList {
    pub commands: Vec<Raw>,
    pub kind: ListKind,
    pub stats: ListStats,
    pub warnings: Vec<String>,
}

impl CompiledList {
    /// Las tandas de 3072 bytes listas para el cable.
    pub fn batches(&self) -> Vec<Vec<u8>> {
        // La lista solo tiene comandos de lista (la validación lo garantiza).
        wire::pack_batches(&self.commands).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompileError {
    Calib(CalibError),
    Safety(SafetyError),
    /// El bloque no trae `GalvoParams`.
    MissingParams {
        block: usize,
    },
    /// Un parámetro del bloque o del movimiento es inválido o está fuera del perfil.
    BadParam {
        block: usize,
        what: String,
    },
    TooManyCommands {
        max: usize,
    },
    /// El trabajo no tiene ningún movimiento.
    Empty,
    /// `CompileOptions::kind` no puede ser `Light`.
    WrongKind,
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::Calib(e) => write!(f, "{e}"),
            CompileError::Safety(e) => write!(f, "{e}"),
            CompileError::MissingParams { block } => write!(
                f,
                "el bloque {block} no tiene parámetros de galvo (frecuencia, retardos...)"
            ),
            CompileError::BadParam { block, what } => write!(f, "bloque {block}: {what}"),
            CompileError::TooManyCommands { max } => {
                write!(f, "el trabajo supera el máximo de {max} comandos")
            }
            CompileError::Empty => write!(f, "el trabajo no tiene movimientos"),
            CompileError::WrongKind => write!(f, "un encuadre se compila con compile_light"),
        }
    }
}

impl std::error::Error for CompileError {}

impl From<CalibError> for CompileError {
    fn from(e: CalibError) -> Self {
        CompileError::Calib(e)
    }
}

impl From<SafetyError> for CompileError {
    fn from(e: SafetyError) -> Self {
        CompileError::Safety(e)
    }
}

/// `GalvoParams` con los valores del perfil, para quien no tiene ajustes por capa (CLI, pruebas).
pub fn params_from_device(
    dev: &GalvoDevice,
    freq_khz: f64,
    pulse_width_ns: Option<u32>,
) -> GalvoParams {
    GalvoParams {
        frequency_khz: freq_khz,
        pulse_width_ns,
        laser_on_us: dev.timing.laser_on_us,
        laser_off_us: dev.timing.laser_off_us,
        end_us: dev.timing.end_us,
        polygon_us: dev.timing.polygon_us,
        jump_speed: dev.jump.speed,
        jump_delay_min_us: dev.jump.delay_min_us,
        jump_delay_max_us: dev.jump.delay_max_us,
        jump_distance_limit: dev.jump.distance_limit,
        repeat: 1,
        pause_before_ms: 0.0,
        pause_after_ms: 0.0,
    }
}

/// Máximo de repeticiones de un bloque.
pub const MAX_REPEAT: u32 = 10_000;

/// Valores vigentes en la tarjeta, para no repetir comandos.
#[derive(Default)]
struct Cur {
    jump_speed: Option<u16>,
    mark_speed: Option<u16>,
    delay_on: Option<Delay>,
    delay_off: Option<Delay>,
    delay_poly: Option<u16>,
    delay_jump: Option<Delay>,
    freq_period: Option<u16>,
    pulse: Option<u16>,
    power: Option<u16>,
}

struct Builder<'a> {
    dev: &'a GalvoDevice,
    map: FieldMap,
    origin: FieldOrigin,
    max_commands: usize,
    cmds: Vec<Raw>,
    cur: Cur,
    pos: (u16, u16),
    stats: ListStats,
    warnings: Vec<String>,
    /// Hay una racha de marcas abierta (para el retardo final).
    in_run: bool,
    /// Último punto alcanzado, en el sistema del `Toolpath`.
    last_pt: Option<Point>,
    end_ticks: u16,
    /// Velocidades vigentes en mm/s (para estimar tiempos).
    jump_mm_s: f64,
    mark_mm_s: f64,
    upmm: f64,
}

impl<'a> Builder<'a> {
    fn new(
        dev: &'a GalvoDevice,
        origin: FieldOrigin,
        max_commands: usize,
        start: (u16, u16),
    ) -> Result<Self, CompileError> {
        Ok(Builder {
            dev,
            map: FieldMap::new(dev.field_mm, &dev.correction)?,
            origin,
            max_commands,
            cmds: Vec::new(),
            cur: Cur::default(),
            pos: start,
            stats: ListStats::default(),
            warnings: Vec::new(),
            in_run: false,
            last_pt: None,
            end_ticks: 0,
            jump_mm_s: 0.0,
            mark_mm_s: 0.0,
            upmm: units_per_mm(dev.field_mm),
        })
    }

    fn push(&mut self, op: ListOp) -> Result<(), CompileError> {
        if self.cmds.len() >= self.max_commands {
            return Err(CompileError::TooManyCommands {
                max: self.max_commands,
            });
        }
        self.cmds.push(op.to_raw());
        Ok(())
    }

    fn units(&self, p: Point) -> Result<(u16, u16), CompileError> {
        let c = self.origin.to_centered(p, self.dev.field_mm);
        Ok(self.map.to_units(c)?)
    }

    fn dist_units(&self, to: (u16, u16)) -> f64 {
        let dx = f64::from(to.0) - f64::from(self.pos.0);
        let dy = f64::from(to.1) - f64::from(self.pos.1);
        dx.hypot(dy)
    }

    fn wait_ticks(&mut self, total: u64) -> Result<(), CompileError> {
        let mut left = total;
        while left > 0 {
            let t = left.min(u64::from(params::MAX_DWELL_TICKS)) as u16;
            self.push(ListOp::DelayTime { ticks: t })?;
            self.stats.est_seconds += f64::from(t) / 100_000.0;
            left -= u64::from(t);
        }
        Ok(())
    }

    /// Cierra la racha de marcas con el retardo final del trazo.
    fn finish_run(&mut self) -> Result<(), CompileError> {
        if self.in_run {
            self.in_run = false;
            if self.end_ticks > 0 {
                self.push(ListOp::DelayTime {
                    ticks: self.end_ticks,
                })?;
                self.stats.est_seconds += f64::from(self.end_ticks) / 100_000.0;
            }
        }
        Ok(())
    }

    /// Retardo de salto por distancia: del mínimo al máximo hasta la distancia límite.
    fn jump_delay(&self, dist_mm: f64, p: &GalvoParams) -> Delay {
        let t = if p.jump_distance_limit > 0.0 {
            (dist_mm / p.jump_distance_limit).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let us = p.jump_delay_min_us + (p.jump_delay_max_us - p.jump_delay_min_us) * t;
        Delay::from_us(us.round() as i64)
    }

    fn jump_to(&mut self, to: Point, p: &GalvoParams) -> Result<(), CompileError> {
        let u = self.units(to)?;
        self.last_pt = Some(to);
        if u == self.pos {
            return Ok(());
        }
        let d = self.dist_units(u);
        let dist_mm = d / self.upmm;
        let delay = self.jump_delay(dist_mm, p);
        if self.cur.delay_jump != Some(delay) {
            self.push(ListOp::JumpDelay(delay))?;
            self.cur.delay_jump = Some(delay);
        }
        self.push(ListOp::JumpTo {
            x: u.0,
            y: u.1,
            dist: d.round().min(65535.0) as u16,
        })?;
        self.stats.jumps += 1;
        self.stats.jump_mm += dist_mm;
        if self.jump_mm_s > 0.0 {
            self.stats.est_seconds += dist_mm / self.jump_mm_s;
        }
        self.stats.est_seconds += f64::from(delay.us) * 1e-6;
        self.pos = u;
        Ok(())
    }

    fn set_power(&mut self, raw: u16) -> Result<(), CompileError> {
        if self.cur.power == Some(raw) {
            return Ok(());
        }
        let op = if params::is_fiber(self.dev.source) {
            ListOp::MarkCurrent(raw)
        } else {
            ListOp::PowerRatio(raw)
        };
        self.push(op)?;
        self.cur.power = Some(raw);
        Ok(())
    }

    fn bad<T>(block: usize, what: impl Into<String>) -> Result<T, CompileError> {
        Err(CompileError::BadParam {
            block,
            what: what.into(),
        })
    }

    /// Valida y convierte los parámetros del bloque, y emite los comandos que cambian.
    /// Devuelve la frecuencia en kHz.
    fn emit_block_params(
        &mut self,
        bi: usize,
        block: &ToolpathBlock,
        p: &GalvoParams,
    ) -> Result<f64, CompileError> {
        let dev = self.dev;
        let fin = |v: f64| v.is_finite();
        if !(fin(p.frequency_khz) && p.frequency_khz > 0.0) {
            return Self::bad(bi, "la frecuencia no es válida");
        }
        if p.frequency_khz < dev.freq_min_khz - 1e-9 || p.frequency_khz > dev.freq_max_khz + 1e-9 {
            return Self::bad(
                bi,
                format!(
                    "frecuencia {} kHz fuera del rango del perfil ({} a {} kHz)",
                    p.frequency_khz, dev.freq_min_khz, dev.freq_max_khz
                ),
            );
        }
        let Some(period) = params::period(dev.source, p.frequency_khz) else {
            return Self::bad(bi, "la frecuencia no cabe en el periodo de la tarjeta");
        };
        // Pulso: obligatorio y de la lista en MOPA; opcional (pero de la lista) en UV; ninguno en
        // el resto.
        match (dev.source, p.pulse_width_ns) {
            (LaserSource::FiberMopa, None) => {
                return Self::bad(bi, "una fuente MOPA necesita el ancho de pulso");
            }
            (LaserSource::FiberQ | LaserSource::Co2, Some(ns)) => {
                return Self::bad(
                    bi,
                    format!("esta fuente no usa ancho de pulso (se pidieron {ns} ns)"),
                );
            }
            _ => {}
        }
        if let Some(ns) = p.pulse_width_ns {
            if !dev.pulse_widths_ns.contains(&ns) {
                return Self::bad(
                    bi,
                    format!(
                        "ancho de pulso {ns} ns que no está en la lista del perfil {:?}",
                        dev.pulse_widths_ns
                    ),
                );
            }
            if ns > u32::from(u16::MAX) {
                return Self::bad(bi, "ancho de pulso demasiado grande");
            }
        }
        for (v, w) in [
            (p.laser_on_us, "el retardo de encendido"),
            (p.laser_off_us, "el retardo de apagado"),
            (p.end_us, "el retardo final"),
            (p.polygon_us, "el retardo de polígono"),
            (p.jump_delay_min_us, "el retardo de salto mínimo"),
            (p.jump_delay_max_us, "el retardo de salto máximo"),
            (p.jump_distance_limit, "la distancia límite de salto"),
            (p.pause_before_ms, "la pausa previa"),
            (p.pause_after_ms, "la pausa posterior"),
        ] {
            if !fin(v) {
                return Self::bad(bi, format!("{w} no es un número"));
            }
        }
        for (v, w) in [
            (p.laser_on_us, "el retardo de encendido"),
            (p.laser_off_us, "el retardo de apagado"),
        ] {
            if v.abs() > 65535.0 {
                return Self::bad(bi, format!("{w} fuera de ±65535 us"));
            }
        }
        if p.jump_delay_min_us < 0.0
            || p.jump_delay_max_us < p.jump_delay_min_us
            || p.jump_delay_max_us > 65535.0
        {
            return Self::bad(
                bi,
                "retardos de salto inválidos (0 <= mín <= máx <= 65535 us)",
            );
        }
        if p.end_us < 0.0 || p.polygon_us < 0.0 || p.pause_before_ms < 0.0 || p.pause_after_ms < 0.0
        {
            return Self::bad(
                bi,
                "los retardos final, de polígono y las pausas no pueden ser negativos",
            );
        }
        let mark_mm_s = block.speed_mm_min / 60.0;
        let Some(mark_u) = params::speed_units_f(mark_mm_s, dev.field_mm) else {
            return Self::bad(bi, "la velocidad de marcado debe ser mayor que 0");
        };
        if mark_u < 1.0 {
            return Self::bad(
                bi,
                format!(
                    "velocidad de marcado {mark_mm_s:.3} mm/s demasiado baja para la tarjeta (mínimo 1 unidad/ms)"
                ),
            );
        }
        let Some(jump_u) = params::speed_units_f(p.jump_speed, dev.field_mm) else {
            return Self::bad(bi, "la velocidad de salto debe ser mayor que 0");
        };
        if jump_u < 1.0 {
            return Self::bad(bi, "velocidad de salto demasiado baja para la tarjeta");
        }
        if p.repeat > MAX_REPEAT {
            return Self::bad(
                bi,
                format!("{} repeticiones (máximo {MAX_REPEAT})", p.repeat),
            );
        }
        let clamp = |v: f64, what: &str, warnings: &mut Vec<String>| -> u16 {
            if v > 65535.0 {
                warnings.push(format!(
                    "bloque {bi}: {what} saturada al máximo de la tarjeta"
                ));
            }
            v.round().clamp(1.0, 65535.0) as u16
        };
        let ju = clamp(jump_u, "la velocidad de salto", &mut self.warnings);
        let mu = clamp(mark_u, "la velocidad de marcado", &mut self.warnings);
        self.jump_mm_s = f64::from(ju) / self.upmm * 1000.0;
        self.mark_mm_s = f64::from(mu) / self.upmm * 1000.0;

        if self.cur.jump_speed != Some(ju) {
            self.push(ListOp::JumpSpeed(ju))?;
            self.cur.jump_speed = Some(ju);
        }
        if params::is_fiber(dev.source) {
            if let Some(ns) = p
                .pulse_width_ns
                .filter(|_| dev.source == LaserSource::FiberMopa)
            {
                self.emit_pulse(ns as u16)?;
            }
            self.emit_freq(ListOp::QSwitchPeriod(period), period)?;
        } else {
            self.emit_freq(ListOp::MarkFreq(period), period)?;
            self.push(ListOp::Co2Fpk(params::fpk(p.frequency_khz)))?;
            if let Some(ns) = p.pulse_width_ns {
                self.emit_pulse(ns as u16)?;
            }
        }
        if self.cur.mark_speed != Some(mu) {
            self.push(ListOp::MarkSpeed(mu))?;
            self.cur.mark_speed = Some(mu);
        }
        let on = Delay::from_us(p.laser_on_us.round() as i64);
        if self.cur.delay_on != Some(on) {
            self.push(ListOp::LaserOnDelay(on))?;
            self.cur.delay_on = Some(on);
        }
        let off = Delay::from_us(p.laser_off_us.round() as i64);
        if self.cur.delay_off != Some(off) {
            self.push(ListOp::LaserOffDelay(off))?;
            self.cur.delay_off = Some(off);
        }
        // Retardo de polígono en pasos de 10 us (MeerK40t), redondeado hacia abajo: ante la
        // duda sobre la unidad, la espera más corta.
        let poly = (p.polygon_us / 10.0).floor().clamp(0.0, 65535.0) as u16;
        if self.cur.delay_poly != Some(poly) {
            self.push(ListOp::PolygonDelay(poly))?;
            self.cur.delay_poly = Some(poly);
        }
        self.end_ticks = (p.end_us / 10.0).round().clamp(0.0, 65535.0) as u16;
        Ok(p.frequency_khz)
    }

    fn emit_pulse(&mut self, ns: u16) -> Result<(), CompileError> {
        if self.cur.pulse != Some(ns) {
            self.push(ListOp::PulseWidth(ns))?;
            self.cur.pulse = Some(ns);
        }
        Ok(())
    }

    fn emit_freq(&mut self, op: ListOp, period: u16) -> Result<(), CompileError> {
        if self.cur.freq_period != Some(period) {
            self.push(op)?;
            self.cur.freq_period = Some(period);
            if !params::is_fiber(self.dev.source) {
                // La razón de potencia de CO2/UV depende de la frecuencia.
                self.cur.power = None;
            }
        }
        Ok(())
    }

    /// Potencia en el código de la tarjeta; 0 si no dispararía.
    fn power_raw(&self, bi: usize, pct: f64, freq_khz: f64) -> Result<u16, CompileError> {
        if !pct.is_finite() || pct < 0.0 {
            return Self::bad(bi, format!("potencia {pct} % inválida"));
        }
        if pct > self.dev.max_power_pct + 1e-9 {
            return Self::bad(
                bi,
                format!(
                    "potencia {pct} % por encima del máximo del perfil ({} %)",
                    self.dev.max_power_pct
                ),
            );
        }
        Ok(if params::is_fiber(self.dev.source) {
            params::fiber_current(pct)
        } else {
            params::power_ratio(pct, freq_khz)
        })
    }

    fn note_mark_bounds(&mut self, p: Point) {
        let b = self.stats.mark_bounds.get_or_insert([p, p]);
        b[0] = Point::new(b[0].x.min(p.x), b[0].y.min(p.y));
        b[1] = Point::new(b[1].x.max(p.x), b[1].y.max(p.y));
    }

    fn block(&mut self, bi: usize, block: &ToolpathBlock) -> Result<(), CompileError> {
        let Some(p) = block.galvo.as_ref() else {
            return Err(CompileError::MissingParams { block: bi });
        };
        if block.air_assist {
            self.warnings.push(format!(
                "bloque {bi}: la asistencia de aire se ignora en galvo"
            ));
        }
        if block.z_mm.is_some() {
            self.warnings
                .push(format!("bloque {bi}: el eje Z se ignora en galvo"));
        }
        self.finish_run()?;
        let freq = self.emit_block_params(bi, block, p)?;
        self.wait_ticks(params::ticks_from_ms(p.pause_before_ms))?;
        for _ in 0..p.repeat.max(1) {
            for mv in &block.moves {
                match *mv {
                    Move::Travel { to } => {
                        self.finish_run()?;
                        self.jump_to(to, p)?;
                    }
                    Move::Cut { to, power_pct } => {
                        let raw = self.power_raw(bi, power_pct, freq)?;
                        if raw == 0 {
                            self.finish_run()?;
                            self.stats.zero_power_cuts += 1;
                            self.jump_to(to, p)?;
                            continue;
                        }
                        let u = self.units(to)?;
                        if u == self.pos {
                            self.last_pt = Some(to);
                            continue;
                        }
                        self.set_power(raw)?;
                        let d = self.dist_units(u);
                        if let Some(from) = self.last_pt {
                            self.note_mark_bounds(from);
                        }
                        self.note_mark_bounds(to);
                        self.push(ListOp::MarkTo {
                            x: u.0,
                            y: u.1,
                            dist: d.round().min(65535.0) as u16,
                        })?;
                        self.in_run = true;
                        self.stats.marks += 1;
                        let dist_mm = d / self.upmm;
                        self.stats.mark_mm += dist_mm;
                        self.stats.est_seconds += dist_mm / self.mark_mm_s;
                        self.pos = u;
                        self.last_pt = Some(to);
                    }
                    Move::Dwell { to, ms, power_pct } => {
                        self.finish_run()?;
                        if !ms.is_finite() || ms < 0.0 {
                            return Self::bad(bi, format!("pausa de {ms} ms inválida"));
                        }
                        self.jump_to(to, p)?;
                        let raw = self.power_raw(bi, power_pct, freq)?;
                        let ticks = params::ticks_from_ms(ms);
                        if raw == 0 || ticks == 0 {
                            self.wait_ticks(ticks)?;
                            continue;
                        }
                        self.set_power(raw)?;
                        self.note_mark_bounds(to);
                        let mut left = ticks;
                        while left > 0 {
                            let t = left.min(u64::from(params::MAX_DWELL_TICKS)) as u16;
                            self.push(ListOp::LaserOnPoint { ticks: t })?;
                            left -= u64::from(t);
                        }
                        self.stats.dots += 1;
                        self.stats.est_seconds += ms / 1000.0;
                        if self.end_ticks > 0 {
                            self.push(ListOp::DelayTime {
                                ticks: self.end_ticks,
                            })?;
                        }
                    }
                }
            }
            self.finish_run()?;
        }
        self.wait_ticks(params::ticks_from_ms(p.pause_after_ms))?;
        Ok(())
    }

    fn finish(mut self, kind: ListKind, lim: &SafetyLimits) -> Result<CompiledList, CompileError> {
        self.push(ListOp::EndOfList)?;
        self.stats.commands = self.cmds.len();
        self.stats.batches = wire::batch_count(self.cmds.len());
        safety::validate_list(&self.cmds, kind, lim)?;
        Ok(CompiledList {
            commands: self.cmds,
            kind,
            stats: self.stats,
            warnings: self.warnings,
        })
    }
}

/// Compila un `Toolpath` para el perfil. Todos los bloques deben traer `GalvoParams`.
pub fn compile(
    tp: &Toolpath,
    dev: &GalvoDevice,
    opts: &CompileOptions,
) -> Result<CompiledList, CompileError> {
    if opts.kind == ListKind::Light {
        return Err(CompileError::WrongKind);
    }
    let lim = SafetyLimits::from_device(dev)?;
    if tp.blocks.iter().all(|b| b.moves.is_empty()) {
        return Err(CompileError::Empty);
    }
    let mut b = Builder::new(dev, opts.origin, opts.max_commands, opts.start_units)?;
    if dev.correction.cor_file.is_some() {
        b.warnings.push(
            "el archivo .cor del perfil se ignora: su formato se puede leer pero su semántica \
             no está documentada (ver cor.rs); la corrección es la paramétrica del perfil"
                .into(),
        );
    }
    b.push(ListOp::Ready)?;
    if params::is_fiber(dev.source) && opts.mo_delay_ms > 0.0 {
        b.wait_ticks(params::ticks_from_ms(opts.mo_delay_ms))?;
    }
    let port = if opts.kind == ListKind::Mark {
        lim.laser_mask()
    } else {
        lim.light_mask()
    };
    b.push(ListOp::WritePort(port))?;
    for (bi, block) in tp.blocks.iter().enumerate() {
        b.block(bi, block)?;
    }
    b.finish(opts.kind, &lim)
}

/// Qué dibujar con el puntero rojo.
#[derive(Debug, Clone, PartialEq)]
pub enum LightShape {
    /// El rectángulo que ocupa el trabajo.
    Rect { min: Point, max: Point },
    /// Los recorridos del trabajo (cada polilínea se recorre por sus vértices).
    Contour(Vec<Vec<Point>>),
}

impl LightShape {
    /// Caja de todos los movimientos de un `Toolpath`.
    pub fn bounds_of(tp: &Toolpath) -> Option<LightShape> {
        let mut it = tp.blocks.iter().flat_map(|b| b.moves.iter().map(Move::to));
        let first = it.next()?;
        let (mut min, mut max) = (first, first);
        for p in it {
            min = Point::new(min.x.min(p.x), min.y.min(p.y));
            max = Point::new(max.x.max(p.x), max.y.max(p.y));
        }
        Some(LightShape::Rect { min, max })
    }

    /// Los trazos de marca de un `Toolpath` (cada marca continua es una polilínea).
    pub fn contour_of(tp: &Toolpath) -> Option<LightShape> {
        let mut lines: Vec<Vec<Point>> = Vec::new();
        let mut current: Option<Vec<Point>> = None;
        let mut at: Option<Point> = None;
        for mv in tp.blocks.iter().flat_map(|b| b.moves.iter()) {
            match *mv {
                Move::Cut { to, power_pct } if power_pct > 0.0 => {
                    current
                        .get_or_insert_with(|| at.into_iter().collect())
                        .push(to);
                }
                _ => lines.extend(current.take()),
            }
            at = Some(mv.to());
        }
        lines.extend(current.take());
        lines.retain(|l| l.len() > 1);
        (!lines.is_empty()).then_some(LightShape::Contour(lines))
    }

    fn polylines(&self) -> Vec<Vec<Point>> {
        match self {
            LightShape::Rect { min, max } => vec![vec![
                *min,
                Point::new(max.x, min.y),
                *max,
                Point::new(min.x, max.y),
                *min,
            ]],
            LightShape::Contour(lines) => lines.clone(),
        }
    }
}

/// Máximo de vértices de un contorno de encuadre (se aligera si lo supera: es solo una guía).
pub const MAX_LIGHT_POINTS: usize = 4000;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LightOptions {
    pub origin: FieldOrigin,
}

/// Compila un encuadre: solo saltos con el puntero rojo; nunca marcas ni potencia.
pub fn compile_light(
    shape: &LightShape,
    dev: &GalvoDevice,
    opts: &LightOptions,
) -> Result<CompiledList, CompileError> {
    let lim = SafetyLimits::from_device(dev)?;
    let mut lines = shape.polylines();
    let total: usize = lines.iter().map(Vec::len).sum();
    let mut warnings = Vec::new();
    if total > MAX_LIGHT_POINTS {
        let keep_every = total.div_ceil(MAX_LIGHT_POINTS);
        for l in &mut lines {
            let last = l.last().copied();
            *l = l.iter().step_by(keep_every).copied().collect();
            if let Some(last) = last
                && l.last() != Some(&last)
            {
                l.push(last);
            }
        }
        warnings.push(format!(
            "el contorno tiene {total} vértices: se aligeró 1 de cada {keep_every} para el encuadre"
        ));
    }
    if lines.iter().all(Vec::is_empty) {
        return Err(CompileError::Empty);
    }
    let mut b = Builder::new(
        dev,
        opts.origin,
        safety::DEFAULT_MAX_COMMANDS,
        (0x8000, 0x8000),
    )?;
    b.warnings = warnings;
    let Some(speed) = params::speed_units_f(dev.light_speed, dev.field_mm).filter(|s| *s >= 1.0)
    else {
        return Err(CompileError::BadParam {
            block: 0,
            what: "la velocidad del encuadre del perfil debe ser mayor que 0".into(),
        });
    };
    let ju = speed.round().clamp(1.0, 65535.0) as u16;
    b.jump_mm_s = f64::from(ju) / b.upmm * 1000.0;
    b.push(ListOp::Ready)?;
    b.push(ListOp::WritePort(lim.light_mask()))?;
    b.push(ListOp::JumpSpeed(ju))?;
    b.cur.jump_speed = Some(ju);
    let p = params_from_device(dev, dev.freq_min_khz, None);
    for line in &lines {
        for pt in line {
            b.jump_to(*pt, &p)?;
        }
    }
    b.finish(ListKind::Light, &lim)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::ListOp;
    use tlaku_core::toolpath::{Move, PowerMode};

    fn dev() -> GalvoDevice {
        GalvoDevice {
            field_mm: 100.0,
            freq_min_khz: 20.0,
            freq_max_khz: 80.0,
            ..GalvoDevice::default()
        }
    }

    fn block(moves: Vec<Move>, dev: &GalvoDevice) -> ToolpathBlock {
        ToolpathBlock {
            label: "t".into(),
            speed_mm_min: 60_000.0, // 1000 mm/s
            air_assist: false,
            power_mode: PowerMode::Constant,
            moves,
            z_mm: None,
            galvo: Some(params_from_device(dev, 30.0, None)),
        }
    }

    fn travel(x: f64, y: f64) -> Move {
        Move::Travel {
            to: Point::new(x, y),
        }
    }

    fn cut(x: f64, y: f64, p: f64) -> Move {
        Move::Cut {
            to: Point::new(x, y),
            power_pct: p,
        }
    }

    fn ops(l: &CompiledList) -> Vec<ListOp> {
        l.commands.iter().map(ListOp::decode).collect()
    }

    #[test]
    fn a_square_becomes_a_jump_and_four_marks() {
        let d = dev();
        let tp = Toolpath {
            blocks: vec![block(
                vec![
                    travel(-10.0, -10.0),
                    cut(10.0, -10.0, 50.0),
                    cut(10.0, 10.0, 50.0),
                    cut(-10.0, 10.0, 50.0),
                    cut(-10.0, -10.0, 50.0),
                ],
                &d,
            )],
        };
        let l = compile(&tp, &d, &CompileOptions::default()).unwrap();
        let o = ops(&l);
        assert_eq!(o[0], ListOp::Ready);
        assert_eq!(o.last(), Some(&ListOp::EndOfList));
        assert_eq!(l.stats.marks, 4);
        assert_eq!(l.stats.jumps, 1);
        assert!((l.stats.mark_mm - 80.0).abs() < 0.01);
        // 100 mm de campo: 655,36 unidades/mm; -10 mm = 0x8000 - 6554.
        let first_jump = o
            .iter()
            .find_map(|c| match c {
                ListOp::JumpTo { x, y, .. } => Some((*x, *y)),
                _ => None,
            })
            .unwrap();
        assert_eq!(first_jump, (0x8000 - 6554, 0x8000 - 6554));
        // La potencia, la frecuencia y las velocidades se fijan antes de la primera marca.
        let first_mark = o
            .iter()
            .position(|c| matches!(c, ListOp::MarkTo { .. }))
            .unwrap();
        for want in [
            |c: &ListOp| matches!(c, ListOp::MarkCurrent(_)),
            |c: &ListOp| matches!(c, ListOp::QSwitchPeriod(667)),
            |c: &ListOp| matches!(c, ListOp::MarkSpeed(_)),
            |c: &ListOp| matches!(c, ListOp::LaserOnDelay(_)),
            |c: &ListOp| matches!(c, ListOp::LaserOffDelay(_)),
            |c: &ListOp| matches!(c, ListOp::PolygonDelay(_)),
            |c: &ListOp| matches!(c, ListOp::WritePort(1)),
        ] {
            assert!(o[..first_mark].iter().any(want));
        }
        // El comando de potencia no se repite entre marcas iguales.
        assert_eq!(
            o.iter()
                .filter(|c| matches!(c, ListOp::MarkCurrent(_)))
                .count(),
            1
        );
    }

    #[test]
    fn travels_are_jumps_and_never_marks() {
        let d = dev();
        let tp = Toolpath {
            blocks: vec![block(
                vec![
                    travel(0.0, 0.0),
                    cut(5.0, 0.0, 40.0),
                    travel(20.0, 20.0),
                    travel(-20.0, 20.0),
                    cut(-20.0, 10.0, 40.0),
                ],
                &d,
            )],
        };
        let l = compile(&tp, &d, &CompileOptions::default()).unwrap();
        assert_eq!(l.stats.marks, 2);
        // El primer viaje a (0, 0) ya está en el centro y no genera salto.
        assert_eq!(l.stats.jumps, 2);
    }

    #[test]
    fn zero_power_cuts_are_emitted_as_jumps() {
        let d = dev();
        let tp = Toolpath {
            blocks: vec![block(
                vec![travel(0.0, 0.0), cut(5.0, 0.0, 0.0), cut(10.0, 0.0, 30.0)],
                &d,
            )],
        };
        let l = compile(&tp, &d, &CompileOptions::default()).unwrap();
        assert_eq!(l.stats.zero_power_cuts, 1);
        assert_eq!(l.stats.marks, 1);
        // El segmento de 0 a 5 mm es un salto; solo el de 5 a 10 mm se marca.
        assert!((l.stats.mark_mm - 5.0).abs() < 0.01);
    }

    #[test]
    fn dwell_with_power_fires_in_chunks_and_without_waits() {
        let d = dev();
        let tp = Toolpath {
            blocks: vec![block(
                vec![
                    Move::Dwell {
                        to: Point::new(1.0, 1.0),
                        ms: 1300.0,
                        power_pct: 20.0,
                    },
                    Move::Dwell {
                        to: Point::new(1.0, 1.0),
                        ms: 50.0,
                        power_pct: 0.0,
                    },
                ],
                &d,
            )],
        };
        let l = compile(&tp, &d, &CompileOptions::default()).unwrap();
        let o = ops(&l);
        let dots: Vec<u16> = o
            .iter()
            .filter_map(|c| match c {
                ListOp::LaserOnPoint { ticks } => Some(*ticks),
                _ => None,
            })
            .collect();
        // 1300 ms = 130000 pasos de 10 us = 60000 + 60000 + 10000.
        assert_eq!(dots, vec![60000, 60000, 10000]);
        assert_eq!(l.stats.dots, 1);
        assert!(
            o.contains(&ListOp::DelayTime { ticks: 5000 }),
            "la espera sin potencia"
        );
    }

    #[test]
    fn repeat_expands_and_pauses_wrap_the_block() {
        let d = dev();
        let mut b = block(vec![travel(0.0, 0.0), cut(10.0, 0.0, 30.0)], &d);
        {
            let p = b.galvo.as_mut().unwrap();
            p.repeat = 3;
            p.pause_before_ms = 10.0;
            p.pause_after_ms = 20.0;
        }
        let tp = Toolpath { blocks: vec![b] };
        let l = compile(&tp, &d, &CompileOptions::default()).unwrap();
        let o = ops(&l);
        assert_eq!(l.stats.marks, 3);
        assert_eq!(
            l.stats.jumps, 2,
            "cada repetición vuelve al origen con un salto"
        );
        // La pausa previa viene antes de la primera marca y la posterior, justo antes del final.
        let first_mark = o
            .iter()
            .position(|c| matches!(c, ListOp::MarkTo { .. }))
            .unwrap();
        assert!(o[..first_mark].contains(&ListOp::DelayTime { ticks: 1000 }));
        assert_eq!(o[o.len() - 2], ListOp::DelayTime { ticks: 2000 });
    }

    #[test]
    fn end_delay_follows_each_stroke() {
        let d = dev();
        let tp = Toolpath {
            blocks: vec![block(
                vec![
                    travel(0.0, 0.0),
                    cut(5.0, 0.0, 30.0),
                    travel(0.0, 5.0),
                    cut(5.0, 5.0, 30.0),
                ],
                &d,
            )],
        };
        let l = compile(&tp, &d, &CompileOptions::default()).unwrap();
        let o = ops(&l);
        // end_us = 300 -> 30 pasos de 10 us, una vez por trazo.
        let n = o
            .iter()
            .filter(|c| **c == ListOp::DelayTime { ticks: 30 })
            .count();
        assert_eq!(n, 2);
        // Y siempre después de una marca, nunca antes de un salto sin marca.
        for (i, c) in o.iter().enumerate() {
            if *c == (ListOp::DelayTime { ticks: 30 }) {
                assert!(matches!(o[i - 1], ListOp::MarkTo { .. }));
            }
        }
    }

    #[test]
    fn jump_delay_interpolates_between_min_and_max() {
        let d = dev();
        let mut b = block(
            vec![
                travel(0.0, 0.0),
                travel(1.0, 0.0),
                travel(40.0, 0.0),
                cut(41.0, 0.0, 10.0),
            ],
            &d,
        );
        {
            let p = b.galvo.as_mut().unwrap();
            p.jump_delay_min_us = 100.0;
            p.jump_delay_max_us = 500.0;
            p.jump_distance_limit = 10.0;
        }
        let tp = Toolpath { blocks: vec![b] };
        let l = compile(&tp, &d, &CompileOptions::default()).unwrap();
        let delays: Vec<i32> = ops(&l)
            .iter()
            .filter_map(|c| match c {
                ListOp::JumpDelay(d) => Some(d.signed_us()),
                _ => None,
            })
            .collect();
        // 0 mm -> primer salto 0 desde el centro: (0,0) es el centro, no se emite;
        // 1 mm -> 100 + 400 x 0,1 = 140 us; 39 mm (> límite) -> 500 us.
        assert_eq!(delays, vec![140, 500]);
    }

    #[test]
    fn bottom_left_origin_shifts_by_half_the_field() {
        let d = dev();
        let tp = Toolpath {
            blocks: vec![block(vec![travel(0.0, 0.0), cut(10.0, 0.0, 30.0)], &d)],
        };
        let opts = CompileOptions {
            origin: FieldOrigin::BottomLeft,
            ..CompileOptions::default()
        };
        let l = compile(&tp, &d, &opts).unwrap();
        let jump = ops(&l)
            .into_iter()
            .find_map(|c| match c {
                ListOp::JumpTo { x, y, .. } => Some((x, y)),
                _ => None,
            })
            .unwrap();
        assert_eq!(jump, (0, 0), "la esquina inferior izquierda del campo");
    }

    #[test]
    fn center_at_matches_the_pipeline_convention() {
        // El pipeline centra el campo en el centro de la cama: bed 200 x 150 -> centro (100, 75).
        let d = dev();
        let tp = Toolpath {
            blocks: vec![block(vec![travel(100.0, 75.0), cut(110.0, 75.0, 30.0)], &d)],
        };
        let opts = CompileOptions {
            origin: FieldOrigin::CenterAt(Point::new(100.0, 75.0)),
            ..CompileOptions::default()
        };
        let l = compile(&tp, &d, &opts).unwrap();
        let o = ops(&l);
        // El primer punto es el centro: no hay salto; la marca llega a +10 mm.
        assert_eq!(l.stats.jumps, 0);
        assert!(o.contains(&ListOp::MarkTo {
            x: 0x8000 + 6554,
            y: 0x8000,
            dist: 6554
        }));
    }

    #[test]
    fn points_outside_the_field_fail() {
        let d = dev();
        let tp = Toolpath {
            blocks: vec![block(vec![travel(0.0, 0.0), cut(60.0, 0.0, 30.0)], &d)],
        };
        assert!(matches!(
            compile(&tp, &d, &CompileOptions::default()),
            Err(CompileError::Calib(CalibError::OutOfField {
                axis: 'X',
                ..
            }))
        ));
    }

    #[test]
    fn profile_limits_are_enforced_before_any_output() {
        let mut d = dev();
        d.max_power_pct = 40.0;
        let mk = |p: f64| Toolpath {
            blocks: vec![block(vec![travel(0.0, 0.0), cut(5.0, 0.0, p)], &d)],
        };
        compile(&mk(40.0), &d, &CompileOptions::default()).unwrap();
        let err = compile(&mk(41.0), &d, &CompileOptions::default()).unwrap_err();
        assert!(matches!(err, CompileError::BadParam { .. }), "{err}");
        assert!(compile(&mk(f64::NAN), &d, &CompileOptions::default()).is_err());
        assert!(compile(&mk(-1.0), &d, &CompileOptions::default()).is_err());

        // Frecuencia fuera del rango del perfil.
        for f in [10.0, 90.0, 0.0, f64::INFINITY] {
            let mut t = mk(10.0);
            t.blocks[0].galvo.as_mut().unwrap().frequency_khz = f;
            assert!(
                matches!(
                    compile(&t, &d, &CompileOptions::default()),
                    Err(CompileError::BadParam { .. })
                ),
                "{f}"
            );
        }
        // Velocidad cero o inalcanzable.
        let mut t = mk(10.0);
        t.blocks[0].speed_mm_min = 0.0;
        assert!(compile(&t, &d, &CompileOptions::default()).is_err());
        t.blocks[0].speed_mm_min = 0.01;
        assert!(compile(&t, &d, &CompileOptions::default()).is_err());
        // Sin parámetros de galvo.
        let mut t = mk(10.0);
        t.blocks[0].galvo = None;
        assert_eq!(
            compile(&t, &d, &CompileOptions::default()),
            Err(CompileError::MissingParams { block: 0 })
        );
        // Sin movimientos.
        let t = Toolpath { blocks: vec![] };
        assert_eq!(
            compile(&t, &d, &CompileOptions::default()),
            Err(CompileError::Empty)
        );
    }

    #[test]
    fn pulse_width_rules_per_source() {
        let mut d = dev();
        d.source = LaserSource::FiberMopa;
        d.pulse_widths_ns = vec![4, 9, 13];
        let with_pulse = |ns: Option<u32>, d: &GalvoDevice| {
            let mut b = block(vec![travel(0.0, 0.0), cut(5.0, 0.0, 10.0)], d);
            b.galvo.as_mut().unwrap().pulse_width_ns = ns;
            Toolpath { blocks: vec![b] }
        };
        let l = compile(&with_pulse(Some(9), &d), &d, &CompileOptions::default()).unwrap();
        assert!(ops(&l).contains(&ListOp::PulseWidth(9)));
        assert!(compile(&with_pulse(Some(5), &d), &d, &CompileOptions::default()).is_err());
        assert!(compile(&with_pulse(None, &d), &d, &CompileOptions::default()).is_err());
        // Q-switch: no admite pulso.
        d.source = LaserSource::FiberQ;
        assert!(compile(&with_pulse(Some(9), &d), &d, &CompileOptions::default()).is_err());
        compile(&with_pulse(None, &d), &d, &CompileOptions::default()).unwrap();
    }

    #[test]
    fn co2_compiles_with_its_own_commands() {
        let mut d = dev();
        d.source = LaserSource::Co2;
        let tp = Toolpath {
            blocks: vec![block(vec![travel(0.0, 0.0), cut(5.0, 0.0, 25.0)], &d)],
        };
        let l = compile(&tp, &d, &CompileOptions::default()).unwrap();
        let o = ops(&l);
        assert!(o.contains(&ListOp::MarkFreq(333)));
        assert!(
            o.contains(&ListOp::PowerRatio(167)),
            "200 x 25 / 30 = 166,7"
        );
        assert!(o.contains(&ListOp::Co2Fpk(67)));
        assert!(
            !o.iter()
                .any(|c| matches!(c, ListOp::MarkCurrent(_) | ListOp::QSwitchPeriod(_)))
        );
        // Sin espera de MO ni comandos de fibra en CO2.
        assert!(
            !o.iter()
                .any(|c| matches!(c, ListOp::DelayTime { ticks: 800 }))
        );
    }

    #[test]
    fn dry_run_uses_the_red_pointer_port_only() {
        let d = dev();
        let tp = Toolpath {
            blocks: vec![block(vec![travel(0.0, 0.0), cut(5.0, 0.0, 30.0)], &d)],
        };
        let opts = CompileOptions {
            kind: ListKind::DryRun,
            ..CompileOptions::default()
        };
        let l = compile(&tp, &d, &opts).unwrap();
        let ports: Vec<u16> = ops(&l)
            .iter()
            .filter_map(|c| match c {
                ListOp::WritePort(v) => Some(*v),
                _ => None,
            })
            .collect();
        assert_eq!(ports, vec![0x100]);
        let bad = CompileOptions {
            kind: ListKind::Light,
            ..CompileOptions::default()
        };
        assert_eq!(compile(&tp, &d, &bad), Err(CompileError::WrongKind));
    }

    #[test]
    fn warnings_cover_ignored_fields() {
        let mut d = dev();
        d.correction.cor_file = Some("lente.cor".into());
        let mut b = block(vec![travel(0.0, 0.0), cut(5.0, 0.0, 30.0)], &d);
        b.air_assist = true;
        b.z_mm = Some(3.0);
        let l = compile(
            &Toolpath { blocks: vec![b] },
            &d,
            &CompileOptions::default(),
        )
        .unwrap();
        assert_eq!(l.warnings.len(), 3);
    }

    #[test]
    fn command_cap_is_enforced() {
        let d = dev();
        let tp = Toolpath {
            blocks: vec![block(vec![travel(0.0, 0.0), cut(5.0, 0.0, 30.0)], &d)],
        };
        let opts = CompileOptions {
            max_commands: 5,
            ..CompileOptions::default()
        };
        assert!(matches!(
            compile(&tp, &d, &opts),
            Err(CompileError::TooManyCommands { .. })
        ));
    }

    #[test]
    fn compiled_lists_always_pass_the_safety_validator() {
        // Trabajo pseudoaleatorio con saltos, marcas, potencias distintas y dwells.
        let d = dev();
        let mut s = 7u64;
        let mut r = move || {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((s >> 33) as f64) / f64::from(1u32 << 31)
        };
        for _ in 0..20 {
            let mut moves = vec![travel(0.0, 0.0)];
            for _ in 0..200 {
                let (x, y) = (r() * 80.0 - 40.0, r() * 80.0 - 40.0);
                moves.push(match (r() * 4.0) as u8 {
                    0 => travel(x, y),
                    1 => Move::Dwell {
                        to: Point::new(x, y),
                        ms: r() * 50.0,
                        power_pct: r() * 100.0,
                    },
                    _ => cut(x, y, (r() * 100.0).floor()),
                });
            }
            let tp = Toolpath {
                blocks: vec![block(moves, &d)],
            };
            let l = compile(&tp, &d, &CompileOptions::default()).unwrap();
            let lim = SafetyLimits::from_device(&d).unwrap();
            safety::validate_list(&l.commands, ListKind::Mark, &lim).unwrap();
        }
    }

    #[test]
    fn light_lists_have_only_jumps() {
        let d = dev();
        let shape = LightShape::Rect {
            min: Point::new(-10.0, -5.0),
            max: Point::new(10.0, 5.0),
        };
        let l = compile_light(&shape, &d, &LightOptions::default()).unwrap();
        let o = ops(&l);
        assert_eq!(l.stats.marks, 0);
        assert_eq!(l.stats.jumps, 5);
        assert!(o.contains(&ListOp::WritePort(0x100)));
        assert!(!o.iter().any(|c| matches!(
            c,
            ListOp::MarkTo { .. } | ListOp::LaserOnPoint { .. } | ListOp::MarkCurrent(_)
        )));
        // 1 salto por esquina más el cierre.
        assert_eq!(l.kind, ListKind::Light);
    }

    #[test]
    fn contours_follow_the_cuts_and_are_thinned_when_huge() {
        let d = dev();
        let d_params = params_from_device(&d, 30.0, None);
        let tp = Toolpath {
            blocks: vec![ToolpathBlock {
                speed_mm_min: 6000.0,
                moves: vec![
                    travel(0.0, 0.0),
                    cut(10.0, 0.0, 30.0),
                    cut(10.0, 10.0, 30.0),
                    travel(-10.0, -10.0),
                    cut(-5.0, -10.0, 30.0),
                ],
                galvo: Some(d_params),
                ..Default::default()
            }],
        };
        let Some(LightShape::Contour(lines)) = LightShape::contour_of(&tp) else {
            panic!("contorno");
        };
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].len(), 3);
        // Un contorno enorme se aligera.
        let big: Vec<Point> = (0..10_000)
            .map(|i| Point::new(f64::from(i % 80) - 40.0, f64::from(i / 80 % 80) - 40.0))
            .collect();
        let l = compile_light(
            &LightShape::Contour(vec![big]),
            &d,
            &LightOptions::default(),
        )
        .unwrap();
        assert!(l.stats.jumps <= MAX_LIGHT_POINTS + 1);
        assert_eq!(l.warnings.len(), 1);
        assert!(matches!(
            LightShape::bounds_of(&tp),
            Some(LightShape::Rect { .. })
        ));
        assert!(LightShape::bounds_of(&Toolpath::default()).is_none());
    }
}
