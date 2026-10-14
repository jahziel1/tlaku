//! Pruebas de la conexión Ruida contra el emulador UDP de `tlaku-ruida` (sin hardware): el mismo
//! camino que con una máquina, por sockets reales en `127.0.0.1`.

use super::*;
use crate::machine::active::Machine;
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Point, Polyline, Rgb};
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{
    DesignObject, Document, JobOptions, LayerSettings, ObjectKind, Origin, RealtimeCommand,
    VectorObject,
};
use tlaku_ruida::emu::{EmuConfig, EmuFaults};

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

    fn jobs(&self) -> Vec<JobState> {
        self.all()
            .into_iter()
            .filter_map(|e| match e {
                Emit::Job(j) => Some(j.state),
                _ => None,
            })
            .collect()
    }

    fn job_messages(&self) -> Vec<String> {
        self.all()
            .into_iter()
            .filter_map(|e| match e {
                Emit::Job(j) => Some(j.message),
                _ => None,
            })
            .collect()
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
    let end = Instant::now() + Duration::from_secs(5);
    while !f() {
        assert!(Instant::now() < end, "se agotó la espera de: {what}");
        thread::sleep(Duration::from_millis(5));
    }
}

fn fast() -> RuidaTiming {
    RuidaTiming {
        status_interval: Duration::from_millis(20),
        frame_interval: Duration::from_millis(10),
        poll_timeout: Duration::from_millis(100),
        upload_ack: Duration::from_millis(300),
        lost_after: 3,
        start_grace: Duration::from_millis(300),
    }
}

/// Un perfil Ruida de 600 x 400 mm con el home abajo a la izquierda.
fn device(run: bool) -> DeviceProfile {
    DeviceProfile {
        kind: tlaku_doc::DeviceKind::Ruida,
        bed: [600.0, 400.0],
        origin: Origin::BottomLeft,
        connection: tlaku_doc::Connection::Udp {
            host: "127.0.0.1".into(),
        },
        ruida: tlaku_doc::RuidaSettings {
            magic: None,
            run_after_upload: run,
        },
        ..DeviceProfile::default()
    }
}

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
        layers: vec![LayerSettings::for_color(red)],
        ..Document::default()
    }
}

struct Rig {
    emu: UdpEmulator,
    sink: Arc<Collect>,
    conn: RuidaConnection,
    handle: RuidaHandle,
    device: DeviceProfile,
}

fn rig_with(run: bool, faults: EmuFaults, timing: RuidaTiming, max_data: usize) -> Rig {
    let device = device(run);
    let emu = UdpEmulator::spawn(EmuConfig {
        bed_um: [600_000, 400_000],
        faults,
        ..EmuConfig::default()
    })
    .unwrap();
    let addr = emu.addr();
    let link = UdpLink::to_controller_ports(addr.ip(), addr.port(), 0).unwrap();
    let sink = Arc::new(Collect::default());
    let mut cfg = session_config(&device, &timing);
    cfg.max_data_bytes = max_data;
    let (conn, _) = RuidaConnection::start(
        link,
        cfg,
        "udp:127.0.0.1",
        sink.clone() as Arc<dyn Sink>,
        timing,
        None,
        &device,
    )
    .unwrap();
    let handle = conn.handle();
    Rig {
        emu,
        sink,
        conn,
        handle,
        device,
    }
}

fn rig(run: bool) -> Rig {
    rig_with(run, EmuFaults::default(), fast(), 1000)
}

impl Rig {
    fn start(&self) -> Result<(), String> {
        self.handle
            .start_job(&square_doc(), &self.device, &JobOptions::default())
    }
}

// ---------------------------------------------------------------------------
// Conexión y estado
// ---------------------------------------------------------------------------

#[test]
fn connecting_probes_the_machine_and_reports_state_and_position() {
    let r = rig(false);
    r.emu.emulator().pos_um = (12_345, 67_000);
    let conns = r.sink.connections();
    assert_eq!(conns.len(), 1);
    assert!(conns[0].connected);
    assert_eq!(conns[0].port.as_deref(), Some("udp:127.0.0.1"));
    let logs = r.sink.logs();
    assert!(
        logs.contains("RDC6442S") && logs.contains("0x65106510"),
        "{logs}"
    );
    assert!(logs.contains("solo lectura"), "{logs}");
    assert!(logs.contains("EXPERIMENTAL"), "{logs}");
    r.sink.wait("estado con la posición", |c| {
        c.statuses()
            .last()
            .is_some_and(|s| s.work_pos == [12.345, 67.0])
    });
    let st = r.sink.statuses().pop().unwrap();
    assert_eq!(st.state, MachineState::Idle);
    assert_eq!(st.machine_pos, st.work_pos);
    assert_eq!(r.handle.work_pos(), Some([12.345, 67.0]));
    // Conectar solo lee: nada de D8, D9 ni escrituras.
    let e = r.emu.emulator();
    assert!(
        e.commands()
            .unwrap()
            .iter()
            .all(|c| matches!(c, tlaku_ruida::cmd::Command::MemRead { .. }))
    );
}

#[test]
fn a_manual_magic_is_used_and_a_wrong_one_is_reported() {
    // El emulador habla con 0x11 y el perfil lo fija: no se prueban otros.
    let emu = UdpEmulator::spawn(EmuConfig {
        packets: PacketConfig {
            magic: Magic(0x11),
            ..PacketConfig::default()
        },
        ..EmuConfig::default()
    })
    .unwrap();
    let addr = emu.addr();
    let mut dev = device(false);
    dev.ruida.magic = Some(0x11);
    let sink = Arc::new(Collect::default());
    let link = UdpLink::to_controller_ports(addr.ip(), addr.port(), 0).unwrap();
    let (conn, info) = RuidaConnection::start(
        link,
        session_config(&dev, &fast()),
        "udp:x",
        sink.clone() as Arc<dyn Sink>,
        fast(),
        None,
        &dev,
    )
    .unwrap();
    assert!(info.firmware.contains("RDC6442S"), "{}", info.firmware);
    conn.close();

    // Con otro magic fijado, el ACK de la máquina delata el suyo: se corrige y se avisa.
    dev.ruida.magic = Some(0x33);
    let link = UdpLink::to_controller_ports(addr.ip(), addr.port(), 0).unwrap();
    let sink = Arc::new(Collect::default());
    let (conn, _) = RuidaConnection::start(
        link,
        session_config(&dev, &fast()),
        "udp:x",
        sink.clone() as Arc<dyn Sink>,
        fast(),
        None,
        &dev,
    )
    .unwrap();
    let logs = sink.logs();
    assert!(
        logs.contains("0x11") && logs.contains("detectado"),
        "{logs}"
    );
    conn.close();
}

#[test]
fn a_silent_address_reports_what_to_check() {
    // Nadie escucha en ese puerto: el sondeo termina en un error que explica qué revisar.
    let dead = UdpEmulator::spawn(EmuConfig::default()).unwrap();
    let addr = dead.addr();
    drop(dead);
    let dev = device(false);
    let link = UdpLink::to_controller_ports(addr.ip(), addr.port(), 0).unwrap();
    let mut cfg = session_config(&dev, &fast());
    cfg.probe_all_magics = false;
    cfg.ack_timeout = Duration::from_millis(50);
    cfg.reply_timeout = Duration::from_millis(50);
    let err = RuidaConnection::start(
        link,
        cfg,
        "udp:x",
        Arc::new(Collect::default()) as Arc<dyn Sink>,
        fast(),
        None,
        &dev,
    )
    .err()
    .unwrap();
    assert!(
        err.contains("no contesta") && err.contains("40200") && err.contains("magic"),
        "{err}"
    );
}

#[test]
fn a_profile_without_ip_cannot_connect() {
    let mut dev = device(false);
    dev.connection = tlaku_doc::Connection::Serial;
    let e = RuidaConnection::connect(
        &dev,
        "udp://x",
        Arc::new(Collect::default()) as Arc<dyn Sink>,
        fast(),
    )
    .err()
    .unwrap();
    assert!(e.contains("IP"), "{e}");
}

#[test]
fn losing_the_machine_closes_the_connection_with_an_error() {
    let r = rig(false);
    let Rig {
        emu, sink, handle, ..
    } = r;
    // El emulador desaparece: los sondeos dejan de tener respuesta.
    drop(emu);
    sink.wait("conexión perdida", |c| {
        c.connections().iter().any(|e| !e.connected)
    });
    let last = sink.connections().pop().unwrap();
    assert!(last.error.unwrap().contains("se perdió la conexión"));
    wait_until("mando cerrado", || handle.is_closed());
    assert!(handle.jog(1.0, 0.0, 3000.0).is_err());
}

#[test]
fn disconnecting_emits_the_event_and_closes_the_handle() {
    let r = rig(false);
    let (sink, handle) = (r.sink.clone(), r.handle.clone());
    r.conn.close();
    assert!(handle.is_closed());
    let last = sink.connections().pop().unwrap();
    assert!(!last.connected && last.error.is_none());
    assert_eq!(handle.pause().unwrap_err(), crate::machine::not_connected());
}

// ---------------------------------------------------------------------------
// Enviar
// ---------------------------------------------------------------------------

#[test]
fn storing_saves_a_document_with_progress_and_does_not_start() {
    let r = rig(false);
    r.start().unwrap();
    {
        let e = r.emu.emulator();
        assert_eq!(e.stored.len(), 1);
        assert!(
            e.stored[0].name.starts_with("TLAKU"),
            "{}",
            e.stored[0].name
        );
        assert!(!e.running);
        assert_eq!(e.rejected, 0);
    }
    assert_eq!(r.sink.jobs(), vec![JobState::Started, JobState::Finished]);
    let msgs = r.sink.job_messages();
    assert!(
        msgs[1].contains("guardado") && msgs[1].contains("panel"),
        "{msgs:?}"
    );
    let p = r.sink.progress();
    assert!(!p.is_empty());
    assert_eq!(p.last().unwrap().acked, p.last().unwrap().total);
    // El estado de la máquina sigue en reposo.
    r.sink.wait("reposo", |c| {
        c.statuses()
            .last()
            .is_some_and(|s| s.state == MachineState::Idle)
    });
}

#[test]
fn running_executes_and_the_end_is_followed_through_the_status() {
    let r = rig(true);
    r.start().unwrap();
    assert!(r.emu.emulator().running);
    r.sink.wait("máquina en marcha", |c| {
        c.statuses()
            .last()
            .is_some_and(|s| s.state == MachineState::Run)
    });
    assert_eq!(r.sink.jobs(), vec![JobState::Started]);
    // La máquina termina por su cuenta: el hilo lo ve y avisa.
    r.emu.emulator().finish_job();
    r.sink.wait("trabajo terminado", |c| {
        c.jobs().contains(&JobState::Finished)
    });
    assert_eq!(r.sink.jobs(), vec![JobState::Started, JobState::Finished]);
    // El `.rd` que llegó es el del trabajo.
    let tp = r.emu.emulator().to_toolpath().unwrap();
    assert_eq!(tp.blocks.len(), 1);
}

#[test]
fn pause_resume_and_stop_reach_the_machine_and_the_ui() {
    let r = rig(true);
    r.start().unwrap();
    r.handle.pause().unwrap();
    assert!(r.emu.emulator().paused);
    r.sink.wait("estado en pausa", |c| {
        c.statuses()
            .last()
            .is_some_and(|s| s.state == MachineState::Hold)
    });
    r.handle.resume().unwrap();
    assert!(!r.emu.emulator().paused);
    r.handle.cancel().unwrap();
    {
        let e = r.emu.emulator();
        assert!(!e.running);
        assert_eq!((e.pauses, e.resumes, e.stops), (1, 1, 1));
    }
    assert_eq!(
        r.sink.jobs(),
        vec![
            JobState::Started,
            JobState::Paused,
            JobState::Started,
            JobState::Cancelled
        ]
    );
    r.sink.wait("reposo", |c| {
        c.statuses()
            .last()
            .is_some_and(|s| s.state == MachineState::Idle)
    });
    // Un trabajo cancelado no se reporta además como terminado.
    thread::sleep(Duration::from_millis(100));
    assert!(!r.sink.jobs().contains(&JobState::Finished));
}

#[test]
fn a_pause_made_on_the_machine_panel_is_followed() {
    let r = rig(true);
    r.start().unwrap();
    r.sink.wait("en marcha", |c| {
        c.statuses()
            .last()
            .is_some_and(|s| s.state == MachineState::Run)
    });
    r.emu.emulator().paused = true;
    r.sink
        .wait("pausa", |c| c.jobs().contains(&JobState::Paused));
    r.emu.emulator().paused = false;
    r.sink.wait("reanudado", |c| c.jobs().len() >= 3);
}

#[test]
fn a_second_job_is_refused_while_one_is_running_and_so_is_a_jog() {
    let r = rig(true);
    r.start().unwrap();
    let e = r.start().unwrap_err();
    assert!(e.contains("trabajo en curso"), "{e}");
    let e = r.handle.jog(1.0, 0.0, 3000.0).unwrap_err();
    assert!(e.contains("trabajo en curso"), "{e}");
    let e = r.handle.info().unwrap_err();
    assert!(e.contains("termine"), "{e}");
    r.handle.cancel().unwrap();
}

#[test]
fn a_busy_machine_refuses_the_upload_without_events() {
    let r = rig(false);
    r.emu.emulator().running = true;
    let e = r.start().unwrap_err();
    assert!(e.contains("no está en reposo"), "{e}");
    assert!(!e.contains("incompleto"), "{e}");
    assert!(r.sink.jobs().is_empty());
    assert!(r.emu.emulator().stored.is_empty());
}

#[test]
fn a_bed_mismatch_with_the_machine_is_refused_before_sending() {
    let mut r = rig(false);
    r.device.bed = [400.0, 300.0];
    let e = r.start().unwrap_err();
    assert!(e.contains("no coincide") && e.contains("600"), "{e}");
    assert!(r.emu.emulator().stored.is_empty());
}

#[test]
fn a_design_outside_the_bed_is_refused_by_the_job_calculation() {
    let mut r = rig(false);
    // La cama del perfil y la de la máquina coinciden, pero el diseño no cabe en una de 20 mm.
    r.device.bed = [20.0, 20.0];
    let e = r.start().unwrap_err();
    assert!(!e.is_empty());
    assert!(r.emu.emulator().stored.is_empty());
}

#[test]
fn a_failed_upload_in_run_mode_sends_stop_and_reports_it() {
    // Sin sondeos intercalados: el orden de los datagramas es el del protocolo. Sondeo: 3
    // lecturas (0 a 2); estado antes de enviar: 3 (3 a 5); luego los paquetes del trabajo.
    let timing = RuidaTiming {
        status_interval: Duration::from_secs(60),
        ..fast()
    };
    let r = rig_with(
        true,
        EmuFaults {
            lose_packet: Some(8),
            ..EmuFaults::default()
        },
        timing,
        60,
    );
    r.start().unwrap(); // los errores a mitad de subida llegan como evento
    r.sink
        .wait("error", |c| c.jobs().contains(&JobState::Error));
    let e = r.emu.emulator();
    assert_eq!(e.stops, 1, "la subida fallida debe terminar con Detener");
    assert!(!e.running);
    drop(e);
    let msgs = r.sink.job_messages().join(" | ");
    assert!(msgs.contains("Detener"), "{msgs}");
    assert!(!r.sink.jobs().contains(&JobState::Finished), "{msgs}");
}

#[test]
fn a_machine_that_ignores_the_network_while_running_is_not_a_lost_connection() {
    let r = rig(true);
    r.start().unwrap();
    // A partir de ahora la máquina calla mientras ejecuta (observación de un foro).
    r.emu.emulator().set_faults(EmuFaults {
        ignore_network_while_running: true,
        ..EmuFaults::default()
    });
    // Pasan más sondeos que `lost_after` sin respuesta (cada uno tarda ~0,4 s en agotarse).
    r.sink.wait("aviso de silencio", |c| {
        c.logs().contains("no contesta mientras ejecuta")
    });
    thread::sleep(Duration::from_millis(1800));
    assert!(!r.handle.is_closed());
    assert!(r.sink.connections().iter().all(|c| c.connected));
    r.sink.wait("sigue marcada como en marcha", |c| {
        c.statuses()
            .last()
            .is_some_and(|s| s.state == MachineState::Run)
    });
    // Cuando la máquina termina y vuelve a contestar, el hilo se entera y avisa.
    r.emu.emulator().finish_job();
    r.sink.wait("trabajo terminado", |c| {
        c.jobs().contains(&JobState::Finished)
    });
    assert!(!r.handle.is_closed());
}

#[test]
fn a_failure_before_the_first_packet_is_an_error_to_the_caller_not_an_event() {
    let timing = RuidaTiming {
        status_interval: Duration::from_secs(60),
        ..fast()
    };
    let r = rig_with(
        false,
        EmuFaults {
            lose_packet: Some(6),
            ..EmuFaults::default()
        },
        timing,
        1000,
    );
    let e = r.start().unwrap_err();
    assert!(e.contains("sin respuesta"), "{e}");
    assert!(!e.contains("paquetes confirmados"), "{e}");
    assert!(r.sink.jobs().is_empty());
}

#[test]
fn cancel_during_the_upload_interrupts_it_and_stops_the_machine() {
    let timing = RuidaTiming {
        status_interval: Duration::from_secs(60),
        ..fast()
    };
    let r = rig_with(true, EmuFaults::default(), timing, 40);
    // La orden de cancelar llega antes de que empiece: la subida se interrumpe en el primer
    // paquete y se manda Detener (modo ejecutar).
    r.handle.shared.cancel.store(true, Ordering::SeqCst);
    r.start().unwrap();
    assert_eq!(r.sink.jobs(), vec![JobState::Cancelled]);
    assert_eq!(
        r.emu.emulator().stops,
        1,
        "ejecutar y cancelar manda Detener"
    );
    assert!(r.emu.emulator().stored.is_empty() && !r.emu.emulator().running);
    r.handle.shared.cancel.store(false, Ordering::SeqCst);
    // Y una cancelación normal durante un trabajo sigue funcionando después.
    r.start().unwrap();
    r.handle.cancel().unwrap();
    assert!(r.sink.jobs().contains(&JobState::Cancelled));
    assert!(!r.emu.emulator().running);
}

// ---------------------------------------------------------------------------
// Movimiento
// ---------------------------------------------------------------------------

#[test]
fn jog_and_jog_to_move_the_head_with_the_laser_off() {
    let r = rig(false);
    r.handle.jog_to(100.0, 50.0, 6000.0).unwrap();
    assert_eq!(r.emu.emulator().pos_um, (100_000, 50_000));
    r.handle.jog(-10.0, 5.0, 3000.0).unwrap();
    assert_eq!(r.emu.emulator().pos_um, (90_000, 55_000));
    // Fuera de la cama: lo rechaza la seguridad, sin mover nada.
    let e = r.handle.jog_to(700.0, 50.0, 6000.0).unwrap_err();
    assert!(!e.is_empty());
    assert_eq!(r.emu.emulator().pos_um, (90_000, 55_000));
    // Solo saltos `D9` (sin disparar el láser: opciones 00 y 02).
    let e = r.emu.emulator();
    assert!(!e.jogs.is_empty());
    assert!(!e.running);
}

#[test]
fn the_head_position_follows_the_profile_origin() {
    // Con el home arriba a la izquierda, la Y del documento se invierte respecto a la máquina,
    // pero el panel trabaja siempre en coordenadas de la máquina del perfil: lo que se pide es
    // lo que se lee.
    let mut dev = device(false);
    dev.origin = Origin::TopLeft;
    let emu = UdpEmulator::spawn(EmuConfig::default()).unwrap();
    let addr = emu.addr();
    let link = UdpLink::to_controller_ports(addr.ip(), addr.port(), 0).unwrap();
    let sink = Arc::new(Collect::default());
    let (conn, _) = RuidaConnection::start(
        link,
        session_config(&dev, &fast()),
        "udp:x",
        sink.clone() as Arc<dyn Sink>,
        fast(),
        None,
        &dev,
    )
    .unwrap();
    let h = conn.handle();
    h.jog_to(40.0, 25.0, 6000.0).unwrap();
    assert_eq!(emu.emulator().pos_um, (40_000, 25_000));
    sink.wait("posición", |c| {
        c.statuses()
            .last()
            .is_some_and(|s| s.work_pos == [40.0, 25.0])
    });
    conn.close();
}

#[test]
fn framing_walks_the_four_corners_without_blocking_and_can_be_stopped() {
    let r = rig(false);
    r.handle
        .frame_job(&square_doc(), &r.device, &JobOptions::default(), 6000.0)
        .unwrap();
    wait_until("las cinco esquinas", || r.emu.emulator().jogs.len() >= 5);
    // Cierra el recorrido en la primera esquina: (10, 10) mm.
    r.sink.wait("encuadre terminado", |c| {
        c.logs().contains("encuadre terminado")
    });
    assert_eq!(r.emu.emulator().pos_um, (10_000, 10_000));
    assert!(!r.emu.emulator().running);
    // El encuadre no es un trabajo: no hay eventos de trabajo.
    assert!(r.sink.jobs().is_empty());
    let e = r
        .handle
        .frame_job(&square_doc(), &r.device, &JobOptions::default(), 0.0)
        .unwrap_err();
    assert!(e.contains("velocidad"), "{e}");
}

#[test]
fn the_rotary_is_noted_in_the_log_but_the_job_is_not_scaled() {
    let mut r = rig(false);
    let plain = build_upload(&square_doc(), &r.device, &JobOptions::default(), None).unwrap();
    assert!(plain.notes.is_empty());
    r.device.rotary.enabled = true;
    let up = build_upload(&square_doc(), &r.device, &JobOptions::default(), None).unwrap();
    assert_eq!(up.notes.len(), 1);
    // El `.rd` es el mismo: la controladora gestiona el rotativo, Tlaku no escala la Y.
    assert_eq!(up.rd, plain.rd);
    r.start().unwrap();
    assert!(r.sink.logs().contains("Rotativo"));
}

#[test]
fn the_frame_follows_the_machine_corner_when_the_home_is_at_the_top() {
    let mut dev = device(false);
    dev.origin = Origin::TopLeft;
    let corners = frame_corners(&square_doc(), &dev, &JobOptions::default(), None).unwrap();
    // Documento (10..30, 10..30) con el home arriba: Y de la máquina = 400 - y.
    let ys: Vec<f64> = corners.iter().map(|c| c.1).collect();
    assert!(
        (ys.iter().cloned().fold(f64::MAX, f64::min) - 370.0).abs() < 1e-6,
        "{corners:?}"
    );
    assert!(
        (ys.iter().cloned().fold(f64::MIN, f64::max) - 390.0).abs() < 1e-6,
        "{corners:?}"
    );
    assert_eq!(corners.first(), corners.last());
    assert_eq!(corners.len(), 5);
}

// ---------------------------------------------------------------------------
// Información y archivos
// ---------------------------------------------------------------------------

#[test]
fn info_reads_the_known_settings_and_never_writes() {
    let r = rig(false);
    let i = r.handle.info().unwrap();
    assert_eq!(i.model.as_deref(), Some("RDC6442S"));
    assert_eq!(i.card_id, "0x65106510");
    assert_eq!(i.magic, 0x88);
    assert_eq!(i.bed, Some([600.0, 400.0]));
    assert_eq!(i.rapid_mm_s, Some(200.0));
    assert_eq!(i.axis_accel_mm_s2, [Some(3000.0), Some(3000.0)]);
    assert_eq!(i.total_jobs, Some(42));
    assert_eq!(i.documents, Some(0));
    let e = r.emu.emulator();
    assert!(
        e.commands()
            .unwrap()
            .iter()
            .all(|c| matches!(c, tlaku_ruida::cmd::Command::MemRead { .. }))
    );
}

#[test]
fn the_file_list_shows_the_stored_documents() {
    let r = rig(false);
    assert!(r.handle.files().unwrap().is_empty());
    r.start().unwrap();
    r.start().unwrap();
    let files = r.handle.files().unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].index, 1);
    assert!(files[0].name.as_deref().unwrap().starts_with("TLAKU"));
    assert_eq!(r.handle.info().unwrap().documents, Some(2));
    // Leer la lista no borra ni ejecuta nada.
    let e = r.emu.emulator();
    assert_eq!(e.stored.len(), 2);
    assert!(!e.running);
}

// ---------------------------------------------------------------------------
// Despacho
// ---------------------------------------------------------------------------

#[test]
fn realtime_commands_map_to_the_ruida_equivalents() {
    let r = rig(true);
    r.start().unwrap();
    let m = Machine::Ruida(r.handle.clone());
    m.realtime(RealtimeCommand::Hold).unwrap();
    assert!(r.emu.emulator().paused);
    m.realtime(RealtimeCommand::Resume).unwrap();
    assert!(!r.emu.emulator().paused);
    m.realtime(RealtimeCommand::Status).unwrap();
    m.realtime(RealtimeCommand::JogCancel).unwrap();
    assert!(!r.emu.emulator().running);
    assert!(matches!(m.grbl(), Err(e) if e.contains("solo de GRBL")));
    assert!(m.ruida().is_ok());
}

// ---------------------------------------------------------------------------
// Máquina virtual
// ---------------------------------------------------------------------------

#[test]
fn the_virtual_port_is_recognised_with_and_without_speed() {
    assert!(is_virtual("virtual:ruida"));
    assert!(is_virtual("virtual:ruida@5"));
    assert!(!is_virtual("virtual:falcon2"));
    assert!(!is_virtual("udp://192.168.0.5"));
    assert_eq!(virtual_speed("virtual:ruida").unwrap(), 1.0);
    assert_eq!(virtual_speed("virtual:ruida@20").unwrap(), 20.0);
    assert!(virtual_speed("virtual:ruida@0").is_err());
    assert!(virtual_speed("virtual:ruida@x").is_err());
}

#[test]
fn the_virtual_machine_runs_a_whole_job_with_a_real_udp_path() {
    let sink = Arc::new(Collect::default());
    let (conn, info) = RuidaConnection::connect(
        &device(true),
        "virtual:ruida@100",
        sink.clone() as Arc<dyn Sink>,
        fast(),
    )
    .unwrap();
    assert!(info.firmware.contains("RDC6442S"));
    assert!(sink.logs().contains("Máquina virtual"));
    let h = conn.handle();
    assert!(h.is_virtual());
    h.start_job(&square_doc(), &device(true), &JobOptions::default())
        .unwrap();
    // El trabajo "dura" lo estimado entre 100: termina solo.
    sink.wait("fin del trabajo virtual", |c| {
        c.jobs().contains(&JobState::Finished)
    });
    sink.wait("de vuelta en reposo", |c| {
        c.statuses()
            .last()
            .is_some_and(|s| s.state == MachineState::Idle)
    });
    // Y guardar como documento también funciona.
    h.start_job(&square_doc(), &device(false), &JobOptions::default())
        .unwrap();
    assert_eq!(h.files().unwrap().len(), 1);
    conn.close();
    assert!(h.is_closed());
}

#[test]
fn closing_the_connection_stops_a_running_job() {
    let r = rig(true);
    r.start().unwrap();
    let sink = r.sink.clone();
    let emu = r.emu;
    r.conn.close();
    let e = emu.emulator();
    assert_eq!(e.stops, 1);
    assert!(!e.running);
    drop(e);
    assert!(sink.jobs().contains(&JobState::Cancelled));
}
