//! Pruebas del streamer contra un GRBL simulado: búfer RX acotado que consume
//! líneas a cierto ritmo, responde `ok` y puede inyectar errores y alarmas.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use laser_grbl::serial::{MemoryTransport, Transport, memory_pair};
use laser_grbl::streamer::{
    ErrorPolicy, Event, MachineState, Outcome, Realtime, StatusReport, StreamError, Streamer,
    StreamerConfig, prepare_line,
};

const GREETING: &str = "Grbl 1.1h ['$' for help]";

#[derive(Clone)]
struct MockConfig {
    /// Capacidad del búfer RX simulado.
    buffer: usize,
    /// Tiempo que tarda en consumir una línea.
    line_time: Duration,
    /// Tiempo que sigue "ocupado" tras consumir la última línea.
    exec_tail: Duration,
    /// Línea consumida (desde 0) -> código de `error:N`.
    errors: HashMap<usize, u32>,
    /// Línea consumida que dispara `ALARM:1`.
    alarm_at: Option<usize>,
    /// Línea consumida que provoca un reinicio de la placa.
    reset_at: Option<usize>,
    /// Línea consumida -> mensaje `[...]` emitido antes del `ok`.
    messages: HashMap<usize, String>,
    /// No responde a nada.
    silent: bool,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            buffer: 128,
            line_time: Duration::from_millis(1),
            exec_tail: Duration::ZERO,
            errors: HashMap::new(),
            alarm_at: None,
            reset_at: None,
            messages: HashMap::new(),
            silent: false,
        }
    }
}

#[derive(Default)]
struct MockState {
    /// Líneas completas recibidas, en orden de llegada.
    lines: Vec<String>,
    /// Bytes en tiempo real recibidos (sin contar `?`).
    realtime: Vec<u8>,
    max_occupancy: usize,
    overflowed: bool,
}

struct Mock {
    state: Arc<Mutex<MockState>>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

fn is_realtime(b: u8) -> bool {
    matches!(b, b'?' | b'!' | b'~' | 0x18) || b >= 0x80
}

impl Mock {
    fn start(cfg: MockConfig) -> (Mock, MemoryTransport) {
        let (host, dev) = memory_pair();
        let state = Arc::new(Mutex::new(MockState::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let handle = {
            let (state, stop) = (state.clone(), stop.clone());
            thread::spawn(move || run_mock(cfg, dev, state, stop))
        };
        let mock = Mock {
            state,
            stop,
            handle: Some(handle),
        };
        (mock, host)
    }

    fn lines(&self) -> Vec<String> {
        self.state.lock().unwrap().lines.clone()
    }

    fn arrived(&self) -> usize {
        self.state.lock().unwrap().lines.len()
    }

    fn realtime(&self) -> Vec<u8> {
        self.state.lock().unwrap().realtime.clone()
    }

    fn max_occupancy(&self) -> usize {
        self.state.lock().unwrap().max_occupancy
    }

    fn overflowed(&self) -> bool {
        self.state.lock().unwrap().overflowed
    }

    /// Corta la conexión como un cable desconectado.
    fn disconnect(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            h.join().unwrap();
        }
    }
}

impl Drop for Mock {
    fn drop(&mut self) {
        self.disconnect();
    }
}

fn run_mock(
    cfg: MockConfig,
    mut dev: MemoryTransport,
    state: Arc<Mutex<MockState>>,
    stop: Arc<AtomicBool>,
) {
    let mut rx: Vec<u8> = Vec::new();
    let mut held = false;
    let mut locked = false;
    let mut consumed = 0usize;
    let mut next_due = Instant::now();
    let mut busy_until = Instant::now();
    let mut buf = [0u8; 256];
    let say = |dev: &mut MemoryTransport, text: &str| {
        let _ = dev.write_all(format!("{text}\r\n").as_bytes());
    };
    while !stop.load(Ordering::SeqCst) {
        let n = match dev.read_timeout(&mut buf, Duration::from_millis(1)) {
            Ok(n) => n,
            Err(_) => return,
        };
        if cfg.silent {
            continue;
        }
        for &b in &buf[..n] {
            if is_realtime(b) {
                match b {
                    b'?' => {
                        let busy = !rx.is_empty() || Instant::now() < busy_until;
                        let st = if held {
                            "Hold:0"
                        } else if busy {
                            "Run"
                        } else {
                            "Idle"
                        };
                        say(&mut dev, &format!("<{st}|MPos:0.000,0.000,0.000|FS:0,0>"));
                        continue;
                    }
                    b'!' => held = true,
                    b'~' => held = false,
                    0x18 => {
                        rx.clear();
                        held = false;
                        locked = false;
                        say(&mut dev, GREETING);
                    }
                    _ => {}
                }
                state.lock().unwrap().realtime.push(b);
            } else {
                rx.push(b);
                let mut s = state.lock().unwrap();
                s.max_occupancy = s.max_occupancy.max(rx.len());
                if rx.len() > cfg.buffer {
                    s.overflowed = true;
                    rx.pop();
                }
                if b == b'\n' {
                    // Llegó una línea completa: se registra sin sacarla del búfer.
                    let start = rx[..rx.len() - 1]
                        .iter()
                        .rposition(|&c| c == b'\n')
                        .map_or(0, |i| i + 1);
                    let line = String::from_utf8_lossy(&rx[start..rx.len() - 1]).into_owned();
                    s.lines.push(line);
                }
            }
        }
        // El consumo sigue en feed hold: el planificador aún tiene espacio.
        while Instant::now() >= next_due {
            let Some(pos) = rx.iter().position(|&c| c == b'\n') else {
                next_due = Instant::now();
                break;
            };
            rx.drain(..=pos);
            next_due += cfg.line_time;
            let idx = consumed;
            consumed += 1;
            busy_until = Instant::now() + cfg.exec_tail;
            if locked {
                say(&mut dev, "error:9");
            } else if cfg.alarm_at == Some(idx) {
                locked = true;
                say(&mut dev, "ALARM:1");
            } else if cfg.reset_at == Some(idx) {
                rx.clear();
                say(&mut dev, GREETING);
            } else if let Some(code) = cfg.errors.get(&idx) {
                say(&mut dev, &format!("error:{code}"));
            } else {
                if let Some(m) = cfg.messages.get(&idx) {
                    say(&mut dev, m);
                }
                say(&mut dev, "ok");
            }
        }
    }
}

fn gcode_lines(n: usize) -> Vec<String> {
    (0..n)
        .map(|i| {
            let pad = "0".repeat(i % 9);
            format!(
                "G1 X{i}.{pad}25 Y{} S{} ; línea {i}",
                i * 3 % 400,
                (i * 7) % 1000
            )
        })
        .collect()
}

fn expected(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .map(|l| prepare_line(l).unwrap())
        .filter(|l| !l.is_empty())
        .map(|l| l + "\n")
        .collect()
}

fn with_newlines(lines: Vec<String>) -> Vec<String> {
    lines.into_iter().map(|l| l + "\n").collect()
}

fn fast_config() -> StreamerConfig {
    StreamerConfig {
        status_interval: Some(Duration::from_millis(10)),
        silence_timeout: Duration::from_secs(3),
        cancel_delay: Duration::from_millis(5),
        ..Default::default()
    }
}

/// Ejecuta el trabajo en este hilo y devuelve el resultado con todos los eventos.
fn run_sync(
    transport: MemoryTransport,
    config: StreamerConfig,
    lines: &[String],
) -> (Outcome, Vec<Event>) {
    let mut streamer = Streamer::new(transport, config);
    let mut events = Vec::new();
    let out = streamer.run(lines, |e| events.push(e)).unwrap();
    (out, events)
}

fn wait_for(events: &std::sync::mpsc::Receiver<Event>, mut f: impl FnMut(&Event) -> bool) -> Event {
    loop {
        let e = events
            .recv_timeout(Duration::from_secs(10))
            .expect("el trabajo no produjo el evento esperado");
        if f(&e) {
            return e;
        }
    }
}

fn progress_acked(events: &[Event]) -> Vec<usize> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Progress { acked, .. } => Some(*acked),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Conteo de caracteres
// ---------------------------------------------------------------------------

#[test]
fn all_lines_arrive_in_order_and_buffer_never_overflows() {
    let lines = gcode_lines(400);
    let (mock, host) = Mock::start(MockConfig::default());
    let (out, events) = run_sync(host, fast_config(), &lines);

    assert_eq!(out, Outcome::Completed { errors: 0 });
    assert_eq!(with_newlines(mock.lines()), expected(&lines));
    assert!(!mock.overflowed());
    assert!(mock.max_occupancy() <= 127);
    // El búfer realmente se aprovecha (no es una línea por vez).
    assert!(
        mock.max_occupancy() > 90,
        "ocupación {}",
        mock.max_occupancy()
    );

    let acked = progress_acked(&events);
    assert_eq!(acked.len(), 400);
    assert!(acked.windows(2).all(|w| w[1] == w[0] + 1));
    match events
        .iter()
        .rev()
        .find(|e| matches!(e, Event::Progress { .. }))
    {
        Some(Event::Progress {
            acked,
            sent,
            total,
            buffer_used,
        }) => assert_eq!((*acked, *sent, *total, *buffer_used), (400, 400, 400, 0)),
        other => panic!("{other:?}"),
    }
    assert_eq!(events.last(), Some(&Event::Finished(out)));
    assert!(events.iter().any(|e| matches!(e, Event::Status(_))));
}

#[test]
fn progress_never_reports_more_bytes_than_the_configured_buffer() {
    let lines = gcode_lines(150);
    let (_mock, host) = Mock::start(MockConfig {
        buffer: 40,
        ..Default::default()
    });
    let config = StreamerConfig {
        rx_buffer: 40,
        ..fast_config()
    };
    let (out, events) = run_sync(host, config, &lines);
    assert_eq!(out, Outcome::Completed { errors: 0 });
    for e in &events {
        if let Event::Progress { buffer_used, .. } = e {
            assert!(*buffer_used <= 40);
        }
    }
}

#[test]
fn small_and_large_buffers_are_respected() {
    for size in [32usize, 256, 1024] {
        let lines = gcode_lines(200);
        let (mock, host) = Mock::start(MockConfig {
            buffer: size,
            ..Default::default()
        });
        let config = StreamerConfig {
            rx_buffer: size,
            ..fast_config()
        };
        let (out, _) = run_sync(host, config, &lines);
        assert_eq!(out, Outcome::Completed { errors: 0 });
        assert!(!mock.overflowed(), "desborde con búfer {size}");
        assert!(mock.max_occupancy() <= size);
        assert_eq!(with_newlines(mock.lines()), expected(&lines));
    }
}

#[test]
fn comments_and_blank_lines_are_not_sent() {
    let lines: Vec<String> = ["; inicio", "G21 (mm)", "", "G90   ", "  (nada)  ", "M4 S0"]
        .map(String::from)
        .to_vec();
    let (mock, host) = Mock::start(MockConfig::default());
    let (out, events) = run_sync(host, fast_config(), &lines);
    assert_eq!(out, Outcome::Completed { errors: 0 });
    assert_eq!(mock.lines(), ["G21", "G90", "M4S0"]);
    assert_eq!(progress_acked(&events), [1, 2, 3]);
}

#[test]
fn line_longer_than_buffer_is_rejected_before_sending() {
    let (mock, host) = Mock::start(MockConfig::default());
    let mut streamer = Streamer::new(host, fast_config());
    let mut lines = gcode_lines(3);
    lines.push(format!("G1 X{}", "1".repeat(200)));
    let err = streamer.run(&lines, |_| {}).unwrap_err();
    assert!(matches!(
        err,
        StreamError::LineTooLong {
            source_line: 4,
            max: 127,
            ..
        }
    ));
    thread::sleep(Duration::from_millis(30));
    assert_eq!(mock.arrived(), 0);
}

#[test]
fn line_exactly_filling_the_buffer_is_accepted() {
    let line = format!("G1X{}", "1".repeat(123)); // 126 bytes + '\n' = 127
    assert_eq!(line.len() + 1, 127);
    let (mock, host) = Mock::start(MockConfig::default());
    let (out, _) = run_sync(host, fast_config(), std::slice::from_ref(&line));
    assert_eq!(out, Outcome::Completed { errors: 0 });
    assert_eq!(mock.lines(), [line]);
    assert!(!mock.overflowed());
}

#[test]
fn realtime_lookalike_in_line_is_rejected() {
    let (mock, host) = Mock::start(MockConfig::default());
    let mut streamer = Streamer::new(host, fast_config());
    let err = streamer
        .run(&["G0 X1", "G1 X2 !", "G0 X3"], |_| {})
        .unwrap_err();
    assert_eq!(
        err,
        StreamError::InvalidChar {
            source_line: 2,
            ch: '!'
        }
    );
    assert_eq!(mock.arrived(), 0);
}

#[test]
fn empty_job_finishes_immediately() {
    let (mock, host) = Mock::start(MockConfig::default());
    let (out, events) = run_sync(host, fast_config(), &[]);
    assert_eq!(out, Outcome::Completed { errors: 0 });
    assert_eq!(events.len(), 1);
    assert_eq!(mock.arrived(), 0);
}

#[test]
fn finishes_only_after_machine_reports_idle() {
    let tail = Duration::from_millis(120);
    let (_mock, host) = Mock::start(MockConfig {
        exec_tail: tail,
        ..Default::default()
    });
    let mut streamer = Streamer::new(host, fast_config());
    let mut last_ack = None;
    let mut finished = None;
    streamer
        .run(&gcode_lines(5), |e| match e {
            Event::Progress { .. } => last_ack = Some(Instant::now()),
            Event::Finished(_) => finished = Some(Instant::now()),
            _ => {}
        })
        .unwrap();
    let waited = finished.unwrap() - last_ack.unwrap();
    assert!(
        waited >= tail / 2,
        "terminó {waited:?} después del último ok"
    );

    // Sin esperar reposo, termina con el último ok.
    let (_mock, host) = Mock::start(MockConfig {
        exec_tail: tail,
        ..Default::default()
    });
    let config = StreamerConfig {
        wait_idle: false,
        ..fast_config()
    };
    let mut streamer = Streamer::new(host, config);
    let mut last_ack = None;
    let mut finished = None;
    streamer
        .run(&gcode_lines(5), |e| match e {
            Event::Progress { .. } => last_ack = Some(Instant::now()),
            Event::Finished(_) => finished = Some(Instant::now()),
            _ => {}
        })
        .unwrap();
    assert!(finished.unwrap() - last_ack.unwrap() < tail / 2);
}

#[test]
fn works_without_status_polling() {
    let (mock, host) = Mock::start(MockConfig::default());
    let config = StreamerConfig {
        status_interval: None,
        ..fast_config()
    };
    let (out, events) = run_sync(host, config, &gcode_lines(50));
    assert_eq!(out, Outcome::Completed { errors: 0 });
    assert!(!events.iter().any(|e| matches!(e, Event::Status(_))));
    assert!(mock.realtime().is_empty());
}

// ---------------------------------------------------------------------------
// Errores, alarmas, mensajes
// ---------------------------------------------------------------------------

#[test]
fn error_stops_the_job_by_default() {
    let lines = gcode_lines(300);
    let (mock, host) = Mock::start(MockConfig {
        errors: HashMap::from([(5, 20)]),
        ..Default::default()
    });
    let (out, events) = run_sync(host, fast_config(), &lines);
    assert_eq!(
        out,
        Outcome::LineError {
            code: 20,
            source_line: 6
        }
    );
    assert!(events.contains(&Event::Error {
        code: 20,
        source_line: Some(6),
        text: Some(prepare_line(&lines[5]).unwrap()),
    }));
    // Solo se pudo enviar lo que cabía en el búfer cuando falló la línea.
    assert!(
        mock.arrived() < 6 + 128 / 10 + 2,
        "llegaron {}",
        mock.arrived()
    );
    assert_eq!(events.last(), Some(&Event::Finished(out)));
}

#[test]
fn error_policy_continue_delivers_everything_and_counts_errors() {
    // Cada tercera línea de entrada es vacía: los números de línea deben seguir siendo los originales.
    let mut lines = Vec::new();
    for (i, l) in gcode_lines(60).into_iter().enumerate() {
        lines.push(l);
        if i % 2 == 1 {
            lines.push(String::new());
        }
    }
    let (mock, host) = Mock::start(MockConfig {
        errors: HashMap::from([(5, 20), (9, 2)]),
        ..Default::default()
    });
    let config = StreamerConfig {
        error_policy: ErrorPolicy::Continue,
        ..fast_config()
    };
    let (out, events) = run_sync(host, config, &lines);
    assert_eq!(out, Outcome::Completed { errors: 2 });
    assert_eq!(with_newlines(mock.lines()), expected(&lines));
    let errors: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::Error {
                code, source_line, ..
            } => Some((*code, source_line.unwrap())),
            _ => None,
        })
        .collect();
    // La línea consumida n.º 5 (la sexta enviada) está en la posición 8 de la entrada, y la 9 en la 14.
    assert_eq!(errors, [(20, 8), (2, 14)]);
    assert_eq!(*progress_acked(&events).last().unwrap(), 60);
}

#[test]
fn alarm_aborts_the_job() {
    let (_mock, host) = Mock::start(MockConfig {
        alarm_at: Some(4),
        ..Default::default()
    });
    let (out, events) = run_sync(host, fast_config(), &gcode_lines(200));
    assert_eq!(out, Outcome::Alarm(1));
    assert!(events.contains(&Event::Alarm(1)));
    assert_eq!(events.last(), Some(&Event::Finished(Outcome::Alarm(1))));
}

#[test]
fn bracket_messages_do_not_disturb_the_count() {
    let (mock, host) = Mock::start(MockConfig {
        messages: HashMap::from([
            (2, "[MSG:Caution: Unlocked]".to_string()),
            (7, "[GC:G0 G54 G17 G21 G90 G94 M5 M9 T0 F0 S0]".to_string()),
        ]),
        ..Default::default()
    });
    let lines = gcode_lines(20);
    let (out, events) = run_sync(host, fast_config(), &lines);
    assert_eq!(out, Outcome::Completed { errors: 0 });
    let msgs: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::Message(m) => Some(m.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        msgs,
        [
            "[MSG:Caution: Unlocked]",
            "[GC:G0 G54 G17 G21 G90 G94 M5 M9 T0 F0 S0]"
        ]
    );
    assert_eq!(*progress_acked(&events).last().unwrap(), 20);
    assert_eq!(mock.arrived(), 20);
}

#[test]
fn board_reset_greeting_aborts_the_job() {
    let (_mock, host) = Mock::start(MockConfig {
        reset_at: Some(3),
        ..Default::default()
    });
    let (out, events) = run_sync(host, fast_config(), &gcode_lines(100));
    assert_eq!(out, Outcome::Reset(GREETING.to_string()));
    assert!(events.contains(&Event::Reset(GREETING.to_string())));
}

#[test]
fn lost_connection_is_reported() {
    let (mut mock, host) = Mock::start(MockConfig {
        line_time: Duration::from_millis(20),
        ..Default::default()
    });
    let job = Streamer::new(host, fast_config())
        .spawn(&gcode_lines(100))
        .unwrap();
    wait_for(&job.events, |e| matches!(e, Event::Progress { .. }));
    mock.disconnect();
    let (_, out) = job.join().unwrap();
    assert!(matches!(out, Outcome::Disconnected(_)), "{out:?}");
}

#[test]
fn silent_device_is_detected_by_timeout() {
    let (_mock, host) = Mock::start(MockConfig {
        silent: true,
        ..Default::default()
    });
    let config = StreamerConfig {
        silence_timeout: Duration::from_millis(150),
        ..fast_config()
    };
    let started = Instant::now();
    let (out, _) = run_sync(host, config, &gcode_lines(10));
    assert!(matches!(out, Outcome::Disconnected(_)), "{out:?}");
    assert!(started.elapsed() < Duration::from_secs(3));
}

// ---------------------------------------------------------------------------
// Pausa, reanudación, cancelación y tiempo real
// ---------------------------------------------------------------------------

fn slow_mock() -> (Mock, MemoryTransport) {
    Mock::start(MockConfig {
        line_time: Duration::from_millis(3),
        ..Default::default()
    })
}

#[test]
fn pause_stops_sending_and_resume_continues() {
    let lines = gcode_lines(200);
    let (mock, host) = slow_mock();
    let job = Streamer::new(host, fast_config()).spawn(&lines).unwrap();
    wait_for(
        &job.events,
        |e| matches!(e, Event::Progress { acked, .. } if *acked >= 10),
    );

    job.control.pause();
    thread::sleep(Duration::from_millis(60));
    let frozen = mock.arrived();
    thread::sleep(Duration::from_millis(120));
    assert_eq!(mock.arrived(), frozen, "se enviaron líneas en pausa");
    assert!(frozen < 200);
    assert_eq!(mock.realtime(), [b'!']);

    // En pausa sigue el sondeo y la controladora informa Hold.
    wait_for(
        &job.events,
        |e| matches!(e, Event::Status(r) if r.state == MachineState::Hold(Some(0))),
    );

    job.control.pause(); // repetir no envía otro feed hold
    job.control.resume();
    wait_for(&job.events, |e| matches!(e, Event::Finished(_)));
    let (_, out) = job.join().unwrap();
    assert_eq!(out, Outcome::Completed { errors: 0 });
    assert_eq!(mock.realtime(), [b'!', b'~']);
    assert_eq!(with_newlines(mock.lines()), expected(&lines));
    assert!(!mock.overflowed());
}

#[test]
fn resume_without_pause_sends_nothing() {
    let (mock, host) = slow_mock();
    let job = Streamer::new(host, fast_config())
        .spawn(&gcode_lines(40))
        .unwrap();
    job.control.resume();
    let (_, out) = job.join().unwrap();
    assert_eq!(out, Outcome::Completed { errors: 0 });
    assert!(mock.realtime().is_empty());
}

#[test]
fn cancel_sends_hold_then_reset_and_finishes() {
    let (mock, host) = slow_mock();
    let job = Streamer::new(host, fast_config())
        .spawn(&gcode_lines(500))
        .unwrap();
    wait_for(
        &job.events,
        |e| matches!(e, Event::Progress { acked, .. } if *acked >= 5),
    );
    job.control.cancel();
    wait_for(&job.events, |e| {
        matches!(e, Event::Finished(Outcome::Cancelled))
    });
    let (streamer, out) = job.join().unwrap();
    assert_eq!(out, Outcome::Cancelled);
    assert_eq!(mock.realtime(), [b'!', 0x18]);
    assert!(mock.arrived() < 500);
    drop(streamer);
}

#[test]
fn cancel_while_paused_works() {
    let (mock, host) = slow_mock();
    let job = Streamer::new(host, fast_config())
        .spawn(&gcode_lines(300))
        .unwrap();
    wait_for(&job.events, |e| matches!(e, Event::Progress { .. }));
    job.control.pause();
    thread::sleep(Duration::from_millis(30));
    job.control.cancel();
    let (_, out) = job.join().unwrap();
    assert_eq!(out, Outcome::Cancelled);
    assert_eq!(mock.realtime(), [b'!', b'!', 0x18]);
}

#[test]
fn realtime_overrides_pass_through_without_counting() {
    let (mock, host) = slow_mock();
    let lines = gcode_lines(60);
    let job = Streamer::new(host, fast_config()).spawn(&lines).unwrap();
    wait_for(&job.events, |e| matches!(e, Event::Progress { .. }));
    for cmd in [
        Realtime::FeedPlus10,
        Realtime::SpindleMinus1,
        Realtime::RapidHalf,
        Realtime::JogCancel,
    ] {
        job.control.realtime(cmd);
    }
    let (_, out) = job.join().unwrap();
    assert_eq!(out, Outcome::Completed { errors: 0 });
    assert_eq!(mock.realtime(), [0x91, 0x9D, 0x96, 0x85]);
    assert_eq!(with_newlines(mock.lines()), expected(&lines));
}

#[test]
fn soft_reset_via_control_ends_job_as_cancelled() {
    let (mock, host) = slow_mock();
    let job = Streamer::new(host, fast_config())
        .spawn(&gcode_lines(300))
        .unwrap();
    wait_for(&job.events, |e| matches!(e, Event::Progress { .. }));
    job.control.realtime(Realtime::SoftReset);
    let (_, out) = job.join().unwrap();
    assert_eq!(out, Outcome::Cancelled);
    assert_eq!(mock.realtime(), [0x18]);
}

#[test]
fn streamer_can_be_reused_after_a_job() {
    let (mock, host) = Mock::start(MockConfig::default());
    let mut streamer = Streamer::new(host, fast_config());
    let first = gcode_lines(30);
    assert_eq!(
        streamer.run(&first, |_| {}).unwrap(),
        Outcome::Completed { errors: 0 }
    );
    let control = streamer.control();
    control.cancel(); // mando obsoleto: el siguiente trabajo no debe verlo
    let second = gcode_lines(30);
    assert_eq!(
        streamer.run(&second, |_| {}).unwrap(),
        Outcome::Completed { errors: 0 }
    );
    assert_eq!(mock.arrived(), 60);
}

#[test]
fn send_realtime_outside_a_job() {
    let (mock, host) = Mock::start(MockConfig::default());
    let mut streamer = Streamer::new(host, fast_config());
    streamer.send_realtime(Realtime::JogCancel).unwrap();
    thread::sleep(Duration::from_millis(30));
    assert_eq!(mock.realtime(), [0x85]);
}

// ---------------------------------------------------------------------------
// Parser del reporte de estado
// ---------------------------------------------------------------------------

#[test]
fn status_basic_idle() {
    let r = StatusReport::parse("<Idle|MPos:0.000,0.000,0.000|FS:0,0>").unwrap();
    assert_eq!(r.state, MachineState::Idle);
    let m = r.mpos.unwrap();
    assert_eq!((m.x, m.y, m.z), (0.0, 0.0, 0.0));
    assert_eq!((r.feed, r.spindle), (Some(0.0), Some(0.0)));
    assert!(r.wpos.is_none() && r.wco.is_none() && r.buffer.is_none());
    assert!(r.overrides.is_none() && r.pins.is_none() && r.line_number.is_none());
}

#[test]
fn status_full_report_with_wpos_buffer_line_pins_accessories() {
    let r = StatusReport::parse(
        "<Run|WPos:-1.250,20.500,3.000|Bf:15,128|Ln:99999|FS:500,8000|Pn:XYZ|A:SFM|Ov:120,50,80>",
    )
    .unwrap();
    assert_eq!(r.state, MachineState::Run);
    let w = r.wpos.unwrap();
    assert_eq!((w.x, w.y, w.z), (-1.25, 20.5, 3.0));
    assert!(r.mpos.is_none());
    let b = r.buffer.unwrap();
    assert_eq!((b.planner_blocks, b.rx_bytes), (15, 128));
    assert_eq!(r.line_number, Some(99999));
    assert_eq!((r.feed, r.spindle), (Some(500.0), Some(8000.0)));
    assert_eq!(r.pins.as_deref(), Some("XYZ"));
    assert_eq!(r.accessories.as_deref(), Some("SFM"));
    let o = r.overrides.unwrap();
    assert_eq!((o.feed, o.rapid, o.spindle), (120, 50, 80));
}

#[test]
fn status_wco_and_feed_only_variant() {
    let r =
        StatusReport::parse("<Idle|MPos:5.000,6.000,0.000|F:250|WCO:10.000,20.000,0.000>").unwrap();
    assert_eq!((r.feed, r.spindle), (Some(250.0), None));
    let wco = r.wco.unwrap();
    assert_eq!((wco.x, wco.y, wco.z), (10.0, 20.0, 0.0));
}

#[test]
fn status_substates() {
    let hold0 = StatusReport::parse("<Hold:0|MPos:0.000,0.000,0.000|FS:0,0>").unwrap();
    assert_eq!(hold0.state, MachineState::Hold(Some(0)));
    let hold1 = StatusReport::parse("<Hold:1|MPos:0.000,0.000,0.000|FS:0,0>").unwrap();
    assert_eq!(hold1.state, MachineState::Hold(Some(1)));
    let door = StatusReport::parse("<Door:1|MPos:0.000,0.000,0.000|FS:0,0>").unwrap();
    assert_eq!(door.state, MachineState::Door(Some(1)));
    assert_eq!(
        StatusReport::parse("<Door:3|MPos:0,0,0>").unwrap().state,
        MachineState::Door(Some(3))
    );
}

#[test]
fn status_every_state_name() {
    for (name, state) in [
        ("Idle", MachineState::Idle),
        ("Run", MachineState::Run),
        ("Jog", MachineState::Jog),
        ("Alarm", MachineState::Alarm),
        ("Check", MachineState::Check),
        ("Home", MachineState::Home),
        ("Sleep", MachineState::Sleep),
    ] {
        let r = StatusReport::parse(&format!("<{name}|MPos:1.000,2.000,3.000>")).unwrap();
        assert_eq!(r.state, state);
    }
}

#[test]
fn status_rejects_non_reports() {
    for bad in [
        "",
        "ok",
        "error:1",
        "[MSG:x]",
        "<Idle",
        "Idle>",
        "<|MPos:0,0,0>",
        "<Nope>",
    ] {
        assert!(StatusReport::parse(bad).is_none(), "{bad:?}");
    }
}
