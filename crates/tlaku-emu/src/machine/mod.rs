//! Núcleo del emulador: una controladora completa como máquina de estados que avanza con
//! tiempo simulado. No tiene hilos ni relojes propios: se le dan bytes con [`Machine::receive`],
//! se le hace avanzar con [`Machine::advance_to`] y se leen sus respuestas con
//! [`Machine::take_output`]. [`crate::VirtualMachine`] la envuelve como `Transport`.
//!
//! Qué modela (ver la documentación del crate para el detalle y los límites):
//!
//! - Búfer RX circular (`rx_size - 1` bytes utilizables) que se llena de verdad si el
//!   intérprete está bloqueado; los bytes que no caben se pierden y se cuentan.
//! - Planificador de `planner_blocks` bloques: una línea de movimiento recibe `ok` al entrar;
//!   con el planificador lleno el intérprete se bloquea (y el RX se acumula).
//! - Estados y comandos en tiempo real de GRBL 1.1 (y extras de grblHAL).
//! - Cinemática trapezoidal con planificador de velocidades de GRBL: pasada hacia atrás sobre
//!   los bloques del búfer (la ventana) y velocidad de unión por desviación de unión (`$11`).
//!   Láser M3/M4/M5 con `$32`, rastro del láser.

mod access;
mod dollar;
mod faults;
mod link;
mod motion;
mod pump;
mod realtime;
mod reprap;

use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

use crate::fault::Scheduled;
use crate::model::{Action, Block, Gc};
use crate::probe::ProbeSurface;
use crate::profile::{LineEnding, Profile, Setting, format_value};
use crate::trace::{LaserTrace, SpindleMode};

/// Paso de simulación.
const STEP: Duration = Duration::from_millis(1);
/// Velocidad mínima (mm/s) para que un bloque siempre termine.
const MIN_SPEED: f64 = 0.5;
/// Máximo de bytes crudos de una línea antes de descartar el resto.
const MAX_RAW_LINE: usize = 2048;

/// Opciones de arranque de la máquina emulada.
#[derive(Debug, Clone)]
pub struct Options {
    /// Imprime el banner al crearla (equivale a abrir el puerto y que la placa se reinicie).
    pub banner_on_connect: bool,
    /// Posición de máquina inicial.
    pub start_position: [f64; 3],
    /// Arranca ya referenciada y sin el bloqueo de alarma que exige `$22=1`.
    pub homed: bool,
    /// Paso de simulación.
    pub step: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            banner_on_connect: true,
            start_position: [0.0; 3],
            homed: false,
            step: STEP,
        }
    }
}

/// Lo que recibió la controladora, para inspección de las pruebas.
#[derive(Debug, Clone, PartialEq)]
pub enum Received {
    /// Línea completa tal como llegó (sin terminador).
    Line(String),
    /// Byte en tiempo real.
    Realtime(u8),
}

/// Entrada del registro de recepción.
#[derive(Debug, Clone, PartialEq)]
pub struct LogEntry {
    pub at: Duration,
    pub what: Received,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reply {
    Normal,
    Error(u32),
    Drop,
}

#[derive(Debug, Clone, Copy)]
struct LineCtx {
    bytes: usize,
    reply: Reply,
}

struct Pending {
    actions: VecDeque<Action>,
    dwell_until: Option<Duration>,
    awaiting_resume: bool,
    homing: bool,
}

type PlanKey = (u64, u32, u32, [u64; 7]);

struct Exec {
    block: Block,
    done: f64,
}

struct Homing {
    start: Duration,
    end: Duration,
    from: [f64; 3],
}

#[derive(Default, Clone, Copy)]
struct ReportState {
    wco_counter: u32,
    ov_counter: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ResetKind {
    /// Reset pedido por el anfitrión: ALARM:3 si había movimiento.
    FromHost,
    /// Reinicio de la placa en pleno movimiento.
    Motion,
    /// Reinicio en reposo.
    Quiet,
}

/// Controladora emulada.
pub struct Machine {
    pub(crate) profile: Profile,
    opts: Options,
    now: Duration,
    // Enlace.
    out: VecDeque<u8>,
    held: Vec<u8>,
    silence: Option<(Duration, bool)>,
    connected: bool,
    line_ending: LineEnding,
    rx: VecDeque<u8>,
    line_buf: Vec<u8>,
    line_bytes: usize,
    in_flight: usize,
    max_in_flight: usize,
    overflow_bytes: usize,
    log: Vec<LogEntry>,
    pub(crate) lines_seen: usize,
    line_ctx: LineCtx,
    next_reply: Option<Reply>,
    last_error: Option<u32>,
    // Parámetros persistentes.
    settings: BTreeMap<u32, Setting>,
    user_info: String,
    startup: [String; 2],
    pub(crate) wcs: [[f64; 3]; 6],
    pub(crate) g28: [f64; 3],
    pub(crate) g30: [f64; 3],
    pub(crate) g92: [f64; 3],
    /// Lo que toca la sonda (coordenadas de máquina); `None` = nada al alcance.
    probe_surface: Option<ProbeSurface>,
    /// Última sonda: posición de máquina y si hubo contacto (`[PRB:...]` de `$#`).
    probe_last: ([f64; 3], bool),
    // Estado modal.
    pub(crate) gc: Gc,
    // Sistema.
    alarm: Option<u32>,
    hold: Option<bool>,
    door: Option<u8>,
    door_open: bool,
    door_resume_at: Option<Duration>,
    check: bool,
    sleeping: bool,
    homing: Option<Homing>,
    halted: bool,
    tool_pending: bool,
    limit_lock: bool,
    position_lost: bool,
    homed: bool,
    jog_cancel: bool,
    block_done: bool,
    resets: usize,
    // Movimiento.
    pos: [f64; 3],
    v: f64,
    planner: VecDeque<Block>,
    /// Cambia cada vez que se altera el planificador; invalida `plan_cache`.
    plan_gen: u64,
    /// Última velocidad de salida calculada por el planificador y la clave con que se calculó.
    plan_cache: Option<(PlanKey, f64)>,
    /// Distancia que el último bloque recorrió de más en su paso final (mm).
    carry: f64,
    current: Option<Exec>,
    block_seq: u64,
    pending: Option<Pending>,
    exec_spindle: (SpindleMode, f64),
    spindle_stop_ovr: bool,
    ovr_feed: u32,
    ovr_rapid: u32,
    ovr_spindle: u32,
    // Rastro.
    trace: LaserTrace,
    open_seg: Option<(usize, u64, bool)>,
    // Reporte.
    rep: ReportState,
    // Fallas.
    faults: Vec<Scheduled>,
    nan: Option<bool>,
    // Protocolo RepRap.
    /// Velocidad (0-255) de cada ventilador, por índice `P`.
    fans: BTreeMap<u32, f64>,
    /// Modo inline de Marlin (`M3 S0 I`): la potencia viaja con cada movimiento.
    inline: bool,
    /// Último número de línea aceptado (líneas numeradas de Marlin).
    last_line: i64,
    /// Altera un byte de la próxima línea (falla `CorruptLine`).
    corrupt_next: bool,
    /// Bytes de la línea en curso, para el analizador de emergencia.
    ep_buf: Vec<u8>,
}

fn fmt3(v: f64) -> String {
    format_value(v, 3)
}

fn fmt_trim(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Quita comentarios y espacios y pasa a mayúsculas.
fn clean_line(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_paren = false;
    for c in raw.chars() {
        match c {
            ';' if !in_paren => break,
            '(' => in_paren = true,
            ')' => in_paren = false,
            _ if in_paren || c.is_whitespace() || c.is_control() => {}
            _ => out.push(c.to_ascii_uppercase()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_line_strips_and_uppercases() {
        assert_eq!(clean_line("g1 x10.5 (hola) y2 ; fin"), "G1X10.5Y2");
        assert_eq!(clean_line("$i=Texto"), "$I=TEXTO");
        assert_eq!(clean_line("   "), "");
    }

    #[test]
    fn fmt_helpers() {
        assert_eq!(fmt_trim(1000.0), "1000");
        assert_eq!(fmt_trim(0.5), "0.5");
        assert_eq!(fmt3(-0.0), "0.000");
    }
}
