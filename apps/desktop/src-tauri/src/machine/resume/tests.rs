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
        sent: acked,
        sent_to_line: sent_line,
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
    // 7 `ok` de 14 líneas enviables (la 0 y la 4 no se envían). El porcentaje sale de la posición:
    // la cabeza va a 5 mm de la segunda fila, o sea 25 de 80 mm de grabado.
    store
        .write_progress(&progress(7, Some(8), Some([15.0, 2.0])))
        .unwrap();
    let j = summarize(&store, &store.load().unwrap(), None);
    assert_eq!((j.done_lines, j.total_lines), (7, 14));
    assert!((j.percent - 31.25).abs() < 1e-6, "{}", j.percent);
    assert!(j.can_resume && j.note.is_none() && j.progress_known);
    assert_eq!(j.mpos, Some([15.0, 2.0]));
    assert_eq!(j.document.as_deref(), Some("pieza.tlaku"));
    // Sin avance legible: se puede continuar eligiendo a mano.
    fs::write(dir.join("progress.json"), b"{\"version\":1,").unwrap();
    let j = summarize(&store, &store.load().unwrap(), None);
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
        let j = summarize(&store, &store.load().unwrap(), None);
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
    let v = view(
        &store,
        &store.load().unwrap(),
        &PlanOptions::default(),
        None,
    )
    .unwrap();
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
    let v = view(
        &store,
        &store.load().unwrap(),
        &PlanOptions::default(),
        None,
    )
    .unwrap();
    assert!(v.suggested.mismatch);
    assert!(!v.warnings.is_empty());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn nothing_acked_restarts_from_the_beginning() {
    let lines = sample();
    let (store, dir) = saved("zero", &lines);
    store.write_progress(&progress(0, None, None)).unwrap();
    let v = view(
        &store,
        &store.load().unwrap(),
        &PlanOptions::default(),
        None,
    )
    .unwrap();
    assert_eq!(v.suggested.line, 0);
    assert!(!v.suggested.mismatch);
    assert_eq!(summarize(&store, &store.load().unwrap(), None).percent, 0.0);
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
    let job = prepare(store.clone(), &l, &args, None).unwrap();
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
    let j = summarize(&store, &l2, None);
    assert!(j.resumed);
    assert_eq!(j.done_lines, run.tail_sendable_before + 2);
    assert_eq!(assess::last_acked_line(&l2), Some(11));
    let sug = suggest(&Parsed::parse(&lines), &l2, &PlanOptions::default(), None);
    assert_eq!(sug.exec_line, 11);
    assert!(sug.line >= 10, "no retrocede a lo que ya se había grabado");
    // Si no llegó ni al resto del trabajo, se repite el mismo punto.
    again
        .write_progress(&progress(1, Some(2), Some([30.0, 0.0])))
        .unwrap();
    let l3 = again.load().unwrap();
    assert_eq!(assess::last_acked_line(&l3), None);
    assert_eq!(
        suggest(&Parsed::parse(&lines), &l3, &PlanOptions::default(), None).line,
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
    assert!(prepare(store, &l, &args, None).is_err());
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
    assert!(
        store.load().is_some(),
        "no apareció el punto de control; error de escritura: {:?}",
        w.take_error()
    );
    w.finish(Finish::Discard, &store);
    assert!(store.load().is_none());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn the_writer_learns_the_buffer_capacity_from_every_report_not_only_from_the_ones_it_saves() {
    // Disco lento: el primer guardado llega cuando el planificador ya está lleno (`planner_free`
    // 0). La capacidad se vio en un reporte anterior, que no se guardó, y no debe perderse: sin
    // ella la ventana de búsqueda queda mínima y la posición no encaja (`Unknown`).
    let lines = sample();
    let dir = tmp("writer-cap");
    let store = Arc::new(CheckpointStore::new(&dir));
    let sent = Arc::new(lines.clone());
    let w = CheckpointWriter::start(
        store.clone(),
        meta("cap", &lines),
        sent.clone(),
        sent,
        // Intervalo largo: ningún guardado ocurre entre los dos reportes.
        Duration::from_secs(3600),
    );
    let snap = |planner_free| StatusSnap {
        mpos: [1.0, 2.0],
        wpos: [1.0, 2.0],
        wco: [0.0, 0.0],
        planner_free,
        rx_free: 100,
    };
    w.note_status(snap(180));
    w.note_status(snap(0));
    w.note_acked(5);
    w.finish(Finish::Keep, &store);
    let p = store.load().unwrap().progress.unwrap();
    assert_eq!(p.planner_free, Some(0));
    assert_eq!(p.planner_cap, Some(180));
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

// ---------------------------------------------------------------------------
// La máquina siguió sola después de que la app murió
// ---------------------------------------------------------------------------

/// Cuadrado de 80 mm en (100,100) a 800 mm/min con 0 % de potencia y diez líneas: todo entra al
/// búfer de la máquina al principio.
fn square() -> Vec<String> {
    "G90\nG21\nM4 S0\nG0 X100 Y100\nG1 X180 F800\nG1 Y180\nG1 X100\nG1 Y100\nM5\nG0 X0 Y0"
        .lines()
        .map(str::to_string)
        .collect()
}

/// El punto de control de la prueba real: 8 `ok` de 10, las 10 entregadas, la cabeza en (128.1, 100).
fn square_checkpoint(name: &str) -> (Arc<CheckpointStore>, PathBuf) {
    let lines = square();
    let (store, dir) = saved(name, &lines);
    let mut p = progress(8, Some(7), Some([128.1, 100.0]));
    p.sent = 10;
    p.sent_to_line = Some(9);
    store.write_progress(&p).unwrap();
    (store, dir)
}

fn now(at: [f64; 2], valid: bool) -> assess::Now {
    assess::Now {
        mpos: at,
        wpos: at,
        valid,
        // El `F800` del cuadrado: la placa no se reinició.
        parser_feed: Some(800.0),
    }
}

#[test]
fn the_percent_comes_from_the_position_on_the_path_not_from_the_oks() {
    let (store, dir) = square_checkpoint("percent-pos");
    let j = summarize(&store, &store.load().unwrap(), None);
    // `done_lines` sigue siendo el dato interno (8 de 10) pero el porcentaje es el recorrido:
    // 28,1 de 320 mm de grabado.
    assert_eq!((j.done_lines, j.total_lines), (8, 10));
    assert!(
        (j.percent - 28.1 / 320.0 * 100.0).abs() < 0.01,
        "{}",
        j.percent
    );
    assert!(j.percent < 10.0);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn without_a_machine_to_compare_it_always_warns_how_much_it_could_have_run_alone() {
    let (store, dir) = square_checkpoint("no-machine");
    let j = summarize(&store, &store.load().unwrap(), None);
    assert_eq!(j.machine, ResumeMachine::NotChecked);
    assert!(j.can_resume && !j.recommend_discard);
    // Entregadas: lo que va desde la línea en ejecución (4) hasta la 9 son 6 líneas, ~292 mm.
    assert_eq!(j.ahead_lines, 6);
    assert!(
        (j.ahead_mm - (51.9 + 80.0 + 80.0 + 80.0 + 141.4)).abs() < 0.2,
        "{}",
        j.ahead_mm
    );
    let w = j.warning.unwrap();
    assert!(
        w.contains("pudo seguir ejecutando") && w.contains("6 líneas"),
        "{w}"
    );
    // Lo mismo si la posición de la máquina no es fiable (alarma, reinicio).
    let j = summarize(
        &store,
        &store.load().unwrap(),
        Some(&now([0.0, 0.0], false)),
    );
    assert_eq!(j.machine, ResumeMachine::Unknown);
    assert!(j.warning.unwrap().contains("pudo seguir ejecutando"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_machine_that_did_not_move_has_nothing_to_warn_about() {
    let (store, dir) = square_checkpoint("unmoved");
    let j = summarize(
        &store,
        &store.load().unwrap(),
        Some(&now([128.2, 100.0], true)),
    );
    assert_eq!(j.machine, ResumeMachine::Unmoved);
    assert!(j.warning.is_none() && !j.recommend_discard);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_machine_that_ended_at_the_return_move_probably_finished_and_discarding_is_recommended() {
    let (store, dir) = square_checkpoint("finished");
    let l = store.load().unwrap();
    let j = summarize(&store, &l, Some(&now([0.0, 0.0], true)));
    assert_eq!(j.machine, ResumeMachine::ProbablyFinished);
    assert!(j.recommend_discard && j.can_resume);
    assert!(j.percent > 99.9, "{}", j.percent);
    let w = j.warning.unwrap();
    assert!(
        w.contains("parece haber terminado") && w.contains("Revisa la pieza"),
        "{w}"
    );
    // El diálogo trae el mismo aviso primero.
    let v = view(
        &store,
        &l,
        &PlanOptions::default(),
        Some(&now([0.0, 0.0], true)),
    )
    .unwrap();
    assert_eq!(v.warnings[0], w);
    // Al cerrar el cuadrado la cabeza está donde empezó el primer lado: se ubica hacia delante,
    // en el último movimiento, no al principio del primer lado.
    let j = summarize(&store, &l, Some(&now([100.0, 100.0], true)));
    assert_eq!(j.machine, ResumeMachine::ProbablyFinished, "{j:?}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_machine_found_in_the_middle_proposes_continuing_from_where_it_is_now() {
    let (store, dir) = square_checkpoint("moved");
    let l = store.load().unwrap();
    let at = [180.0, 140.0];
    let j = summarize(&store, &l, Some(&now(at, true)));
    assert_eq!(j.machine, ResumeMachine::Moved);
    assert!(!j.recommend_discard);
    // 80 mm del primer lado y 40 del segundo, de 320.
    assert!((j.percent - 37.5).abs() < 0.01, "{}", j.percent);
    let w = j.warning.unwrap();
    assert!(
        w.contains("siguió ejecutando por su cuenta") && w.contains("6 líneas"),
        "{w}"
    );
    // El punto propuesto parte de la posición de ahora (segundo lado), no de la guardada.
    let v = view(&store, &l, &PlanOptions::default(), Some(&now(at, true))).unwrap();
    assert_eq!(v.suggested.executing_line, 5);
    assert_eq!(v.suggested.line, 5);
    let p = v.suggested.at.unwrap();
    assert!(
        (p[0] - 180.0).abs() < 0.01 && (p[1] - 138.5).abs() < 0.01,
        "{p:?}"
    );
    // Una posición que no cae sobre el trabajo no se puede ubicar: se queda el comportamiento de siempre.
    let j = summarize(&store, &l, Some(&now([50.0, 150.0], true)));
    assert_eq!(j.machine, ResumeMachine::Unknown);
    assert!(j.warning.is_some());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_long_straight_side_resumes_in_the_middle_with_a_small_overlap() {
    // Falcon2 Pro con corte de luz de verdad: cuadrado de 80 mm desde (100,100) a 600 mm/min,
    // 12 líneas, 8 `ok`, última línea 9, cabeza en (167.725, 100) sobre el primer lado.
    let lines: Vec<String> = "G90\nG21\nM5\nM4 S0\nG0 X100 Y100\nG1 X180 F600\nG1 Y180\nG1 X100\nG1 Y100\nM5\nG0 X0 Y0\nM9"
        .lines()
        .map(str::to_string)
        .collect();
    let (store, dir) = saved("falcon-power", &lines);
    store
        .write_progress(&progress(8, Some(9), Some([167.725, 100.0])))
        .unwrap();
    let l = store.load().unwrap();
    let v = view(&store, &l, &PlanOptions::default(), None).unwrap();
    assert_eq!(v.suggested.executing_line, 5);
    assert_eq!(v.suggested.line, 5);
    let at = v.suggested.at.expect("punto a mitad del primer lado");
    assert!(
        (at[0] - 166.225).abs() < 0.01 && (at[1] - 100.0).abs() < 1e-9,
        "{at:?}"
    );
    // Un solo porcentaje, por posición: 67,7 de 320 mm.
    let pct = 67.725 / 320.0 * 100.0;
    assert!((v.job.percent - pct).abs() < 0.01, "{}", v.job.percent);
    assert!((v.suggested.percent - 66.225 / 320.0 * 100.0).abs() < 0.01);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_board_at_zero_without_alarm_after_a_power_cut_is_not_taken_for_a_finished_job() {
    // Falcon2 Pro tras un corte de luz de verdad: Idle, sin alarma, MPos (0, 0) que es el
    // arranque de la placa. El trabajo también termina en (0, 0), así que la MPos no distingue.
    let (store, dir) = square_checkpoint("power-zero");
    let mut p = store.load().unwrap().progress.unwrap();
    p.modal.f = Some(800.0);
    store.write_progress(&p).unwrap();
    let l = store.load().unwrap();
    // `$G` dice F0: se reinició.
    let reset = assess::Now {
        parser_feed: Some(0.0),
        ..now([0.0, 0.0], true)
    };
    let j = summarize(&store, &l, Some(&reset));
    assert_eq!(j.machine, ResumeMachine::Unknown);
    assert!(!j.recommend_discard && j.can_resume);
    assert!(j.warning.unwrap().contains("pudo seguir ejecutando"));
    // Sin `$G`, (0, 0) tampoco se toma por «terminó».
    let blind = assess::Now {
        parser_feed: None,
        ..now([0.0, 0.0], true)
    };
    assert_eq!(
        summarize(&store, &l, Some(&blind)).machine,
        ResumeMachine::Unknown
    );
    // Con el `F800` intacto la placa no se reinició: terminó sola.
    assert_eq!(
        summarize(&store, &l, Some(&now([0.0, 0.0], true))).machine,
        ResumeMachine::ProbablyFinished
    );
    let _ = fs::remove_dir_all(dir);
}
