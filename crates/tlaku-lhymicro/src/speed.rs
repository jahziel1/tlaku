//! Códigos de velocidad de LHYMICRO-GL: de mm/s a `CV...` / `V...G...` y de vuelta.
//!
//! Adaptado de `meerk40t/lihuiyu/laserspeed.py` (MeerK40t, MIT, Copyright (c) 2021 meerk40t; ver
//! THIRD_PARTY.md). Las ecuaciones son hechos del protocolo; el código es una reescritura en
//! Rust con dos cambios deliberados de seguridad: una velocidad fuera de rango **se rechaza**
//! (MeerK40t la sustituye en silencio por 19,05 mm/s o la recorta a 0) y los códigos "con error"
//! `V167...` del software chino no se generan.
//!
//! # Modelo (según MeerK40t)
//!
//! La placa mueve motores paso a paso de 1000 pasos por pulgada (1 mil = 0,0254 mm). El código
//! lleva un contador de 16 bits que es **lineal en el periodo entre pasos**:
//!
//! ```text
//! valor = 65536 - (b + m * T)        T = periodo en ms = 25,4 / velocidad(mm/s)
//! ```
//!
//! con `b` y `m` según la placa y el "engranaje" (aceleración 1 a 4):
//!
//! | Placa | `m` | `b` (engranajes 1-2 / 3 / 4) |
//! |---|---|---|
//! | M, M1, M2, M3 | 12120 | 5120 / 5632 / 6144 |
//! | B2 | 24240 | 784 / 896 / 1024 |
//! | A, B, B1 | 2000 | 784 / 896 / 1024 |
//!
//! Con el sufijo `C` (sin rampa de aceleración, "twitchless"): `b = 8` y `m / 12` en M-M3, y
//! `b` del engranaje y `m / 12` en B2; no existe para A, B, B1. El contador se escribe como dos
//! números de tres cifras (alto y bajo) y el engranaje como una cifra. Las placas con razón
//! diagonal añaden `paso` (tres cifras) y un segundo contador de 16 bits con el retardo de las
//! diagonales: `d = razón * m * T / paso` con razón por defecto 0,261199033289 y
//! `paso = min(floor(mm/s) + 1, 128)`.
//!
//! MeerK40t mide que la M2 real va al 91,98 % de lo que dice el software chino (`m` real 11142-11148
//! frente a 12120): `fix_speeds` usa la pendiente corregida. Es un **dato medido por un tercero**
//! (no verificado aquí); sin `fix_speeds` el código es el del software chino, que es el que la
//! placa espera de fábrica.

use std::fmt;

/// Milímetros por mil (milésima de pulgada), la unidad de la placa.
pub const MM_PER_MIL: f64 = 0.0254;
/// Razón diagonal por defecto.
pub const DEFAULT_D_RATIO: f64 = 0.261_199_033_289;
/// Relación medida entre la velocidad real y la pedida en una M2 con el código chino.
pub const M2_SPEED_RATIO: f64 = 0.919_493_599_053_179;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Board {
    A,
    B,
    B1,
    B2,
    M,
    M1,
    M2,
    M3,
}

impl Board {
    pub fn parse(s: &str) -> Option<Board> {
        Some(match s.trim().to_ascii_uppercase().as_str() {
            "A" => Board::A,
            "B" => Board::B,
            "B1" => Board::B1,
            "B2" => Board::B2,
            "M" => Board::M,
            "M1" => Board::M1,
            "M2" => Board::M2,
            "M3" => Board::M3,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Board::A => "A",
            Board::B => "B",
            Board::B1 => "B1",
            Board::B2 => "B2",
            Board::M => "M",
            Board::M1 => "M1",
            Board::M2 => "M2",
            Board::M3 => "M3",
        }
    }

    /// Las placas A, B y M no llevan el bloque de razón diagonal en el código.
    fn has_diagonal(self) -> bool {
        !matches!(self, Board::A | Board::B | Board::M)
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Paso del modo trama (`G`): la placa avanza ese número de mils al invertir el sentido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterStep {
    /// Vectorial: sin `G`.
    None,
    /// Se avanza en la ida y en la vuelta (`Gnnn`).
    Bidirectional(u16),
    /// Solo en uno de los sentidos (`Gnnn Gmmm`).
    Unidirectional(u16, u16),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpeedParams {
    pub board: Board,
    pub mm_per_s: f64,
    pub raster: RasterStep,
    /// `None` = el del software chino (0,261199033289).
    pub d_ratio: Option<f64>,
    /// `None` = el engranaje que corresponde a la velocidad.
    pub acceleration: Option<u8>,
    /// `Some(true)` fuerza el sufijo `C` (sin rampa); `None` = el automático de la placa
    /// (solo por debajo de 7 mm/s en M2, M3 y B2).
    pub suffix_c: Option<bool>,
    pub fix_speeds: bool,
    /// La trama es horizontal (el cabezal barre, no la barra): cambia el engranaje.
    pub raster_horizontal: bool,
}

impl SpeedParams {
    pub fn new(board: Board, mm_per_s: f64) -> Self {
        SpeedParams {
            board,
            mm_per_s,
            raster: RasterStep::None,
            d_ratio: None,
            acceleration: None,
            suffix_c: None,
            fix_speeds: false,
            raster_horizontal: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpeedError {
    /// No es un número finito mayor que cero.
    Invalid(f64),
    /// El contador saldría negativo: la placa no puede ir tan despacio con esas ecuaciones.
    TooSlow {
        mm_per_s: f64,
        min_mm_per_s: f64,
    },
    BadCode(String),
}

impl fmt::Display for SpeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpeedError::Invalid(v) => write!(f, "velocidad inválida: {v} mm/s"),
            SpeedError::TooSlow {
                mm_per_s,
                min_mm_per_s,
            } => write!(
                f,
                "{mm_per_s} mm/s es demasiado lento para esta placa (mínimo {min_mm_per_s:.2} mm/s)"
            ),
            SpeedError::BadCode(c) => write!(f, "código de velocidad no válido: {c:?}"),
        }
    }
}

impl std::error::Error for SpeedError {}

/// Engranaje (1 a 4) que usa el software chino para una velocidad.
pub fn acceleration_for_speed(
    mm_per_s: f64,
    raster: bool,
    raster_horizontal: bool,
    fix_speeds: bool,
) -> u8 {
    let s = if fix_speeds {
        mm_per_s / M2_SPEED_RATIO
    } else {
        mm_per_s
    };
    if s <= 25.4 {
        1
    } else if s <= 60.0 {
        2
    } else if raster && raster_horizontal {
        if s < 127.0 {
            2
        } else if s <= 320.0 {
            3
        } else {
            4
        }
    } else if s < 127.0 {
        3
    } else {
        4
    }
}

/// Sufijo `C` automático: M2, M3 y B2 por debajo de 7 mm/s (por debajo de eso el contador del
/// engranaje normal se desbordaría).
pub fn default_suffix_c(board: Board, mm_per_s: f64) -> bool {
    matches!(board, Board::B2 | Board::M2 | Board::M3) && mm_per_s < 7.0
}

/// Ecuación `(b, m)` de la placa.
pub fn equation(board: Board, accel: u8, suffix_c: bool, fix_speeds: bool) -> (f64, f64) {
    let mut b = match accel {
        3 => 896.0,
        4 => 1024.0,
        _ => 784.0,
    };
    if matches!(board, Board::A | Board::B | Board::B1) {
        return (b, 2000.0);
    }
    let mut m = if fix_speeds { 11_148.0 } else { 12_120.0 };
    if board == Board::B2 {
        m *= 2.0;
        if suffix_c {
            return (b, m / 12.0);
        }
    } else {
        b = match accel {
            3 => 5632.0,
            4 => 6144.0,
            _ => 5120.0,
        };
        if suffix_c {
            return (8.0, m / 12.0);
        }
    }
    (b, m)
}

fn period_ms(mm_per_s: f64) -> f64 {
    25.4 / mm_per_s
}

/// Contador de 16 bits para una velocidad (puede salir negativo si es demasiado lenta).
pub fn value_from_speed(mm_per_s: f64, b: f64, m: f64) -> f64 {
    65536.0 - (m * period_ms(mm_per_s) + b)
}

/// Velocidad (mm/s) que corresponde a un contador `65536 - decodificado`.
pub fn speed_from_value(value: f64, b: f64, m: f64) -> f64 {
    let period = (value - b) / m;
    if period == 0.0 {
        return 0.0;
    }
    25.4 / period
}

/// Dos números de tres cifras: byte alto y bajo.
pub fn encode_16bit(value: i64) -> String {
    format!("{:03}{:03}", (value >> 8) & 0xFF_FFFF, value & 0xFF)
}

/// Inverso de [`encode_16bit`] (también acepta las formas "con error" de 8 cifras).
pub fn decode_16bit(code: &str) -> Option<i64> {
    if code.len() < 4 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let split = code.len() - 3;
    let mut hi: i64 = code[..split].parse().ok()?;
    let lo: i64 = code[split..].parse().ok()?;
    if hi > 16_000_000 {
        hi -= 16_777_216;
    }
    if hi > 0x7FFF {
        hi -= 0xFFFF;
    }
    Some((hi << 8) + lo)
}

/// Velocidad mínima (mm/s) con contador no negativo para una ecuación.
fn min_speed(b: f64, m: f64) -> f64 {
    25.4 * m / (65536.0 - b)
}

/// Código de velocidad listo para el flujo (`CV...C`, `CV...` o `V...G...`).
pub fn speed_code(p: &SpeedParams) -> Result<String, SpeedError> {
    if !(p.mm_per_s.is_finite() && p.mm_per_s > 0.0) {
        return Err(SpeedError::Invalid(p.mm_per_s));
    }
    let raster = p.raster != RasterStep::None;
    // Cosa rara heredada del software chino (y de MeerK40t): en trama lenta (< 7 mm/s en
    // M2, M3 y B2) la ecuación usa las constantes del modo `C` aunque el código no lleve la `C`.
    // Por eso un código de trama lento no se puede leer de vuelta sin ambigüedad.
    let suffix_c = p
        .suffix_c
        .unwrap_or_else(|| default_suffix_c(p.board, p.mm_per_s));
    let mut accel = p.acceleration.unwrap_or_else(|| {
        acceleration_for_speed(p.mm_per_s, raster, p.raster_horizontal, p.fix_speeds)
    });
    if suffix_c && !raster {
        accel = 1;
    }
    let (b, m) = equation(p.board, accel, suffix_c, p.fix_speeds);
    let value = value_from_speed(p.mm_per_s, b, m);
    if value < 0.0 {
        return Err(SpeedError::TooSlow {
            mm_per_s: p.mm_per_s,
            min_mm_per_s: min_speed(b, m),
        });
    }
    let encoded = encode_16bit(value as i64);
    match p.raster {
        RasterStep::Bidirectional(g) => return Ok(format!("V{encoded}{accel}G{g:03}")),
        RasterStep::Unidirectional(g0, g1) => {
            return Ok(format!("V{encoded}{accel}G{g0:03}G{g1:03}"));
        }
        RasterStep::None => {}
    }
    let d_ratio = p.d_ratio.unwrap_or(DEFAULT_D_RATIO);
    if d_ratio == 0.0 || !p.board.has_diagonal() {
        return Ok(if suffix_c {
            format!("CV{encoded}1C")
        } else {
            format!("CV{encoded}{accel}")
        });
    }
    let step = ((p.mm_per_s.floor() as i64) + 1).min(128);
    let d_value = (d_ratio * m * period_ms(p.mm_per_s) / step as f64).clamp(0.0, 65535.0);
    let encoded_d = encode_16bit(d_value as i64);
    Ok(if suffix_c {
        format!("CV{encoded}1{step:03}{encoded_d}C")
    } else {
        format!("CV{encoded}{accel}{step:03}{encoded_d}")
    })
}

/// Partes de un código de velocidad.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSpeed {
    /// `65536 - contador`: el valor que entra en la ecuación.
    pub code_value: i64,
    pub accel: u8,
    pub step_value: u32,
    pub diagonal: i64,
    pub raster: RasterStep,
    pub suffix_c: bool,
}

/// Descompone un código de velocidad (`CV...`, `V...`, con o sin `C` final).
pub fn parse_speed_code(code: &str) -> Result<ParsedSpeed, SpeedError> {
    let bad = || SpeedError::BadCode(code.to_string());
    let mut s = code;
    if let Some(rest) = s.strip_prefix('C') {
        s = rest;
    }
    let suffix_c = if let Some(rest) = s.strip_suffix('C') {
        s = rest;
        true
    } else {
        false
    };
    let s = s.strip_prefix('V').ok_or_else(bad)?;
    if s.len() < 7 || !s.is_char_boundary(7) {
        return Err(bad());
    }
    let code_value = 65536 - decode_16bit(&s[..6]).ok_or_else(bad)?;
    let accel: u8 = s[6..7].parse().map_err(|_| bad())?;
    let mut rest = &s[7..];
    // Pasos de trama al final: `Gnnn` o `GnnnGmmm`.
    let mut gs: Vec<u16> = Vec::new();
    while rest.len() >= 4 && rest.as_bytes()[rest.len() - 4] == b'G' {
        let n: u16 = rest[rest.len() - 3..].parse().map_err(|_| bad())?;
        gs.insert(0, n);
        rest = &rest[..rest.len() - 4];
    }
    let raster = match gs.as_slice() {
        [] => RasterStep::None,
        [g] => RasterStep::Bidirectional(*g),
        [a, b] => RasterStep::Unidirectional(*a, *b),
        _ => return Err(bad()),
    };
    let (mut step_value, mut diagonal) = (0, 0);
    if rest.len() >= 9 {
        step_value = rest[..3].parse().map_err(|_| bad())?;
        diagonal = decode_16bit(&rest[3..]).ok_or_else(bad)?;
    } else if !rest.is_empty() {
        return Err(bad());
    }
    Ok(ParsedSpeed {
        code_value,
        accel,
        step_value,
        diagonal,
        raster,
        suffix_c,
    })
}

/// Velocidad (mm/s) que corresponde a un código para una placa.
pub fn speed_from_code(code: &str, board: Board, fix_speeds: bool) -> Result<f64, SpeedError> {
    let p = parse_speed_code(code)?;
    let (b, m) = equation(board, p.accel, p.suffix_c, fix_speeds);
    Ok(speed_from_value(p.code_value as f64, b, m))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VECTORS: &str = include_str!("../tests/fixtures/speedcodes_meerk40t.txt");

    struct Vector {
        code: String,
        board: Board,
        mm_s: f64,
        step: u16,
    }

    fn vectors() -> Vec<Vector> {
        VECTORS
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .map(|l| {
                let f: Vec<&str> = l.split(' ').collect();
                Vector {
                    code: f[0].to_string(),
                    board: Board::parse(f[1]).unwrap(),
                    mm_s: f[2].parse().unwrap(),
                    step: f[3].parse().unwrap(),
                }
            })
            .collect()
    }

    fn generate(v: &Vector) -> Result<String, SpeedError> {
        let mut p = SpeedParams::new(v.board, v.mm_s);
        if v.step != 0 {
            p.raster = RasterStep::Bidirectional(v.step);
        }
        speed_code(&p)
    }

    #[test]
    fn worked_examples_of_the_meerk40t_docs() {
        // Ejemplo de laserspeed.py: 25,4 mm/s en M2 con engranaje 1 -> contador 48296
        // (141 y 80 en tres cifras) = 65536 - (5120 + 12120 * 1).
        let (b, m) = equation(Board::M2, 1, false, false);
        let v = value_from_speed(25.4, b, m);
        assert_eq!(v as i64, 48296);
        assert_eq!(encode_16bit(48296), "188168");
        // 12,7 mm/s = 2 ms -> 36176 = 141 * 256 + 80.
        let v = value_from_speed(12.7, b, m);
        assert_eq!(v as i64, 36176);
        assert_eq!(encode_16bit(36176), "141080");
    }

    #[test]
    fn rectangle_example_from_the_meerk40t_egv_test() {
        // `ICV2490731016000027CN...` es el código de 15 mm/s en M2 (engranaje 1 con C).
        let mut p = SpeedParams::new(Board::M2, 15.0);
        p.suffix_c = Some(true);
        assert_eq!(speed_code(&p).unwrap(), "CV2490731016000027C");
    }

    #[test]
    fn generated_codes_match_the_vectors() {
        let mut checked = 0;
        for v in vectors() {
            // El software chino sustituía por ~19 mm/s lo que pasaba de 240 mm/s en vectorial.
            if v.mm_s <= 0.0 || (v.step == 0 && v.mm_s > 240.0) {
                continue;
            }
            let created = match generate(&v) {
                Ok(c) => c,
                Err(SpeedError::TooSlow { .. }) => {
                    // Los vectores con contador negativo son los códigos "con error".
                    assert!(
                        v.code.starts_with("CV167") || v.code.starts_with("V167"),
                        "{}",
                        v.code
                    );
                    continue;
                }
                Err(e) => panic!("{} {} {}: {e}", v.board, v.mm_s, v.step),
            };
            let ctx = format!(
                "{} {} mm/s paso {}: {} vs {}",
                v.board, v.mm_s, v.step, created, v.code
            );
            if v.step != 0 {
                let a = decode_16bit(&created[1..7]).unwrap();
                let e = decode_16bit(&v.code[1..7]).unwrap();
                assert!((a - e).abs() <= 1, "{ctx}");
                assert_eq!(created.as_bytes()[7], v.code.as_bytes()[7], "{ctx}");
                assert_eq!(created[8..], v.code[8..], "{ctx}");
            } else {
                let a = decode_16bit(&created[2..8]).unwrap();
                let e = decode_16bit(&v.code[2..8]).unwrap();
                assert!((a - e).abs() <= 1, "{ctx}");
                assert_eq!(created.as_bytes()[8], v.code.as_bytes()[8], "{ctx}");
                if !matches!(v.board, Board::A | Board::B | Board::M) {
                    assert_eq!(created[9..12], v.code[9..12], "paso: {ctx}");
                    let a = decode_16bit(&created[12..18]).unwrap() as f64;
                    let e = decode_16bit(&v.code[12..18]).unwrap() as f64;
                    assert!((a - e).abs() <= (e / 3000.0).max(1.0), "diagonal: {ctx}");
                }
            }
            checked += 1;
        }
        assert!(checked > 1000, "se comprobaron {checked} vectores");
    }

    #[test]
    fn decoded_speeds_match_the_vectors() {
        for v in vectors() {
            if v.mm_s <= 0.0 || (v.step == 0 && v.mm_s > 240.0) {
                continue;
            }
            if v.step != 0 && v.mm_s < 7.0 {
                continue; // código ambiguo en el original
            }
            if v.code.contains("V167") {
                continue;
            }
            let s = speed_from_code(&v.code, v.board, false).unwrap();
            assert!(
                (s - v.mm_s).abs() <= v.mm_s / 100.0,
                "{} {}: {} vs {}",
                v.board,
                v.code,
                s,
                v.mm_s
            );
        }
    }

    #[test]
    fn full_circle_per_gear_and_board() {
        let boards = [
            Board::A,
            Board::B,
            Board::B1,
            Board::B2,
            Board::M,
            Board::M1,
            Board::M2,
            Board::M3,
        ];
        for board in boards {
            for gear in 1..=4u8 {
                for i in (30..2400).step_by(7) {
                    let mm_s = i as f64 / 10.0;
                    let mut p = SpeedParams::new(board, mm_s);
                    p.acceleration = Some(gear);
                    p.suffix_c = Some(false);
                    match speed_code(&p) {
                        Ok(code) => {
                            let back = speed_from_code(&code, board, false).unwrap();
                            assert!(
                                (back - mm_s).abs() <= mm_s / 200.0,
                                "{board} g{gear} {mm_s}: {code} -> {back}"
                            );
                        }
                        Err(SpeedError::TooSlow { .. }) => {}
                        Err(e) => panic!("{e}"),
                    }
                }
            }
        }
    }

    #[test]
    fn slow_speeds_use_the_c_suffix_on_m2_and_never_overflow() {
        for i in 4..70 {
            let mm_s = i as f64 / 10.0;
            let code = speed_code(&SpeedParams::new(Board::M2, mm_s)).unwrap();
            assert!(code.ends_with('C'), "{mm_s}: {code}");
            let back = speed_from_code(&code, Board::M2, false).unwrap();
            assert!((back - mm_s).abs() <= mm_s / 100.0, "{mm_s}: {back}");
        }
        // Sin C, 7 mm/s ya cabe.
        assert!(
            !speed_code(&SpeedParams::new(Board::M2, 7.0))
                .unwrap()
                .ends_with('C')
        );
    }

    #[test]
    fn too_slow_is_an_error_not_a_silent_clamp() {
        // En una placa M (sin sufijo C) a 1 mm/s el contador sería negativo.
        let e = speed_code(&SpeedParams::new(Board::M, 1.0)).unwrap_err();
        assert!(matches!(e, SpeedError::TooSlow { .. }), "{e}");
        assert!(speed_code(&SpeedParams::new(Board::M2, 0.0)).is_err());
        assert!(speed_code(&SpeedParams::new(Board::M2, f64::NAN)).is_err());
        assert!(speed_code(&SpeedParams::new(Board::M2, -3.0)).is_err());
    }

    #[test]
    fn gears_follow_the_chinese_software_ranges() {
        assert_eq!(acceleration_for_speed(10.0, false, true, false), 1);
        assert_eq!(acceleration_for_speed(25.4, false, true, false), 1);
        assert_eq!(acceleration_for_speed(40.0, false, true, false), 2);
        assert_eq!(acceleration_for_speed(100.0, false, true, false), 3);
        assert_eq!(acceleration_for_speed(200.0, false, true, false), 4);
        // Trama horizontal: engranaje 2 hasta 127 y 3 hasta 320.
        assert_eq!(acceleration_for_speed(100.0, true, true, false), 2);
        assert_eq!(acceleration_for_speed(200.0, true, true, false), 3);
        assert_eq!(acceleration_for_speed(400.0, true, true, false), 4);
    }

    #[test]
    fn fix_speeds_changes_the_slope() {
        let mut a = SpeedParams::new(Board::M2, 50.0);
        let plain = speed_code(&a).unwrap();
        a.fix_speeds = true;
        let fixed = speed_code(&a).unwrap();
        assert_ne!(plain, fixed);
        // El código corregido es más rápido; el mismo código leído con la pendiente real da una
        // velocidad ~8 % menor (la relación medida por MeerK40t).
        let s_plain = speed_from_code(&plain, Board::M2, false).unwrap();
        let s_real = speed_from_code(&plain, Board::M2, true).unwrap();
        assert!(
            (s_real / s_plain - M2_SPEED_RATIO).abs() < 0.01,
            "{}",
            s_real / s_plain
        );
        let s_fixed = speed_from_code(&fixed, Board::M2, true).unwrap();
        assert!((s_fixed - 50.0).abs() < 0.5);
    }

    #[test]
    fn parse_pieces_of_a_code() {
        let p = parse_speed_code("CV2490731016000027C").unwrap();
        assert_eq!((p.accel, p.step_value, p.suffix_c), (1, 16, true));
        assert_eq!(p.code_value, 65536 - (249 * 256 + 73));
        assert_eq!(p.diagonal, 27);
        let p = parse_speed_code("V1552121G002").unwrap();
        assert_eq!(p.raster, RasterStep::Bidirectional(2));
        assert_eq!(p.accel, 1);
        let p = parse_speed_code("V1552121G002G001").unwrap();
        assert_eq!(p.raster, RasterStep::Unidirectional(2, 1));
        assert!(parse_speed_code("hola").is_err());
        assert!(parse_speed_code("CV12").is_err());
    }

    #[test]
    fn board_names_round_trip() {
        for name in ["A", "B", "B1", "B2", "M", "M1", "M2", "M3"] {
            assert_eq!(Board::parse(name).unwrap().name(), name);
        }
        assert_eq!(Board::parse(" m2 "), Some(Board::M2));
        assert_eq!(Board::parse("X"), None);
    }
}
