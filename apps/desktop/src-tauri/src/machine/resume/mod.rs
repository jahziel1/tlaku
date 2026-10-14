//! Continuar un trabajo interrumpido (corte de luz, app cerrada, conexión perdida).
//!
//! # Cómo funciona
//!
//! 1. **Punto de control.** Al iniciar un trabajo de G-code, el trabajador de la conexión le pasa
//!    a un [`CheckpointWriter`] (hilo propio) el G-code exacto y los datos del trabajo. Mientras se
//!    envía, el bucle solo anota la última línea con `ok` y el último reporte de estado; el hilo
//!    escribe cada ~1 s `progress.json` de forma atómica. Al terminar bien o cancelar a propósito
//!    se borra; en cualquier otro final (error, alarma, conexión perdida, app cerrada) se queda.
//! 2. **Aviso.** `interrupted_job` lee la carpeta y resume el punto de control; la interfaz lo
//!    ofrece al abrir y cuando se pierde la conexión.
//! 3. **Punto de reinicio.** [`plan::locate`] busca, entre las líneas confirmadas, la que pasa por
//!    la última MPos (el `ok` solo dice que la línea entró al búfer) y [`plan::landing`] retrocede
//!    con prudencia sin cruzar un `G0`; en filas de imagen, hasta el principio de la fila.
//! 4. **Reanudar.** [`plan::build_program`] antepone un preámbulo (láser apagado, `G0` hasta el
//!    punto, y solo entonces `M3`/`M4` con su `S` y `F`) al resto del G-code. El trabajador lo
//!    envía por el camino de siempre, con el preflight de siempre.
//!
//! Un trabajo que es a su vez una reanudación guarda igualmente el G-code **original** y anota en
//! `meta.json` ([`store::ResumeRun`]) cuántas líneas de preámbulo llevó y desde qué línea del
//! original siguió, de modo que se puede interrumpir y reanudar cuantas veces haga falta.

pub mod plan;
pub mod start;
pub mod store;
pub mod walker;
pub mod writer;

use plan::{Estimate, Landing, PlanOptions};
use std::sync::Arc;
use std::time::Duration;
use store::{CheckpointMeta, CheckpointStore, Loaded, OriginInfo, ResumeRun, VERSION, now_ms};
use tlaku_doc::{
    DeviceProfile, InterruptedJob, JobOptions, ResumeKind, ResumePoint, ResumeStart, ResumeView,
};
use tlaku_i18n::tr;
use walker::{Parsed, StepKind};

pub use writer::{CheckpointWriter, Finish, StatusSnap};

/// Cada cuánto se guarda el avance. Un cuarto de segundo acota lo que se repite al reanudar (unos
/// 12 mm a 3000 mm/min) y escribe menos de 2 KB por segundo, desde su propio hilo.
pub const CHECKPOINT_INTERVAL: Duration = Duration::from_millis(250);

/// Carpeta del punto de control dentro de la carpeta de datos de la app.
pub const DIR_NAME: &str = "interrupted-job";

/// Lo que el trabajador necesita para llevar el punto de control de un trabajo.
#[derive(Debug, Clone)]
pub struct CheckpointPlan {
    pub store: Arc<CheckpointStore>,
    pub meta: CheckpointMeta,
    /// El G-code original, si lo que se envía no es el original (una reanudación).
    pub original: Option<Arc<Vec<String>>>,
}

fn new_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    format!(
        "{:x}-{:x}-{:x}",
        now_ms(),
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    )
}

fn kind_of(device: &DeviceProfile) -> ResumeKind {
    use tlaku_doc::DeviceKind;
    match device.kind {
        DeviceKind::Ruida => ResumeKind::Ruida,
        DeviceKind::LhyMicro => ResumeKind::K40,
        DeviceKind::Galvo => ResumeKind::Galvo,
        _ => ResumeKind::Gcode,
    }
}

fn origin_of(options: &JobOptions, head: Option<[f64; 2]>) -> OriginInfo {
    OriginInfo {
        start_from: options.start_from,
        job_origin: options.job_origin,
        user_origin: options.user_origin,
        head,
    }
}

/// Plan del punto de control de un trabajo de G-code que se está por enviar. `None` si el perfil
/// no se podría reanudar (dialecto sin `S` en línea, `G0` que no apaga el láser).
pub fn plan_for_job(
    store: Arc<CheckpointStore>,
    device: &DeviceProfile,
    options: &JobOptions,
    document: Option<String>,
    head: Option<[f64; 2]>,
    lines: &[String],
) -> Option<CheckpointPlan> {
    if kind_of(device) != ResumeKind::Gcode || !plan::dialect_supported(&device.dialect) {
        return None;
    }
    Some(CheckpointPlan {
        store,
        meta: CheckpointMeta {
            version: VERSION,
            id: new_id(),
            kind: ResumeKind::Gcode,
            started_at_ms: now_ms(),
            document,
            device: device.clone(),
            port: String::new(),
            origin: Some(origin_of(options, head)),
            total_lines: u32::try_from(plan::sendable_count(lines)).unwrap_or(u32::MAX),
            resume: None,
        },
        original: None,
    })
}

/// Marcador de un trabajo de Ruida, K40 o galvo: sin G-code, solo para avisar al abrir la app.
pub fn write_marker(
    store: &CheckpointStore,
    device: &DeviceProfile,
    options: &JobOptions,
    document: Option<String>,
    port: &str,
) -> Result<(), String> {
    let meta = CheckpointMeta {
        version: VERSION,
        id: new_id(),
        kind: kind_of(device),
        started_at_ms: now_ms(),
        document,
        device: device.clone(),
        port: port.to_string(),
        origin: Some(origin_of(options, None)),
        total_lines: 0,
        resume: None,
    };
    store.begin(&meta, None)
}

/// Un evento final del trabajo: borra el marcador de las máquinas que no se reanudan (el de
/// G-code lo lleva el trabajador). Devuelve si borró algo.
pub fn clear_marker_on_end(store: &CheckpointStore) -> bool {
    match store.load() {
        Some(l) if l.meta.kind != ResumeKind::Gcode => {
            store.clear();
            true
        }
        _ => false,
    }
}

/// Se conectó otra máquina: el punto de control de la anterior ya no se puede reanudar con esta
/// y se borra. Devuelve si lo borró (con la misma máquina no hace nada).
pub fn forget_if_other_machine(store: &CheckpointStore, device_id: &str) -> bool {
    match store.load() {
        Some(l) if l.meta.device.id != device_id => {
            store.clear();
            true
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Resumen para el aviso
// ---------------------------------------------------------------------------

/// Líneas enviables del original que ya tienen `ok`.
fn done_in_original(meta: &CheckpointMeta, acked: u32) -> u32 {
    let n = match &meta.resume {
        None => acked,
        Some(r) => r.tail_sendable_before + acked.saturating_sub(r.preamble_sendable),
    };
    n.min(meta.total_lines)
}

/// Última línea del G-code original con `ok`, si la hay.
fn last_original_line(l: &Loaded) -> Option<u32> {
    let p = l.progress.as_ref()?;
    let sent = p.sent_line?;
    match &l.meta.resume {
        None => Some(sent),
        Some(r) => sent
            .checked_sub(r.preamble_len)
            .map(|k| r.tail_first.saturating_add(k)),
    }
}

/// Resume el punto de control para la interfaz.
pub fn summarize(l: &Loaded) -> InterruptedJob {
    let m = &l.meta;
    let p = l.progress.as_ref();
    let done = p.map_or(0, |p| done_in_original(m, p.acked));
    let total = m.total_lines;
    let percent = if total == 0 {
        0.0
    } else {
        (f64::from(done) / f64::from(total) * 100.0).clamp(0.0, 100.0)
    };
    let (can_resume, note) = match m.kind {
        ResumeKind::Gcode if !plan::dialect_supported(&m.device.dialect) => {
            (false, Some(tr!("desktop.resume.note_dialect")))
        }
        ResumeKind::Gcode if p.is_none() => (true, Some(tr!("desktop.resume.note_no_progress"))),
        ResumeKind::Gcode => (true, None),
        ResumeKind::Ruida => (false, Some(tr!("desktop.resume.note_ruida"))),
        ResumeKind::K40 => (false, Some(tr!("desktop.resume.note_k40"))),
        ResumeKind::Galvo => (false, Some(tr!("desktop.resume.note_galvo"))),
    };
    InterruptedJob {
        id: m.id.clone(),
        kind: m.kind,
        started_at_ms: m.started_at_ms,
        updated_at_ms: p.map_or(m.started_at_ms, |p| p.t_ms),
        device_id: m.device.id.clone(),
        device_name: m.device.name.clone(),
        device_kind: m.device.kind,
        port: m.port.clone(),
        document: m.document.clone(),
        total_lines: total,
        done_lines: done,
        percent,
        progress_known: p.is_some(),
        mpos: p.and_then(|p| p.mpos),
        modal: p.map(|p| p.modal.clone()),
        can_resume,
        note,
        resumed: m.resume.is_some(),
    }
}

// ---------------------------------------------------------------------------
// Punto de reinicio
// ---------------------------------------------------------------------------

/// El punto que se propone y cómo se llegó a él.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    pub line: u32,
    pub step: Option<usize>,
    pub at: Option<[f64; 2]>,
    pub exec_line: u32,
    pub distance_mm: f64,
    pub mismatch: bool,
    pub row: bool,
}

/// Propone dónde reanudar: localiza la línea real y retrocede con prudencia.
pub fn suggest(parsed: &Parsed, l: &Loaded, opts: &PlanOptions) -> Suggestion {
    let base = l.meta.resume.as_ref().map_or(0, |r| r.tail_first);
    let floor = parsed.step_at_or_after(base);
    let from_base = |mismatch: bool| Suggestion {
        line: base,
        step: (floor < parsed.steps.len()).then_some(floor),
        at: None,
        exec_line: base,
        distance_mm: 0.0,
        mismatch,
        row: false,
    };
    let Some(last) = last_original_line(l) else {
        // Sin avance legible hay que elegir a mano; con avance 0, se vuelve a empezar.
        return from_base(l.progress.is_none());
    };
    let p = l.progress.as_ref();
    let used = |cap: Option<u32>, free: Option<u32>| match (cap, free) {
        (Some(c), Some(f)) => c.saturating_sub(f),
        _ => 0,
    };
    let window = plan::window_for(
        used(
            p.and_then(|p| p.planner_cap),
            p.and_then(|p| p.planner_free),
        ),
        used(p.and_then(|p| p.rx_cap), p.and_then(|p| p.rx_free)),
    );
    let wpos = p.and_then(|p| {
        p.wpos
            .or_else(|| Some([p.mpos?[0] - p.wco?[0], p.mpos?[1] - p.wco?[1]]))
    });
    let Some(est) = plan::locate(parsed, last, wpos, window, floor) else {
        return from_base(false);
    };
    let Landing { step, at, row } = plan::landing(parsed, &est, opts, floor);
    let Estimate {
        step: exec,
        distance_mm,
        mismatch,
        ..
    } = est;
    Suggestion {
        line: parsed.steps[step].line,
        step: Some(step),
        at,
        exec_line: parsed.steps[exec].line,
        distance_mm,
        mismatch,
        row,
    }
}

/// Tope de movimientos que se mandan a la interfaz para dibujar; por encima se toma uno de cada
/// varios (el punto sugerido y el último confirmado siempre van).
const MAX_VIEW_MOVES: usize = 400_000;

/// El recorrido guardado y el punto sugerido, para el diálogo de reanudación.
pub fn view(store: &CheckpointStore, l: &Loaded, opts: &PlanOptions) -> Result<ResumeView, String> {
    let lines = store.read_gcode()?;
    let parsed = Parsed::parse(&lines);
    let job = summarize(l);
    let sug = suggest(&parsed, l, opts);
    let s_max = l.meta.device.s_max.max(1.0);
    let last_step = last_original_line(l)
        .map(|ln| parsed.step_at_or_after(ln.saturating_add(1)))
        .filter(|&n| n > 0)
        .map(|n| n - 1);
    let stride = parsed.steps.len().div_ceil(MAX_VIEW_MOVES).max(1);
    let keep = |i: usize| {
        stride == 1 || i.is_multiple_of(stride) || Some(i) == sug.step || Some(i) == last_step
    };
    let mut moves: Vec<f32> = Vec::new();
    let mut move_lines: Vec<u32> = Vec::new();
    let mut kept: Vec<usize> = Vec::new();
    if let Some(first) = parsed.steps.first() {
        moves.extend([0.0, first.from[0] as f32, first.from[1] as f32, 0.0]);
        move_lines.push(first.line);
    }
    for (i, s) in parsed.steps.iter().enumerate() {
        if !keep(i) {
            continue;
        }
        let power = if s.kind == StepKind::Cut {
            (f64::from(s.s) / s_max * 100.0).clamp(0.0, 100.0) as f32
        } else {
            0.0
        };
        moves.extend([
            if s.kind == StepKind::Cut { 1.0 } else { 0.0 },
            s.to[0] as f32,
            s.to[1] as f32,
            power,
        ]);
        move_lines.push(s.line);
        kept.push(i);
    }
    let move_of = |step: Option<usize>| -> u32 {
        step.map_or(0, |s| {
            kept.partition_point(|&k| k < s)
                .min(kept.len().saturating_sub(1))
                .saturating_add(1)
                .try_into()
                .unwrap_or(0)
        })
    };
    let total = f64::from(l.meta.total_lines.max(1));
    let percent_at = |line: u32| (parsed.sendable_before(line) as f64 / total * 100.0).min(100.0);
    let z_delta = plan::z_delta(&lines, sug.exec_line, sug.line as usize);
    let mut warnings = Vec::new();
    if parsed.flags.offsets || parsed.flags.machine_moves {
        warnings.push(tr!("desktop.resume.warn_offsets"));
    }
    if sug.mismatch && l.progress.is_some() {
        warnings.push(tr!("desktop.resume.warn_mismatch"));
    }
    if l.progress.is_none() {
        warnings.push(tr!("desktop.resume.warn_no_progress"));
    }
    if z_delta.abs() > 1e-6 && l.meta.device.z_axis {
        warnings.push(tr!("desktop.resume.warn_z", dz = format!("{z_delta:.3}")));
    }
    Ok(ResumeView {
        job,
        moves,
        move_lines,
        bounds: parsed.bounds().unwrap_or([[0.0; 2]; 2]),
        last_acked_move: move_of(last_step),
        suggested: ResumePoint {
            line: sug.line,
            move_index: move_of(sug.step),
            at: sug.at,
            executing_line: sug.exec_line,
            distance_mm: sug.distance_mm,
            mismatch: sug.mismatch,
            row_restart: sug.row,
            percent: percent_at(sug.line),
            z_delta,
        },
        warnings,
    })
}

// ---------------------------------------------------------------------------
// Reanudar
// ---------------------------------------------------------------------------

/// Lo que se envía para reanudar y el plan de su propio punto de control.
pub struct ResumeJob {
    pub lines: Vec<String>,
    pub plan: CheckpointPlan,
    pub device: DeviceProfile,
}

/// Arma la reanudación en `args.line` a partir del punto de control guardado.
pub fn prepare(
    store: Arc<CheckpointStore>,
    l: &Loaded,
    args: &ResumeStart,
) -> Result<ResumeJob, String> {
    if l.meta.kind != ResumeKind::Gcode {
        return Err(tr!("desktop.resume.not_resumable"));
    }
    let lines = store.read_gcode()?;
    let parsed = Parsed::parse(&lines);
    if parsed.flags.offsets || parsed.flags.machine_moves {
        return Err(tr!("desktop.resume.offsets_unsupported"));
    }
    let start = args.line as usize;
    let device = l.meta.device.clone();
    let sug = suggest(&parsed, l, &PlanOptions::default());
    let z = if sug.mismatch {
        0.0
    } else {
        plan::z_delta(&lines, sug.exec_line, start)
    };
    let program = plan::build_program(&lines, start, args.at, z, &device.dialect, device.z_axis)?;
    let preamble_sendable = plan::sendable_count(&program.lines[..program.preamble_len]);
    let run = ResumeRun {
        tail_first: args.line,
        preamble_len: u32::try_from(program.preamble_len).unwrap_or(u32::MAX),
        preamble_sendable: u32::try_from(preamble_sendable).unwrap_or(u32::MAX),
        tail_sendable_before: u32::try_from(parsed.sendable_before(args.line)).unwrap_or(u32::MAX),
        from_id: l.meta.id.clone(),
    };
    let mut meta = l.meta.clone();
    meta.id = new_id();
    meta.started_at_ms = now_ms();
    meta.resume = Some(run);
    Ok(ResumeJob {
        lines: program.lines,
        plan: CheckpointPlan {
            store,
            meta,
            original: Some(Arc::new(lines)),
        },
        device,
    })
}

#[cfg(test)]
mod tests;
