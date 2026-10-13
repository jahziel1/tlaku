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
}

impl Move {
    pub fn to(&self) -> Point {
        match *self {
            Move::Travel { to } | Move::Cut { to, .. } => to,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolpathBlock {
    /// Texto para comentarios en la salida y para la vista previa.
    pub label: String,
    pub speed_mm_min: f64,
    pub air_assist: bool,
    pub power_mode: PowerMode,
    pub moves: Vec<Move>,
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
