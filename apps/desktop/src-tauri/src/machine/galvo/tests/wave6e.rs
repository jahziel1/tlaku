//! Ola 6E: piezas por formas enteras, rotativo del galvo, `.cor` aplicado en el anfitrión y la
//! garantía de que nada de esto manda a la tarjeta un comando de escritura de configuración.

use super::*;
use tlaku_core::toolpath::Move;
use tlaku_doc::{CylinderAxis, GalvoRotary};

/// Extensión (min x, max x) de todos los movimientos de un paso.
fn x_extent(step: &PreparedStep) -> (f64, f64) {
    let xs: Vec<f64> = step
        .toolpath
        .blocks
        .iter()
        .flat_map(|b| b.moves.iter().map(|m: &Move| m.to().x))
        .collect();
    (
        xs.iter().copied().fold(f64::MAX, f64::min),
        xs.iter().copied().fold(f64::MIN, f64::max),
    )
}

/// Tres rectángulos de 60 mm de ancho: [0, 60], [70, 130] y [150, 210]. Con franjas fijas de
/// 100 mm el segundo cruza el borde de x = 100.
fn three_shapes() -> Document {
    doc_of(vec![
        rect("a", 0.0, 20.0, 60.0, 40.0),
        rect("b", 70.0, 20.0, 130.0, 40.0),
        rect("c", 150.0, 20.0, 210.0, 40.0),
    ])
}

fn marks_in(step: &PreparedStep) -> usize {
    step.toolpath
        .blocks
        .iter()
        .flat_map(|b| &b.moves)
        .filter(|m| matches!(m, Move::Cut { .. }))
        .count()
}

#[test]
fn whole_shapes_mark_each_shape_in_one_piece_and_plain_strips_cut_it() {
    // Franjas fijas: el rectángulo b cruza x = 100 y se marca en dos partes.
    let plain = split_device();
    let job = prepare(&three_shapes(), &plain, &JobOptions::default(), false).unwrap();
    let widths: Vec<f64> = job
        .steps
        .iter()
        .map(|s| {
            let (lo, hi) = x_extent(s);
            hi - lo
        })
        .collect();
    // Tres franjas de 100 mm (0..100, 100..200, 200..210): b se parte en dos (en x = 100) y c en
    // otras dos, así que ninguna franja trae solo rectángulos de 60 mm.
    assert_eq!(widths.len(), 3, "{widths:?}");
    assert!(widths.iter().any(|w| (w - 60.0).abs() > 1.0), "{widths:?}");

    // Formas enteras: tres piezas, cada rectángulo completo (60 mm de ancho).
    let mut d = split_device();
    d.galvo.split.whole_shapes = true;
    let job = prepare(&three_shapes(), &d, &JobOptions::default(), false).unwrap();
    assert_eq!(job.steps.len(), 3, "{:?}", job.warnings);
    for s in &job.steps {
        let (lo, hi) = x_extent(s);
        assert!((hi - lo - 60.0).abs() < 1e-6, "{lo}..{hi}");
        // Cada pieza se centra en el campo (la cama de 100 mm: centro en 50).
        assert!(((lo + hi) / 2.0 - 50.0).abs() < 1e-6, "{lo}..{hi}");
        assert_eq!(marks_in(s), 4, "un rectángulo entero son 4 marcas");
    }
    // La mesa se mueve entre piezas: de a (centro 30) a b (100) y de b a c (180).
    let (b, c) = (&job.steps[1].info, &job.steps[2].info);
    assert!(
        (b.dx + 70.0).abs() < 1e-6 && (c.dx + 80.0).abs() < 1e-6,
        "{b:?} {c:?}"
    );
    assert_eq!(job.manual_moves(), 2);
    assert!(job.warnings.is_empty(), "{:?}", job.warnings);
}

#[test]
fn whole_shapes_respect_the_users_selection() {
    let mut d = split_device();
    d.galvo.split.whole_shapes = true;
    let options = JobOptions {
        selection: Some(vec!["a".into(), "c".into()]),
        ..JobOptions::default()
    };
    let job = prepare(&three_shapes(), &d, &options, false).unwrap();
    // a [0, 60] y c [150, 210] no caben juntas en 100 mm: dos piezas, sin b.
    assert_eq!(job.steps.len(), 2);
    let total: usize = job.steps.iter().map(marks_in).sum();
    assert_eq!(total, 8);
}

#[test]
fn a_shape_larger_than_the_field_falls_back_to_strips_and_says_so() {
    let mut d = split_device();
    d.galvo.split.whole_shapes = true;
    let doc = doc_of(vec![rect("grande", 0.0, 20.0, 130.0, 40.0)]);
    let job = prepare(&doc, &d, &JobOptions::default(), false).unwrap();
    assert_eq!(job.steps.len(), 2, "franjas de 100 mm");
    assert!(
        job.warnings.iter().any(|w| w.contains("grande")),
        "{:?}",
        job.warnings
    );
}

/// Rotativo del galvo (M30): 100 mm de superficie por vuelta, franjas de 25 mm.
fn rotary_galvo(axis: CylinderAxis, reverse: bool) -> DeviceProfile {
    let mut d = split_device();
    d.galvo.split.piece_mm = 25.0;
    d.galvo.rotary = GalvoRotary {
        enabled: true,
        diameter_mm: 100.0 / std::f64::consts::PI,
        steps_per_rev: 1200,
        axis,
        reverse,
    };
    d
}

#[test]
fn the_galvo_rotary_replaces_the_general_one_and_asks_for_the_turn() {
    // Un diseño de 100 mm de alto en Y: cuatro franjas; entre ellas, un cuarto de vuelta.
    let doc = doc_of(vec![rect("a", 40.0, 0.0, 60.0, 100.0)]);
    let d = rotary_galvo(CylinderAxis::X, false);
    let job = prepare(&doc, &d, &JobOptions::default(), false).unwrap();
    assert_eq!(job.steps.len(), 4);
    for s in &job.steps[1..] {
        assert!((s.info.rotary_deg + 90.0).abs() < 1e-6, "{:?}", s.info);
        assert_eq!((s.info.dx, s.info.dy), (0.0, 0.0));
    }
    let d = rotary_galvo(CylinderAxis::X, true);
    let job = prepare(&doc, &d, &JobOptions::default(), false).unwrap();
    assert!((job.steps[1].info.rotary_deg - 90.0).abs() < 1e-6);
    // El perfil con el rotativo general de siempre (y el del galvo apagado) sigue igual.
    let mut legacy = split_device();
    legacy.galvo.split.piece_mm = 25.0;
    legacy.rotary.enabled = true;
    legacy.rotary.object_diameter = 100.0 / std::f64::consts::PI;
    let job = prepare(&doc, &legacy, &JobOptions::default(), false).unwrap();
    assert!((job.steps[1].info.rotary_deg + 90.0).abs() < 1e-6);
    // Con solo el rotativo del galvo encendido el trabajo ya cuenta como una secuencia.
    let mut only = device();
    only.galvo.rotary.enabled = true;
    assert!(is_sequenced(&only));
}

#[test]
fn the_galvo_rotary_on_the_y_axis_turns_between_vertical_strips() {
    let doc = doc_of(vec![rect("a", 0.0, 40.0, 100.0, 60.0)]);
    let d = rotary_galvo(CylinderAxis::Y, false);
    let job = prepare(&doc, &d, &JobOptions::default(), false).unwrap();
    assert_eq!(job.steps.len(), 4);
    assert!((job.steps[1].info.rotary_deg - 90.0).abs() < 1e-6);
    // Un diseño más alto que la pieza no cabe: el rotativo no mueve Y.
    let tall = doc_of(vec![rect("a", 0.0, 0.0, 100.0, 60.0)]);
    assert!(prepare(&tall, &d, &JobOptions::default(), false).is_err());
}

/// Un `.cor` de prueba con el mismo desplazamiento en todos los nodos.
fn write_cor(name: &str, dx: f64, dy: f64) -> String {
    let dir = std::env::temp_dir().join(format!("tlaku-desktop-6e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut b: Vec<u8> = "LMC1COR_1.0"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    b.extend([0, 0]);
    b.extend(std::iter::repeat_n(0u8, 63 * 8));
    for _ in 0..65 * 65 {
        b.extend(dx.to_le_bytes());
        b.extend(dy.to_le_bytes());
    }
    let path = dir.join(name);
    std::fs::write(&path, b).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn a_job_with_a_cor_file_marks_displaced_and_sends_only_whitelisted_commands() {
    let mut d = device();
    // 110 mm de campo: 65536 / 110 = 595,78 unidades por mm; 300 unidades son 0,5035 mm.
    d.galvo.correction.cor_file = Some(write_cor("lente.cor", 300.0, 0.0));
    let rig = rig_with(d, Faults::default(), Duration::ZERO).unwrap();
    rig.start(&square_doc()).unwrap();
    rig.sink
        .wait("el fin", |s| s.job_states().contains(&JobState::Finished));
    {
        let e = lock(&rig.emu);
        assert!(e.violations.is_empty(), "{:?}", e.violations);
        assert!(
            e.forbidden_commands().is_empty(),
            "{:?}",
            e.forbidden_commands()
        );
        for code in [
            tlaku_galvo::codes::imm::WRITE_COR_TABLE,
            tlaku_galvo::codes::imm::WRITE_COR_LINE,
            tlaku_galvo::codes::imm::SET_FLY_RES,
        ] {
            assert_eq!(e.count_immediates(code), 0, "{code:#06x}");
        }
        assert!(e.laser_safe());
        // El cuadrado (45..65 en una cama de 110, centro 55) va de -10 a +10 mm del centro: sin
        // `.cor` su esquina izquierda caería en 32768 - 10 x 595,78 = 26810 (26810,2); con él, 300
        // unidades más.
        let first = e.emitted()[0];
        assert_eq!(first.from.0, 26_810 + 300, "{first:?}");
    }
    rig.conn.close();
}

#[test]
fn the_preview_reports_the_cor_file_and_refuses_one_that_does_not_load() {
    let mut d = device();
    d.galvo.correction.cor_file = Some(write_cor("vista.cor", 100.0, 0.0));
    let p = preview(&square_doc(), &d, &JobOptions::default()).unwrap();
    assert!(
        p.warnings.iter().any(|w| w.contains(".cor")),
        "{:?}",
        p.warnings
    );
    d.galvo.correction.cor_file = Some("/no/existe/lente.cor".into());
    let e = preview(&square_doc(), &d, &JobOptions::default()).unwrap_err();
    assert!(e.contains(".cor"), "{e}");
    // Sin `.cor` no hay aviso.
    d.galvo.correction.cor_file = None;
    let p = preview(&square_doc(), &d, &JobOptions::default()).unwrap();
    assert!(!p.warnings.iter().any(|w| w.contains(".cor")));
}

#[test]
fn a_missing_cor_file_stops_the_job_before_anything_is_sent() {
    let mut d = device();
    d.galvo.correction.cor_file = Some("/no/existe/lente.cor".into());
    let rig = rig_with(d, Faults::default(), Duration::ZERO).unwrap();
    let e = rig.start(&square_doc()).unwrap_err();
    assert!(e.contains(".cor"), "{e}");
    let st = lock(&rig.emu);
    assert_eq!(st.batches_received, 0);
    assert!(st.laser_safe());
    drop(st);
    rig.conn.close();
}

#[test]
fn the_cylinder_correction_and_the_whole_pipeline_never_send_config_commands() {
    // Cilindro + cono + marcado dividido por formas, de punta a punta con la tarjeta simulada.
    let mut d = split_device();
    d.galvo.split.whole_shapes = true;
    d.galvo.cylinder.enabled = true;
    d.galvo.cylinder.diameter_mm = 80.0;
    d.galvo.cylinder.mirror_distance_mm = 160.0;
    let rig = rig_with(d, Faults::default(), Duration::ZERO).unwrap();
    let doc = doc_of(vec![
        rect("a", 30.0, 40.0, 70.0, 60.0),
        rect("b", 110.0, 40.0, 150.0, 60.0),
    ]);
    rig.start(&doc).unwrap();
    rig.sink.wait("la espera", |s| {
        !s.waits().is_empty() || s.job_states().contains(&JobState::Finished)
    });
    if !rig.sink.waits().is_empty() {
        rig.handle.confirm_move().unwrap();
    }
    rig.sink
        .wait("el fin", |s| s.job_states().contains(&JobState::Finished));
    let st = lock(&rig.emu);
    assert!(st.violations.is_empty(), "{:?}", st.violations);
    assert!(
        st.forbidden_commands().is_empty(),
        "{:?}",
        st.forbidden_commands()
    );
    assert!(st.laser_safe());
    drop(st);
    rig.conn.close();
}
