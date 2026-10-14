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
        checkpoint_interval: Duration::from_millis(30),
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

mod connection;
mod firmware;
mod jobs;
mod jog;
mod network;
mod overrides;
mod probe;
mod reprap;
mod resume;
mod virtual_machine;
