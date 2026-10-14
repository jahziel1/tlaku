//! Ajustes, respaldos, documentos y nombre de almacenamiento (ola 6E; EXPERIMENTAL), contra el
//! emulador UDP y la máquina virtual `virtual:ruida`.

use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};
use tlaku_doc::RuidaSettingChange;

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Carpeta temporal para los respaldos; se borra al soltarla.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let n = COUNTER.fetch_add(1, AtomicOrdering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("tlaku-ruida-backups-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn backups(&self) -> PathBuf {
        backups_dir(&self.0)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn change(key: &str, value: f64) -> RuidaSettingChange {
    RuidaSettingChange {
        key: key.into(),
        value,
    }
}

fn value_of(s: &tlaku_doc::RuidaControllerSettings, key: &str) -> Option<f64> {
    s.settings
        .iter()
        .find(|x| x.key == key)
        .and_then(|x| x.value)
}

/// Sin la bandera `experimental_writes` ninguna orden de escritura llega al emulador.
#[test]
fn without_the_experimental_flag_no_write_command_reaches_the_machine() {
    let r = rig(false);
    let dir = TempDir::new();
    // Dejamos un documento guardado para tener qué borrar o ejecutar.
    r.handle
        .prepare_store(Some("UNO".into()), false, false)
        .unwrap();
    r.start().unwrap();
    let before = r.emu.emulator().received;
    let h = &r.handle;
    let errs = [
        h.settings_apply(
            vec![change("g0_speed", 150_000.0)],
            true,
            dir.backups(),
            false,
        )
        .map(|_| ()),
        h.settings_restore(h.settings_export().unwrap(), true, dir.backups(), false)
            .map(|_| ()),
        h.calibrate("x_step".into(), 100.0, 99.0, false).map(|_| ()),
        h.file_delete(1, "UNO".into(), true, false).map(|_| ()),
        h.file_run(1, "UNO".into(), true, false).map(|_| ()),
        h.prepare_store(Some("UNO".into()), true, false),
    ];
    for e in errs {
        let msg = e.unwrap_err();
        assert!(msg.contains("desactivadas"), "{msg}");
    }
    let e = r.emu.emulator();
    // Solo la lectura del respaldo manual (`settings_export`) habló con la máquina.
    assert!(e.writes.is_empty() && e.deleted.is_empty() && e.ran.is_empty());
    assert!(e.received > before);
    drop(e);
    assert!(!dir.backups().exists());
    // Leer sigue funcionando sin la bandera.
    assert!(h.settings_read().is_ok() && h.files().is_ok() && h.info().is_ok());
    assert!(
        h.settings_preview(vec![change("g0_speed", 150_000.0)])
            .is_ok()
    );
}

#[test]
fn reading_the_settings_returns_every_listed_one_and_only_reads() {
    let r = rig(false);
    let s = r.handle.settings_read().unwrap();
    assert_eq!(s.model.as_deref(), Some("RDC6442S"));
    assert_eq!(s.card_id, "0x65106510");
    assert_eq!(s.settings.len(), tlaku_ruida::settings::WRITABLE.len());
    assert_eq!(value_of(&s, "g0_speed"), Some(200_000.0));
    assert_eq!(value_of(&s, "x_travel"), Some(600_000.0));
    let first = &s.settings[0];
    assert_eq!(
        (first.key.as_str(), first.addr.as_str()),
        ("g0_speed", "0x005")
    );
    assert_eq!(first.kind, "speed");
    assert!(r.emu.emulator().writes.is_empty());
}

#[test]
fn applying_needs_confirmation_and_writes_nothing_without_it() {
    let r = rig(false);
    let dir = TempDir::new();
    let err = r
        .handle
        .settings_apply(
            vec![change("g0_speed", 150_000.0)],
            false,
            dir.backups(),
            true,
        )
        .unwrap_err();
    assert!(err.contains("confirmación"), "{err}");
    assert!(r.emu.emulator().writes.is_empty());
    assert!(
        !dir.backups().exists(),
        "ni siquiera se creó la carpeta de respaldos"
    );
}

#[test]
fn applying_saves_a_dated_backup_writes_verifies_and_logs_it() {
    let r = rig(false);
    let dir = TempDir::new();
    let rep = r
        .handle
        .settings_apply(
            vec![
                change("g0_speed", 150_000.0),
                change("rotary_pulse", 4_000.0),
            ],
            true,
            dir.backups(),
            true,
        )
        .unwrap();
    assert!(rep.ok && !rep.restore_advised, "{rep:?}");
    assert_eq!(rep.changes.len(), 2);
    assert_eq!(
        (
            rep.changes[0].key.as_str(),
            rep.changes[0].current,
            rep.changes[0].new
        ),
        ("g0_speed", 200_000.0, 150_000.0)
    );
    assert!(rep.verified.iter().all(|v| v.ok));
    // El respaldo tiene la lectura de ANTES, con la fecha en el nombre.
    let path = rep.backup.clone().unwrap();
    assert!(
        path.contains("ruida-") && path.ends_with(".tlakuruida"),
        "{path}"
    );
    let text = std::fs::read_to_string(&path).unwrap();
    let file = tlaku_ruida::settings::parse_file(&text).unwrap();
    assert_eq!(file.card().unwrap(), 0x6510_6510);
    let g0 = file.settings.iter().find(|e| e.key == "g0_speed").unwrap();
    assert_eq!(g0.value, 200_000);
    // La máquina quedó con los valores nuevos.
    assert_eq!(
        value_of(&r.handle.settings_read().unwrap(), "g0_speed"),
        Some(150_000.0)
    );
    let logs = r.sink.logs();
    assert!(
        logs.contains("respaldo guardado") && logs.contains("verificado"),
        "{logs}"
    );
    // Solo se escribieron direcciones de la lista cerrada.
    assert!(
        r.emu
            .emulator()
            .writes
            .iter()
            .all(|(a, _, _)| tlaku_ruida::settings::find(*a).is_some())
    );
}

#[test]
fn unlisted_keys_and_values_are_refused_before_touching_the_machine() {
    let r = rig(false);
    let dir = TempDir::new();
    for bad in [
        change("laser_max_power", 100.0),
        change("g0_speed", 12.5),
        change("g0_speed", f64::NAN),
        change("g0_speed", 99_000_000.0),
        change("g0_speed", 0.0),
    ] {
        let err = r
            .handle
            .settings_apply(vec![bad.clone()], true, dir.backups(), true)
            .unwrap_err();
        assert!(!err.is_empty(), "{bad:?}");
    }
    assert!(r.emu.emulator().writes.is_empty());
    assert!(!dir.backups().exists());
}

#[test]
fn preview_shows_current_and_new_without_writing() {
    let r = rig(false);
    let plan = r
        .handle
        .settings_preview(vec![
            change("x_max_speed", 300_000.0),
            change("g0_speed", 200_000.0),
        ])
        .unwrap();
    assert_eq!(plan.unchanged, 1);
    assert_eq!(plan.changes.len(), 1);
    assert_eq!(plan.changes[0].current, 400_000.0);
    assert_eq!(plan.changes[0].new, 300_000.0);
    assert!(r.emu.emulator().writes.is_empty());
}

#[test]
fn a_machine_that_ignores_writes_is_detected_and_the_backup_restores_it() {
    let r = rig(false);
    let dir = TempDir::new();
    r.emu.emulator().set_faults(EmuFaults {
        corrupt_writes: true,
        ..Default::default()
    });
    let rep = r
        .handle
        .settings_apply(
            vec![change("home_speed", 30_000.0)],
            true,
            dir.backups(),
            true,
        )
        .unwrap();
    assert!(!rep.ok && rep.restore_advised);
    assert_eq!(rep.verified[0].actual, Some(30_001.0));
    // La máquina vuelve a portarse bien y se restaura el respaldo previo, con confirmación.
    r.emu.emulator().set_faults(EmuFaults::default());
    let backup = read_backup(&dir.backups(), &list_backups(&dir.backups())[0].name).unwrap();
    assert!(
        r.handle
            .settings_restore(backup.clone(), false, dir.backups(), true)
            .is_err()
    );
    let back = r
        .handle
        .settings_restore(backup, true, dir.backups(), true)
        .unwrap();
    assert!(back.ok, "{back:?}");
    assert_eq!(
        value_of(&r.handle.settings_read().unwrap(), "home_speed"),
        Some(20_000.0)
    );
    // Restaurar respaldó otra vez lo que había antes de restaurar: dos respaldos en total.
    assert_eq!(list_backups(&dir.backups()).len(), 2);
}

#[test]
fn export_preview_and_restore_a_manual_backup() {
    let r = rig(false);
    let dir = TempDir::new();
    let text = r.handle.settings_export().unwrap();
    assert!(tlaku_ruida::settings::parse_file(&text).is_ok());
    // Sin cambios: el respaldo coincide con la máquina.
    let same = r.handle.settings_preview_file(text.clone()).unwrap();
    assert!(same.changes.is_empty() && same.warnings.is_empty());
    r.handle
        .settings_apply(
            vec![change("x_start_speed", 5_000.0)],
            true,
            dir.backups(),
            true,
        )
        .unwrap();
    let diff = r.handle.settings_preview_file(text.clone()).unwrap();
    assert_eq!(diff.changes.len(), 1);
    assert_eq!(
        (diff.changes[0].current, diff.changes[0].new),
        (5_000.0, 8_000.0)
    );
    r.handle
        .settings_restore(text, true, dir.backups(), true)
        .unwrap();
    assert_eq!(
        value_of(&r.handle.settings_read().unwrap(), "x_start_speed"),
        Some(8_000.0)
    );
}

#[test]
fn a_file_from_another_card_warns_in_the_preview_and_is_refused_on_restore() {
    let r = rig(false);
    let dir = TempDir::new();
    let text = r
        .handle
        .settings_export()
        .unwrap()
        .replace("0x65106510", "0x90109010");
    let plan = r.handle.settings_preview_file(text.clone()).unwrap();
    assert_eq!(plan.warnings.len(), 1, "{plan:?}");
    let err = r
        .handle
        .settings_restore(text, true, dir.backups(), true)
        .unwrap_err();
    assert!(err.contains("modelos distintos"), "{err}");
    assert!(r.emu.emulator().writes.is_empty());
    // Y un texto que no es un respaldo se rechaza sin tocar nada.
    assert!(r.handle.settings_preview_file("{}".into()).is_err());
    assert!(
        r.handle
            .settings_restore("basura".into(), true, dir.backups(), true)
            .is_err()
    );
}

#[test]
fn settings_are_refused_while_a_job_runs() {
    let r = rig(true);
    r.start().unwrap();
    let dir = TempDir::new();
    let err = r.handle.settings_read().unwrap_err();
    assert!(!err.is_empty());
    assert!(
        r.handle
            .settings_apply(
                vec![change("g0_speed", 150_000.0)],
                true,
                dir.backups(),
                true
            )
            .is_err()
    );
    assert!(
        r.emu.emulator().writes.is_empty() || r.emu.emulator().writes.iter().all(|w| w.0 == 0x320)
    );
}

#[test]
fn calibrating_returns_a_proposal_and_validates_the_measure() {
    let r = rig(false);
    let c = r
        .handle
        .calibrate("x_step".into(), 100.0, 99.0, true)
        .unwrap();
    assert_eq!((c.current, c.proposed), (6350.0, 6287.0));
    assert!((c.steps_per_mm_after - 1000.0 / 6287.0).abs() < 1e-9);
    assert!(
        r.handle
            .calibrate("x_step".into(), 100.0, 10.0, true)
            .is_err()
    );
    assert!(
        r.handle
            .calibrate("g0_speed".into(), 100.0, 99.0, true)
            .is_err()
    );
    // Calibrar solo propone: no escribe.
    assert!(r.emu.emulator().writes.is_empty());
    // Y la propuesta pasa por el mismo camino de escritura.
    let dir = TempDir::new();
    let rep = r
        .handle
        .settings_apply(
            vec![change("x_step", c.proposed)],
            true,
            dir.backups(),
            true,
        )
        .unwrap();
    assert!(rep.ok);
}

#[test]
fn automatic_backups_are_listed_newest_first_and_names_cannot_escape_the_folder() {
    let r = rig(false);
    let dir = TempDir::new();
    for v in [150_000.0, 140_000.0] {
        r.handle
            .settings_apply(vec![change("g0_speed", v)], true, dir.backups(), true)
            .unwrap();
        std::thread::sleep(Duration::from_millis(1100));
    }
    let list = list_backups(&dir.backups());
    assert_eq!(list.len(), 2);
    assert!(list[0].name > list[1].name);
    assert_eq!(list[0].card_id, "0x65106510");
    assert!(read_backup(&dir.backups(), &list[0].name).is_ok());
    for bad in [
        "../x.tlakuruida",
        "a/b.tlakuruida",
        "x.txt",
        "",
        "..",
        "a\\b.tlakuruida",
    ] {
        assert!(read_backup(&dir.backups(), bad).is_err(), "{bad}");
    }
    // Un archivo ajeno en la carpeta no estorba.
    std::fs::write(dir.backups().join("notas.txt"), "hola").unwrap();
    std::fs::write(dir.backups().join("roto.tlakuruida"), "no es json").unwrap();
    assert_eq!(list_backups(&dir.backups()).len(), 2);
}

#[test]
fn two_backups_in_the_same_second_do_not_overwrite_each_other() {
    let dir = TempDir::new();
    let snap = tlaku_ruida::settings::SettingsSnapshot {
        card_id: 0x6510_6510,
        model: None,
        values: Vec::new(),
    };
    let a = super::backup::save_snapshot(&dir.backups(), &snap, 1_700_000_000).unwrap();
    let b = super::backup::save_snapshot(&dir.backups(), &snap, 1_700_000_000).unwrap();
    assert_ne!(a, b);
    assert!(a.ends_with("ruida-20231114-221320.tlakuruida"), "{a}");
    assert!(b.ends_with("ruida-20231114-221320-2.tlakuruida"), "{b}");
}

// ---------------------------------------------------------------------------
// Documentos: nombre, reemplazo, borrar y ejecutar
// ---------------------------------------------------------------------------

fn files(r: &Rig) -> Vec<String> {
    r.handle
        .files()
        .unwrap()
        .into_iter()
        .map(|f| f.name.unwrap_or_default())
        .collect()
}

#[test]
fn the_chosen_name_is_used_and_invalid_ones_are_refused() {
    let r = rig(false);
    r.handle
        .prepare_store(Some("MIPLAN".into()), false, true)
        .unwrap();
    r.start().unwrap();
    assert_eq!(files(&r), ["MIPLAN"]);
    assert!(r.sink.job_messages().iter().any(|m| m.contains("MIPLAN")));
    for bad in ["", "tiene espacio", "NOMBRELARGO", "ñandú"] {
        assert!(
            r.handle
                .prepare_store(Some(bad.into()), false, true)
                .is_err(),
            "{bad}"
        );
    }
    // Sin nombre elegido vuelve el automático.
    r.handle.prepare_store(None, false, true).unwrap();
    r.start().unwrap();
    assert!(files(&r)[1].starts_with("TLAKU"));
}

#[test]
fn a_repeated_name_fails_without_touching_anything_unless_overwriting_was_confirmed() {
    let r = rig(false);
    r.handle
        .prepare_store(Some("PIEZA".into()), false, true)
        .unwrap();
    r.start().unwrap();
    // Mismo nombre sin confirmar: error claro y el documento viejo sigue.
    r.handle
        .prepare_store(Some("PIEZA".into()), false, true)
        .unwrap();
    let err = r.start().unwrap_err();
    assert!(err.contains("PIEZA") && err.contains("ya hay"), "{err}");
    assert_eq!(files(&r), ["PIEZA"]);
    assert!(r.emu.emulator().deleted.is_empty());
    // Confirmado: se borra el viejo y queda uno solo con ese nombre.
    r.handle
        .prepare_store(Some("PIEZA".into()), true, true)
        .unwrap();
    r.start().unwrap();
    assert_eq!(files(&r), ["PIEZA"]);
    assert_eq!(r.emu.emulator().deleted, vec![1]);
    // El plan valía para UN envío: el siguiente usa el nombre automático y no pisa nada.
    r.start().unwrap();
    let names = files(&r);
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(names[1].starts_with("TLAKU"), "{names:?}");
    assert_eq!(r.emu.emulator().deleted, vec![1]);
}

#[test]
fn a_stale_plan_is_ignored_so_a_cancelled_dialog_cannot_overwrite_later() {
    let r = rig(false);
    r.handle
        .prepare_store(Some("VIEJO".into()), false, true)
        .unwrap();
    r.start().unwrap();
    // El diálogo dejó "reemplazar VIEJO" preparado, pero pasaron más de dos minutos.
    r.handle
        .prepare_store(Some("VIEJO".into()), true, true)
        .unwrap();
    r.handle.shared.store.lock().unwrap().at = Instant::now().checked_sub(Duration::from_secs(300));
    r.start().unwrap();
    let names = files(&r);
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(r.emu.emulator().deleted.is_empty());
}

#[test]
fn deleting_a_document_asks_for_confirmation_and_checks_its_name() {
    let r = rig(false);
    for n in ["UNO", "DOS"] {
        r.handle.prepare_store(Some(n.into()), false, true).unwrap();
        r.start().unwrap();
    }
    assert!(r.handle.file_delete(1, "UNO".into(), false, true).is_err());
    assert!(r.handle.file_delete(1, "DOS".into(), true, true).is_err());
    assert_eq!(files(&r), ["UNO", "DOS"]);
    let res = r.handle.file_delete(1, "UNO".into(), true, true).unwrap();
    assert!(res.confirmed);
    assert_eq!(files(&r), ["DOS"]);
    assert!(r.sink.logs().contains("Borrando el documento 1"));
}

#[test]
fn running_a_stored_document_follows_the_job_like_any_other() {
    let r = rig(false);
    r.handle
        .prepare_store(Some("CORTE".into()), false, true)
        .unwrap();
    r.start().unwrap();
    assert!(r.handle.file_run(1, "CORTE".into(), false, true).is_err());
    assert!(r.handle.file_run(1, "OTRO".into(), true, true).is_err());
    assert!(r.emu.emulator().ran.is_empty());
    let res = r.handle.file_run(1, "CORTE".into(), true, true).unwrap();
    assert!(res.confirmed);
    assert_eq!(r.emu.emulator().ran, ["CORTE"]);
    r.sink.wait("trabajo en marcha", |c| {
        c.jobs().contains(&JobState::Started)
    });
    // Se puede detener como cualquier trabajo.
    r.handle.cancel().unwrap();
    r.sink
        .wait("cancelado", |c| c.jobs().contains(&JobState::Cancelled));
}

// ---------------------------------------------------------------------------
// Máquina virtual
// ---------------------------------------------------------------------------

#[test]
fn the_virtual_machine_supports_settings_documents_and_backups() {
    let sink = Arc::new(Collect::default());
    let (conn, _) = RuidaConnection::connect(
        &device(false),
        "virtual:ruida@100",
        sink.clone() as Arc<dyn Sink>,
        fast(),
    )
    .unwrap();
    let h = conn.handle();
    let dir = TempDir::new();
    let s = h.settings_read().unwrap();
    // La cama virtual sale del perfil.
    assert_eq!(value_of(&s, "x_travel"), Some(600_000.0));
    let rep = h
        .settings_apply(
            vec![change("y_max_accel", 2_000_000.0)],
            true,
            dir.backups(),
            true,
        )
        .unwrap();
    assert!(rep.ok);
    assert_eq!(list_backups(&dir.backups()).len(), 1);
    // Documentos: guardar con nombre, ejecutar y borrar.
    h.prepare_store(Some("VIRTUAL".into()), false, true)
        .unwrap();
    h.start_job(&square_doc(), &device(false), &JobOptions::default())
        .unwrap();
    assert_eq!(h.files().unwrap()[0].name.as_deref(), Some("VIRTUAL"));
    assert!(
        h.file_run(1, "VIRTUAL".into(), true, true)
            .unwrap()
            .confirmed
    );
    // El primer «terminado» fue el de guardar; el segundo, el de ejecutar el documento.
    sink.wait("fin del documento virtual", |c| {
        c.jobs()
            .iter()
            .filter(|j| **j == JobState::Finished)
            .count()
            >= 2
    });
    assert!(
        h.file_delete(1, "VIRTUAL".into(), true, true)
            .unwrap()
            .confirmed
    );
    assert!(h.files().unwrap().is_empty());
    conn.close();
}
