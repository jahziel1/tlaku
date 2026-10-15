//! El flujo completo de «alinear con el cabezal» del lado del motor: recorre la rejilla, toma una
//! foto en cada punto, ajusta y, si la cámara solo ve una parte de la cama, afina la rejilla donde
//! se vio el cabezal. Lo usan las pruebas (máquina virtual + cámara simulada) y `hwtest` (máquina
//! y cámara reales). La interfaz hace lo mismo con su cámara web, paso a paso, con los comandos
//! `camera_head_align_*`.
//!
//! Nunca enciende el láser: el único movimiento que pide es [`AlignRig::move_to`], que es un
//! posicionamiento (`jog_to` apaga el láser antes de mover).

use super::*;
use tlaku_camera::{HeadAlignPlan, PlanOptions, plan_grid, plan_refine};

/// Quien mueve el cabezal y toma las fotos.
pub trait AlignRig {
    /// Lleva el láser a `mm` (documento), sin láser, y espera a que se detenga y se asiente.
    fn move_to(&mut self, mm: [f64; 2]) -> Result<(), String>;
    /// Foto actual (PNG o JPEG en base64), ya corregida de lente si hay calibración.
    fn capture(&mut self) -> Result<String, String>;
    /// Aviso de avance (opcional).
    fn event(&mut self, _event: &FlowEvent) {}
}

#[derive(Debug, Clone)]
pub enum FlowEvent {
    /// Empieza una ronda con estos puntos (la primera, la rejilla; las siguientes, la afinada).
    Round {
        round: usize,
        points: Vec<[f64; 2]>,
    },
    /// Foto `index` de `total` (de esta ronda) tomada con el cabezal en `mm`.
    Shot {
        index: usize,
        total: usize,
        mm: [f64; 2],
    },
    /// Resultado tras la ronda.
    Solved { round: usize, detected: usize },
}

#[derive(Debug, Clone)]
pub struct FlowOptions {
    pub plan: PlanOptions,
    pub config: HeadAlignConfig,
    /// Rondas de afinado como máximo tras la primera.
    pub max_rounds: usize,
    /// Puntos con el cabezal localizado con los que se da por buena la cobertura.
    pub target_points: usize,
    /// Puntos por lado de las rejillas afinadas.
    pub refine_grid: usize,
}

impl FlowOptions {
    pub fn new(plan: PlanOptions, config: HeadAlignConfig) -> Self {
        Self {
            plan,
            config,
            max_rounds: 3,
            target_points: 9,
            refine_grid: 3,
        }
    }
}

pub struct FlowResult {
    pub report: HeadAlignReport,
    pub frames: Vec<HeadAlignFrame>,
    pub rounds: usize,
}

fn shoot(
    rig: &mut dyn AlignRig,
    round: usize,
    positions: &[[f64; 2]],
    frames: &mut Vec<HeadAlignFrame>,
) -> Result<(), String> {
    rig.event(&FlowEvent::Round {
        round,
        points: positions.to_vec(),
    });
    for (i, p) in positions.iter().enumerate() {
        rig.move_to(*p)?;
        frames.push(HeadAlignFrame {
            png_base64: rig.capture()?,
            machine_mm: *p,
        });
        rig.event(&FlowEvent::Shot {
            index: i + 1,
            total: positions.len(),
            mm: *p,
        });
    }
    Ok(())
}

/// Posiciones de una rejilla más la referencia, sin repetir las ya visitadas.
fn fresh(plan: &HeadAlignPlan, frames: &[HeadAlignFrame]) -> Vec<[f64; 2]> {
    let mut all = plan.points.clone();
    all.push(plan.reference);
    all.retain(|p| {
        !frames
            .iter()
            .any(|f| (f.machine_mm[0] - p[0]).hypot(f.machine_mm[1] - p[1]) < 1.0)
    });
    all
}

/// ¿Se vio el cabezal en todo el borde de la rejilla? Si es así la cámara probablemente ve más allá
/// de ella y conviene probar una rejilla mayor.
fn edge_all_seen(plan: &HeadAlignPlan, seen: &[[f64; 2]]) -> bool {
    let a = plan.area;
    let on_edge = |p: &[f64; 2]| {
        [p[0] - a[0], a[2] - p[0], p[1] - a[1], a[3] - p[1]]
            .iter()
            .any(|d| d.abs() < 1.0)
    };
    plan.points.iter().filter(|p| on_edge(p)).all(|p| {
        seen.iter()
            .any(|s| (s[0] - p[0]).hypot(s[1] - p[1]) < 1.0)
    })
}

pub fn run_head_align_flow(
    rig: &mut dyn AlignRig,
    opts: &FlowOptions,
) -> Result<FlowResult, String> {
    let scout = plan_grid(&opts.plan)?;
    let mut config = opts.config.clone();
    config.safe_area = Some(scout.area);
    let mut frames: Vec<HeadAlignFrame> = Vec::new();
    shoot(rig, 0, &fresh(&scout, &frames), &mut frames)?;
    let mut report = run_head_align_solve(&frames, &config)?;
    rig.event(&FlowEvent::Solved {
        round: 0,
        detected: report.detected,
    });
    let mut round = 0;
    let mut last = scout.clone();
    // Se afina mientras falten puntos o la cámara siga viendo el cabezal en el borde de la rejilla
    // (puede ver más); `plan_refine` termina cuando ya no hay por dónde crecer.
    while round < opts.max_rounds
        && (report.detected < opts.target_points || edge_all_seen(&last, &report.seen_mm))
    {
        let Some(plan) = plan_refine(scout.area, &report.seen_mm, opts.refine_grid, 0.5, 60.0)
        else {
            break;
        };
        let positions = fresh(&plan, &frames);
        if positions.is_empty() {
            break;
        }
        round += 1;
        last = plan;
        shoot(rig, round, &positions, &mut frames)?;
        report = run_head_align_solve(&frames, &config)?;
        rig.event(&FlowEvent::Solved {
            round,
            detected: report.detected,
        });
    }
    Ok(FlowResult {
        report,
        frames,
        rounds: round,
    })
}
