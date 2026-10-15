//! Guarda de la cama para el movimiento manual y reinicios de la placa (Falcon2 real: sin
//! `$20`, la cabeza salió 4,85 mm de la cama; la placa se reinició a mitad de sesión).

use super::*;
use tlaku_emu::{Fault, Trigger};

const FALCON_BED: [f64; 2] = [400.0, 415.0];

fn falcon() -> Rig {
    let rig = connect_virtual("virtual:falcon2@20");
    rig.handle.set_bed(Some(FALCON_BED));
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig
}

/// `$H` completo en la máquina virtual (0,4 s de reloj a ×20).
fn home(rig: &Rig) {
    let r = rig.handle.command("$H").unwrap();
    assert_eq!(r.last().unwrap(), "ok", "{r:?}");
}

fn log_has(rig: &Rig, part: &str) -> bool {
    rig.sink.logs().iter().any(|(_, t)| t.contains(part))
}

fn count_logs(rig: &Rig, part: &str) -> usize {
    rig.sink
        .logs()
        .iter()
        .filter(|(_, t)| t.contains(part))
        .count()
}

fn sent_jogs(rig: &Rig) -> Vec<String> {
    emu(rig)
        .received_lines()
        .into_iter()
        .filter(|l| l.starts_with("$J="))
        .collect()
}

#[test]
fn a_jog_that_leaves_the_bed_is_rejected_after_homing() {
    let rig = falcon();
    home(&rig);
    let err = rig.handle.jog(-5.0, 0.0, 2000.0).unwrap_err();
    assert!(
        err.contains("saldría de la cama") && err.contains("X llegaría a -5.00 mm"),
        "{err}"
    );
    assert!(err.contains("límite 0 a 400"), "{err}");
    // No se mandó nada a la placa, y tampoco se recortó en silencio.
    assert!(sent_jogs(&rig).is_empty(), "{:?}", sent_jogs(&rig));
    // Por Y, contra el alto de la cama.
    let err = rig.handle.jog(0.0, 420.0, 2000.0).unwrap_err();
    assert!(
        err.contains("Y llegaría a 420.00 mm") && err.contains("límite 0 a 415"),
        "{err}"
    );
    // Los dos ejes a la vez dicen ambos.
    let err = rig.handle.jog(-1.0, -2.0, 2000.0).unwrap_err();
    assert!(
        err.contains("X llegaría") && err.contains("Y llegaría"),
        "{err}"
    );
    // Un movimiento hacia dentro sí pasa.
    rig.handle.jog(5.0, 5.0, 2000.0).unwrap();
    assert_eq!(sent_jogs(&rig).len(), 1);
}

#[test]
fn jog_to_outside_the_bed_is_rejected_after_homing() {
    let rig = falcon();
    home(&rig);
    let err = rig.handle.jog_to(-10.0, 20.0, 3000.0).unwrap_err();
    assert!(err.contains("X llegaría a -10.00 mm"), "{err}");
    let err = rig.handle.jog_to(10.0, 416.0, 3000.0).unwrap_err();
    assert!(
        err.contains("Y llegaría a 416.00 mm") && err.contains("límite 0 a 415"),
        "{err}"
    );
    assert!(sent_jogs(&rig).is_empty());
    // Ni siquiera el `M5` previo salió.
    assert!(!emu(&rig).received_lines().contains(&"M5".to_string()));
    rig.handle.jog_to(399.0, 414.0, 3000.0).unwrap();
}

#[test]
fn the_message_exists_in_english() {
    let en = tlaku_i18n::with_locale(tlaku_i18n::Locale::En, || {
        tlaku_i18n::tr!("desktop.worker.jog_outside_bed", detail = "X")
    });
    assert!(en.contains("would leave the bed"), "{en}");
}

#[test]
fn without_homing_the_jog_is_not_blocked_and_warns_once() {
    // La virtual arranca referenciada, pero esta sesión no hizo homing: no hay certeza.
    let rig = falcon();
    rig.handle.jog(-5.0, 0.0, 2000.0).unwrap();
    rig.handle.jog(5.0, 0.0, 2000.0).unwrap();
    assert_eq!(sent_jogs(&rig).len(), 2);
    assert_eq!(count_logs(&rig, "Sin límites conocidos"), 1);
}

#[test]
fn a_negative_machine_space_is_not_blocked() {
    // Una GRBL clásica con homing hacia el lado positivo (`$23=0`): el espacio es negativo.
    let rig = connect_virtual("virtual:grbl11@20");
    rig.handle.set_bed(Some([300.0, 200.0]));
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.command("$22=1").unwrap();
    rig.handle.command("$$").unwrap();
    home(&rig);
    rig.handle.jog(-5.0, 0.0, 2000.0).unwrap();
    assert_eq!(count_logs(&rig, "Sin límites conocidos"), 1);
}

#[test]
fn the_space_is_positive_only_near_zero_and_growing_inwards() {
    use crate::machine::worker::limits::space_is_positive;
    assert!(space_is_positive([0.025, 0.025], None));
    assert!(space_is_positive([1.0, 1.0], Some(3)));
    // MPos negativa tras el homing (GRBL de espacio negativo, con retroceso $27).
    assert!(!space_is_positive([-1.0, -1.0], None));
    assert!(!space_is_positive([-1.0, 0.0], Some(3)));
    // Lejos del cero no es un homing al cero.
    assert!(!space_is_positive([120.0, 0.0], None));
    // `$23` dice que el homing busca el lado positivo.
    assert!(!space_is_positive([0.0, 0.0], Some(0)));
}

#[test]
fn the_bed_comes_from_the_machine_settings_when_there_is_no_profile() {
    let rig = connect_virtual("virtual:falcon2@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    // Sin perfil: vale `$130`/`$131` de la placa (400 x 400 en la Falcon2 virtual).
    rig.handle.command("$$").unwrap();
    home(&rig);
    let err = rig.handle.jog(0.0, 405.0, 2000.0).unwrap_err();
    assert!(
        err.contains("Y llegaría a 405.00 mm") && err.contains("límite 0 a 400"),
        "{err}"
    );
}

#[test]
fn reading_settings_with_homing_but_no_soft_limits_warns_once() {
    let rig = connect_virtual("virtual:falcon2@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.command("$$").unwrap();
    rig.handle.command("$$").unwrap();
    assert!(log_has(&rig, "homing pero no límites por software ($20=0)"));
    assert!(log_has(&rig, "$130/$131"));
    assert_eq!(count_logs(&rig, "no límites por software"), 1);
    // Con `$20=1` no hay aviso.
    let rig = connect_virtual("virtual:falcon2@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.command("$20=1").unwrap();
    rig.handle.command("$$").unwrap();
    assert!(!log_has(&rig, "no límites por software"));
    // Y nunca se escribe solo: el motor no manda `$20=1` por su cuenta.
    let rig = connect_virtual("virtual:falcon2@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.command("$$").unwrap();
    assert!(
        !emu(&rig)
            .received_lines()
            .iter()
            .any(|l| l.starts_with("$20="))
    );
    assert_eq!(emu(&rig).setting(20), 0.0);
}

fn unconfirmed(rig: &Rig) -> bool {
    rig.sink
        .statuses()
        .last()
        .is_some_and(|s| s.position_unconfirmed == Some(true))
}

#[test]
fn a_reboot_at_rest_warns_and_leaves_the_position_unconfirmed_until_homing() {
    let rig = falcon();
    home(&rig);
    assert!(rig.handle.jog(-5.0, 0.0, 2000.0).is_err());
    assert!(rig.handle.is_homed());
    emu(&rig).inject(Trigger::Now, Fault::Reboot);
    rig.sink
        .wait("posición sin confirmar", |_| unconfirmed(&rig));
    assert!(!rig.handle.is_homed());
    assert!(log_has(&rig, "La máquina se reinició"));
    assert!(log_has(&rig, "Haz homing antes de mover"));
    // Un solo aviso aunque el arranque de la Falcon2 traiga varias líneas de saludo.
    thread::sleep(Duration::from_millis(300));
    assert_eq!(count_logs(&rig, "La máquina se reinició"), 1);
    // La guarda deja de dar por buena la posición: no bloquea por la cama (la placa, en alarma
    // tras el reinicio, rechaza el movimiento por su cuenta).
    let err = rig.handle.jog(-5.0, 0.0, 2000.0).unwrap_err();
    assert!(!err.contains("saldría de la cama"), "{err}");
    // Un homing la confirma y la guarda vuelve.
    home(&rig);
    rig.sink.wait("posición confirmada", |_| !unconfirmed(&rig));
    let err = rig.handle.jog(-5.0, 0.0, 2000.0).unwrap_err();
    assert!(err.contains("saldría de la cama"), "{err}");
}

#[test]
fn the_greeting_after_our_own_reset_is_expected() {
    let rig = falcon();
    home(&rig);
    let e = emu(&rig);
    let before = e.resets();
    // Reset manual (el mismo `0x18` que manda la parada segura): el saludo es esperado.
    rig.handle.realtime(RealtimeCommand::Reset).unwrap();
    rig.sink.wait("reinicio", |_| e.resets() > before);
    let n = rig.sink.statuses().len();
    rig.sink.wait("sondeo", |s| s.statuses().len() > n + 3);
    assert!(!unconfirmed(&rig));
    assert!(
        rig.sink
            .statuses()
            .iter()
            .all(|s| s.position_unconfirmed.is_none())
    );
    assert!(!log_has(&rig, "La máquina se reinició"));
}

#[test]
fn cancelling_a_job_does_not_flag_a_reboot() {
    let rig = falcon();
    home(&rig);
    rig.handle.start_job(long_job(), 127).unwrap();
    assert!(emu(&rig).wait_state(tlaku_emu::StateKind::Run, Duration::from_secs(3)));
    rig.handle.cancel().unwrap();
    rig.sink
        .wait("cancelado", |s| s.jobs().contains(&JobState::Cancelled));
    let n = rig.sink.statuses().len();
    rig.sink.wait("sondeo", |s| s.statuses().len() > n + 5);
    assert!(
        rig.sink
            .statuses()
            .iter()
            .all(|s| s.position_unconfirmed.is_none())
    );
    assert!(!log_has(&rig, "La máquina se reinició"));
    assert!(
        !rig.sink
            .job_messages()
            .iter()
            .any(|m| m.contains("a mitad del trabajo"))
    );
}

#[test]
fn a_reboot_in_the_middle_of_a_job_stops_it_and_flags_the_position() {
    let rig = falcon();
    home(&rig);
    rig.handle.start_job(long_job(), 127).unwrap();
    let e = emu(&rig);
    assert!(e.wait_state(tlaku_emu::StateKind::Run, Duration::from_secs(3)));
    e.inject(Trigger::Now, Fault::Reboot);
    rig.sink
        .wait("trabajo con error", |s| s.jobs().contains(&JobState::Error));
    let ev = rig
        .sink
        .all()
        .into_iter()
        .find_map(|x| match x {
            Emit::Job(j) if j.state == JobState::Error => Some(j),
            _ => None,
        })
        .unwrap();
    assert!(
        ev.message
            .contains("la máquina se reinició a mitad del trabajo"),
        "{}",
        ev.message
    );
    assert!(ev.position_lost);
    rig.sink
        .wait("posición sin confirmar", |_| unconfirmed(&rig));
    assert!(!rig.handle.is_homed());
    assert!(!rig.handle.is_running());
    // Se hizo la parada segura: feed hold y soft reset.
    let rt = e.realtime_bytes();
    assert!(rt.contains(&b'!') && rt.contains(&0x18), "{rt:?}");
}
