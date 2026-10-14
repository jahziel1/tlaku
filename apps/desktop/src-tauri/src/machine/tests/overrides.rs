//! Overrides en vivo de velocidad y potencia (`tlaku-emu` los implementa).

use super::*;

fn last_overrides(sink: &Collect) -> Option<Overrides> {
    sink.statuses().last().and_then(|s| s.overrides)
}

#[test]
fn feed_and_power_overrides_reach_the_machine_and_come_back_in_the_status() {
    let rig = connect_virtual("virtual:grbl11@50");
    // Sin un `Ov:` visto, el plan parte de un reinicio: se espera al primero para que no varíe.
    rig.sink.wait("primer Ov:", |s| last_overrides(s).is_some());
    rig.handle.set_override(OverrideKind::Feed, 130).unwrap();
    rig.handle.set_override(OverrideKind::Spindle, 85).unwrap();
    let e = emu(&rig);
    // El hilo de la conexión escribe los bytes después de que `set_override` vuelve: se espera a
    // que estén todos antes de compararlos (con carga, la lectura inmediata llegaba antes).
    let ov_bytes = || -> Vec<u8> {
        e.realtime_bytes()
            .into_iter()
            .filter(|b| *b >= 0x90)
            .collect()
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while ov_bytes().len() < 9 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let rt = e.realtime_bytes();
    // Desde el 100 % que informa la controladora: velocidad +10 tres veces; potencia -10 y -1
    // cinco veces. Sin reinicio, porque ya se conoce el valor de partida.
    let overrides = ov_bytes();
    assert_eq!(
        overrides,
        [0x91, 0x91, 0x91, 0x9B, 0x9D, 0x9D, 0x9D, 0x9D, 0x9D],
        "{rt:x?}"
    );
    // El `Ov:` real de la controladora llega a la interfaz.
    rig.sink.wait("Ov: en el reporte", |s| {
        last_overrides(s).is_some_and(|o| o.feed == 130 && o.spindle == 85)
    });
    // Volver al 100 % es un solo byte.
    let before = e.realtime_bytes().len();
    rig.handle.set_override(OverrideKind::Feed, 100).unwrap();
    // Solo se miran los bytes de override: entre medias puede colarse un `?` del sondeo.
    let new_overrides = || -> Vec<u8> {
        e.realtime_bytes()[before..]
            .iter()
            .copied()
            .filter(|b| *b >= 0x90)
            .collect()
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while new_overrides().is_empty() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(new_overrides(), [0x90]);
    rig.sink.wait("100 % otra vez", |s| {
        last_overrides(s).is_some_and(|o| o.feed == 100 && o.spindle == 85)
    });
}

#[test]
fn overrides_are_clamped_to_the_grbl_range() {
    let rig = connect_virtual("virtual:grbl11@50");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.set_override(OverrideKind::Feed, 900).unwrap();
    rig.sink.wait("tope de 200 %", |s| {
        last_overrides(s).is_some_and(|o| o.feed == 200)
    });
    rig.handle.set_override(OverrideKind::Feed, 1).unwrap();
    rig.sink.wait("piso de 10 %", |s| {
        last_overrides(s).is_some_and(|o| o.feed == 10)
    });
}

#[test]
fn overrides_apply_to_a_job_in_progress() {
    let rig = connect_virtual("virtual:grbl11@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.start_job(long_job(), 127).unwrap();
    let e = emu(&rig);
    assert!(e.wait_state(tlaku_emu::StateKind::Run, Duration::from_secs(3)));
    rig.handle.set_override(OverrideKind::Feed, 150).unwrap();
    rig.handle.set_override(OverrideKind::Spindle, 120).unwrap();
    rig.sink.wait("overrides durante el trabajo", |s| {
        last_overrides(s).is_some_and(|o| o.feed == 150 && o.spindle == 120)
    });
    // El trabajo sigue en marcha, sin pausa ni cancelación.
    assert_eq!(rig.sink.jobs(), [JobState::Started]);
    assert!(
        rig.sink
            .logs()
            .iter()
            .any(|(d, t)| *d == LogDir::Out && t.contains("override de velocidad")),
        "{:?}",
        rig.sink.logs()
    );
    rig.handle.cancel().unwrap();
}

#[test]
fn marlin_and_smoothie_refuse_overrides_with_a_reason() {
    for port in ["virtual:marlin@50", "virtual:smoothie@50"] {
        let rig = connect_virtual(port);
        rig.sink.wait("estado", |s| !s.statuses().is_empty());
        let err = rig
            .handle
            .set_override(OverrideKind::Feed, 120)
            .unwrap_err();
        assert!(
            err.contains("solo existen en controladoras GRBL"),
            "{port}: {err}"
        );
        assert!(emu(&rig).realtime_bytes().is_empty(), "{port}");
        assert!(last_overrides(&rig.sink).is_none(), "{port}");
    }
}

#[test]
fn overrides_with_the_fake_machine_send_one_byte_per_step_and_remember_the_value() {
    // La `Fake` no emite `Ov:`: el valor pedido se recuerda hasta que la controladora lo informe,
    // así dos pedidos seguidos parten del anterior y no de un reinicio.
    let rig = connect();
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.set_override(OverrideKind::Feed, 120).unwrap();
    rig.handle.set_override(OverrideKind::Feed, 125).unwrap();
    rig.sink.wait("bytes", |_| {
        rig.fake.seen.lock().unwrap().realtime.len() >= 8
    });
    let rt = rig.fake.seen.lock().unwrap().realtime.clone();
    let overrides: Vec<u8> = rt.into_iter().filter(|b| *b >= 0x90).collect();
    // reset, +10, +10, +1 x5
    assert_eq!(overrides, [0x90, 0x91, 0x91, 0x93, 0x93, 0x93, 0x93, 0x93]);
    assert_eq!(last_overrides(&rig.sink).map(|o| o.feed), Some(125));
}
