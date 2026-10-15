//! Qué pasó con la máquina mientras la app no estaba: el `ok` de GRBL solo dice que la línea
//! entró a su búfer, así que si la app murió con el trabajo (o buena parte) ya entregado, la
//! máquina sigue sola hasta vaciarlo. Aquí se compara la MPos de ahora con la del último punto
//! de control y se decide si sigue en el mismo sitio, si siguió a mitad del trabajo, si llegó al
//! final (probablemente terminó) o si no se puede saber.
//!
//! También mide el avance por **posición** (longitud de grabado recorrida hasta la cabeza entre
//! la total), que es lo que se le muestra al usuario; las líneas con `ok` quedan como dato
//! interno.

use super::plan::{self, Estimate, ON_PATH_MM, TIE_MM};
use super::store::Loaded;
use super::walker::{Parsed, Step, StepKind};
use tlaku_doc::ResumeMachine;

/// Cuánto debe haberse movido la cabeza desde el último reporte guardado para contar como
/// «la máquina siguió sola» (mm). Menos es ruido del reporte.
pub const MOVED_MM: f64 = 0.5;
/// Si de grabado ya no queda más que esto después de la cabeza, el trabajo está terminado (mm).
pub const FINISHED_LEFT_MM: f64 = 0.5;

/// Pasos de más que se miran después del último entregado que consta en el punto de control: se
/// guarda cada ~250 ms y la app pudo entregar más líneas entre el último guardado y su muerte.
const FORWARD_SLACK_STEPS: usize = 512;

/// La máquina conectada ahora.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Now {
    /// Posición de máquina del último reporte.
    pub mpos: [f64; 2],
    /// Posición de trabajo del último reporte.
    pub wpos: [f64; 2],
    /// La posición es fiable: no hay alarma ni reinicio que la haya podido perder.
    pub valid: bool,
    /// `F` del estado del intérprete (`$G`), si se pudo leer. Una placa que se reinició (corte de
    /// luz) lo deja en 0; una que terminó el trabajo sola conserva el `F` del G-code.
    pub parser_feed: Option<f64>,
}

impl Now {
    /// Con la placa en reposo y sin alarma, ¿hay señales de que se reinició y la MPos es el 0
    /// de arranque y no la posición real de la cabeza? Una placa reiniciada marca MPos (0, 0) y
    /// `F0`; una que terminó el trabajo sola conserva el `F` del G-code. Si `$G` no contestó,
    /// una MPos justo en (0, 0) es indistinguible de un reinicio.
    fn looks_reset(&self, saved_feed: Option<f64>) -> bool {
        const ZERO: f64 = 0.01;
        match (self.parser_feed, saved_feed) {
            (Some(f), Some(was)) if was > 0.0 => f <= 0.0,
            (Some(_), _) => false,
            (None, _) => self.mpos[0].abs() < ZERO && self.mpos[1].abs() < ZERO,
        }
    }
}

/// El resultado de comparar el punto de control con la máquina.
#[derive(Debug, Clone, PartialEq)]
pub struct Assessment {
    pub machine: ResumeMachine,
    /// Dónde está la cabeza sobre el recorrido, según el último punto de control (con la MPos
    /// de ahora si la máquina siguió sola). `None` si no hay avance legible.
    pub position: Option<Estimate>,
    /// Primer paso que se puede elegir (en una reanudación, el primero del resto del trabajo).
    pub floor: usize,
    /// Líneas enviadas a la máquina que aún no había ejecutado al guardarse el punto de control.
    pub ahead_lines: u32,
    /// Lo mismo en mm de recorrido.
    pub ahead_mm: f64,
}

impl Assessment {
    pub fn is_finished(&self) -> bool {
        self.machine == ResumeMachine::ProbablyFinished
    }
}

/// Pasa una línea del programa enviado (con el preámbulo de una reanudación) a la del original.
fn to_original(l: &Loaded, line: u32) -> Option<u32> {
    match &l.meta.resume {
        None => Some(line),
        Some(r) => line
            .checked_sub(r.preamble_len)
            .map(|k| r.tail_first.saturating_add(k)),
    }
}

/// Última línea del G-code original con `ok`, si la hay.
pub fn last_acked_line(l: &Loaded) -> Option<u32> {
    to_original(l, l.progress.as_ref()?.sent_line?)
}

/// Última línea del G-code original entregada a la máquina (con o sin `ok`). Los puntos de
/// control viejos no la traen: vale la última con `ok`.
pub fn last_sent_line(l: &Loaded) -> Option<u32> {
    let p = l.progress.as_ref()?;
    p.sent_to_line
        .and_then(|s| to_original(l, s))
        .filter(|&s| Some(s) >= last_acked_line(l))
        .or_else(|| last_acked_line(l))
}

/// Dónde estaba la cabeza en el último punto de control: la línea confirmada que pasa por la
/// última MPos. Devuelve también el primer paso elegible.
pub fn saved_position(parsed: &Parsed, l: &Loaded) -> (Option<Estimate>, usize) {
    let base = l.meta.resume.as_ref().map_or(0, |r| r.tail_first);
    let floor = parsed.step_at_or_after(base);
    let (Some(last), Some(p)) = (last_acked_line(l), l.progress.as_ref()) else {
        return (None, floor);
    };
    let used = |cap: Option<u32>, free: Option<u32>| match (cap, free) {
        (Some(c), Some(f)) => c.saturating_sub(f),
        _ => 0,
    };
    let window = plan::window_for(
        used(p.planner_cap, p.planner_free),
        used(p.rx_cap, p.rx_free),
    );
    let wpos = p.wpos.or_else(|| {
        let (m, w) = (p.mpos?, p.wco?);
        Some([m[0] - w[0], m[1] - w[1]])
    });
    (plan::locate(parsed, last, wpos, window, floor), floor)
}

/// Mide el recorrido de grabado (los pasos que no son `G0`; si no hay ninguno, todos).
pub struct PathMeter<'a> {
    steps: &'a [Step],
    rapid_counts: bool,
    total: f64,
}

impl<'a> PathMeter<'a> {
    pub fn new(parsed: &'a Parsed) -> Self {
        let sum = |all: bool| -> f64 {
            parsed
                .steps
                .iter()
                .filter(|s| all || s.kind != StepKind::Rapid)
                .map(Step::length)
                .sum()
        };
        let engraving = sum(false);
        let rapid_counts = engraving <= f64::EPSILON;
        Self {
            steps: &parsed.steps,
            rapid_counts,
            total: if rapid_counts { sum(true) } else { engraving },
        }
    }

    fn weight(&self, s: &Step) -> f64 {
        if self.rapid_counts || s.kind != StepKind::Rapid {
            s.length()
        } else {
            0.0
        }
    }

    /// Recorrido hecho al llegar a la fracción `t` del paso `step`.
    fn done(&self, step: usize, t: f64) -> f64 {
        let step = step.min(self.steps.len());
        let before: f64 = self.steps[..step].iter().map(|s| self.weight(s)).sum();
        before
            + self
                .steps
                .get(step)
                .map_or(0.0, |s| self.weight(s) * t.clamp(0.0, 1.0))
    }

    /// 0 a 100: la parte del recorrido hecha al llegar a esa posición.
    pub fn percent(&self, step: usize, t: f64) -> f64 {
        if self.total <= f64::EPSILON {
            return 0.0;
        }
        (self.done(step, t) / self.total * 100.0).clamp(0.0, 100.0)
    }

    /// Grabado que queda después de esa posición (mm).
    fn engraving_left(&self, step: usize, t: f64) -> f64 {
        let cut = |s: &Step| {
            if s.kind == StepKind::Rapid {
                0.0
            } else {
                s.length()
            }
        };
        let here = self
            .steps
            .get(step)
            .map_or(0.0, |s| cut(s) * (1.0 - t.clamp(0.0, 1.0)));
        here + self.steps.iter().skip(step + 1).map(cut).sum::<f64>()
    }
}

/// Distancia de `p` al segmento y fracción del punto más cercano.
fn project(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> (f64, f64) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 <= f64::EPSILON {
        0.0
    } else {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
    };
    ((p[0] - (a[0] + t * dx)).hypot(p[1] - (a[1] + t * dy)), t)
}

/// Ubica `p` en el recorrido **sin retroceder** de `from`, hasta el último paso entregado que
/// consta (`last_sent`) más un margen: la máquina solo avanza. Si hay varios sitios a la misma distancia (un trazo
/// que vuelve a pasar por el mismo punto), el primero que no quede atrás de `from`.
pub fn locate_forward(
    parsed: &Parsed,
    last_sent: u32,
    p: [f64; 2],
    from: &Estimate,
) -> Option<Estimate> {
    let hi = (parsed.step_at_or_after(last_sent.saturating_add(1)) + FORWARD_SLACK_STEPS)
        .min(parsed.steps.len());
    let eligible: Vec<(usize, f64, f64)> = (from.step..hi)
        .filter_map(|k| {
            let s = &parsed.steps[k];
            let (d, t) = project(p, s.from, s.to);
            // En el paso donde estaba, solo vale de ahí en adelante.
            (k > from.step || t >= from.t - 1e-6).then_some((k, d, t))
        })
        .collect();
    let best = eligible
        .iter()
        .map(|&(_, d, _)| d)
        .fold(f64::INFINITY, f64::min);
    if best > ON_PATH_MM {
        return None;
    }
    let &(step, distance_mm, t) = eligible.iter().find(|&&(_, d, _)| d <= best + TIE_MM)?;
    Some(Estimate {
        step,
        t,
        distance_mm,
        mismatch: false,
    })
}

/// Lo que la máquina tenía en su memoria al guardarse el último punto de control: líneas y mm
/// entre la cabeza y el último paso entregado.
fn ahead(parsed: &Parsed, l: &Loaded, from: &Estimate) -> (u32, f64) {
    let Some(sent) = last_sent_line(l) else {
        return (0, 0.0);
    };
    let hi = parsed.step_at_or_after(sent.saturating_add(1));
    if from.step >= parsed.steps.len() {
        return (0, 0.0);
    }
    let first_line = parsed.steps[from.step].line;
    let lines = parsed
        .sendable_before(sent.saturating_add(1))
        .saturating_sub(parsed.sendable_before(first_line));
    let mm = if hi > from.step {
        parsed.steps[from.step].length() * (1.0 - from.t.clamp(0.0, 1.0))
            + parsed.steps[from.step + 1..hi]
                .iter()
                .map(Step::length)
                .sum::<f64>()
    } else {
        0.0
    };
    (u32::try_from(lines).unwrap_or(u32::MAX), mm)
}

/// Compara el punto de control con la máquina conectada (`now`; `None` = sin máquina).
pub fn assess(parsed: &Parsed, l: &Loaded, now: Option<&Now>) -> Assessment {
    let (saved, floor) = saved_position(parsed, l);
    let (ahead_lines, ahead_mm) = saved.as_ref().map_or((0, 0.0), |e| ahead(parsed, l, e));
    let done = |machine: ResumeMachine, position: Option<Estimate>| Assessment {
        machine,
        position,
        floor,
        ahead_lines,
        ahead_mm,
    };
    let (Some(now), Some(p)) = (now, l.progress.as_ref()) else {
        return done(ResumeMachine::NotChecked, saved);
    };
    // Sin posición fiable (alarma, reinicio) o sin dónde estaba, no hay con qué comparar.
    let (Some(est), Some(was), true) = (saved.as_ref(), p.mpos, now.valid) else {
        return done(ResumeMachine::Unknown, saved);
    };
    if est.mismatch {
        return done(ResumeMachine::Unknown, saved);
    }
    if (now.mpos[0] - was[0]).hypot(now.mpos[1] - was[1]) <= MOVED_MM {
        return done(ResumeMachine::Unmoved, saved);
    }
    // Un reinicio (corte de luz) deja la placa en reposo y en (0, 0) aunque la cabeza siga donde
    // estaba: esa MPos no es de la cabeza y no dice nada del trabajo.
    if now.looks_reset(p.modal.f) {
        return done(ResumeMachine::Unknown, saved);
    }
    // La máquina se movió: se ubica la MPos de ahora, hacia delante de donde estaba.
    let wpos = p
        .wco
        .map_or(now.wpos, |w| [now.mpos[0] - w[0], now.mpos[1] - w[1]]);
    let sent = last_sent_line(l).unwrap_or(0);
    let Some(here) = locate_forward(parsed, sent, wpos, est) else {
        return done(ResumeMachine::Unknown, saved);
    };
    let left = PathMeter::new(parsed).engraving_left(here.step, here.t);
    let machine = if left <= FINISHED_LEFT_MM {
        ResumeMachine::ProbablyFinished
    } else {
        ResumeMachine::Moved
    };
    done(machine, Some(here))
}
