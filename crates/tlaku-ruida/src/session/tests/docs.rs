//! Documentos de la memoria: borrar, ejecutar y reemplazar por nombre.

use super::*;
use crate::session::{DeleteReport, valid_doc_name};

fn store(s: &mut RuidaSession<MemoryLink>, name: &str) {
    s.send_job(
        &square_job(),
        &SendMode::Store { name: name.into() },
        &AtomicBool::new(false),
        &mut no_progress(),
    )
    .unwrap();
}

fn names(s: &mut RuidaSession<MemoryLink>) -> Vec<String> {
    s.list_documents()
        .unwrap()
        .into_iter()
        .map(|f| f.name.unwrap_or_default())
        .collect()
}

fn three() -> (
    RuidaSession<MemoryLink>,
    std::sync::Arc<std::sync::Mutex<Emulator>>,
) {
    let (mut s, _l, emu) = mem_session(EmuConfig::default(), fast_cfg());
    for n in ["UNO", "DOS", "TRES"] {
        store(&mut s, n);
    }
    (s, emu)
}

#[test]
fn deleting_requires_confirmation_and_the_right_name() {
    let (mut s, emu) = three();
    assert!(matches!(
        s.delete_document(2, "DOS", false),
        Err(SessionError::NotConfirmed)
    ));
    // El número 2 no es "TRES": no se borra nada.
    let err = s.delete_document(2, "TRES", true).unwrap_err();
    assert!(matches!(
        err,
        SessionError::DocumentMismatch { index: 2, .. }
    ));
    assert!(emu.lock().unwrap().deleted.is_empty());
    assert_eq!(names(&mut s), ["UNO", "DOS", "TRES"]);
}

#[test]
fn deleting_removes_exactly_that_document_and_verifies_the_count() {
    let (mut s, emu) = three();
    let r = s.delete_document(2, "dos", true).unwrap();
    assert_eq!(
        r,
        DeleteReport {
            index: 2,
            name: "dos".into(),
            count_before: 3,
            count_after: Some(2)
        }
    );
    assert!(r.verified());
    assert_eq!(names(&mut s), ["UNO", "TRES"]);
    let e = emu.lock().unwrap();
    assert_eq!(e.deleted, vec![2]);
    // La orden salió con el formato documentado: E8 00 n(2) 00 00.
    assert!(
        e.stream()
            .windows(6)
            .any(|w| w == [0xE8, 0x00, 0x00, 0x02, 0x00, 0x00])
    );
}

#[test]
fn delete_all_is_never_sent_and_bad_indexes_are_refused() {
    let (mut s, emu) = three();
    for (i, n) in [(0, "UNO"), (4, "X"), (100, "X")] {
        assert!(matches!(
            s.delete_document(i, n, true),
            Err(SessionError::NoSuchDocument { .. })
        ));
    }
    let e = emu.lock().unwrap();
    assert!(e.deleted.is_empty());
    // Ningún E8 00 en todo lo que llegó.
    assert!(!e.stream().windows(2).any(|w| w == [0xE8, 0x00]));
}

#[test]
fn a_busy_machine_neither_deletes_nor_runs() {
    let (mut s, emu) = three();
    emu.lock().unwrap().running = true;
    assert!(matches!(
        s.delete_document(1, "UNO", true),
        Err(SessionError::Busy(_))
    ));
    assert!(matches!(
        s.run_document(1, "UNO", true),
        Err(SessionError::Busy(_))
    ));
    let e = emu.lock().unwrap();
    assert!(e.deleted.is_empty() && e.ran.is_empty());
}

#[test]
fn running_a_document_needs_confirmation_and_the_right_name_and_then_starts() {
    let (mut s, emu) = three();
    assert!(matches!(
        s.run_document(3, "TRES", false),
        Err(SessionError::NotConfirmed)
    ));
    assert!(matches!(
        s.run_document(3, "UNO", true),
        Err(SessionError::DocumentMismatch { .. })
    ));
    assert!(emu.lock().unwrap().ran.is_empty());
    let r = s.run_document(3, "TRES", true).unwrap();
    assert!(r.started);
    assert_eq!(emu.lock().unwrap().ran, ["TRES"]);
    assert!(
        emu.lock()
            .unwrap()
            .stream()
            .windows(4)
            .any(|w| w == [0xE8, 0x03, 0x00, 0x03])
    );
    // Y se puede detener como cualquier trabajo.
    s.stop().unwrap();
    assert!(s.status().unwrap().is_idle());
}

#[test]
fn name_checks_find_the_document_ignoring_case() {
    let (mut s, _emu) = three();
    assert_eq!(s.find_document("dos").unwrap(), Some(2));
    assert_eq!(s.find_document("NOPE").unwrap(), None);
}

#[test]
fn storing_with_an_existing_name_asks_first_and_replaces_only_when_allowed() {
    let (mut s, emu) = three();
    // Sin permiso: error y nada borrado.
    let err = s.prepare_store("DOS", false).unwrap_err();
    assert!(matches!(err, SessionError::NameExists { index: 2, .. }));
    assert!(emu.lock().unwrap().deleted.is_empty());
    // Con permiso: borra el viejo y deja el nombre libre.
    assert_eq!(s.prepare_store("DOS", true).unwrap(), Some(2));
    store(&mut s, "DOS");
    assert_eq!(names(&mut s), ["UNO", "TRES", "DOS"]);
    // Un nombre nuevo no borra nada.
    assert_eq!(s.prepare_store("CUATRO", true).unwrap(), None);
    assert_eq!(emu.lock().unwrap().deleted, vec![2]);
}

#[test]
fn document_names_are_short_ascii_identifiers() {
    for ok in ["TLAKU001", "A", "mi-plan1"] {
        assert!(valid_doc_name(ok), "{ok}");
    }
    for bad in ["", "NOMBRE-LARGO", "con espacio", "ñ", "a/b", "A.rd"] {
        assert!(!valid_doc_name(bad), "{bad}");
    }
    let (mut s, _e) = three();
    assert!(s.prepare_store("mal nombre", true).is_err());
}

#[test]
fn the_emulator_keeps_duplicates_when_the_same_name_is_stored_twice() {
    // Hipótesis del emulador: no hay sobrescritura implícita.
    let (mut s, _l, _e) = mem_session(EmuConfig::default(), fast_cfg());
    store(&mut s, "REPE");
    store(&mut s, "REPE");
    assert_eq!(names(&mut s), ["REPE", "REPE"]);
}
