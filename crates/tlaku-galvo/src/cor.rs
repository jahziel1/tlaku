//! Lectura del archivo de corrección `.cor` de EZCAD (CorFile2).
//!
//! **Solo se lee; no se aplica ni se escribe a la tarjeta.** La *estructura* del archivo está
//! documentada en código MIT (galvoplotter y MeerK40t `balormk`, funciones
//! `_read_correction_file`, `_read_float_correction_file`, `_read_int_correction_file` y
//! `get_scale_from_correction_file`), pero no la *semántica*: no se sabe en qué unidades están
//! los desplazamientos, cómo se orienta la rejilla de 65 x 65 respecto del campo ni cómo los
//! combina EZCAD con la tabla de 63 x 63 de la tarjeta. Por eso `compile` ignora `cor_file` (con
//! aviso) hasta que haya una captura o hardware que lo confirmen. Confianza: estructura media
//! (dos variantes vistas en foros), semántica ninguna.
//!
//! Disposición:
//! - Los primeros `0x16` bytes son una etiqueta UTF-16 LE.
//! - `LMC1COR_1.0`: 2 bytes sin significado, 63 `f64` (el 44.º, índice 43, es la escala) y
//!   65 x 65 pares `f64` (dx, dy) en orden de fila.
//! - Cualquier otra etiqueta (formato antiguo): 6 bytes sin significado, un `f64` de escala y
//!   65 x 65 pares `i32` little-endian.

use std::fmt;

pub const GRID: usize = 65;
const LABEL_BYTES: usize = 0x16;
const LABEL_FLOAT: &str = "LMC1COR_1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorFormat {
    /// `LMC1COR_1.0`, con valores `f64`.
    Float,
    /// Formato antiguo, con valores `i32`.
    Int,
}

/// Contenido de un `.cor` tal como está en el archivo, sin interpretar.
#[derive(Debug, Clone, PartialEq)]
pub struct CorTable {
    pub format: CorFormat,
    pub scale: f64,
    /// 65 x 65 desplazamientos en orden de fila, tal cual vienen en el archivo.
    pub dx: Vec<f64>,
    pub dy: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorError {
    /// El archivo es más corto que el formato.
    Truncated { needed: usize, got: usize },
    /// Un valor no es un número finito.
    NotFinite,
}

impl fmt::Display for CorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CorError::Truncated { needed, got } => {
                write!(
                    f,
                    "archivo .cor truncado: hacen falta {needed} bytes y hay {got}"
                )
            }
            CorError::NotFinite => write!(f, "el archivo .cor trae un valor que no es un número"),
        }
    }
}

impl std::error::Error for CorError {}

fn label_is_float(bytes: &[u8]) -> bool {
    let units: Vec<u16> = bytes[..LABEL_BYTES]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16(&units).is_ok_and(|s| s == LABEL_FLOAT)
}

fn f64_at(b: &[u8], at: usize) -> f64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[at..at + 8]);
    f64::from_le_bytes(a)
}

/// Lee un `.cor`. No entra en pánico con bytes arbitrarios.
pub fn parse_cor(bytes: &[u8]) -> Result<CorTable, CorError> {
    if bytes.len() < LABEL_BYTES {
        return Err(CorError::Truncated {
            needed: LABEL_BYTES,
            got: bytes.len(),
        });
    }
    let n = GRID * GRID;
    let (format, scale, table_at, needed) = if label_is_float(bytes) {
        let table_at = LABEL_BYTES + 0x1FA;
        let needed = table_at + n * 16;
        if bytes.len() < needed {
            return Err(CorError::Truncated {
                needed,
                got: bytes.len(),
            });
        }
        (
            CorFormat::Float,
            f64_at(bytes, LABEL_BYTES + 2 + 43 * 8),
            table_at,
            needed,
        )
    } else {
        let table_at = LABEL_BYTES + 0xE;
        let needed = table_at + n * 8;
        if bytes.len() < needed {
            return Err(CorError::Truncated {
                needed,
                got: bytes.len(),
            });
        }
        (
            CorFormat::Int,
            f64_at(bytes, LABEL_BYTES + 6),
            table_at,
            needed,
        )
    };
    let _ = needed;
    let mut dx = Vec::with_capacity(n);
    let mut dy = Vec::with_capacity(n);
    for i in 0..n {
        let (x, y) = match format {
            CorFormat::Float => (
                f64_at(bytes, table_at + i * 16),
                f64_at(bytes, table_at + i * 16 + 8),
            ),
            CorFormat::Int => {
                let at = table_at + i * 8;
                let w = |o: usize| {
                    i32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]])
                };
                (f64::from(w(at)), f64::from(w(at + 4)))
            }
        };
        dx.push(x);
        dy.push(y);
    }
    if !(scale.is_finite() && dx.iter().chain(&dy).all(|v| v.is_finite())) {
        return Err(CorError::NotFinite);
    }
    Ok(CorTable {
        format,
        scale,
        dx,
        dy,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn float_file(scale: f64, dx: impl Fn(usize) -> f64) -> Vec<u8> {
        let mut b: Vec<u8> = LABEL_FLOAT
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(b.len(), LABEL_BYTES);
        b.extend([0, 0]);
        for i in 0..63 {
            let v = if i == 43 { scale } else { 0.0 };
            b.extend(v.to_le_bytes());
        }
        for i in 0..GRID * GRID {
            b.extend(dx(i).to_le_bytes());
            b.extend((-(i as f64)).to_le_bytes());
        }
        b
    }

    fn int_file(scale: f64) -> Vec<u8> {
        let mut b = vec![b'X'; LABEL_BYTES];
        b.extend([0; 6]);
        b.extend(scale.to_le_bytes());
        for i in 0..GRID * GRID {
            b.extend((i as i32).to_le_bytes());
            b.extend((-(i as i32)).to_le_bytes());
        }
        b
    }

    #[test]
    fn float_variant_is_read() {
        let t = parse_cor(&float_file(1.25, |i| i as f64 * 0.5)).unwrap();
        assert_eq!(t.format, CorFormat::Float);
        assert_eq!(t.scale, 1.25);
        assert_eq!(t.dx.len(), 4225);
        assert_eq!(t.dx[10], 5.0);
        assert_eq!(t.dy[10], -10.0);
    }

    #[test]
    fn int_variant_is_read() {
        let t = parse_cor(&int_file(0.9)).unwrap();
        assert_eq!(t.format, CorFormat::Int);
        assert_eq!(t.scale, 0.9);
        assert_eq!((t.dx[4224], t.dy[4224]), (4224.0, -4224.0));
    }

    #[test]
    fn short_or_garbage_files_fail_without_panicking() {
        assert!(matches!(parse_cor(&[]), Err(CorError::Truncated { .. })));
        assert!(matches!(
            parse_cor(&int_file(1.0)[..1000]),
            Err(CorError::Truncated { .. })
        ));
        let mut f = float_file(1.0, |_| 0.0);
        f.truncate(f.len() - 1);
        assert!(matches!(parse_cor(&f), Err(CorError::Truncated { .. })));
        assert_eq!(
            parse_cor(&float_file(f64::NAN, |_| 0.0)),
            Err(CorError::NotFinite)
        );
        // Bytes pseudoaleatorios de cualquier largo.
        let mut s = 99u64;
        for len in [0usize, 5, 22, 100, 33836, 40000] {
            let b: Vec<u8> = (0..len)
                .map(|_| {
                    s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
                    (s >> 40) as u8
                })
                .collect();
            let _ = parse_cor(&b);
        }
    }
}
