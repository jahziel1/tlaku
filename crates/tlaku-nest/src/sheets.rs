//! Varias hojas: cuando las piezas no caben en una, se continúa en la siguiente.
//!
//! Se llenan en orden las hojas de la lista (por ejemplo, retazos guardados) y, cuando se acaban,
//! se agregan hojas nuevas iguales a `refill`. Cada hoja es un problema de acomodo independiente
//! (mismo motor, sin solapes ni salidas del contorno); la búsqueda de cada una prioriza colocar
//! la mayor área posible, así que las hojas intermedias salen llenas y la última lo que sobra.
//! Una hoja de la lista donde no cabe ninguna pieza se salta sin gastar tiempo y no cuenta como
//! usada.

use crate::solve::{all_genes, make_container, prepare, solve_sheet};
use crate::{
    BadOption, NestContainer, NestError, NestOptions, NestPiece, Outcome, Placement, check_options,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Entrada de [`nest_sheets`].
#[derive(Debug, Clone)]
pub struct SheetsInput {
    pub pieces: Vec<NestPiece>,
    /// Hojas disponibles, en el orden en que se intentan (retazos antes que hojas nuevas).
    pub sheets: Vec<NestContainer>,
    /// Hoja nueva que se agrega cuando se acabaron las de la lista (`None` = no hay más).
    pub refill: Option<NestContainer>,
    /// Tope de hojas usadas (al menos 1).
    pub max_sheets: usize,
    /// El tope de tiempo es de todo el cálculo; `max_evaluations`, de cada hoja.
    pub options: NestOptions,
}

/// El acomodo de una hoja.
#[derive(Debug, Clone)]
pub struct SheetOutcome {
    /// Posición en `SheetsInput::sheets`; `sheets.len() + k` es la k-ésima hoja nueva (`refill`).
    pub index: usize,
    pub placements: Vec<Placement>,
    /// Área colocada entre área neta de la hoja, en por ciento.
    pub utilization: f64,
    pub placed_area: f64,
    pub container_area: f64,
}

#[derive(Debug, Clone)]
pub struct SheetsOutcome {
    /// Las hojas usadas, en orden. Una copia está en una sola.
    pub sheets: Vec<SheetOutcome>,
    /// `(pieza, copia)` de las que no cupieron en ninguna hoja.
    pub unplaced: Vec<(usize, u32)>,
    /// Área colocada entre área de las hojas usadas, en por ciento.
    pub utilization: f64,
    pub placed_area: f64,
    pub container_area: f64,
    pub evaluations: u64,
    /// Cambia solo cuando la solución es otra.
    pub revision: u64,
    pub interrupted: bool,
    pub elapsed: Duration,
}

impl SheetsOutcome {
    pub fn placed_count(&self) -> usize {
        self.sheets.iter().map(|s| s.placements.len()).sum()
    }

    fn recompute(&mut self) {
        self.placed_area = self.sheets.iter().map(|s| s.placed_area).sum();
        self.container_area = self.sheets.iter().map(|s| s.container_area).sum();
        self.utilization = if self.container_area > 0.0 {
            self.placed_area / self.container_area * 100.0
        } else {
            0.0
        };
    }
}

fn sheet_from(index: usize, o: &Outcome) -> SheetOutcome {
    SheetOutcome {
        index,
        placements: o.placements.clone(),
        utilization: o.utilization,
        placed_area: o.placed_area,
        container_area: o.container_area,
    }
}

/// Acomoda las piezas repartiéndolas en las hojas necesarias. `stop` interrumpe (devuelve lo
/// mejor hasta ahora); `progress` recibe el estado completo tras cada evaluación.
pub fn nest_sheets(
    input: &SheetsInput,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(&SheetsOutcome),
) -> Result<SheetsOutcome, NestError> {
    let started = Instant::now();
    let o = &input.options;
    check_options(o)?;
    if input.max_sheets == 0 {
        return Err(NestError::InvalidOption(BadOption::MaxSheets));
    }
    let prepared = prepare(&input.pieces, o)?;
    // Se validan todas las hojas antes de empezar.
    let mut areas: Vec<f64> = Vec::with_capacity(input.sheets.len());
    for c in &input.sheets {
        areas.push(make_container(c, o)?.net_area);
    }
    let refill_area = match &input.refill {
        Some(c) => Some(make_container(c, o)?.net_area),
        None => None,
    };
    if input.sheets.is_empty() && input.refill.is_none() {
        return Err(NestError::InvalidContainer);
    }

    let deadline = started + o.time_limit;
    let mut remaining = all_genes(&input.pieces);
    let mut snap = SheetsOutcome {
        sheets: Vec::new(),
        unplaced: remaining.clone(),
        utilization: 0.0,
        placed_area: 0.0,
        container_area: 0.0,
        evaluations: 0,
        revision: 0,
        interrupted: false,
        elapsed: Duration::ZERO,
    };
    let mut evaluations = 0u64;
    let mut next_list = 0usize;
    let mut fresh = 0usize;
    let max_sheets = input.max_sheets.max(1);

    while !remaining.is_empty() && snap.sheets.len() < max_sheets {
        if stop.load(Ordering::Relaxed) || Instant::now() >= deadline {
            snap.interrupted = true;
            break;
        }
        let (index, container, area, from_list) = if next_list < input.sheets.len() {
            (next_list, &input.sheets[next_list], areas[next_list], true)
        } else if let (Some(c), Some(a)) = (&input.refill, refill_area) {
            (input.sheets.len() + fresh, c, a, false)
        } else {
            break;
        };
        if from_list {
            next_list += 1;
        } else {
            fresh += 1;
        }

        // Tiempo de esta hoja: lo que queda entre las hojas que se calcula que faltan.
        let left = deadline.saturating_duration_since(Instant::now());
        let need: f64 = remaining
            .iter()
            .map(|&(p, _)| prepared.pieces[p].true_area)
            .sum();
        let sheets_left = max_sheets - snap.sheets.len();
        let est = ((need / (area * 0.75).max(1e-9)).ceil() as usize).clamp(1, sheets_left);
        let budget = (left / est as u32).max(left.min(Duration::from_millis(300)));

        let slot = snap.sheets.len();
        let base_evals = evaluations;
        let mut last_rev = 0u64;
        let out = {
            let snap_ref = &mut snap;
            let mut obs = |c: &Outcome| {
                if c.revision != last_rev {
                    last_rev = c.revision;
                    snap_ref.revision += 1;
                    if c.placements.is_empty() {
                        snap_ref.sheets.truncate(slot);
                    } else if snap_ref.sheets.len() > slot {
                        snap_ref.sheets[slot] = sheet_from(index, c);
                    } else {
                        snap_ref.sheets.push(sheet_from(index, c));
                    }
                    snap_ref.unplaced = c.unplaced.clone();
                    snap_ref.recompute();
                }
                snap_ref.evaluations = base_evals + c.evaluations;
                snap_ref.elapsed = started.elapsed();
                progress(snap_ref);
            };
            solve_sheet(
                &prepared, &remaining, container, o, stop, started, budget, &mut obs,
            )?
        };
        evaluations += out.evaluations;
        snap.evaluations = evaluations;
        if out.placements.is_empty() {
            snap.sheets.truncate(slot);
            if !from_list {
                // Una hoja nueva donde no cabe nada: lo que queda no cabe en ninguna.
                break;
            }
            continue;
        }
        let done = sheet_from(index, &out);
        if snap.sheets.len() > slot {
            snap.sheets[slot] = done;
        } else {
            snap.sheets.push(done);
        }
        remaining = out.unplaced;
        snap.unplaced = remaining.clone();
        snap.recompute();
        snap.revision += 1;
        if out.interrupted && (stop.load(Ordering::Relaxed) || Instant::now() >= deadline) {
            snap.interrupted = true;
            break;
        }
    }
    snap.unplaced = remaining;
    snap.unplaced.sort_unstable();
    snap.elapsed = started.elapsed();
    snap.evaluations = evaluations;
    snap.recompute();
    Ok(snap)
}
