//! Decodificador: bytes en claro a comandos.

use super::encode::{LASER_POWER, LAYER_POWER, is_time_sub, power_from_sub};
use super::{Command, DecodeError, ProcessOp, RapidRef};
use crate::num::{self, NumError};
use std::ops::Range;

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
        0xC7 | 0xC8 => {
            exact(d, 2, op, base)?;
            let raw = dec(num::decode_power(d), base)?;
            if op == 0xC7 {
                ImmPower(raw)
            } else {
                EndPower(raw)
            }
        }
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
        0xD9 => match (sub, d.get(1).copied().and_then(RapidRef::from_byte)) {
            (Some(0x00 | 0x01), Some(rel)) => {
                exact(d, 7, op, base)?;
                let v = abs_at(d, 2, base)?;
                if d[0] == 0x00 {
                    RapidX { rel, v }
                } else {
                    RapidY { rel, v }
                }
            }
            (Some(0x10), Some(rel)) => {
                exact(d, 12, op, base)?;
                RapidXY {
                    rel,
                    x: abs_at(d, 2, base)?,
                    y: abs_at(d, 7, base)?,
                }
            }
            // Opciones con el bit del haz de luz, ejes Z/U u otros subcódigos: sin interpretar.
            _ => unknown(),
        },
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
            Some(s @ (0x04 | 0x08)) => copies(op, s, d, base)?,
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
            Some(0x05) => copies(op, 0x05, d, base)?,
            _ => unknown(),
        },
        _ => unknown(),
    })
}

fn copies(op: u8, sub: u8, d: &[u8], base: usize) -> Result<Command, DecodeError> {
    exact(d, 15, op, base)?;
    Ok(Command::Copies {
        op,
        sub,
        cols: dec(num::decode_unsigned(&d[1..3], 2), base)? as u16,
        rows: dec(num::decode_unsigned(&d[3..5], 2), base)? as u16,
        step_x: abs_at(d, 5, base)?,
        step_y: abs_at(d, 10, base)?,
    })
}
