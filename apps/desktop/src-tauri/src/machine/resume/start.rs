//! Reanudar sobre una conexión viva: qué falta, las condiciones de seguridad y el envío.
//!
//! Para reanudar hacen falta TODAS estas condiciones (las comprueba [`resume_now`], no la
//! interfaz):
//!
//! - el punto de control es de un trabajo de G-code que se puede reanudar;
//! - la máquina está en reposo (no en pleno encuadre ni a mitad de un movimiento) y contestó a
//!   `$$`;
//! - la máquina tiene homing (`$22=1`) y se hizo `$H` desde que la placa se reinició o perdió la
//!   posición; si no tiene homing, el usuario confirmó que ni la cabeza ni el material se movieron
//!   y que el origen es el mismo;
//! - el usuario encuadró el trabajo completo y el material seguía en su lugar;
//! - la lista de comprobación de siempre está completa;
//! - el G-code resultante pasa el preflight de siempre (lo hace el trabajador al iniciar).

use super::assess::Now;
use super::store::{CheckpointStore, Loaded, now_ms};
use super::walker::Parsed;
use super::{plan, prepare};
use crate::machine::Handle;
use std::sync::Arc;
use tlaku_doc::{MachineState, ResumeKind, ResumeReadiness, ResumeStart};
use tlaku_grbl::link::LinkProfile;
use tlaku_i18n::tr;

/// `$22` de las líneas de `$$`.
pub fn homing_setting(lines: &[String]) -> Option<bool> {
    lines.iter().find_map(|l| {
        let v = l.trim().strip_prefix("$22=")?;
        let v: f64 = v.split_whitespace().next()?.parse().ok()?;
        Some(v != 0.0)
    })
}

/// Si la máquina tiene homing y si ya se hizo. Marlin y Smoothieware no contestan `$$` con un
/// ajuste: sin dato no se asume homing (`homing: None` con `settings_read: true`). Si la máquina
/// está ocupada (un encuadre, un movimiento) no se le pregunta nada y `settings_read` es falso:
/// no es lo mismo que «no tiene homing».
pub fn readiness(h: &Handle) -> ResumeReadiness {
    let state = fresh_state(h);
    let answer = if is_busy(state) {
        None
    } else {
        h.command("$$").ok()
    };
    ResumeReadiness {
        homing: answer.as_deref().and_then(homing_setting),
        settings_read: answer.is_some(),
        homed: h.is_homed(),
        state,
    }
}

/// Cuánto se espera un reporte de estado nuevo cuando el último dice «ocupada».
const STATE_REFRESH: std::time::Duration = std::time::Duration::from_millis(1500);

/// El estado de la máquina ahora. El último reporte puede ir atrasado: acaba de terminar un
/// `$H` (sigue diciendo `Home`) o acaba de lanzarse un encuadre (aún dice `Idle`). Un envío en
/// curso manda; si el reporte dice «ocupada» sin que haya envío, se pide otro y se espera un
/// poco a que cambie.
fn fresh_state(h: &Handle) -> MachineState {
    let reported = |h: &Handle| h.status_report().map_or(MachineState::Idle, |s| s.state);
    if h.is_streaming() {
        return MachineState::Run;
    }
    let mut state = reported(h);
    if is_busy(state) {
        let _ = h.realtime(tlaku_doc::RealtimeCommand::Status);
        let end = std::time::Instant::now() + STATE_REFRESH;
        while is_busy(state) && std::time::Instant::now() < end {
            std::thread::sleep(std::time::Duration::from_millis(25));
            state = reported(h);
        }
    }
    if h.is_streaming() {
        return MachineState::Run;
    }
    state
}

/// Nombre del estado en el idioma activo, el mismo que muestra la barra de estado (`machine.state.*`).
fn state_label(state: MachineState) -> String {
    match state {
        MachineState::Idle => tr!("desktop.resume.state.idle"),
        MachineState::Run => tr!("desktop.resume.state.run"),
        MachineState::Hold => tr!("desktop.resume.state.hold"),
        MachineState::Jog => tr!("desktop.resume.state.jog"),
        MachineState::Alarm => tr!("desktop.resume.state.alarm"),
        MachineState::Door => tr!("desktop.resume.state.door"),
        MachineState::Check => tr!("desktop.resume.state.check"),
        MachineState::Home => tr!("desktop.resume.state.home"),
        MachineState::Sleep => tr!("desktop.resume.state.sleep"),
        MachineState::Other => tr!("desktop.resume.state.other"),
    }
}

/// La máquina no está en reposo ni en alarma: se está moviendo, en pausa, en homing...
fn is_busy(state: MachineState) -> bool {
    !matches!(state, MachineState::Idle | MachineState::Alarm)
}

/// Cuánto puede tener de viejo el punto de control para que la MPos de ahora se compare con la
/// guardada (10 minutos). El caso es «la app murió y la placa siguió sola con lo que tenía en su
/// búfer»: unos pocos segundos, o unos minutos con trazos lentos. Un punto de ayer describe otra
/// sesión: lo que la máquina haya hecho desde entonces no tiene que ver con el trabajo.
pub const MAX_CHECKPOINT_AGE_MS: u64 = 10 * 60 * 1000;

/// Margen para relojes que se corrigieron hacia atrás entre el punto de control y ahora.
const CLOCK_SKEW_MS: u64 = 60 * 1000;

/// ¿Vale la pena comparar la MPos viva con la del punto de control? Solo si el punto es reciente
/// (`saved_ms` es la hora de su último guardado y `now_ms` la de ahora) y desde que se abrió la
/// conexión no se hizo nada que mueva la cabeza (`altered`). Con homing, jog, encuadre u otro
/// trabajo la MPos describe lo que Tlaku acaba de hacer, no lo que pasó con el trabajo.
pub fn may_relocate(saved_ms: u64, now_ms: u64, altered: bool) -> bool {
    !altered
        && saved_ms <= now_ms.saturating_add(CLOCK_SKEW_MS)
        && now_ms.saturating_sub(saved_ms) <= MAX_CHECKPOINT_AGE_MS
}

/// Hora del último guardado del punto de control (ms desde 1970).
fn saved_at_ms(l: &Loaded) -> u64 {
    l.progress.as_ref().map_or(l.meta.started_at_ms, |p| p.t_ms)
}

/// La máquina conectada vista por la comparación con el punto de control: su MPos de ahora y si
/// esa posición es fiable. `None` si no está en reposo (en movimiento o en pausa la MPos no dice
/// dónde terminó) o aún no mandó un reporte.
///
/// La posición **no** es fiable con alarma (una placa con homing que se reinició queda en alarma
/// hasta el `$H`) o si el trabajador marcó que la placa se reinició (`position_unconfirmed`). El
/// `homed` de la conexión no sirve aquí: empieza en falso en cada conexión aunque la placa
/// conserve su referencia.
///
/// Es la lectura cruda; para decidir sobre un punto de control usa [`current_for`].
pub fn current(h: &Handle) -> Option<Now> {
    let s = h.status_report()?;
    if !matches!(s.state, MachineState::Idle | MachineState::Alarm) {
        return None;
    }
    let idle = s.state == MachineState::Idle;
    Some(Now {
        mpos: s.machine_pos,
        wpos: s.work_pos,
        valid: idle && s.position_unconfirmed != Some(true),
        parser_feed: if idle { parser_feed(h) } else { None },
    })
}

/// La máquina como la ve la comparación **de este punto de control**: como [`current`], pero
/// `None` (no comparar, se usa la posición guardada) si el punto es viejo o si, desde que se abrió
/// la conexión, se movió la cabeza.
///
/// Lo decide **una sola vez** por conexión y punto de control (la primera vez que la máquina está
/// en reposo con posición, o en cuanto ya no se puede comparar) y lo recuerda: el aviso, la vista
/// y el reinicio ven así la misma máquina, y el homing o el encuadre que la propia reanudación
/// exige no cambian la sugerencia (la MPos de después describe lo que Tlaku acaba de hacer, no lo
/// que pasó con el trabajo).
pub fn current_for(h: &Handle, l: &Loaded) -> Option<Now> {
    let mut slot = h.resume_baseline().lock().ok()?;
    if let Some((id, now)) = slot.as_ref()
        && *id == l.meta.id
    {
        return *now;
    }
    let decided = if may_relocate(saved_at_ms(l), now_ms(), h.position_altered()) {
        // Sin reporte aún o en movimiento no se decide: se vuelve a mirar en la próxima.
        Some(current(h)?)
    } else {
        None
    };
    *slot = Some((l.meta.id.clone(), decided));
    decided
}

/// `F` del estado del intérprete (`[GC:... F600 S0]` de `$G`); `None` si no contesta.
fn parser_feed(h: &Handle) -> Option<f64> {
    let lines = h.command("$G").ok()?;
    lines.iter().find_map(|l| {
        l.trim()
            .strip_prefix("[GC:")?
            .split_whitespace()
            .find_map(|w| w.strip_prefix('F')?.trim_end_matches(']').parse().ok())
    })
}

/// Comprueba las condiciones de seguridad (ver la cabecera del módulo).
pub fn check_safety(
    kind: ResumeKind,
    ready: &ResumeReadiness,
    args: &ResumeStart,
) -> Result<(), String> {
    if kind != ResumeKind::Gcode {
        return Err(tr!("desktop.resume.not_resumable"));
    }
    if is_busy(ready.state) {
        return Err(tr!("desktop.resume.busy", state = state_label(ready.state)));
    }
    if !ready.settings_read {
        return Err(tr!("desktop.resume.no_reply"));
    }
    match ready.homing {
        Some(true) if !ready.homed => return Err(tr!("desktop.resume.needs_home")),
        Some(true) => {}
        _ if !args.confirm_unmoved => return Err(tr!("desktop.resume.needs_confirm")),
        _ => {}
    }
    if !args.framed_ok {
        return Err(tr!("desktop.resume.needs_frame"));
    }
    if !args.checklist_ok {
        return Err(tr!("desktop.resume.needs_checklist"));
    }
    Ok(())
}

/// Encuadre del área de trabajo (el rectángulo de lo que graba el G-code guardado, sin los
/// desplazamientos desde y hacia el origen) con el láser apagado.
pub fn frame(h: &Handle, store: &CheckpointStore, l: &Loaded, speed: f64) -> Result<(), String> {
    // El encuadre mueve la cabeza: antes de moverla se fija con qué se compara el punto de control.
    let _ = current_for(h, l);
    let lines = store.read_gcode()?;
    let parsed = Parsed::parse(&lines);
    let frame = plan::frame_lines(&parsed, speed)?;
    h.frame_link(frame, LinkProfile::for_device(&l.meta.device))
}

/// Continúa el trabajo desde `args.line`. Falla, sin tocar la máquina, si falta algo de lo que
/// exige la reanudación o el preflight de siempre.
pub fn resume_now(
    h: &Handle,
    store: Arc<CheckpointStore>,
    l: &Loaded,
    args: &ResumeStart,
) -> Result<(), String> {
    if l.meta.id != args.id {
        return Err(tr!("desktop.resume.nothing_to_resume"));
    }
    check_safety(l.meta.kind, &readiness(h), args)?;
    let job = prepare(store, l, args, current_for(h, l).as_ref())?;
    let device = job.device;
    h.start_job_checkpointed(
        job.lines,
        LinkProfile::for_device(&device),
        Some(device.s_max),
        None,
        Some(job.plan),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(confirm: bool, framed: bool, checklist: bool) -> ResumeStart {
        ResumeStart {
            id: "x".into(),
            line: 0,
            at: None,
            confirm_unmoved: confirm,
            framed_ok: framed,
            checklist_ok: checklist,
        }
    }

    fn ready(homing: Option<bool>, homed: bool) -> ResumeReadiness {
        ResumeReadiness {
            homing,
            settings_read: true,
            homed,
            state: MachineState::Idle,
        }
    }

    #[test]
    fn homing_machines_need_home_and_the_rest_need_the_confirmation() {
        let g = ResumeKind::Gcode;
        let all = args(true, true, true);
        let no_confirm = args(false, true, true);
        assert!(check_safety(g, &ready(Some(true), false), &all).is_err());
        assert!(check_safety(g, &ready(Some(true), true), &no_confirm).is_ok());
        assert!(check_safety(g, &ready(Some(false), false), &no_confirm).is_err());
        assert!(check_safety(g, &ready(None, false), &no_confirm).is_err());
        assert!(check_safety(g, &ready(Some(false), false), &all).is_ok());
        assert!(check_safety(g, &ready(None, false), &all).is_ok());
    }

    #[test]
    fn a_busy_machine_or_one_that_did_not_answer_is_not_asked_to_confirm_anything() {
        let g = ResumeKind::Gcode;
        let all = args(false, true, true);
        let busy = ResumeReadiness {
            homing: None,
            settings_read: false,
            homed: true,
            state: MachineState::Run,
        };
        let e = check_safety(g, &busy, &all).unwrap_err();
        assert!(
            e.contains("ocupada") && e.contains("(En marcha)") && !e.contains("run"),
            "{e}"
        );
        assert!(!e.contains("no tiene homing"), "{e}");
        let silent = ResumeReadiness {
            state: MachineState::Idle,
            ..busy
        };
        let e = check_safety(g, &silent, &all).unwrap_err();
        assert!(e.contains("no contestó"), "{e}");
        assert!(!e.contains("no tiene homing"), "{e}");
        // Marlin contesta `$$` sin el ajuste: ahí sí se pide la confirmación.
        let e = check_safety(g, &ready(None, false), &all).unwrap_err();
        assert!(e.contains("no tiene homing"), "{e}");
    }

    #[test]
    fn the_live_position_is_only_compared_with_a_recent_checkpoint_and_an_untouched_head() {
        const MIN: u64 = 60_000;
        let now = 100_000 * MIN;
        // La app murió hace un minuto y desde que se conectó no se movió nada.
        assert!(may_relocate(now - MIN, now, false));
        assert!(may_relocate(now - 10 * MIN, now, false));
        // Un punto de ayer.
        assert!(!may_relocate(now - 24 * 60 * MIN, now, false));
        assert!(!may_relocate(now - 11 * MIN, now, false));
        // Con homing, jog o encuadre desde la conexión, aunque el punto sea de hace un segundo.
        assert!(!may_relocate(now - 1_000, now, true));
        // Un reloj corregido hacia atrás un poco no invalida el punto; mucho, sí.
        assert!(may_relocate(now + 30_000, now, false));
        assert!(!may_relocate(now + 5 * MIN, now, false));
    }

    #[test]
    fn the_frame_and_the_checklist_are_always_required() {
        let g = ResumeKind::Gcode;
        let r = ready(Some(true), true);
        assert!(check_safety(g, &r, &args(true, false, true)).is_err());
        assert!(check_safety(g, &r, &args(true, true, false)).is_err());
        assert!(check_safety(g, &r, &args(true, true, true)).is_ok());
    }

    #[test]
    fn only_gcode_jobs_resume() {
        for k in [ResumeKind::Ruida, ResumeKind::K40, ResumeKind::Galvo] {
            assert!(check_safety(k, &ready(Some(true), true), &args(true, true, true)).is_err());
        }
    }

    #[test]
    fn reads_the_homing_setting() {
        let lines: Vec<String> = ["$21=1", "$22=1", "ok"].map(String::from).to_vec();
        assert_eq!(homing_setting(&lines), Some(true));
        assert_eq!(homing_setting(&["$22=0".to_string()]), Some(false));
        assert_eq!(homing_setting(&["error:3".to_string()]), None);
    }
}
