//! Representación intermedia entre la planificación y los drivers de cada máquina.
//!
//! Los generadores (líneas, rellenos, imágenes) producen un `Toolpath`; los drivers
//! (GRBL, Ruida, galvo) solo saben traducir un `Toolpath` a su protocolo.

use crate::{Job, Point};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Move {
    /// Desplazamiento rápido con el láser apagado (G0 en GRBL).
    Travel { to: Point },
    /// Movimiento a la velocidad del bloque con el láser a `power_pct` (0-100).
    /// Con potencia 0 sirve para overscan: el cabezal acelera sin disparar.
    Cut { to: Point, power_pct: f64 },
    /// Pausa de `ms` milisegundos en `to` (que es donde ya está la cabeza) con el láser a
    /// `power_pct` (0 = apagado). Sirve para el modo punto, el corte pasante y las pausas de
    /// inicio y fin de trazo. No mueve la cabeza: `to` solo existe para que `Move::to` siga
    /// dando siempre un punto (cajas, origen de la cama, espejo del rotativo y estimación de
    /// tiempo lo usan sin tratar la pausa aparte), y los drivers lo ignoran como movimiento.
    Dwell { to: Point, ms: f64, power_pct: f64 },
}

impl Move {
    /// Punto donde termina el movimiento (en `Dwell`, donde se queda la cabeza).
    pub fn to(&self) -> Point {
        match *self {
            Move::Travel { to } | Move::Cut { to, .. } | Move::Dwell { to, .. } => to,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PowerMode {
    /// La controladora escala la potencia con la velocidad real (M4 en GRBL).
    #[default]
    Dynamic,
    /// Potencia fija aunque el cabezal frene (M3 en GRBL).
    Constant,
}

/// Movimientos que comparten velocidad y ajustes, normalmente una capa o una pasada.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ToolpathBlock {
    /// Texto para comentarios en la salida y para la vista previa.
    pub label: String,
    pub speed_mm_min: f64,
    pub air_assist: bool,
    pub power_mode: PowerMode,
    pub moves: Vec<Move>,
    /// Posición Z (mm, 0 = foco, positivo = hacia dentro del material) al empezar el bloque;
    /// `None` = no mover Z. Solo en máquinas con eje Z.
    #[serde(default)]
    pub z_mm: Option<f64>,
    /// Parámetros de pulso y retardos para máquinas galvo; `None` en máquinas de pórtico (GRBL,
    /// Ruida, K40 y el simulador los ignoran).
    #[serde(default)]
    pub galvo: Option<GalvoParams>,
}

/// Parámetros de un bloque para el driver galvo (ver `docs/hardware/galvo.md`, sección 8.2).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct GalvoParams {
    pub frequency_khz: f64,
    pub pulse_width_ns: Option<u32>,
    /// Retardos en µs: encendido, apagado, final y polígono.
    pub laser_on_us: f64,
    pub laser_off_us: f64,
    pub end_us: f64,
    pub polygon_us: f64,
    /// Saltos: mm/s, retardos mín/máx (µs) y distancia límite (mm).
    pub jump_speed: f64,
    pub jump_delay_min_us: f64,
    pub jump_delay_max_us: f64,
    pub jump_distance_limit: f64,
    /// Repeticiones del bloque en la tarjeta.
    pub repeat: u32,
    pub pause_before_ms: f64,
    pub pause_after_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Toolpath {
    pub blocks: Vec<ToolpathBlock>,
}

impl Toolpath {
    /// Convierte un trabajo de líneas (modo Line) en movimientos, respetando pasadas
    /// y cerrando las trayectorias cerradas. Las trayectorias sin puntos se omiten.
    pub fn from_line_job(job: &Job) -> Self {
        let blocks = job
            .layers
            .iter()
            .map(|layer| {
                let st = &layer.settings;
                let mut moves = Vec::new();
                for _ in 0..st.passes {
                    for path in layer.paths.iter().filter(|p| !p.points.is_empty()) {
                        moves.push(Move::Travel { to: path.start() });
                        let rest = path.points[1..]
                            .iter()
                            .chain(path.closed.then_some(&path.points[0]));
                        moves.extend(rest.map(|&to| Move::Cut {
                            to,
                            power_pct: st.power_pct,
                        }));
                    }
                }
                ToolpathBlock {
                    label: format!(
                        "Capa {} velocidad={} potencia={}% pasadas={}",
                        layer.color, st.speed_mm_min, st.power_pct, st.passes
                    ),
                    speed_mm_min: st.speed_mm_min,
                    air_assist: st.air_assist,
                    power_mode: PowerMode::Dynamic,
                    moves,
                    z_mm: None,
                    galvo: None,
                }
            })
            .collect();
        Toolpath { blocks }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CutSettings, Layer, Polyline, Rgb};

    #[test]
    fn line_job_closes_paths_and_repeats_passes() {
        let job = Job {
            layers: vec![Layer {
                color: Rgb(0, 0, 0),
                settings: CutSettings {
                    passes: 2,
                    ..Default::default()
                },
                paths: vec![Polyline {
                    points: vec![
                        Point::new(0.0, 0.0),
                        Point::new(10.0, 0.0),
                        Point::new(10.0, 10.0),
                    ],
                    closed: true,
                }],
            }],
        };
        let tp = Toolpath::from_line_job(&job);
        let moves = &tp.blocks[0].moves;
        assert_eq!(moves.len(), 8);
        assert_eq!(
            moves[0],
            Move::Travel {
                to: Point::new(0.0, 0.0)
            }
        );
        assert_eq!(moves[3].to(), Point::new(0.0, 0.0));
    }

    #[test]
    fn empty_paths_are_skipped() {
        let job = Job {
            layers: vec![Layer {
                color: Rgb(0, 0, 0),
                settings: CutSettings::default(),
                paths: vec![Polyline {
                    points: vec![],
                    closed: true,
                }],
            }],
        };
        assert!(Toolpath::from_line_job(&job).blocks[0].moves.is_empty());
    }
}
