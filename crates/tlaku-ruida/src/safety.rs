//! Límites de seguridad antes de generar o enviar nada.
//!
//! El protocolo no impone límites de potencia, velocidad ni coordenadas (ruida.md sección 7): los
//! valores van tal cual y el firmware reacciona de forma desconocida a lo que se salga de rango.
//! Aquí se rechaza todo antes de producir un solo byte.
//!
//! Reglas aplicadas (ruida.md sección 7):
//! - nada de NaN ni infinitos;
//! - potencia de 0 a 100 %;
//! - velocidad mayor que 0 y menor o igual que el tope del perfil;
//! - coordenadas dentro de la cama;
//! - ninguna escritura de memoria (`DA 01`) salvo permiso explícito.

use crate::cmd::{self, Command, RapidRef};
use crate::num;
use std::fmt;
use tlaku_core::toolpath::{Move, Toolpath};

/// Margen (mm) para no rechazar un diseño que toca el borde por error de redondeo.
const BED_EPS_MM: f64 = 1e-6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SafetyLimits {
    /// Ancho y alto de la cama en mm, origen en la esquina de la que parten las coordenadas.
    pub bed_mm: [f64; 2],
    /// Velocidad máxima del perfil en mm/min.
    pub max_speed_mm_min: f64,
    /// Potencia máxima permitida en % (nunca más de 100).
    pub max_power_pct: f64,
}

impl Default for SafetyLimits {
    /// Valores de relleno (cama 600 x 400 mm, 600 mm/s, 100 %): el perfil de la máquina real
    /// debe fijarlos. No se corresponden con ninguna máquina concreta.
    fn default() -> Self {
        SafetyLimits {
            bed_mm: [600.0, 400.0],
            max_speed_mm_min: 36_000.0,
            max_power_pct: 100.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SafetyError {
    /// Los límites del perfil no son válidos.
    BadLimits(&'static str),
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
    Power {
        block: usize,
        pct: f64,
        max: f64,
    },
    Speed {
        block: usize,
        mm_min: f64,
        max: f64,
    },
    /// Eje Z y rotativo quedan fuera de la primera etapa.
    ZUnsupported {
        block: usize,
    },
    /// Comando prohibido en el flujo (escritura de memoria sin permiso).
    ForbiddenCommand {
        index: usize,
        what: String,
    },
    /// Un comando del flujo lleva coordenadas fuera de la cama, en µm.
    StreamOutOfBed {
        index: usize,
        x_um: i64,
        y_um: i64,
    },
    StreamPower {
        index: usize,
        pct: f64,
        max: f64,
    },
    StreamSpeed {
        index: usize,
        mm_min: f64,
        max: f64,
    },
    Decode(String),
}

impl fmt::Display for SafetyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SafetyError::BadLimits(w) => write!(f, "límites de seguridad inválidos: {w}"),
            SafetyError::NonFinite { block, what } => {
                write!(f, "bloque {block}: {what} no es un número finito")
            }
            SafetyError::OutOfBed {
                block,
                index,
                x_mm,
                y_mm,
            } => write!(
                f,
                "bloque {block}, movimiento {index}: ({x_mm}, {y_mm}) mm queda fuera de la cama"
            ),
            SafetyError::Power { block, pct, max } => write!(
                f,
                "bloque {block}: potencia {pct} % fuera del rango permitido 0 a {max} %"
            ),
            SafetyError::Speed { block, mm_min, max } => write!(
                f,
                "bloque {block}: velocidad {mm_min} mm/min fuera del rango permitido (mayor que 0 y hasta {max})"
            ),
            SafetyError::ZUnsupported { block } => write!(
                f,
                "bloque {block}: el eje Z / rotativo no está soportado todavía en Ruida"
            ),
            SafetyError::ForbiddenCommand { index, what } => {
                write!(f, "comando {index}: {what} no está permitido")
            }
            SafetyError::StreamOutOfBed { index, x_um, y_um } => write!(
                f,
                "comando {index}: la posición ({x_um}, {y_um}) µm queda fuera de la cama"
            ),
            SafetyError::StreamPower { index, pct, max } => {
                write!(
                    f,
                    "comando {index}: potencia {pct:.3} % por encima de {max} %"
                )
            }
            SafetyError::StreamSpeed { index, mm_min, max } => write!(
                f,
                "comando {index}: velocidad {mm_min:.1} mm/min por encima de {max} mm/min"
            ),
            SafetyError::Decode(e) => write!(f, "flujo ilegible: {e}"),
        }
    }
}

impl std::error::Error for SafetyError {}

impl SafetyLimits {
    pub fn validate(&self) -> Result<(), SafetyError> {
        let finite_pos = |v: f64| v.is_finite() && v > 0.0;
        if !self.bed_mm.iter().copied().all(finite_pos) {
            return Err(SafetyError::BadLimits("la cama debe medir más de 0 mm"));
        }
        if !finite_pos(self.max_speed_mm_min) {
            return Err(SafetyError::BadLimits(
                "la velocidad máxima debe ser mayor que 0",
            ));
        }
        if !(self.max_power_pct.is_finite() && (0.0..=100.0).contains(&self.max_power_pct)) {
            return Err(SafetyError::BadLimits(
                "la potencia máxima debe estar entre 0 y 100 %",
            ));
        }
        Ok(())
    }
}

/// Verifica el `Toolpath` (coordenadas del documento, mm) antes de generar el archivo.
pub fn check_toolpath(tp: &Toolpath, limits: &SafetyLimits) -> Result<(), SafetyError> {
    limits.validate()?;
    for (b, block) in tp.blocks.iter().enumerate() {
        if block.z_mm.is_some() {
            return Err(SafetyError::ZUnsupported { block: b });
        }
        let s = block.speed_mm_min;
        if !s.is_finite() {
            return Err(SafetyError::NonFinite {
                block: b,
                what: "la velocidad",
            });
        }
        if s <= 0.0 || s > limits.max_speed_mm_min {
            return Err(SafetyError::Speed {
                block: b,
                mm_min: s,
                max: limits.max_speed_mm_min,
            });
        }
        for (i, m) in block.moves.iter().enumerate() {
            let p = m.to();
            if !(p.x.is_finite() && p.y.is_finite()) {
                return Err(SafetyError::NonFinite {
                    block: b,
                    what: "una coordenada",
                });
            }
            if p.x < -BED_EPS_MM
                || p.y < -BED_EPS_MM
                || p.x > limits.bed_mm[0] + BED_EPS_MM
                || p.y > limits.bed_mm[1] + BED_EPS_MM
            {
                return Err(SafetyError::OutOfBed {
                    block: b,
                    index: i,
                    x_mm: p.x,
                    y_mm: p.y,
                });
            }
            if let Move::Cut { power_pct, .. } = m {
                if !power_pct.is_finite() {
                    return Err(SafetyError::NonFinite {
                        block: b,
                        what: "la potencia",
                    });
                }
                if *power_pct < 0.0 || *power_pct > limits.max_power_pct {
                    return Err(SafetyError::Power {
                        block: b,
                        pct: *power_pct,
                        max: limits.max_power_pct,
                    });
                }
            }
        }
    }
    Ok(())
}

/// Verifica una lista de comandos ya generada: potencias, velocidades, posición (absoluta o
/// resultado de sumar los relativos, partiendo de (0, 0)) y comandos prohibidos.
/// `allow_memory_write` permite `DA 01` (solo para el pie opcional del archivo).
pub fn check_commands(
    cmds: &[Command],
    limits: &SafetyLimits,
    allow_memory_write: bool,
) -> Result<(), SafetyError> {
    check_commands_at(cmds, limits, allow_memory_write, (0, 0))
}

/// Como [`check_commands`] pero con la posición inicial del cabezal (µm, sistema de la máquina):
/// los saltos y movimientos relativos se suman a ella.
pub fn check_commands_at(
    cmds: &[Command],
    limits: &SafetyLimits,
    allow_memory_write: bool,
    start: (i64, i64),
) -> Result<(), SafetyError> {
    limits.validate()?;
    let bed_um = [
        (limits.bed_mm[0] * 1000.0).round() as i64,
        (limits.bed_mm[1] * 1000.0).round() as i64,
    ];
    let max_um_s = limits.max_speed_mm_min * 1000.0 / 60.0;
    let (mut x, mut y) = start;
    let in_bed = |x: i64, y: i64| x >= 0 && y >= 0 && x <= bed_um[0] && y <= bed_um[1];
    let check_pow = |index: usize, raw: u16| {
        let pct = num::power_to_pct(raw);
        // Un paso de la escala (0,006 %) de tolerancia: el 100 % es 16383.
        if pct > limits.max_power_pct + 100.0 / 16383.0 {
            Err(SafetyError::StreamPower {
                index,
                pct,
                max: limits.max_power_pct,
            })
        } else {
            Ok(())
        }
    };
    let check_speed = |index: usize, um_s: u64| {
        if um_s == 0 || um_s as f64 > max_um_s + 0.5 {
            Err(SafetyError::StreamSpeed {
                index,
                mm_min: num::speed_to_mm_min(um_s),
                max: limits.max_speed_mm_min,
            })
        } else {
            Ok(())
        }
    };
    for (i, c) in cmds.iter().enumerate() {
        let (nx, ny) = match c {
            Command::MoveAbs { x, y } | Command::CutAbs { x, y } => (*x, *y),
            Command::MoveRel { dx, dy } | Command::CutRel { dx, dy } => {
                (x + i64::from(*dx), y + i64::from(*dy))
            }
            Command::MoveRelX(d) | Command::CutRelX(d) => (x + i64::from(*d), y),
            Command::MoveRelY(d) | Command::CutRelY(d) => (x, y + i64::from(*d)),
            Command::RapidXY {
                rel: RapidRef::Machine,
                x,
                y,
            } => (*x, *y),
            Command::RapidXY {
                rel: RapidRef::Current,
                x: dx,
                y: dy,
            } => (x + dx, y + dy),
            Command::RapidX {
                rel: RapidRef::Machine,
                v,
            } => (*v, y),
            Command::RapidX {
                rel: RapidRef::Current,
                v,
            } => (x + v, y),
            Command::RapidY {
                rel: RapidRef::Machine,
                v,
            } => (x, *v),
            Command::RapidY {
                rel: RapidRef::Current,
                v,
            } => (x, y + v),
            Command::LaserPower { raw, .. }
            | Command::LayerPower { raw, .. }
            | Command::ImmPower(raw)
            | Command::EndPower(raw) => {
                check_pow(i, *raw)?;
                (x, y)
            }
            // Un salto `D9` que no es de los conocidos puede llevar el bit del haz de luz
            // (dispara el láser): nunca se envía.
            Command::Unknown { op: 0xD9, .. } => {
                return Err(SafetyError::ForbiddenCommand {
                    index: i,
                    what: "un salto D9 con opciones desconocidas (puede encender el láser)".into(),
                });
            }
            Command::Speed(v) | Command::LayerSpeed { um_s: v, .. } => {
                check_speed(i, *v)?;
                (x, y)
            }
            Command::MemWrite { .. } if !allow_memory_write => {
                return Err(SafetyError::ForbiddenCommand {
                    index: i,
                    what: "la escritura de memoria (DA 01)".into(),
                });
            }
            _ => (x, y),
        };
        if (nx, ny) != (x, y) {
            if !in_bed(nx, ny) {
                return Err(SafetyError::StreamOutOfBed {
                    index: i,
                    x_um: nx,
                    y_um: ny,
                });
            }
            (x, y) = (nx, ny);
        }
    }
    Ok(())
}

/// Como [`check_commands`] sobre bytes en claro.
pub fn check_stream(
    plain: &[u8],
    limits: &SafetyLimits,
    allow_memory_write: bool,
) -> Result<(), SafetyError> {
    let cmds = cmd::decode_stream(plain).map_err(|e| SafetyError::Decode(e.to_string()))?;
    check_commands(&cmds, limits, allow_memory_write)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::Limit;
    use tlaku_core::Point;
    use tlaku_core::toolpath::ToolpathBlock;

    fn tp(moves: Vec<Move>, speed: f64) -> Toolpath {
        Toolpath {
            blocks: vec![ToolpathBlock {
                speed_mm_min: speed,
                moves,
                ..Default::default()
            }],
        }
    }

    fn cut(x: f64, y: f64, p: f64) -> Move {
        Move::Cut {
            to: Point::new(x, y),
            power_pct: p,
        }
    }

    #[test]
    fn accepts_valid_job_and_edges() {
        let l = SafetyLimits::default();
        let t = tp(vec![cut(0.0, 0.0, 0.0), cut(600.0, 400.0, 100.0)], 3000.0);
        check_toolpath(&t, &l).unwrap();
    }

    #[test]
    fn rejects_non_finite() {
        let l = SafetyLimits::default();
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(
                check_toolpath(&tp(vec![cut(bad, 1.0, 10.0)], 3000.0), &l),
                Err(SafetyError::NonFinite { .. })
            ));
            assert!(matches!(
                check_toolpath(&tp(vec![cut(1.0, 1.0, bad)], 3000.0), &l),
                Err(SafetyError::NonFinite { .. })
            ));
            assert!(matches!(
                check_toolpath(&tp(vec![cut(1.0, 1.0, 10.0)], bad), &l),
                Err(SafetyError::NonFinite { .. })
            ));
        }
    }

    #[test]
    fn rejects_out_of_bed_power_and_speed() {
        let l = SafetyLimits::default();
        assert!(matches!(
            check_toolpath(&tp(vec![cut(600.1, 1.0, 10.0)], 3000.0), &l),
            Err(SafetyError::OutOfBed { .. })
        ));
        assert!(matches!(
            check_toolpath(&tp(vec![cut(-0.1, 1.0, 10.0)], 3000.0), &l),
            Err(SafetyError::OutOfBed { .. })
        ));
        assert!(matches!(
            check_toolpath(&tp(vec![cut(1.0, 1.0, 100.5)], 3000.0), &l),
            Err(SafetyError::Power { .. })
        ));
        assert!(matches!(
            check_toolpath(&tp(vec![cut(1.0, 1.0, -1.0)], 3000.0), &l),
            Err(SafetyError::Power { .. })
        ));
        assert!(matches!(
            check_toolpath(&tp(vec![cut(1.0, 1.0, 10.0)], 0.0), &l),
            Err(SafetyError::Speed { .. })
        ));
        assert!(matches!(
            check_toolpath(&tp(vec![cut(1.0, 1.0, 10.0)], 36_000.1), &l),
            Err(SafetyError::Speed { .. })
        ));
    }

    #[test]
    fn rejects_z_and_bad_limits() {
        let l = SafetyLimits::default();
        let mut t = tp(vec![cut(1.0, 1.0, 10.0)], 3000.0);
        t.blocks[0].z_mm = Some(1.0);
        assert!(matches!(
            check_toolpath(&t, &l),
            Err(SafetyError::ZUnsupported { .. })
        ));
        let bad = SafetyLimits {
            max_power_pct: 101.0,
            ..l
        };
        assert!(matches!(
            check_toolpath(&tp(vec![], 1.0), &bad),
            Err(SafetyError::BadLimits(_))
        ));
        let bad = SafetyLimits {
            bed_mm: [0.0, 10.0],
            ..l
        };
        assert!(bad.validate().is_err());
    }

    #[test]
    fn stream_checks_track_relative_position() {
        let l = SafetyLimits {
            bed_mm: [10.0, 10.0],
            ..SafetyLimits::default()
        };
        let ok = [
            Command::MoveAbs { x: 9_000, y: 9_000 },
            Command::CutRel { dx: 1_000, dy: 0 },
        ];
        check_commands(&ok, &l, false).unwrap();
        let bad = [
            Command::MoveAbs { x: 9_000, y: 9_000 },
            Command::CutRel { dx: 1_001, dy: 0 },
        ];
        assert!(matches!(
            check_commands(&bad, &l, false),
            Err(SafetyError::StreamOutOfBed { index: 1, .. })
        ));
    }

    #[test]
    fn stream_rejects_memory_write_unless_allowed() {
        let l = SafetyLimits::default();
        let w = [Command::MemWrite {
            addr: 0x320,
            values: vec![1],
        }];
        assert!(matches!(
            check_commands(&w, &l, false),
            Err(SafetyError::ForbiddenCommand { .. })
        ));
        check_commands(&w, &l, true).unwrap();
        // La lectura siempre se permite.
        check_commands(&[Command::MemRead { addr: 0x2FE }], &l, false).unwrap();
    }

    #[test]
    fn stream_rejects_power_and_speed_over_limit() {
        let l = SafetyLimits {
            max_power_pct: 50.0,
            ..SafetyLimits::default()
        };
        let p = |raw| Command::LaserPower {
            laser: 1,
            limit: Limit::Max,
            raw,
        };
        check_commands(&[p(8192)], &l, false).unwrap();
        assert!(check_commands(&[p(8300)], &l, false).is_err());
        assert!(check_commands(&[Command::Speed(0)], &l, false).is_err());
        assert!(check_commands(&[Command::Speed(600_001)], &l, false).is_err());
        check_commands(&[Command::Speed(600_000)], &l, false).unwrap();
    }
}
