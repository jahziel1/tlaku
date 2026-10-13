//! De `Toolpath` a archivo `.rd` (flujo de comandos Ruida con cabecera, capas, cuerpo y pie).
//!
//! La estructura sale de los archivos de RDWorks (ejemplo 1 de EduTech, ruida.md sección 5) y se
//! contrasta con lo que escriben MeerK40t y ruida-pa (ambos MIT, verificados por ellos en una
//! RDC6442S; ver ruida.md sección 12): `D8 00` va siempre al principio, el nombre de elemento es
//! `UNNAMED `, las matrices de copias son 1x1 y la máscara de tubos de `CA 03` es 1.
//!
//! Orden del flujo generado:
//! 1. Arranque: `D8 10`, `F0`, `E6 01`, `F1 02 00`, `D8 00`.
//! 2. Cajas del trabajo (`E7 06/03/07/50/51/04/05`).
//! 3. Cabecera de cada capa (`C9 04`, `C6 31..38`, `CA 06`, `CA 41`, `E7 52/53/61/62`).
//! 4. `CA 22`, offsets (siempre 0) y bloque de elemento (`E7 54/55`, `F1`, `F2`).
//! 5. `EA`, `E7 60/13/17/23/24/08`.
//! 6. Cuerpo de cada capa, **en el orden de los bloques**: `CA 01 00`, `CA 02`, `CA 01 30/10`,
//!    aire, velocidad, retardos, potencias, `CA 03`, `CA 10 00` y los movimientos.
//! 7. Pie: `EB`, `E7 00`, suma `E5 05` (opcional), `DA 01 06 20` (opcional) y `D7`.
//!
//! Cada `ToolpathBlock` con al menos un corte es una capa. Los bloques sin cortes se omiten.
//!
//! **Potencia variable** (ráster, relleno con overscan): si los cortes de un bloque no comparten
//! potencia, la capa lleva mín = 0 y máx = la mayor del bloque, y antes de cada corte cuya
//! potencia cambia se emite `C7 P` (potencia inmediata, la que usa ruida-pa para escribir
//! imágenes). Los cortes con potencia 0 (el overscan) se escriben como saltos con el láser
//! apagado ([`ZeroPower::Travel`]), como hacen MeerK40t y ruida-pa. No hay eje Z ni rotativo.

use crate::cmd::{self, Command, Limit, ProcessOp};
use crate::num::{self, NumError};
use crate::safety::{self, SafetyError, SafetyLimits};
use crate::swizzle::Magic;
use std::fmt;
use tlaku_core::Point;
use tlaku_core::toolpath::{Move, Toolpath};

/// Capas posibles (int7).
pub const MAX_LAYERS: usize = 128;

/// Nombre de elemento que escribe RDWorks en todos los archivos vistos (8 caracteres).
pub const ELEMENT_NAME: &str = "UNNAMED ";

/// Empaqueta 8 caracteres ASCII para `F2 02`: cuatro caracteres por cada valor de 35 bits
/// (32 bits útiles, big endian) y cada valor en 5 bytes de 7 bits. `UNNAMED ` da
/// `05 2A 39 1C 41 04 6A 15 08 20`, los bytes de todos los `.rd` de RDWorks vistos.
pub fn encode_element_name(name: &str) -> Option<[u8; 10]> {
    let b = name.as_bytes();
    if b.len() != 8 || !b.is_ascii() {
        return None;
    }
    let mut out = [0u8; 10];
    for half in 0..2 {
        let v = u32::from_be_bytes([
            b[4 * half],
            b[4 * half + 1],
            b[4 * half + 2],
            b[4 * half + 3],
        ]);
        for k in 0..5 {
            out[5 * half + k] = ((v >> (7 * (4 - k))) & 0x7F) as u8;
        }
    }
    Some(out)
}

/// Inversa de [`encode_element_name`]; los bytes no imprimibles salen como `?`.
pub fn decode_element_name(bytes: &[u8; 10]) -> String {
    let mut s = String::with_capacity(8);
    for half in 0..2 {
        let mut v = 0u64;
        for &b in &bytes[5 * half..5 * half + 5] {
            v = (v << 7) | u64::from(b & 0x7F);
        }
        for c in (v as u32).to_be_bytes() {
            s.push(if (0x20..0x7F).contains(&c) {
                c as char
            } else {
                '?'
            });
        }
    }
    s
}

/// Esquina de la cama donde está el home de la máquina. Las coordenadas de un `.rd` con
/// `D8 10` (cero de la máquina) son distancias desde el home hacia dentro de la cama
/// (ruida-pa nombra las cajas como "top right" y MeerK40t necesita "Flip X" en una máquina con
/// el home a la derecha). Tlaku usa origen abajo a la izquierda con Y hacia arriba, así que el
/// perfil debe decir dónde está el home. Sin hardware no hay un valor universal: el defecto es
/// arriba a la izquierda (lo más común) y la prueba 2 de ruida.md 10.6 lo confirma.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HomeCorner {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl HomeCorner {
    fn flips(self) -> (bool, bool) {
        match self {
            HomeCorner::TopLeft => (false, true),
            HomeCorner::TopRight => (true, true),
            HomeCorner::BottomLeft => (false, false),
            HomeCorner::BottomRight => (true, false),
        }
    }

    /// Punto de Tlaku (mm, abajo-izquierda, Y arriba) a coordenadas de la máquina (mm).
    pub fn to_machine(self, p: Point, bed_mm: [f64; 2]) -> (f64, f64) {
        let (fx, fy) = self.flips();
        (
            if fx { bed_mm[0] - p.x } else { p.x },
            if fy { bed_mm[1] - p.y } else { p.y },
        )
    }

    /// Inversa de [`HomeCorner::to_machine`].
    pub fn from_machine(self, x: f64, y: f64, bed_mm: [f64; 2]) -> Point {
        let (fx, fy) = self.flips();
        Point::new(
            if fx { bed_mm[0] - x } else { x },
            if fy { bed_mm[1] - y } else { y },
        )
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "top-left" | "tl" => Some(HomeCorner::TopLeft),
            "top-right" | "tr" => Some(HomeCorner::TopRight),
            "bottom-left" | "bl" => Some(HomeCorner::BottomLeft),
            "bottom-right" | "br" => Some(HomeCorner::BottomRight),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            HomeCorner::TopLeft => "top-left",
            HomeCorner::TopRight => "top-right",
            HomeCorner::BottomLeft => "bottom-left",
            HomeCorner::BottomRight => "bottom-right",
        }
    }
}

/// Qué hacer con un corte a potencia 0 dentro de un bloque de potencia variable (el overscan).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ZeroPower {
    /// Salto con el láser apagado (`88/89`): lo que hacen MeerK40t y ruida-pa.
    #[default]
    Travel,
    /// `C7 0` y corte: el cabezal recorre el tramo a la velocidad de la capa con potencia 0.
    /// Sin verificar en hardware.
    Cut,
}

/// Ajustes de una capa que el `Toolpath` no trae.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LayerOptions {
    /// Color RGB de la capa (en el archivo va como BBGGRR).
    pub color: [u8; 3],
    /// Potencia mínima en %. `None` = igual a la máxima (potencia constante). Con valores
    /// distintos la controladora escala con la velocidad (comportamiento de foros, ruida.md 4.2;
    /// ruida-pa lo modela igual: a poca velocidad sube hacia el máximo). Se ignora en los
    /// bloques de potencia variable.
    pub min_power_pct: Option<f64>,
}

/// Qué suma `E5 05` lleva al final del trabajo.
///
/// MeerK40t (MIT, probado en una RDC6442S) escribe la suma de **todos** los bytes de los
/// comandos del trabajo más `0xD7`; ruida-pa (MIT) la describe igual (excluye lecturas `DA 00`
/// y teclas `A7`, y el propio `E5 05`). Con capturas de LightBurn el resultado difiere de forma
/// constante en 220: falta algún byte por identificar. Los `.rd` de RDWorks no llevan `E5 05`
/// y funcionan en todas las máquinas, así que el defecto es no emitirlo; no se sabe si el
/// firmware rechaza una suma errónea (ruida-pa dice que "puede mostrar un mensaje").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileChecksum {
    /// No emitir `E5 05` (como RDWorks).
    #[default]
    None,
    /// Suma de los bytes de todos los comandos anteriores más `0xD7` (fórmula de MeerK40t).
    SumAll,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuidaJobOptions {
    /// Límites de seguridad; se comprueban antes y después de generar.
    pub limits: SafetyLimits,
    /// Ajustes por bloque del `Toolpath` (mismo índice); los que falten usan los de por defecto.
    pub layers: Vec<LayerOptions>,
    /// Esquina del home de la máquina (define el origen y el signo de X e Y del archivo).
    pub home: HomeCorner,
    /// Desplazamiento en mm que se suma a todas las coordenadas (ya en el sistema de la máquina).
    pub offset_mm: [f64; 2],
    /// Usar movimientos relativos cuando el salto cabe en 14 bits con signo (+-8,191 mm).
    pub use_relative: bool,
    /// Usar `8A/8B/AA/AB` cuando un solo eje cambia. Fuentes menos firmes que `89/A9`: apagado.
    pub use_axis_shortcuts: bool,
    pub file_checksum: FileChecksum,
    /// Emitir `DA 01 06 20 L L` en el pie, con L = longitud de corte en mm (inferencia propia a
    /// partir del ejemplo: 327 mm de perímetro). Es una **escritura de memoria** que MeerK40t
    /// no emite: apagado.
    pub footer_stats: bool,
    /// Potencia de los láseres 2 a 4 en %. 0 por seguridad; el ejemplo usa ~30 %.
    pub secondary_laser_power_pct: f64,
    /// `CA 03 n`: tubos activos al empezar. MeerK40t escribe 1 (solo el láser 1, verificado en
    /// una RDC6442S); RDWorks escribe 0x0D o 0x0F según la configuración de la máquina.
    pub laser_tube_start: u8,
    /// Cortes a potencia 0 de los bloques de potencia variable.
    pub zero_power: ZeroPower,
}

impl Default for RuidaJobOptions {
    fn default() -> Self {
        RuidaJobOptions {
            limits: SafetyLimits::default(),
            layers: Vec::new(),
            home: HomeCorner::default(),
            offset_mm: [0.0, 0.0],
            use_relative: true,
            use_axis_shortcuts: false,
            file_checksum: FileChecksum::None,
            footer_stats: false,
            secondary_laser_power_pct: 0.0,
            laser_tube_start: 1,
            zero_power: ZeroPower::Travel,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RdError {
    Safety(SafetyError),
    Num(NumError),
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
    /// Potencia cruda del corte (solo importa en los bloques de potencia variable).
    power: u16,
}

struct LayerPlan {
    /// Índice de capa en el archivo (0 a 127).
    index: u8,
    speed_um_s: u64,
    power_max: u16,
    power_min: u16,
    /// Los cortes no comparten potencia: se escribe `C7` antes de cada cambio.
    variable: bool,
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
    let (x, y) = opts.home.to_machine(p, opts.limits.bed_mm);
    Ok((
        num::mm_to_um(x + opts.offset_mm[0])?,
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
                Move::Travel { .. } | Move::Dwell { .. } => None,
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
        let variable = lo != hi;
        let lopt = opts.layers.get(bi).cloned().unwrap_or_default();
        let power_min = if variable {
            // La capa deja que `C7` mande: mínimo 0 y máximo el mayor del bloque.
            0
        } else {
            match lopt.min_power_pct {
                Some(m) => {
                    let r = num::power_from_pct(m)?;
                    if r > hi {
                        return Err(RdError::MinPowerAboveMax { block: bi });
                    }
                    r
                }
                None => hi,
            }
        };
        if plans.len() >= MAX_LAYERS {
            return Err(RdError::TooManyLayers {
                layers: plans.len() + 1,
            });
        }
        let mut steps = Vec::new();
        let mut bbox: Option<BBox> = None;
        for m in &block.moves {
            // El RD que generamos no tiene pausa: se omite (la cabeza ya está en ese punto).
            if matches!(m, Move::Dwell { .. }) {
                continue;
            }
            let p = to_um(m.to(), opts)?;
            match &mut bbox {
                Some(b) => b.add(p),
                None => bbox = Some(BBox::of(p)),
            }
            if last == Some(p) {
                continue;
            }
            last = Some(p);
            let (cut, power) = match m {
                Move::Travel { .. } | Move::Dwell { .. } => (false, 0),
                Move::Cut { power_pct, .. } => {
                    let raw = num::power_from_pct(*power_pct)?;
                    let laser = !variable || raw > 0 || opts.zero_power == ZeroPower::Cut;
                    (laser, raw)
                }
            };
            steps.push(Step {
                cut,
                x: p.0,
                y: p.1,
                power,
            });
        }
        plans.push(LayerPlan {
            index: plans.len() as u8,
            speed_um_s: num::speed_from_mm_min(block.speed_mm_min)?,
            power_max: hi,
            power_min,
            variable,
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

/// Matriz de una sola copia (1 columna x 1 fila). RDWorks escribe el tamaño del dibujo como paso
/// (con signos que cambian de un archivo a otro), pero con 1x1 el paso no se usa; MeerK40t
/// escribe pasos arbitrarios y funciona. Aquí 0.
fn single_copy(op: u8, sub: u8) -> Command {
    Command::Copies {
        op,
        sub,
        cols: 1,
        rows: 1,
        step_x: 0,
        step_y: 0,
    }
}

/// Instrucciones de movimiento de una capa: relativas si el salto cabe, absolutas si no. La
/// primera de cada capa es siempre absoluta y la posición se lleva en enteros (sin deriva).
/// En las capas de potencia variable, `C7 P` precede a cada corte cuya potencia cambia.
fn emit_moves(plan: &LayerPlan, opts: &RuidaJobOptions, out: &mut Vec<Command>) {
    let mut pos: Option<(i64, i64)> = None;
    let mut imm: Option<u16> = None;
    for s in &plan.steps {
        if plan.variable && s.cut && imm != Some(s.power) {
            out.push(Command::ImmPower(s.power));
            imm = Some(s.power);
        }
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
    if opts.laser_tube_start > 0x7F {
        return Err(RdError::BadOption("laser_tube_start debe caber en 7 bits"));
    }
    let name = encode_element_name(ELEMENT_NAME).expect("nombre válido");
    let plans = plan_layers(tp, opts)?;
    let mut job_box = plans[0].bbox;
    for p in &plans[1..] {
        job_box.union(&p.bbox);
    }
    let mut c: Vec<Command> = Vec::new();

    // 1. Arranque. `D8 00` va siempre: está en todos los archivos de RDWorks, en la captura
    // de Nova 35 y en lo que envían MeerK40t y ruida-pa (verificado en RDC6442S).
    c.push(Command::Process(ProcessOp::RefPoint));
    c.push(Command::SetReference);
    c.push(Command::Absolute);
    c.push(Command::Small {
        op: 0xF1,
        sub: 0x02,
        n: 0,
    });
    c.push(Command::Process(ProcessOp::Start));

    // 2. Cajas del trabajo.
    c.push(point2(0xE7, 0x06, (0, 0)));
    c.push(point2(0xE7, 0x03, job_box.min));
    c.push(point2(0xE7, 0x07, job_box.max));
    c.push(point2(0xE7, 0x50, job_box.min));
    c.push(point2(0xE7, 0x51, job_box.max));
    c.push(single_copy(0xE7, 0x04));
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

    // 4. Máximo de capa, offsets (todos a 0) y bloque de elemento.
    c.push(Command::MaxLayer(plans.len() as u8 - 1));
    for sub in [0x54, 0x55] {
        for n in 0..2u8 {
            c.push(Command::PartAbs { sub, n, v: 0 });
        }
    }
    c.push(point2(0xF1, 0x03, (0, 0)));
    for (op, sub) in [(0xF1, 0x00), (0xF1, 0x01), (0xF2, 0x00), (0xF2, 0x01)] {
        c.push(Command::Small { op, sub, n: 0 });
    }
    c.push(Command::ElementName(name));
    c.push(point2(0xF2, 0x03, job_box.min));
    c.push(point2(0xF2, 0x04, job_box.max));
    c.push(point2(0xF2, 0x06, (0, 0)));
    c.push(Command::Small {
        op: 0xF2,
        sub: 0x07,
        n: 0,
    });
    c.push(single_copy(0xF2, 0x05));

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
    c.push(single_copy(0xE7, 0x08));

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
        // Retardos de encendido y apagado del láser (`C6 12/13`), a 0: los que escriben
        // MeerK40t y los archivos de RDWorks. El ejemplo de EduTech usa `C6 15/16`.
        c.push(Command::LaserTime {
            sub: 0x12,
            micros: 0,
        });
        c.push(Command::LaserTime {
            sub: 0x13,
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
            n: opts.laser_tube_start,
        });
        c.push(Command::Small {
            op: 0xCA,
            sub: 0x10,
            n: 0x00,
        });
        emit_moves(p, opts, &mut c);
    }

    // 7. Pie.
    c.push(Command::Marker(0xEB));
    c.push(Command::EndBlock);
    if opts.footer_stats {
        let len = cut_length_mm(&plans);
        c.push(Command::MemWrite {
            addr: 0x0320,
            values: vec![len, len],
        });
    }
    if opts.file_checksum == FileChecksum::SumAll {
        let sum = sum_bytes(&c)? + 0xD7;
        c.push(Command::FileChecksum(sum));
    }
    c.push(Command::EndOfFile);
    Ok(c)
}

/// Suma de los bytes de los comandos (sin lecturas `DA 00`, que no entran en la suma).
fn sum_bytes(cmds: &[Command]) -> Result<u64, NumError> {
    let mut sum = 0u64;
    for c in cmds {
        if matches!(c, Command::MemRead { .. }) {
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

    /// Opciones sin cambio de ejes (home abajo a la izquierda): coordenadas = las del `Toolpath`.
    fn dflt() -> RuidaJobOptions {
        RuidaJobOptions {
            home: HomeCorner::BottomLeft,
            ..Default::default()
        }
    }

    fn pct(raw: u16) -> f64 {
        f64::from(raw) * 100.0 / 16383.0
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
            footer_stats: true,
            ..dflt()
        }
    }

    /// El ejemplo 1 con las diferencias **intencionadas** de Tlaku aplicadas, para poder
    /// compararlo con lo que genera (ver ruida.md sección 12): offsets de pluma y de pantalla
    /// a 0, retardos `C6 12/13` en vez de `C6 15/16`, `CA 03 01` en vez de `0D` y matrices 1x1
    /// con pasos 0.
    fn example_with_tlaku_choices() -> Vec<Command> {
        cmd::decode_stream(&hex(EXAMPLE_1))
            .unwrap()
            .into_iter()
            .map(|c| match c {
                Command::PartAbs { sub, n, .. } => Command::PartAbs { sub, n, v: 0 },
                Command::Point2 {
                    op: 0xF1,
                    sub: 0x03,
                    ..
                } => Command::Point2 {
                    op: 0xF1,
                    sub: 0x03,
                    x: 0,
                    y: 0,
                },
                Command::LaserTime { sub, micros } => Command::LaserTime {
                    sub: sub - 3,
                    micros,
                },
                Command::Small {
                    op: 0xCA,
                    sub: 0x03,
                    ..
                } => Command::Small {
                    op: 0xCA,
                    sub: 0x03,
                    n: 1,
                },
                Command::Copies {
                    op,
                    sub,
                    cols,
                    rows,
                    ..
                } if sub != 0x04 => Command::Copies {
                    op,
                    sub,
                    cols,
                    rows,
                    step_x: 0,
                    step_y: 0,
                },
                other => other,
            })
            .collect()
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
        let expected = example_with_tlaku_choices();
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
        let ex = example_with_tlaku_choices();
        let tp = Toolpath {
            blocks: vec![rect_block(pct(3112))],
        };
        let mut cmds = build_commands(&tp, &example_options()).unwrap();
        for (a, b) in cmds.iter_mut().zip(&ex) {
            if matches!(a, Command::Point2 { .. } | Command::PartPoint { .. }) {
                *a = b.clone();
            }
        }
        assert_eq!(
            cmd::encode_stream(&cmds).unwrap(),
            cmd::encode_stream(&ex).unwrap()
        );
    }

    #[test]
    fn rd_is_valid_stream_and_roundtrips_through_swizzle() {
        let tp = Toolpath {
            blocks: vec![rect_block(40.0)],
        };
        let plain = build_rd(&tp, &dflt()).unwrap();
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
        let cmds = build_commands(&tp, &dflt()).unwrap();
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
        assert_eq!(raw_of(1, Limit::Min), 8191);
        assert_eq!(raw_of(1, Limit::Max), 8191);
        assert_eq!(raw_of(2, Limit::Max), 0);
        assert_eq!(raw_of(4, Limit::Min), 0);
    }

    /// Dos filas de un ráster: overscan a potencia 0, tramos de potencia distinta y overscan.
    fn raster_block() -> ToolpathBlock {
        let cut = |x: f64, y: f64, p: f64| Move::Cut {
            to: Point::new(x, y),
            power_pct: p,
        };
        let travel = |x: f64, y: f64| Move::Travel {
            to: Point::new(x, y),
        };
        ToolpathBlock {
            label: "ráster".into(),
            speed_mm_min: 6000.0,
            moves: vec![
                travel(8.0, 10.0),
                cut(10.0, 10.0, 0.0),
                cut(11.0, 10.0, 40.0),
                cut(12.0, 10.0, 80.0),
                cut(14.0, 10.0, 0.0),
                travel(14.0, 10.1),
                cut(12.0, 10.1, 0.0),
                cut(11.0, 10.1, 80.0),
                cut(10.0, 10.1, 40.0),
                cut(8.0, 10.1, 0.0),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn variable_power_uses_imd_power_and_zero_power_travels() {
        let tp = Toolpath {
            blocks: vec![raster_block()],
        };
        let cmds = build_commands(&tp, &dflt()).unwrap();
        // Capa con mín 0 y máx = la mayor potencia (80 %).
        let hi = num::power_from_pct(80.0).unwrap();
        let lp = |limit: Limit| {
            cmds.iter()
                .find_map(|c| match c {
                    Command::LayerPower {
                        laser: 1,
                        limit: l,
                        raw,
                        ..
                    } if *l == limit => Some(*raw),
                    _ => None,
                })
                .unwrap()
        };
        assert_eq!((lp(Limit::Min), lp(Limit::Max)), (0, hi));
        // C7 solo cuando cambia y nunca para potencia 0 (los tramos de overscan son saltos).
        let imm: Vec<u16> = cmds
            .iter()
            .filter_map(|c| match c {
                Command::ImmPower(r) => Some(*r),
                _ => None,
            })
            .collect();
        let (p40, p80) = (
            num::power_from_pct(40.0).unwrap(),
            num::power_from_pct(80.0).unwrap(),
        );
        assert_eq!(imm, [p40, p80, p40]);
        // Cada corte va precedido por el C7 vigente: se reconstruye la potencia de cada tramo.
        let (mut cur, mut got) = (None, Vec::new());
        for c in &cmds {
            match c {
                Command::ImmPower(r) => cur = Some(*r),
                Command::CutRel { .. } | Command::CutAbs { .. } => got.push(cur.unwrap()),
                _ => {}
            }
        }
        assert_eq!(got, [p40, p80, p80, p40]);
        // Los tramos a potencia 0 salen como saltos.
        let travels = cmds
            .iter()
            .filter(|c| matches!(c, Command::MoveAbs { .. } | Command::MoveRel { .. }))
            .count();
        assert_eq!(travels, 6);
        // Con `ZeroPower::Cut` el tramo se corta con C7 0.
        let o = RuidaJobOptions {
            zero_power: ZeroPower::Cut,
            ..dflt()
        };
        let cmds = build_commands(&tp, &o).unwrap();
        assert!(cmds.contains(&Command::ImmPower(0)));
        // Solo quedan los dos saltos de verdad (el primero y el del cambio de fila).
        let moves = cmds
            .iter()
            .filter(|c| matches!(c, Command::MoveAbs { .. } | Command::MoveRel { .. }))
            .count();
        assert_eq!(moves, 2);
        build_rd(&tp, &o).unwrap();
        build_rd(&tp, &dflt()).unwrap();
    }

    #[test]
    fn constant_power_blocks_do_not_use_imd_power() {
        let tp = Toolpath {
            blocks: vec![rect_block(50.0)],
        };
        let cmds = build_commands(&tp, &dflt()).unwrap();
        assert!(!cmds.iter().any(|c| matches!(c, Command::ImmPower(_))));
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
        let cmds = build_commands(&tp, &dflt()).unwrap();
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
            ..dflt()
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
            ..dflt()
        };
        let cmds = build_commands(&tp, &o).unwrap();
        assert!(cmds.contains(&Command::CutRelX(1000)));
        assert!(cmds.contains(&Command::CutRelY(1000)));
        let cmds = build_commands(&tp, &dflt()).unwrap();
        assert!(cmds.contains(&Command::CutRel { dx: 1000, dy: 0 }));
    }

    #[test]
    fn dwells_are_skipped_because_rd_has_no_pause() {
        let line = |dwells: bool| {
            let mut moves = vec![Move::Travel {
                to: Point::new(1.0, 1.0),
            }];
            if dwells {
                moves.push(Move::Dwell {
                    to: Point::new(1.0, 1.0),
                    ms: 50.0,
                    power_pct: 80.0,
                });
            }
            moves.push(Move::Cut {
                to: Point::new(2.0, 1.0),
                power_pct: 10.0,
            });
            if dwells {
                moves.push(Move::Dwell {
                    to: Point::new(2.0, 1.0),
                    ms: 50.0,
                    power_pct: 10.0,
                });
            }
            Toolpath {
                blocks: vec![ToolpathBlock {
                    speed_mm_min: 1200.0,
                    moves,
                    ..Default::default()
                }],
            }
        };
        assert_eq!(
            build_commands(&line(true), &dflt()).unwrap(),
            build_commands(&line(false), &dflt()).unwrap()
        );
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
    fn home_corner_maps_the_axes_and_offset() {
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
        // Cama por defecto 600 x 400.
        for (home, x, y) in [
            (HomeCorner::TopLeft, 11_000, 370_500),
            (HomeCorner::TopRight, 591_000, 370_500),
            (HomeCorner::BottomLeft, 11_000, 30_500),
            (HomeCorner::BottomRight, 591_000, 30_500),
        ] {
            let o = RuidaJobOptions {
                home,
                offset_mm: [1.0, 0.5],
                ..Default::default()
            };
            let cmds = build_commands(&tp, &o).unwrap();
            // x' = 10 (o 600 - 10) + 1; y' = 400 - 30 (o 30) + 0.5
            let want = if matches!(home, HomeCorner::TopRight | HomeCorner::BottomRight) {
                x
            } else {
                11_000
            };
            assert!(cmds.contains(&Command::CutAbs { x: want, y }), "{home:?}");
        }
    }

    #[test]
    fn home_corner_roundtrip_and_parse() {
        let bed = [600.0, 400.0];
        for home in [
            HomeCorner::TopLeft,
            HomeCorner::TopRight,
            HomeCorner::BottomLeft,
            HomeCorner::BottomRight,
        ] {
            let p = Point::new(123.4, 56.7);
            let (x, y) = home.to_machine(p, bed);
            let back = home.from_machine(x, y, bed);
            assert!((back.x - p.x).abs() < 1e-9 && (back.y - p.y).abs() < 1e-9);
            assert_eq!(HomeCorner::parse(home.name()), Some(home));
        }
        assert_eq!(HomeCorner::parse("TOP_LEFT"), Some(HomeCorner::TopLeft));
        assert_eq!(HomeCorner::parse("centro"), None);
    }

    #[test]
    fn element_name_matches_every_rdworks_file() {
        // Los bytes de f202... del ejemplo 1 de EduTech y de los demás archivos de RDWorks.
        let bytes = [0x05, 0x2A, 0x39, 0x1C, 0x41, 0x04, 0x6A, 0x15, 0x08, 0x20];
        assert_eq!(encode_element_name(ELEMENT_NAME), Some(bytes));
        assert_eq!(decode_element_name(&bytes), "UNNAMED ");
        assert_eq!(encode_element_name("corto"), None);
        assert_eq!(encode_element_name("ñññññññ1"), None);
    }

    #[test]
    fn rejects_empty_unsafe_and_z() {
        let o = dflt();
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
            build_rd(&Toolpath { blocks }, &dflt()),
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
    fn file_checksum_is_optional() {
        let tp = Toolpath {
            blocks: vec![rect_block(10.0)],
        };
        let none = build_commands(&tp, &dflt()).unwrap();
        assert!(!none.iter().any(|c| matches!(c, Command::FileChecksum(_))));
        let o = RuidaJobOptions {
            file_checksum: FileChecksum::SumAll,
            footer_stats: true,
            ..Default::default()
        };
        let cmds = build_commands(&tp, &o).unwrap();
        let i = cmds
            .iter()
            .position(|c| matches!(c, Command::FileChecksum(_)))
            .unwrap();
        // Va justo antes de D7 y después del pie.
        assert!(matches!(cmds[i - 1], Command::MemWrite { .. }));
        assert_eq!(cmds[i + 1], Command::EndOfFile);
        let Command::FileChecksum(s) = cmds[i] else {
            unreachable!()
        };
        // Fórmula de MeerK40t: suma de todos los bytes anteriores más 0xD7 del fin de archivo.
        let expect: u64 = cmd::encode_stream(&cmds[..i])
            .unwrap()
            .iter()
            .map(|&b| u64::from(b))
            .sum::<u64>()
            + 0xD7;
        assert_eq!(s, expect);
    }

    #[test]
    fn process_start_is_always_written_after_the_reference_setup() {
        let tp = Toolpath {
            blocks: vec![rect_block(10.0)],
        };
        let cmds = build_commands(&tp, &dflt()).unwrap();
        // D8 10, F0, E6 01, F1 02 00 y D8 00 (captura de Nova 35, RDWorks, MeerK40t).
        assert_eq!(cmds[0], Command::Process(ProcessOp::RefPoint));
        assert_eq!(cmds[4], Command::Process(ProcessOp::Start));
        assert_eq!(
            cmds.iter()
                .filter(|c| **c == Command::Process(ProcessOp::Start))
                .count(),
            1
        );
    }

    #[test]
    fn laser_tube_mask_and_delays() {
        let tp = Toolpath {
            blocks: vec![rect_block(10.0)],
        };
        let cmds = build_commands(&tp, &dflt()).unwrap();
        assert!(cmds.contains(&Command::Small {
            op: 0xCA,
            sub: 3,
            n: 1
        }));
        assert!(cmds.contains(&Command::LaserTime {
            sub: 0x12,
            micros: 0
        }));
        assert!(cmds.contains(&Command::LaserTime {
            sub: 0x13,
            micros: 0
        }));
        let o = RuidaJobOptions {
            laser_tube_start: 200,
            ..Default::default()
        };
        assert!(matches!(
            build_commands(&tp, &o),
            Err(RdError::BadOption(_))
        ));
    }
}
