//! De `Toolpath` a LHYMICRO-GL.
//!
//! El codificador sigue el esquema de `lihuiyu/driver.py` de MeerK40t (MIT; ver THIRD_PARTY.md),
//! reescrito: un modo normal para los saltos y un modo compacto para cortar.
//!
//! ```text
//! salto (láser apagado)   I B<dist> R<dist> S1P \n
//! entrar a cortar         I CV<velocidad> N <direcciones> S1E
//! mover (en compacto)     [B|T|R|L|M] [D|U] <dist>      (el láser va con `D`/`U`)
//! salir de cortar         [U] F N S E - \n              (el `-` espera a que la placa termine)
//! ```
//!
//! # Lo que la placa no hace
//!
//! - **Solo se mueve en ocho direcciones** (recto o diagonal exacta). Un segmento cualquiera se
//!   descompone con Bresenham en tramos rectos y diagonales. Las posiciones se redondean al mil
//!   (0,0254 mm) en absoluto, así que el error no se acumula.
//! - **No controla la potencia** (las M2 de serie): una pasada con potencia mayor que 0 enciende
//!   el láser y con 0 lo apaga. La potencia de la capa se ignora con un [`Warning`]; se ajusta en
//!   el panel o con el potenciómetro de la máquina. Las imágenes con niveles de gris se
//!   binarizan (cualquier potencia mayor que 0 quema): conviene un tramado.
//! - **Sin pulsos en el sitio.** `Move::Dwell` (modo punto, corte pasante, pausas) necesitaría
//!   que el anfitrión temporice el láser encendido, y si falla la conexión el láser quedaría
//!   encendido. Se omite con aviso.
//! - **Trama.** Las filas de un grabado se codifican como movimientos vectoriales con el láser
//!   encendido y apagado. El modo `G` de la placa (que invierte el sentido y avanza solo) no se
//!   usa: no se puede probar sin máquina y el ahorro es de bytes, no de calidad.
//! - **Eje Z, asistencia de aire**: ignorados con aviso.

use crate::packet::{Frame, FrameError, frame_stream};
use crate::speed::{self, Board, MM_PER_MIL, SpeedError, SpeedParams};
use std::fmt;
use tlaku_core::Point;
use tlaku_core::toolpath::{Move, Toolpath};

/// Esquina de la cama donde está el home (los finales de carrera) en el sistema de Tlaku
/// (origen abajo a la izquierda, Y hacia arriba). El K40 de serie lo tiene arriba a la izquierda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HomeCorner {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl HomeCorner {
    pub fn parse(s: &str) -> Option<HomeCorner> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "top-left" => HomeCorner::TopLeft,
            "top-right" => HomeCorner::TopRight,
            "bottom-left" => HomeCorner::BottomLeft,
            "bottom-right" => HomeCorner::BottomRight,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            HomeCorner::TopLeft => "top-left",
            HomeCorner::TopRight => "top-right",
            HomeCorner::BottomLeft => "bottom-left",
            HomeCorner::BottomRight => "bottom-right",
        }
    }

    fn right(self) -> bool {
        matches!(self, HomeCorner::TopRight | HomeCorner::BottomRight)
    }

    fn top(self) -> bool {
        matches!(self, HomeCorner::TopLeft | HomeCorner::TopRight)
    }

    /// Punto de Tlaku (mm) a distancia desde el home en cada eje de la placa (mm). Los ejes de
    /// la placa crecen alejándose del home.
    pub fn to_machine_mm(self, p: Point, bed_mm: [f64; 2]) -> (f64, f64) {
        let x = if self.right() { bed_mm[0] - p.x } else { p.x };
        let y = if self.top() { bed_mm[1] - p.y } else { p.y };
        (x, y)
    }

    /// Inverso de [`HomeCorner::to_machine_mm`].
    pub fn from_machine_mm(self, m: (f64, f64), bed_mm: [f64; 2]) -> Point {
        let x = if self.right() { bed_mm[0] - m.0 } else { m.0 };
        let y = if self.top() { bed_mm[1] - m.1 } else { m.1 };
        Point::new(x, y)
    }

    /// Desplazamiento de Tlaku (mm) a desplazamiento de la placa (mm).
    pub fn delta_to_machine(self, dx: f64, dy: f64) -> (f64, f64) {
        (
            if self.right() { -dx } else { dx },
            if self.top() { -dy } else { dy },
        )
    }
}

/// Mils a milímetros.
pub fn mil_to_mm(mil: i64) -> f64 {
    mil as f64 * MM_PER_MIL
}

/// Milímetros al mil más cercano.
pub fn mm_to_mil(mm: f64) -> i64 {
    (mm / MM_PER_MIL).round() as i64
}

#[derive(Debug, Clone)]
pub struct EncodeConfig {
    pub board: Board,
    /// Ancho y alto de la cama (mm).
    pub bed_mm: [f64; 2],
    pub home: HomeCorner,
    /// Dónde está el cabezal al empezar (mils, ejes de la placa). `(0, 0)` justo después del home.
    pub start_mil: (i64, i64),
    /// Dejar el riel bloqueado tras cada movimiento (lo normal). Con `false` se añade `IS2P`.
    pub autolock: bool,
    /// Velocidad máxima de corte (mm/s). Lo que pase de aquí es un error, no se recorta.
    pub max_speed_mm_s: f64,
    /// Un viaje más corto que esto (mils) se hace dentro del programa con el láser apagado; uno
    /// más largo sale del modo compacto y usa un salto rápido. 256 mil son 6,5 mm.
    pub min_jog_mil: i64,
    /// Forzar el modo sin rampa de aceleración (sufijo `C`). Por defecto se usa el engranaje que
    /// escoge el software chino para cada velocidad y `C` solo por debajo de 7 mm/s.
    pub constant_speed: bool,
    /// Usar la pendiente medida por MeerK40t en vez de la del software chino.
    pub fix_speeds: bool,
    pub d_ratio: Option<f64>,
}

impl Default for EncodeConfig {
    fn default() -> Self {
        EncodeConfig {
            board: Board::M2,
            // Cama habitual del K40 (área útil ~ 300 x 200).
            bed_mm: [300.0, 200.0],
            home: HomeCorner::TopLeft,
            start_mil: (0, 0),
            autolock: true,
            max_speed_mm_s: 140.0,
            min_jog_mil: 256,
            constant_speed: false,
            fix_speeds: false,
            d_ratio: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Warning {
    /// La capa pedía potencia `pct` y la placa no la controla.
    PowerIgnored {
        block: usize,
        pct: f64,
    },
    /// El bloque tiene niveles de potencia intermedios: se graba en blanco y negro.
    GrayThresholded {
        block: usize,
    },
    AirAssistIgnored {
        block: usize,
    },
    ZIgnored {
        block: usize,
    },
    /// Pausa o pulso en el sitio que no se puede hacer con seguridad.
    DwellSkipped {
        block: usize,
        ms: f64,
        fires: bool,
    },
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Warning::PowerIgnored { block, pct } => write!(
                f,
                "bloque {block}: la potencia de {pct:.0} % se ignora; la M2 Nano no la controla \
                 por software (se ajusta en el panel o el potenciómetro de la máquina)"
            ),
            Warning::GrayThresholded { block } => write!(
                f,
                "bloque {block}: niveles de gris binarizados (cualquier potencia mayor que 0 \
                 quema); usa un tramado para simular grises"
            ),
            Warning::AirAssistIgnored { block } => write!(
                f,
                "bloque {block}: la asistencia de aire no se controla desde la M2 Nano"
            ),
            Warning::ZIgnored { block } => {
                write!(
                    f,
                    "bloque {block}: el eje Z no existe en la M2 Nano y se ignora"
                )
            }
            Warning::DwellSkipped { block, ms, fires } => write!(
                f,
                "bloque {block}: {} de {ms} ms omitida (la placa no hace pulsos en el sitio)",
                if *fires { "pulso" } else { "pausa" }
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EncodeError {
    BadConfig(&'static str),
    NonFinite {
        block: usize,
        what: &'static str,
    },
    OutOfBed {
        block: usize,
        index: usize,
        x_mm: f64,
        y_mm: f64,
    },
    Speed {
        block: usize,
        mm_s: f64,
        reason: String,
    },
    Frame(FrameError),
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EncodeError::BadConfig(w) => write!(f, "configuración inválida: {w}"),
            EncodeError::NonFinite { block, what } => {
                write!(f, "bloque {block}: {what} no es un número finito")
            }
            EncodeError::OutOfBed {
                block,
                index,
                x_mm,
                y_mm,
            } => write!(
                f,
                "bloque {block}, movimiento {index}: ({x_mm:.3}, {y_mm:.3}) mm queda fuera de la cama"
            ),
            EncodeError::Speed {
                block,
                mm_s,
                reason,
            } => write!(f, "bloque {block}: velocidad {mm_s:.2} mm/s: {reason}"),
            EncodeError::Frame(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for EncodeError {}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProgramStats {
    /// Longitud con el láser encendido (mm).
    pub burn_mm: f64,
    /// Longitud recorrida con el láser apagado dentro del programa (mm).
    pub travel_mm: f64,
    /// Longitud de los saltos rápidos (mm).
    pub rapid_mm: f64,
    /// Número de saltos rápidos.
    pub rapids: usize,
    /// Veces que se entra al modo compacto.
    pub programs: usize,
    /// Estimación del tiempo del trabajo (s), por exceso: salto rápido a 40 mm/s.
    pub est_seconds: f64,
}

/// Velocidad con la que se estima el tiempo de un salto rápido (mm/s). Es un valor prudente
/// (más lento que el real), solo para fijar tiempos de espera.
pub const RAPID_ESTIMATE_MM_S: f64 = 40.0;

/// Trabajo codificado.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    /// Flujo de comandos (el contenido de un `.egv`, sin cabecera).
    pub stream: Vec<u8>,
    pub start_mil: (i64, i64),
    pub end_mil: (i64, i64),
    pub warnings: Vec<Warning>,
    pub stats: ProgramStats,
    pub board: Board,
    pub bed_mm: [f64; 2],
    pub home: HomeCorner,
}

impl Program {
    pub fn frames(&self) -> Result<Vec<Frame>, FrameError> {
        frame_stream(&self.stream)
    }

    pub fn end_mm(&self) -> Point {
        let m = (mil_to_mm(self.end_mil.0), mil_to_mm(self.end_mil.1));
        self.home.from_machine_mm(m, self.bed_mm)
    }

    /// Contenido de un archivo `.egv`: cabecera de texto y el flujo.
    pub fn to_egv_file(&self, creator: &str) -> Vec<u8> {
        let mut out = format!(
            "Document type : LHYMICRO-GL file\nFile version: 1.0.01\nCopyright: Unknown\n\
             Creator-Software: {creator}\n\n%0%0%0%0%\n"
        )
        .into_bytes();
        out.extend_from_slice(&self.stream);
        out
    }
}

/// Distancia de la placa en el alfabeto del lenguaje: `a`-`y` son 1-25, `|a`-`|z` son 26-51,
/// `z` vale 255 y de 52 a 254 se escriben tres dígitos.
pub fn distance(v: u64) -> String {
    let mut s = String::new();
    let mut v = v;
    if v >= 255 {
        s.push_str(&"z".repeat((v / 255) as usize));
        v %= 255;
    }
    match v {
        0 => {}
        1..=25 => s.push((b'a' + (v as u8) - 1) as char),
        26..=51 => {
            s.push('|');
            s.push((b'a' + (v as u8) - 26) as char);
        }
        _ => s.push_str(&format!("{v:03}")),
    }
    s
}

/// Salto rápido con el láser apagado (modo normal): `I`, el eje X, el eje Y y `S1P`; con
/// `autolock` falso se añade `IS2P` (desbloquear el riel). Las direcciones son las de la placa:
/// `B` derecha y `R` abajo, alejándose del home.
pub fn rapid_text(dx: i64, dy: i64, autolock: bool) -> String {
    let mut s = String::from("I");
    if dx != 0 {
        s.push(if dx > 0 { 'B' } else { 'T' });
        s.push_str(&distance(dx.unsigned_abs()));
    }
    if dy != 0 {
        s.push(if dy > 0 { 'R' } else { 'L' });
        s.push_str(&distance(dy.unsigned_abs()));
    }
    s.push_str("S1P\n");
    if !autolock {
        s.push_str("IS2P\n");
    }
    s
}

/// Pasos unitarios de Bresenham (algoritmo de Zingl) agrupados en tramos rectos o diagonales de
/// igual dirección: `(dx, dy)` de cada tramo.
pub fn line_runs(dx: i64, dy: i64) -> Vec<(i64, i64)> {
    let (sx, sy) = (dx.signum(), dy.signum());
    let (adx, ady) = (dx.abs(), dy.abs());
    let mut err = adx - ady;
    let (mut x, mut y) = (0i64, 0i64);
    let mut runs: Vec<(i64, i64)> = Vec::new();
    let mut last: Option<(i64, i64)> = None;
    while (x, y) != (dx, dy) {
        let e2 = 2 * err;
        let mut step = (0, 0);
        if e2 >= -ady {
            err -= ady;
            x += sx;
            step.0 = sx;
        }
        if e2 <= adx {
            err += adx;
            y += sy;
            step.1 = sy;
        }
        match (&mut last, runs.last_mut()) {
            (Some(l), Some(run)) if *l == step => {
                run.0 += step.0;
                run.1 += step.1;
            }
            _ => {
                runs.push(step);
                last = Some(step);
            }
        }
    }
    runs
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Modo normal: saltos con `S1P`.
    Rapid,
    /// Modo compacto: movimientos con la velocidad del programa.
    Program,
}

struct Encoder<'a> {
    cfg: &'a EncodeConfig,
    out: Vec<u8>,
    pos: (i64, i64),
    mode: Mode,
    /// Destino de un viaje diferido: se ejecuta cuando haga falta (al cortar o al terminar).
    pending: Option<(i64, i64)>,
    code: Option<String>,
    leftward: bool,
    topward: bool,
    x_engaged: bool,
    y_engaged: bool,
    laser: bool,
    stats: ProgramStats,
}

impl<'a> Encoder<'a> {
    fn new(cfg: &'a EncodeConfig) -> Self {
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

    fn flush_pending(&mut self) {
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
    fn exit_program(&mut self) {
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

    fn cut(&mut self, to: (i64, i64), on: bool, code: &str) {
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

    fn travel(&mut self, to: (i64, i64)) {
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

fn validate_config(cfg: &EncodeConfig) -> Result<(), EncodeError> {
    if !(cfg.bed_mm[0].is_finite() && cfg.bed_mm[1].is_finite())
        || cfg.bed_mm[0] <= 0.0
        || cfg.bed_mm[1] <= 0.0
    {
        return Err(EncodeError::BadConfig("la cama debe medir más de 0 mm"));
    }
    if !(cfg.max_speed_mm_s.is_finite() && cfg.max_speed_mm_s > 0.0) {
        return Err(EncodeError::BadConfig(
            "la velocidad máxima debe ser mayor que 0",
        ));
    }
    if cfg.min_jog_mil < 0 {
        return Err(EncodeError::BadConfig("min_jog_mil no puede ser negativo"));
    }
    Ok(())
}

/// Margen (mm) para no rechazar un diseño que toca el borde por el redondeo al mil.
const BED_EPS_MM: f64 = MM_PER_MIL;

/// Codifica un `Toolpath` (mm, origen abajo a la izquierda, Y arriba) en LHYMICRO-GL.
pub fn encode(toolpath: &Toolpath, cfg: &EncodeConfig) -> Result<Program, EncodeError> {
    validate_config(cfg)?;
    let bed = cfg.bed_mm;
    let to_mil = |p: Point| -> (i64, i64) {
        let m = cfg.home.to_machine_mm(p, bed);
        (mm_to_mil(m.0), mm_to_mil(m.1))
    };

    // 1) Validación de todo el trabajo antes de producir un solo byte.
    let mut codes: Vec<String> = Vec::with_capacity(toolpath.blocks.len());
    let mut warnings: Vec<Warning> = Vec::new();
    for (bi, block) in toolpath.blocks.iter().enumerate() {
        if !(block.speed_mm_min.is_finite() && block.speed_mm_min > 0.0) {
            return Err(EncodeError::NonFinite {
                block: bi,
                what: "la velocidad",
            });
        }
        let mm_s = block.speed_mm_min / 60.0;
        if mm_s > cfg.max_speed_mm_s {
            return Err(EncodeError::Speed {
                block: bi,
                mm_s,
                reason: format!("supera el máximo de {} mm/s", cfg.max_speed_mm_s),
            });
        }
        let mut params = SpeedParams::new(cfg.board, mm_s);
        params.fix_speeds = cfg.fix_speeds;
        params.d_ratio = cfg.d_ratio;
        if cfg.constant_speed {
            params.suffix_c = Some(true);
        }
        let code = speed::speed_code(&params).map_err(|e: SpeedError| EncodeError::Speed {
            block: bi,
            mm_s,
            reason: e.to_string(),
        })?;
        codes.push(code);

        let mut max_power = 0.0f64;
        let mut levels: Vec<u32> = Vec::new();
        for (mi, mv) in block.moves.iter().enumerate() {
            let to = mv.to();
            if !(to.x.is_finite() && to.y.is_finite()) {
                return Err(EncodeError::NonFinite {
                    block: bi,
                    what: "una coordenada",
                });
            }
            let m = cfg.home.to_machine_mm(to, bed);
            if m.0 < -BED_EPS_MM
                || m.1 < -BED_EPS_MM
                || m.0 > bed[0] + BED_EPS_MM
                || m.1 > bed[1] + BED_EPS_MM
            {
                return Err(EncodeError::OutOfBed {
                    block: bi,
                    index: mi,
                    x_mm: to.x,
                    y_mm: to.y,
                });
            }
            match *mv {
                Move::Cut { power_pct, .. } => {
                    if !power_pct.is_finite() || power_pct < 0.0 {
                        return Err(EncodeError::NonFinite {
                            block: bi,
                            what: "la potencia",
                        });
                    }
                    if power_pct > 0.0 {
                        max_power = max_power.max(power_pct);
                        let level = (power_pct * 100.0).round() as u32;
                        if !levels.contains(&level) {
                            levels.push(level);
                        }
                    }
                }
                Move::Dwell { ms, power_pct, .. } => {
                    warnings.push(Warning::DwellSkipped {
                        block: bi,
                        ms,
                        fires: power_pct > 0.0,
                    });
                }
                Move::Travel { .. } => {}
            }
        }
        if max_power > 0.0 {
            warnings.push(Warning::PowerIgnored {
                block: bi,
                pct: max_power,
            });
        }
        if levels.len() > 1 {
            warnings.push(Warning::GrayThresholded { block: bi });
        }
        if block.air_assist {
            warnings.push(Warning::AirAssistIgnored { block: bi });
        }
        if block.z_mm.is_some() {
            warnings.push(Warning::ZIgnored { block: bi });
        }
    }

    // 2) Codificación.
    let mut enc = Encoder::new(cfg);
    for (bi, block) in toolpath.blocks.iter().enumerate() {
        let code = &codes[bi];
        if enc.mode == Mode::Program && enc.code.as_deref() != Some(code) {
            enc.exit_program();
        }
        let mm_s = block.speed_mm_min / 60.0;
        let before = enc.stats.burn_mm + enc.stats.travel_mm;
        for mv in &block.moves {
            match *mv {
                Move::Travel { to } => enc.travel(to_mil(to)),
                Move::Cut { to, power_pct } => enc.cut(to_mil(to), power_pct > 0.0, code),
                Move::Dwell { .. } => {}
            }
        }
        enc.stats.est_seconds += (enc.stats.burn_mm + enc.stats.travel_mm - before) / mm_s;
    }
    enc.exit_program();
    enc.flush_pending();

    Ok(Program {
        stream: enc.out,
        start_mil: cfg.start_mil,
        end_mil: enc.pos,
        warnings,
        stats: enc.stats,
        board: cfg.board,
        bed_mm: bed,
        home: cfg.home,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::{Event, MoveMode, decode_stream};
    use tlaku_core::toolpath::ToolpathBlock;

    fn cut(x: f64, y: f64) -> Move {
        Move::Cut {
            to: Point::new(x, y),
            power_pct: 50.0,
        }
    }

    fn travel(x: f64, y: f64) -> Move {
        Move::Travel {
            to: Point::new(x, y),
        }
    }

    fn block(speed_mm_min: f64, moves: Vec<Move>) -> ToolpathBlock {
        ToolpathBlock {
            label: "t".into(),
            speed_mm_min,
            moves,
            ..Default::default()
        }
    }

    fn tp(blocks: Vec<ToolpathBlock>) -> Toolpath {
        Toolpath { blocks }
    }

    /// Cama de 300 x 200 con el home arriba a la izquierda; el punto (x, y) de Tlaku queda en
    /// X = x, Y = 200 - y de la placa.
    fn cfg() -> EncodeConfig {
        EncodeConfig::default()
    }

    fn s(p: &Program) -> String {
        String::from_utf8(p.stream.clone()).unwrap()
    }

    #[test]
    fn distance_alphabet_of_meerk40t() {
        assert_eq!(distance(0), "");
        assert_eq!(distance(1), "a");
        assert_eq!(distance(25), "y");
        assert_eq!(distance(26), "|a");
        assert_eq!(distance(51), "|z");
        assert_eq!(distance(52), "052");
        assert_eq!(distance(254), "254");
        assert_eq!(distance(255), "z");
        assert_eq!(distance(256), "za");
        assert_eq!(distance(787), "zzzv");
        assert_eq!(distance(394), "z139");
        assert_eq!(distance(510), "zz");
        assert_eq!(distance(511), "zza");
        assert_eq!(distance(255 * 3 + 52), "zzz052");
    }

    #[test]
    fn distances_round_trip_through_the_decoder() {
        for v in (1..3000u64).chain([12000, 12240]) {
            let stream = format!("IB{}S1P", distance(v));
            let ev = decode_stream(stream.as_bytes());
            let moved = ev.iter().find_map(|e| match e {
                Event::Move { to, .. } => Some(to.0),
                _ => None,
            });
            assert_eq!(moved, Some(v as i64), "{v}: {stream}");
        }
    }

    #[test]
    fn bresenham_runs_cover_the_displacement_with_straight_and_diagonal_steps() {
        for (dx, dy) in [
            (10, 0),
            (0, -7),
            (5, 5),
            (-5, 5),
            (10, 3),
            (3, 10),
            (-17, 4),
            (1, 1),
            (100, -37),
            (0, 0),
        ] {
            let runs = line_runs(dx, dy);
            let (sx, sy) = runs.iter().fold((0, 0), |a, r| (a.0 + r.0, a.1 + r.1));
            assert_eq!((sx, sy), (dx, dy));
            for (rx, ry) in &runs {
                assert!(*rx == 0 || *ry == 0 || rx.abs() == ry.abs(), "{rx},{ry}");
            }
        }
        assert_eq!(line_runs(10, 0), [(10, 0)]);
        assert_eq!(line_runs(4, 4), [(4, 4)]);
        assert!(line_runs(0, 0).is_empty());
    }

    #[test]
    fn bresenham_stays_within_half_a_mil_of_the_ideal_line() {
        let (dx, dy) = (1000i64, 333i64);
        let (mut x, mut y) = (0i64, 0i64);
        for (rx, ry) in line_runs(dx, dy) {
            // Recorre el tramo paso a paso.
            let n = rx.abs().max(ry.abs());
            for _ in 0..n {
                x += rx.signum();
                y += ry.signum();
                let ideal_y = x as f64 * dy as f64 / dx as f64;
                assert!((y as f64 - ideal_y).abs() <= 1.0, "({x},{y}) vs {ideal_y}");
            }
        }
        assert_eq!((x, y), (dx, dy));
    }

    #[test]
    fn meerk40t_rectangle_is_reproduced_byte_for_byte_except_the_safety_u() {
        // Rectángulo de 1 cm en (2 cm, 2 cm) a 15 mm/s. En el sistema de la placa: salto a
        // (787, 787) y cuatro lados de 394 mil. Pasado a Tlaku (home arriba-izquierda, Y arriba):
        // x = 787 mil = 19,9898 mm; y = 200 - 19,9898 mm.
        let ox = 787.0 * MM_PER_MIL;
        let oy = 200.0 - 787.0 * MM_PER_MIL;
        let side = 394.0 * MM_PER_MIL;
        let path = tp(vec![block(
            900.0,
            vec![
                travel(ox, oy),
                cut(ox + side, oy),
                cut(ox + side, oy - side),
                cut(ox, oy - side),
                cut(ox, oy),
            ],
        )]);
        let mut c = cfg();
        c.constant_speed = true; // MeerK40t usa `C` por defecto ("twitches" desactivado).
        let p = encode(&path, &c).unwrap();
        // Esperado de MeerK40t (`egv_rect`), con una `U` de seguridad antes de `FNSE`.
        let expected = "IBzzzvRzzzvS1P\nICV2490731016000027CNLBS1EDz139Rz139Tz139Lz139FNSE-\n";
        assert_eq!(s(&p).replace("UFNSE", "FNSE"), expected);
        assert!(s(&p).contains("Lz139UFNSE-\n"));
        assert_eq!(p.end_mil, (787, 787));
        assert_eq!(p.stats.programs, 1);
        assert_eq!(p.stats.rapids, 1);
        assert!((p.stats.burn_mm - 4.0 * 394.0 * MM_PER_MIL).abs() < 0.05);
    }

    #[test]
    fn default_speed_uses_the_gear_of_the_chinese_software() {
        // 15 mm/s sin `C`: engranaje 1 con razón diagonal.
        let path = tp(vec![block(
            900.0,
            vec![travel(10.0, 190.0), cut(20.0, 190.0)],
        )]);
        let p = encode(&path, &cfg()).unwrap();
        let code = s(&p);
        let v = code.find("CV").unwrap();
        let end = code[v..].find('N').unwrap();
        let sc = &code[v..v + end];
        assert!(!sc.ends_with('C'), "{sc}");
        let back = speed::speed_from_code(sc, Board::M2, false).unwrap();
        assert!((back - 15.0).abs() < 0.2, "{sc} -> {back}");
    }

    #[test]
    fn decoding_the_program_gives_back_the_toolpath_geometry() {
        let path = tp(vec![block(
            1200.0,
            vec![
                travel(20.0, 180.0),
                cut(60.0, 180.0),
                cut(60.0, 140.0),
                cut(20.0, 100.0),
                cut(20.0, 180.0),
            ],
        )]);
        let p = encode(&path, &cfg()).unwrap();
        let ev = decode_stream(&p.stream);
        // Posición final de la placa.
        let last = ev.iter().rev().find_map(|e| match e {
            Event::Move { to, .. } => Some(*to),
            _ => None,
        });
        assert_eq!(last, Some(p.end_mil));
        // Todo trazo con láser encendido está a menos de 1,5 mil de algún segmento cortado.
        let seg: Vec<((f64, f64), (f64, f64))> = [
            ((20.0, 180.0), (60.0, 180.0)),
            ((60.0, 180.0), (60.0, 140.0)),
            ((60.0, 140.0), (20.0, 100.0)),
            ((20.0, 100.0), (20.0, 180.0)),
        ]
        .iter()
        .map(|&((ax, ay), (bx, by))| {
            // A mm de la placa (Y invertida: home arriba).
            ((ax, 200.0 - ay), (bx, 200.0 - by))
        })
        .collect();
        let mut burned = 0.0;
        for e in &ev {
            if let Event::Move {
                from,
                to,
                laser: true,
                mode: MoveMode::Program,
            } = e
            {
                let (fx, fy) = (mil_to_mm(from.0), mil_to_mm(from.1));
                let (tx, ty) = (mil_to_mm(to.0), mil_to_mm(to.1));
                burned += ((tx - fx).powi(2) + (ty - fy).powi(2)).sqrt();
                for (px, py) in [(fx, fy), (tx, ty), ((fx + tx) / 2.0, (fy + ty) / 2.0)] {
                    let d = seg
                        .iter()
                        .map(|&(a, b)| dist_point_segment((px, py), a, b))
                        .fold(f64::INFINITY, f64::min);
                    assert!(d < 0.05, "punto quemado a {d} mm del trazado: {px},{py}");
                }
            }
        }
        let want = 40.0 + 40.0 + (40.0f64.powi(2) + 40.0f64.powi(2)).sqrt() + 80.0;
        assert!((burned - want).abs() < 0.3, "{burned} vs {want}");
    }

    fn dist_point_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
        let (abx, aby) = (b.0 - a.0, b.1 - a.1);
        let l2 = abx * abx + aby * aby;
        let t = if l2 == 0.0 {
            0.0
        } else {
            (((p.0 - a.0) * abx + (p.1 - a.1) * aby) / l2).clamp(0.0, 1.0)
        };
        let (cx, cy) = (a.0 + t * abx, a.1 + t * aby);
        ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt()
    }

    #[test]
    fn long_travels_use_a_rapid_and_do_not_burn() {
        let path = tp(vec![block(
            1200.0,
            vec![
                travel(10.0, 190.0),
                cut(20.0, 190.0),
                travel(100.0, 100.0),
                cut(110.0, 100.0),
            ],
        )]);
        let p = encode(&path, &cfg()).unwrap();
        let text = s(&p);
        assert_eq!(text.matches("FNSE-").count(), 2, "{text}");
        assert_eq!(p.stats.programs, 2);
        assert_eq!(p.stats.rapids, 2);
        let ev = decode_stream(&p.stream);
        for e in &ev {
            if let Event::Move { laser, mode, .. } = e
                && *mode == MoveMode::Rapid
            {
                assert!(!laser, "un salto rápido no puede quemar");
            }
        }
        assert!(p.stats.burn_mm > 19.0 && p.stats.burn_mm < 21.0);
    }

    #[test]
    fn short_travels_stay_inside_the_program_with_the_laser_off() {
        // Dos trazos separados 2 mm: sin salir del modo compacto.
        let path = tp(vec![block(
            1200.0,
            vec![
                travel(10.0, 190.0),
                cut(20.0, 190.0),
                travel(22.0, 190.0),
                cut(30.0, 190.0),
            ],
        )]);
        let p = encode(&path, &cfg()).unwrap();
        let text = s(&p);
        assert_eq!(text.matches("S1E").count(), 1, "{text}");
        assert_eq!(p.stats.rapids, 1, "solo el salto inicial");
        assert!((p.stats.travel_mm - 2.0).abs() < 0.05);
        let ev = decode_stream(&p.stream);
        let on: Vec<_> = ev
            .iter()
            .filter(|e| matches!(e, Event::Move { laser: true, .. }))
            .collect();
        assert_eq!(on.len(), 2);
    }

    #[test]
    fn power_zero_cuts_are_moves_with_the_laser_off() {
        let path = tp(vec![block(
            1200.0,
            vec![
                travel(10.0, 190.0),
                Move::Cut {
                    to: Point::new(15.0, 190.0),
                    power_pct: 0.0,
                },
                cut(25.0, 190.0),
                Move::Cut {
                    to: Point::new(30.0, 190.0),
                    power_pct: 0.0,
                },
            ],
        )]);
        let p = encode(&path, &cfg()).unwrap();
        let ev = decode_stream(&p.stream);
        let burned: f64 = ev
            .iter()
            .filter_map(|e| match e {
                Event::Move {
                    from,
                    to,
                    laser: true,
                    ..
                } => Some(mil_to_mm((to.0 - from.0).abs() + (to.1 - from.1).abs())),
                _ => None,
            })
            .sum();
        assert!((burned - 10.0).abs() < 0.1, "{burned}");
        assert!((p.stats.burn_mm - 10.0).abs() < 0.1);
        assert!((p.stats.travel_mm - 10.0).abs() < 0.1);
    }

    #[test]
    fn an_explicit_u_precedes_the_exit_when_the_laser_is_on() {
        let path = tp(vec![block(
            600.0,
            vec![travel(10.0, 190.0), cut(20.0, 190.0)],
        )]);
        let p = encode(&path, &cfg()).unwrap();
        assert!(s(&p).ends_with("UFNSE-\n"), "{}", s(&p));
    }

    #[test]
    fn blocks_with_the_same_speed_share_one_program_and_different_speeds_do_not() {
        let mk = |sp1: f64, sp2: f64| {
            tp(vec![
                block(sp1, vec![travel(10.0, 190.0), cut(20.0, 190.0)]),
                block(sp2, vec![travel(20.0, 188.0), cut(30.0, 188.0)]),
            ])
        };
        let same = encode(&mk(1200.0, 1200.0), &cfg()).unwrap();
        assert_eq!(same.stats.programs, 1);
        let diff = encode(&mk(1200.0, 3000.0), &cfg()).unwrap();
        assert_eq!(diff.stats.programs, 2);
        assert!(s(&diff).matches("FNSE-").count() == 2);
    }

    #[test]
    fn warnings_for_what_the_board_cannot_do() {
        let mut b = block(
            1200.0,
            vec![
                travel(10.0, 190.0),
                Move::Cut {
                    to: Point::new(20.0, 190.0),
                    power_pct: 30.0,
                },
                Move::Cut {
                    to: Point::new(30.0, 190.0),
                    power_pct: 60.0,
                },
                Move::Dwell {
                    to: Point::new(30.0, 190.0),
                    ms: 50.0,
                    power_pct: 40.0,
                },
            ],
        );
        b.air_assist = true;
        b.z_mm = Some(2.0);
        let p = encode(&tp(vec![b]), &cfg()).unwrap();
        let w = &p.warnings;
        assert!(w.contains(&Warning::PowerIgnored {
            block: 0,
            pct: 60.0
        }));
        assert!(w.contains(&Warning::GrayThresholded { block: 0 }));
        assert!(w.contains(&Warning::AirAssistIgnored { block: 0 }));
        assert!(w.contains(&Warning::ZIgnored { block: 0 }));
        assert!(w.contains(&Warning::DwellSkipped {
            block: 0,
            ms: 50.0,
            fires: true
        }));
        for x in w {
            assert!(!x.to_string().is_empty());
        }
        // Y el láser nunca queda encendido en el sitio.
        let ev = decode_stream(&p.stream);
        assert!(
            !ev.iter()
                .any(|e| matches!(e, Event::Move { from, to, laser: true, .. } if from == to))
        );
    }

    #[test]
    fn binary_power_has_no_gray_warning_and_zero_power_has_no_power_warning() {
        let p = encode(
            &tp(vec![block(
                1200.0,
                vec![travel(10.0, 190.0), cut(20.0, 190.0)],
            )]),
            &cfg(),
        )
        .unwrap();
        assert!(
            !p.warnings
                .iter()
                .any(|w| matches!(w, Warning::GrayThresholded { .. }))
        );
        let p = encode(
            &tp(vec![block(
                1200.0,
                vec![
                    travel(10.0, 190.0),
                    Move::Cut {
                        to: Point::new(20.0, 190.0),
                        power_pct: 0.0,
                    },
                ],
            )]),
            &cfg(),
        )
        .unwrap();
        assert!(p.warnings.is_empty());
    }

    #[test]
    fn out_of_bed_nan_and_bad_speed_are_errors_before_any_output() {
        let c = cfg();
        let e = encode(
            &tp(vec![block(
                1200.0,
                vec![travel(10.0, 190.0), cut(301.0, 190.0)],
            )]),
            &c,
        )
        .unwrap_err();
        assert!(matches!(e, EncodeError::OutOfBed { index: 1, .. }), "{e}");
        let e = encode(&tp(vec![block(1200.0, vec![travel(-1.0, 190.0)])]), &c).unwrap_err();
        assert!(matches!(e, EncodeError::OutOfBed { .. }));
        let e = encode(&tp(vec![block(1200.0, vec![travel(10.0, 201.0)])]), &c).unwrap_err();
        assert!(matches!(e, EncodeError::OutOfBed { .. }));
        let e = encode(&tp(vec![block(1200.0, vec![travel(f64::NAN, 1.0)])]), &c).unwrap_err();
        assert!(matches!(e, EncodeError::NonFinite { .. }));
        // Velocidad: cero, NaN, por encima del máximo y demasiado lenta.
        for sp in [0.0, f64::NAN, -5.0] {
            let e = encode(&tp(vec![block(sp, vec![travel(1.0, 1.0)])]), &c).unwrap_err();
            assert!(matches!(e, EncodeError::NonFinite { .. }), "{sp}: {e}");
        }
        let e = encode(&tp(vec![block(60.0 * 141.0, vec![travel(1.0, 1.0)])]), &c).unwrap_err();
        assert!(matches!(e, EncodeError::Speed { .. }));
        let e = encode(&tp(vec![block(60.0 * 0.1, vec![travel(1.0, 1.0)])]), &c).unwrap_err();
        assert!(matches!(e, EncodeError::Speed { .. }), "{e}");
        // Un bloque inválido tras uno válido: no se produce nada.
        let bad = tp(vec![
            block(1200.0, vec![travel(10.0, 190.0), cut(20.0, 190.0)]),
            block(1200.0, vec![cut(500.0, 10.0)]),
        ]);
        assert!(encode(&bad, &c).is_err());
    }

    #[test]
    fn bed_edge_with_rounding_is_accepted() {
        let c = cfg();
        let p = encode(
            &tp(vec![block(
                1200.0,
                vec![travel(0.0, 200.0), cut(300.0, 200.0), cut(300.0, 0.0)],
            )]),
            &c,
        )
        .unwrap();
        assert_eq!(p.end_mil, (mm_to_mil(300.0), mm_to_mil(200.0)));
    }

    #[test]
    fn home_corner_flips_the_axes() {
        for (corner, start_tlaku, end_tlaku, want_end) in [
            (
                HomeCorner::TopLeft,
                (0.0, 200.0),
                (50.0, 150.0),
                (50.0, 50.0),
            ),
            (
                HomeCorner::BottomLeft,
                (0.0, 0.0),
                (50.0, 50.0),
                (50.0, 50.0),
            ),
            (
                HomeCorner::TopRight,
                (300.0, 200.0),
                (250.0, 150.0),
                (50.0, 50.0),
            ),
            (
                HomeCorner::BottomRight,
                (300.0, 0.0),
                (250.0, 50.0),
                (50.0, 50.0),
            ),
        ] {
            let mut c = cfg();
            c.home = corner;
            let p = encode(
                &tp(vec![block(
                    1200.0,
                    vec![
                        travel(start_tlaku.0, start_tlaku.1),
                        travel(end_tlaku.0, end_tlaku.1),
                    ],
                )]),
                &c,
            )
            .unwrap();
            let want = (mm_to_mil(want_end.0), mm_to_mil(want_end.1));
            assert_eq!(p.end_mil, want, "{}", corner.name());
            let back = p.end_mm();
            assert!((back.x - end_tlaku.0).abs() < 0.02 && (back.y - end_tlaku.1).abs() < 0.02);
        }
        assert_eq!(HomeCorner::parse("Top-Left"), Some(HomeCorner::TopLeft));
        assert_eq!(HomeCorner::parse("centro"), None);
    }

    #[test]
    fn autolock_off_appends_unlock_after_each_rapid() {
        let mut c = cfg();
        c.autolock = false;
        let p = encode(&tp(vec![block(1200.0, vec![travel(50.0, 100.0)])]), &c).unwrap();
        assert!(s(&p).ends_with("S1P\nIS2P\n"), "{}", s(&p));
        let p = encode(&tp(vec![block(1200.0, vec![travel(50.0, 100.0)])]), &cfg()).unwrap();
        assert!(s(&p).ends_with("S1P\n") && !s(&p).contains("S2P"));
    }

    #[test]
    fn frames_have_valid_bytes_and_wait_only_at_program_exit() {
        let path = tp(vec![block(
            1200.0,
            vec![
                travel(10.0, 190.0),
                cut(60.0, 190.0),
                cut(60.0, 130.0),
                cut(10.0, 130.0),
                cut(10.0, 190.0),
            ],
        )]);
        let p = encode(&path, &cfg()).unwrap();
        let frames = p.frames().unwrap();
        assert!(frames.len() >= 3);
        let waits: Vec<_> = frames.iter().filter(|f| f.wait_finish).collect();
        assert_eq!(waits.len(), 1);
        assert!(frames.last().unwrap().wait_finish);
        assert!(frames.last().unwrap().text().ends_with("FNSE"));
    }

    #[test]
    fn egv_file_has_the_header_of_the_lhymicro_gl_format() {
        let p = encode(&tp(vec![block(1200.0, vec![travel(50.0, 100.0)])]), &cfg()).unwrap();
        let f = String::from_utf8(p.to_egv_file("Tlaku test")).unwrap();
        assert!(f.starts_with("Document type : LHYMICRO-GL file\nFile version: 1.0.01\n"));
        assert!(f.contains("Creator-Software: Tlaku test\n\n%0%0%0%0%\nI"));
    }

    #[test]
    fn empty_toolpath_gives_an_empty_program() {
        let p = encode(&Toolpath::default(), &cfg()).unwrap();
        assert!(p.stream.is_empty());
        assert_eq!(p.end_mil, p.start_mil);
        assert!(p.frames().unwrap().is_empty());
    }

    #[test]
    fn start_position_is_the_origin_of_the_first_rapid() {
        let mut c = cfg();
        c.start_mil = (100, 50);
        let p = encode(&tp(vec![block(1200.0, vec![travel(0.0, 200.0)])]), &c).unwrap();
        // De (100, 50) a (0, 0): izquierda 100 y arriba 50.
        assert_eq!(s(&p), "IT100L|yS1P\n");
    }

    /// Generador congruencial para pruebas deterministas (sin dependencias).
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((self.0 >> 33) as f64) / ((1u64 << 31) as f64)
        }
        fn range(&mut self, lo: f64, hi: f64) -> f64 {
            lo + (hi - lo) * self.next()
        }
    }

    #[test]
    fn random_toolpaths_round_trip_through_the_decoder() {
        let mut rng = Lcg(0x5EED);
        for case in 0..40 {
            // Trayectorias con tramos largos, cortos, diagonales y viajes de todos los tamaños.
            let mut moves = Vec::new();
            let mut cur = Point::new(rng.range(0.0, 300.0), rng.range(0.0, 200.0));
            moves.push(Move::Travel { to: cur });
            let mut cuts: Vec<((f64, f64), (f64, f64))> = Vec::new();
            for _ in 0..(5 + (rng.next() * 25.0) as usize) {
                let kind = rng.next();
                let next = if kind < 0.3 {
                    // Tramo corto.
                    Point::new(
                        (cur.x + rng.range(-1.5, 1.5)).clamp(0.0, 300.0),
                        (cur.y + rng.range(-1.5, 1.5)).clamp(0.0, 200.0),
                    )
                } else if kind < 0.45 {
                    // Diagonal exacta.
                    let d = rng.range(-20.0, 20.0);
                    Point::new((cur.x + d).clamp(0.0, 300.0), (cur.y + d).clamp(0.0, 200.0))
                } else {
                    Point::new(rng.range(0.0, 300.0), rng.range(0.0, 200.0))
                };
                if rng.next() < 0.7 {
                    let power = if rng.next() < 0.15 { 0.0 } else { 40.0 };
                    moves.push(Move::Cut {
                        to: next,
                        power_pct: power,
                    });
                    if power > 0.0 {
                        cuts.push(((cur.x, 200.0 - cur.y), (next.x, 200.0 - next.y)));
                    }
                } else {
                    moves.push(Move::Travel { to: next });
                }
                cur = next;
            }
            let speed = rng.range(300.0, 6000.0);
            let path = tp(vec![block(speed, moves)]);
            let p = encode(&path, &cfg()).unwrap();
            // El flujo siempre se puede partir en paquetes válidos.
            let frames = p.frames().unwrap_or_else(|e| panic!("caso {case}: {e}"));
            assert!(
                frames
                    .iter()
                    .all(|f| f.payload.iter().all(|&b| b.is_ascii()))
            );
            let ev = decode_stream(&p.stream);
            // Posición final.
            let last = ev.iter().rev().find_map(|e| match e {
                Event::Move { to, .. } => Some(*to),
                _ => None,
            });
            if p.end_mil != p.start_mil {
                assert_eq!(last, Some(p.end_mil), "caso {case}");
            }
            // Ningún salto rápido enciende el láser; todo lo quemado está sobre un corte.
            let mut burned = 0.0;
            for e in &ev {
                if let Event::Move {
                    from,
                    to,
                    laser,
                    mode,
                } = e
                {
                    if *mode == MoveMode::Rapid {
                        assert!(!laser, "caso {case}: salto rápido con láser");
                    }
                    if *laser {
                        let (f, t) = (
                            (mil_to_mm(from.0), mil_to_mm(from.1)),
                            (mil_to_mm(to.0), mil_to_mm(to.1)),
                        );
                        burned += ((t.0 - f.0).powi(2) + (t.1 - f.1).powi(2)).sqrt();
                        // Las rectas Bresenham pueden separarse hasta 1 mil de la ideal.
                        for q in [f, t, ((f.0 + t.0) / 2.0, (f.1 + t.1) / 2.0)] {
                            let d = cuts
                                .iter()
                                .map(|&(a, b)| dist_point_segment(q, a, b))
                                .fold(f64::INFINITY, f64::min);
                            assert!(d < 0.08, "caso {case}: quemó a {d} mm de un corte en {q:?}");
                        }
                    }
                }
            }
            // Los extremos de cada corte los visita el cabezal.
            let ends: Vec<(f64, f64)> = ev
                .iter()
                .filter_map(|e| match e {
                    Event::Move { from, to, .. } => Some([
                        (mil_to_mm(from.0), mil_to_mm(from.1)),
                        (mil_to_mm(to.0), mil_to_mm(to.1)),
                    ]),
                    _ => None,
                })
                .flatten()
                .collect();
            for &(a, b) in &cuts {
                for q in [a, b] {
                    let d = ends
                        .iter()
                        .map(|e| ((e.0 - q.0).powi(2) + (e.1 - q.1).powi(2)).sqrt())
                        .fold(f64::INFINITY, f64::min);
                    assert!(d < 0.05, "caso {case}: extremo {q:?} a {d} mm");
                }
            }
            // La longitud ideal que informa la estadística coincide con la de los cortes.
            let want: f64 = cuts
                .iter()
                .map(|&(a, b)| ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt())
                .sum();
            let tol = 0.04 * cuts.len() as f64 + 0.2;
            assert!(
                (p.stats.burn_mm - want).abs() <= tol,
                "caso {case}: {} vs {want}",
                p.stats.burn_mm
            );
            // La trayectoria real (con diagonales) no es más de ~42 % más larga que la recta.
            assert!(
                burned <= want * 1.42 + tol,
                "caso {case}: {burned} vs {want}"
            );
            assert!(
                burned + tol >= want * 0.99,
                "caso {case}: {burned} vs {want}"
            );
        }
    }

    #[test]
    fn diagonal_cuts_use_the_m_command() {
        let p = encode(
            &tp(vec![block(
                1200.0,
                vec![travel(10.0, 190.0), cut(20.0, 180.0)],
            )]),
            &cfg(),
        )
        .unwrap();
        assert!(s(&p).contains("M"), "{}", s(&p));
        let ev = decode_stream(&p.stream);
        let end = ev
            .iter()
            .rev()
            .find_map(|e| match e {
                Event::Move { to, .. } => Some(*to),
                _ => None,
            })
            .unwrap();
        assert_eq!(end, p.end_mil);
    }
}
