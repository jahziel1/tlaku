//! Escritura de ajustes: lista cerrada, respaldo previo, confirmación, verificación y restauración.

use super::*;
use crate::settings::{self, SettingChange, SettingsFile, SettingsSnapshot};
use std::cell::RefCell;

fn ch(addr: u16, value: i64) -> SettingChange {
    SettingChange { addr, value }
}

/// Respaldo en memoria: guarda la instantánea y devuelve su descripción.
fn memory_backup(
    store: &RefCell<Vec<SettingsSnapshot>>,
) -> impl FnMut(&SettingsSnapshot) -> Result<String, String> + '_ {
    |snap| {
        store.borrow_mut().push(snap.clone());
        Ok(format!("memoria-{}", store.borrow().len()))
    }
}

#[test]
fn read_settings_returns_every_listed_setting_with_the_card() {
    let (mut s, _l, _e) = mem_session(EmuConfig::default(), fast_cfg());
    let snap = s.read_settings().unwrap();
    assert_eq!(snap.card_id, 0x6510_6510);
    assert_eq!(snap.model.as_deref(), Some("RDC6442S"));
    assert!(snap.complete());
    assert_eq!(snap.get(0x005), Some(200_000));
    assert_eq!(snap.get(0x026), Some(600_000));
    assert_eq!(snap.get(0x036), Some(400_000));
    assert_eq!(snap.get(0x11F), Some(5_000));
}

#[test]
fn nothing_is_written_without_explicit_confirmation() {
    let (mut s, _l, emu) = mem_session(EmuConfig::default(), fast_cfg());
    let store = RefCell::new(Vec::new());
    let err = s
        .apply_settings(&[ch(0x005, 100_000)], false, &mut memory_backup(&store))
        .unwrap_err();
    assert!(matches!(err, SessionError::NotConfirmed));
    assert!(emu.lock().unwrap().writes.is_empty());
    assert!(store.borrow().is_empty(), "ni siquiera se hizo el respaldo");
}

#[test]
fn applying_backs_up_first_writes_only_what_changes_and_verifies() {
    let (mut s, _l, emu) = mem_session(EmuConfig::default(), fast_cfg());
    let store = RefCell::new(Vec::new());
    let report = s
        .apply_settings(
            &[
                ch(0x11F, 4_000),
                ch(0x005, 150_000),
                ch(0x023, 400_000), // ya vale eso: no se escribe
            ],
            true,
            &mut memory_backup(&store),
        )
        .unwrap();
    // Respaldo único, con la lectura completa de ANTES de escribir.
    assert_eq!(store.borrow().len(), 1);
    assert!(store.borrow()[0].complete());
    assert_eq!(store.borrow()[0].get(0x005), Some(200_000));
    assert_eq!(report.backup.as_deref(), Some("memoria-1"));
    assert!(report.is_ok() && !report.restore_advised());
    assert_eq!(report.plan.unchanged, 1);
    assert_eq!(report.plan.changes.len(), 2);
    // Se escribió en el orden de la lista cerrada, con el valor repetido.
    let e = emu.lock().unwrap();
    assert_eq!(
        e.writes,
        vec![(0x005, 150_000, 150_000), (0x11F, 4_000, 4_000)]
    );
    assert_eq!(e.mem[&0x005], 150_000);
    // Verificación: lo releído coincide.
    assert_eq!(
        report
            .verified
            .iter()
            .map(|v| (v.addr, v.actual))
            .collect::<Vec<_>>(),
        vec![(0x005, Some(150_000)), (0x11F, Some(4_000))]
    );
}

#[test]
fn a_failed_backup_aborts_before_any_write() {
    let (mut s, _l, emu) = mem_session(EmuConfig::default(), fast_cfg());
    let err = s
        .apply_settings(&[ch(0x005, 100_000)], true, &mut |_| {
            Err("disco lleno".into())
        })
        .unwrap_err();
    assert!(matches!(&err, SessionError::BackupFailed(m) if m == "disco lleno"));
    assert!(emu.lock().unwrap().writes.is_empty());
}

#[test]
fn addresses_outside_the_closed_list_and_out_of_range_values_never_reach_the_machine() {
    let (mut s, _l, emu) = mem_session(EmuConfig::default(), fast_cfg());
    let store = RefCell::new(Vec::new());
    for bad in [
        ch(0x200, 1),     // estado
        ch(0x2FE, 5),     // ID de tarjeta
        ch(0x013, 16383), // potencia máxima del láser 1
        ch(0x591, 0),     // bloqueo de tarjeta
        ch(0x005, 0),     // rango
        ch(0x005, 10_000_000),
    ] {
        let err = s
            .apply_settings(&[ch(0x11F, 4_000), bad], true, &mut memory_backup(&store))
            .unwrap_err();
        assert!(matches!(err, SessionError::Settings(_)), "{bad:?}");
    }
    assert!(emu.lock().unwrap().writes.is_empty());
    assert!(store.borrow().is_empty());
}

#[test]
fn a_busy_machine_gets_no_writes() {
    let (mut s, _l, emu) = mem_session(EmuConfig::default(), fast_cfg());
    emu.lock().unwrap().running = true;
    let store = RefCell::new(Vec::new());
    let err = s
        .apply_settings(&[ch(0x005, 100_000)], true, &mut memory_backup(&store))
        .unwrap_err();
    assert!(matches!(err, SessionError::Busy(_)));
    assert!(emu.lock().unwrap().writes.is_empty());
}

#[test]
fn a_machine_that_ignores_writes_is_reported_and_restore_is_advised() {
    let cfg = EmuConfig {
        faults: EmuFaults {
            ignore_writes: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let (mut s, _l, _e) = mem_session(cfg, fast_cfg());
    let store = RefCell::new(Vec::new());
    let report = s
        .apply_settings(&[ch(0x005, 100_000)], true, &mut memory_backup(&store))
        .unwrap();
    assert!(!report.is_ok() && report.restore_advised());
    assert_eq!(report.verified[0].expected, 100_000);
    assert_eq!(report.verified[0].actual, Some(200_000));
}

#[test]
fn a_machine_that_corrupts_writes_is_reported() {
    let cfg = EmuConfig {
        faults: EmuFaults {
            corrupt_writes: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let (mut s, _l, _e) = mem_session(cfg, fast_cfg());
    let store = RefCell::new(Vec::new());
    let report = s
        .apply_settings(&[ch(0x005, 100_000)], true, &mut memory_backup(&store))
        .unwrap();
    assert!(report.restore_advised());
    assert_eq!(report.verified[0].actual, Some(100_001));
}

#[test]
fn restoring_the_backup_puts_everything_back_with_the_same_protections() {
    let (mut s, _l, emu) = mem_session(EmuConfig::default(), fast_cfg());
    let store = RefCell::new(Vec::new());
    let original = s.read_settings().unwrap();
    let report = s
        .apply_settings(
            &[ch(0x005, 150_000), ch(0x021, 6_000), ch(0x02A, -2_000)],
            true,
            &mut memory_backup(&store),
        )
        .unwrap();
    assert!(report.is_ok());
    assert_ne!(s.read_settings().unwrap(), original);

    // El respaldo previo, convertido en archivo y leído de vuelta.
    let file = SettingsFile::from_snapshot(&store.borrow()[0], "t");
    let file = settings::parse_file(&file.to_json()).unwrap();

    // Sin confirmar no restaura nada.
    let writes_before = emu.lock().unwrap().writes.len();
    assert!(matches!(
        s.restore_settings(&file, false, &mut memory_backup(&store)),
        Err(SessionError::NotConfirmed)
    ));
    assert_eq!(emu.lock().unwrap().writes.len(), writes_before);

    let restored = s
        .restore_settings(&file, true, &mut memory_backup(&store))
        .unwrap();
    assert!(restored.is_ok());
    assert_eq!(restored.plan.changes.len(), 3);
    assert_eq!(s.read_settings().unwrap(), original);
    // Restaurar también respalda el estado previo a restaurar.
    assert_eq!(store.borrow().len(), 2);
}

#[test]
fn a_backup_from_another_card_is_refused() {
    let (mut s, _l, emu) = mem_session(EmuConfig::default(), fast_cfg());
    let mut snap = s.read_settings().unwrap();
    snap.card_id = 0x9010_9010;
    let file = SettingsFile::from_snapshot(&snap, "t");
    let store = RefCell::new(Vec::new());
    let err = s
        .restore_settings(&file, true, &mut memory_backup(&store))
        .unwrap_err();
    assert!(matches!(
        err,
        SessionError::Settings(settings::SettingsError::CardMismatch { .. })
    ));
    assert!(emu.lock().unwrap().writes.is_empty());
}

#[test]
fn applying_the_current_values_does_nothing() {
    let (mut s, _l, emu) = mem_session(EmuConfig::default(), fast_cfg());
    let store = RefCell::new(Vec::new());
    let report = s
        .apply_settings(&[ch(0x005, 200_000)], true, &mut memory_backup(&store))
        .unwrap();
    assert!(report.plan.changes.is_empty() && report.backup.is_none());
    assert!(emu.lock().unwrap().writes.is_empty());
    assert!(store.borrow().is_empty());
}

#[test]
fn a_write_failure_stops_the_rest_and_says_where() {
    // La máquina deja de contestar justo en la primera escritura. Antes de ella: el estado (3
    // lecturas), el ID de tarjeta y los ajustes de la lista. No hay reintento por timeout
    // (podría duplicar la orden).
    let (mut s, _l, emu) = mem_session(EmuConfig::default(), fast_cfg());
    let store = RefCell::new(Vec::new());
    let before_first_write = 3 + 1 + settings::WRITABLE.len();
    let at = emu.lock().unwrap().received + before_first_write;
    emu.lock().unwrap().set_faults(EmuFaults {
        silent_from: Some(at),
        ..Default::default()
    });
    let report = s
        .apply_settings(
            &[ch(0x005, 100_000), ch(0x00C, 30_000)],
            true,
            &mut memory_backup(&store),
        )
        .unwrap();
    assert_eq!(report.write_error.as_ref().map(|e| e.0), Some(0x005));
    assert!(report.restore_advised());
    // El respaldo ya estaba hecho y la segunda escritura no se envió.
    assert_eq!(store.borrow().len(), 1);
    assert!(emu.lock().unwrap().writes.is_empty());
}

#[test]
fn only_listed_addresses_are_ever_written_by_a_full_flow() {
    let (mut s, _l, emu) = mem_session(EmuConfig::default(), fast_cfg());
    let store = RefCell::new(Vec::new());
    let all: Vec<SettingChange> = settings::WRITABLE
        .iter()
        .map(|d| match d.kind {
            // Un paso solo admite cambios de hasta el doble.
            settings::SettingKind::Step => ch(d.addr, 6_000),
            _ => ch(d.addr, (d.min.max(1) + d.max) / 2),
        })
        .collect();
    s.apply_settings(&all, true, &mut memory_backup(&store))
        .unwrap();
    let e = emu.lock().unwrap();
    assert!(!e.writes.is_empty());
    for (a, v0, v1) in &e.writes {
        assert!(settings::find(*a).is_some(), "0x{a:03X} fuera de la lista");
        assert_eq!(v0, v1);
    }
}

#[test]
fn a_new_bed_size_is_seen_by_the_probe_afterwards() {
    let (mut s, _l, _e) = mem_session(EmuConfig::default(), fast_cfg());
    let store = RefCell::new(Vec::new());
    s.apply_settings(&[ch(0x026, 700_000)], true, &mut memory_backup(&store))
        .unwrap();
    assert_eq!(s.read_bed_um().unwrap(), [700_000, 400_000]);
}
