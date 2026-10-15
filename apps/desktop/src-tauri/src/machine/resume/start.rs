//! Reanudar sobre una conexión viva: qué falta, las condiciones de seguridad y el envío.
//!
//! Para reanudar hacen falta TODAS estas condiciones (las comprueba [`resume_now`], no la
//! interfaz):
//!
//! - el punto de control es de un trabajo de G-code que se puede reanudar;
//! - la máquina tiene homing (`$22=1`) y se hizo `$H` desde que la placa se reinició o perdió la
//!   posición; si no tiene homing, el usuario confirmó que ni la cabeza ni el material se movieron
//!   y que el origen es el mismo;
//! - el usuario encuadró el trabajo completo y el material seguía en su lugar;
//! - la lista de comprobación de siempre está completa;
//! - el G-code resultante pasa el preflight de siempre (lo hace el trabajador al iniciar).

use super::assess::Now;
use super::store::{CheckpointStore, Loaded};
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

/// Si la máquina tiene homing y si ya se hizo. Marlin y Smoothieware no contestan `$$`: sin dato
/// no se asume homing.
pub fn readiness(h: &Handle) -> ResumeReadiness {
    ResumeReadiness {
        homing: h
            .command("$$")
            .ok()
            .and_then(|lines| homing_setting(&lines)),
        homed: h.is_homed(),
        state: h.status_report().map_or(MachineState::Idle, |s| s.state),
    }
}

/// La máquina conectada vista por la comparación con el punto de control: su MPos de ahora y si
/// esa posición es fiable. `None` si no está en reposo (en movimiento o en pausa la MPos no dice
/// dónde terminó) o aún no mandó un reporte.
///
/// La posición **no** es fiable con alarma (una placa con homing que se reinició queda en alarma
/// hasta el `$H`) o si el trabajador marcó que la placa se reinició (`position_unconfirmed`). El
/// `homed` de la conexión no sirve aquí: empieza en falso en cada conexión aunque la placa
/// conserve su referencia.
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

/// Encuadre del trabajo completo (el rectángulo del G-code guardado) con el láser apagado.
pub fn frame(h: &Handle, store: &CheckpointStore, l: &Loaded, speed: f64) -> Result<(), String> {
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
    let job = prepare(store, l, args, current(h).as_ref())?;
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
