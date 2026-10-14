//! Posicionar el láser (movimiento absoluto), ejes extra y disparo de prueba.

use super::*;

#[test]
fn jog_to_turns_the_laser_off_and_moves_to_the_absolute_position() {
    let rig = connect_virtual("virtual:grbl11@50");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.jog_to(25.0, 40.5, 3000.0).unwrap();
    let e = emu(&rig);
    let got = e.received_lines();
    assert!(got.contains(&"M5".to_string()), "{got:?}");
    assert!(
        got.iter()
            .any(|l| l.replace(' ', "") == "$J=G90G21X25Y40.5F3000"),
        "{got:?}"
    );
    e.wait_idle(Duration::from_secs(5));
    assert!(!e.laser_on());
    let w = e.wpos();
    assert!(
        (w[0] - 25.0).abs() < 1e-6 && (w[1] - 40.5).abs() < 1e-6,
        "{w:?}"
    );
    // La posición de trabajo que ve la interfaz es la misma.
    rig.sink.wait("posición en el reporte", |s| {
        s.statuses()
            .last()
            .is_some_and(|x| (x.work_pos[0] - 25.0).abs() < 1e-3)
    });
}

#[test]
fn jog_to_rejects_bad_numbers_and_a_running_job() {
    let rig = connect_virtual("virtual:falcon2@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    assert!(rig.handle.jog_to(f64::NAN, 0.0, 1000.0).is_err());
    assert!(rig.handle.jog_to(1.0, 1.0, 0.0).is_err());
    rig.handle.start_job(long_job(), 127).unwrap();
    let e = emu(&rig);
    assert!(e.wait_state(tlaku_emu::StateKind::Run, Duration::from_secs(3)));
    let err = rig.handle.jog_to(1.0, 1.0, 1000.0).unwrap_err();
    assert!(err.contains("trabajo en curso"), "{err}");
    rig.handle.cancel().unwrap();
}

#[test]
fn jog_to_works_on_marlin_with_plain_moves() {
    let rig = connect_virtual("virtual:marlin@50");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.jog_to(10.0, 20.0, 2400.0).unwrap();
    let got = emu(&rig).received_lines();
    assert!(
        got.iter().any(|l| l.starts_with("G1 X10 Y20 F2400")),
        "{got:?}"
    );
    assert!(got.iter().all(|l| !l.starts_with("$J")), "{got:?}");
}

#[test]
fn z_and_u_jogs_send_a_jog_line_and_reject_unknown_axes() {
    let rig = connect();
    rig.handle
        .jog_axis(LinkFamily::Grbl, 'z', -2.5, 300.0)
        .unwrap();
    rig.handle
        .jog_axis_to(LinkFamily::Grbl, 'U', 12.0, 600.0)
        .unwrap();
    let lines = rig.fake.lines();
    // El motor quita los espacios (GRBL los ignora) antes de mandar.
    assert!(
        lines.contains(&"$J=G91G21Z-2.5F300".to_string()),
        "{lines:?}"
    );
    assert!(lines.contains(&"$J=G90G21U12F600".to_string()), "{lines:?}");
    for bad in ['X', 'Y', 'Q', '1'] {
        assert!(
            rig.handle
                .jog_axis(LinkFamily::Grbl, bad, 1.0, 100.0)
                .is_err(),
            "{bad}"
        );
    }
    assert!(
        rig.handle
            .jog_axis(LinkFamily::Grbl, 'Z', f64::NAN, 100.0)
            .is_err()
    );
    assert!(
        rig.handle
            .jog_axis(LinkFamily::Grbl, 'Z', 1.0, 0.0)
            .is_err()
    );
    // Un rechazo de la controladora se explica.
    rig.fake.set_state("Alarm");
}

#[test]
fn a_z_jog_on_marlin_goes_relative_and_returns_to_absolute() {
    let rig = connect_virtual("virtual:marlin@20");
    rig.handle
        .jog_axis(LinkFamily::Marlin, 'Z', 1.0, 300.0)
        .unwrap();
    // El sondeo de posición (M114) se intercala: no cuenta.
    let sent: Vec<String> = emu(&rig)
        .received_lines()
        .into_iter()
        .filter(|l| l != "M114")
        .collect();
    let at = sent.iter().position(|l| l.starts_with("G91")).expect("G91");
    assert!(
        sent[at + 1].starts_with("G1Z1") || sent[at + 1].starts_with("G1 Z1"),
        "{sent:?}"
    );
    assert_eq!(sent[at + 2], "G90", "{sent:?}");
}

#[test]
fn fire_turns_the_laser_on_waits_and_always_ends_with_m5() {
    let rig = connect();
    rig.handle.fire(LinkFamily::Grbl, 5.0, 1000.0, 250).unwrap();
    let lines = rig.fake.lines();
    let at = lines.iter().position(|l| l == "M3S50").expect("M3 S50");
    assert_eq!(lines[at + 1], "G4P0.25", "{lines:?}");
    assert_eq!(lines[at + 2], "M5", "{lines:?}");
}

#[test]
fn fire_still_sends_m5_when_the_dwell_fails_and_when_the_laser_cannot_be_confirmed_off() {
    let rig = connect();
    // La segunda línea de G-code (la pausa) falla: el M5 sale igual y el error se informa.
    *rig.fake.fault_at.lock().unwrap() = Some((2, "error:20".into()));
    let e = rig
        .handle
        .fire(LinkFamily::Grbl, 5.0, 1000.0, 100)
        .unwrap_err();
    assert!(e.contains("error:20"), "{e}");
    assert_eq!(rig.fake.lines().last().map(String::as_str), Some("M5"));

    // Si hasta el M5 falla, el mensaje dice claramente que no se pudo confirmar el apagado.
    let rig = connect();
    *rig.fake.fault_at.lock().unwrap() = Some((3, "error:20".into()));
    let e = rig
        .handle
        .fire(LinkFamily::Grbl, 5.0, 1000.0, 100)
        .unwrap_err();
    assert!(
        e.contains("no se pudo confirmar") || e.contains("No se pudo confirmar"),
        "{e}"
    );
}

#[test]
fn fire_refuses_dangerous_values_without_touching_the_machine() {
    let rig = connect();
    for (pct, ms, s_max) in [
        (0.0, 100, 1000.0),
        (50.0, 100, 1000.0),
        (f64::NAN, 100, 1000.0),
        (5.0, 5, 1000.0),
        (5.0, 60_000, 1000.0),
        (5.0, 100, 0.0),
    ] {
        assert!(
            rig.handle.fire(LinkFamily::Grbl, pct, s_max, ms).is_err(),
            "{pct} {ms} {s_max}"
        );
    }
    assert!(rig.fake.lines().iter().all(|l| !l.starts_with('M')));
}

#[test]
fn fire_and_axis_jogs_are_refused_while_a_job_runs() {
    let rig = connect_virtual("virtual:falcon2@1");
    rig.handle.start_job(long_job(), 127).unwrap();
    rig.sink
        .wait("trabajo", |s| s.jobs().contains(&JobState::Started));
    assert!(rig.handle.fire(LinkFamily::Grbl, 5.0, 1000.0, 100).is_err());
    assert!(
        rig.handle
            .jog_axis(LinkFamily::Grbl, 'Z', 1.0, 100.0)
            .is_err()
    );
    let sent = emu(&rig).received_lines();
    assert!(
        !sent
            .iter()
            .any(|l| l.starts_with("M3S") || l.starts_with("M3 S")),
        "{sent:?}"
    );
    rig.handle.cancel().unwrap();
}

#[test]
fn engine_messages_follow_the_language_on_the_virtual_machine() {
    use tlaku_i18n::{Locale, with_locale};
    let mut rig = connect_virtual("virtual:grbl11@50");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    // Los errores que se arman en el hilo que llama salen en el idioma de ese hilo; en español por
    // omisión y en inglés dentro de `with_locale`.
    let es = rig
        .handle
        .jog_axis(LinkFamily::Grbl, 'Q', 1.0, 100.0)
        .unwrap_err();
    assert!(es.contains("no es válido"), "{es}");
    let en = with_locale(Locale::En, || {
        rig.handle
            .jog_axis(LinkFamily::Grbl, 'Q', 1.0, 100.0)
            .unwrap_err()
    });
    assert!(en.contains("is not valid"), "{en}");
    let en = with_locale(Locale::En, || {
        rig.handle
            .jog_axis(LinkFamily::Grbl, 'Z', 1.0, 0.0)
            .unwrap_err()
    });
    assert!(en.contains("move speed must be greater than 0"), "{en}");
    rig.conn.take().unwrap().close();
    let en = with_locale(Locale::En, || rig.handle.command("$$").unwrap_err());
    assert!(en.contains("No machine is connected"), "{en}");
}
