//! Programa codificado, estadísticas y utilidades de texto del lenguaje (distancias, saltos).

use super::{HomeCorner, Warning, mil_to_mm};
use crate::packet::{Frame, FrameError, frame_stream};
use crate::speed::Board;
use tlaku_core::Point;

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
