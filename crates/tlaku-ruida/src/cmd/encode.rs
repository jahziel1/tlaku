//! Codificación de comandos a bytes en claro.

use super::{Command, Limit};
use crate::num::{self, NumError};

// ----------------------------------------------------------------------------------------
// Tablas de subcódigos de potencia

/// (láser, sub mínima, sub máxima) de `C6` sin capa.
pub(super) const LASER_POWER: [(u8, u8, u8); 4] = [
    (1, 0x01, 0x02),
    (2, 0x21, 0x22),
    (3, 0x05, 0x06),
    (4, 0x07, 0x08),
];
/// (láser, sub mínima, sub máxima) de `C6` con capa.
pub(super) const LAYER_POWER: [(u8, u8, u8); 4] = [
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

pub(super) fn power_from_sub(table: &[(u8, u8, u8); 4], sub: u8) -> Option<(u8, Limit)> {
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

pub(super) fn is_time_sub(sub: u8) -> bool {
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
            ImmPower(raw) => {
                out.push(0xC7);
                num::encode_power(*raw, out)
            }
            EndPower(raw) => {
                out.push(0xC8);
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
            Copies {
                op,
                sub,
                cols,
                rows,
                step_x,
                step_y,
            } => {
                push_op(*op, out)?;
                push_data(*sub, "subcódigo", out)?;
                num::encode_unsigned(u64::from(*cols), 2, "columnas", out)?;
                num::encode_unsigned(u64::from(*rows), 2, "filas", out)?;
                num::encode_abs(*step_x, out)?;
                num::encode_abs(*step_y, out)
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
            RapidX { rel, v } => {
                out.extend([0xD9, 0x00, rel.byte()]);
                num::encode_abs(*v, out)
            }
            RapidY { rel, v } => {
                out.extend([0xD9, 0x01, rel.byte()]);
                num::encode_abs(*v, out)
            }
            RapidXY { rel, x, y } => {
                out.extend([0xD9, 0x10, rel.byte()]);
                num::encode_abs(*x, out)?;
                num::encode_abs(*y, out)
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
