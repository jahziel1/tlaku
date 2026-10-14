//! Pruebas del punto de control sin máquina: resumen, vista, reanudación encadenada y escritor.

use super::store::{CheckpointProgress, CheckpointStore, Loaded};
use super::*;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use tlaku_doc::{DeviceKind, ResumeModal};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tlaku-resume-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    d
}

/// Dos filas de 20 mm y un cuadrado: 16 líneas con comentarios y una vacía.
fn sample() -> Vec<String> {
    "; Generado por Tlaku\nG90\nG21\nM4 S0\n\nG0 X0 Y0\nG1 X20 S500 F3000\nG0 X20 Y2\nG1 X0\nG0 X30 Y0\nG1 X40\nG1 Y10\nG1 X30\nG1 Y0\nM5\nG0 X0 Y0"
        .lines()
        .map(str::to_string)
        .collect()
}

fn device() -> DeviceProfile {
    DeviceProfile::for_kind(DeviceKind::Grbl)
}

fn meta(id: &str, lines: &[String]) -> CheckpointMeta {
    CheckpointMeta {
        version: VERSION,
        id: id.into(),
        kind: ResumeKind::Gcode,
        started_at_ms: 10,
        document: Some("pieza.tlaku".into()),
        device: device(),
        port: "virtual:falcon2".into(),
        origin: None,
        total_lines: plan::sendable_count(lines) as u32,
        resume: None,
    }
}

fn progress(acked: u32, sent_line: Option<u32>, wpos: Option<[f64; 2]>) -> CheckpointProgress {
    CheckpointProgress {
        version: VERSION,
        seq: 1,
        acked,
        sent_line,
        mpos: wpos,
        wpos,
        wco: Some([0.0, 0.0]),
        planner_free: Some(15),
        rx_free: Some(127),
        planner_cap: Some(15),
        rx_cap: Some(127),
        modal: ResumeModal::default(),
        t_ms: 99,
    }
}

fn saved(name: &str, lines: &[String]) -> (Arc<CheckpointStore>, PathBuf) {
    let dir = tmp(name);
    let store = Arc::new(CheckpointStore::new(&dir));
    store.begin(&meta("a", lines), Some(lines)).unwrap();
    (store, dir)
}

#[test]
fn summary_reports_percent_line_and_whether_it_can_resume() {
    let lines = sample();
    let (store, dir) = saved("summary", &lines);
    // 7 `ok` de 14 líneas enviables (la 0 y la 4 no se envían).
    store
        .write_progress(&progress(7, Some(8), Some([15.0, 2.0])))
        .unwrap();
    let j = summarize(&store.load().unwrap());
    assert_eq!((j.done_lines, j.total_lines), (7, 14));
    assert!((j.percent - 50.0).abs() < 1e-9);
    assert!(j.can_resume && j.note.is_none() && j.progress_known);
    assert_eq!(j.mpos, Some([15.0, 2.0]));
    assert_eq!(j.document.as_deref(), Some("pieza.tlaku"));
    // Sin avance legible: se puede continuar eligiendo a mano.
    fs::write(dir.join("progress.json"), b"{\"version\":1,").unwrap();
    let j = summarize(&store.load().unwrap());
    assert!(j.can_resume && !j.progress_known && j.note.is_some());
    assert_eq!(j.done_lines, 0);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn markers_of_other_machines_explain_why_they_cannot_resume() {
    let dir = tmp("markers");
    let store = CheckpointStore::new(&dir);
    for (dk, kind, word) in [
        (DeviceKind::Ruida, ResumeKind::Ruida, "controladora"),
        (DeviceKind::LhyMicro, ResumeKind::K40, "K40"),
        (DeviceKind::Galvo, ResumeKind::Galvo, "galvo"),
    ] {
        let dev = DeviceProfile::for_kind(dk);
        write_marker(&store, &dev, &JobOptions::default(), None, "").unwrap();
        let j = summarize(&store.load().unwrap());
        assert_eq!(j.kind, kind);
        assert!(!j.can_resume);
        assert!(j.note.unwrap().contains(word), "{kind:?}");
        // Al terminar el trabajo el marcador se borra; el de G-code no lo toca esta función.
        assert!(clear_marker_on_end(&store));
        assert!(store.load().is_none());
    }
    let lines = sample();
    store.begin(&meta("g", &lines), Some(&lines)).unwrap();
    assert!(!clear_marker_on_end(&store));
    assert!(store.load().is_some());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn view_colors_what_is_done_and_suggests_a_safe_point() {
    let lines = sample();
    let (store, dir) = saved("view", &lines);
    // Todo hasta la línea 10 (`G1 X40`) tiene `ok`; la cabeza va por X35 de la segunda figura.
    store
        .write_progress(&progress(9, Some(10), Some([35.0, 0.0])))
        .unwrap();
    let v = view(&store, &store.load().unwrap(), &PlanOptions::default()).unwrap();
    assert_eq!(v.suggested.executing_line, 10);
    assert!(!v.suggested.mismatch);
    // Retrocede al menos hasta el principio del trazo (la línea 10 sigue al G0 de la 9).
    assert!(v.suggested.line <= 10 && v.suggested.line >= 9);
    // `moves` empieza en el punto de partida y cada movimiento sabe su línea.
    assert_eq!(v.moves.len() / 4, v.move_lines.len());
    assert_eq!(
        v.move_lines[v.suggested.move_index as usize],
        v.suggested.line
    );
    assert!(v.last_acked_move >= v.suggested.move_index);
    assert_eq!(v.bounds, [[0.0, 0.0], [40.0, 10.0]]);
    assert!(v.suggested.percent > 0.0 && v.suggested.percent < 100.0);
    // Los cortes llevan la potencia en por ciento de S máx (500 de 1000).
    let first_cut = (0..v.moves.len() / 4)
        .find(|&i| v.moves[i * 4] == 1.0)
        .unwrap();
    assert!((v.moves[first_cut * 4 + 3] - 50.0).abs() < 1e-3);
    // Una posición que no está en ninguna línea da una estimación prudente y lo avisa.
    store
        .write_progress(&progress(9, Some(10), Some([200.0, 200.0])))
        .unwrap();
    let v = view(&store, &store.load().unwrap(), &PlanOptions::default()).unwrap();
    assert!(v.suggested.mismatch);
    assert!(!v.warnings.is_empty());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn nothing_acked_restarts_from_the_beginning() {
    let lines = sample();
    let (store, dir) = saved("zero", &lines);
    store.write_progress(&progress(0, None, None)).unwrap();
    let v = view(&store, &store.load().unwrap(), &PlanOptions::default()).unwrap();
    assert_eq!(v.suggested.line, 0);
    assert!(!v.suggested.mismatch);
    assert_eq!(summarize(&store.load().unwrap()).percent, 0.0);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn prepare_builds_the_preamble_and_maps_a_second_interruption_to_the_original() {
    let lines = sample();
    let (store, dir) = saved("chain", &lines);
    store
        .write_progress(&progress(9, Some(10), Some([35.0, 0.0])))
        .unwrap();
    let l = store.load().unwrap();
    let args = ResumeStart {
        id: "a".into(),
        line: 10,
        at: None,
        confirm_unmoved: true,
        framed_ok: true,
        checklist_ok: true,
    };
    let job = prepare(store.clone(), &l, &args).unwrap();
    let run = job.plan.meta.resume.clone().unwrap();
    assert_eq!(run.tail_first, 10);
    assert_eq!(run.from_id, "a");
    assert_ne!(job.plan.meta.id, "a");
    // El preámbulo ordena M5 primero, va con G0 al punto y enciende después.
    let pre = &job.lines[..run.preamble_len as usize];
    assert_eq!(pre[1], "M5");
    let g0 = pre.iter().position(|l| l.starts_with("G0 X30 Y0")).unwrap();
    let m4 = pre.iter().position(|l| l.starts_with("M4 S500")).unwrap();
    assert!(g0 < m4);
    assert_eq!(&job.lines[run.preamble_len as usize..], &lines[10..]);
    assert_eq!(
        job.plan.original.as_ref().unwrap().as_slice(),
        lines.as_slice()
    );

    // Segunda interrupción: el punto de control del trabajo reanudado guarda el original y
    // traduce su avance a líneas del original.
    let again = Arc::new(CheckpointStore::new(tmp("chain2")));
    again.begin(&job.plan.meta, Some(&lines)).unwrap();
    let sendable_pre = run.preamble_sendable;
    // Dos `ok` del resto del trabajo: la primera línea del resto es la 10.
    let sent_line = run.preamble_len + 1;
    again
        .write_progress(&progress(
            sendable_pre + 2,
            Some(sent_line),
            Some([40.0, 5.0]),
        ))
        .unwrap();
    let l2 = again.load().unwrap();
    let j = summarize(&l2);
    assert!(j.resumed);
    assert_eq!(j.done_lines, run.tail_sendable_before + 2);
    assert_eq!(last_original_line(&l2), Some(11));
    let sug = suggest(&Parsed::parse(&lines), &l2, &PlanOptions::default());
    assert_eq!(sug.exec_line, 11);
    assert!(sug.line >= 10, "no retrocede a lo que ya se había grabado");
    // Si no llegó ni al resto del trabajo, se repite el mismo punto.
    again
        .write_progress(&progress(1, Some(2), Some([30.0, 0.0])))
        .unwrap();
    let l3 = again.load().unwrap();
    assert_eq!(last_original_line(&l3), None);
    assert_eq!(
        suggest(&Parsed::parse(&lines), &l3, &PlanOptions::default()).line,
        10
    );
    let _ = fs::remove_dir_all(tmp("chain2"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn prepare_refuses_gcode_that_changes_the_coordinate_system() {
    let mut lines = sample();
    lines.insert(3, "G92 X5".into());
    let (store, dir) = saved("g92", &lines);
    let l: Loaded = store.load().unwrap();
    let args = ResumeStart {
        id: "a".into(),
        line: 8,
        at: None,
        confirm_unmoved: true,
        framed_ok: true,
        checklist_ok: true,
    };
    assert!(prepare(store, &l, &args).is_err());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn the_writer_does_not_block_the_sender_and_saves_ack_status_and_modal_state() {
    let lines = sample();
    let dir = tmp("writer");
    let store = Arc::new(CheckpointStore::new(&dir));
    let sent = Arc::new(lines.clone());
    let w = CheckpointWriter::start(
        store.clone(),
        meta("w", &lines),
        sent.clone(),
        sent,
        Duration::from_millis(20),
    );
    // Anotar es barato aunque el disco tarde: nada de esto toca el disco.
    let t0 = Instant::now();
    for i in 0..20_000 {
        w.note_acked(i % 8);
        w.note_status(StatusSnap {
            mpos: [1.0, 2.0],
            wpos: [1.0, 2.0],
            wco: [0.0, 0.0],
            planner_free: 10,
            rx_free: 100,
        });
    }
    assert!(
        t0.elapsed() < Duration::from_millis(500),
        "{:?}",
        t0.elapsed()
    );
    w.note_acked(7);
    w.finish(Finish::Keep, &store);
    let l = store.load().unwrap();
    let p = l.progress.unwrap();
    assert_eq!(p.acked, 7);
    assert_eq!(p.sent_line, Some(8));
    assert_eq!(p.mpos, Some([1.0, 2.0]));
    assert_eq!(p.planner_cap, Some(10));
    // Estado modal tras las 7 primeras líneas enviables (hasta `G1 X0`).
    assert_eq!(p.modal.spindle, tlaku_doc::ResumeSpindle::M4);
    assert_eq!((p.modal.s, p.modal.f), (Some(500.0), Some(3000.0)));
    assert_eq!(store.read_gcode().unwrap(), lines);

    // Terminar bien (o cancelar a propósito) lo borra todo.
    let sent = Arc::new(lines.clone());
    let w = CheckpointWriter::start(
        store.clone(),
        meta("w2", &lines),
        sent.clone(),
        sent,
        Duration::from_millis(20),
    );
    w.note_acked(3);
    // El hilo guarda el G-code y `meta.json` con `sync_all`: en discos lentos (Windows en la CI)
    // tarda más de unas decenas de ms. Se espera a que aparezca, sin un plazo fijo corto.
    let deadline = Instant::now() + Duration::from_secs(10);
    while store.load().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    if store.load().is_none() {
        let err = w.take_error();
        let listing = fs::read_dir(&dir)
            .map(|d| {
                d.flatten()
                    .map(|e| format!("{:?} {:?}", e.file_name(), e.metadata().map(|m| m.len())))
                    .collect::<Vec<_>>()
            })
            .map_err(|e| e.to_string());
        let probe = store.begin(&meta("probe", &lines), Some(&lines));
        panic!("DIAG err={err:?} dir={listing:?} probe={probe:?}");
    }
    w.finish(Finish::Discard, &store);
    assert!(store.load().is_none());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_dropped_writer_keeps_the_checkpoint() {
    let lines = sample();
    let dir = tmp("dropped");
    let store = Arc::new(CheckpointStore::new(&dir));
    let sent = Arc::new(lines.clone());
    let w = CheckpointWriter::start(
        store.clone(),
        meta("d", &lines),
        sent.clone(),
        sent,
        Duration::from_millis(20),
    );
    w.note_acked(2);
    drop(w);
    let l = store.load().unwrap();
    assert_eq!(l.meta.id, "d");
    assert_eq!(l.progress.unwrap().acked, 2);
    let _ = fs::remove_dir_all(dir);
}
