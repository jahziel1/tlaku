//! Placa que deja de contestar (`docs/validacion-hardware.md` §2.8): el silencio nunca da la
//! conexión por perdida (como LightBurn, se espera); solo avisa (`machine://silence`) y Detener y
//! Desconectar funcionan siempre.

use super::*;

impl Collect {
    /// Los `silence_ms` avisados, en orden.
    pub(super) fn silences(&self) -> Vec<u32> {
        self.all()
            .into_iter()
            .filter_map(|e| match e {
                Emit::Silence(s) => Some(s.silence_ms),
                _ => None,
            })
            .collect()
    }
}

/// Plazos con aviso temprano a los 250 ms y la pérdida muy lejos (no se alcanza).
fn with_notice() -> Timing {
    Timing {
        silence_notice: Duration::from_millis(250),
        ..fast()
    }
}

#[test]
fn the_early_notice_comes_when_an_idle_board_stops_answering_and_clears_when_it_comes_back() {
    let rig = connect_with(with_notice());
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    assert!(rig.sink.silences().is_empty());
    rig.fake.mute.store(true, Ordering::SeqCst);
    rig.sink.wait("aviso", |s| !s.silences().is_empty());
    assert!(!rig.handle.is_closed());
    let first = rig.sink.silences()[0];
    assert!(first >= 250, "{first}");
    rig.fake.mute.store(false, Ordering::SeqCst);
    rig.sink
        .wait("aviso retirado", |s| s.silences().last() == Some(&0));
    // Una sola vez, y la conexión nunca cayó.
    let all = rig.sink.silences();
    assert_eq!(all.iter().filter(|&&m| m == 0).count(), 1, "{all:?}");
    assert!(rig.sink.connections().iter().all(|c| c.connected));
}

#[test]
fn the_early_notice_also_comes_during_a_job_and_does_not_change_its_outcome() {
    let rig = connect_with(with_notice());
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.sink.wait("trabajo en marcha", |s| {
        s.jobs().contains(&JobState::Started)
    });
    rig.fake.mute.store(true, Ordering::SeqCst);
    rig.sink.wait("aviso", |s| !s.silences().is_empty());
    assert!(rig.sink.silences()[0] >= 250);
    rig.fake.mute.store(false, Ordering::SeqCst);
    rig.sink
        .wait("aviso retirado", |s| s.silences().last() == Some(&0));
    // La placa estaba muda, no sorda: lo perdido son los `ok` de ese lapso; el trabajo sigue vivo
    // y no se mandó ningún feed hold.
    assert!(!rig.handle.is_closed());
    assert!(!rig.fake.realtime().contains(&b'!'));
}

#[test]
fn a_board_that_answers_normally_never_shows_the_notice() {
    let rig = connect_with(with_notice());
    rig.sink.wait("sondeo", |s| s.statuses().len() > 15);
    assert!(rig.sink.silences().is_empty());
}

#[test]
fn the_notice_is_off_with_a_zero_notice_time() {
    let rig = connect_with(Timing { ..fast() });
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.fake.mute.store(true, Ordering::SeqCst);
    thread::sleep(Duration::from_millis(700));
    assert!(rig.sink.silences().is_empty());
}

#[test]
fn a_job_with_a_board_silent_for_longer_than_the_old_limits_is_not_dropped_and_no_hold_is_sent() {
    let rig = connect_with(with_notice());
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    // La placa no devuelve los `ok`: el trabajo queda a medias.
    rig.fake.hold_ok.store(true, Ordering::SeqCst);
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.sink.wait("trabajo en marcha", |s| {
        s.jobs().contains(&JobState::Started)
    });
    rig.fake.mute.store(true, Ordering::SeqCst);
    // Más que el plazo de 5 s de antes a esta escala (400 ms ×12).
    thread::sleep(Duration::from_millis(5000));
    assert!(!rig.handle.is_closed(), "{:?}", rig.sink.connections());
    assert!(rig.sink.connections().iter().all(|c| c.connected));
    assert_eq!(rig.sink.jobs(), [JobState::Started]);
    assert!(rig.sink.silences().iter().any(|&m| m >= 4000));
    // Nada se mandó por el silencio: ni `!` ni reset.
    assert!(rig.fake.realtime().is_empty(), "{:?}", rig.fake.realtime());
}

#[test]
fn stop_and_disconnect_work_at_once_with_a_board_that_never_answers() {
    let mut rig = connect_with(with_notice());
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.fake.hold_ok.store(true, Ordering::SeqCst);
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.sink.wait("trabajo en marcha", |s| {
        s.jobs().contains(&JobState::Started)
    });
    rig.fake.mute.store(true, Ordering::SeqCst);
    rig.sink.wait("aviso", |s| !s.silences().is_empty());
    // Detener: el feed hold y el reset de siempre (la placa muda no contesta, y aun así termina).
    let t = Instant::now();
    rig.handle.cancel().unwrap();
    rig.sink.wait("trabajo cancelado", |s| {
        s.jobs().contains(&JobState::Cancelled)
    });
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
    assert!(!rig.handle.is_closed());
    // Desconectar con la placa muda también es inmediato.
    let t = Instant::now();
    rig.conn.take().unwrap().close();
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
}
