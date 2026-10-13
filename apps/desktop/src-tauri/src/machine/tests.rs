//! Pruebas de la conexión con una GRBL falsa sobre `memory_pair` (sin hardware).

use super::*;
use std::sync::atomic::AtomicUsize;
use tlaku_grbl::serial::{MemoryTransport, memory_pair};

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

    fn logs(&self) -> Vec<(LogDir, String)> {
        self.all()
            .into_iter()
            .filter_map(|e| match e {
                Emit::Log(l) => Some((l.dir, l.text)),
                _ => None,
            })
            .collect()
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
        let end = Instant::now() + Duration::from_secs(5);
        while !f(self) {
            assert!(Instant::now() < end, "se agotó la espera de: {what}");
            thread::sleep(Duration::from_millis(5));
        }
    }
}

/// Lo que la máquina falsa guarda de lo que recibe.
#[derive(Default)]
struct Seen {
    /// Líneas completas, sin el `\n`.
    lines: Vec<String>,
    /// Bytes de tiempo real, en orden.
    realtime: Vec<u8>,
}

struct Fake {
    state: Arc<Mutex<&'static str>>,
    seen: Arc<Mutex<Seen>>,
    /// Mientras sea `true` no contesta `ok` a las líneas de G-code (el trabajo no avanza).
    hold_ok: Arc<AtomicBool>,
    /// Cuántos `?` ha contestado.
    polls: Arc<AtomicUsize>,
    /// Si es `true`, ignora todo (máquina muda).
    mute: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Fake {
    fn start(mut dev: MemoryTransport) -> Self {
        let mut f = Fake {
            state: Arc::new(Mutex::new("Idle")),
            seen: Arc::default(),
            hold_ok: Arc::default(),
            polls: Arc::default(),
            mute: Arc::default(),
            stop: Arc::default(),
            thread: None,
        };
        let (state, seen, hold, polls, mute, stop) = (
            f.state.clone(),
            f.seen.clone(),
            f.hold_ok.clone(),
            f.polls.clone(),
            f.mute.clone(),
            f.stop.clone(),
        );
        let thread = thread::spawn(move || {
            let mut partial = String::new();
            let mut queued: VecDeque<String> = VecDeque::new();
            let mut buf = [0u8; 256];
            while !stop.load(Ordering::SeqCst) {
                // Las líneas retenidas se liberan en cuanto se suelta `hold_ok`.
                if !hold.load(Ordering::SeqCst) {
                    while queued.pop_front().is_some() {
                        let _ = dev.write_all(b"ok\r\n");
                    }
                }
                let Ok(n) = dev.read_timeout(&mut buf, Duration::from_millis(5)) else {
                    return;
                };
                if mute.load(Ordering::SeqCst) {
                    continue;
                }
                for &b in &buf[..n] {
                    match b {
                        b'?' => {
                            polls.fetch_add(1, Ordering::SeqCst);
                            let st = *state.lock().unwrap();
                            let _ = dev.write_all(
                                format!("<{st}|MPos:11.000,22.000,0.000|FS:0,0|WCO:1.000,2.000,0.000>\r\n")
                                    .as_bytes(),
                            );
                        }
                        0x18 => {
                            seen.lock().unwrap().realtime.push(b);
                            queued.clear();
                            *state.lock().unwrap() = "Idle";
                            let _ = dev.write_all(b"\r\nGrbl 1.1h ['$' for help]\r\n");
                        }
                        b'!' | b'~' | 0x85 => seen.lock().unwrap().realtime.push(b),
                        b'\n' => {
                            let line = std::mem::take(&mut partial);
                            seen.lock().unwrap().lines.push(line.clone());
                            let reply = match line.as_str() {
                                "$I" => "[VER:1.1h.20190825:]\r\n[OPT:V,15,128]\r\nok\r\n",
                                "$$" => "$22=0\r\nok\r\n",
                                l if l.starts_with("$J=") && l.contains("X999") => "error:15\r\n",
                                "BAD" => "error:20\r\n",
                                _ if hold.load(Ordering::SeqCst) && !line.starts_with('$') => {
                                    queued.push_back(line);
                                    continue;
                                }
                                _ => "ok\r\n",
                            };
                            let _ = dev.write_all(reply.as_bytes());
                        }
                        b'\r' => {}
                        _ => partial.push(b as char),
                    }
                }
            }
        });
        f.thread = Some(thread);
        f
    }

    fn set_state(&self, s: &'static str) {
        *self.state.lock().unwrap() = s;
    }

    fn lines(&self) -> Vec<String> {
        self.seen.lock().unwrap().lines.clone()
    }

    fn realtime(&self) -> Vec<u8> {
        self.seen.lock().unwrap().realtime.clone()
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn fast() -> Timing {
    Timing {
        status_interval: Duration::from_millis(20),
        silence_timeout: Duration::from_millis(400),
        command_timeout: Duration::from_millis(800),
        home_timeout: Duration::from_secs(2),
        identify_timeout: Duration::from_secs(2),
        tick: Duration::from_millis(5),
        progress_interval: Duration::from_millis(1),
        preflight: PreflightConfig {
            status_timeout: Duration::from_secs(2),
            settings_timeout: Duration::from_millis(300),
            port: "fake".into(),
        },
    }
}

struct Rig {
    conn: Option<Connection>,
    handle: Handle,
    sink: Arc<Collect>,
    fake: Fake,
}

fn connect_with(timing: Timing) -> Rig {
    let (host, dev) = memory_pair();
    let fake = Fake::start(dev);
    let sink = Arc::new(Collect::default());
    let (conn, info) =
        Connection::start(Box::new(host), "/dev/cu.fake", sink.clone(), timing).expect("conectar");
    assert_eq!(info.firmware, "Grbl 1.1h");
    let handle = conn.handle();
    Rig {
        conn: Some(conn),
        handle,
        sink,
        fake,
    }
}

fn connect() -> Rig {
    connect_with(fast())
}

fn job_lines() -> Vec<String> {
    ["G90", "G21", "M5", "G0 X1 Y1", "G1 X5 Y5 S100 F1000", "M5"]
        .map(String::from)
        .to_vec()
}

// ---------------------------------------------------------------------------
// Conexión y estado
// ---------------------------------------------------------------------------

#[test]
fn firmware_name_is_extracted_from_ver() {
    let v = |s: &str| firmware_name(&[s.to_string()]);
    assert_eq!(v("[VER:1.1h.20190825:]"), "Grbl 1.1h");
    assert_eq!(v("[VER:1.1f:Falcon]"), "Grbl 1.1f");
    assert_eq!(firmware_name(&["[OPT:V,15,128]".into()]), "GRBL");
}

#[test]
fn connect_emits_connection_and_status_without_logging_queries() {
    let rig = connect();
    rig.sink.wait("estado", |s| s.statuses().len() >= 3);
    let c = rig.sink.connections();
    assert_eq!(c.len(), 1);
    assert!(c[0].connected && c[0].port.as_deref() == Some("/dev/cu.fake") && c[0].error.is_none());
    let s = rig.sink.statuses().pop().unwrap();
    assert_eq!(s.state, MachineState::Idle);
    assert_eq!(s.machine_pos, [11.0, 22.0]);
    // Posición de trabajo = máquina - WCO.
    assert_eq!(s.work_pos, [10.0, 20.0]);
    assert_eq!(rig.handle.work_pos(), Some([10.0, 20.0]));
    // El registro tiene `$I` y su respuesta, pero ningún reporte de estado.
    let logs = rig.sink.logs();
    assert!(logs.contains(&(LogDir::Out, "$I".into())));
    assert!(logs.contains(&(LogDir::In, "[VER:1.1h.20190825:]".into())));
    assert!(
        logs.iter().all(|(_, t)| !t.starts_with('<') && t != "?"),
        "{logs:?}"
    );
}

#[test]
fn a_machine_that_does_not_answer_identification_fails_to_connect() {
    let (host, _silent) = memory_pair();
    let timing = Timing {
        identify_timeout: Duration::from_millis(100),
        ..fast()
    };
    let r = Connection::start(Box::new(host), "p", Arc::new(Collect::default()), timing);
    let e = r.err().expect("debe fallar");
    assert!(e.contains("no respondió") && e.contains("GRBL 1.1"), "{e}");
}

#[test]
fn commands_return_the_reply_up_to_ok_and_are_logged() {
    let rig = connect();
    let r = rig.handle.command("$$").unwrap();
    assert_eq!(r, ["$22=0", "ok"]);
    let r = rig.handle.command("BAD").unwrap();
    assert_eq!(r, ["error:20: comando G-code no soportado o no válido"]);
    let logs = rig.sink.logs();
    assert!(logs.contains(&(LogDir::Out, "$$".into())));
    assert!(logs.contains(&(LogDir::In, "$22=0".into())));
    // Los caracteres de tiempo real no pasan como texto.
    let e = rig.handle.command("G1 X1 !").unwrap_err();
    assert!(e.contains("tiempo real"), "{e}");
    assert!(rig.handle.command("   ").unwrap_err().contains("vacía"));
}

#[test]
fn jog_sends_a_relative_jog_line_and_explains_rejections() {
    let rig = connect();
    rig.handle.jog(10.0, -5.5, 1000.0).unwrap();
    assert!(
        rig.fake
            .lines()
            .contains(&"$J=G91 G21 X10 Y-5.5 F1000".to_string()),
        "{:?}",
        rig.fake.lines()
    );
    let e = rig.handle.jog(999.0, 0.0, 1000.0).unwrap_err();
    assert!(e.contains("error:15") || e.contains("recorrido"), "{e}");
    assert!(
        rig.handle
            .jog(1.0, 1.0, 0.0)
            .unwrap_err()
            .contains("mayor que 0")
    );
    assert!(rig.handle.jog(f64::NAN, 1.0, 100.0).is_err());
}

#[test]
fn realtime_commands_send_single_bytes_and_cancel_idle_is_a_jog_cancel() {
    let rig = connect();
    rig.handle.realtime(RealtimeCommand::Hold).unwrap();
    rig.handle.realtime(RealtimeCommand::Resume).unwrap();
    rig.handle.cancel().unwrap();
    rig.handle.realtime(RealtimeCommand::Reset).unwrap();
    rig.sink.wait("saludo tras el reinicio", |s| {
        s.logs().iter().any(|(_, t)| t.starts_with("Grbl 1.1h"))
    });
    assert_eq!(rig.fake.realtime(), [b'!', b'~', 0x85, 0x18]);
}

// ---------------------------------------------------------------------------
// Trabajos
// ---------------------------------------------------------------------------

#[test]
fn start_job_flows_from_connection_to_finished_with_progress() {
    let rig = connect();
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.sink.wait("trabajo terminado", |s| {
        s.jobs().contains(&JobState::Finished)
    });
    assert_eq!(rig.sink.jobs(), [JobState::Started, JobState::Finished]);
    let progress = rig.sink.progress();
    assert_eq!(progress.first().unwrap().acked, 0);
    let last = progress.last().unwrap();
    assert_eq!((last.acked, last.total), (6, 6));
    assert!(progress.windows(2).all(|w| w[0].acked <= w[1].acked));
    // La máquina recibió todo el G-code, sin espacios.
    let lines = rig.fake.lines();
    for l in ["G90", "G21", "M5", "G0X1Y1", "G1X5Y5S100F1000"] {
        assert!(lines.contains(&l.to_string()), "falta {l} en {lines:?}");
    }
    // Y la conexión sigue usable después.
    assert_eq!(rig.handle.command("$$").unwrap().last().unwrap(), "ok");
    rig.sink
        .wait("sondeo tras el trabajo", |s| s.statuses().len() > 5);
}

#[test]
fn start_job_refuses_alarm_hold_door_and_run_without_sending_gcode() {
    for (state, needle) in [
        ("Alarm", "ALARMA"),
        ("Hold:0", "pausa"),
        ("Door:1", "puerta"),
        ("Run", "trabajo en curso"),
    ] {
        let rig = connect();
        rig.fake.set_state(state);
        let e = rig.handle.start_job(job_lines(), 127).unwrap_err();
        assert!(e.contains(needle), "{state}: {e}");
        assert!(e.contains("No se transmitió nada"), "{state}: {e}");
        assert!(rig.sink.jobs().is_empty(), "{state}");
        // Ni una línea de G-code: solo la identificación y las consultas.
        assert!(
            rig.fake.lines().iter().all(|l| l == "$I" || l == "$$"),
            "{state}: {:?}",
            rig.fake.lines()
        );
        // La conexión sigue viva y el sondeo continúa.
        let n = rig.sink.statuses().len();
        rig.sink.wait("sondeo", |s| s.statuses().len() > n + 2);
    }
}

#[test]
fn alarm_message_points_to_the_console_instead_of_the_cli() {
    let rig = connect();
    rig.fake.set_state("Alarm");
    let e = rig.handle.start_job(job_lines(), 127).unwrap_err();
    assert!(e.contains("consola de la pestaña Máquina"), "{e}");
    assert!(!e.contains("tlaku"), "{e}");
}

#[test]
fn invalid_gcode_is_rejected_before_touching_the_machine() {
    let rig = connect();
    let e = rig
        .handle
        .start_job(vec!["G1 X1 ?".into()], 127)
        .unwrap_err();
    assert!(e.contains("tiempo real"), "{e}");
    let e = rig.handle.start_job(vec![], 127).unwrap_err();
    assert!(e.contains("ninguna línea"), "{e}");
    assert!(rig.fake.lines().iter().all(|l| l == "$I"));
}

#[test]
fn pause_resume_and_cancel_drive_the_running_job() {
    let rig = connect();
    rig.fake.hold_ok.store(true, Ordering::SeqCst);
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.sink.wait("inicio", |s| s.jobs() == [JobState::Started]);
    // Mientras corre, no se aceptan comandos sueltos ni otro trabajo.
    assert!(
        rig.handle
            .command("$$")
            .unwrap_err()
            .contains("trabajo en curso")
    );
    assert!(rig.handle.jog(1.0, 0.0, 100.0).is_err());
    assert!(rig.handle.start_job(job_lines(), 127).is_err());

    rig.handle.pause().unwrap();
    rig.handle.resume().unwrap();
    rig.handle.cancel().unwrap();
    rig.sink
        .wait("cancelado", |s| s.jobs().contains(&JobState::Cancelled));
    assert_eq!(
        rig.sink.jobs(),
        [
            JobState::Started,
            JobState::Paused,
            JobState::Started,
            JobState::Cancelled
        ]
    );
    let rt = rig.fake.realtime();
    assert!(rt.starts_with(b"!~"), "{rt:?}");
    assert!(rt.contains(&b'!') && rt.contains(&0x18), "{rt:?}");
    // Después de cancelar la conexión sigue funcionando.
    rig.fake.hold_ok.store(false, Ordering::SeqCst);
    assert_eq!(rig.handle.command("$$").unwrap().last().unwrap(), "ok");
}

#[test]
fn reset_during_a_job_cancels_it() {
    let rig = connect();
    rig.fake.hold_ok.store(true, Ordering::SeqCst);
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.handle.realtime(RealtimeCommand::Reset).unwrap();
    rig.sink
        .wait("cancelado", |s| s.jobs().contains(&JobState::Cancelled));
}

#[test]
fn frame_runs_without_job_events_or_progress() {
    let rig = connect();
    let lines = tlaku_grbl::frame::frame_gcode(
        tlaku_core::Bounds {
            min: tlaku_core::Point::new(0.0, 0.0),
            max: tlaku_core::Point::new(10.0, 5.0),
        },
        1500.0,
    );
    rig.handle.frame(lines, 127).unwrap();
    rig.sink.wait("encuadre", |s| {
        s.logs().iter().any(|(_, t)| t == "; encuadre terminado")
    });
    assert!(rig.sink.jobs().is_empty());
    assert!(rig.sink.progress().is_empty());
    assert!(rig.fake.lines().contains(&"G1X10Y5S0F1500".to_string()));
    // Con la máquina en alarma tampoco se encuadra.
    rig.fake.set_state("Alarm");
    assert!(rig.handle.frame(vec!["G0 X1".into()], 127).is_err());
}

// ---------------------------------------------------------------------------
// Desconexión
// ---------------------------------------------------------------------------

#[test]
fn closing_disconnects_without_error() {
    let mut rig = connect();
    rig.conn.take().unwrap().close();
    let c = rig.sink.connections();
    assert_eq!(c.len(), 2);
    assert!(!c[1].connected && c[1].error.is_none());
    assert!(rig.handle.is_closed());
    assert_eq!(rig.handle.command("$$").unwrap_err(), NOT_CONNECTED);
}

#[test]
fn closing_during_a_job_cancels_it_first() {
    let mut rig = connect();
    rig.fake.hold_ok.store(true, Ordering::SeqCst);
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.conn.take().unwrap().close();
    assert_eq!(rig.sink.jobs(), [JobState::Started, JobState::Cancelled]);
    assert!(rig.fake.realtime().contains(&0x18));
    assert!(!rig.sink.connections().last().unwrap().connected);
}

#[test]
fn a_dropped_cable_is_reported_as_a_lost_connection() {
    let rig = connect();
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    // Se corta el cable: el otro extremo desaparece.
    let Rig {
        handle, sink, fake, ..
    } = rig;
    drop(fake);
    sink.wait("conexión perdida", |s| {
        s.connections().iter().any(|c| !c.connected)
    });
    let c = sink.connections().pop().unwrap();
    assert!(
        c.error.as_deref().is_some_and(|e| e.contains("conexión")),
        "{c:?}"
    );
    assert!(handle.is_closed());
    assert_eq!(
        handle.start_job(job_lines(), 127).unwrap_err(),
        NOT_CONNECTED
    );
}

#[test]
fn a_silent_machine_is_reported_as_lost() {
    let rig = connect();
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.fake.mute.store(true, Ordering::SeqCst);
    rig.sink
        .wait("silencio", |s| s.connections().iter().any(|c| !c.connected));
    let c = rig.sink.connections().pop().unwrap();
    assert!(
        c.error
            .as_deref()
            .is_some_and(|e| e.contains("dejó de responder")),
        "{c:?}"
    );
}

#[test]
fn status_uses_wpos_when_the_controller_reports_work_coordinates() {
    let (host, mut dev) = memory_pair();
    let sink = Arc::new(Collect::default());
    dev.write_all(b"[VER:1.1h.20190825:]\r\nok\r\n").unwrap();
    let (_conn, _) = Connection::start(Box::new(host), "p", sink.clone(), fast()).unwrap();
    dev.write_all(b"<Run|WPos:3.000,4.000,0.000|Bf:12,100|FS:1500,255|WCO:1.000,1.000,0.000>\r\n")
        .unwrap();
    sink.wait("estado Run", |s| {
        s.statuses().iter().any(|x| x.state == MachineState::Run)
    });
    let s = sink
        .statuses()
        .into_iter()
        .find(|x| x.state == MachineState::Run)
        .unwrap();
    assert_eq!(s.work_pos, [3.0, 4.0]);
    assert_eq!(s.machine_pos, [4.0, 5.0]);
    assert_eq!((s.feed, s.spindle), (1500.0, 255.0));
    assert_eq!(
        s.buffer,
        BufferStatus {
            planner: 12,
            rx: 100
        }
    );
}
