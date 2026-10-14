//! Pruebas del codificador, agrupadas por tema.

mod basic;
mod edges;
mod moves;
mod random;

use super::*;
use crate::decode::{Event, MoveMode, decode_stream};
use tlaku_core::toolpath::ToolpathBlock;

fn cut(x: f64, y: f64) -> Move {
    Move::Cut {
        to: Point::new(x, y),
        power_pct: 50.0,
    }
}

fn travel(x: f64, y: f64) -> Move {
    Move::Travel {
        to: Point::new(x, y),
    }
}

fn block(speed_mm_min: f64, moves: Vec<Move>) -> ToolpathBlock {
    ToolpathBlock {
        label: "t".into(),
        speed_mm_min,
        moves,
        ..Default::default()
    }
}

fn tp(blocks: Vec<ToolpathBlock>) -> Toolpath {
    Toolpath { blocks }
}

/// Cama de 300 x 200 con el home arriba a la izquierda; el punto (x, y) de Tlaku queda en
/// X = x, Y = 200 - y de la placa.
fn cfg() -> EncodeConfig {
    EncodeConfig::default()
}

fn s(p: &Program) -> String {
    String::from_utf8(p.stream.clone()).unwrap()
}

fn dist_point_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let l2 = abx * abx + aby * aby;
    let t = if l2 == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * abx + (p.1 - a.1) * aby) / l2).clamp(0.0, 1.0)
    };
    let (cx, cy) = (a.0 + t * abx, a.1 + t * aby);
    ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt()
}
