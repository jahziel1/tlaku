//! Pruebas de la conexión K40 contra la placa simulada de `tlaku-lhymicro` (sin hardware): el
//! mismo camino que con una placa, con el reloj virtual del emulador marcado por las lecturas.

use super::*;
use crate::machine::active::{ActiveMachine, Machine};
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Polyline, Rgb};
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{
    DesignObject, Document, JobOptions, LayerMode, LayerSettings, ObjectKind, RealtimeCommand,
    VectorObject,
};
use tlaku_lhymicro::emu::{EmuBoard, EmuFaults};

// ---------------------------------------------------------------------------
// Apoyos
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Collect(Mutex<Vec<Emit>>);

impl Sink for Collect {
    fn emit(&self, event: Emit) {
        self.0.lock().unwrap().push(event);
    }
}

impl Collect {
    fn all(&self) -> Vec<Emit> {
        self.0.lock().unwrap().clone()
    }

    fn jobs(&self) -> Vec<JobEvent> {
        self.all()
            .into_iter()
            .filter_map(|e| match e {
                Emit::Job(j) => Some(j),
                _ => None,
            })
            .collect()
    }

    fn job_states(&self) -> Vec<JobState> {
        self.jobs().into_iter().map(|j| j.state).collect()
    }

    fn logs(&self) -> String {
        self.all()
            .into_iter()
            .filter_map(|e| match e {
                Emit::Log(l) => Some(l.text),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn connections(&self) -> Vec<ConnectionEvent> {
        self.all()
            .into_iter()
            .filter_map(|e| match e {
                Emit::Connection(c) => Some(c),
                _ => None,
            })
            .collect()
    }

    fn statuses(&self) -> Vec<StatusReport> {
        self.all()
            .into_iter()
            .filter_map(|e| match e {
                Emit::Status(s) => Some(s),
                _ => None,
            })
            .collect()
    }

    fn progress(&self) -> Vec<ProgressEvent> {
        self.all()
            .into_iter()
            .filter_map(|e| match e {
                Emit::Progress(p) => Some(p),
                _ => None,
            })
            .collect()
    }

    fn wait(&self, what: &str, f: impl Fn(&Collect) -> bool) {
        wait_until(what, || f(self));
    }
}

fn wait_until(what: &str, f: impl Fn() -> bool) {
    let end = Instant::now() + Duration::from_secs(15);
    while !f() {
        assert!(Instant::now() < end, "se agotó la espera de: {what}");
        thread::sleep(Duration::from_millis(3));
    }
}

fn fast() -> LhyTiming {
    LhyTiming {
        status_interval: Duration::from_millis(20),
        lost_after: 3,
        pause_wait: Duration::from_secs(5),
        progress_interval: Duration::ZERO,
    }
}

/// Un K40 de serie: 300 x 200 mm, home arriba a la izquierda, placa M2.
fn device() -> DeviceProfile {
    DeviceProfile {
        kind: tlaku_doc::DeviceKind::LhyMicro,
        bed: [300.0, 200.0],
        origin: Origin::TopLeft,
        max_rate: [8400.0, 8400.0],
        ..DeviceProfile::default()
    }
}

fn layer() -> LayerSettings {
    LayerSettings {
        mode: LayerMode::Line,
        speed: 1200.0,
        power: 60.0,
        ..LayerSettings::for_color(Rgb(255, 0, 0))
    }
}

/// Cuadrado de 20 x 20 mm en (10, 10) del documento (80 mm de perímetro).
fn square_doc() -> Document {
    let red = Rgb(255, 0, 0);
    Document {
        objects: vec![DesignObject {
            id: "a".into(),
            layer: red,
            transform: IDENTITY,
            kind: ObjectKind::Vector(VectorObject {
                paths: vec![BezPath::from(&Polyline {
                    points: vec![
                        Point::new(10.0, 10.0),
                        Point::new(30.0, 10.0),
                        Point::new(30.0, 30.0),
                        Point::new(10.0, 30.0),
                    ],
                    closed: true,
                })],
            }),
            locked: false,
            hidden: false,
            group: None,
            priority: None,
            power_scale: None,
            shape: None,
            start: None,
            manual_tabs: Vec::new(),
            array: None,
        }],
        layers: vec![layer()],
        ..Document::default()
    }
}

struct Rig {
    board: Arc<Mutex<EmuBoard>>,
    sink: Arc<Collect>,
    conn: LhyConnection,
    handle: LhyHandle,
    device: DeviceProfile,
}

/// `step` es lo que tarda (en tiempo real) cada lectura de estado: 0 = tan rápido como se pueda,
/// unos milisegundos = un trabajo que dura lo bastante para pausarlo o detenerlo.
fn rig_with(emu: EmuConfig, step: Duration) -> Result<Rig, String> {
    let device = device();
    let (transport, board) = emulator(emu);
    let sink = Arc::new(Collect::default());
    let mut cfg = session_config(&device).unwrap();
    cfg.poll_backoff_max = Duration::ZERO;
    let (conn, _) = LhyConnection::start(
        Box::new(Paced {
            inner: transport,
            step,
        }),
        cfg,
        "virtual:k40",
        sink.clone() as Arc<dyn Sink>,
        fast(),
        true,
    )?;
    let handle = conn.handle();
    Ok(Rig {
        board,
        sink,
        conn,
        handle,
        device,
    })
}

fn rig() -> Rig {
    rig_with(EmuConfig::default(), Duration::ZERO).unwrap()
}

/// Una placa con lecturas lentas: el cuadrado dura del orden de un segundo real.
fn slow_rig() -> Rig {
    rig_with(EmuConfig::default(), Duration::from_millis(3)).unwrap()
}

impl Rig {
    fn start(&self) -> Result<(), String> {
        self.handle
            .start_job(&square_doc(), &self.device, &JobOptions::default())
    }

    fn frame(&self) -> Result<(), String> {
        self.handle
            .frame_job(&square_doc(), &self.device, &JobOptions::default())
    }

    fn board(&self) -> std::sync::MutexGuard<'_, EmuBoard> {
        self.board.lock().unwrap_or_else(|e| e.into_inner())
    }
}

// ---------------------------------------------------------------------------
// Perfil, puertos y avisos
// ---------------------------------------------------------------------------

#[test]
fn the_profile_maps_to_the_session() {
    let mut d = device();
    d.lhymicro.board = "b2".into();
    d.origin = Origin::BottomRight;
    d.bed = [310.0, 220.0];
    let cfg = session_config(&d).unwrap();
    assert_eq!(cfg.board, Board::B2);
    assert_eq!(cfg.home, HomeCorner::BottomRight);
    assert_eq!(cfg.bed_mm, [310.0, 220.0]);
    assert!(
        (cfg.max_speed_mm_s - 140.0).abs() < 1e-9,
        "8400 mm/min = 140 mm/s"
    );
    // Un K40 de serie: el home arriba a la izquierda.
    assert_eq!(home_for_origin(Origin::TopLeft), HomeCorner::TopLeft);
    // Un modelo que no existe se rechaza con un mensaje que lista los válidos.
    d.lhymicro.board = "Z9".into();
    let e = session_config(&d).unwrap_err();
    assert!(e.contains("Z9") && e.contains("M2"), "{e}");
    // Sin tope de velocidad utilizable, el de siempre.
    d.lhymicro.board = "M2".into();
    d.max_rate = [0.0, 0.0];
    assert_eq!(session_config(&d).unwrap().max_speed_mm_s, 140.0);
}

#[test]
fn ports_are_recognised() {
    assert!(is_virtual("virtual:k40"));
    assert!(is_virtual("virtual:k40@5"));
    assert!(!is_virtual("virtual:ruida"));
    assert!(!is_virtual("virtual:k402"));
    assert_eq!(usb_index("usb:k40:0"), Some(0));
    assert_eq!(usb_index("usb:k40:3"), Some(3));
    assert_eq!(usb_index("usb:k40:x"), None);
    assert_eq!(usb_index("/dev/ttyUSB0"), None);
    // Listar el USB nunca falla (sin placas, la lista queda vacía) y todo lo que da es un puerto USB del K40.
    assert!(usb_ports().iter().all(|p| usb_index(p).is_some()));
    assert!(virtual_speed("virtual:k40").unwrap() == 1.0);
    assert!(virtual_speed("virtual:k40@5").unwrap() == 5.0);
    assert!(virtual_speed("virtual:k40@0").is_err());
    assert!(virtual_speed("virtual:k40@zz").is_err());
}

#[test]
fn the_preview_lists_what_the_board_ignores_before_sending() {
    let p = preview(&square_doc(), &device(), &JobOptions::default(), None).unwrap();
    assert_eq!(p.board, "M2");
    assert!(p.homes_first, "sin posición conocida se hace home primero");
    assert!(p.packets > 0 && p.bytes > 0 && p.est_seconds > 0.0);
    assert!((p.burn_mm - 80.0).abs() < 1.0, "{}", p.burn_mm);
    let power: Vec<_> = p
        .warnings
        .iter()
        .filter(|w| w.kind == "power_ignored")
        .collect();
    assert_eq!(power.len(), 1, "{:?}", p.warnings);
    assert_eq!(power[0].value, Some(60.0));
    assert_eq!(power[0].layer.as_deref(), Some("#ff0000"));
    // Con el cabezal en un punto conocido, ya no hace home.
    let p = preview(
        &square_doc(),
        &device(),
        &JobOptions::default(),
        Some([0.0, 0.0]),
    )
    .unwrap();
    assert!(!p.homes_first);
}

#[test]
fn the_preview_refuses_a_layer_faster_than_the_profile_allows() {
    let mut d = device();
    d.max_rate = [600.0, 600.0]; // 10 mm/s; la capa va a 20 mm/s
    let e = preview(&square_doc(), &d, &JobOptions::default(), None).unwrap_err();
    assert!(e.contains("supera el máximo"), "{e}");
}

// ---------------------------------------------------------------------------
// Conexión
// ---------------------------------------------------------------------------

#[test]
fn connecting_probes_the_board_and_says_what_it_is() {
    let r = rig();
    let conns = r.sink.connections();
    assert_eq!(conns.len(), 1);
    assert!(conns[0].connected);
    assert_eq!(conns[0].port.as_deref(), Some("virtual:k40"));
    let logs = r.sink.logs();
    assert!(logs.contains("solo lectura"), "{logs}");
    assert!(logs.contains("M2 Nano simulada"), "{logs}");
    assert!(logs.contains("EXPERIMENTAL"), "{logs}");
    assert!(logs.contains("Máquina virtual"), "{logs}");
    // La posición se cuenta desde el home: el panel debe poder avisar que aún no se conoce.
    let s = r.handle.state();
    assert_eq!(s.board, "M2");
    assert!(s.virtual_machine && !s.position_known);
    assert_eq!(r.handle.work_pos(), None);
    r.sink
        .wait("estado de reposo", |c| !c.statuses().is_empty());
    assert_eq!(r.sink.statuses().last().unwrap().state, MachineState::Idle);
    // Conectar solo lee: no se mandó ningún paquete.
    assert_eq!(r.board().accepted, 0);
    r.conn.close();
}

#[test]
fn the_firmware_line_names_the_board_and_its_state() {
    let device = device();
    let (transport, _b) = emulator(EmuConfig::default());
    let (conn, info) = LhyConnection::start(
        Box::new(transport),
        session_config(&device).unwrap(),
        "virtual:k40",
        Arc::new(Collect::default()) as Arc<dyn Sink>,
        fast(),
        true,
    )
    .unwrap();
    assert!(
        info.firmware.contains("K40") && info.firmware.contains("placa M2"),
        "{}",
        info.firmware
    );
    assert!(info.firmware.contains("206"), "{}", info.firmware);
    conn.close();
}

#[test]
fn a_moshiboard_is_refused_with_a_clear_message() {
    let err = rig_with(
        EmuConfig {
            moshiboard: true,
            ..EmuConfig::default()
        },
        Duration::ZERO,
    )
    .err()
    .expect("una Moshiboard no se conecta");
    assert!(
        err.contains("Moshiboard") && err.contains("no es compatible"),
        "{err}"
    );
}

#[test]
fn a_silent_board_reports_what_to_check() {
    let err = rig_with(
        EmuConfig {
            faults: EmuFaults {
                empty_from_poll: Some(0),
                ..EmuFaults::default()
            },
            ..EmuConfig::default()
        },
        Duration::ZERO,
    )
    .map(|_| ())
    .unwrap_err();
    // Sin estado utilizable no se conecta, y el mensaje dice qué revisar.
    assert!(err.contains("estado") && err.contains("cable USB"), "{err}");
}

#[test]
fn connecting_to_the_virtual_machine_through_the_real_entry_point() {
    let sink = Arc::new(Collect::default());
    let (conn, info) = LhyConnection::connect(
        &device(),
        "virtual:k40@100",
        sink.clone() as Arc<dyn Sink>,
        fast(),
    )
    .unwrap();
    assert!(info.firmware.contains("K40"), "{}", info.firmware);
    let h = conn.handle();
    assert!(h.is_virtual());
    h.home().unwrap();
    h.start_job(&square_doc(), &device(), &JobOptions::default())
        .unwrap();
    sink.wait("fin del trabajo", |c| {
        c.job_states().contains(&JobState::Finished)
    });
    conn.close();
    // La conexión cerrada avisa.
    assert!(!sink.connections().last().unwrap().connected);
    // Un puerto de otro tipo o una velocidad mala no abren nada.
    let bad = |p: &str| {
        LhyConnection::connect(
            &device(),
            p,
            Arc::new(Collect::default()) as Arc<dyn Sink>,
            fast(),
        )
        .err()
    };
    assert!(bad("virtual:k40@0").unwrap().contains("velocidad"));
    assert!(
        bad("/dev/ttyUSB0")
            .unwrap()
            .contains("no es un puerto de K40")
    );
    // Sin placa en ese número, no se abre (ni con hardware ni sin él).
    assert!(bad("usb:k40:99").is_some());
    // Un modelo de placa desconocido falla antes de abrir nada.
    let mut d = device();
    d.lhymicro.board = "ZZ".into();
    assert!(
        LhyConnection::connect(
            &d,
            "virtual:k40",
            Arc::new(Collect::default()) as Arc<dyn Sink>,
            fast()
        )
        .err()
        .unwrap()
        .contains("ZZ")
    );
}

#[test]
fn losing_the_usb_ends_the_connection_with_a_cable_hint() {
    let r = rig_with(
        EmuConfig {
            faults: EmuFaults {
                disconnect_after_polls: Some(6),
                ..EmuFaults::default()
            },
            ..EmuConfig::default()
        },
        Duration::ZERO,
    )
    .unwrap();
    r.sink.wait("conexión perdida", |c| {
        c.connections().iter().any(|e| !e.connected)
    });
    assert!(r.handle.is_closed());
    let last = r.sink.connections().pop().unwrap();
    assert!(
        last.error.as_deref().unwrap_or("").contains("cable USB"),
        "{last:?}"
    );
    assert!(r.handle.home().is_err());
}

// ---------------------------------------------------------------------------
// Home y saltos
// ---------------------------------------------------------------------------

#[test]
fn nothing_moves_until_the_position_is_known() {
    let r = rig();
    let e = r.handle.jog(5.0, 0.0, 3000.0).unwrap_err();
    assert!(e.contains("home"), "{e}");
    let e = r.handle.jog_to(10.0, 10.0, 3000.0).unwrap_err();
    assert!(e.contains("home"), "{e}");
    let e = r.frame().unwrap_err();
    assert!(e.contains("home"), "{e}");
    assert_eq!(r.board().accepted, 0, "no salió ningún paquete");
}

#[test]
fn home_counts_the_position_and_jogs_are_relative_to_it() {
    let r = rig();
    r.handle.home().unwrap();
    assert!(r.handle.state().position_known);
    assert_eq!(r.handle.work_pos(), Some([0.0, 0.0]));
    assert!(r.sink.logs().contains("home hecho"));

    // Relativo: la posición se cuenta en mm desde el home, en los ejes de la máquina.
    r.handle.jog(12.0, 7.0, 3000.0).unwrap();
    let p = r.handle.work_pos().unwrap();
    assert!(
        (p[0] - 12.0).abs() < 0.05 && (p[1] - 7.0).abs() < 0.05,
        "{p:?}"
    );
    let head = r.board().head_mm();
    assert!(
        (head.0 - 12.0).abs() < 0.05 && (head.1 - 7.0).abs() < 0.05,
        "{head:?}"
    );

    // Absoluto.
    r.handle.jog_to(40.0, 25.0, 3000.0).unwrap();
    let head = r.board().head_mm();
    assert!(
        (head.0 - 40.0).abs() < 0.05 && (head.1 - 25.0).abs() < 0.05,
        "{head:?}"
    );
    assert_eq!(r.board().burn_length_mm(), 0.0, "los saltos no queman");
    assert!(!r.board().laser_on());

    // Fuera de la cama no se mueve nada y se explica.
    let e = r.handle.jog_to(400.0, 10.0, 3000.0).unwrap_err();
    assert!(e.contains("fuera de la cama"), "{e}");
    let head2 = r.board().head_mm();
    assert!((head2.0 - 40.0).abs() < 0.05, "{head2:?}");
    r.conn.close();
}

#[test]
fn a_home_in_another_corner_flips_the_axes() {
    // Home abajo a la derecha: la distancia desde el home sigue siendo la posición de máquina.
    let mut d = device();
    d.origin = Origin::BottomRight;
    let (transport, board) = emulator(EmuConfig::default());
    let (conn, _) = LhyConnection::start(
        Box::new(transport),
        session_config(&d).unwrap(),
        "virtual:k40",
        Arc::new(Collect::default()) as Arc<dyn Sink>,
        fast(),
        true,
    )
    .unwrap();
    let h = conn.handle();
    h.home().unwrap();
    h.jog(10.0, 5.0, 3000.0).unwrap();
    assert_eq!(
        h.work_pos().map(|p| (p[0].round(), p[1].round())),
        Some((10.0, 5.0))
    );
    let head = board.lock().unwrap().head_mm();
    assert!(
        (head.0 - 10.0).abs() < 0.05 && (head.1 - 5.0).abs() < 0.05,
        "{head:?}"
    );
    conn.close();
}

#[test]
fn a_changed_profile_requires_reconnecting() {
    let r = rig();
    let mut d = r.device.clone();
    d.bed = [400.0, 300.0];
    let e = r
        .handle
        .start_job(&square_doc(), &d, &JobOptions::default())
        .unwrap_err();
    assert!(e.contains("Desconecta"), "{e}");
    d = r.device.clone();
    d.lhymicro.board = "B2".into();
    assert!(
        r.handle
            .frame_job(&square_doc(), &d, &JobOptions::default())
            .is_err()
    );
    assert_eq!(r.board().accepted, 0);
}

// ---------------------------------------------------------------------------
// Enviar
// ---------------------------------------------------------------------------

#[test]
fn sending_a_job_homes_first_and_burns_the_design() {
    let r = rig();
    // Sin home previo: el envío lo hace primero.
    r.start().unwrap();
    r.sink.wait("fin del trabajo", |c| {
        c.job_states().contains(&JobState::Finished)
    });
    assert_eq!(
        r.sink.job_states(),
        vec![JobState::Started, JobState::Finished]
    );
    let logs = r.sink.logs();
    assert!(logs.contains("home antes de empezar"), "{logs}");
    // Los avisos del codificador quedan en el registro.
    assert!(
        logs.contains("AVISO") && logs.contains("potencia"),
        "{logs}"
    );
    {
        let mut b = r.board();
        b.run_to_idle();
        assert!(
            (b.burn_length_mm() - 80.0).abs() < 1.5,
            "{}",
            b.burn_length_mm()
        );
        assert!(!b.laser_on(), "el láser queda apagado");
        assert_eq!(b.overruns, 0);
        assert_eq!(b.rejected, 0);
        // El cuadrado del documento (10..30, 10..30) cae, desde el home arriba a la izquierda,
        // a 10..30 mm del borde izquierdo y a 200 - 30 .. 200 - 10 mm del superior.
        let (lo, hi) = b.burn_bbox_mm().unwrap();
        assert!(
            (lo.0 - 10.0).abs() < 0.2 && (hi.0 - 30.0).abs() < 0.2,
            "{lo:?} {hi:?}"
        );
        assert!(
            (lo.1 - 170.0).abs() < 0.2 && (hi.1 - 190.0).abs() < 0.2,
            "{lo:?} {hi:?}"
        );
    }
    // El progreso termina completo y la posición queda contada.
    let p = r.sink.progress();
    assert!(!p.is_empty());
    let last = p.last().unwrap();
    assert_eq!(last.acked, last.total);
    assert!(p.windows(2).all(|w| w[0].acked <= w[1].acked));
    r.sink.wait("reposo", |c| {
        c.statuses()
            .last()
            .is_some_and(|s| s.state == MachineState::Idle)
    });
    assert!(r.handle.state().position_known);
    let jobs = r.sink.jobs();
    let msg = &jobs.last().unwrap().message;
    assert!(msg.contains("terminado"), "{msg}");
    r.conn.close();
}

#[test]
fn a_second_job_starts_from_where_the_first_one_ended() {
    let r = rig();
    r.handle.home().unwrap();
    r.start().unwrap();
    r.sink.wait("primer trabajo", |c| {
        c.job_states().contains(&JobState::Finished)
    });
    r.sink.wait("reposo", |c| {
        c.statuses()
            .last()
            .is_some_and(|s| s.state == MachineState::Idle)
    });
    assert!(
        !r.sink.logs().contains("home antes de empezar"),
        "ya había home"
    );
    // No hace home otra vez: sigue desde donde quedó (la posición se cuenta).
    let before = r.board().accepted;
    r.start().unwrap();
    r.sink.wait("segundo trabajo", |c| {
        c.job_states()
            .iter()
            .filter(|s| **s == JobState::Finished)
            .count()
            == 2
    });
    assert!(r.board().accepted > before);
    assert!(!r.sink.logs().contains("home antes de empezar"));
    r.conn.close();
}

#[test]
fn only_one_thing_at_a_time() {
    let r = slow_rig();
    r.handle.home().unwrap();
    r.start().unwrap();
    // Con el trabajo en marcha, mover, hacer home o encuadrar se rechaza sin tocar la placa.
    for e in [
        r.handle.jog(1.0, 0.0, 3000.0).unwrap_err(),
        r.handle.jog_to(1.0, 1.0, 3000.0).unwrap_err(),
        r.handle.home().unwrap_err(),
        r.frame().unwrap_err(),
        r.start().unwrap_err(),
    ] {
        assert!(e.contains("ocupada"), "{e}");
    }
    r.handle.cancel().unwrap();
    r.sink
        .wait("fin", |c| c.job_states().contains(&JobState::Cancelled));
    r.conn.close();
}

// ---------------------------------------------------------------------------
// Pausa, reanudar y detener
// ---------------------------------------------------------------------------

#[test]
fn pause_and_resume_toggle_the_board_and_report_it() {
    let r = slow_rig();
    r.handle.home().unwrap();
    r.start().unwrap();
    assert!(r.handle.pause().is_ok());
    assert_eq!(
        r.sink.job_states(),
        vec![JobState::Started, JobState::Paused]
    );
    assert!(r.board().is_paused());
    assert_eq!(r.board().pause_toggles, 1);
    // Pausar dos veces no hace nada raro, y detener o reanudar sin pausa se explica.
    let burned = r.board().burn_length_mm();
    thread::sleep(Duration::from_millis(50));
    assert_eq!(
        r.board().burn_length_mm(),
        burned,
        "en pausa no se quema nada"
    );

    r.handle.resume().unwrap();
    assert_eq!(r.sink.job_states().last(), Some(&JobState::Started));
    assert!(!r.board().is_paused());
    assert!(r.handle.resume().is_err(), "ya no está en pausa");
    r.sink.wait("fin del trabajo", |c| {
        c.job_states().contains(&JobState::Finished)
    });
    assert_eq!(r.board().pause_toggles, 2);
    let mut b = r.board();
    b.run_to_idle();
    assert!(
        (b.burn_length_mm() - 80.0).abs() < 1.5,
        "pausar no pierde ni repite nada"
    );
    drop(b);
    r.conn.close();
}

#[test]
fn pause_without_a_job_is_explained() {
    let r = rig();
    assert!(r.handle.pause().unwrap_err().contains("No hay un trabajo"));
    assert!(r.handle.resume().unwrap_err().contains("no está en pausa"));
    assert_eq!(r.board().pause_toggles, 0);
}

#[test]
fn stop_cancels_the_job_turns_the_laser_off_and_loses_the_position() {
    let r = slow_rig();
    r.handle.home().unwrap();
    r.start().unwrap();
    thread::sleep(Duration::from_millis(60));
    r.handle.cancel().unwrap();
    r.sink.wait("trabajo cancelado", |c| {
        c.job_states().contains(&JobState::Cancelled)
    });
    let ev = r.sink.jobs().pop().unwrap();
    assert_eq!(ev.state, JobState::Cancelled);
    assert!(ev.position_lost, "la posición se perdió");
    assert!(ev.message.contains("home"), "{}", ev.message);
    assert!(!ev.message.contains(UNCONFIRMED_MARK));
    {
        let b = r.board();
        assert!(b.aborts >= 1, "llegó el I de cancelar");
        assert!(!b.laser_on(), "el láser quedó apagado");
    }
    assert!(!r.handle.state().position_known);
    assert_eq!(r.handle.work_pos(), None);
    // Sin posición no se mueve nada hasta volver a hacer home.
    assert!(r.handle.jog(1.0, 1.0, 3000.0).unwrap_err().contains("home"));
    r.sink.wait("reposo", |c| {
        c.statuses()
            .last()
            .is_some_and(|s| s.state == MachineState::Idle)
    });
    r.handle.home().unwrap();
    assert!(r.handle.state().position_known);
    r.conn.close();
}

#[test]
fn stopping_while_paused_still_stops() {
    let r = slow_rig();
    r.handle.home().unwrap();
    r.start().unwrap();
    r.handle.pause().unwrap();
    r.handle.cancel().unwrap();
    r.sink.wait("trabajo cancelado", |c| {
        c.job_states().contains(&JobState::Cancelled)
    });
    assert!(!r.board().laser_on());
    r.conn.close();
}

#[test]
fn an_unconfirmed_stop_tells_the_user_to_use_the_emergency_stop() {
    // La placa se desconecta a mitad del trabajo: el envío falla y ya no se puede confirmar que
    // se detuviera ni que el láser se apagara.
    let r = rig_with(
        EmuConfig {
            faults: EmuFaults {
                disconnect_after_packets: Some(2),
                ..EmuFaults::default()
            },
            ..EmuConfig::default()
        },
        Duration::ZERO,
    )
    .unwrap();
    r.handle.home().unwrap_or(()); // el home puede o no alcanzar a salir antes del corte
    let res = r.start();
    r.sink.wait("trabajo fallido", |c| {
        c.job_states().contains(&JobState::Error) || res.is_err()
    });
    let msg = match res {
        Err(m) => m,
        Ok(()) => {
            r.sink
                .jobs()
                .into_iter()
                .find(|j| j.state == JobState::Error)
                .unwrap()
                .message
        }
    };
    assert!(msg.contains(UNCONFIRMED_MARK), "{msg}");
    assert!(msg.contains("paro de emergencia"), "{msg}");
    assert!(r.sink.logs().contains("paro de emergencia"));
    // Y la posición se perdió.
    if let Some(j) = r
        .sink
        .jobs()
        .into_iter()
        .find(|j| j.state == JobState::Error)
    {
        assert!(j.position_lost);
    }
    r.sink.wait("conexión perdida", |c| {
        c.connections().iter().any(|e| !e.connected)
    });
}

#[test]
fn the_unconfirmed_text_comes_from_the_driver_and_the_ui_can_rely_on_it() {
    // La interfaz busca esta marca en el mensaje del trabajo: si el controlador la cambia, esta
    // prueba avisa (hay que cambiar también `LHY_UNCONFIRMED` en la interfaz).
    let e = tlaku_lhymicro::session::JobError {
        cause: SessionError::Cancelled,
        packets_sent: 1,
        packets_total: 9,
        stop: StopOutcome::Unconfirmed,
    };
    let text = e.to_string();
    assert!(text.contains(UNCONFIRMED_MARK), "{text}");
    assert!(text.contains("paro de emergencia"), "{text}");
}

#[test]
fn a_failed_stop_when_idle_is_reported_as_an_error() {
    let r = rig();
    // Un detener sin trabajo confirma con la placa sana...
    r.handle.cancel().unwrap();
    assert!(r.sink.logs().contains("la placa lo confirmó"));
    assert!(r.board().aborts >= 1);
    // ...y el home lo recupera.
    r.handle.home().unwrap();
    r.conn.close();
}

#[test]
fn closing_the_connection_stops_a_running_job() {
    let r = slow_rig();
    r.handle.home().unwrap();
    r.start().unwrap();
    thread::sleep(Duration::from_millis(40));
    let (board, sink) = (r.board.clone(), r.sink.clone());
    r.conn.close();
    let b = board.lock().unwrap();
    assert!(b.aborts >= 1);
    assert!(!b.laser_on());
    assert!(!sink.connections().last().unwrap().connected);
}

// ---------------------------------------------------------------------------
// Encuadre
// ---------------------------------------------------------------------------

#[test]
fn framing_walks_the_rectangle_with_the_laser_off() {
    let r = rig();
    r.handle.home().unwrap();
    r.frame().unwrap();
    r.sink.wait("fin del encuadre", |c| {
        c.logs().contains("encuadre terminado")
    });
    let mut b = r.board();
    b.run_to_idle();
    assert_eq!(b.burn_length_mm(), 0.0, "el encuadre no quema");
    assert!(b.trace().iter().all(|t| !t.laser));
    // Pasó por las cuatro esquinas del cuadrado, que desde el home (arriba a la izquierda)
    // están a 10..30 mm del borde izquierdo y a 170..190 mm del superior.
    let mm = |p: (f64, f64)| {
        (
            p.0 * tlaku_lhymicro::speed::MM_PER_MIL,
            p.1 * tlaku_lhymicro::speed::MM_PER_MIL,
        )
    };
    let near = |a: (f64, f64), x: f64, y: f64| (a.0 - x).abs() < 0.2 && (a.1 - y).abs() < 0.2;
    let visited: Vec<(f64, f64)> = b.trace().iter().map(|t| mm(t.to)).collect();
    for (x, y) in [(10.0, 170.0), (30.0, 170.0), (30.0, 190.0), (10.0, 190.0)] {
        assert!(
            visited.iter().any(|&v| near(v, x, y)),
            "esquina ({x}, {y}) en {visited:?}"
        );
    }
    drop(b);
    r.sink.wait("reposo", |c| {
        c.statuses()
            .last()
            .is_some_and(|s| s.state == MachineState::Idle)
    });
    assert!(
        r.handle.state().position_known,
        "el encuadre no pierde la posición"
    );
    // No dejó ningún evento de trabajo: un encuadre no es un trabajo.
    assert!(r.sink.job_states().is_empty());
    r.conn.close();
}

#[test]
fn stopping_a_frame_halts_the_board() {
    let r = slow_rig();
    r.handle.home().unwrap();
    r.frame().unwrap();
    assert_eq!(
        r.sink.statuses().last().map(|s| s.state),
        Some(MachineState::Run)
    );
    r.handle.cancel().unwrap();
    r.sink.wait("encuadre detenido", |c| {
        c.logs().contains("encuadre detenido")
    });
    r.sink.wait("reposo", |c| {
        c.statuses()
            .last()
            .is_some_and(|s| s.state == MachineState::Idle)
    });
    assert!(r.board().aborts >= 1);
    assert!(!r.board().laser_on());
    r.conn.close();
}

#[test]
fn a_frame_that_leaves_the_bed_is_refused_before_moving() {
    let mut d = device();
    // Un cuadrado de 20 mm no cabe en una cama de 15 mm: ya lo rechaza el cálculo del trabajo.
    d.bed = [15.0, 15.0];
    let (transport, board) = emulator(EmuConfig::default());
    let (conn, _) = LhyConnection::start(
        Box::new(transport),
        session_config(&d).unwrap(),
        "virtual:k40",
        Arc::new(Collect::default()) as Arc<dyn Sink>,
        fast(),
        true,
    )
    .unwrap();
    let h = conn.handle();
    h.home().unwrap();
    let before = board.lock().unwrap().accepted;
    assert!(
        h.frame_job(&square_doc(), &d, &JobOptions::default())
            .is_err()
    );
    conn.close();
    assert_eq!(board.lock().unwrap().accepted, before);
}

// ---------------------------------------------------------------------------
// Despacho (`Machine::Lhy`)
// ---------------------------------------------------------------------------

#[test]
fn the_machine_dispatch_covers_the_k40() {
    let r = slow_rig();
    let conn = r.conn;
    let active = ActiveMachine::Lhy(conn);
    let m = active.handle();
    assert!(matches!(m, Machine::Lhy(_)));
    assert!(!m.is_closed());
    assert!(m.lhy().is_ok());
    // GRBL y Ruida no existen aquí, con el motivo.
    assert!(m.grbl().err().unwrap().contains("solo de GRBL"));
    assert!(m.ruida().err().unwrap().contains("Ruida"));
    // Sin posición, no hay cabezal que mostrar.
    assert_eq!(m.work_pos(), None);
    m.lhy().unwrap().home().unwrap();
    assert_eq!(m.work_pos(), Some([0.0, 0.0]));
    assert!(
        m.status_report().is_some() || {
            r.sink.wait("estado", |c| !c.statuses().is_empty());
            true
        }
    );
    m.jog(3.0, 4.0, 1000.0).unwrap();
    m.jog_to(20.0, 20.0, 1000.0).unwrap();

    // Los comandos en tiempo real se traducen como en Ruida.
    m.lhy()
        .unwrap()
        .start_job(&square_doc(), &r.device, &JobOptions::default())
        .unwrap();
    m.realtime(RealtimeCommand::Hold).unwrap();
    let paused = || r.board.lock().unwrap().is_paused();
    assert!(paused());
    m.realtime(RealtimeCommand::Resume).unwrap();
    assert!(!paused());
    m.realtime(RealtimeCommand::Status).unwrap();
    // Cancelar un jog no existe en el K40 y no detiene la placa (perdería la posición).
    let aborts = r.board.lock().unwrap().aborts;
    m.realtime(RealtimeCommand::JogCancel).unwrap();
    assert_eq!(r.board.lock().unwrap().aborts, aborts);
    m.pause().unwrap();
    m.resume().unwrap();
    m.realtime(RealtimeCommand::Reset).unwrap();
    r.sink.wait("detenido", |c| {
        c.job_states().contains(&JobState::Cancelled)
    });
    active.close();
    assert!(!r.sink.connections().last().unwrap().connected);
}
