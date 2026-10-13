//! De `Toolpath` a archivo `.rd` (flujo de comandos Ruida con cabecera, capas, cuerpo y pie).
//!
//! La estructura sale del ejemplo 1 de EduTech (ruida.md sección 5), sustituyendo solo los
//! valores conocidos. No hay una especificación formal: varios campos de la cabecera (matrices,
//! índices y nombre de elemento, offsets) tienen significado incierto y por eso son opciones con
//! los valores del ejemplo (marcados `SIN CONFIRMAR`).
//!
//! Orden del flujo generado:
//! 1. Arranque: `D8 10`, `F0`, `E6 01`, `F1 02 00` y, opcionalmente, `D8 00`.
//! 2. Cajas del trabajo (`E7 06/03/07/50/51/04/05`).
//! 3. Cabecera de cada capa (`C9 04`, `C6 31..38`, `CA 06`, `CA 41`, `E7 52/53/61/62`).
//! 4. `CA 22`, offsets y bloque de elemento (`E7 54/55`, `F1`, `F2`).
//! 5. `EA`, `E7 60/13/17/23/24/08`.
//! 6. Cuerpo de cada capa: `CA 01 00`, `CA 02`, `CA 01 30/10/12|13`, velocidad, potencias,
//!    `CA 03 0D`, `CA 10 00` y los movimientos.
//! 7. Pie: `EB`, `E7 00`, suma `E5 05` (opcional), `DA 01 06 20` (opcional) y `D7`.
//!
//! Cada `ToolpathBlock` con al menos un corte es una capa. Los bloques sin cortes se omiten.
//!
//! No soportado todavía: **potencia variable dentro de un bloque** (ráster, overscan con
//! potencia 0 mezclada con potencia > 0). `C7`/`C8` no tienen semántica documentada, así que un
//! bloque cuyos cortes no comparten potencia devuelve [`RdError::VariablePower`]. Tampoco el eje
//! Z ni el rotativo.

use crate::cmd::{self, Command, Limit, ProcessOp};
use crate::num::{self, NumError};
use crate::safety::{self, SafetyError, SafetyLimits};
use crate::swizzle::Magic;
use std::fmt;
use tlaku_core::Point;
use tlaku_core::toolpath::{Move, Toolpath};

/// Capas posibles (int7).
pub const MAX_LAYERS: usize = 128;

/// Ajustes de una capa que el `Toolpath` no trae.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LayerOptions {
    /// Color RGB de la capa (en el archivo va como BBGGRR).
    pub color: [u8; 3],
    /// Potencia mínima en %. `None` = igual a la máxima (potencia constante). Con valores
    /// distintos la controladora escala con la velocidad (comportamiento de foros, ruida.md 4.2;
    /// SIN CONFIRMAR).
    pub min_power_pct: Option<f64>,
}

/// Qué suma `E5 05` lleva al final del trabajo. Duda abierta (ruida.md 4.4): `ruida-pa` dice que
/// es una suma corrida de los bytes de grabado/corte y de configuración de capa, y su cálculo no
/// coincide con las capturas de LightBurn (diferencia de 220); el ejemplo de EduTech no la
/// incluye.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileChecksum {
    /// No emitir `E5 05` (como el ejemplo de EduTech).
    #[default]
    None,
    /// Suma de los valores de todos los bytes desde el inicio del flujo hasta `E7 00` inclusive,
    /// sin contar los comandos `DA`. EXPERIMENTAL, SIN CONFIRMAR.
    SumToEndBlock,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuidaJobOptions {
    /// Límites de seguridad; se comprueban antes y después de generar.
    pub limits: SafetyLimits,
    /// Ajustes por bloque del `Toolpath` (mismo índice); los que falten usan los de por defecto.
    pub layers: Vec<LayerOptions>,
    /// Invierte Y respecto a la altura de la cama (`y' = alto - y`). Por defecto no: el
    /// sistema de coordenadas de Ruida (origen y signo de Y) no está documentado. SIN CONFIRMAR.
    pub flip_y: bool,
    /// Desplazamiento en mm que se suma a todas las coordenadas (después de invertir Y).
    pub offset_mm: [f64; 2],
    /// Usar movimientos relativos cuando el salto cabe en 14 bits con signo (+-8,191 mm).
    pub use_relative: bool,
    /// Usar `8A/8B/AA/AB` cuando un solo eje cambia. Fuentes menos firmes que `89/A9`: apagado.
    pub use_axis_shortcuts: bool,
    /// Emitir `D8 00` (iniciar proceso) al principio como el ejemplo. Si la subida se corta,
    /// la máquina podría haber empezado (ruida.md sección 7). FabLab/JNW lo omiten.
    pub emit_process_start: bool,
    pub file_checksum: FileChecksum,
    /// Emitir `DA 01 06 20 L L` en el pie, con L = longitud de corte en mm (inferencia propia a
    /// partir del ejemplo: 327 mm de perímetro). Es una **escritura de memoria**: apagado.
    pub footer_stats: bool,
    /// Potencia de los láseres 2 a 4 en %. 0 por seguridad; el ejemplo usa ~30 %.
    pub secondary_laser_power_pct: f64,
    /// `E7 54 n` para n = 0 y 1, µm (el ejemplo usa 50 000). SIN CONFIRMAR.
    pub pen_offset_um: [i64; 2],
    /// `E7 55 n` para n = 0 y 1, µm (el ejemplo usa 0). SIN CONFIRMAR.
    pub layer_offset_um: [i64; 2],
    /// `F1 03 X Y`, µm (el ejemplo usa 50 000, 50 000). SIN CONFIRMAR.
    pub screen_offset_um: [i64; 2],
    /// `F2 02`: nombre de elemento "codificado"; por defecto los bytes del ejemplo. SIN CONFIRMAR.
    pub element_name: [u8; 10],
    /// `F2 05` y `E7 08`: matriz del elemento; por defecto la del ejemplo. SIN CONFIRMAR.
    pub element_array: [i32; 7],
}

impl Default for RuidaJobOptions {
    fn default() -> Self {
        RuidaJobOptions {
            limits: SafetyLimits::default(),
            layers: Vec::new(),
            flip_y: false,
            offset_mm: [0.0, 0.0],
            use_relative: true,
            use_axis_shortcuts: false,
            emit_process_start: true,
            file_checksum: FileChecksum::None,
            footer_stats: false,
            secondary_laser_power_pct: 0.0,
            pen_offset_um: [0, 0],
            layer_offset_um: [0, 0],
            screen_offset_um: [0, 0],
            element_name: [0x05, 0x2A, 0x39, 0x1C, 0x41, 0x04, 0x6A, 0x15, 0x08, 0x20],
            element_array: [1, 1, -1, -710, -7808, 4, 7597],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RdError {
    Safety(SafetyError),
    Num(NumError),
    /// Los cortes de un bloque no comparten potencia (ráster / overscan): no soportado todavía.
    VariablePower {
        block: usize,
        min_pct: f64,
        max_pct: f64,
    },
    /// No hay ningún bloque con cortes.
    EmptyJob,
    /// Más bloques con cortes que capas posibles (128).
    TooManyLayers {
        layers: usize,
    },
    /// La potencia mínima configurada es mayor que la máxima del bloque.
    MinPowerAboveMax {
        block: usize,
    },
    BadOption(&'static str),
}

impl fmt::Display for RdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RdError::Safety(e) => write!(f, "{e}"),
            RdError::Num(e) => write!(f, "{e}"),
            RdError::VariablePower {
                block,
                min_pct,
                max_pct,
            } => write!(
                f,
                "bloque {block}: la potencia varía entre {min_pct:.2} % y {max_pct:.2} % dentro del bloque; \
                 el ráster y el overscan con potencia variable aún no están soportados en Ruida \
                 (C7/C8 sin semántica documentada). Divide el trabajo en capas de potencia constante"
            ),
            RdError::EmptyJob => write!(f, "el trabajo no tiene ningún corte"),
            RdError::TooManyLayers { layers } => write!(
                f,
                "el trabajo tiene {layers} capas con cortes y Ruida admite {MAX_LAYERS}"
            ),
            RdError::MinPowerAboveMax { block } => write!(
                f,
                "bloque {block}: la potencia mínima configurada supera la potencia del bloque"
            ),
            RdError::BadOption(w) => write!(f, "opción inválida: {w}"),
        }
    }
}

impl std::error::Error for RdError {}

impl From<SafetyError> for RdError {
    fn from(e: SafetyError) -> Self {
        RdError::Safety(e)
    }
}

impl From<NumError> for RdError {
    fn from(e: NumError) -> Self {
        RdError::Num(e)
    }
}

/// Un movimiento ya convertido a µm del archivo.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Step {
    cut: bool,
    x: i64,
    y: i64,
}

struct LayerPlan {
    /// Índice de capa en el archivo (0 a 127).
    index: u8,
    speed_um_s: u64,
    power_max: u16,
    power_min: u16,
    air: bool,
    color: u64,
    steps: Vec<Step>,
    bbox: BBox,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct BBox {
    min: (i64, i64),
    max: (i64, i64),
}

impl BBox {
    fn of(p: (i64, i64)) -> Self {
        BBox { min: p, max: p }
    }

    fn add(&mut self, p: (i64, i64)) {
        self.min = (self.min.0.min(p.0), self.min.1.min(p.1));
        self.max = (self.max.0.max(p.0), self.max.1.max(p.1));
    }

    fn union(&mut self, o: &BBox) {
        self.add(o.min);
        self.add(o.max);
    }
}

fn to_um(p: Point, opts: &RuidaJobOptions) -> Result<(i64, i64), NumError> {
    let y = if opts.flip_y {
        opts.limits.bed_mm[1] - p.y
    } else {
        p.y
    };
    Ok((
        num::mm_to_um(p.x + opts.offset_mm[0])?,
        num::mm_to_um(y + opts.offset_mm[1])?,
    ))
}

fn plan_layers(tp: &Toolpath, opts: &RuidaJobOptions) -> Result<Vec<LayerPlan>, RdError> {
    let mut plans = Vec::new();
    // Última posición emitida (µm) para descartar movimientos de longitud cero.
    let mut last: Option<(i64, i64)> = None;
    for (bi, block) in tp.blocks.iter().enumerate() {
        let powers: Vec<f64> = block
            .moves
            .iter()
            .filter_map(|m| match m {
                Move::Cut { power_pct, .. } => Some(*power_pct),
                Move::Travel { .. } => None,
            })
            .collect();
        if powers.is_empty() {
            continue;
        }
        // PowerMode::Dynamic se trata como constante: la semántica de mín/máx está sin validar.
        let raws = powers
            .iter()
            .map(|&p| num::power_from_pct(p))
            .collect::<Result<Vec<_>, _>>()?;
        let (lo, hi) = (*raws.iter().min().unwrap(), *raws.iter().max().unwrap());
        if lo != hi {
            let pmin = powers.iter().copied().fold(f64::INFINITY, f64::min);
            let pmax = powers.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            return Err(RdError::VariablePower {
                block: bi,
                min_pct: pmin,
                max_pct: pmax,
            });
        }
        let lopt = opts.layers.get(bi).cloned().unwrap_or_default();
        let power_min = match lopt.min_power_pct {
            Some(m) => {
                let r = num::power_from_pct(m)?;
                if r > hi {
                    return Err(RdError::MinPowerAboveMax { block: bi });
                }
                r
            }
            None => hi,
        };
        if plans.len() >= MAX_LAYERS {
            return Err(RdError::TooManyLayers {
                layers: plans.len() + 1,
            });
        }
        let mut steps = Vec::new();
        let mut bbox: Option<BBox> = None;
        for m in &block.moves {
            let p = to_um(m.to(), opts)?;
            match &mut bbox {
                Some(b) => b.add(p),
                None => bbox = Some(BBox::of(p)),
            }
            if last == Some(p) {
                continue;
            }
            last = Some(p);
            steps.push(Step {
                cut: matches!(m, Move::Cut { .. }),
                x: p.0,
                y: p.1,
            });
        }
        plans.push(LayerPlan {
            index: plans.len() as u8,
            speed_um_s: num::speed_from_mm_min(block.speed_mm_min)?,
            power_max: hi,
            power_min,
            air: block.air_assist,
            color: num::rgb_to_color(lopt.color[0], lopt.color[1], lopt.color[2]),
            steps,
            bbox: bbox.expect("el bloque tiene al menos un corte"),
        });
    }
    if plans.is_empty() {
        return Err(RdError::EmptyJob);
    }
    Ok(plans)
}

fn point2(op: u8, sub: u8, p: (i64, i64)) -> Command {
    Command::Point2 {
        op,
        sub,
        x: p.0,
        y: p.1,
    }
}

fn part_point(sub: u8, part: u8, p: (i64, i64)) -> Command {
    Command::PartPoint {
        sub,
        part,
        x: p.0,
        y: p.1,
    }
}

/// Instrucciones de movimiento de una capa: relativas si el salto cabe, absolutas si no. La
/// primera de cada capa es siempre absoluta y la posición se lleva en enteros (sin deriva).
fn emit_moves(steps: &[Step], opts: &RuidaJobOptions, out: &mut Vec<Command>) {
    let mut pos: Option<(i64, i64)> = None;
    for s in steps {
        let rel = match pos {
            Some((px, py)) if opts.use_relative => {
                let (dx, dy) = (s.x - px, s.y - py);
                let fits =
                    |d: i64| (i64::from(num::REL_MIN)..=i64::from(num::REL_MAX)).contains(&d);
                (fits(dx) && fits(dy)).then_some((dx as i32, dy as i32))
            }
            _ => None,
        };
        out.push(match (rel, s.cut) {
            (Some((dx, 0)), cut) if opts.use_axis_shortcuts => {
                if cut {
                    Command::CutRelX(dx)
                } else {
                    Command::MoveRelX(dx)
                }
            }
            (Some((0, dy)), cut) if opts.use_axis_shortcuts => {
                if cut {
                    Command::CutRelY(dy)
                } else {
                    Command::MoveRelY(dy)
                }
            }
            (Some((dx, dy)), true) => Command::CutRel { dx, dy },
            (Some((dx, dy)), false) => Command::MoveRel { dx, dy },
            (None, true) => Command::CutAbs { x: s.x, y: s.y },
            (None, false) => Command::MoveAbs { x: s.x, y: s.y },
        });
        pos = Some((s.x, s.y));
    }
}

/// Longitud de corte en mm (parte entera) a partir de los pasos de todas las capas.
fn cut_length_mm(plans: &[LayerPlan]) -> u64 {
    let mut total = 0.0f64;
    let mut pos: Option<(i64, i64)> = None;
    for p in plans {
        for s in &p.steps {
            if let (true, Some((px, py))) = (s.cut, pos) {
                total += ((s.x - px) as f64).hypot((s.y - py) as f64);
            }
            pos = Some((s.x, s.y));
        }
    }
    (total / 1000.0).floor() as u64
}

/// Genera la lista de comandos del trabajo (sin bytes ni swizzle).
#[allow(clippy::vec_init_then_push)]
pub fn build_commands(tp: &Toolpath, opts: &RuidaJobOptions) -> Result<Vec<Command>, RdError> {
    safety::check_toolpath(tp, &opts.limits)?;
    let sec = num::power_from_pct(opts.secondary_laser_power_pct)?;
    let plans = plan_layers(tp, opts)?;
    let mut job_box = plans[0].bbox;
    for p in &plans[1..] {
        job_box.union(&p.bbox);
    }
    let mut c: Vec<Command> = Vec::new();

    // 1. Arranque.
    c.push(Command::Process(ProcessOp::RefPoint));
    c.push(Command::SetReference);
    c.push(Command::Absolute);
    c.push(Command::Small {
        op: 0xF1,
        sub: 0x02,
        n: 0,
    });
    if opts.emit_process_start {
        c.push(Command::Process(ProcessOp::Start));
    }

    // 2. Cajas del trabajo.
    c.push(point2(0xE7, 0x06, (0, 0)));
    c.push(point2(0xE7, 0x03, job_box.min));
    c.push(point2(0xE7, 0x07, job_box.max));
    c.push(point2(0xE7, 0x50, job_box.min));
    c.push(point2(0xE7, 0x51, job_box.max));
    c.push(Command::Arr7 {
        op: 0xE7,
        sub: 0x04,
        v: [1, 1, 0, 0, 0, 0, 0],
    });
    c.push(Command::Small {
        op: 0xE7,
        sub: 0x05,
        n: 0,
    });

    // 3. Cabecera de cada capa.
    for p in &plans {
        let l = p.index;
        c.push(Command::LayerSpeed {
            layer: l,
            um_s: p.speed_um_s,
        });
        for (laser, mn, mx) in [
            (1, p.power_min, p.power_max),
            (2, sec, sec),
            (3, sec, sec),
            (4, sec, sec),
        ] {
            // Orden del ejemplo: 1 mín, 1 máx, 2 mín, 2 máx, 3 mín, 3 máx, 4 mín, 4 máx.
            c.push(Command::LayerPower {
                laser,
                limit: Limit::Min,
                layer: l,
                raw: mn,
            });
            c.push(Command::LayerPower {
                laser,
                limit: Limit::Max,
                layer: l,
                raw: mx,
            });
        }
        c.push(Command::LayerColor {
            layer: l,
            color: p.color,
        });
        c.push(Command::LayerMode { layer: l, mode: 0 });
        c.push(part_point(0x52, l, p.bbox.min));
        c.push(part_point(0x53, l, p.bbox.max));
        c.push(part_point(0x61, l, p.bbox.min));
        c.push(part_point(0x62, l, p.bbox.max));
    }

    // 4. Máximo de capa, offsets y bloque de elemento.
    c.push(Command::MaxLayer(plans.len() as u8 - 1));
    for n in 0..2u8 {
        c.push(Command::PartAbs {
            sub: 0x54,
            n,
            v: opts.pen_offset_um[n as usize],
        });
    }
    for n in 0..2u8 {
        c.push(Command::PartAbs {
            sub: 0x55,
            n,
            v: opts.layer_offset_um[n as usize],
        });
    }
    c.push(point2(
        0xF1,
        0x03,
        (opts.screen_offset_um[0], opts.screen_offset_um[1]),
    ));
    for (op, sub) in [(0xF1, 0x00), (0xF1, 0x01), (0xF2, 0x00), (0xF2, 0x01)] {
        c.push(Command::Small { op, sub, n: 0 });
    }
    c.push(Command::ElementName(opts.element_name));
    c.push(point2(0xF2, 0x03, job_box.min));
    c.push(point2(0xF2, 0x04, job_box.max));
    c.push(point2(0xF2, 0x06, (0, 0)));
    c.push(Command::Small {
        op: 0xF2,
        sub: 0x07,
        n: 0,
    });
    c.push(Command::Arr7 {
        op: 0xF2,
        sub: 0x05,
        v: opts.element_array,
    });

    // 5. Bloque de elemento del cuerpo.
    c.push(Command::ShortArg { op: 0xEA, n: 0 });
    c.push(Command::Small {
        op: 0xE7,
        sub: 0x60,
        n: 0,
    });
    c.push(point2(0xE7, 0x13, job_box.min));
    c.push(point2(0xE7, 0x17, job_box.max));
    c.push(point2(0xE7, 0x23, (0, 0)));
    c.push(Command::Small {
        op: 0xE7,
        sub: 0x24,
        n: 0,
    });
    c.push(Command::Arr7 {
        op: 0xE7,
        sub: 0x08,
        v: opts.element_array,
    });

    // 6. Cuerpo de cada capa.
    for p in &plans {
        c.push(Command::EndLayer);
        c.push(Command::LayerSelect(p.index));
        c.push(Command::LayerFlag(0x30));
        c.push(Command::LayerFlag(0x10));
        c.push(if p.air {
            Command::AirOn
        } else {
            Command::AirOff
        });
        c.push(Command::Speed(p.speed_um_s));
        c.push(Command::LaserTime {
            sub: 0x15,
            micros: 0,
        });
        c.push(Command::LaserTime {
            sub: 0x16,
            micros: 0,
        });
        for (laser, mn, mx) in [
            (1, p.power_min, p.power_max),
            (2, sec, sec),
            (3, sec, sec),
            (4, sec, sec),
        ] {
            c.push(Command::LaserPower {
                laser,
                limit: Limit::Min,
                raw: mn,
            });
            c.push(Command::LaserPower {
                laser,
                limit: Limit::Max,
                raw: mx,
            });
        }
        c.push(Command::Small {
            op: 0xCA,
            sub: 0x03,
            n: 0x0D,
        });
        c.push(Command::Small {
            op: 0xCA,
            sub: 0x10,
            n: 0x00,
        });
        emit_moves(&p.steps, opts, &mut c);
    }

    // 7. Pie.
    c.push(Command::Marker(0xEB));
    c.push(Command::EndBlock);
    if opts.file_checksum == FileChecksum::SumToEndBlock {
        let sum = sum_bytes(&c)?;
        c.push(Command::FileChecksum(sum));
    }
    if opts.footer_stats {
        let len = cut_length_mm(&plans);
        c.push(Command::MemWrite {
            addr: 0x0320,
            values: vec![len, len],
        });
    }
    c.push(Command::EndOfFile);
    Ok(c)
}

/// Suma de los bytes de los comandos (sin `DA`).
fn sum_bytes(cmds: &[Command]) -> Result<u64, NumError> {
    let mut sum = 0u64;
    for c in cmds {
        if matches!(c, Command::MemRead { .. } | Command::MemWrite { .. }) {
            continue;
        }
        sum += c.to_bytes()?.iter().map(|&b| u64::from(b)).sum::<u64>();
    }
    Ok(sum)
}

/// Genera los bytes del `.rd` **sin swizzle**. Antes de devolverlos verifica el flujo resultante
/// contra los límites de seguridad.
pub fn build_rd(tp: &Toolpath, opts: &RuidaJobOptions) -> Result<Vec<u8>, RdError> {
    let cmds = build_commands(tp, opts)?;
    safety::check_commands(&cmds, &opts.limits, opts.footer_stats)?;
    Ok(cmd::encode_stream(&cmds)?)
}

/// Aplica el swizzle a un flujo en claro (un `.rd` en disco y los datos por UDP/USB van así).
pub fn to_swizzled(plain: &[u8], magic: Magic) -> Vec<u8> {
    magic.swizzle_all(plain)
}

/// Inversa de [`to_swizzled`].
pub fn from_swizzled(wire: &[u8], magic: Magic) -> Vec<u8> {
    magic.unswizzle_all(wire)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_core::toolpath::ToolpathBlock;

    /// Ejemplo 1 de EduTech (ruida.md sección 5), bytes sin swizzle.
    const EXAMPLE_1: &str = "
        d810 f0 e601 f10200 d800
        e70600000000000000000000
        e70300000b51120000021258
        e707000011164c0000064e02
        e75000000b51120000021258
        e751000011164c0000064e02
        e7040001000100000000000000000000
        e70500
        c904000000060d20
        c631001319 c632001828
        c641002632 c642002632
        c635002632 c636002632 c637002632 c638002632
        ca06000000000000
        ca410000
        e7520000000b51120000021258 e75300000011164c0000064e02
        e7610000000b51120000021258 e76200000011164c0000064e02
        ca2200
        e754000000030650 e754010000030650
        e755000000000000 e755010000000000
        f10300000306500000030650
        f10000 f10100 f20000 f20100
        f202052a391c41046a150820
        f20300000b51120000021255 f20400001116500000064e03
        f20600000000000000000000 f20700
        f205000100017f7f7a3a430000043b2d
        ea00 e76000 e71300000b51120000021255 e71700001116500000064e03
        e72300000000000000000000 e72400
        e708000100017f7f7a3a430000043b2d
        ca0100 ca0200 ca0130 ca0110 ca0113
        c9020000060d20
        c6150000000000 c6160000000000
        c6011319 c6021828 c6212632 c6222632 c6052632 c6062632 c6072632 c6082632
        ca030d ca1000
        8800001116500000021256
        a800000b51120000021256
        a800000b51120000064e03
        a800001116500000064e03
        a800001116500000021256
        eb e700
        da01062000000002470000000247
        d7";

    fn hex(s: &str) -> Vec<u8> {
        let s: String = s.split_whitespace().collect();
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
            .collect()
    }

    fn pct(raw: u16) -> f64 {
        f64::from(raw) * 100.0 / 16384.0
    }

    fn rect_block(power: f64) -> ToolpathBlock {
        let c = |x: f64, y: f64| Move::Cut {
            to: Point::new(x, y),
            power_pct: power,
        };
        ToolpathBlock {
            label: "rectángulo".into(),
            speed_mm_min: 6000.0,
            air_assist: true,
            moves: vec![
                Move::Travel {
                    to: Point::new(281.424, 35.158),
                },
                c(190.610, 35.158),
                c(190.610, 108.291),
                c(281.424, 108.291),
                c(281.424, 35.158),
            ],
            ..Default::default()
        }
    }

    fn example_options() -> RuidaJobOptions {
        RuidaJobOptions {
            layers: vec![LayerOptions {
                color: [0, 0, 0],
                min_power_pct: Some(pct(2457)),
            }],
            secondary_laser_power_pct: pct(4914),
            pen_offset_um: [50_000, 50_000],
            screen_offset_um: [50_000, 50_000],
            footer_stats: true,
            ..Default::default()
        }
    }

    #[test]
    fn example_1_decodes_and_reencodes_byte_for_byte() {
        let bytes = hex(EXAMPLE_1);
        let cmds = cmd::decode_stream(&bytes).unwrap();
        assert!(
            cmds.iter().all(|c| !matches!(c, Command::Unknown { .. })),
            "todo el ejemplo debe estar tipado"
        );
        assert_eq!(cmd::encode_stream(&cmds).unwrap(), bytes);
        // Valores que EduTech decodifica a mano.
        assert!(cmds.contains(&Command::LayerSpeed {
            layer: 0,
            um_s: 100_000
        }));
        assert!(cmds.contains(&Command::AirOn));
        assert_eq!(cmds.last(), Some(&Command::EndOfFile));
    }

    #[test]
    fn example_1_regenerated_matches_except_for_documented_differences() {
        let expected = cmd::decode_stream(&hex(EXAMPLE_1)).unwrap();
        let tp = Toolpath {
            blocks: vec![rect_block(pct(3112))],
        };
        let ours = build_commands(&tp, &example_options()).unwrap();
        // Mismos comandos, mismo orden, mismo número.
        assert_eq!(ours.len(), expected.len());
        let mut diffs = Vec::new();
        for (i, (a, b)) in expected.iter().zip(&ours).enumerate() {
            if a != b {
                diffs.push((i, a.clone(), b.clone()));
            }
        }
        // Las únicas diferencias son las cajas: EduTech las trae con +-1 a 4 µm de diferencia
        // respecto a la trayectoria que dibuja (redondeo propio de RDWorks en milésimas de
        // pulgada). Aquí las calculamos exactamente desde las coordenadas. Ver ruida.md 5.
        for (i, a, b) in &diffs {
            let ok = |c: &Command| matches!(c, Command::Point2 { .. } | Command::PartPoint { .. });
            assert!(ok(a) && ok(b), "diferencia inesperada en {i}: {a} vs {b}");
        }
        // 4 + 4 (caja de proceso y de documento) + 8 (cajas de capa) + 4 (elemento) + 2 (E7 13/17)
        // menos los que coinciden por casualidad.
        assert!(!diffs.is_empty());
        println!("diferencias con el ejemplo 1: {}", diffs.len());
        for (i, a, b) in &diffs {
            println!("  #{i}: ejemplo {a}   nuestro {b}");
        }
        // La trayectoria, la velocidad, las potencias, el aire y el pie son idénticos.
        let tail = |v: &[Command]| v[v.len() - 12..].to_vec();
        assert_eq!(tail(&expected), tail(&ours));
    }

    #[test]
    fn example_with_exact_example_boxes_is_identical_in_bytes() {
        // Si la trayectoria y las cajas coinciden con las del ejemplo, los bytes son idénticos.
        let expected = hex(EXAMPLE_1);
        let tp = Toolpath {
            blocks: vec![rect_block(pct(3112))],
        };
        let mut cmds = build_commands(&tp, &example_options()).unwrap();
        let ex = cmd::decode_stream(&expected).unwrap();
        for (a, b) in cmds.iter_mut().zip(&ex) {
            if matches!(a, Command::Point2 { .. } | Command::PartPoint { .. }) {
                *a = b.clone();
            }
        }
        assert_eq!(cmd::encode_stream(&cmds).unwrap(), expected);
    }

    #[test]
    fn rd_is_valid_stream_and_roundtrips_through_swizzle() {
        let tp = Toolpath {
            blocks: vec![rect_block(40.0)],
        };
        let plain = build_rd(&tp, &RuidaJobOptions::default()).unwrap();
        assert!(plain.iter().filter(|&&b| b >= 0x80).count() > 20);
        let cmds = cmd::decode_stream(&plain).unwrap();
        assert!(cmds.iter().all(|c| !matches!(c, Command::Unknown { .. })));
        assert_eq!(*plain.last().unwrap(), 0xD7);
        // No hay escritura de memoria por defecto.
        assert!(!cmds.iter().any(|c| matches!(c, Command::MemWrite { .. })));
        let wire = to_swizzled(&plain, Magic::default());
        assert_ne!(wire, plain);
        assert_eq!(from_swizzled(&wire, Magic::default()), plain);
        assert_eq!(*wire.last().unwrap(), 0x60);
    }

    #[test]
    fn default_secondary_lasers_are_off_and_power_constant() {
        let tp = Toolpath {
            blocks: vec![rect_block(50.0)],
        };
        let cmds = build_commands(&tp, &RuidaJobOptions::default()).unwrap();
        let raw_of = |laser: u8, limit: Limit| {
            cmds.iter()
                .find_map(|c| match c {
                    Command::LaserPower {
                        laser: l,
                        limit: li,
                        raw,
                    } if *l == laser && *li == limit => Some(*raw),
                    _ => None,
                })
                .unwrap()
        };
        assert_eq!(raw_of(1, Limit::Min), 8192);
        assert_eq!(raw_of(1, Limit::Max), 8192);
        assert_eq!(raw_of(2, Limit::Max), 0);
        assert_eq!(raw_of(4, Limit::Min), 0);
    }

    #[test]
    fn variable_power_is_rejected_with_clear_error() {
        let mut b = rect_block(50.0);
        b.moves.push(Move::Cut {
            to: Point::new(10.0, 10.0),
            power_pct: 0.0,
        });
        let tp = Toolpath { blocks: vec![b] };
        let e = build_rd(&tp, &RuidaJobOptions::default()).unwrap_err();
        assert!(matches!(e, RdError::VariablePower { block: 0, .. }));
        let text = e.to_string();
        assert!(text.contains("no están soportados") || text.contains("aún no"));
    }

    #[test]
    fn relative_moves_used_when_they_fit_and_never_drift() {
        // Una polilínea con saltos de 3 mm y uno de 20 mm; todo en µm exactos.
        let pts = [
            (10.0, 10.0),
            (13.0, 10.0),
            (13.0, 13.0),
            (33.0, 13.0),
            (33.0123, 13.0456),
            (30.0, 10.0),
        ];
        let moves: Vec<Move> = pts
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| {
                let to = Point::new(x, y);
                if i == 0 {
                    Move::Travel { to }
                } else {
                    Move::Cut {
                        to,
                        power_pct: 20.0,
                    }
                }
            })
            .collect();
        let tp = Toolpath {
            blocks: vec![ToolpathBlock {
                speed_mm_min: 1200.0,
                moves,
                ..Default::default()
            }],
        };
        let cmds = build_commands(&tp, &RuidaJobOptions::default()).unwrap();
        let body: Vec<&Command> = cmds
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    Command::MoveAbs { .. }
                        | Command::CutAbs { .. }
                        | Command::MoveRel { .. }
                        | Command::CutRel { .. }
                )
            })
            .collect();
        assert!(matches!(body[0], Command::MoveAbs { .. }));
        assert!(matches!(body[1], Command::CutRel { dx: 3000, dy: 0 }));
        assert!(matches!(body[2], Command::CutRel { dx: 0, dy: 3000 }));
        // 20 mm no cabe en 14 bits (+-8,191 mm): absoluto.
        assert!(matches!(
            body[3],
            Command::CutAbs {
                x: 33_000,
                y: 13_000
            }
        ));
        assert!(matches!(body[4], Command::CutRel { dx: 12, dy: 46 }));
        // Reconstrucción por suma: llega exactamente al último punto en µm.
        let (mut x, mut y) = (0i64, 0i64);
        for c in &body {
            match c {
                Command::MoveAbs { x: a, y: b } | Command::CutAbs { x: a, y: b } => {
                    (x, y) = (*a, *b)
                }
                Command::MoveRel { dx, dy } | Command::CutRel { dx, dy } => {
                    x += i64::from(*dx);
                    y += i64::from(*dy);
                }
                _ => unreachable!(),
            }
        }
        assert_eq!((x, y), (30_000, 10_000));
        // Sin relativos si se pide.
        let abs_only = RuidaJobOptions {
            use_relative: false,
            ..Default::default()
        };
        let cmds = build_commands(&tp, &abs_only).unwrap();
        assert!(!cmds.iter().any(|c| matches!(c, Command::CutRel { .. })));
    }

    #[test]
    fn axis_shortcuts_are_optional() {
        let tp = Toolpath {
            blocks: vec![ToolpathBlock {
                speed_mm_min: 1200.0,
                moves: vec![
                    Move::Travel {
                        to: Point::new(1.0, 1.0),
                    },
                    Move::Cut {
                        to: Point::new(2.0, 1.0),
                        power_pct: 10.0,
                    },
                    Move::Cut {
                        to: Point::new(2.0, 2.0),
                        power_pct: 10.0,
                    },
                ],
                ..Default::default()
            }],
        };
        let o = RuidaJobOptions {
            use_axis_shortcuts: true,
            ..Default::default()
        };
        let cmds = build_commands(&tp, &o).unwrap();
        assert!(cmds.contains(&Command::CutRelX(1000)));
        assert!(cmds.contains(&Command::CutRelY(1000)));
        let cmds = build_commands(&tp, &RuidaJobOptions::default()).unwrap();
        assert!(cmds.contains(&Command::CutRel { dx: 1000, dy: 0 }));
    }

    #[test]
    fn each_block_is_a_layer_and_empty_blocks_are_skipped() {
        let mut b2 = rect_block(30.0);
        b2.speed_mm_min = 3000.0;
        b2.air_assist = false;
        let empty = ToolpathBlock {
            speed_mm_min: 100.0,
            moves: vec![Move::Travel {
                to: Point::new(1.0, 1.0),
            }],
            ..Default::default()
        };
        let tp = Toolpath {
            blocks: vec![rect_block(50.0), empty, b2],
        };
        let o = RuidaJobOptions {
            layers: vec![
                LayerOptions::default(),
                LayerOptions::default(),
                LayerOptions {
                    color: [255, 0, 0],
                    min_power_pct: None,
                },
            ],
            ..Default::default()
        };
        let cmds = build_commands(&tp, &o).unwrap();
        assert!(cmds.contains(&Command::MaxLayer(1)));
        assert!(cmds.contains(&Command::LayerSelect(0)));
        assert!(cmds.contains(&Command::LayerSelect(1)));
        assert!(cmds.contains(&Command::LayerSpeed {
            layer: 1,
            um_s: 50_000
        }));
        assert!(cmds.contains(&Command::LayerColor {
            layer: 1,
            color: 0xFF
        }));
        let air: Vec<&Command> = cmds
            .iter()
            .filter(|c| matches!(c, Command::AirOn | Command::AirOff))
            .collect();
        assert_eq!(air, [&Command::AirOn, &Command::AirOff]);
    }

    #[test]
    fn flip_and_offset() {
        let tp = Toolpath {
            blocks: vec![ToolpathBlock {
                speed_mm_min: 1200.0,
                moves: vec![Move::Cut {
                    to: Point::new(10.0, 30.0),
                    power_pct: 10.0,
                }],
                ..Default::default()
            }],
        };
        let o = RuidaJobOptions {
            flip_y: true,
            offset_mm: [1.0, 0.5],
            ..Default::default()
        };
        let cmds = build_commands(&tp, &o).unwrap();
        // alto de cama 400: y' = 400 - 30 + 0.5
        assert!(cmds.contains(&Command::CutAbs {
            x: 11_000,
            y: 370_500
        }));
    }

    #[test]
    fn rejects_empty_unsafe_and_z() {
        let o = RuidaJobOptions::default();
        assert_eq!(
            build_rd(&Toolpath::default(), &o).unwrap_err(),
            RdError::EmptyJob
        );
        let mut b = rect_block(120.0);
        assert!(matches!(
            build_rd(
                &Toolpath {
                    blocks: vec![b.clone()]
                },
                &o
            ),
            Err(RdError::Safety(SafetyError::Power { .. }))
        ));
        b = rect_block(10.0);
        b.moves.push(Move::Cut {
            to: Point::new(f64::NAN, 1.0),
            power_pct: 10.0,
        });
        assert!(matches!(
            build_rd(&Toolpath { blocks: vec![b] }, &o),
            Err(RdError::Safety(SafetyError::NonFinite { .. }))
        ));
        let mut b = rect_block(10.0);
        b.z_mm = Some(2.0);
        assert!(matches!(
            build_rd(&Toolpath { blocks: vec![b] }, &o),
            Err(RdError::Safety(SafetyError::ZUnsupported { .. }))
        ));
        // Un offset que saca el trabajo de la cama lo detecta la comprobación del flujo.
        let tp = Toolpath {
            blocks: vec![rect_block(10.0)],
        };
        let off = RuidaJobOptions {
            offset_mm: [400.0, 0.0],
            ..Default::default()
        };
        assert!(matches!(
            build_rd(&tp, &off),
            Err(RdError::Safety(SafetyError::StreamOutOfBed { .. }))
        ));
    }

    #[test]
    fn too_many_layers() {
        let blocks = (0..129).map(|_| rect_block(10.0)).collect();
        assert!(matches!(
            build_rd(&Toolpath { blocks }, &RuidaJobOptions::default()),
            Err(RdError::TooManyLayers { .. })
        ));
    }

    #[test]
    fn min_power_above_max_is_rejected() {
        let o = RuidaJobOptions {
            layers: vec![LayerOptions {
                color: [0; 3],
                min_power_pct: Some(60.0),
            }],
            ..Default::default()
        };
        assert!(matches!(
            build_rd(
                &Toolpath {
                    blocks: vec![rect_block(50.0)]
                },
                &o
            ),
            Err(RdError::MinPowerAboveMax { block: 0 })
        ));
    }

    #[test]
    fn memory_write_footer_only_when_requested() {
        let tp = Toolpath {
            blocks: vec![rect_block(10.0)],
        };
        let on = RuidaJobOptions {
            footer_stats: true,
            ..Default::default()
        };
        let cmds = build_commands(&tp, &on).unwrap();
        assert!(cmds.iter().any(
            |c| matches!(c, Command::MemWrite { addr: 0x320, values } if values == &[327, 327])
        ));
        build_rd(&tp, &on).unwrap();
    }

    #[test]
    fn file_checksum_is_optional_and_excludes_da() {
        let tp = Toolpath {
            blocks: vec![rect_block(10.0)],
        };
        let none = build_commands(&tp, &RuidaJobOptions::default()).unwrap();
        assert!(!none.iter().any(|c| matches!(c, Command::FileChecksum(_))));
        let o = RuidaJobOptions {
            file_checksum: FileChecksum::SumToEndBlock,
            footer_stats: true,
            ..Default::default()
        };
        let cmds = build_commands(&tp, &o).unwrap();
        let i = cmds
            .iter()
            .position(|c| matches!(c, Command::FileChecksum(_)))
            .unwrap();
        assert_eq!(cmds[i - 1], Command::EndBlock);
        let Command::FileChecksum(s) = cmds[i] else {
            unreachable!()
        };
        let expect: u64 = cmd::encode_stream(&cmds[..i])
            .unwrap()
            .iter()
            .map(|&b| u64::from(b))
            .sum();
        assert_eq!(s, expect);
    }

    #[test]
    fn process_start_is_optional() {
        let tp = Toolpath {
            blocks: vec![rect_block(10.0)],
        };
        let o = RuidaJobOptions {
            emit_process_start: false,
            ..Default::default()
        };
        let cmds = build_commands(&tp, &o).unwrap();
        assert!(!cmds.contains(&Command::Process(ProcessOp::Start)));
    }
}
