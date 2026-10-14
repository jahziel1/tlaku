//! Sonda de contacto (M48): `probe` sobre la máquina virtual con su disco de prueba.

use super::*;

/// Va con el cabezal (sin láser) a un punto de la cama y espera a que llegue.
fn goto(rig: &Rig, x: f64, y: f64) {
    rig.handle.jog_to(x, y, 6000.0).unwrap();
    assert!(emu(rig).wait_idle(Duration::from_secs(10)));
}

#[test]
fn a_touch_stops_on_the_edge_and_retracts() {
    let rig = connect_virtual("virtual:grbl11@50");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    // El disco virtual: centro (100, 100), radio 15. Desde la izquierda, el borde está en X85.
    goto(&rig, 70.0, 100.0);
    let t = rig.handle.probe(30.0, 0.0, 200.0, 2.0).unwrap();
    assert!(t.hit, "{t:?}");
    assert!(
        (t.work_pos[0] - 85.0).abs() < 1e-6 && (t.work_pos[1] - 100.0).abs() < 1e-6,
        "{t:?}"
    );
    let e = emu(&rig);
    assert!(e.wait_idle(Duration::from_secs(10)));
    // Retrocedió 2 mm y volvió a G90.
    let w = e.wpos();
    assert!((w[0] - 83.0).abs() < 1e-6, "{w:?}");
    let got: Vec<String> = e
        .received_lines()
        .iter()
        .map(|l| l.replace(' ', ""))
        .collect();
    for want in ["M5", "G91", "G38.3X30Y0F200", "G1X-2Y0F200", "G90"] {
        assert!(got.contains(&want.to_string()), "falta {want}: {got:?}");
    }
    assert!(!e.laser_on());
    // Ya fuera del disco: el estado vuelve a ser sin contacto.
    assert!(!e.probe_pin());
}

#[test]
fn a_miss_is_a_result_not_an_alarm() {
    let rig = connect_virtual("virtual:grbl11@50");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    goto(&rig, 20.0, 30.0);
    let t = rig.handle.probe(10.0, 0.0, 300.0, 2.0).unwrap();
    assert!(!t.hit);
    assert!((t.work_pos[0] - 30.0).abs() < 1e-6, "{t:?}");
    let e = emu(&rig);
    assert!(e.wait_idle(Duration::from_secs(10)));
    assert_eq!(e.alarm(), None);
    // Sin contacto no retrocede.
    assert!((e.wpos()[0] - 30.0).abs() < 1e-6);
}

#[test]
fn unsafe_arguments_never_reach_the_machine() {
    let rig = connect_virtual("virtual:grbl11@50");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    let before = emu(&rig).received_lines().len();
    assert!(
        rig.handle
            .probe(500.0, 0.0, 100.0, 2.0)
            .unwrap_err()
            .contains("recorrido")
    );
    assert!(
        rig.handle
            .probe(10.0, 0.0, 5000.0, 2.0)
            .unwrap_err()
            .contains("velocidad")
    );
    assert!(rig.handle.probe(f64::NAN, 0.0, 100.0, 2.0).is_err());
    assert_eq!(emu(&rig).received_lines().len(), before);
}

#[test]
fn already_touching_is_reported_and_leaves_absolute_mode() {
    let rig = connect_virtual("virtual:grbl11@50");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    goto(&rig, 100.0, 100.0);
    let err = rig.handle.probe(10.0, 0.0, 100.0, 2.0).unwrap_err();
    assert!(err.contains("ya está activa"), "{err}");
    let got: Vec<String> = emu(&rig)
        .received_lines()
        .iter()
        .map(|l| l.replace(' ', ""))
        .collect();
    // Aun con el fallo se vuelve a G90.
    assert_eq!(got.last().map(String::as_str), Some("G90"), "{got:?}");
}

#[test]
fn marlin_has_no_probe_and_a_running_job_blocks_it() {
    let rig = connect_virtual("virtual:marlin@50");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    assert!(
        rig.handle
            .probe(10.0, 0.0, 100.0, 2.0)
            .unwrap_err()
            .contains("GRBL")
    );
    let rig = connect_virtual("virtual:grbl11@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.start_job(long_job(), 127).unwrap();
    assert!(emu(&rig).wait_state(tlaku_emu::StateKind::Run, Duration::from_secs(3)));
    let err = rig.handle.probe(10.0, 0.0, 100.0, 2.0).unwrap_err();
    assert!(err.contains("trabajo en curso"), "{err}");
    rig.handle.cancel().unwrap();
}
