//! Codificación de números del protocolo Ruida: grupos de 7 bits, el más significativo primero.
//!
//! Un byte con el bit 7 en 1 inicia un comando; los bytes de datos solo usan 7 bits
//! (`0x00..=0x7F`). Fuente: ruida.md sección 3 (EduTech Wiki, Schuermans, FabLab Nürnberg).
//!
//! Tipos:
//! - int7: 1 byte (índice de capa 0 a 127).
//! - Abscoord: 5 bytes (35 bits) con signo, en µm.
//! - Relcoord: 2 bytes (14 bits) con signo, en µm (+-8,191 mm por salto).
//! - Potencia: 2 bytes (14 bits sin signo), 100/2^14 % por unidad.
//! - Velocidad: 5 bytes, µm/s.
//! - Tiempo (µs), frecuencia (Hz): 5 bytes. Color: 5 bytes con 24 bits útiles BBGGRR.

use std::fmt;

/// Bytes de una coordenada absoluta.
pub const ABS_LEN: usize = 5;
/// Bytes de una coordenada relativa.
pub const REL_LEN: usize = 2;
/// Bytes de una potencia.
pub const POWER_LEN: usize = 2;
/// Bytes de un valor de 35 bits sin signo (velocidad, tiempo, frecuencia, color, sumas).
pub const WIDE_LEN: usize = 5;

/// Potencia máxima representable (14 bits).
pub const POWER_MAX_RAW: u16 = 0x3FFF;
/// Valor crudo del 100 %. La escala es `% * 16383 / 100` truncada: así lo escribe RDWorks en
/// archivos reales (ruida.md sección 4.2, comprobado con 72 pares porcentaje-valor; con 16384 o
/// con redondeo al más cercano no coincide) y MeerK40t usa el mismo 16383.
const POWER_STEPS: f64 = 16383.0;

pub const ABS_MAX: i64 = (1 << 34) - 1;
pub const ABS_MIN: i64 = -(1 << 34);
pub const REL_MAX: i32 = (1 << 13) - 1;
pub const REL_MIN: i32 = -(1 << 13);

#[derive(Debug, Clone, PartialEq)]
pub enum NumError {
    /// El valor no cabe en el tipo; nunca se trunca en silencio.
    OutOfRange { what: &'static str, value: String },
    /// Valor decimal que no es finito (NaN o infinito).
    NotFinite { what: &'static str },
    /// Faltan bytes para decodificar.
    Truncated { needed: usize, found: usize },
    /// Un byte de datos tiene el bit 7 en 1 (es el inicio de otro comando).
    BadDataByte { index: usize, byte: u8 },
}

impl fmt::Display for NumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NumError::OutOfRange { what, value } => {
                write!(f, "{what}: el valor {value} no cabe en el formato Ruida")
            }
            NumError::NotFinite { what } => write!(f, "{what}: el valor no es finito"),
            NumError::Truncated { needed, found } => {
                write!(
                    f,
                    "datos truncados: se necesitan {needed} bytes y hay {found}"
                )
            }
            NumError::BadDataByte { index, byte } => write!(
                f,
                "el byte {index} (0x{byte:02X}) tiene el bit 7 en 1 y no puede ser un dato"
            ),
        }
    }
}

impl std::error::Error for NumError {}

/// Escribe `value` en `n` grupos de 7 bits sin signo.
pub fn encode_unsigned(
    value: u64,
    n: usize,
    what: &'static str,
    out: &mut Vec<u8>,
) -> Result<(), NumError> {
    let bits = 7 * n as u32;
    if bits < 64 && value >> bits != 0 {
        return Err(NumError::OutOfRange {
            what,
            value: value.to_string(),
        });
    }
    for i in (0..n).rev() {
        out.push(((value >> (7 * i as u32)) & 0x7F) as u8);
    }
    Ok(())
}

/// Lee `n` grupos de 7 bits sin signo.
pub fn decode_unsigned(bytes: &[u8], n: usize) -> Result<u64, NumError> {
    if bytes.len() < n {
        return Err(NumError::Truncated {
            needed: n,
            found: bytes.len(),
        });
    }
    let mut v = 0u64;
    for (i, &b) in bytes[..n].iter().enumerate() {
        if b > 0x7F {
            return Err(NumError::BadDataByte { index: i, byte: b });
        }
        v = (v << 7) | u64::from(b);
    }
    Ok(v)
}

/// Escribe `value` en complemento a dos de `7 * n` bits.
pub fn encode_signed(
    value: i64,
    n: usize,
    what: &'static str,
    out: &mut Vec<u8>,
) -> Result<(), NumError> {
    let bits = 7 * n as u32;
    let min = -(1i64 << (bits - 1));
    let max = (1i64 << (bits - 1)) - 1;
    if value < min || value > max {
        return Err(NumError::OutOfRange {
            what,
            value: value.to_string(),
        });
    }
    let mask = (1u64 << bits) - 1;
    encode_unsigned((value as u64) & mask, n, what, out)
}

/// Lee un complemento a dos de `7 * n` bits y extiende el signo.
pub fn decode_signed(bytes: &[u8], n: usize) -> Result<i64, NumError> {
    let raw = decode_unsigned(bytes, n)?;
    let bits = 7 * n as u32;
    let sign = 1u64 << (bits - 1);
    Ok(if raw & sign != 0 {
        (raw as i64) - (1i64 << bits)
    } else {
        raw as i64
    })
}

pub fn encode_int7(v: u8, what: &'static str, out: &mut Vec<u8>) -> Result<(), NumError> {
    encode_unsigned(u64::from(v), 1, what, out)
}

pub fn decode_int7(bytes: &[u8]) -> Result<u8, NumError> {
    Ok(decode_unsigned(bytes, 1)? as u8)
}

/// Coordenada absoluta de 5 bytes, µm.
pub fn encode_abs(um: i64, out: &mut Vec<u8>) -> Result<(), NumError> {
    encode_signed(um, ABS_LEN, "coordenada absoluta", out)
}

pub fn decode_abs(bytes: &[u8]) -> Result<i64, NumError> {
    decode_signed(bytes, ABS_LEN)
}

/// Coordenada relativa de 2 bytes, µm.
pub fn encode_rel(um: i32, out: &mut Vec<u8>) -> Result<(), NumError> {
    encode_signed(i64::from(um), REL_LEN, "coordenada relativa", out)
}

pub fn decode_rel(bytes: &[u8]) -> Result<i32, NumError> {
    Ok(decode_signed(bytes, REL_LEN)? as i32)
}

/// Potencia cruda de 14 bits.
pub fn encode_power(raw: u16, out: &mut Vec<u8>) -> Result<(), NumError> {
    encode_unsigned(u64::from(raw), POWER_LEN, "potencia", out)
}

pub fn decode_power(bytes: &[u8]) -> Result<u16, NumError> {
    Ok(decode_unsigned(bytes, POWER_LEN)? as u16)
}

/// Valor de 35 bits sin signo (velocidad µm/s, tiempo, frecuencia, color, sumas).
pub fn encode_wide(v: u64, what: &'static str, out: &mut Vec<u8>) -> Result<(), NumError> {
    encode_unsigned(v, WIDE_LEN, what, out)
}

pub fn decode_wide(bytes: &[u8]) -> Result<u64, NumError> {
    decode_unsigned(bytes, WIDE_LEN)
}

/// Color BBGGRR (azul en el byte alto), 24 bits en 5 bytes.
pub fn rgb_to_color(r: u8, g: u8, b: u8) -> u64 {
    (u64::from(b) << 16) | (u64::from(g) << 8) | u64::from(r)
}

/// Inversa de [`rgb_to_color`]; ignora los bits sobre el 24.
pub fn color_to_rgb(color: u64) -> (u8, u8, u8) {
    (
        (color & 0xFF) as u8,
        ((color >> 8) & 0xFF) as u8,
        ((color >> 16) & 0xFF) as u8,
    )
}

/// Porcentaje (0 a 100) a potencia cruda: `trunc(p * 16383 / 100)`; el 100 % da 0x3FFF. Fuera de
/// 0..=100 es un error. Truncar (no redondear) coincide con los archivos de RDWorks y nunca
/// supera el porcentaje pedido.
pub fn power_from_pct(pct: f64) -> Result<u16, NumError> {
    if !pct.is_finite() {
        return Err(NumError::NotFinite { what: "potencia" });
    }
    if !(0.0..=100.0).contains(&pct) {
        return Err(NumError::OutOfRange {
            what: "potencia (%)",
            value: pct.to_string(),
        });
    }
    // 1e-9: evita que un resultado exacto (p. ej. 100 %) caiga por error de coma flotante.
    let raw = (pct * POWER_STEPS / 100.0 + 1e-9).floor();
    Ok((raw as u32).min(u32::from(POWER_MAX_RAW)) as u16)
}

/// Potencia cruda a porcentaje.
pub fn power_to_pct(raw: u16) -> f64 {
    f64::from(raw) * 100.0 / POWER_STEPS
}

/// Milímetros a µm enteros (redondeo al más cercano). Error si no es finito o no cabe en una
/// coordenada absoluta.
pub fn mm_to_um(mm: f64) -> Result<i64, NumError> {
    if !mm.is_finite() {
        return Err(NumError::NotFinite { what: "coordenada" });
    }
    let um = (mm * 1000.0).round();
    if um < ABS_MIN as f64 || um > ABS_MAX as f64 {
        return Err(NumError::OutOfRange {
            what: "coordenada (mm)",
            value: mm.to_string(),
        });
    }
    Ok(um as i64)
}

/// mm/min a µm/s: `v * 1000 / 60`, redondeado. Debe ser finito y positivo.
pub fn speed_from_mm_min(mm_min: f64) -> Result<u64, NumError> {
    if !mm_min.is_finite() {
        return Err(NumError::NotFinite { what: "velocidad" });
    }
    let um_s = (mm_min * 1000.0 / 60.0).round();
    if um_s < 1.0 || um_s >= (1u64 << 35) as f64 {
        return Err(NumError::OutOfRange {
            what: "velocidad (mm/min)",
            value: mm_min.to_string(),
        });
    }
    Ok(um_s as u64)
}

/// µm/s a mm/min.
pub fn speed_to_mm_min(um_s: u64) -> f64 {
    um_s as f64 * 60.0 / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc<F: FnOnce(&mut Vec<u8>) -> Result<(), NumError>>(f: F) -> Vec<u8> {
        let mut v = Vec::new();
        f(&mut v).unwrap();
        v
    }

    #[test]
    fn vector_v3_speed_100_mm_s() {
        // 100 mm/s = 100 000 µm/s -> 00 00 06 0D 20 (ejemplo 1 de EduTech: c904 00 000006 0d20).
        let b = enc(|o| encode_wide(100_000, "v", o));
        assert_eq!(b, [0x00, 0x00, 0x06, 0x0D, 0x20]);
        assert_eq!(decode_wide(&b).unwrap(), 100_000);
        assert_eq!(speed_from_mm_min(6000.0).unwrap(), 100_000);
    }

    #[test]
    fn vector_v4_power() {
        // 2457 -> 13 19 (c631 00 1319). Es el 15 % de la escala 16383 (EduTech lo leía como
        // 14,996 % con 16384).
        let b = enc(|o| encode_power(2457, o));
        assert_eq!(b, [0x13, 0x19]);
        assert_eq!(decode_power(&b).unwrap(), 2457);
        assert_eq!(power_from_pct(15.0).unwrap(), 2457);
        assert!((power_to_pct(2457) - 15.0).abs() < 5e-3);
    }

    #[test]
    fn vector_v5_abs() {
        // 190 610 µm -> 00 00 0B 51 12
        let b = enc(|o| encode_abs(190_610, o));
        assert_eq!(b, [0x00, 0x00, 0x0B, 0x51, 0x12]);
        assert_eq!(decode_abs(&b).unwrap(), 190_610);
    }

    #[test]
    fn vector_v6_minus_one() {
        let b = enc(|o| encode_abs(-1, o));
        assert_eq!(b, [0x7F; 5]);
        assert_eq!(decode_abs(&b).unwrap(), -1);
    }

    #[test]
    fn signed_limits_from_schuermans() {
        // +1 = 0x1, mayor positivo 0x3FFFFFFFF, mayor negativo 0x400000000 (35 bits).
        assert_eq!(enc(|o| encode_abs(1, o)), [0, 0, 0, 0, 1]);
        assert_eq!(
            enc(|o| encode_abs(ABS_MAX, o)),
            [0x3F, 0x7F, 0x7F, 0x7F, 0x7F]
        );
        assert_eq!(enc(|o| encode_abs(ABS_MIN, o)), [0x40, 0, 0, 0, 0]);
    }

    #[test]
    fn relative_limits() {
        // -1 = 0x3FFF, mayor positivo 0x1FFF, mayor negativo 0x2000.
        assert_eq!(enc(|o| encode_rel(-1, o)), [0x7F, 0x7F]);
        assert_eq!(enc(|o| encode_rel(REL_MAX, o)), [0x3F, 0x7F]);
        assert_eq!(enc(|o| encode_rel(REL_MIN, o)), [0x40, 0x00]);
        assert!(encode_rel(REL_MAX + 1, &mut Vec::new()).is_err());
        assert!(encode_rel(REL_MIN - 1, &mut Vec::new()).is_err());
    }

    #[test]
    fn abs_round_trip_edges() {
        for v in [0, 1, -1, 127, 128, -128, 190_610, ABS_MAX, ABS_MIN, -35_160] {
            let b = enc(|o| encode_abs(v, o));
            assert_eq!(b.len(), 5);
            assert!(b.iter().all(|&x| x < 0x80));
            assert_eq!(decode_abs(&b).unwrap(), v);
        }
        assert!(encode_abs(ABS_MAX + 1, &mut Vec::new()).is_err());
        assert!(encode_abs(ABS_MIN - 1, &mut Vec::new()).is_err());
    }

    #[test]
    fn rel_round_trip_all() {
        for v in REL_MIN..=REL_MAX {
            let b = enc(|o| encode_rel(v, o));
            assert_eq!(decode_rel(&b).unwrap(), v);
        }
    }

    #[test]
    fn decode_rejects_high_bit_and_short() {
        assert_eq!(
            decode_abs(&[0, 0, 0x80, 0, 0]),
            Err(NumError::BadDataByte {
                index: 2,
                byte: 0x80
            })
        );
        assert!(matches!(
            decode_abs(&[0, 0]),
            Err(NumError::Truncated {
                needed: 5,
                found: 2
            })
        ));
    }

    #[test]
    fn power_pct_edges() {
        assert_eq!(power_from_pct(0.0).unwrap(), 0);
        assert_eq!(power_from_pct(100.0).unwrap(), POWER_MAX_RAW);
        assert_eq!(power_from_pct(50.0).unwrap(), 8191);
        // Pares porcentaje -> bytes del cable que escribe RDWorks (solo como referencia,
        // ruida.md 4.2): 1 % = 01 23, 2 % = 02 47, 69 % = 58 28, 99 % = 7E 5B.
        for (pct, bytes) in [
            (1.0, [0x01, 0x23]),
            (2.0, [0x02, 0x47]),
            (69.0, [0x58, 0x28]),
            (99.0, [0x7E, 0x5B]),
        ] {
            let raw = power_from_pct(pct).unwrap();
            assert_eq!(enc(|o| encode_power(raw, o)), bytes, "{pct} %");
        }
        assert!(power_from_pct(100.0001).is_err());
        assert!(power_from_pct(-0.1).is_err());
        assert!(power_from_pct(f64::NAN).is_err());
        assert!(power_from_pct(f64::INFINITY).is_err());
    }

    #[test]
    fn mm_and_speed_conversions() {
        assert_eq!(mm_to_um(190.61).unwrap(), 190_610);
        assert_eq!(mm_to_um(-0.0004).unwrap(), 0);
        assert!(mm_to_um(f64::NAN).is_err());
        assert!(mm_to_um(1e9).is_err());
        assert!(speed_from_mm_min(0.0).is_err());
        assert!(speed_from_mm_min(-5.0).is_err());
        assert!(speed_from_mm_min(f64::INFINITY).is_err());
        assert!((speed_to_mm_min(100_000) - 6000.0).abs() < 1e-9);
    }

    #[test]
    fn color_is_bgr() {
        // Rojo = 0x0000FF -> 00 00 00 01 7F (ejemplo 2 de EduTech: ca06 01 000000017f).
        let c = rgb_to_color(255, 0, 0);
        assert_eq!(c, 0xFF);
        assert_eq!(enc(|o| encode_wide(c, "c", o)), [0, 0, 0, 1, 0x7F]);
        assert_eq!(color_to_rgb(rgb_to_color(1, 2, 3)), (1, 2, 3));
    }

    #[test]
    fn unsigned_out_of_range_is_error() {
        assert!(encode_wide(1 << 35, "x", &mut Vec::new()).is_err());
        assert!(encode_int7(128, "x", &mut Vec::new()).is_err());
    }
}
