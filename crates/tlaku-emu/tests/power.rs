//! Corte de luz (`Fault::PowerLoss`): la cabeza se queda donde estaba, la placa pierde la
//! posición y se reinicia, y el cable se cae hasta que alguien lo vuelve a abrir.

mod common;

use std::time::Duration;

use common::*;
use tlaku_emu::{Fault, Profile, StateKind, Trigger};
use tlaku_grbl::serial::Transport;

#[test]
fn a_power_cut_stops_the_head_and_loses_the_position_but_not_the_trace() {
    let mut r = Rig::with(Profile::falcon2_pro(), |b| b.homed(true));
    for l in ["M4 S0", "G1 X50 S800 F3000"] {
        r.line(l);
    }
    r.secs(1);
    let burned = r.h.trace().total_length();
    assert!(burned > 5.0 && r.h.laser_on(), "{burned}");
    let at = r.h.mpos();
    r.h.inject(Trigger::Now, Fault::PowerLoss);
    // La cabeza no se mueve más, el láser se apaga, y el rastro de lo grabado se conserva.
    r.secs(1);
    assert_eq!(r.h.mpos(), at);
    assert!(!r.h.laser_on());
    assert!((r.h.trace().total_length() - burned).abs() < 1e-6);
    assert!(r.h.position_lost());
    assert!(!r.h.is_connected());
    assert!(r.vm.write_all(b"?").is_err());
}

#[test]
fn after_a_power_cut_a_homing_machine_waits_for_homing() {
    let mut r = Rig::with(Profile::falcon2_pro(), |b| b.homed(true));
    r.line("G1 X20 F3000");
    r.secs(1);
    r.h.inject(Trigger::Now, Fault::PowerLoss);
    let mut vm = r.h.reconnect();
    // Reabrir el puerto: la placa arranca en alarma (`$22=1`) y no ejecuta G-code hasta el `$H`.
    assert!(matches!(r.h.state(), StateKind::Alarm(_)));
    vm.write_all(b"G0 X5\n").unwrap();
    let mut buf = [0u8; 256];
    let n = vm.read_timeout(&mut buf, Duration::from_millis(5)).unwrap();
    assert!(String::from_utf8_lossy(&buf[..n]).contains("error:9"));
    vm.write_all(b"$H\n").unwrap();
    r.secs(10);
    assert_eq!(r.h.state(), StateKind::Idle);
    assert!(!r.h.position_lost());
    assert_eq!(r.h.mpos(), [0.0, 0.0, 0.0]);
}

#[test]
fn a_machine_without_homing_comes_back_idle() {
    let mut r = Rig::new(Profile::grbl_1_1h());
    r.line("G1 X20 F3000");
    r.secs(1);
    r.h.inject(Trigger::Now, Fault::PowerLoss);
    let mut vm = r.h.reconnect();
    assert_eq!(r.h.state(), StateKind::Idle);
    vm.write_all(b"G0 X5\n").unwrap();
    let mut buf = [0u8; 256];
    let n = vm.read_timeout(&mut buf, Duration::from_millis(5)).unwrap();
    assert!(String::from_utf8_lossy(&buf[..n]).contains("ok"));
}
