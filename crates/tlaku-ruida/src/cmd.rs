//! Comandos Ruida con su codificación y un decodificador (bytes a comandos legibles).
//!
//! Los bytes son los del flujo **en claro** (sin swizzle ni checksum de paquete). Un byte con el
//! bit 7 en 1 inicia un comando y los datos solo usan 7 bits, así que el corte del flujo en
//! comandos no necesita una tabla de longitudes (ruida.md sección 3). Aun así cada comando
//! conocido valida su longitud exacta.
//!
//! Las variantes se dividen en dos grupos:
//! - **Confirmadas** (ruida.md sección 4): movimiento y corte, potencia y velocidad, capas, aire,
//!   fin de archivo y de bloque, control del proceso y lectura de memoria.
//! - **De estructura** (`Point2`, `PartPoint`, `PartAbs`, `Arr7`, `Small`, `Marker`, `ShortArg`,
//!   `ElementName`): campos de la cabecera del ejemplo 1 de EduTech cuyo significado exacto
//!   no está documentado. Se codifican y decodifican sin interpretar.
//!
//! Todo lo demás se decodifica como [`Command::Unknown`] (opcode y datos) para poder depurar.

use crate::num::{self, NumError};
use std::fmt;
use std::ops::Range;

/// Mínima o máxima de una potencia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    Min,
    Max,
}

/// Subcódigos de `D8`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessOp {
    /// `D8 00` iniciar proceso.
    Start,
    /// `D8 01` detener.
    Stop,
    /// `D8 02` pausar.
    Pause,
    /// `D8 03` reanudar.
    Resume,
    /// `D8 10` modo de punto de referencia (el ejemplo 1 empieza con `D8 10` + `F0`).
    RefPoint,
    /// `D8 2C` home Z.
    HomeZ,
    /// `D8 2D` home U.
    HomeU,
    /// `D8 2E` enfocar Z.
    FocusZ,
    /// Cualquier otro subcódigo (sin documentar).
    Other(u8),
}

impl ProcessOp {
    pub fn sub(self) -> u8 {
        match self {
            ProcessOp::Start => 0x00,
            ProcessOp::Stop => 0x01,
            ProcessOp::Pause => 0x02,
            ProcessOp::Resume => 0x03,
            ProcessOp::RefPoint => 0x10,
            ProcessOp::HomeZ => 0x2C,
            ProcessOp::HomeU => 0x2D,
            ProcessOp::FocusZ => 0x2E,
            ProcessOp::Other(s) => s,
        }
    }

    pub fn from_sub(s: u8) -> Self {
        match s {
            0x00 => ProcessOp::Start,
            0x01 => ProcessOp::Stop,
            0x02 => ProcessOp::Pause,
            0x03 => ProcessOp::Resume,
            0x10 => ProcessOp::RefPoint,
            0x2C => ProcessOp::HomeZ,
            0x2D => ProcessOp::HomeU,
            0x2E => ProcessOp::FocusZ,
            other => ProcessOp::Other(other),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    // ----- Movimiento y corte (coordenadas en µm) -----
    /// `88 X(5) Y(5)`: mover rápido, láser apagado.
    MoveAbs { x: i64, y: i64 },
    /// `89 dX(2) dY(2)`.
    MoveRel { dx: i32, dy: i32 },
    /// `8A dX(2)`.
    MoveRelX(i32),
    /// `8B dY(2)`.
    MoveRelY(i32),
    /// `A8 X(5) Y(5)`: cortar con el láser encendido.
    CutAbs { x: i64, y: i64 },
    /// `A9 dX(2) dY(2)`.
    CutRel { dx: i32, dy: i32 },
    /// `AA dX(2)`.
    CutRelX(i32),
    /// `AB dY(2)`.
    CutRelY(i32),

    // ----- Potencia y velocidad -----
    /// `C6 01/02` (láser 1), `C6 21/22` (2), `C6 05/06` (3), `C6 07/08` (4): `P(2)` (14 bits).
    LaserPower { laser: u8, limit: Limit, raw: u16 },
    /// `C6 31/32` (láser 1), `41/42` (2), `35/36` (3), `37/38` (4): `capa P(2)`.
    LayerPower {
        laser: u8,
        limit: Limit,
        layer: u8,
        raw: u16,
    },
    /// `C9 02 V(5)`: velocidad en µm/s.
    Speed(u64),
    /// `C9 04 capa V(5)`.
    LayerSpeed { layer: u8, um_s: u64 },
    /// `C6 10..13` (intervalo, retardo añadido, retardo de encendido y apagado) y `C6 15/16`
    /// (aparecen en el ejemplo 1 con valor 0; significado SIN CONFIRMAR): `T(5)`.
    LaserTime { sub: u8, micros: u64 },
    /// `C6 60 láser capa F(5)`: frecuencia en Hz.
    LayerFrequency { laser: u8, layer: u8, hz: u64 },

    // ----- Capas y aire -----
    /// `CA 01 00`: fin de capa.
    EndLayer,
    /// `CA 01 12`: aire apagado.
    AirOff,
    /// `CA 01 13`: aire encendido.
    AirOn,
    /// Otro `CA 01 xx` (modos de trabajo, dispositivo `10`/`11`, `30`/`31`...).
    LayerFlag(u8),
    /// `CA 02 capa`: capa actual.
    LayerSelect(u8),
    /// `CA 05 color(5)`.
    Color(u64),
    /// `CA 06 capa color(5)`: color BBGGRR.
    LayerColor { layer: u8, color: u64 },
    /// `CA 22 capa`: número máximo de capa.
    MaxLayer(u8),
    /// `CA 41 capa modo`: modo de trabajo de la parte (SIN CONFIRMAR el significado del modo).
    LayerMode { layer: u8, mode: u8 },

    // ----- Archivo y proceso -----
    /// `D8 xx`.
    Process(ProcessOp),
    /// `E7 00`: fin de bloque.
    EndBlock,
    /// `D7`: fin de archivo.
    EndOfFile,
    /// `E6 01`: modo absoluto.
    Absolute,
    /// `F0`: fijar punto de referencia.
    SetReference,
    /// `E5 05 suma(5)`: suma de verificación del archivo (conjunto de bytes SIN CONFIRMAR).
    FileChecksum(u64),
    /// `E7 01 nombre\0`.
    FileName(String),
    /// `DA 00 dirección(2)`: consulta de memoria (solo lectura).
    MemRead { addr: u16 },
    /// `DA 01 dirección(2) valor(5)...`: escritura de memoria o respuesta de una lectura. En el
    /// pie del ejemplo 1 lleva dos valores; la sintaxis exacta es SIN CONFIRMAR.
    MemWrite { addr: u16, values: Vec<u64> },

    // ----- Estructura de la cabecera (significado sin documentar) -----
    /// `op sub X(5) Y(5)`: `E7 03/06/07/13/17/23/50/51`, `F1 03`, `F2 03/04/06`.
    Point2 { op: u8, sub: u8, x: i64, y: i64 },
    /// `E7 52/53/61/62 parte X(5) Y(5)`.
    PartPoint { sub: u8, part: u8, x: i64, y: i64 },
    /// `E7 54/55 n valor(5)`: offset de pluma / de capa.
    PartAbs { sub: u8, n: u8, v: i64 },
    /// `op sub` + 7 valores de 14 bits con signo: `E7 04`, `E7 08`, `F2 05`.
    Arr7 { op: u8, sub: u8, v: [i32; 7] },
    /// `op sub n`: `E7 05/24/60`, `F1 00/01/02`, `F2 00/01/07`, `CA 03`, `CA 10`.
    Small { op: u8, sub: u8, n: u8 },
    /// `EB` (sin parámetros; significado SIN CONFIRMAR).
    Marker(u8),
    /// `EA n`.
    ShortArg { op: u8, n: u8 },
    /// `F2 02` + 10 bytes de 7 bits (nombre de elemento "codificado"; SIN CONFIRMAR).
    ElementName([u8; 10]),

    /// Opcode o subcódigo no reconocido: se conservan los bytes tal cual.
    Unknown { op: u8, data: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum DecodeError {
    /// El flujo no empieza con un byte de opcode (bit 7 en 1).
    NotAnOpcode { offset: usize, byte: u8 },
    /// El comando conocido tiene un número de bytes de datos distinto del esperado.
    BadLength {
        offset: usize,
        op: u8,
        expected: usize,
        found: usize,
    },
    /// Un número no se pudo decodificar.
    Num { offset: usize, source: NumError },
    /// El valor no es válido para ese comando.
    Invalid { offset: usize, what: &'static str },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::NotAnOpcode { offset, byte } => write!(
                f,
                "byte {offset}: 0x{byte:02X} no es un opcode (bit 7 en 0); el flujo debe empezar en un comando"
            ),
            DecodeError::BadLength {
                offset,
                op,
                expected,
                found,
            } => write!(
                f,
                "byte {offset}: el comando 0x{op:02X} debería llevar {expected} bytes de datos y lleva {found}"
            ),
            DecodeError::Num { offset, source } => write!(f, "byte {offset}: {source}"),
            DecodeError::Invalid { offset, what } => write!(f, "byte {offset}: {what}"),
        }
    }
}

impl std::error::Error for DecodeError {}

// ----------------------------------------------------------------------------------------
// Tablas de subcódigos de potencia

/// (láser, sub mínima, sub máxima) de `C6` sin capa.
const LASER_POWER: [(u8, u8, u8); 4] = [
    (1, 0x01, 0x02),
    (2, 0x21, 0x22),
    (3, 0x05, 0x06),
    (4, 0x07, 0x08),
];
/// (láser, sub mínima, sub máxima) de `C6` con capa.
const LAYER_POWER: [(u8, u8, u8); 4] = [
    (1, 0x31, 0x32),
    (2, 0x41, 0x42),
    (3, 0x35, 0x36),
    (4, 0x37, 0x38),
];

fn power_sub(table: &[(u8, u8, u8); 4], laser: u8, limit: Limit) -> Option<u8> {
    table.iter().find(|t| t.0 == laser).map(|t| match limit {
        Limit::Min => t.1,
        Limit::Max => t.2,
    })
}

fn power_from_sub(table: &[(u8, u8, u8); 4], sub: u8) -> Option<(u8, Limit)> {
    table.iter().find_map(|&(l, mn, mx)| {
        if sub == mn {
            Some((l, Limit::Min))
        } else if sub == mx {
            Some((l, Limit::Max))
        } else {
            None
        }
    })
}

fn is_time_sub(sub: u8) -> bool {
    matches!(sub, 0x10..=0x13 | 0x15 | 0x16)
}

fn bad(what: &'static str, v: impl ToString) -> NumError {
    NumError::OutOfRange {
        what,
        value: v.to_string(),
    }
}

fn push_op(op: u8, out: &mut Vec<u8>) -> Result<(), NumError> {
    if op < 0x80 {
        return Err(bad("opcode (debe tener el bit 7 en 1)", op));
    }
    out.push(op);
    Ok(())
}

fn push_data(b: u8, what: &'static str, out: &mut Vec<u8>) -> Result<(), NumError> {
    if b > 0x7F {
        return Err(bad(what, b));
    }
    out.push(b);
    Ok(())
}

impl Command {
    /// Añade los bytes del comando a `out`. Si falla, `out` queda como estaba.
    pub fn encode(&self, out: &mut Vec<u8>) -> Result<(), NumError> {
        let start = out.len();
        let r = self.encode_inner(out);
        if r.is_err() {
            out.truncate(start);
        }
        r
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, NumError> {
        let mut v = Vec::new();
        self.encode(&mut v)?;
        Ok(v)
    }

    fn encode_inner(&self, out: &mut Vec<u8>) -> Result<(), NumError> {
        use Command::*;
        match self {
            MoveAbs { x, y } => abs_pair(0x88, None, *x, *y, out),
            CutAbs { x, y } => abs_pair(0xA8, None, *x, *y, out),
            MoveRel { dx, dy } => rel_pair(0x89, *dx, *dy, out),
            CutRel { dx, dy } => rel_pair(0xA9, *dx, *dy, out),
            MoveRelX(d) => rel_one(0x8A, *d, out),
            MoveRelY(d) => rel_one(0x8B, *d, out),
            CutRelX(d) => rel_one(0xAA, *d, out),
            CutRelY(d) => rel_one(0xAB, *d, out),
            LaserPower { laser, limit, raw } => {
                let sub = power_sub(&LASER_POWER, *laser, *limit)
                    .ok_or_else(|| bad("láser (1 a 4)", laser))?;
                out.extend([0xC6, sub]);
                num::encode_power(*raw, out)
            }
            LayerPower {
                laser,
                limit,
                layer,
                raw,
            } => {
                let sub = power_sub(&LAYER_POWER, *laser, *limit)
                    .ok_or_else(|| bad("láser (1 a 4)", laser))?;
                out.extend([0xC6, sub]);
                num::encode_int7(*layer, "capa", out)?;
                num::encode_power(*raw, out)
            }
            Speed(v) => {
                out.extend([0xC9, 0x02]);
                num::encode_wide(*v, "velocidad", out)
            }
            LayerSpeed { layer, um_s } => {
                out.extend([0xC9, 0x04]);
                num::encode_int7(*layer, "capa", out)?;
                num::encode_wide(*um_s, "velocidad", out)
            }
            LaserTime { sub, micros } => {
                if !is_time_sub(*sub) {
                    return Err(bad("subcódigo de tiempo C6", sub));
                }
                out.extend([0xC6, *sub]);
                num::encode_wide(*micros, "tiempo", out)
            }
            LayerFrequency { laser, layer, hz } => {
                out.extend([0xC6, 0x60]);
                num::encode_int7(*laser, "láser", out)?;
                num::encode_int7(*layer, "capa", out)?;
                num::encode_wide(*hz, "frecuencia", out)
            }
            EndLayer => {
                out.extend([0xCA, 0x01, 0x00]);
                Ok(())
            }
            AirOff => {
                out.extend([0xCA, 0x01, 0x12]);
                Ok(())
            }
            AirOn => {
                out.extend([0xCA, 0x01, 0x13]);
                Ok(())
            }
            LayerFlag(f) => {
                out.extend([0xCA, 0x01]);
                push_data(*f, "bandera de capa", out)
            }
            LayerSelect(l) => {
                out.extend([0xCA, 0x02]);
                num::encode_int7(*l, "capa", out)
            }
            Color(c) => {
                out.extend([0xCA, 0x05]);
                num::encode_wide(*c, "color", out)
            }
            LayerColor { layer, color } => {
                out.extend([0xCA, 0x06]);
                num::encode_int7(*layer, "capa", out)?;
                num::encode_wide(*color, "color", out)
            }
            MaxLayer(l) => {
                out.extend([0xCA, 0x22]);
                num::encode_int7(*l, "capa", out)
            }
            LayerMode { layer, mode } => {
                out.extend([0xCA, 0x41]);
                num::encode_int7(*layer, "capa", out)?;
                num::encode_int7(*mode, "modo", out)
            }
            Process(p) => {
                out.extend([0xD8]);
                push_data(p.sub(), "subcódigo D8", out)
            }
            EndBlock => {
                out.extend([0xE7, 0x00]);
                Ok(())
            }
            EndOfFile => {
                out.push(0xD7);
                Ok(())
            }
            Absolute => {
                out.extend([0xE6, 0x01]);
                Ok(())
            }
            SetReference => {
                out.push(0xF0);
                Ok(())
            }
            FileChecksum(s) => {
                out.extend([0xE5, 0x05]);
                num::encode_wide(*s, "suma de archivo", out)
            }
            FileName(name) => {
                if name.bytes().any(|b| b == 0 || b > 0x7F) {
                    return Err(bad("nombre de archivo (solo ASCII sin NUL)", name));
                }
                out.extend([0xE7, 0x01]);
                out.extend(name.bytes());
                out.push(0);
                Ok(())
            }
            MemRead { addr } => {
                out.extend([0xDA, 0x00]);
                encode_addr(*addr, out)
            }
            MemWrite { addr, values } => {
                if values.is_empty() {
                    return Err(bad("valores de escritura de memoria", "vacío"));
                }
                out.extend([0xDA, 0x01]);
                encode_addr(*addr, out)?;
                for v in values {
                    num::encode_wide(*v, "valor de memoria", out)?;
                }
                Ok(())
            }
            Point2 { op, sub, x, y } => abs_pair(*op, Some(*sub), *x, *y, out),
            PartPoint { sub, part, x, y } => {
                out.extend([0xE7]);
                push_data(*sub, "subcódigo", out)?;
                num::encode_int7(*part, "parte", out)?;
                num::encode_abs(*x, out)?;
                num::encode_abs(*y, out)
            }
            PartAbs { sub, n, v } => {
                out.extend([0xE7]);
                push_data(*sub, "subcódigo", out)?;
                num::encode_int7(*n, "índice", out)?;
                num::encode_abs(*v, out)
            }
            Arr7 { op, sub, v } => {
                push_op(*op, out)?;
                push_data(*sub, "subcódigo", out)?;
                for &e in v {
                    num::encode_signed(i64::from(e), 2, "valor de matriz", out)?;
                }
                Ok(())
            }
            Small { op, sub, n } => {
                push_op(*op, out)?;
                push_data(*sub, "subcódigo", out)?;
                push_data(*n, "parámetro", out)
            }
            Marker(op) => push_op(*op, out),
            ShortArg { op, n } => {
                push_op(*op, out)?;
                push_data(*n, "parámetro", out)
            }
            ElementName(b) => {
                out.extend([0xF2, 0x02]);
                for &x in b {
                    push_data(x, "nombre de elemento", out)?;
                }
                Ok(())
            }
            Unknown { op, data } => {
                push_op(*op, out)?;
                for &b in data {
                    push_data(b, "dato", out)?;
                }
                Ok(())
            }
        }
    }
}

fn abs_pair(op: u8, sub: Option<u8>, x: i64, y: i64, out: &mut Vec<u8>) -> Result<(), NumError> {
    push_op(op, out)?;
    if let Some(s) = sub {
        push_data(s, "subcódigo", out)?;
    }
    num::encode_abs(x, out)?;
    num::encode_abs(y, out)
}

fn rel_pair(op: u8, dx: i32, dy: i32, out: &mut Vec<u8>) -> Result<(), NumError> {
    out.push(op);
    num::encode_rel(dx, out)?;
    num::encode_rel(dy, out)
}

fn rel_one(op: u8, d: i32, out: &mut Vec<u8>) -> Result<(), NumError> {
    out.push(op);
    num::encode_rel(d, out)
}

/// Dirección de memoria: dos bytes de 7 bits, el alto primero (0x2FE = `05 7E`).
fn encode_addr(addr: u16, out: &mut Vec<u8>) -> Result<(), NumError> {
    num::encode_unsigned(u64::from(addr), 2, "dirección de memoria", out)
}

// ----------------------------------------------------------------------------------------
// Decodificador

/// Divide el flujo en comandos: cada uno empieza en un byte con el bit 7 en 1 y llega hasta
/// el siguiente. No interpreta los comandos. Error si el flujo empieza con un byte de datos.
pub fn command_ranges(bytes: &[u8]) -> Result<Vec<Range<usize>>, DecodeError> {
    let mut ranges = Vec::new();
    let mut start = None;
    for (i, &b) in bytes.iter().enumerate() {
        if b >= 0x80 {
            if let Some(s) = start {
                ranges.push(s..i);
            }
            start = Some(i);
        } else if start.is_none() {
            return Err(DecodeError::NotAnOpcode { offset: i, byte: b });
        }
    }
    if let Some(s) = start {
        ranges.push(s..bytes.len());
    }
    Ok(ranges)
}

/// Decodifica el primer comando de `bytes`. Devuelve el comando y los bytes que ocupa.
pub fn decode_one(bytes: &[u8]) -> Result<(Command, usize), DecodeError> {
    decode_at(bytes, 0)
}

fn decode_at(bytes: &[u8], base: usize) -> Result<(Command, usize), DecodeError> {
    let Some(&op) = bytes.first() else {
        return Err(DecodeError::Invalid {
            offset: base,
            what: "flujo vacío",
        });
    };
    if op < 0x80 {
        return Err(DecodeError::NotAnOpcode {
            offset: base,
            byte: op,
        });
    }
    let n = bytes[1..].iter().take_while(|&&b| b < 0x80).count();
    let data = &bytes[1..1 + n];
    let cmd = decode_body(op, data, base)?;
    Ok((cmd, 1 + n))
}

/// Decodifica todo el flujo. Los opcodes que no se reconocen salen como [`Command::Unknown`].
pub fn decode_stream(bytes: &[u8]) -> Result<Vec<Command>, DecodeError> {
    let mut out = Vec::new();
    for r in command_ranges(bytes)? {
        let (cmd, used) = decode_at(&bytes[r.clone()], r.start)?;
        debug_assert_eq!(used, r.len());
        out.push(cmd);
    }
    Ok(out)
}

/// Codifica una lista de comandos.
pub fn encode_stream(cmds: &[Command]) -> Result<Vec<u8>, NumError> {
    let mut out = Vec::new();
    for c in cmds {
        c.encode(&mut out)?;
    }
    Ok(out)
}

/// Texto de depuración: un comando por línea con su desplazamiento en el flujo.
pub fn disassemble(bytes: &[u8]) -> Result<String, DecodeError> {
    let mut s = String::new();
    for r in command_ranges(bytes)? {
        let (cmd, _) = decode_at(&bytes[r.clone()], r.start)?;
        let hex: Vec<String> = bytes[r.clone()]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        s.push_str(&format!(
            "{:06}  {:<44} {}\n",
            r.start,
            cmd.to_string(),
            hex.join(" ")
        ));
    }
    Ok(s)
}

fn exact(d: &[u8], n: usize, op: u8, base: usize) -> Result<(), DecodeError> {
    if d.len() == n {
        Ok(())
    } else {
        Err(DecodeError::BadLength {
            offset: base,
            op,
            expected: n,
            found: d.len(),
        })
    }
}

fn dec<T>(r: Result<T, NumError>, base: usize) -> Result<T, DecodeError> {
    r.map_err(|source| DecodeError::Num {
        offset: base,
        source,
    })
}

fn abs_at(d: &[u8], at: usize, base: usize) -> Result<i64, DecodeError> {
    dec(num::decode_abs(&d[at..at + 5]), base)
}

fn decode_body(op: u8, d: &[u8], base: usize) -> Result<Command, DecodeError> {
    use Command::*;
    let sub = d.first().copied();
    let unknown = || Unknown {
        op,
        data: d.to_vec(),
    };
    Ok(match op {
        0x88 | 0xA8 => {
            exact(d, 10, op, base)?;
            let (x, y) = (abs_at(d, 0, base)?, abs_at(d, 5, base)?);
            if op == 0x88 {
                MoveAbs { x, y }
            } else {
                CutAbs { x, y }
            }
        }
        0x89 | 0xA9 => {
            exact(d, 4, op, base)?;
            let dx = dec(num::decode_rel(&d[0..2]), base)?;
            let dy = dec(num::decode_rel(&d[2..4]), base)?;
            if op == 0x89 {
                MoveRel { dx, dy }
            } else {
                CutRel { dx, dy }
            }
        }
        0x8A | 0x8B | 0xAA | 0xAB => {
            exact(d, 2, op, base)?;
            let v = dec(num::decode_rel(d), base)?;
            match op {
                0x8A => MoveRelX(v),
                0x8B => MoveRelY(v),
                0xAA => CutRelX(v),
                _ => CutRelY(v),
            }
        }
        0xC6 => match sub {
            Some(s) if power_from_sub(&LASER_POWER, s).is_some() => {
                exact(d, 3, op, base)?;
                let (laser, limit) = power_from_sub(&LASER_POWER, s).unwrap();
                LaserPower {
                    laser,
                    limit,
                    raw: dec(num::decode_power(&d[1..3]), base)?,
                }
            }
            Some(s) if power_from_sub(&LAYER_POWER, s).is_some() => {
                exact(d, 4, op, base)?;
                let (laser, limit) = power_from_sub(&LAYER_POWER, s).unwrap();
                LayerPower {
                    laser,
                    limit,
                    layer: dec(num::decode_int7(&d[1..2]), base)?,
                    raw: dec(num::decode_power(&d[2..4]), base)?,
                }
            }
            Some(s) if is_time_sub(s) => {
                exact(d, 6, op, base)?;
                LaserTime {
                    sub: s,
                    micros: dec(num::decode_wide(&d[1..6]), base)?,
                }
            }
            Some(0x60) => {
                exact(d, 8, op, base)?;
                LayerFrequency {
                    laser: dec(num::decode_int7(&d[1..2]), base)?,
                    layer: dec(num::decode_int7(&d[2..3]), base)?,
                    hz: dec(num::decode_wide(&d[3..8]), base)?,
                }
            }
            _ => unknown(),
        },
        0xC9 => match sub {
            Some(0x02) => {
                exact(d, 6, op, base)?;
                Speed(dec(num::decode_wide(&d[1..6]), base)?)
            }
            Some(0x04) => {
                exact(d, 7, op, base)?;
                LayerSpeed {
                    layer: dec(num::decode_int7(&d[1..2]), base)?,
                    um_s: dec(num::decode_wide(&d[2..7]), base)?,
                }
            }
            _ => unknown(),
        },
        0xCA => match sub {
            Some(0x01) => {
                exact(d, 2, op, base)?;
                match d[1] {
                    0x00 => EndLayer,
                    0x12 => AirOff,
                    0x13 => AirOn,
                    f => LayerFlag(f),
                }
            }
            Some(0x02) => {
                exact(d, 2, op, base)?;
                LayerSelect(d[1])
            }
            Some(0x05) => {
                exact(d, 6, op, base)?;
                Color(dec(num::decode_wide(&d[1..6]), base)?)
            }
            Some(0x06) => {
                exact(d, 7, op, base)?;
                LayerColor {
                    layer: d[1],
                    color: dec(num::decode_wide(&d[2..7]), base)?,
                }
            }
            Some(0x22) => {
                exact(d, 2, op, base)?;
                MaxLayer(d[1])
            }
            Some(0x41) => {
                exact(d, 3, op, base)?;
                LayerMode {
                    layer: d[1],
                    mode: d[2],
                }
            }
            Some(s @ (0x03 | 0x10)) => {
                exact(d, 2, op, base)?;
                Small {
                    op,
                    sub: s,
                    n: d[1],
                }
            }
            _ => unknown(),
        },
        0xD7 => {
            exact(d, 0, op, base)?;
            EndOfFile
        }
        0xD8 => {
            exact(d, 1, op, base)?;
            Process(ProcessOp::from_sub(d[0]))
        }
        0xDA => match sub {
            Some(0x00) => {
                exact(d, 3, op, base)?;
                MemRead {
                    addr: dec(num::decode_unsigned(&d[1..3], 2), base)? as u16,
                }
            }
            Some(0x01) => {
                // sub + dirección(2) + uno o más valores de 5 bytes.
                if d.len() < 8 || !(d.len() - 3).is_multiple_of(5) {
                    return Err(DecodeError::BadLength {
                        offset: base,
                        op,
                        expected: 8,
                        found: d.len(),
                    });
                }
                let addr = dec(num::decode_unsigned(&d[1..3], 2), base)? as u16;
                let values = d[3..]
                    .chunks(5)
                    .map(|c| dec(num::decode_wide(c), base))
                    .collect::<Result<Vec<_>, _>>()?;
                MemWrite { addr, values }
            }
            _ => unknown(),
        },
        0xE5 => match sub {
            Some(0x05) => {
                exact(d, 6, op, base)?;
                FileChecksum(dec(num::decode_wide(&d[1..6]), base)?)
            }
            _ => unknown(),
        },
        0xE6 => match sub {
            Some(0x01) => {
                exact(d, 1, op, base)?;
                Absolute
            }
            _ => unknown(),
        },
        0xE7 => match sub {
            Some(0x00) => {
                exact(d, 1, op, base)?;
                EndBlock
            }
            Some(0x01) => {
                if d.len() < 2 || d[d.len() - 1] != 0 || d[1..d.len() - 1].contains(&0) {
                    return Err(DecodeError::Invalid {
                        offset: base,
                        what: "el nombre de archivo (E7 01) debe terminar en un único cero",
                    });
                }
                FileName(String::from_utf8_lossy(&d[1..d.len() - 1]).into_owned())
            }
            Some(s @ (0x03 | 0x06 | 0x07 | 0x13 | 0x17 | 0x23 | 0x50 | 0x51)) => {
                exact(d, 11, op, base)?;
                Point2 {
                    op,
                    sub: s,
                    x: abs_at(d, 1, base)?,
                    y: abs_at(d, 6, base)?,
                }
            }
            Some(s @ (0x52 | 0x53 | 0x61 | 0x62)) => {
                exact(d, 12, op, base)?;
                PartPoint {
                    sub: s,
                    part: d[1],
                    x: abs_at(d, 2, base)?,
                    y: abs_at(d, 7, base)?,
                }
            }
            Some(s @ (0x54 | 0x55)) => {
                exact(d, 7, op, base)?;
                PartAbs {
                    sub: s,
                    n: d[1],
                    v: abs_at(d, 2, base)?,
                }
            }
            Some(s @ (0x04 | 0x08)) => arr7(op, s, d, base)?,
            Some(s @ (0x05 | 0x24 | 0x60)) => {
                exact(d, 2, op, base)?;
                Small {
                    op,
                    sub: s,
                    n: d[1],
                }
            }
            _ => unknown(),
        },
        0xEA => {
            exact(d, 1, op, base)?;
            ShortArg { op, n: d[0] }
        }
        0xEB => {
            exact(d, 0, op, base)?;
            Marker(op)
        }
        0xF0 => {
            exact(d, 0, op, base)?;
            SetReference
        }
        0xF1 => match sub {
            Some(s @ 0x00..=0x02) => {
                exact(d, 2, op, base)?;
                Small {
                    op,
                    sub: s,
                    n: d[1],
                }
            }
            Some(0x03) => {
                exact(d, 11, op, base)?;
                Point2 {
                    op,
                    sub: 0x03,
                    x: abs_at(d, 1, base)?,
                    y: abs_at(d, 6, base)?,
                }
            }
            _ => unknown(),
        },
        0xF2 => match sub {
            Some(s @ (0x00 | 0x01 | 0x07)) => {
                exact(d, 2, op, base)?;
                Small {
                    op,
                    sub: s,
                    n: d[1],
                }
            }
            Some(0x02) => {
                exact(d, 11, op, base)?;
                let mut b = [0u8; 10];
                b.copy_from_slice(&d[1..11]);
                ElementName(b)
            }
            Some(s @ (0x03 | 0x04 | 0x06)) => {
                exact(d, 11, op, base)?;
                Point2 {
                    op,
                    sub: s,
                    x: abs_at(d, 1, base)?,
                    y: abs_at(d, 6, base)?,
                }
            }
            Some(0x05) => arr7(op, 0x05, d, base)?,
            _ => unknown(),
        },
        _ => unknown(),
    })
}

fn arr7(op: u8, sub: u8, d: &[u8], base: usize) -> Result<Command, DecodeError> {
    exact(d, 15, op, base)?;
    let mut v = [0i32; 7];
    for (i, slot) in v.iter_mut().enumerate() {
        *slot = dec(num::decode_signed(&d[1 + 2 * i..3 + 2 * i], 2), base)? as i32;
    }
    Ok(Command::Arr7 { op, sub, v })
}

// ----------------------------------------------------------------------------------------
// Texto legible

fn mm(um: i64) -> String {
    format!("{:.3}mm", um as f64 / 1000.0)
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Command::*;
        match self {
            MoveAbs { x, y } => write!(f, "MOVE_ABS x={} y={}", mm(*x), mm(*y)),
            CutAbs { x, y } => write!(f, "CUT_ABS x={} y={}", mm(*x), mm(*y)),
            MoveRel { dx, dy } => write!(f, "MOVE_REL dx={} dy={}", mm(*dx as i64), mm(*dy as i64)),
            CutRel { dx, dy } => write!(f, "CUT_REL dx={} dy={}", mm(*dx as i64), mm(*dy as i64)),
            MoveRelX(d) => write!(f, "MOVE_REL_X dx={}", mm(*d as i64)),
            MoveRelY(d) => write!(f, "MOVE_REL_Y dy={}", mm(*d as i64)),
            CutRelX(d) => write!(f, "CUT_REL_X dx={}", mm(*d as i64)),
            CutRelY(d) => write!(f, "CUT_REL_Y dy={}", mm(*d as i64)),
            LaserPower { laser, limit, raw } => write!(
                f,
                "POWER laser={laser} {} {:.3}%",
                lim(*limit),
                num::power_to_pct(*raw)
            ),
            LayerPower {
                laser,
                limit,
                layer,
                raw,
            } => write!(
                f,
                "LAYER_POWER laser={laser} layer={layer} {} {:.3}%",
                lim(*limit),
                num::power_to_pct(*raw)
            ),
            Speed(v) => write!(f, "SPEED {:.3}mm/s", *v as f64 / 1000.0),
            LayerSpeed { layer, um_s } => {
                write!(
                    f,
                    "LAYER_SPEED layer={layer} {:.3}mm/s",
                    *um_s as f64 / 1000.0
                )
            }
            LaserTime { sub, micros } => write!(f, "LASER_TIME sub=0x{sub:02X} {micros}us"),
            LayerFrequency { laser, layer, hz } => {
                write!(f, "LAYER_FREQ laser={laser} layer={layer} {hz}Hz")
            }
            EndLayer => write!(f, "END_LAYER"),
            AirOff => write!(f, "AIR_OFF"),
            AirOn => write!(f, "AIR_ON"),
            LayerFlag(x) => write!(f, "LAYER_FLAG 0x{x:02X}"),
            LayerSelect(l) => write!(f, "LAYER {l}"),
            Color(c) => write!(f, "COLOR 0x{c:06X}"),
            LayerColor { layer, color } => write!(f, "LAYER_COLOR layer={layer} bgr=0x{color:06X}"),
            MaxLayer(l) => write!(f, "MAX_LAYER {l}"),
            LayerMode { layer, mode } => write!(f, "LAYER_MODE layer={layer} mode={mode}"),
            Process(p) => write!(f, "PROCESS {p:?}"),
            EndBlock => write!(f, "END_BLOCK"),
            EndOfFile => write!(f, "END_OF_FILE"),
            Absolute => write!(f, "ABSOLUTE"),
            SetReference => write!(f, "SET_REFERENCE"),
            FileChecksum(s) => write!(f, "FILE_CHECKSUM {s}"),
            FileName(n) => write!(f, "FILE_NAME {n:?}"),
            MemRead { addr } => write!(f, "MEM_READ 0x{addr:03X}"),
            MemWrite { addr, values } => write!(f, "MEM_WRITE 0x{addr:03X} {values:?}"),
            Point2 { op, sub, x, y } => {
                write!(f, "POINT2 {op:02X}.{sub:02X} x={} y={}", mm(*x), mm(*y))
            }
            PartPoint { sub, part, x, y } => write!(
                f,
                "PART_POINT E7.{sub:02X} part={part} x={} y={}",
                mm(*x),
                mm(*y)
            ),
            PartAbs { sub, n, v } => write!(f, "PART_ABS E7.{sub:02X} n={n} v={}", mm(*v)),
            Arr7 { op, sub, v } => write!(f, "ARRAY7 {op:02X}.{sub:02X} {v:?}"),
            Small { op, sub, n } => write!(f, "SMALL {op:02X}.{sub:02X} {n}"),
            Marker(op) => write!(f, "MARKER {op:02X}"),
            ShortArg { op, n } => write!(f, "SHORT {op:02X} {n}"),
            ElementName(b) => write!(f, "ELEMENT_NAME {b:02x?}"),
            Unknown { op, data } => write!(f, "UNKNOWN {op:02X} {data:02x?}"),
        }
    }
}

fn lim(l: Limit) -> &'static str {
    match l {
        Limit::Min => "min",
        Limit::Max => "max",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        let s: String = s.split_whitespace().collect();
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
            .collect()
    }

    fn one(s: &str) -> Command {
        let b = hex(s);
        let (c, used) = decode_one(&b).unwrap();
        assert_eq!(used, b.len(), "{s}");
        assert_eq!(c.to_bytes().unwrap(), b, "ida y vuelta de {s}: {c}");
        c
    }

    #[test]
    fn movement_and_cut() {
        assert_eq!(
            one("88 00000b5112 0000021258"),
            Command::MoveAbs {
                x: 190_610,
                y: 35_160
            }
        );
        assert_eq!(
            one("a8 00000b5112 0000021256"),
            Command::CutAbs {
                x: 190_610,
                y: 35_158
            }
        );
        assert_eq!(one("89 7f7f 0001"), Command::MoveRel { dx: -1, dy: 1 });
        assert_eq!(
            one("a9 3f7f 4000"),
            Command::CutRel {
                dx: 8191,
                dy: -8192
            }
        );
        assert_eq!(one("8a 7f7f"), Command::MoveRelX(-1));
        assert_eq!(one("8b 0001"), Command::MoveRelY(1));
        assert_eq!(one("aa 0001"), Command::CutRelX(1));
        assert_eq!(one("ab 7f7f"), Command::CutRelY(-1));
    }

    #[test]
    fn power_and_speed() {
        assert_eq!(
            one("c6 31 00 1319"),
            Command::LayerPower {
                laser: 1,
                limit: Limit::Min,
                layer: 0,
                raw: 2457
            }
        );
        assert_eq!(
            one("c6 42 03 2632"),
            Command::LayerPower {
                laser: 2,
                limit: Limit::Max,
                layer: 3,
                raw: 4914
            }
        );
        assert_eq!(
            one("c6 08 2632"),
            Command::LaserPower {
                laser: 4,
                limit: Limit::Max,
                raw: 4914
            }
        );
        assert_eq!(
            one("c9 04 00 000006 0d20"),
            Command::LayerSpeed {
                layer: 0,
                um_s: 100_000
            }
        );
        assert_eq!(one("c9 02 000006 0d20"), Command::Speed(100_000));
        assert_eq!(
            one("c6 15 0000000000"),
            Command::LaserTime {
                sub: 0x15,
                micros: 0
            }
        );
        assert_eq!(
            one("c6 60 01 02 0000000a14"),
            Command::LayerFrequency {
                laser: 1,
                layer: 2,
                hz: 1300
            }
        );
    }

    #[test]
    fn layers_and_air() {
        assert_eq!(one("ca 01 00"), Command::EndLayer);
        assert_eq!(one("ca 01 12"), Command::AirOff);
        assert_eq!(one("ca 01 13"), Command::AirOn);
        assert_eq!(one("ca 01 30"), Command::LayerFlag(0x30));
        assert_eq!(one("ca 02 05"), Command::LayerSelect(5));
        assert_eq!(
            one("ca 06 01 000000017f"),
            Command::LayerColor {
                layer: 1,
                color: 0xFF
            }
        );
        assert_eq!(one("ca 22 00"), Command::MaxLayer(0));
        assert_eq!(one("ca 41 00 00"), Command::LayerMode { layer: 0, mode: 0 });
        assert_eq!(
            one("ca 03 0d"),
            Command::Small {
                op: 0xCA,
                sub: 3,
                n: 0x0D
            }
        );
        assert_eq!(one("ca 05 0000000000"), Command::Color(0));
    }

    #[test]
    fn file_and_process() {
        assert_eq!(one("d7"), Command::EndOfFile);
        assert_eq!(one("e7 00"), Command::EndBlock);
        assert_eq!(one("d8 00"), Command::Process(ProcessOp::Start));
        assert_eq!(one("d8 02"), Command::Process(ProcessOp::Pause));
        assert_eq!(one("d8 03"), Command::Process(ProcessOp::Resume));
        assert_eq!(one("d8 01"), Command::Process(ProcessOp::Stop));
        assert_eq!(one("d8 10"), Command::Process(ProcessOp::RefPoint));
        assert_eq!(one("d8 55"), Command::Process(ProcessOp::Other(0x55)));
        assert_eq!(one("e6 01"), Command::Absolute);
        assert_eq!(one("f0"), Command::SetReference);
        assert_eq!(one("eb"), Command::Marker(0xEB));
        assert_eq!(one("e5 05 0000000123"), Command::FileChecksum(163));
        assert_eq!(one("e7 01 41 42 00"), Command::FileName("AB".into()));
    }

    #[test]
    fn memory_read_uses_two_seven_bit_bytes() {
        // 0x2FE = 5*128 + 0x7E -> `05 7E`.
        assert_eq!(one("da 00 05 7e"), Command::MemRead { addr: 0x2FE });
        assert_eq!(
            one("da 01 06 20 0000000247 0000000247"),
            Command::MemWrite {
                addr: 0x320,
                values: vec![327, 327]
            }
        );
    }

    #[test]
    fn structure_commands() {
        assert_eq!(
            one("e7 03 00000b5112 0000021258"),
            Command::Point2 {
                op: 0xE7,
                sub: 3,
                x: 190_610,
                y: 35_160
            }
        );
        assert_eq!(
            one("e7 52 00 00000b5112 0000021258"),
            Command::PartPoint {
                sub: 0x52,
                part: 0,
                x: 190_610,
                y: 35_160
            }
        );
        assert_eq!(
            one("e7 54 01 0000030650"),
            Command::PartAbs {
                sub: 0x54,
                n: 1,
                v: 50_000
            }
        );
        // Los siete valores que EduTech decodifica como 1,1,-1,-710,-7808,4,7597.
        assert_eq!(
            one("e7 08 0001 0001 7f7f 7a3a 4300 0004 3b2d"),
            Command::Arr7 {
                op: 0xE7,
                sub: 8,
                v: [1, 1, -1, -710, -7808, 4, 7597]
            }
        );
        assert_eq!(
            one("f1 03 0000030650 0000030650"),
            Command::Point2 {
                op: 0xF1,
                sub: 3,
                x: 50_000,
                y: 50_000
            }
        );
        assert!(matches!(
            one("f2 02 052a391c41046a150820"),
            Command::ElementName(_)
        ));
        assert_eq!(one("ea 00"), Command::ShortArg { op: 0xEA, n: 0 });
    }

    #[test]
    fn unknown_opcodes_are_preserved() {
        assert_eq!(
            one("e8 02"),
            Command::Unknown {
                op: 0xE8,
                data: vec![2]
            }
        );
        assert_eq!(
            one("c9 03 01 02"),
            Command::Unknown {
                op: 0xC9,
                data: vec![3, 1, 2]
            }
        );
    }

    #[test]
    fn bad_input_is_reported() {
        // Datos al inicio.
        assert!(matches!(
            decode_stream(&[0x00, 0x88]),
            Err(DecodeError::NotAnOpcode { offset: 0, .. })
        ));
        // Longitud incorrecta.
        assert!(matches!(
            decode_stream(&hex("88 00000b5112")),
            Err(DecodeError::BadLength { op: 0x88, .. })
        ));
        // Truncado en medio de un comando seguido de otro.
        assert!(matches!(
            decode_stream(&hex("a8 00 d7")),
            Err(DecodeError::BadLength { op: 0xA8, .. })
        ));
        assert!(matches!(
            decode_stream(&hex("e7 01 41 42")),
            Err(DecodeError::Invalid { .. })
        ));
    }

    #[test]
    fn encode_validates_values() {
        let mut v = Vec::new();
        assert!(Command::MoveRel { dx: 9000, dy: 0 }.encode(&mut v).is_err());
        assert!(v.is_empty(), "un error no deja bytes a medias");
        assert!(
            Command::LaserPower {
                laser: 5,
                limit: Limit::Max,
                raw: 0
            }
            .encode(&mut v)
            .is_err()
        );
        assert!(Command::LayerSelect(128).encode(&mut v).is_err());
        assert!(Command::FileName("ñ".into()).encode(&mut v).is_err());
        assert!(
            Command::MemWrite {
                addr: 1,
                values: vec![]
            }
            .encode(&mut v)
            .is_err()
        );
        assert!(v.is_empty());
    }

    #[test]
    fn stream_framing_and_disassembly() {
        let bytes = hex("d8 10 f0 e6 01 f1 02 00 d8 00");
        let cmds = decode_stream(&bytes).unwrap();
        assert_eq!(cmds.len(), 5);
        assert_eq!(encode_stream(&cmds).unwrap(), bytes);
        let text = disassemble(&bytes).unwrap();
        assert!(text.contains("PROCESS Start"));
        assert_eq!(text.lines().count(), 5);
    }
}
