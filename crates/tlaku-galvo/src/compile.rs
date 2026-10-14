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
use tlaku_i18n::tr;

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
    /// Desfase del puntero rojo respecto al haz, mm; solo se usa con `ListKind::DryRun`. En una
    /// marca real no se aplica nunca.
    pub pointer_offset: [f64; 2],
}

impl Default for CompileOptions {
    fn default() -> Self {
        CompileOptions {
            origin: FieldOrigin::Center,
            kind: ListKind::Mark,
            mo_delay_ms: 8.0,
            max_commands: safety::DEFAULT_MAX_COMMANDS,
            start_units: (0x8000, 0x8000),
            pointer_offset: [0.0, 0.0],
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
    /// De ellas, las recorridas como overscan (a la velocidad de marcado y sin retardo de salto).
    pub overscan_passes: usize,
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
            CompileError::MissingParams { block } => {
                write!(f, "{}", tr!("galvo.compile.missing_params", block = block))
            }
            CompileError::BadParam { block, what } => write!(
                f,
                "{}",
                tr!("galvo.compile.bad_param", block = block, what = what)
            ),
            CompileError::TooManyCommands { max } => {
                write!(f, "{}", tr!("galvo.compile.too_many_commands", max = max))
            }
            CompileError::Empty => write!(f, "{}", tr!("galvo.compile.empty")),
            CompileError::WrongKind => write!(f, "{}", tr!("galvo.compile.wrong_kind")),
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
    /// Desfase (mm) que se resta a cada punto: el del puntero rojo respecto al haz. Solo se usa
    /// con el puntero (encuadre y recorrido en rojo); en una marca real es siempre 0.
    shift: (f64, f64),
    /// Velocidad de salto vigente del bloque (unidades de la tarjeta); un paso de overscan la
    /// cambia por la de marcado y la restaura antes del siguiente salto.
    block_jump_u: Option<u16>,
    /// Velocidad de marcado vigente del bloque (unidades de la tarjeta).
    block_mark_u: Option<u16>,
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
            shift: (0.0, 0.0),
            block_jump_u: None,
            block_mark_u: None,
        })
    }

    /// Activa el desfase del puntero rojo (solo para listas que no marcan): el puntero cae en
    /// el haz más `offset`, así que se manda a cada punto menos el desfase.
    fn with_pointer_offset(mut self, offset: [f64; 2]) -> Result<Self, CompileError> {
        if !offset.iter().all(|v| v.is_finite())
            || offset
                .iter()
                .any(|v| v.abs() > safety::MAX_POINTER_OFFSET_MM)
        {
            return Err(CompileError::BadParam {
                block: 0,
                what: tr!(
                    "galvo.compile.pointer_offset",
                    max = safety::MAX_POINTER_OFFSET_MM
                ),
            });
        }
        self.shift = (offset[0], offset[1]);
        Ok(self)
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
        Ok(self
            .map
            .to_units((c.0 - self.shift.0, c.1 - self.shift.1))?)
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
        if let Some(ju) = self.block_jump_u
            && self.cur.jump_speed != Some(ju)
        {
            // Tras un paso de overscan: de vuelta a la velocidad de salto del bloque.
            self.push(ListOp::JumpSpeed(ju))?;
            self.cur.jump_speed = Some(ju);
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

    /// Overscan (M44): recorre `to` con el láser APAGADO (`JumpTo`, nunca `MarkTo`) a la velocidad
    /// de marcado y sin retardo de salto, para que los espejos ya vayan a la velocidad del trazo
    /// cuando el láser se encienda (o sigan a ella al apagarse). Sin verificar con hardware.
    fn overscan_pass(&mut self, to: Point) -> Result<(), CompileError> {
        let u = self.units(to)?;
        self.last_pt = Some(to);
        if u == self.pos {
            return Ok(());
        }
        let Some(mu) = self.block_mark_u else {
            return Err(CompileError::BadParam {
                block: 0,
                what: tr!("galvo.compile.overscan_needs_speed"),
            });
        };
        if self.cur.jump_speed != Some(mu) {
            self.push(ListOp::JumpSpeed(mu))?;
            self.cur.jump_speed = Some(mu);
        }
        let no_delay = Delay::from_us(0);
        if self.cur.delay_jump != Some(no_delay) {
            self.push(ListOp::JumpDelay(no_delay))?;
            self.cur.delay_jump = Some(no_delay);
        }
        let d = self.dist_units(u);
        self.push(ListOp::JumpTo {
            x: u.0,
            y: u.1,
            dist: d.round().min(65535.0) as u16,
        })?;
        self.stats.jumps += 1;
        self.stats.overscan_passes += 1;
        let dist_mm = d / self.upmm;
        self.stats.jump_mm += dist_mm;
        if self.mark_mm_s > 0.0 {
            self.stats.est_seconds += dist_mm / self.mark_mm_s;
        }
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
            return Self::bad(bi, tr!("galvo.compile.freq_invalid"));
        }
        if p.frequency_khz < dev.freq_min_khz - 1e-9 || p.frequency_khz > dev.freq_max_khz + 1e-9 {
            return Self::bad(
                bi,
                tr!(
                    "galvo.compile.freq_out_of_range",
                    khz = p.frequency_khz,
                    min = dev.freq_min_khz,
                    max = dev.freq_max_khz
                ),
            );
        }
        let Some(period) = params::period(dev.source, p.frequency_khz) else {
            return Self::bad(bi, tr!("galvo.compile.freq_period"));
        };
        // Pulso: obligatorio y de la lista en MOPA; opcional (pero de la lista) en UV; ninguno en
        // el resto.
        match (dev.source, p.pulse_width_ns) {
            (LaserSource::FiberMopa, None) => {
                return Self::bad(bi, tr!("galvo.compile.mopa_needs_pulse"));
            }
            (LaserSource::FiberQ | LaserSource::Co2, Some(ns)) => {
                return Self::bad(bi, tr!("galvo.compile.source_no_pulse", ns = ns));
            }
            _ => {}
        }
        if let Some(ns) = p.pulse_width_ns {
            if !dev.pulse_widths_ns.contains(&ns) {
                return Self::bad(
                    bi,
                    tr!(
                        "galvo.compile.pulse_not_in_list",
                        ns = ns,
                        list = format!("{:?}", dev.pulse_widths_ns)
                    ),
                );
            }
            if ns > u32::from(u16::MAX) {
                return Self::bad(bi, tr!("galvo.compile.pulse_too_big"));
            }
        }
        for (v, w) in [
            (p.laser_on_us, tr!("galvo.compile.what_laser_on_delay")),
            (p.laser_off_us, tr!("galvo.compile.what_laser_off_delay")),
            (p.end_us, tr!("galvo.compile.what_end_delay")),
            (p.polygon_us, tr!("galvo.compile.what_polygon_delay")),
            (
                p.jump_delay_min_us,
                tr!("galvo.compile.what_jump_delay_min"),
            ),
            (
                p.jump_delay_max_us,
                tr!("galvo.compile.what_jump_delay_max"),
            ),
            (
                p.jump_distance_limit,
                tr!("galvo.compile.what_jump_distance"),
            ),
            (p.pause_before_ms, tr!("galvo.compile.what_pause_before")),
            (p.pause_after_ms, tr!("galvo.compile.what_pause_after")),
        ] {
            if !fin(v) {
                return Self::bad(bi, tr!("galvo.compile.not_a_number", what = w));
            }
        }
        for (v, w) in [
            (p.laser_on_us, tr!("galvo.compile.what_laser_on_delay")),
            (p.laser_off_us, tr!("galvo.compile.what_laser_off_delay")),
        ] {
            if v.abs() > 65535.0 {
                return Self::bad(bi, tr!("galvo.compile.delay_out_of_range", what = w));
            }
        }
        if p.jump_delay_min_us < 0.0
            || p.jump_delay_max_us < p.jump_delay_min_us
            || p.jump_delay_max_us > 65535.0
        {
            return Self::bad(bi, tr!("galvo.compile.jump_delays_invalid"));
        }
        if p.end_us < 0.0 || p.polygon_us < 0.0 || p.pause_before_ms < 0.0 || p.pause_after_ms < 0.0
        {
            return Self::bad(bi, tr!("galvo.compile.delays_negative"));
        }
        let mark_mm_s = block.speed_mm_min / 60.0;
        let Some(mark_u) = params::speed_units_f(mark_mm_s, dev.field_mm) else {
            return Self::bad(bi, tr!("galvo.compile.mark_speed_zero"));
        };
        if mark_u < 1.0 {
            return Self::bad(
                bi,
                tr!(
                    "galvo.compile.mark_speed_low",
                    speed = format!("{mark_mm_s:.3}")
                ),
            );
        }
        let Some(jump_u) = params::speed_units_f(p.jump_speed, dev.field_mm) else {
            return Self::bad(bi, tr!("galvo.compile.jump_speed_zero"));
        };
        if jump_u < 1.0 {
            return Self::bad(bi, tr!("galvo.compile.jump_speed_low"));
        }
        if p.repeat > MAX_REPEAT {
            return Self::bad(
                bi,
                tr!(
                    "galvo.compile.too_many_repeats",
                    n = p.repeat,
                    max = MAX_REPEAT
                ),
            );
        }
        let clamp = |v: f64, what: String, warnings: &mut Vec<String>| -> u16 {
            if v > 65535.0 {
                warnings.push(tr!("galvo.compile.speed_clamped", block = bi, what = what));
            }
            v.round().clamp(1.0, 65535.0) as u16
        };
        let ju = clamp(
            jump_u,
            tr!("galvo.compile.what_jump_speed"),
            &mut self.warnings,
        );
        let mu = clamp(
            mark_u,
            tr!("galvo.compile.what_mark_speed"),
            &mut self.warnings,
        );
        self.jump_mm_s = f64::from(ju) / self.upmm * 1000.0;
        self.mark_mm_s = f64::from(mu) / self.upmm * 1000.0;
        self.block_jump_u = Some(ju);
        self.block_mark_u = Some(mu);

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
            return Self::bad(bi, tr!("galvo.compile.power_invalid", pct = pct));
        }
        if pct > self.dev.max_power_pct + 1e-9 {
            return Self::bad(
                bi,
                tr!(
                    "galvo.compile.power_above",
                    pct = pct,
                    max = self.dev.max_power_pct
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
            self.warnings
                .push(tr!("galvo.compile.air_assist_ignored", block = bi));
        }
        if block.z_mm.is_some() {
            self.warnings
                .push(tr!("galvo.compile.z_ignored", block = bi));
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
                            if self.dev.overscan_mm > 0.0 {
                                self.overscan_pass(to)?;
                            } else {
                                self.jump_to(to, p)?;
                            }
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
                            return Self::bad(bi, tr!("galvo.compile.dwell_invalid", ms = ms));
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
    if opts.kind == ListKind::DryRun {
        // El recorrido en rojo pasa por donde marcará el láser: se compensa el desfase del puntero.
        b = b.with_pointer_offset(opts.pointer_offset)?;
    }
    if dev.correction.cor_file.is_some() {
        b.warnings.push(tr!("galvo.compile.cor_ignored"));
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

    /// Puntos donde el trabajo marca: extremos de cada tramo con potencia y puntos con disparo.
    fn marked_points(tp: &Toolpath) -> Vec<Point> {
        let mut pts = Vec::new();
        let mut at: Option<Point> = None;
        for mv in tp.blocks.iter().flat_map(|b| b.moves.iter()) {
            match *mv {
                Move::Cut { to, power_pct } if power_pct > 0.0 => {
                    pts.extend(at.take());
                    pts.push(to);
                }
                Move::Dwell { to, power_pct, .. } if power_pct > 0.0 => pts.push(to),
                _ => {}
            }
            at = Some(mv.to());
        }
        pts
    }

    /// Casco convexo de lo que el trabajo marca (cerrado). Es un encuadre más ajustado que el
    /// rectángulo para un diseño en diagonal o redondo. Con un solo punto o puntos alineados da
    /// el segmento que los une.
    pub fn hull_of(tp: &Toolpath) -> Option<LightShape> {
        let hull = convex_hull(Self::marked_points(tp));
        match hull.len() {
            0 | 1 => None,
            2 => Some(LightShape::Contour(vec![hull])),
            _ => {
                let mut ring = hull;
                ring.push(ring[0]);
                Some(LightShape::Contour(vec![ring]))
            }
        }
    }

    /// Un rectángulo por cada objeto suelto del trabajo: se agrupan los trazos cuyas cajas (con
    /// `gap_mm` de holgura) se tocan, y cada grupo se encuadra por separado. Devuelve la forma y
    /// cuántos objetos hay. Los rectángulos se recorren de uno al más cercano.
    pub fn objects_of(tp: &Toolpath, gap_mm: f64) -> Option<(LightShape, usize)> {
        let Some(LightShape::Contour(lines)) = Self::contour_of(tp) else {
            return None;
        };
        let gap = if gap_mm.is_finite() {
            gap_mm.max(0.0)
        } else {
            0.0
        };
        let mut boxes: Vec<[f64; 4]> = lines
            .iter()
            .map(|l| {
                l.iter()
                    .fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| {
                        [b[0].min(p.x), b[1].min(p.y), b[2].max(p.x), b[3].max(p.y)]
                    })
            })
            .collect();
        // Se funden las cajas que se tocan hasta que ya no haya ninguna.
        let touches = |a: &[f64; 4], b: &[f64; 4]| {
            a[0] - gap <= b[2] && b[0] - gap <= a[2] && a[1] - gap <= b[3] && b[1] - gap <= a[3]
        };
        loop {
            let mut merged = false;
            'scan: for i in 0..boxes.len() {
                for j in (i + 1)..boxes.len() {
                    if touches(&boxes[i], &boxes[j]) {
                        let b = boxes.remove(j);
                        let a = &mut boxes[i];
                        *a = [
                            a[0].min(b[0]),
                            a[1].min(b[1]),
                            a[2].max(b[2]),
                            a[3].max(b[3]),
                        ];
                        merged = true;
                        break 'scan;
                    }
                }
            }
            if !merged {
                break;
            }
        }
        let count = boxes.len();
        // Del más abajo-izquierda al siguiente más cercano.
        let mut order: Vec<[f64; 4]> = Vec::with_capacity(count);
        let mut cur = Point::new(f64::MIN, f64::MIN);
        while !boxes.is_empty() {
            let (k, _) = boxes
                .iter()
                .enumerate()
                .map(|(k, b)| {
                    let d = if order.is_empty() {
                        b[0] + b[1]
                    } else {
                        (b[0] - cur.x).hypot(b[1] - cur.y)
                    };
                    (k, d)
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))?;
            let b = boxes.remove(k);
            cur = Point::new(b[0], b[1]);
            order.push(b);
        }
        let rings = order
            .iter()
            .map(|b| {
                vec![
                    Point::new(b[0], b[1]),
                    Point::new(b[2], b[1]),
                    Point::new(b[2], b[3]),
                    Point::new(b[0], b[3]),
                    Point::new(b[0], b[1]),
                ]
            })
            .collect();
        Some((LightShape::Contour(rings), count))
    }

    /// La misma forma movida y escalada (encuadre en vivo): se escala desde el centro de su caja
    /// y luego se desplaza. Con el ajuste neutro devuelve una copia.
    pub fn adjusted(&self, a: &LightAdjust) -> LightShape {
        if a.is_neutral() {
            return self.clone();
        }
        let (c, _) = self.center_and_extent();
        let f = |p: Point| {
            Point::new(
                c.x + (p.x - c.x) * a.scale + a.dx,
                c.y + (p.y - c.y) * a.scale + a.dy,
            )
        };
        match self {
            LightShape::Rect { min, max } => {
                let (p, q) = (f(*min), f(*max));
                LightShape::Rect {
                    min: Point::new(p.x.min(q.x), p.y.min(q.y)),
                    max: Point::new(p.x.max(q.x), p.y.max(q.y)),
                }
            }
            LightShape::Contour(lines) => LightShape::Contour(
                lines
                    .iter()
                    .map(|l| l.iter().map(|p| f(*p)).collect())
                    .collect(),
            ),
        }
    }

    /// Centro y tamaño (ancho, alto) de la caja de la forma.
    pub fn center_and_extent(&self) -> (Point, (f64, f64)) {
        let (min, max) = match self {
            LightShape::Rect { min, max } => (*min, *max),
            LightShape::Contour(lines) => {
                let mut it = lines.iter().flatten();
                let first = it.next().copied().unwrap_or(Point::new(0.0, 0.0));
                it.fold((first, first), |(lo, hi), p| {
                    (
                        Point::new(lo.x.min(p.x), lo.y.min(p.y)),
                        Point::new(hi.x.max(p.x), hi.y.max(p.y)),
                    )
                })
            }
        };
        (
            Point::new((min.x + max.x) / 2.0, (min.y + max.y) / 2.0),
            (max.x - min.x, max.y - min.y),
        )
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

/// Ajuste en vivo del encuadre: desplazamiento (mm) y escala respecto al centro de la forma.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightAdjust {
    pub dx: f64,
    pub dy: f64,
    pub scale: f64,
}

impl Default for LightAdjust {
    fn default() -> Self {
        LightAdjust {
            dx: 0.0,
            dy: 0.0,
            scale: 1.0,
        }
    }
}

impl LightAdjust {
    /// Escala mínima y máxima aceptadas (un error de dedo no debe mandar el haz muy lejos).
    pub const SCALE_RANGE: (f64, f64) = (0.05, 20.0);
    /// Desplazamiento máximo, mm.
    pub const MAX_SHIFT_MM: f64 = 1000.0;

    pub fn is_neutral(&self) -> bool {
        *self == LightAdjust::default()
    }

    /// Valores finitos y dentro de los rangos.
    pub fn validate(&self) -> Result<(), String> {
        if !(self.dx.is_finite() && self.dy.is_finite() && self.scale.is_finite()) {
            return Err(tr!("galvo.compile.adjust_not_number"));
        }
        if self.dx.abs() > Self::MAX_SHIFT_MM || self.dy.abs() > Self::MAX_SHIFT_MM {
            return Err(tr!("galvo.compile.adjust_shift", max = Self::MAX_SHIFT_MM));
        }
        if !(Self::SCALE_RANGE.0..=Self::SCALE_RANGE.1).contains(&self.scale) {
            return Err(tr!(
                "galvo.compile.adjust_scale",
                min = Self::SCALE_RANGE.0,
                max = Self::SCALE_RANGE.1
            ));
        }
        Ok(())
    }
}

/// Casco convexo (cadena monótona de Andrew), sin repetir el primer punto; sentido antihorario.
fn convex_hull(mut pts: Vec<Point>) -> Vec<Point> {
    pts.retain(|p| p.x.is_finite() && p.y.is_finite());
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    pts.dedup_by(|a, b| a.x == b.x && a.y == b.y);
    if pts.len() < 3 {
        return pts;
    }
    let cross =
        |o: Point, a: Point, b: Point| (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
    let mut hull: Vec<Point> = Vec::with_capacity(pts.len() * 2);
    for &p in &pts {
        while hull.len() >= 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
            hull.pop();
        }
        hull.push(p);
    }
    let lower = hull.len() + 1;
    for &p in pts.iter().rev().skip(1) {
        while hull.len() >= lower && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
            hull.pop();
        }
        hull.push(p);
    }
    hull.pop();
    hull
}

/// Máximo de vértices de un contorno de encuadre (se aligera si lo supera: es solo una guía).
pub const MAX_LIGHT_POINTS: usize = 4000;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LightOptions {
    pub origin: FieldOrigin,
    /// Dónde cae el puntero rojo respecto al haz, mm (el puntero cae en haz + desfase). El
    /// encuadre lo compensa para que el puntero pase por donde marcará el láser.
    pub pointer_offset: [f64; 2],
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
        warnings.push(tr!(
            "galvo.compile.light_thinned",
            total = total,
            keep_every = keep_every
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
    )?
    .with_pointer_offset(opts.pointer_offset)?;
    b.warnings = warnings;
    let Some(speed) = params::speed_units_f(dev.light_speed, dev.field_mm).filter(|s| *s >= 1.0)
    else {
        return Err(CompileError::BadParam {
            block: 0,
            what: tr!("galvo.compile.light_speed_zero"),
        });
    };
    let ju = speed.round().clamp(1.0, 65535.0) as u16;
    b.jump_mm_s = f64::from(ju) / b.upmm * 1000.0;
    b.push(ListOp::Ready)?;
    b.push(ListOp::WritePort(lim.light_mask()))?;
    b.push(ListOp::JumpSpeed(ju))?;
    b.cur.jump_speed = Some(ju);
    b.block_jump_u = Some(ju);
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
