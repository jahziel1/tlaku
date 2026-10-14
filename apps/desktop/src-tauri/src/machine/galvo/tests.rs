//! Pruebas de la conexión galvo contra la tarjeta simulada de `tlaku-galvo` (sin hardware): el
//! mismo camino que con una tarjeta, con el reloj virtual del emulador marcado por las lecturas.

use super::*;
use crate::machine::active::{ActiveMachine, Machine};
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Polyline, Rgb};
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{
    DesignObject, Document, GalvoDevice, JobOptions, LayerMode, LayerSettings, ObjectKind,
    RealtimeCommand, RepeatMarking, SplitMarking, VectorObject,
};
use tlaku_galvo::emu::{EmuHandle, Faults, lock};

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

    fn galvo(&self) -> Vec<GalvoEvent> {
        self.all()
            .into_iter()
            .filter_map(|e| match e {
                Emit::Galvo(g) => Some(g),
                _ => None,
            })
            .collect()
    }

    fn steps(&self) -> Vec<StepInfo> {
        self.galvo()
            .into_iter()
            .filter_map(|g| match g {
                GalvoEvent::Step { info } => Some(info),
                _ => None,
            })
            .collect()
    }

    fn waits(&self) -> Vec<StepInfo> {
        self.galvo()
            .into_iter()
            .filter_map(|g| match g {
                GalvoEvent::WaitMove { info } => Some(info),
                _ => None,
            })
            .collect()
    }

    fn loops(&self) -> u32 {
        self.galvo()
            .into_iter()
            .filter_map(|g| match g {
                GalvoEvent::Light { loops, .. } => Some(loops),
                _ => None,
            })
            .max()
            .unwrap_or(0)
    }

    fn may_be_on_events(&self) -> usize {
        self.galvo()
            .into_iter()
            .filter(|g| matches!(g, GalvoEvent::LaserMayBeOn { .. }))
            .count()
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
    let end = Instant::now() + Duration::from_secs(20);
    while !f() {
        assert!(Instant::now() < end, "se agotó la espera de: {what}");
        thread::sleep(Duration::from_millis(3));
    }
}

fn fast() -> GalvoTimingCfg {
    GalvoTimingCfg {
        status_interval: Duration::from_millis(20),
        lost_after: 3,
        progress_interval: Duration::ZERO,
        light_interval: Duration::ZERO,
        virtual_pace: Duration::ZERO,
    }
}

/// Un galvo de fibra de 110 mm: la cama es el campo.
fn device() -> DeviceProfile {
    DeviceProfile {
        kind: DeviceKind::Galvo,
        bed: [110.0, 110.0],
        galvo: GalvoDevice {
            field_mm: 110.0,
            ..GalvoDevice::default()
        },
        ..DeviceProfile::default()
    }
}

fn layer() -> LayerSettings {
    LayerSettings {
        mode: LayerMode::Line,
        // El galvo se piensa en mm/s: 1000 mm/s son 60000 mm/min.
        speed: 60_000.0,
        power: 20.0,
        ..LayerSettings::for_color(Rgb(255, 0, 0))
    }
}

fn vector(id: &str, pts: &[(f64, f64)]) -> DesignObject {
    DesignObject {
        id: id.into(),
        layer: Rgb(255, 0, 0),
        transform: IDENTITY,
        kind: ObjectKind::Vector(VectorObject::new(vec![BezPath::from(&Polyline {
            points: pts.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            closed: true,
        })])),
        locked: false,
        hidden: false,
        group: None,
        priority: None,
        power_scale: None,
        shape: None,
        start: None,
        manual_tabs: Vec::new(),
        array: None,
    }
}

fn rect(id: &str, x0: f64, y0: f64, x1: f64, y1: f64) -> DesignObject {
    vector(id, &[(x0, y0), (x1, y0), (x1, y1), (x0, y1)])
}

fn doc_of(objects: Vec<DesignObject>) -> Document {
    Document {
        objects,
        layers: vec![layer()],
        ..Document::default()
    }
}

/// Cuadrado de 20 x 20 mm en el centro del campo.
fn square_doc() -> Document {
    doc_of(vec![rect("a", 45.0, 45.0, 65.0, 65.0)])
}

/// Un relleno grande y denso: dura lo bastante para detenerlo a mitad.
fn big_fill_doc() -> Document {
    let mut doc = doc_of(vec![rect("a", 5.0, 5.0, 105.0, 105.0)]);
    doc.layers[0].mode = LayerMode::Fill;
    doc.layers[0].interval = 0.05;
    doc
}

struct Rig {
    emu: EmuHandle,
    sink: Arc<Collect>,
    conn: GalvoConnection,
    handle: GalvoHandle,
    device: DeviceProfile,
}

/// `step` es lo que tarda (en tiempo real) cada lectura de estado: 0 = tan rápido como se pueda,
/// unos milisegundos = un trabajo que dura lo bastante para detenerlo.
fn rig_with(device: DeviceProfile, faults: Faults, step: Duration) -> Result<Rig, String> {
    let (transport, emu) = emulator(EmuConfig {
        source: device.galvo.source,
        field_mm: device.galvo.field_mm,
        faults,
        ..EmuConfig::default()
    });
    let sink = Arc::new(Collect::default());
    let mut cfg = session_config(&device).unwrap();
    cfg.poll_interval = Duration::ZERO;
    let (conn, _) = GalvoConnection::start(
        Box::new(Paced {
            inner: transport,
            step,
        }),
        cfg,
        device.bed,
        VIRTUAL_PORT,
        sink.clone() as Arc<dyn Sink>,
        fast(),
        true,
    )?;
    let handle = conn.handle();
    Ok(Rig {
        emu,
        sink,
        conn,
        handle,
        device,
    })
}

fn rig() -> Rig {
    rig_with(device(), Faults::default(), Duration::ZERO).unwrap()
}

impl Rig {
    fn start(&self, doc: &Document) -> Result<(), String> {
        self.handle
            .start_job(doc, &self.device, &JobOptions::default(), false)
    }
}

// ---------------------------------------------------------------------------
// Conexión, sondeo y seguridad del USB
// ---------------------------------------------------------------------------

#[test]
fn connecting_only_reads_and_reports_the_board() {
    let rig = rig();
    {
        let e = lock(&rig.emu);
        assert!(e.violations.is_empty(), "{:?}", e.violations);
        assert_eq!(e.batches_received, 0, "el sondeo no envía listas");
        assert!(e.laser_safe());
        assert!(!e.armed());
        // Solo preguntó serie, versión y entradas.
        assert_eq!(e.immediates.len(), 3, "{:?}", e.immediates);
    }
    let st = rig.handle.state();
    assert!(st.virtual_machine && rig.handle.is_virtual());
    assert_eq!((st.vendor_id, st.product_id), (0x9588, 0x9899));
    assert!(!st.laser_may_be_on && !st.red_light_on && !st.working);
    assert!(!st.position_known);
    assert_eq!(st.field_mm, 110.0);
    assert!(rig.sink.logs().contains("sondeo galvo (solo lectura)"));
    assert!(rig.sink.logs().contains("EXPERIMENTAL"));
    assert!(
        rig.sink
            .connections()
            .iter()
            .any(|c| c.connected && c.port.as_deref() == Some("virtual:galvo"))
    );
    // En reposo sondea el estado y las entradas, y emite el estado de la máquina.
    wait_until("el primer estado", || rig.handle.status_report().is_some());
    assert!(rig.handle.state().inputs_known);
    rig.conn.close();
}

#[test]
fn usb_needs_the_full_safety_acknowledgement_before_touching_usb() {
    let sink: Arc<dyn Sink> = Arc::new(Collect::default());
    let d = device();
    let partial = [
        SafetyAck::default(),
        SafetyAck {
            eyewear: true,
            ..SafetyAck::default()
        },
        SafetyAck {
            eyewear: true,
            enclosure: true,
            emergency_stop: false,
        },
        SafetyAck {
            eyewear: false,
            enclosure: true,
            emergency_stop: true,
        },
    ];
    // Sin reconocimiento y con reconocimientos incompletos: el mismo error, antes de abrir nada.
    let r = GalvoConnection::connect(&d, "usb:galvo:0", None, sink.clone(), fast());
    assert_eq!(r.err(), Some(ack_required()));
    for ack in partial {
        let r = GalvoConnection::connect(&d, "usb:galvo:0", Some(&ack), sink.clone(), fast());
        assert_eq!(r.err(), Some(ack_required()), "{ack:?}");
        assert!(ack.token().is_err());
    }
    // El token solo existe con las tres confirmaciones.
    let full = SafetyAck {
        eyewear: true,
        enclosure: true,
        emergency_stop: true,
    };
    assert!(full.token().is_ok());
    // La máquina virtual no necesita el reconocimiento: no hay láser.
    let (conn, info) = GalvoConnection::connect(&d, VIRTUAL_PORT, None, sink, fast()).unwrap();
    assert!(info.firmware.contains("9588:9899"));
    conn.close();
}

#[test]
fn ports_are_recognized_and_foreign_ports_rejected() {
    assert!(is_virtual("virtual:galvo"));
    assert!(!is_virtual("virtual:k40"));
    assert_eq!(usb_index("usb:galvo:2"), Some(2));
    assert_eq!(usb_index("usb:k40:2"), None);
    assert_eq!(usb_index("usb:galvo:x"), None);
    let sink: Arc<dyn Sink> = Arc::new(Collect::default());
    let err = GalvoConnection::connect(&device(), "COM3", None, sink.clone(), fast())
        .err()
        .unwrap();
    assert!(err.contains("no es un puerto galvo"), "{err}");
    // Un perfil que no es galvo no se abre.
    let grbl = DeviceProfile::default();
    let err = GalvoConnection::connect(&grbl, VIRTUAL_PORT, None, sink, fast())
        .err()
        .unwrap();
    assert!(err.contains("no es de tipo galvo"), "{err}");
}

#[test]
fn only_the_document_origin_is_accepted_and_virtual_speed_is_parsed() {
    let mut d = device();
    d.origin = tlaku_doc::Origin::TopLeft;
    let err = session_config(&d).err().unwrap();
    assert!(err.contains("abajo a la izquierda"), "{err}");
    assert!(is_virtual("virtual:galvo@5") && is_virtual(VIRTUAL_PORT));
    assert_eq!(virtual_speed("virtual:galvo@5"), Ok(5.0));
    assert_eq!(virtual_speed("virtual:galvo"), Ok(1.0));
    assert!(virtual_speed("virtual:galvo@x").is_err());
    assert!(virtual_speed("virtual:galvo@0").is_err());
    // Una velocidad válida abre la máquina virtual.
    let sink: Arc<dyn Sink> = Arc::new(Collect::default());
    let (conn, _) =
        GalvoConnection::connect(&device(), "virtual:galvo@20", None, sink, fast()).unwrap();
    conn.close();
}

#[test]
fn an_invalid_profile_is_refused_in_spanish() {
    // Una fuente MOPA sin anchos de pulso en el perfil no es válida.
    let mut d = device();
    d.galvo.source = tlaku_doc::LaserSource::FiberMopa;
    let sink: Arc<dyn Sink> = Arc::new(Collect::default());
    let err = GalvoConnection::connect(&d, VIRTUAL_PORT, None, sink, fast())
        .err()
        .unwrap();
    assert!(err.contains("perfil del galvo"), "{err}");
}

// ---------------------------------------------------------------------------
// Marcar y detener
// ---------------------------------------------------------------------------

#[test]
fn a_mark_runs_with_progress_and_leaves_the_source_off() {
    let rig = rig();
    rig.start(&square_doc()).unwrap();
    rig.sink.wait("el fin del trabajo", |s| {
        s.job_states().contains(&JobState::Finished)
    });
    let states = rig.sink.job_states();
    assert_eq!(states.first(), Some(&JobState::Started), "{states:?}");
    assert_eq!(states.last(), Some(&JobState::Finished));
    assert!(!rig.sink.progress().is_empty());
    let last = rig.sink.progress().last().copied().unwrap();
    assert_eq!(last.acked, last.total);
    {
        let e = lock(&rig.emu);
        assert!(e.violations.is_empty(), "{:?}", e.violations);
        assert!(
            e.armed_history.contains(&true),
            "la fuente se armó para marcar"
        );
        assert!(e.laser_safe(), "la fuente quedó apagada");
        assert!(!e.emitted().is_empty(), "el haz salió");
    }
    // Sigue libre para otro trabajo y para desconectar.
    wait_until("la máquina libre", || !rig.handle.state().working);
    let st = rig.handle.state();
    assert!(!st.laser_may_be_on);
    assert_eq!(rig.sink.steps().len(), 1);
    assert!(
        rig.sink.waits().is_empty(),
        "un trabajo simple no pide mover nada"
    );
    rig.conn.close();
}

#[test]
fn stopping_a_mark_cancels_it_and_switches_everything_off() {
    let rig = rig_with(device(), Faults::default(), Duration::from_millis(2)).unwrap();
    rig.start(&big_fill_doc()).unwrap();
    // Con el trabajo en marcha no se acepta otro.
    let again = rig.start(&square_doc());
    assert!(again.unwrap_err().contains("ocupada"));
    rig.handle.cancel().unwrap();
    rig.sink.wait("el trabajo detenido", |s| {
        s.job_states().contains(&JobState::Cancelled)
    });
    assert!(!rig.sink.job_states().contains(&JobState::Finished));
    let msg = rig.sink.jobs().last().unwrap().message.clone();
    assert!(msg.contains("detenido") && !has_stop_code(&msg), "{msg}");
    {
        let e = lock(&rig.emu);
        assert!(e.violations.is_empty(), "{:?}", e.violations);
        assert!(e.laser_safe());
    }
    wait_until("la máquina libre", || !rig.handle.state().working);
    assert!(!rig.handle.state().laser_may_be_on);
    // Un «detener» anterior no cancela el trabajo siguiente.
    rig.start(&square_doc()).unwrap();
    rig.sink.wait("el segundo trabajo", |s| {
        s.job_states().contains(&JobState::Finished)
    });
    rig.conn.close();
}

#[test]
fn red_run_never_arms_the_source() {
    let rig = rig();
    rig.handle
        .start_job(&square_doc(), &rig.device, &JobOptions::default(), true)
        .unwrap();
    rig.sink.wait("el fin del recorrido", |s| {
        s.job_states().contains(&JobState::Finished)
    });
    let msg = rig.sink.jobs().last().unwrap().message.clone();
    assert!(msg.contains("nunca se armó"), "{msg}");
    let e = lock(&rig.emu);
    assert!(!e.armed_history.contains(&true));
    assert!(e.emitted().is_empty(), "el haz nunca salió");
    assert!(e.trace.iter().any(|s| s.red), "el puntero rojo sí dibujó");
    assert!(e.laser_safe());
    drop(e);
    rig.conn.close();
}

#[test]
fn a_design_that_does_not_fit_the_field_is_refused_before_touching_the_board() {
    let rig = rig();
    let before = lock(&rig.emu).immediates.len();
    // Un cuadrado de 200 mm no cabe en el campo de 110 mm (y el marcado dividido está apagado).
    let doc = doc_of(vec![rect("a", 0.0, 0.0, 200.0, 200.0)]);
    let err = rig.start(&doc).unwrap_err();
    assert!(err.contains("campo"), "{err}");
    let err = rig
        .handle
        .frame_job(&doc, &rig.device, &JobOptions::default(), false)
        .unwrap_err();
    assert!(err.contains("campo"), "{err}");
    assert_eq!(lock(&rig.emu).immediates.len(), before, "no se envió nada");
    assert!(rig.sink.job_states().is_empty());
    assert!(!rig.handle.state().working);
    rig.conn.close();
}

// ---------------------------------------------------------------------------
// Encuadre con el puntero rojo
// ---------------------------------------------------------------------------

#[test]
fn framing_loops_with_the_red_pointer_until_stopped() {
    let rig = rig_with(device(), Faults::default(), Duration::from_millis(1)).unwrap();
    rig.handle
        .frame_job(&square_doc(), &rig.device, &JobOptions::default(), false)
        .unwrap();
    // Repite vuelta tras vuelta sin que nadie lo pida.
    rig.sink.wait("tres vueltas", |s| s.loops() >= 3);
    assert!(rig.handle.state().working);
    assert!(
        rig.handle
            .frame_job(&square_doc(), &rig.device, &JobOptions::default(), true)
            .unwrap_err()
            .contains("ocupada")
    );
    rig.handle.cancel().unwrap();
    wait_until("el encuadre detenido", || !rig.handle.state().working);
    {
        let e = lock(&rig.emu);
        assert!(e.violations.is_empty(), "{:?}", e.violations);
        assert!(e.emitted().is_empty(), "el encuadre nunca saca el haz");
        assert!(!e.armed_history.contains(&true));
        assert!(e.trace.iter().any(|s| s.red));
        assert!(e.laser_safe());
        assert!(!e.red_on(), "el puntero queda apagado al detener");
    }
    // Detener un encuadre no es un error del trabajo.
    assert!(!rig.sink.job_states().contains(&JobState::Error));
    assert!(!rig.handle.state().laser_may_be_on);
    // El contorno también se encuadra.
    rig.handle
        .frame_job(&square_doc(), &rig.device, &JobOptions::default(), true)
        .unwrap();
    rig.sink.wait("el contorno", |s| s.loops() >= 1);
    rig.handle.cancel().unwrap();
    wait_until("el contorno detenido", || !rig.handle.state().working);
    assert!(lock(&rig.emu).laser_safe());
    rig.conn.close();
}

#[test]
fn the_red_pointer_toggles_and_disconnecting_switches_it_off() {
    let rig = rig();
    rig.handle.set_red_light(true).unwrap();
    assert!(rig.handle.state().red_light_on);
    assert!(lock(&rig.emu).red_on());
    assert!(!lock(&rig.emu).armed());
    rig.handle.set_red_light(false).unwrap();
    assert!(!rig.handle.state().red_light_on);
    assert!(!lock(&rig.emu).red_on());
    // Encendido al desconectar: se apaga solo.
    rig.handle.set_red_light(true).unwrap();
    let emu = rig.emu.clone();
    let sink = rig.sink.clone();
    rig.conn.close();
    assert!(!lock(&emu).red_on());
    assert!(lock(&emu).laser_safe());
    assert!(
        sink.connections()
            .iter()
            .any(|c| !c.connected && c.error.is_none())
    );
}

#[test]
fn the_pointer_cannot_be_toggled_while_something_runs() {
    let rig = rig_with(device(), Faults::default(), Duration::from_millis(2)).unwrap();
    rig.handle
        .frame_job(&square_doc(), &rig.device, &JobOptions::default(), false)
        .unwrap();
    // Ni encenderlo ni apagarlo se encolan detrás del encuadre: la llamada no se queda colgada.
    assert!(
        rig.handle
            .set_red_light(true)
            .unwrap_err()
            .contains("ocupada")
    );
    assert!(
        rig.handle
            .set_red_light(false)
            .unwrap_err()
            .contains("Detener")
    );
    // Y un «detener» en vuelo no se borra por intentarlo.
    rig.handle.cancel().unwrap();
    let _ = rig.handle.set_red_light(false);
    wait_until("el encuadre detenido", || !rig.handle.state().working);
    assert!(lock(&rig.emu).laser_safe());
    rig.conn.close();
}

#[test]
fn cancelling_when_idle_switches_the_pointer_off() {
    let rig = rig();
    rig.handle.set_red_light(true).unwrap();
    rig.handle.cancel().unwrap();
    assert!(!lock(&rig.emu).red_on());
    assert!(!rig.handle.state().red_light_on);
    rig.conn.close();
}

#[test]
fn jogging_moves_the_beam_inside_the_field_only() {
    let rig = rig();
    // Sin posición conocida, el salto relativo parte del centro del campo.
    rig.handle.jog(5.0, -5.0, 0.0).unwrap();
    let st = rig.handle.state();
    assert!(st.position_known);
    assert_eq!(st.position, [60.0, 50.0]);
    rig.handle.jog_to(10.0, 100.0, 0.0).unwrap();
    assert_eq!(rig.handle.work_pos(), Some([10.0, 100.0]));
    let err = rig.handle.jog_to(200.0, 10.0, 0.0).unwrap_err();
    assert!(err.contains("fuera del campo"), "{err}");
    assert!(rig.handle.jog_to(f64::NAN, 1.0, 0.0).is_err());
    assert_eq!(rig.handle.work_pos(), Some([10.0, 100.0]));
    {
        let e = lock(&rig.emu);
        assert!(e.violations.is_empty());
        assert!(!e.armed());
        assert!(e.emitted().is_empty());
    }
    // Un trabajo deja la posición como desconocida.
    rig.start(&square_doc()).unwrap();
    rig.sink.wait("el trabajo", |s| {
        s.job_states().contains(&JobState::Finished)
    });
    assert!(!rig.handle.state().position_known);
    rig.conn.close();
}

// ---------------------------------------------------------------------------
// Marcado dividido, repetido y rotativo
// ---------------------------------------------------------------------------

/// Dos cuadrados lejanos: con un campo de 100 mm el diseño es de 3 x 1 piezas y la del medio
/// queda vacía.
fn wide_doc() -> Document {
    doc_of(vec![
        rect("a", 5.0, 40.0, 25.0, 60.0),
        rect("b", 275.0, 40.0, 295.0, 60.0),
    ])
}

fn split_device() -> DeviceProfile {
    let mut d = device();
    d.bed = [100.0, 100.0];
    d.galvo.field_mm = 100.0;
    d.galvo.split = SplitMarking {
        enabled: true,
        ..SplitMarking::default()
    };
    d
}

#[test]
fn prepare_splits_skips_empty_pieces_and_merges_their_moves() {
    let d = split_device();
    let job = prepare(&wide_doc(), &d, &JobOptions::default(), false).unwrap();
    // Tres piezas en X; la del centro no tiene nada y se omite.
    assert_eq!(
        job.steps.len(),
        2,
        "{:?}",
        job.steps.iter().map(|s| &s.info).collect::<Vec<_>>()
    );
    assert!(
        job.warnings.iter().any(|w| w.contains("se omite")),
        "{:?}",
        job.warnings
    );
    let (a, b) = (&job.steps[0].info, &job.steps[1].info);
    assert_eq!((a.step, a.steps, b.step, b.steps), (1, 2, 2, 2));
    assert_eq!((a.pieces, a.repeats), (3, 1));
    assert!(!a.needs_move(), "el primer paso no mueve nada");
    // El material se mueve de la primera a la tercera pieza (centros en 55 y 250 mm) de golpe,
    // sumando el movimiento de la pieza vacía; no son dos movimientos.
    assert!(b.needs_move());
    assert!((b.dx + 195.0).abs() < 1e-6 && b.dy.abs() < 1e-6, "{b:?}");
    assert_eq!((b.piece, b.pieces), (3, 3));
    assert_eq!(job.manual_moves(), 1);
}

#[test]
fn a_split_job_waits_for_the_user_to_move_the_material() {
    let d = split_device();
    let rig = rig_with(d, Faults::default(), Duration::ZERO).unwrap();
    rig.start(&wide_doc()).unwrap();
    rig.sink
        .wait("la espera del movimiento", |s| !s.waits().is_empty());
    let w = rig.sink.waits().pop().unwrap();
    assert_eq!((w.step, w.steps), (2, 2));
    assert!(rig.handle.state().waiting_move.is_some());
    // Mientras espera no marca el segundo paso ni termina.
    thread::sleep(Duration::from_millis(150));
    assert!(!rig.sink.job_states().contains(&JobState::Finished));
    assert_eq!(rig.sink.steps().len(), 1);
    let armed_before = lock(&rig.emu).batches_received;
    rig.handle.confirm_move().unwrap();
    rig.sink
        .wait("el fin", |s| s.job_states().contains(&JobState::Finished));
    assert_eq!(rig.sink.steps().len(), 2);
    assert_eq!(rig.sink.waits().len(), 1);
    assert!(lock(&rig.emu).batches_received > armed_before);
    assert!(rig.handle.state().waiting_move.is_none());
    // Sin nada que esperar, confirmar es un error claro.
    assert!(
        rig.handle
            .confirm_move()
            .unwrap_err()
            .contains("Ningún paso")
    );
    // El progreso acumula las tandas de todos los pasos.
    let p = rig.sink.progress().last().copied().unwrap();
    assert_eq!(p.acked, p.total);
    let e = lock(&rig.emu);
    assert!(e.violations.is_empty());
    assert!(e.laser_safe());
    drop(e);
    rig.conn.close();
}

#[test]
fn stopping_while_waiting_for_the_material_cancels_the_job() {
    let rig = rig_with(split_device(), Faults::default(), Duration::ZERO).unwrap();
    rig.start(&wide_doc()).unwrap();
    rig.sink.wait("la espera", |s| !s.waits().is_empty());
    rig.handle.cancel().unwrap();
    rig.sink.wait("el trabajo cancelado", |s| {
        s.job_states().contains(&JobState::Cancelled)
    });
    assert_eq!(rig.sink.steps().len(), 1, "el segundo paso no se marcó");
    assert!(rig.handle.state().waiting_move.is_none());
    assert!(lock(&rig.emu).laser_safe());
    wait_until("libre", || !rig.handle.state().working);
    rig.conn.close();
}

#[test]
fn repeat_marking_marks_the_design_several_times() {
    let mut d = device();
    d.galvo.repeat = RepeatMarking {
        count: 3,
        increment_mm: [0.0, 0.0],
        pause_ms: 0.0,
    };
    let rig = rig_with(d, Faults::default(), Duration::ZERO).unwrap();
    rig.start(&square_doc()).unwrap();
    rig.sink
        .wait("el fin", |s| s.job_states().contains(&JobState::Finished));
    let steps = rig.sink.steps();
    assert_eq!(steps.len(), 3);
    assert_eq!(
        steps.iter().map(|s| s.repeat).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    // Sin avance entre repeticiones el material no se mueve: no pide confirmar nada.
    assert!(rig.sink.waits().is_empty());
    assert!(lock(&rig.emu).laser_safe());
    rig.conn.close();
}

#[test]
fn repeat_with_an_increment_asks_to_move_the_material_each_time() {
    let mut d = device();
    d.galvo.repeat = RepeatMarking {
        count: 2,
        increment_mm: [0.0, 12.0],
        pause_ms: 10.0,
    };
    let rig = rig_with(d, Faults::default(), Duration::ZERO).unwrap();
    rig.start(&square_doc()).unwrap();
    rig.sink.wait("la espera", |s| !s.waits().is_empty());
    let w = rig.sink.waits().pop().unwrap();
    assert_eq!((w.repeat, w.repeats), (2, 2));
    assert!((w.dy - 12.0).abs() < 1e-9 && w.dx == 0.0, "{w:?}");
    rig.handle.confirm_move().unwrap();
    rig.sink
        .wait("el fin", |s| s.job_states().contains(&JobState::Finished));
    assert_eq!(rig.sink.steps().len(), 2);
    rig.conn.close();
}

#[test]
fn rotary_jobs_turn_the_rotary_between_pieces() {
    let mut d = split_device();
    d.rotary.enabled = true;
    d.rotary.object_diameter = 40.0;
    // Un diseño alto (en Y) que da la vuelta a la pieza en franjas.
    let doc = doc_of(vec![rect("a", 40.0, 0.0, 60.0, 250.0)]);
    let job = prepare(&doc, &d, &JobOptions::default(), false).unwrap();
    assert!(job.steps.len() >= 2);
    let second = &job.steps[1].info;
    assert!(
        second.rotary_deg != 0.0 && second.dx == 0.0 && second.dy == 0.0,
        "{second:?}"
    );
    assert!(second.needs_move());
}

// ---------------------------------------------------------------------------
// Fallos: la fuente puede seguir encendida
// ---------------------------------------------------------------------------

#[test]
fn a_dropped_usb_mid_job_flags_that_the_source_may_be_on() {
    // El USB se cae tras unas escrituras: ni el trabajo ni el apagado llegan a la tarjeta.
    let faults = Faults {
        disconnect_after_writes: Some(14),
        ..Faults::default()
    };
    let rig = rig_with(device(), faults, Duration::ZERO).unwrap();
    let r = rig.start(&big_fill_doc());
    // Falla antes de la primera tanda confirmada (rechazo directo) o a mitad (evento): en ambos
    // casos el mensaje lo dice y la conexión queda marcada.
    let msg = match r {
        Err(e) => e,
        Ok(()) => {
            rig.sink
                .wait("el error", |s| s.job_states().contains(&JobState::Error));
            rig.sink.jobs().last().unwrap().message.clone()
        }
    };
    assert!(has_stop_code(&msg), "{msg}");
    assert!(rig.handle.state().laser_may_be_on);
    assert!(rig.sink.may_be_on_events() >= 1);
    assert!(rig.sink.logs().contains("CORTA LA ENERGÍA"));
    // La marca se mantiene: ni siquiera un nuevo intento la borra mientras no haya un apagado
    // confirmado (aquí el USB sigue caído).
    let _ = rig.handle.cancel();
    assert!(rig.handle.state().laser_may_be_on);
    // Al cerrar con la fuente en duda, la desconexión también lo dice.
    let sink = rig.sink.clone();
    rig.conn.close();
    let last = sink.connections().pop().unwrap();
    assert!(!last.connected);
    assert!(has_stop_code(&last.error.unwrap_or_default()));
}

/// ¿El mensaje lleva el código estable de «detener NO se confirmó»? (la interfaz lo reconoce así,
/// en cualquier idioma).
fn has_stop_code(msg: &str) -> bool {
    tlaku_i18n::split_code(msg).0 == Some(STOP_UNCONFIRMED)
}

#[test]
fn the_unconfirmed_stop_is_still_detected_and_shown_in_english() {
    use tlaku_i18n::{Locale, split_code, with_locale};
    let e = JobError {
        cause: SessionError::Cancelled,
        batches_sent: 1,
        batches_total: 2,
        stop: StopOutcome::Unconfirmed,
    };
    let msg = with_locale(Locale::En, || job_error_text(&e));
    // La interfaz reconoce el aviso por el código, igual en español que en inglés.
    assert!(has_stop_code(&msg), "{msg}");
    assert_eq!(
        split_code(&msg).0,
        Some(tlaku_galvo::session::STOP_UNCONFIRMED_CODE)
    );
    // El texto propio de la parada en reposo sale en inglés, con el mismo código.
    let own = with_locale(Locale::En, || {
        tlaku_i18n::coded(STOP_UNCONFIRMED, tr!("desktop.galvo.stop_unconfirmed"))
    });
    assert!(has_stop_code(&own), "{own}");
    assert!(
        split_code(&own).1.starts_with("There was NO confirmation"),
        "{own}"
    );
    // Una parada confirmada no lleva el código.
    let ok = JobError {
        stop: StopOutcome::Acked,
        ..e
    };
    assert!(!has_stop_code(&with_locale(Locale::En, || job_error_text(
        &ok
    ))));
}

#[test]
fn a_confirmed_stop_clears_the_laser_warning() {
    let rig = rig();
    // Se fuerza la marca como la dejaría un apagado sin confirmar.
    rig.handle
        .shared
        .laser_may_be_on
        .store(true, Ordering::SeqCst);
    assert!(rig.handle.state().laser_may_be_on);
    rig.handle.cancel().unwrap();
    assert!(!rig.handle.state().laser_may_be_on);
    rig.conn.close();
}

#[test]
fn the_unconfirmed_text_matches_what_the_driver_says() {
    // La interfaz reconoce el aviso por el código estable del mensaje (no por su texto).
    let e = JobError {
        cause: SessionError::Cancelled,
        batches_sent: 1,
        batches_total: 2,
        stop: StopOutcome::Unconfirmed,
    };
    assert!(e.laser_may_be_on());
    let text = job_error_text(&e);
    assert!(has_stop_code(&text), "{text}");
    assert_eq!(
        tlaku_i18n::split_code(&text).0,
        Some(tlaku_galvo::session::STOP_UNCONFIRMED_CODE)
    );
}

// ---------------------------------------------------------------------------
// Perfil, despacho y operaciones que no aplican
// ---------------------------------------------------------------------------

#[test]
fn a_changed_driver_profile_requires_reconnecting() {
    let rig = rig();
    let mut changed = rig.device.clone();
    changed.galvo.correction.scale = [1.01, 1.0];
    let err = rig
        .handle
        .start_job(&square_doc(), &changed, &JobOptions::default(), false)
        .unwrap_err();
    assert!(err.contains("cambió"), "{err}");
    let mut other_bed = rig.device.clone();
    other_bed.bed = [120.0, 110.0];
    assert!(
        rig.handle
            .frame_job(&square_doc(), &other_bed, &JobOptions::default(), false)
            .is_err()
    );
    // El marcado repetido lo calcula el pipeline: cambiarlo no exige reconectar.
    let mut repeat = rig.device.clone();
    repeat.galvo.repeat.count = 2;
    assert!(
        rig.handle
            .start_job(&square_doc(), &repeat, &JobOptions::default(), true)
            .is_ok()
    );
    rig.sink.wait("el recorrido", |s| {
        s.job_states().contains(&JobState::Finished)
    });
    rig.conn.close();
}

#[test]
fn the_galvo_has_no_pause_and_only_the_galvo_commands_apply() {
    let rig = rig();
    let sink = rig.sink.clone();
    let device = rig.device.clone();
    let emu = rig.emu.clone();
    let m = Machine::Galvo(rig.handle.clone());
    let active = ActiveMachine::Galvo(rig.conn);
    assert!(m.pause().unwrap_err().contains("no tiene pausa"));
    assert!(m.resume().unwrap_err().contains("no tiene pausa"));
    assert!(m.realtime(RealtimeCommand::Hold).is_err());
    assert!(m.realtime(RealtimeCommand::Resume).is_err());
    assert!(m.realtime(RealtimeCommand::Status).is_ok());
    assert!(m.grbl().err().unwrap().contains("solo de GRBL"));
    assert!(m.ruida().is_err() && m.lhy().is_err());
    assert!(m.galvo().is_ok());
    assert!(!m.is_closed());
    // Mover y detener pasan por el despacho común.
    m.jog(1.0, 1.0, 0.0).unwrap();
    assert_eq!(m.work_pos(), Some([56.0, 56.0]));
    m.jog_to(20.0, 30.0, 0.0).unwrap();
    assert!(m.status_report().is_some());
    m.cancel().unwrap();
    m.realtime(RealtimeCommand::Reset).unwrap();
    // Cerrar por el despacho apaga todo.
    let _ = device;
    active.close();
    assert!(m.is_closed());
    assert!(lock(&emu).laser_safe());
    assert!(sink.connections().iter().any(|c| !c.connected));
}

#[test]
fn a_closed_connection_refuses_every_operation() {
    let rig = rig();
    let h = rig.handle.clone();
    let doc = square_doc();
    let device = rig.device.clone();
    rig.conn.close();
    assert!(h.is_closed());
    for err in [
        h.start_job(&doc, &device, &JobOptions::default(), false)
            .unwrap_err(),
        h.cancel().unwrap_err(),
        h.confirm_move().unwrap_err(),
        h.jog_to(1.0, 1.0, 0.0).unwrap_err(),
    ] {
        assert!(
            err.contains("No hay máquina conectada") || err.contains("Se perdió"),
            "{err}"
        );
    }
}

#[test]
fn the_json_shapes_match_the_typescript_contract() {
    let info = StepInfo {
        step: 2,
        steps: 3,
        repeat: 1,
        repeats: 2,
        piece: 2,
        pieces: 3,
        dx: -100.0,
        dy: 0.0,
        rotary_deg: 0.0,
        pause_ms: 250.0,
    };
    let v = serde_json::to_value(GalvoEvent::WaitMove { info: info.clone() }).unwrap();
    assert_eq!(v["kind"], "wait_move");
    assert_eq!(v["info"]["dx"], -100.0);
    assert_eq!(v["info"]["pause_ms"], 250.0);
    let v = serde_json::to_value(GalvoEvent::LaserMayBeOn {
        message: "x".into(),
    })
    .unwrap();
    assert_eq!(v["kind"], "laser_may_be_on");
    assert_eq!(
        serde_json::to_value(GalvoEvent::Light {
            loops: 3,
            adjust_gen: 0,
            adjust_rejected: false
        })
        .unwrap()["loops"],
        3
    );
    assert_eq!(
        serde_json::to_value(GalvoEvent::Step { info }).unwrap()["kind"],
        "step"
    );
    // El reconocimiento llega de la interfaz con estos nombres.
    let ack: SafetyAck =
        serde_json::from_str(r#"{"eyewear":true,"enclosure":true,"emergency_stop":true}"#).unwrap();
    assert!(ack.token().is_ok());
    let rig = rig();
    let st = serde_json::to_value(rig.handle.state()).unwrap();
    for key in [
        "board",
        "vendor_id",
        "product_id",
        "virtual_machine",
        "serial",
        "version",
        "status_raw",
        "busy",
        "ready",
        "axis",
        "inputs",
        "inputs_known",
        "working",
        "red_light_on",
        "laser_may_be_on",
        "position",
        "position_known",
        "field_mm",
        "source",
        "waiting_move",
    ] {
        assert!(st.get(key).is_some(), "falta {key} en GalvoState");
    }
    assert_eq!(st["source"], "fiber_q");
    assert!(st["waiting_move"].is_null());
    rig.conn.close();
}

mod frame6c;
mod wave6e;
