//! Plan de capas: conversión de movimientos a µm y cajas del trabajo.

use super::{MAX_LAYERS, RdError, RuidaJobOptions, ZeroPower};
use crate::num::{self, NumError};
use tlaku_core::Point;
use tlaku_core::toolpath::{Move, Toolpath};

/// Un movimiento ya convertido a µm del archivo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Step {
    pub(super) cut: bool,
    pub(super) x: i64,
    pub(super) y: i64,
    /// Potencia cruda del corte (solo importa en los bloques de potencia variable).
    pub(super) power: u16,
}

pub(super) struct LayerPlan {
    /// Índice de capa en el archivo (0 a 127).
    pub(super) index: u8,
    pub(super) speed_um_s: u64,
    pub(super) power_max: u16,
    pub(super) power_min: u16,
    /// Los cortes no comparten potencia: se escribe `C7` antes de cada cambio.
    pub(super) variable: bool,
    pub(super) air: bool,
    pub(super) color: u64,
    pub(super) steps: Vec<Step>,
    pub(super) bbox: BBox,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct BBox {
    pub(super) min: (i64, i64),
    pub(super) max: (i64, i64),
}

impl BBox {
    pub(super) fn of(p: (i64, i64)) -> Self {
        BBox { min: p, max: p }
    }

    pub(super) fn add(&mut self, p: (i64, i64)) {
        self.min = (self.min.0.min(p.0), self.min.1.min(p.1));
        self.max = (self.max.0.max(p.0), self.max.1.max(p.1));
    }

    pub(super) fn union(&mut self, o: &BBox) {
        self.add(o.min);
        self.add(o.max);
    }
}

fn to_um(p: Point, opts: &RuidaJobOptions) -> Result<(i64, i64), NumError> {
    let (x, y) = opts.home.to_machine(p, opts.limits.bed_mm);
    Ok((
        num::mm_to_um(x + opts.offset_mm[0])?,
        num::mm_to_um(y + opts.offset_mm[1])?,
    ))
}

pub(super) fn plan_layers(
    tp: &Toolpath,
    opts: &RuidaJobOptions,
) -> Result<Vec<LayerPlan>, RdError> {
    let mut plans = Vec::new();
    // Última posición emitida (µm) para descartar movimientos de longitud cero.
    let mut last: Option<(i64, i64)> = None;
    for (bi, block) in tp.blocks.iter().enumerate() {
        let powers: Vec<f64> = block
            .moves
            .iter()
            .filter_map(|m| match m {
                Move::Cut { power_pct, .. } => Some(*power_pct),
                Move::Travel { .. } | Move::Dwell { .. } => None,
            })
            .collect();
        if powers.is_empty() {
            continue;
        }
        // PowerMode::Dynamic se trata como constante: la semántica de mín/máx está sin validar.
        let raws = powers
            .iter()
            .map(|&p| num::power_from_pct(p))
            .collect::<Result<Vec<_>, _>>()?;
        let (lo, hi) = (*raws.iter().min().unwrap(), *raws.iter().max().unwrap());
        let variable = lo != hi;
        let lopt = opts.layers.get(bi).cloned().unwrap_or_default();
        let power_min = if variable {
            // La capa deja que `C7` mande: mínimo 0 y máximo el mayor del bloque.
            0
        } else {
            match lopt.min_power_pct {
                Some(m) => {
                    let r = num::power_from_pct(m)?;
                    if r > hi {
                        return Err(RdError::MinPowerAboveMax { block: bi });
                    }
                    r
                }
                None => hi,
            }
        };
        if plans.len() >= MAX_LAYERS {
            return Err(RdError::TooManyLayers {
                layers: plans.len() + 1,
            });
        }
        let mut steps = Vec::new();
        let mut bbox: Option<BBox> = None;
        for m in &block.moves {
            // El RD que generamos no tiene pausa: se omite (la cabeza ya está en ese punto).
            if matches!(m, Move::Dwell { .. }) {
                continue;
            }
            let p = to_um(m.to(), opts)?;
            match &mut bbox {
                Some(b) => b.add(p),
                None => bbox = Some(BBox::of(p)),
            }
            if last == Some(p) {
                continue;
            }
            last = Some(p);
            let (cut, power) = match m {
                Move::Travel { .. } | Move::Dwell { .. } => (false, 0),
                Move::Cut { power_pct, .. } => {
                    let raw = num::power_from_pct(*power_pct)?;
                    let laser = !variable || raw > 0 || opts.zero_power == ZeroPower::Cut;
                    (laser, raw)
                }
            };
            steps.push(Step {
                cut,
                x: p.0,
                y: p.1,
                power,
            });
        }
        plans.push(LayerPlan {
            index: plans.len() as u8,
            speed_um_s: num::speed_from_mm_min(block.speed_mm_min)?,
            power_max: hi,
            power_min,
            variable,
            air: block.air_assist,
            color: num::rgb_to_color(lopt.color[0], lopt.color[1], lopt.color[2]),
            steps,
            bbox: bbox.expect("el bloque tiene al menos un corte"),
        });
    }
    if plans.is_empty() {
        return Err(RdError::EmptyJob);
    }
    Ok(plans)
}
