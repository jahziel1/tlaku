//! Resolución de un contenedor: preparación de las piezas (compartida entre hojas), tablas de
//! ángulos por pieza y la búsqueda sobre una hoja.

use crate::engine::{Ctl, Engine, Gene, Solution};
use crate::prep::{self, PieceType};
use crate::search::{Limits, Step, search};
use crate::{
    BadOption, MAX_INSTANCES, NestContainer, NestError, NestOptions, NestPiece, Outcome,
    PieceRotation, Placement, Rotations, check_options,
};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

/// Piezas ya preparadas (contorno inflado y partes convexas) y la tabla de ángulos.
pub struct Prepared {
    pub pieces: Vec<PieceType>,
    /// Todos los ángulos que usa algún giro, normalizados a `[0, 360)`.
    pub angles: Vec<f64>,
    /// Por pieza: índices de `angles` permitidos.
    pub piece_angles: Vec<Vec<usize>>,
}

fn global_angles(r: Rotations) -> Result<Vec<f64>, NestError> {
    match r {
        Rotations::Fixed => Ok(vec![0.0]),
        Rotations::Quarter => Ok(vec![0.0, 90.0, 180.0, 270.0]),
        Rotations::Step(s) => {
            if !s.is_finite() || !(1.0..=180.0).contains(&s) {
                return Err(NestError::InvalidOption(BadOption::RotationStep));
            }
            let n = (360.0 / s).floor() as usize;
            Ok((0..n).map(|i| i as f64 * s).collect())
        }
        Rotations::Free => Ok((0..72).map(|i| i as f64 * 5.0).collect()),
    }
}

/// Índice de `angle` en `angles` (con tolerancia de 0,01°); lo agrega si no está.
fn angle_index(angles: &mut Vec<f64>, angle: f64) -> usize {
    let a = angle.rem_euclid(360.0);
    // 359,999° y 0° son el mismo ángulo.
    let a = if (360.0 - a) < 0.005 { 0.0 } else { a };
    if let Some(i) = angles.iter().position(|&x| (x - a).abs() < 0.005) {
        return i;
    }
    angles.push(a);
    angles.len() - 1
}

/// Prepara las piezas y las tablas de ángulos. Las piezas sin copias no participan, pero
/// conservan su índice.
pub fn prepare(pieces: &[NestPiece], o: &NestOptions) -> Result<Prepared, NestError> {
    let mut angles = global_angles(o.rotations)?;
    let n_global = angles.len();
    let total: usize = pieces.iter().map(|p| p.copies as usize).sum();
    if total == 0 {
        return Err(NestError::NoPieces);
    }
    if total > MAX_INSTANCES {
        return Err(NestError::TooMany(total));
    }
    let mut piece_angles = Vec::with_capacity(pieces.len());
    for p in pieces {
        piece_angles.push(match &p.rotation {
            PieceRotation::Inherit => (0..n_global).collect(),
            PieceRotation::Fixed => vec![angle_index(&mut angles, 0.0)],
            PieceRotation::Only(list) => {
                if list.iter().any(|a| !a.is_finite()) {
                    return Err(NestError::InvalidOption(BadOption::PieceAngle));
                }
                let mut idx: Vec<usize> =
                    list.iter().map(|&a| angle_index(&mut angles, a)).collect();
                idx.sort_unstable();
                idx.dedup();
                if idx.is_empty() {
                    idx.push(angle_index(&mut angles, 0.0));
                }
                idx
            }
        });
    }
    let mut prepared = Vec::with_capacity(pieces.len());
    for (i, p) in pieces.iter().enumerate() {
        match prep::prepare_piece(&p.region, o.spacing, o.use_holes) {
            Some(pt) => prepared.push(pt),
            None if p.copies == 0 => prepared.push(prep::empty_piece()),
            None => return Err(NestError::EmptyPiece(i)),
        }
    }
    Ok(Prepared {
        pieces: prepared,
        angles,
        piece_angles,
    })
}

/// Todas las copias `(pieza, copia)` en orden de índice.
pub fn all_genes(pieces: &[NestPiece]) -> Vec<(usize, u32)> {
    let mut out = Vec::new();
    for (i, p) in pieces.iter().enumerate() {
        for c in 0..p.copies {
            out.push((i, c));
        }
    }
    out
}

pub fn make_container(
    c: &NestContainer,
    o: &NestOptions,
) -> Result<crate::prep::Container, NestError> {
    match c {
        NestContainer::Rect { width, height } => {
            prep::prepare_container(Some((*width, *height)), &[], o.spacing, o.margin)
        }
        NestContainer::Region(paths) => prep::prepare_container(None, paths, o.spacing, o.margin),
    }
    .ok_or(NestError::InvalidContainer)
}

/// Resuelve un contenedor con las copias `genes`. `budget` es el tiempo de esta hoja y `started`
/// el instante desde el que se cuenta `elapsed`.
#[allow(clippy::too_many_arguments)]
pub fn solve_sheet(
    prepared: &Prepared,
    genes: &[(usize, u32)],
    container: &NestContainer,
    o: &NestOptions,
    stop: &AtomicBool,
    started: Instant,
    budget: Duration,
    progress: &mut dyn FnMut(&Outcome),
) -> Result<Outcome, NestError> {
    check_options(o)?;
    let container = make_container(container, o)?;
    let container_area = container.net_area;
    let mut engine = Engine::new(
        prepared.pieces.clone(),
        container,
        prepared.angles.clone(),
        prepared.piece_angles.clone(),
        o.gravity,
        o.use_holes,
        o.allow_mirror,
    );

    // Orden inicial: de mayor a menor área; a igualdad, por índice (estable y reproducible).
    let mut start: Vec<Gene> = genes
        .iter()
        .map(|&(piece, copy)| Gene {
            piece,
            copy,
            rot: None,
            mirror: false,
        })
        .collect();
    start.sort_by(|a, b| {
        let (pa, pb) = (&engine.pieces[a.piece], &engine.pieces[b.piece]);
        pb.inflated_area
            .total_cmp(&pa.inflated_area)
            .then(a.piece.cmp(&b.piece))
            .then(a.copy.cmp(&b.copy))
    });

    let sheet_start = Instant::now();
    let ctl = Ctl {
        stop,
        deadline: sheet_start + budget,
    };
    let limits = Limits {
        max_evaluations: o.max_evaluations,
        started: sheet_start,
        time_limit_s: budget.as_secs_f64(),
    };

    let mut cached: Option<Outcome> = None;
    let mut revision = 0u64;
    let found = {
        // La conversión a `Outcome` solo se hace cuando hay mejora, para no copiar cientos de
        // transformaciones en cada evaluación.
        let mut observer = |eng: &Engine, step: &Step| {
            if step.improved || cached.is_none() {
                revision += 1;
                let mut out = build_outcome(eng, step.solution, container_area, started);
                out.revision = revision;
                cached = Some(out);
            }
            if let Some(c) = cached.as_mut() {
                c.evaluations = step.evaluations;
                c.elapsed = started.elapsed();
                progress(c);
            }
        };
        search(&mut engine, start, &ctl, &limits, o.seed, &mut observer)
    };
    let mut out = build_outcome(&engine, &found.solution, container_area, started);
    out.evaluations = found.evaluations;
    out.revision = revision;
    out.interrupted = found.interrupted;
    Ok(out)
}

fn build_outcome(
    engine: &Engine,
    sol: &Solution,
    container_area: f64,
    started: Instant,
) -> Outcome {
    let placements = sol
        .placed
        .iter()
        .map(|p| {
            let v = engine.variant(p.variant);
            Placement {
                piece: p.piece,
                copy: p.copy,
                rotation_deg: v.angle,
                mirror: v.mirror,
                transform: engine.transform(p.piece, p.variant, p.pos),
            }
        })
        .collect();
    let mut unplaced = sol.unplaced.clone();
    unplaced.sort_unstable();
    Outcome {
        placements,
        unplaced,
        utilization: if container_area > 0.0 {
            sol.placed_area / container_area * 100.0
        } else {
            0.0
        },
        placed_area: sol.placed_area,
        container_area,
        evaluations: 0,
        revision: 0,
        interrupted: false,
        elapsed: started.elapsed(),
    }
}
