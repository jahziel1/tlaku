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

    fn job_messages(&self) -> Vec<String> {
        self.all()
            .into_iter()
            .filter_map(|e| match e {
                Emit::Job(j) => Some(j.message),
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
    /// Respuesta a `$$` (sin el `ok` final).
    settings: Arc<Mutex<String>>,
    /// Respuesta a `$#` (sin el `ok` final).
    params: Arc<Mutex<String>>,
    /// Bytes que la máquina escribe por su cuenta en cuanto puede.
    inject: Arc<Mutex<Vec<u8>>>,
    /// Tras `$H` calla (ni `?` ni nada) estos milisegundos y luego contesta `ok`.
    home_delay_ms: Arc<AtomicUsize>,
    /// Bloqueo crítico: ignora todo hasta recibir un soft reset (0x18).
    locked: Arc<AtomicBool>,
    /// La n-ésima línea de G-code (no `$`) recibe esta respuesta en lugar de `ok`.
    fault_at: Arc<Mutex<Option<(usize, String)>>>,
    /// Tras `!` informa `Hold:1` tres consultas y luego `Hold:0`.
    brake: Arc<AtomicBool>,
    /// Con `brake`: nunca llega a `Hold:0` (se queda frenando).
    never_settle: Arc<AtomicBool>,
    /// Si el último soft reset llegó cuando aún frenaba (o corría).
    reset_in_motion: Arc<AtomicBool>,
    /// Al recibir G-code pasa a `Run` (como una máquina que empieza a moverse).
    run_on_gcode: Arc<AtomicBool>,
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
            settings: Arc::new(Mutex::new("$22=0\r\n$30=1000\r\n$32=1\r\n".into())),
            params: Arc::default(),
            inject: Arc::default(),
            home_delay_ms: Arc::default(),
            locked: Arc::default(),
            fault_at: Arc::default(),
            brake: Arc::default(),
            never_settle: Arc::default(),
            reset_in_motion: Arc::default(),
            run_on_gcode: Arc::default(),
        };
        let (state, seen, hold, polls, mute, stop) = (
            f.state.clone(),
            f.seen.clone(),
            f.hold_ok.clone(),
            f.polls.clone(),
            f.mute.clone(),
            f.stop.clone(),
        );
        let (settings, params, inject, home_delay, locked) = (
            f.settings.clone(),
            f.params.clone(),
            f.inject.clone(),
            f.home_delay_ms.clone(),
            f.locked.clone(),
        );
        let (fault_at, brake, never_settle, reset_in_motion, run_on_gcode) = (
            f.fault_at.clone(),
            f.brake.clone(),
            f.never_settle.clone(),
            f.reset_in_motion.clone(),
            f.run_on_gcode.clone(),
        );
        let thread = thread::spawn(move || {
            let mut partial = String::new();
            let mut queued: VecDeque<String> = VecDeque::new();
            let mut buf = [0u8; 256];
            let mut homing_until: Option<Instant> = None;
            let mut gcode_lines = 0usize;
            let mut braking: Option<usize> = None;
            while !stop.load(Ordering::SeqCst) {
                // Las líneas retenidas se liberan en cuanto se suelta `hold_ok`.
                if !hold.load(Ordering::SeqCst) {
                    while queued.pop_front().is_some() {
                        let _ = dev.write_all(b"ok\r\n");
                    }
                }
                {
                    let mut raw = inject.lock().unwrap();
                    if !raw.is_empty() {
                        let _ = dev.write_all(&raw);
                        raw.clear();
                    }
                }
                if homing_until.is_some_and(|t| Instant::now() >= t) {
                    homing_until = None;
                    let _ = dev.write_all(b"ok\r\n");
                }
                let Ok(n) = dev.read_timeout(&mut buf, Duration::from_millis(5)) else {
                    return;
                };
                if mute.load(Ordering::SeqCst) {
                    continue;
                }
                for &b in &buf[..n] {
                    if locked.load(Ordering::SeqCst) && b != 0x18 {
                        continue;
                    }
                    match b {
                        b'?' if homing_until.is_some() => {}
                        b'?' => {
                            polls.fetch_add(1, Ordering::SeqCst);
                            let st = match braking.as_mut() {
                                Some(k) => {
                                    *k += 1;
                                    if *k <= 3 || never_settle.load(Ordering::SeqCst) {
                                        "Hold:1"
                                    } else {
                                        "Hold:0"
                                    }
                                }
                                None => *state.lock().unwrap(),
                            };
                            let _ = dev.write_all(
                                format!("<{st}|MPos:11.000,22.000,0.000|FS:0,0|WCO:1.000,2.000,0.000>\r\n")
                                    .as_bytes(),
                            );
                        }
                        0x18 => {
                            seen.lock().unwrap().realtime.push(b);
                            reset_in_motion.store(
                                braking
                                    .is_none_or(|k| k <= 3 || never_settle.load(Ordering::SeqCst)),
                                Ordering::SeqCst,
                            );
                            braking = None;
                            locked.store(false, Ordering::SeqCst);
                            queued.clear();
                            *state.lock().unwrap() = "Idle";
                            let _ = dev.write_all(b"\r\nGrbl 1.1h ['$' for help]\r\n");
                        }
                        b'!' => {
                            seen.lock().unwrap().realtime.push(b);
                            if brake.load(Ordering::SeqCst) {
                                braking = Some(0);
                            }
                        }
                        b'~' | 0x85 | 0x90..=0x9E => seen.lock().unwrap().realtime.push(b),
                        b'\n' => {
                            let line = std::mem::take(&mut partial);
                            seen.lock().unwrap().lines.push(line.clone());
                            let gcode = !line.starts_with('$');
                            if gcode {
                                gcode_lines += 1;
                                if run_on_gcode.load(Ordering::SeqCst) {
                                    *state.lock().unwrap() = "Run";
                                }
                            }
                            let fault = fault_at
                                .lock()
                                .unwrap()
                                .as_ref()
                                .filter(|(n, _)| gcode && *n == gcode_lines)
                                .map(|(_, r)| format!("{r}\r\n"));
                            let reply = match line.as_str() {
                                _ if fault.is_some() => fault.unwrap_or_default(),
                                "$I" => {
                                    "[VER:1.1h.20190825:]\r\n[OPT:V,15,128]\r\nok\r\n".to_string()
                                }
                                "$$" => format!("{}ok\r\n", settings.lock().unwrap()),
                                "$#" => format!("{}ok\r\n", params.lock().unwrap()),
                                "$H" if home_delay.load(Ordering::SeqCst) > 0 => {
                                    homing_until = Some(
                                        Instant::now()
                                            + Duration::from_millis(
                                                home_delay.load(Ordering::SeqCst) as u64,
                                            ),
                                    );
                                    continue;
                                }
                                l if l.starts_with("$J=") && l.contains("X999") => {
                                    "error:15\r\n".to_string()
                                }
                                "BAD" => "error:20\r\n".to_string(),
                                _ if hold.load(Ordering::SeqCst) && !line.starts_with('$') => {
                                    queued.push_back(line);
                                    continue;
                                }
                                _ => "ok\r\n".to_string(),
                            };
                            // Un límite duro bloquea la placa hasta el soft reset.
                            if reply.starts_with("ALARM:1") {
                                locked.store(true, Ordering::SeqCst);
                            }
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

    fn say(&self, text: &str) {
        self.inject
            .lock()
            .unwrap()
            .extend_from_slice(text.as_bytes());
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
        hold_timeout: Duration::from_millis(400),
        reset_wait: Duration::from_millis(300),
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
    assert_eq!(r, ["$22=0", "$30=1000", "$32=1", "ok"]);
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

// ---------------------------------------------------------------------------
// Firmware, banners reales y reportes raros (grbl.md / falcon2.md)
// ---------------------------------------------------------------------------

#[test]
fn firmware_name_and_family_cover_grbl_fluidnc_grblhal_and_the_falcon2() {
    let id = |lines: &[&str]| identify(&lines.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    assert_eq!(
        id(&["[VER:1.1h.20190825:]", "[OPT:V,15,128]"]),
        ("Grbl 1.1h".into(), Firmware::Grbl)
    );
    // Falcon2 Pro: líneas propias de Creality alrededor de `[VER:]`.
    assert_eq!(
        id(&[
            "[VER:1.1f.20220810:]",
            "[OPT:VHL,127,65536]",
            "[software versions: CV50-Pro-MASTER-Release V1.0.0 20231129-1024]",
            "inquire_info->power:22W ,CV50-LASER-22W-Release V3.0.5 20231129-1001",
        ]),
        ("Grbl 1.1f".into(), Firmware::Grbl)
    );
    assert_eq!(
        id(&["[VER:3.9 FluidNC v3.9.7:]", "[OPT:MPHS]"]),
        ("FluidNC v3.9.7".into(), Firmware::FluidNc)
    );
    assert_eq!(
        id(&[
            "[VER:1.1f.20240101:]",
            "[OPT:VNM,35,1024,0]",
            "[NEWOPT:ENUMS,RT+]"
        ]),
        ("grblHAL 1.1f".into(), Firmware::GrblHal)
    );
    // `$I` que solo devuelve `ok`: nombre genérico.
    assert_eq!(id(&[]), ("GRBL".into(), Firmware::Unknown));
    assert_eq!(id(&["[OPT:V,15,128]"]).0, "GRBL");
}

#[test]
fn connecting_through_the_whole_falcon2_banner_and_an_info_reply_of_only_ok() {
    // Banner real de la Falcon2 Pro (falcon2.md, sección 3), llegado antes de la respuesta de `$I`.
    let banner = "usb connect succeed\r\n\
        [OK]system start-up CV50-Pro-MASTER-Release V1.0.1 20240325-1042\r\n\
        [OK] ILmp_app_init OK\r\n[OK] gas_init OK\r\n[MSG:Using cpu_map...esp32-s3]\r\n\
        Grbl 1.1f ['$' for help]\r\n[OK] ILmp Link connect ok\r\n";
    let info = "[VER:1.1f.20220810:]\r\n[OPT:VHL,127,65536]\r\n\
        [software versions: CV50-Pro-MASTER-Release V1.0.0 20231129-1024]\r\n\
        inquire_info->power:22W ,CV50-LASER-22W-Release V3.0.5 20231129-1001\r\nok\r\n";
    for (reply, firmware) in [(info, "Grbl 1.1f"), ("ok\r\n", "GRBL")] {
        let (host, mut dev) = memory_pair();
        let sink = Arc::new(Collect::default());
        dev.write_all(banner.as_bytes()).unwrap();
        dev.write_all(reply.as_bytes()).unwrap();
        let (conn, result) = Connection::start(Box::new(host), "p", sink.clone(), fast()).unwrap();
        assert_eq!(result.firmware, firmware);
        // Ninguna de las líneas de Creality se tomó por el final del comando.
        assert!(
            sink.logs()
                .iter()
                .any(|(_, t)| t.contains("ILmp Link connect ok") || t == "ok")
        );
        conn.close();
    }
}

#[test]
fn a_status_report_with_nan_never_becomes_a_position() {
    let (host, mut dev) = memory_pair();
    let sink = Arc::new(Collect::default());
    dev.write_all(b"[VER:1.1f.20220810:]\r\nok\r\n").unwrap();
    let (_conn, _) = Connection::start(Box::new(host), "p", sink.clone(), fast()).unwrap();
    // WCO con nan (G54 dañado en la Falcon2) y luego una MPos con nan.
    dev.write_all(b"<Run|MPos:5.000,6.000,0.000|FS:100,5|WCO:-nan,1.579,1.619>\r\n")
        .unwrap();
    dev.write_all(b"<Run|MPos:nan,6.000,0.000|FS:100,5>\r\n")
        .unwrap();
    sink.wait("estado", |s| {
        s.statuses().iter().any(|x| x.state == MachineState::Run)
    });
    for s in sink.statuses() {
        assert!(
            s.machine_pos
                .iter()
                .chain(&s.work_pos)
                .all(|v| v.is_finite()),
            "{s:?}"
        );
    }
    let s = sink
        .statuses()
        .into_iter()
        .find(|x| x.state == MachineState::Run)
        .unwrap();
    // Sin WCO válido, la posición de trabajo es la de máquina.
    assert_eq!((s.machine_pos, s.work_pos), ([5.0, 6.0], [5.0, 6.0]));
}

#[test]
fn an_unknown_grblhal_state_is_shown_as_busy_and_blocks_a_new_job() {
    let rig = connect();
    rig.fake.set_state("Tool");
    rig.sink.wait("estado ocupado", |s| {
        s.statuses()
            .last()
            .is_some_and(|x| x.state == MachineState::Other)
    });
    // La interfaz recibe el nombre original del estado para mostrarlo.
    let last = rig.sink.statuses().pop().unwrap();
    assert_eq!(last.other_state.as_deref(), Some("Tool"));
    let e = rig.handle.start_job(job_lines(), 127).unwrap_err();
    assert!(
        e.contains("no reconoce") && e.contains("No se transmitió nada"),
        "{e}"
    );
}

#[test]
fn extended_codes_are_read_with_the_firmware_family() {
    // grblHAL: `[NEWOPT:]` en `$I`; ALARM:10 es un paro de emergencia.
    let (host, mut dev) = memory_pair();
    let sink = Arc::new(Collect::default());
    dev.write_all(b"[VER:1.1f.20240101:]\r\n[NEWOPT:ENUMS,RT+]\r\nok\r\n")
        .unwrap();
    let (_conn, r) = Connection::start(Box::new(host), "p", sink.clone(), fast()).unwrap();
    assert_eq!(r.firmware, "grblHAL 1.1f");
    dev.write_all(b"ALARM:10\r\nerror:50\r\nERROR:06.\r\n")
        .unwrap();
    sink.wait("registro", |s| {
        s.logs().iter().any(|(_, t)| t.starts_with("ERROR:06"))
    });
    let logs = sink.logs();
    let has = |needle: &str| logs.iter().any(|(_, t)| t.contains(needle));
    assert!(has("ALARM:10: parada de emergencia"), "{logs:?}");
    assert!(has("error:50: parada de emergencia activa"), "{logs:?}");
    // `ERROR:06.` de Creality se explica como tal y se normaliza.
    assert!(has("ERROR:06: lente contaminada"), "{logs:?}");

    // GRBL 1.1 identificado: el mismo ALARM:10 es el autoescuadre.
    let rig = connect();
    rig.fake.say("ALARM:10\r\n");
    rig.sink.wait("alarma", |s| {
        s.logs().iter().any(|(_, t)| t.starts_with("ALARM:10"))
    });
    assert!(
        rig.sink
            .logs()
            .iter()
            .any(|(_, t)| t.starts_with("ALARM:10") && t.contains("autoescuadre"))
    );

    // Sin saber el firmware (`$I` solo devuelve `ok`), el texto dice que depende de él.
    let (host, mut dev) = memory_pair();
    let sink = Arc::new(Collect::default());
    dev.write_all(b"ok\r\n").unwrap();
    let (_conn, r) = Connection::start(Box::new(host), "p", sink.clone(), fast()).unwrap();
    assert_eq!(r.firmware, "GRBL");
    dev.write_all(b"ALARM:10\r\n").unwrap();
    sink.wait("alarma", |s| {
        s.logs().iter().any(|(_, t)| t.starts_with("ALARM:10"))
    });
    assert!(
        sink.logs()
            .iter()
            .any(|(_, t)| t.starts_with("ALARM:10") && t.contains("depende del firmware"))
    );
}

// ---------------------------------------------------------------------------
// Preflight: modo láser y escala de potencia
// ---------------------------------------------------------------------------

#[test]
fn laser_mode_off_blocks_the_job_but_not_the_frame() {
    let rig = connect();
    *rig.fake.settings.lock().unwrap() = "$22=1\r\n$30=1000\r\n$32=0\r\n".into();
    let e = rig.handle.start_job(job_lines(), 127).unwrap_err();
    assert!(e.contains("$32=0") && e.contains("$32=1"), "{e}");
    assert!(
        e.contains("G0") && e.contains("No se transmitió nada"),
        "{e}"
    );
    assert!(e.contains("consola de la pestaña Máquina"), "{e}");
    assert!(rig.sink.jobs().is_empty());
    assert!(
        rig.fake.lines().iter().all(|l| l.starts_with('$')),
        "{:?}",
        rig.fake.lines()
    );
    // El encuadre va con el láser apagado: no necesita el modo láser.
    rig.handle
        .frame(vec!["M5".into(), "G0 X1 Y1".into()], 127)
        .unwrap();
    rig.sink.wait("encuadre", |s| {
        s.logs().iter().any(|(_, t)| t == "; encuadre terminado")
    });
    // Y la conexión sigue viva para corregirlo desde la consola.
    assert!(!rig.handle.is_closed());
}

#[test]
fn an_s_max_that_differs_from_the_controller_only_warns_in_the_log() {
    let rig = connect();
    // GRBL-LPC: `$30=1`.
    *rig.fake.settings.lock().unwrap() = "$22=0\r\n$30=1.000\r\n$32=1\r\n".into();
    rig.handle
        .start_job_for(job_lines(), 127, Some(1000.0))
        .unwrap();
    rig.sink.wait("trabajo terminado", |s| {
        s.jobs().contains(&JobState::Finished)
    });
    let logs = rig.sink.logs();
    assert!(
        logs.iter().any(|(d, t)| *d == LogDir::In
            && t.starts_with("Aviso: $30=1 ")
            && t.contains("S máx = 1000")),
        "{logs:?}"
    );
}

#[test]
fn nan_in_the_offsets_is_warned_about_before_the_job() {
    let rig = connect();
    *rig.fake.params.lock().unwrap() =
        "[G54:-nan,1.579,1.619]\r\n[G55:0.000,0.000,0.000]\r\n".into();
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.sink.wait("trabajo terminado", |s| {
        s.jobs().contains(&JobState::Finished)
    });
    assert!(
        rig.sink
            .logs()
            .iter()
            .any(|(_, t)| t.starts_with("Aviso: G54") && t.contains("$RST=#")),
        "{:?}",
        rig.sink.logs()
    );
}

// ---------------------------------------------------------------------------
// Paradas seguras durante un trabajo
// ---------------------------------------------------------------------------

#[test]
fn a_creality_fault_stops_the_job_safely_and_explains_it() {
    let rig = connect();
    rig.fake.brake.store(true, Ordering::SeqCst);
    rig.fake.run_on_gcode.store(true, Ordering::SeqCst);
    *rig.fake.fault_at.lock().unwrap() = Some((3, "ERROR:05".into()));
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.sink
        .wait("falla", |s| s.jobs().contains(&JobState::Error));
    let msgs = rig.sink.job_messages();
    let m = msgs.last().unwrap();
    assert!(m.contains("ERROR:05") && m.contains("llama"), "{m}");
    assert!(m.contains("Reset") && !m.contains("error:5"), "{m}");
    // Feed hold, espera a que frene y solo entonces el reset.
    assert_eq!(rig.fake.realtime(), [b'!', 0x18]);
    assert!(!rig.fake.reset_in_motion.load(Ordering::SeqCst));
    // La conexión sigue y vuelve a estar disponible.
    assert!(!rig.handle.is_closed());
    assert_eq!(rig.handle.command("$$").unwrap().last().unwrap(), "ok");
}

#[test]
fn an_error_with_the_stop_policy_really_stops_the_machine() {
    let rig = connect();
    rig.fake.brake.store(true, Ordering::SeqCst);
    rig.fake.run_on_gcode.store(true, Ordering::SeqCst);
    let lines = ["G90", "G21", "BAD", "G0 X1 Y1", "M5"]
        .map(String::from)
        .to_vec();
    rig.handle.start_job(lines, 127).unwrap();
    rig.sink
        .wait("error", |s| s.jobs().contains(&JobState::Error));
    let m = rig.sink.job_messages().pop().unwrap();
    assert!(
        m.contains("error:20") && m.contains("línea 3") && m.contains("se detuvo"),
        "{m}"
    );
    // Antes solo se registraba y la máquina seguía con lo que tenía en el búfer.
    assert_eq!(rig.fake.realtime(), [b'!', 0x18]);
    assert!(!rig.fake.reset_in_motion.load(Ordering::SeqCst));
}

#[test]
fn cancel_waits_for_the_hold_to_finish_before_resetting() {
    let rig = connect();
    rig.fake.brake.store(true, Ordering::SeqCst);
    rig.fake.run_on_gcode.store(true, Ordering::SeqCst);
    rig.fake.hold_ok.store(true, Ordering::SeqCst);
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.handle.cancel().unwrap();
    rig.sink
        .wait("cancelado", |s| s.jobs().contains(&JobState::Cancelled));
    assert_eq!(rig.fake.realtime(), [b'!', 0x18]);
    // El reset llegó con la máquina ya quieta: sin ALARM:3 ni aviso de posición.
    assert!(!rig.fake.reset_in_motion.load(Ordering::SeqCst));
    let m = rig.sink.job_messages().pop().unwrap();
    assert!(m.contains("esperó a que frenara"), "{m}");
    assert!(!m.contains("posición"), "{m}");
}

#[test]
fn cancel_that_cannot_wait_warns_that_the_position_may_be_lost() {
    let rig = connect();
    rig.fake.brake.store(true, Ordering::SeqCst);
    rig.fake.run_on_gcode.store(true, Ordering::SeqCst);
    rig.fake.never_settle.store(true, Ordering::SeqCst);
    rig.fake.hold_ok.store(true, Ordering::SeqCst);
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.handle.cancel().unwrap();
    rig.sink
        .wait("cancelado", |s| s.jobs().contains(&JobState::Cancelled));
    assert!(rig.fake.reset_in_motion.load(Ordering::SeqCst));
    let m = rig.sink.job_messages().pop().unwrap();
    assert!(m.contains("posición puede haberse perdido"), "{m}");
    assert!(
        rig.sink
            .logs()
            .iter()
            .any(|(_, t)| t.contains("posición puede haberse perdido")),
        "{:?}",
        rig.sink.logs()
    );
}

// ---------------------------------------------------------------------------
// Regla del silencio: `$H` y bloqueo crítico no son "conexión perdida"
// ---------------------------------------------------------------------------

#[test]
fn a_long_homing_silence_is_not_a_lost_connection() {
    let rig = connect();
    // 900 ms sin contestar nada (ni `?`), más del doble del plazo normal (400 ms).
    rig.fake.home_delay_ms.store(900, Ordering::SeqCst);
    let r = rig.handle.command("$H").unwrap();
    assert_eq!(r.last().unwrap(), "ok");
    assert!(!rig.handle.is_closed());
    assert!(rig.sink.connections().iter().all(|c| c.connected));
    // Tras el homing vale otra vez el plazo normal.
    rig.sink
        .wait("sondeo tras el homing", |s| s.statuses().len() > 3);
    rig.fake.mute.store(true, Ordering::SeqCst);
    rig.sink
        .wait("silencio", |s| s.connections().iter().any(|c| !c.connected));
}

#[test]
fn a_hard_limit_lock_is_tolerated_until_the_reset_brings_the_banner() {
    let rig = connect();
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.fake.locked.store(true, Ordering::SeqCst);
    rig.fake.say("ALARM:1\r\n[MSG:Reset to continue]\r\n");
    rig.sink.wait("alarma", |s| {
        s.logs().iter().any(|(_, t)| t.starts_with("ALARM:1"))
    });
    // Más del doble del plazo normal de silencio, sin una sola respuesta a `?`.
    thread::sleep(Duration::from_millis(1000));
    assert!(!rig.handle.is_closed(), "{:?}", rig.sink.connections());
    // Un soft reset desbloquea la placa; llega el saludo y vuelven los reportes.
    let n = rig.sink.statuses().len();
    rig.handle.realtime(RealtimeCommand::Reset).unwrap();
    rig.sink.wait("saludo", |s| {
        s.logs().iter().any(|(_, t)| t.starts_with("Grbl 1.1h"))
    });
    rig.sink
        .wait("sondeo tras el reset", |s| s.statuses().len() > n + 2);
    assert!(!rig.handle.is_closed());
}

#[test]
fn a_user_reset_while_the_machine_runs_warns_that_the_position_is_lost() {
    let rig = connect();
    rig.fake.set_state("Run");
    rig.sink.wait("estado Run", |s| {
        s.statuses()
            .last()
            .is_some_and(|x| x.state == MachineState::Run)
    });
    rig.handle.realtime(RealtimeCommand::Reset).unwrap();
    assert!(
        rig.sink
            .logs()
            .iter()
            .any(|(_, t)| t.contains("posición puede haberse perdido")),
        "{:?}",
        rig.sink.logs()
    );
}

#[test]
fn a_hard_limit_alarm_that_ends_a_job_does_not_end_the_connection() {
    let rig = connect();
    *rig.fake.fault_at.lock().unwrap() = Some((3, "ALARM:1\r\n[MSG:Reset to continue]".into()));
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.sink
        .wait("alarma", |s| s.jobs().contains(&JobState::Error));
    let m = rig.sink.job_messages().pop().unwrap();
    assert!(m.contains("ALARM:1") && m.contains("límite físico"), "{m}");
    // La placa quedó bloqueada y muda: más del doble del plazo normal sin que se dé por perdida.
    thread::sleep(Duration::from_millis(1000));
    assert!(!rig.handle.is_closed(), "{:?}", rig.sink.connections());
    rig.handle.realtime(RealtimeCommand::Reset).unwrap();
    let n = rig.sink.statuses().len();
    rig.sink
        .wait("sondeo tras el reset", |s| s.statuses().len() > n + 2);
}

// ---------------------------------------------------------------------------
// Máquina virtual (tlaku-emu con reloj de pared)
// ---------------------------------------------------------------------------

/// Conecta a un puerto virtual con la misma temporización de las pruebas.
fn connect_virtual(port: &str) -> Rig {
    let sink = Arc::new(Collect::default());
    let (conn, _) = Connection::start_virtual(port, sink.clone(), fast()).expect("conectar");
    let handle = conn.handle();
    // La `Fake` solo es relleno: la máquina de verdad es el emulador de `handle.emu`.
    let (_host, dev) = memory_pair();
    Rig {
        conn: Some(conn),
        handle,
        sink,
        fake: Fake::start(dev),
    }
}

/// Un trabajo largo (unos 24 s a ×1, 1,2 s a ×20) para poder pausar y cancelar en mitad.
fn long_job() -> Vec<String> {
    ["G90", "G21", "M4", "G1 X200 Y0 S500 F500", "M5"]
        .map(String::from)
        .to_vec()
}

fn emu(rig: &Rig) -> tlaku_emu::Handle {
    rig.handle.emu.clone().expect("máquina virtual")
}

#[test]
fn virtual_ports_come_with_a_prefix_that_no_real_port_uses() {
    let ports = virtual_port::ports();
    assert!(ports.len() >= 4);
    assert!(ports.iter().all(|p| virtual_port::is_virtual(p)));
    assert!(ports.contains(&"virtual:falcon2".to_string()));
    assert!(!virtual_port::is_virtual("/dev/cu.usbserial-1420"));
    assert!(!virtual_port::is_virtual("COM3"));
    // La velocidad va en el id; lo absurdo se rechaza con un mensaje en español.
    assert!(virtual_port::open("virtual:falcon2@5").is_ok());
    for bad in [
        "virtual:nada",
        "virtual:falcon2@0",
        "virtual:falcon2@x",
        "virtual:falcon2@1000",
    ] {
        let e = virtual_port::open(bad).err().unwrap_or_default();
        assert!(e.contains("virtual"), "{bad}: {e}");
    }
}

#[test]
fn connecting_to_each_virtual_machine_identifies_its_firmware() {
    for (port, fw) in [
        ("virtual:falcon2", "Grbl 1.1f"),
        ("virtual:grbl11", "Grbl 1.1h"),
        ("virtual:grblhal", "grblHAL"),
        ("virtual:fluidnc", "FluidNC"),
        ("virtual:marlin", "Marlin"),
        ("virtual:smoothie", "Smoothie"),
    ] {
        let sink = Arc::new(Collect::default());
        let (conn, info) = Connection::start_virtual(port, sink.clone(), fast()).expect(port);
        assert!(info.firmware.starts_with(fw), "{port}: {}", info.firmware);
        assert!(conn.handle().is_virtual());
        sink.wait("estado", |s| !s.statuses().is_empty());
        // El registro lo deja claro: no hay hardware real.
        assert!(
            sink.logs()
                .iter()
                .any(|(_, t)| t.contains("Máquina virtual")),
            "{:?}",
            sink.logs()
        );
        conn.close();
    }
}

#[test]
fn a_short_job_runs_to_the_end_on_the_virtual_machine() {
    let rig = connect_virtual("virtual:grbl11@50");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.start_job(job_lines(), 127).unwrap();
    rig.sink.wait("trabajo terminado", |s| {
        s.jobs().contains(&JobState::Finished)
    });
    assert_eq!(rig.sink.jobs(), [JobState::Started, JobState::Finished]);
    let last = rig.sink.progress().pop().unwrap();
    assert_eq!((last.acked, last.total), (6, 6));
    // La virtual recibió el G-code sin espacios, y el láser terminó apagado.
    let e = emu(&rig);
    let got = e.received_lines();
    for l in ["G90", "G21", "G0X1Y1", "G1X5Y5S100F1000"] {
        assert!(got.contains(&l.to_string()), "falta {l} en {got:?}");
    }
    e.wait_idle(Duration::from_secs(3));
    assert!(!e.laser_on());
    // Todo lo demás sigue funcionando: consola y jog.
    assert_eq!(rig.handle.command("$$").unwrap().last().unwrap(), "ok");
    rig.handle.jog(2.0, 0.0, 1000.0).unwrap();
}

#[test]
fn pause_resume_and_cancel_work_on_the_virtual_machine() {
    let rig = connect_virtual("virtual:falcon2@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.start_job(long_job(), 127).unwrap();
    let e = emu(&rig);
    assert!(e.wait_state(tlaku_emu::StateKind::Run, Duration::from_secs(3)));

    rig.handle.pause().unwrap();
    rig.sink
        .wait("pausa", |s| s.jobs().contains(&JobState::Paused));
    rig.sink.wait("Hold en la interfaz", |s| {
        s.statuses()
            .last()
            .is_some_and(|x| x.state == MachineState::Hold)
    });
    rig.handle.resume().unwrap();
    rig.sink.wait("reanudado", |s| s.jobs().len() >= 3);

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
    // Frenó antes del reset: la posición no se perdió y el láser quedó apagado.
    assert!(!e.laser_on());
    assert!(!e.position_lost());
    let cancelled = rig.sink.all().into_iter().find_map(|x| match x {
        Emit::Job(j) if j.state == JobState::Cancelled => Some(j),
        _ => None,
    });
    assert!(!cancelled.unwrap().position_lost);
    // La conexión sigue y vuelve a estar libre.
    assert!(!rig.handle.is_closed());
    assert_eq!(rig.handle.command("$$").unwrap().last().unwrap(), "ok");
}

#[test]
fn an_injected_creality_error_stops_the_virtual_job_and_explains_it() {
    let rig = connect_virtual("virtual:falcon2@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.start_job(long_job(), 127).unwrap();
    let e = emu(&rig);
    assert!(e.wait_state(tlaku_emu::StateKind::Run, Duration::from_secs(3)));
    // El mismo `ERROR:05` que inyectaría un fallo real de llama (por el mando de la interfaz).
    rig.handle.simulate_fault(SimulatedFault::Flame).unwrap();
    rig.sink
        .wait("falla", |s| s.jobs().contains(&JobState::Error));
    let m = rig.sink.job_messages().pop().unwrap();
    assert!(m.contains("ERROR:05") && m.contains("llama"), "{m}");
    assert!(
        rig.sink
            .logs()
            .iter()
            .any(|(d, t)| *d == LogDir::In && t.starts_with("ERROR:05")),
        "{:?}",
        rig.sink.logs()
    );
    // Se detuvo de forma segura: láser apagado y la conexión sigue viva.
    assert!(!e.laser_on());
    assert!(!rig.handle.is_closed());
}

#[test]
fn an_injected_error_05_is_explained_with_the_emulators_own_wording() {
    // Sin pasar por `simulate_fault`: se inyecta directo en el emulador, como lo haría una
    // prueba de la controladora.
    let rig = connect_virtual("virtual:falcon2@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.start_job(long_job(), 127).unwrap();
    let e = emu(&rig);
    assert!(e.wait_state(tlaku_emu::StateKind::Run, Duration::from_secs(3)));
    e.inject(
        tlaku_emu::Trigger::Now,
        tlaku_emu::Fault::CrealityError {
            code: 5,
            halt: true,
        },
    );
    rig.sink
        .wait("falla", |s| s.jobs().contains(&JobState::Error));
    let m = rig.sink.job_messages().pop().unwrap();
    assert!(m.contains("ERROR:05"), "{m}");
}

#[test]
fn alarm_3_during_a_virtual_job_reports_a_possibly_lost_position() {
    let rig = connect_virtual("virtual:grbl11@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.start_job(long_job(), 127).unwrap();
    let e = emu(&rig);
    assert!(e.wait_state(tlaku_emu::StateKind::Run, Duration::from_secs(3)));
    rig.handle.simulate_fault(SimulatedFault::Alarm3).unwrap();
    rig.sink
        .wait("alarma", |s| s.jobs().contains(&JobState::Error));
    let ev = rig
        .sink
        .all()
        .into_iter()
        .find_map(|x| match x {
            Emit::Job(j) if j.state == JobState::Error => Some(j),
            _ => None,
        })
        .unwrap();
    assert!(ev.message.contains("ALARM:3"), "{}", ev.message);
    assert!(ev.position_lost, "{ev:?}");
}

#[test]
fn a_virtual_disconnect_is_reported_as_a_lost_connection() {
    let rig = connect_virtual("virtual:grbl11@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle
        .simulate_fault(SimulatedFault::Disconnect)
        .unwrap();
    rig.sink.wait("conexión perdida", |s| {
        s.connections().iter().any(|c| !c.connected)
    });
    assert!(rig.handle.is_closed());
    assert_eq!(
        rig.handle
            .simulate_fault(SimulatedFault::Flame)
            .unwrap_err(),
        NOT_CONNECTED
    );
}

#[test]
fn faults_cannot_be_simulated_on_a_real_machine() {
    let rig = connect();
    assert!(!rig.handle.is_virtual());
    let e = rig
        .handle
        .simulate_fault(SimulatedFault::Flame)
        .unwrap_err();
    assert!(e.contains("máquina virtual"), "{e}");
}

// ---------------------------------------------------------------------------
// Marlin y Smoothieware (una línea por `ok`, sin `?`)
// ---------------------------------------------------------------------------

fn marlin_link() -> LinkProfile {
    LinkProfile::marlin(&tlaku_doc::GcodeDialect::marlin_inline())
}

fn smoothie_link() -> LinkProfile {
    LinkProfile::smoothie(&tlaku_doc::GcodeDialect::marlin_inline())
}

/// Un trabajo para Marlin/Smoothie: sin comandos GRBL, con espacios.
fn reprap_job() -> Vec<String> {
    [
        "G90",
        "G21",
        "M3 S0 I",
        "G0 X1 Y1",
        "G1 X5 Y5 S100 F1000",
        "M5",
    ]
    .map(String::from)
    .to_vec()
}

fn reprap_long_job() -> Vec<String> {
    ["G90", "G21", "M3 S0 I", "G1 X200 Y0 S500 F500", "M5"]
        .map(String::from)
        .to_vec()
}

#[test]
fn marlin_is_identified_with_m115_and_reports_its_position_without_a_question_mark() {
    let rig = connect_virtual("virtual:marlin@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    let e = emu(&rig);
    // Nunca se le manda `?`, que Marlin no entiende.
    assert!(e.realtime_bytes().is_empty());
    let sent = e.received_lines();
    assert!(sent.contains(&"M115".to_string()), "{sent:?}");
    assert!(sent.contains(&"M114".to_string()), "{sent:?}");
    // La consulta de posición no ensucia el registro.
    assert!(
        !rig.sink.logs().iter().any(|(_, t)| t.contains("M114")),
        "{:?}",
        rig.sink.logs()
    );
    // Y la conexión no se da por perdida por callar.
    thread::sleep(Duration::from_millis(600));
    assert!(!rig.handle.is_closed());
}

#[test]
fn a_job_runs_on_virtual_marlin_and_smoothie_one_line_per_ok() {
    for (port, link) in [
        ("virtual:marlin@50", marlin_link()),
        ("virtual:smoothie@50", smoothie_link()),
    ] {
        let rig = connect_virtual(port);
        rig.sink.wait("estado", |s| !s.statuses().is_empty());
        rig.handle
            .start_job_link(reprap_job(), link, Some(255.0))
            .unwrap_or_else(|e| panic!("{port}: {e}"));
        rig.sink.wait("trabajo terminado", |s| {
            s.jobs().contains(&JobState::Finished)
        });
        assert_eq!(
            rig.sink.jobs(),
            [JobState::Started, JobState::Finished],
            "{port}"
        );
        let last = rig.sink.progress().pop().unwrap();
        assert_eq!((last.acked, last.total), (6, 6), "{port}");
        // Los espacios se conservan (rompería `M3 S0 I`) y el G-code llega entero.
        let got = emu(&rig).received_lines();
        for l in ["M3 S0 I", "G1 X5 Y5 S100 F1000"] {
            assert!(got.contains(&l.to_string()), "{port}: falta {l} en {got:?}");
        }
        // El estado volvió a reposo y la consola sigue.
        rig.sink.wait("reposo", |s| {
            s.statuses()
                .last()
                .is_some_and(|x| x.state == MachineState::Idle)
        });
    }
}

#[test]
fn virtual_marlin_job_can_be_paused_resumed_and_cancelled_and_warns_about_the_position() {
    let rig = connect_virtual("virtual:marlin@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle
        .start_job_link(reprap_long_job(), marlin_link(), None)
        .unwrap();
    rig.sink.wait("en marcha", |s| {
        s.statuses()
            .last()
            .is_some_and(|x| x.state == MachineState::Run)
    });
    rig.handle.pause().unwrap();
    rig.sink
        .wait("pausa", |s| s.jobs().contains(&JobState::Paused));
    rig.sink.wait("Hold", |s| {
        s.statuses()
            .last()
            .is_some_and(|x| x.state == MachineState::Hold)
    });
    rig.handle.resume().unwrap();
    rig.sink.wait("reanudado", |s| s.jobs().len() >= 3);
    rig.handle.cancel().unwrap();
    rig.sink
        .wait("cancelado", |s| s.jobs().contains(&JobState::Cancelled));
    // Marlin no tiene reinicio en caliente: se avisa de que la posición puede estar perdida.
    let ev = rig
        .sink
        .all()
        .into_iter()
        .find_map(|x| match x {
            Emit::Job(j) if j.state == JobState::Cancelled => Some(j),
            _ => None,
        })
        .unwrap();
    assert!(ev.position_lost, "{ev:?}");
    // La parada rápida llega cuando la controladora termina la línea que tenía en curso (sin
    // `EMERGENCY_PARSER`, `M410` espera su turno) y el láser queda apagado.
    let e = emu(&rig);
    // (El tiempo de `run_until` es simulado; aquí se espera en tiempo real.)
    let end = Instant::now() + Duration::from_secs(5);
    while Instant::now() < end && !e.received_lines().contains(&"M410".to_string()) {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(
        e.received_lines().contains(&"M410".to_string()),
        "{:?} estado={:?}",
        e.received_lines(),
        e.state()
    );
    assert!(!e.laser_on());
    assert!(!rig.handle.is_closed());
}

#[test]
fn a_profile_of_another_family_is_refused_before_sending_anything() {
    let rig = connect_virtual("virtual:marlin@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    let e = rig
        .handle
        .start_job_link(job_lines(), LinkProfile::grbl_with_buffer(127), None)
        .unwrap_err();
    assert!(
        e.contains("Marlin") && e.contains("GRBL") && e.contains("No se transmitió nada"),
        "{e}"
    );
    let grbl = connect_virtual("virtual:grbl11@20");
    let e = grbl
        .handle
        .start_job_link(reprap_job(), marlin_link(), None)
        .unwrap_err();
    assert!(e.contains("Marlin") && e.contains("GRBL"), "{e}");
    assert!(
        emu(&grbl)
            .received_lines()
            .iter()
            .all(|l| !l.starts_with('G'))
    );
}

#[test]
fn jog_on_virtual_marlin_uses_relative_moves_and_restores_absolute_mode() {
    let rig = connect_virtual("virtual:marlin@50");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.jog(3.0, 2.0, 1000.0).unwrap();
    let got = emu(&rig).received_lines();
    let at = got.iter().position(|l| l == "G91").expect("G91");
    assert_eq!(&got[at..at + 3], ["G91", "G1 X3 Y2 F1000", "G90"]);
    assert!(got.iter().all(|l| !l.starts_with("$J")));
    // Los comandos en tiempo real de GRBL no existen aquí.
    let e = rig.handle.realtime(RealtimeCommand::Reset).unwrap_err();
    assert!(e.contains("tiempo real"), "{e}");
    assert!(emu(&rig).realtime_bytes().is_empty());
}

// ---------------------------------------------------------------------------
// Posicionar el láser (movimiento absoluto)
// ---------------------------------------------------------------------------

#[test]
fn jog_to_turns_the_laser_off_and_moves_to_the_absolute_position() {
    let rig = connect_virtual("virtual:grbl11@50");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.jog_to(25.0, 40.5, 3000.0).unwrap();
    let e = emu(&rig);
    let got = e.received_lines();
    assert!(got.contains(&"M5".to_string()), "{got:?}");
    assert!(
        got.iter()
            .any(|l| l.replace(' ', "") == "$J=G90G21X25Y40.5F3000"),
        "{got:?}"
    );
    e.wait_idle(Duration::from_secs(5));
    assert!(!e.laser_on());
    let w = e.wpos();
    assert!(
        (w[0] - 25.0).abs() < 1e-6 && (w[1] - 40.5).abs() < 1e-6,
        "{w:?}"
    );
    // La posición de trabajo que ve la interfaz es la misma.
    rig.sink.wait("posición en el reporte", |s| {
        s.statuses()
            .last()
            .is_some_and(|x| (x.work_pos[0] - 25.0).abs() < 1e-3)
    });
}

#[test]
fn jog_to_rejects_bad_numbers_and_a_running_job() {
    let rig = connect_virtual("virtual:falcon2@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    assert!(rig.handle.jog_to(f64::NAN, 0.0, 1000.0).is_err());
    assert!(rig.handle.jog_to(1.0, 1.0, 0.0).is_err());
    rig.handle.start_job(long_job(), 127).unwrap();
    let e = emu(&rig);
    assert!(e.wait_state(tlaku_emu::StateKind::Run, Duration::from_secs(3)));
    let err = rig.handle.jog_to(1.0, 1.0, 1000.0).unwrap_err();
    assert!(err.contains("trabajo en curso"), "{err}");
    rig.handle.cancel().unwrap();
}

#[test]
fn jog_to_works_on_marlin_with_plain_moves() {
    let rig = connect_virtual("virtual:marlin@50");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.jog_to(10.0, 20.0, 2400.0).unwrap();
    let got = emu(&rig).received_lines();
    assert!(
        got.iter().any(|l| l.starts_with("G1 X10 Y20 F2400")),
        "{got:?}"
    );
    assert!(got.iter().all(|l| !l.starts_with("$J")), "{got:?}");
}

// ---------------------------------------------------------------------------
// Overrides en vivo de velocidad y potencia (tlaku-emu los implementa)
// ---------------------------------------------------------------------------

fn last_overrides(sink: &Collect) -> Option<Overrides> {
    sink.statuses().last().and_then(|s| s.overrides)
}

#[test]
fn feed_and_power_overrides_reach_the_machine_and_come_back_in_the_status() {
    let rig = connect_virtual("virtual:grbl11@50");
    // Sin un `Ov:` visto, el plan parte de un reinicio: se espera al primero para que no varíe.
    rig.sink.wait("primer Ov:", |s| last_overrides(s).is_some());
    rig.handle.set_override(OverrideKind::Feed, 130).unwrap();
    rig.handle.set_override(OverrideKind::Spindle, 85).unwrap();
    let e = emu(&rig);
    let rt = e.realtime_bytes();
    // Desde el 100 % que informa la controladora: velocidad +10 tres veces; potencia -10 y -1
    // cinco veces. Sin reinicio, porque ya se conoce el valor de partida.
    let overrides: Vec<u8> = rt.iter().copied().filter(|b| *b >= 0x90).collect();
    assert_eq!(
        overrides,
        [0x91, 0x91, 0x91, 0x9B, 0x9D, 0x9D, 0x9D, 0x9D, 0x9D],
        "{rt:x?}"
    );
    // El `Ov:` real de la controladora llega a la interfaz.
    rig.sink.wait("Ov: en el reporte", |s| {
        last_overrides(s).is_some_and(|o| o.feed == 130 && o.spindle == 85)
    });
    // Volver al 100 % es un solo byte.
    let before = e.realtime_bytes().len();
    rig.handle.set_override(OverrideKind::Feed, 100).unwrap();
    assert_eq!(e.realtime_bytes()[before..], [0x90]);
    rig.sink.wait("100 % otra vez", |s| {
        last_overrides(s).is_some_and(|o| o.feed == 100 && o.spindle == 85)
    });
}

#[test]
fn overrides_are_clamped_to_the_grbl_range() {
    let rig = connect_virtual("virtual:grbl11@50");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.set_override(OverrideKind::Feed, 900).unwrap();
    rig.sink.wait("tope de 200 %", |s| {
        last_overrides(s).is_some_and(|o| o.feed == 200)
    });
    rig.handle.set_override(OverrideKind::Feed, 1).unwrap();
    rig.sink.wait("piso de 10 %", |s| {
        last_overrides(s).is_some_and(|o| o.feed == 10)
    });
}

#[test]
fn overrides_apply_to_a_job_in_progress() {
    let rig = connect_virtual("virtual:grbl11@20");
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.start_job(long_job(), 127).unwrap();
    let e = emu(&rig);
    assert!(e.wait_state(tlaku_emu::StateKind::Run, Duration::from_secs(3)));
    rig.handle.set_override(OverrideKind::Feed, 150).unwrap();
    rig.handle.set_override(OverrideKind::Spindle, 120).unwrap();
    rig.sink.wait("overrides durante el trabajo", |s| {
        last_overrides(s).is_some_and(|o| o.feed == 150 && o.spindle == 120)
    });
    // El trabajo sigue en marcha, sin pausa ni cancelación.
    assert_eq!(rig.sink.jobs(), [JobState::Started]);
    assert!(
        rig.sink
            .logs()
            .iter()
            .any(|(d, t)| *d == LogDir::Out && t.contains("override de velocidad")),
        "{:?}",
        rig.sink.logs()
    );
    rig.handle.cancel().unwrap();
}

#[test]
fn marlin_and_smoothie_refuse_overrides_with_a_reason() {
    for port in ["virtual:marlin@50", "virtual:smoothie@50"] {
        let rig = connect_virtual(port);
        rig.sink.wait("estado", |s| !s.statuses().is_empty());
        let err = rig
            .handle
            .set_override(OverrideKind::Feed, 120)
            .unwrap_err();
        assert!(
            err.contains("solo existen en controladoras GRBL"),
            "{port}: {err}"
        );
        assert!(emu(&rig).realtime_bytes().is_empty(), "{port}");
        assert!(last_overrides(&rig.sink).is_none(), "{port}");
    }
}

#[test]
fn overrides_with_the_fake_machine_send_one_byte_per_step_and_remember_the_value() {
    // La `Fake` no emite `Ov:`: el valor pedido se recuerda hasta que la controladora lo informe,
    // así dos pedidos seguidos parten del anterior y no de un reinicio.
    let rig = connect();
    rig.sink.wait("estado", |s| !s.statuses().is_empty());
    rig.handle.set_override(OverrideKind::Feed, 120).unwrap();
    rig.handle.set_override(OverrideKind::Feed, 125).unwrap();
    rig.sink.wait("bytes", |_| {
        rig.fake.seen.lock().unwrap().realtime.len() >= 8
    });
    let rt = rig.fake.seen.lock().unwrap().realtime.clone();
    let overrides: Vec<u8> = rt.into_iter().filter(|b| *b >= 0x90).collect();
    // reset, +10, +10, +1 x5
    assert_eq!(overrides, [0x90, 0x91, 0x91, 0x93, 0x93, 0x93, 0x93, 0x93]);
    assert_eq!(last_overrides(&rig.sink).map(|o| o.feed), Some(125));
}
