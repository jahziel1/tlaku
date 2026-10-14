//! Lectura y aplicación del archivo de corrección `.cor` de EZCAD (CorFile2).
//!
//! **Se lee y se aplica en el anfitrión, antes de cuantizar** (ver [`CorGrid`] y
//! `calib::FieldMap`); **nunca se escribe a la tarjeta**: `WriteCorTable` y `WriteCorLine` son
//! `NeverSent` en `codes.rs` y no hay forma de construirlos desde `cmd`.
//!
//! # Fuente
//!
//! La *estructura* del archivo viene de código MIT (galvoplotter y MeerK40t `balormk`, funciones
//! `_read_correction_file`, `_read_float_correction_file`, `_read_int_correction_file` y
//! `get_scale_from_correction_file`; ver THIRD_PARTY.md). Esos programas no interpretan la tabla:
//! la redondean a enteros, pasan los negativos a signo y magnitud (`-v + 0x8000`) y la mandan tal
//! cual a la tarjeta, que la aplica ella. De ahí sale la lectura que usa Tlaku:
//!
//! - Son 65 x 65 pares (dx, dy) en **unidades de la tarjeta** (1/65536 del campo por eje), un par
//!   por nodo, en orden de fila (`for j in 0..65 { for k in 0..65 }`).
//! - Los 65 nodos de un eje cubren las 65536 posiciones: el nodo `i` está en `i x 1024` unidades.
//! - El desplazamiento de un nodo se **suma** a la posición ordenada y entre nodos se interpola
//!   en forma bilineal.
//!
//! # Lo que es inferencia (sin verificar con una tarjeta real)
//!
//! Qué índice de la rejilla es X y cuál Y (se asume que `k`, el que corre más rápido, es X), el
//! signo del desplazamiento y que los nodos estén repartidos en 0..65536 (EZCAD podría usar otro
//! rango). Por eso el perfil trae `cor_transpose` y `cor_invert`, y por eso se **rechaza** un
//! archivo que desplace más de [`MAX_DELTA_UNITS`] unidades: una lectura equivocada no debe
//! sacar el haz de donde el diseño lo pide. El escalar `scale` del archivo (posición 43 de los 63
//! `f64` de la cabecera) no se usa: ningún código abierto dice qué significa. La cabecera de
//! 63 valores tampoco se interpreta. Se valida con el encuadre del puntero rojo y la rejilla de
//! calibración (`docs/validacion-hardware.md`).
//!
//! Disposición del archivo:
//! - Los primeros `0x16` bytes son una etiqueta UTF-16 LE.
//! - `LMC1COR_1.0`: 2 bytes sin significado, 63 `f64` (el 44.º, índice 43, es la escala) y
//!   65 x 65 pares `f64` (dx, dy) en orden de fila.
//! - Cualquier otra etiqueta (formato antiguo): 6 bytes sin significado, un `f64` de escala y
//!   65 x 65 pares `i32` little-endian.

use std::fmt;
use tlaku_i18n::tr;

pub const GRID: usize = 65;
/// Unidades entre dos nodos contiguos: 65 nodos cubren las 65536 posiciones de un eje.
pub const NODE_STEP_UNITS: f64 = 1024.0;
/// Mayor desplazamiento (unidades de la tarjeta) que se acepta en un nodo: 1/32 del campo, unos
/// 3 mm en una lente de 100 mm. Una lente real se corrige con mucho menos; si el archivo pide más,
/// lo más probable es que las unidades o la orientación no sean las que se supone.
pub const MAX_DELTA_UNITS: f64 = 2048.0;
/// Tamaño máximo del archivo que se lee (un `.cor` mide unos 34 KB).
pub const MAX_FILE_BYTES: u64 = 1 << 20;
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

#[derive(Debug, Clone, PartialEq)]
pub enum CorError {
    /// El archivo es más corto que el formato.
    Truncated { needed: usize, got: usize },
    /// Un valor no es un número finito.
    NotFinite,
    /// La tabla desplaza más de [`MAX_DELTA_UNITS`] unidades: no se aplica (ver la cabecera).
    TooLarge { seen: f64 },
    /// No se pudo leer el archivo.
    Io(String),
    /// El archivo es mucho más grande que un `.cor` (se lee entero en memoria).
    FileTooBig { bytes: u64 },
}

impl fmt::Display for CorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CorError::Truncated { needed, got } => {
                write!(
                    f,
                    "{}",
                    tr!("galvo.cor.truncated", needed = needed, got = got)
                )
            }
            CorError::NotFinite => write!(f, "{}", tr!("galvo.cor.not_finite")),
            CorError::TooLarge { seen } => write!(
                f,
                "{}",
                tr!(
                    "galvo.cor.too_large",
                    seen = format!("{seen:.0}"),
                    limit = MAX_DELTA_UNITS
                )
            ),
            CorError::Io(e) => write!(f, "{}", tr!("galvo.cor.io", error = e)),
            CorError::FileTooBig { bytes } => write!(
                f,
                "{}",
                tr!(
                    "galvo.cor.file_too_big",
                    bytes = bytes,
                    max = MAX_FILE_BYTES
                )
            ),
        }
    }
}

impl std::error::Error for CorError {}

fn label_is_float(bytes: &[u8]) -> bool {
    let units: Vec<u16> = bytes[..LABEL_BYTES]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
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

/// Un `.cor` en formato `LMC1COR_1.0` con el mismo desplazamiento en todos los nodos (solo
/// pruebas de este crate).
#[cfg(test)]
pub(crate) fn test_file(dx: f64, dy: f64) -> Vec<u8> {
    let mut b: Vec<u8> = LABEL_FLOAT
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    b.extend([0, 0]);
    b.extend(std::iter::repeat_n(0u8, 63 * 8));
    for _ in 0..GRID * GRID {
        b.extend(dx.to_le_bytes());
        b.extend(dy.to_le_bytes());
    }
    b
}

/// La tabla de un `.cor` lista para aplicar: desplazamientos en unidades de la tarjeta, ya con la
/// orientación y el signo del perfil. Se indexa por `[iy * 65 + ix]`.
#[derive(Debug, Clone, PartialEq)]
pub struct CorGrid {
    dx: Vec<f64>,
    dy: Vec<f64>,
}

impl CorGrid {
    /// La rejilla sin desplazamientos (identidad).
    pub fn zero() -> Self {
        CorGrid {
            dx: vec![0.0; GRID * GRID],
            dy: vec![0.0; GRID * GRID],
        }
    }

    /// Una rejilla desde los 65 x 65 pares de `table` (ver la cabecera del módulo). `transpose`
    /// cambia cuál índice del archivo es X; `invert` cambia el signo de los desplazamientos. Falla
    /// si algún nodo desplaza más de [`MAX_DELTA_UNITS`].
    pub fn from_table(table: &CorTable, transpose: bool, invert: bool) -> Result<Self, CorError> {
        let n = GRID * GRID;
        if table.dx.len() != n || table.dy.len() != n {
            return Err(CorError::Truncated {
                needed: n,
                got: table.dx.len().min(table.dy.len()),
            });
        }
        let sign = if invert { -1.0 } else { 1.0 };
        let mut seen = 0.0_f64;
        let mut grid = CorGrid::zero();
        for iy in 0..GRID {
            for ix in 0..GRID {
                let src = if transpose {
                    ix * GRID + iy
                } else {
                    iy * GRID + ix
                };
                let (x, y) = (table.dx[src] * sign, table.dy[src] * sign);
                seen = seen.max(x.abs()).max(y.abs());
                grid.dx[iy * GRID + ix] = x;
                grid.dy[iy * GRID + ix] = y;
            }
        }
        if seen > MAX_DELTA_UNITS {
            return Err(CorError::TooLarge { seen });
        }
        Ok(grid)
    }

    /// Lee un `.cor` del disco. Solo lectura: no abre nada para escribir.
    pub fn load(path: &std::path::Path, transpose: bool, invert: bool) -> Result<Self, CorError> {
        let meta = std::fs::metadata(path).map_err(|e| CorError::Io(e.to_string()))?;
        if meta.len() > MAX_FILE_BYTES {
            return Err(CorError::FileTooBig { bytes: meta.len() });
        }
        let bytes = std::fs::read(path).map_err(|e| CorError::Io(e.to_string()))?;
        Self::from_table(&parse_cor(&bytes)?, transpose, invert)
    }

    /// Desplazamiento (dx, dy), en unidades de la tarjeta, de la posición `(ux, uy)` (también en
    /// unidades, 0..65535): interpolación bilineal entre los cuatro nodos que la rodean. Fuera de
    /// 0..65536 se usa el nodo del borde.
    pub fn delta_units(&self, ux: f64, uy: f64) -> (f64, f64) {
        let edge = (GRID - 1) as f64;
        let fx = (ux / NODE_STEP_UNITS).clamp(0.0, edge);
        let fy = (uy / NODE_STEP_UNITS).clamp(0.0, edge);
        let (ix, iy) = (
            (fx.floor() as usize).min(GRID - 2),
            (fy.floor() as usize).min(GRID - 2),
        );
        let (tx, ty) = (fx - ix as f64, fy - iy as f64);
        let at = |t: &[f64], x: usize, y: usize| t[y * GRID + x];
        let mix = |t: &[f64]| {
            let top = at(t, ix, iy) * (1.0 - tx) + at(t, ix + 1, iy) * tx;
            let bottom = at(t, ix, iy + 1) * (1.0 - tx) + at(t, ix + 1, iy + 1) * tx;
            top * (1.0 - ty) + bottom * ty
        };
        (mix(&self.dx), mix(&self.dy))
    }

    /// Mayor desplazamiento de la tabla, unidades.
    pub fn max_delta_units(&self) -> f64 {
        self.dx
            .iter()
            .chain(&self.dy)
            .fold(0.0, |m, v| m.max(v.abs()))
    }
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
    /// Tabla con un solo nodo distinto de cero, en (fila `row`, columna `col`) del archivo.
    fn one_node(row: usize, col: usize, dx: f64, dy: f64) -> CorTable {
        let mut t = CorTable {
            format: CorFormat::Float,
            scale: 1.0,
            dx: vec![0.0; GRID * GRID],
            dy: vec![0.0; GRID * GRID],
        };
        t.dx[row * GRID + col] = dx;
        t.dy[row * GRID + col] = dy;
        t
    }

    #[test]
    fn a_zero_table_changes_nothing() {
        let g = CorGrid::from_table(&one_node(0, 0, 0.0, 0.0), false, false).unwrap();
        for (x, y) in [(0.0, 0.0), (32768.0, 32768.0), (65535.0, 1234.5)] {
            assert_eq!(g.delta_units(x, y), (0.0, 0.0));
        }
        assert_eq!(g.max_delta_units(), 0.0);
    }

    /// Cálculo a mano: el nodo (columna 1, fila 1) vale dx = 8, dy = -4 y está en (1024, 1024)
    /// unidades. En (1536, 1024) la interpolación en x da la mitad de cada lado (8 y 0): 4 y -2.
    /// En (1536, 1536) además la mitad en y: 2 y -1. En el propio nodo, 8 y -4. Dos nodos más
    /// allá no hay nada.
    #[test]
    fn bilinear_interpolation_matches_hand_values() {
        let g = CorGrid::from_table(&one_node(1, 1, 8.0, -4.0), false, false).unwrap();
        assert_eq!(g.delta_units(1024.0, 1024.0), (8.0, -4.0));
        assert_eq!(g.delta_units(1536.0, 1024.0), (4.0, -2.0));
        assert_eq!(g.delta_units(1536.0, 1536.0), (2.0, -1.0));
        assert_eq!(g.delta_units(512.0, 512.0), (2.0, -1.0));
        assert_eq!(g.delta_units(3072.0, 1024.0), (0.0, 0.0));
    }

    /// Columna = X y fila = Y por omisión; `transpose` los intercambia; `invert` cambia el signo.
    #[test]
    fn orientation_and_sign_follow_the_profile_flags() {
        // Fila 3, columna 7 del archivo.
        let t = one_node(3, 7, 10.0, 20.0);
        let plain = CorGrid::from_table(&t, false, false).unwrap();
        assert_eq!(plain.delta_units(7.0 * 1024.0, 3.0 * 1024.0), (10.0, 20.0));
        assert_eq!(plain.delta_units(3.0 * 1024.0, 7.0 * 1024.0), (0.0, 0.0));
        let swapped = CorGrid::from_table(&t, true, false).unwrap();
        assert_eq!(
            swapped.delta_units(3.0 * 1024.0, 7.0 * 1024.0),
            (10.0, 20.0)
        );
        assert_eq!(swapped.delta_units(7.0 * 1024.0, 3.0 * 1024.0), (0.0, 0.0));
        let flipped = CorGrid::from_table(&t, false, true).unwrap();
        assert_eq!(
            flipped.delta_units(7.0 * 1024.0, 3.0 * 1024.0),
            (-10.0, -20.0)
        );
    }

    #[test]
    fn positions_outside_the_table_use_the_edge_nodes() {
        let g = CorGrid::from_table(&one_node(64, 64, 6.0, 0.0), false, false).unwrap();
        // 65535 está a 1/1024 de nodo antes del último: el peso del último nodo en cada eje es
        // 65535 / 1024 - 63 y el valor, 6 por el producto de los dos.
        let t = 65535.0 / 1024.0 - 63.0;
        assert!((g.delta_units(65535.0, 65535.0).0 - 6.0 * t * t).abs() < 1e-12);
        assert_eq!(g.delta_units(1e9, 1e9), (6.0, 0.0));
        assert_eq!(g.delta_units(-5.0, -5.0), (0.0, 0.0));
    }

    #[test]
    fn a_table_that_moves_too_much_is_refused() {
        let ok = one_node(2, 2, MAX_DELTA_UNITS, 0.0);
        assert!(CorGrid::from_table(&ok, false, false).is_ok());
        let big = one_node(2, 2, 0.0, -(MAX_DELTA_UNITS + 1.0));
        assert_eq!(
            CorGrid::from_table(&big, false, false),
            Err(CorError::TooLarge {
                seen: MAX_DELTA_UNITS + 1.0
            })
        );
    }

    #[test]
    fn loading_reads_the_file_and_never_needs_to_write() {
        let dir = std::env::temp_dir().join(format!("tlaku-cor-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("lente.cor");
        let mut bytes = test_file(0.0, 0.0);
        let at = LABEL_BYTES + 0x1FA + 66 * 16;
        bytes[at..at + 8].copy_from_slice(&12.0_f64.to_le_bytes());
        std::fs::write(&path, bytes).unwrap();
        let g = CorGrid::load(&path, false, false).unwrap();
        // El nodo 66 es la fila 1, columna 1.
        assert_eq!(g.delta_units(1024.0, 1024.0).0, 12.0);
        // Una ruta que no existe es un error de lectura, no un pánico.
        assert!(matches!(
            CorGrid::load(&dir.join("no-existe.cor"), false, false),
            Err(CorError::Io(_))
        ));
        // Un archivo gigante se rechaza sin leerlo.
        let huge = dir.join("huge.cor");
        let f = std::fs::File::create(&huge).unwrap();
        f.set_len(MAX_FILE_BYTES + 1).unwrap();
        assert!(matches!(
            CorGrid::load(&huge, false, false),
            Err(CorError::FileTooBig { .. })
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
