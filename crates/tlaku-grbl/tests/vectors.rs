//! Vectores de prueba de `docs/hardware/grbl.md` (sección 11) y `docs/hardware/falcon2.md`
//! (sección 8) contra el `Streamer`, con una máquina guionizada sobre `memory_pair`.
//!
//! A diferencia de `streamer.rs` (un GRBL simulado con búfer), aquí cada prueba escribe en
//! pocas líneas lo que responde la máquina: banners reales, `ERROR:05` en pleno envío,
//! `ALARM:3` seguido del saludo, hold que tarda en frenar, etc.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use tlaku_grbl::serial::{MemoryTransport, Transport, memory_pair};
use tlaku_grbl::streamer::{
    ErrorPolicy, Event, MachineState, Outcome, Realtime, StatusReport, Streamer, StreamerConfig,
};

// ---------------------------------------------------------------------------
// Máquina guionizada
// ---------------------------------------------------------------------------

/// Lo que recibe la máquina: una línea completa o un byte de tiempo real.
enum Input<'a> {
    Line(&'a str),
    Byte(u8),
}

fn is_realtime(b: u8) -> bool {
    matches!(b, b'?' | b'!' | b'~' | 0x18) || b >= 0x80
}

/// Texto → bytes con `\r\n` tras cada línea.
fn say(lines: &[&str]) -> Vec<u8> {
    lines
        .iter()
        .flat_map(|l| format!("{l}\r\n").into_bytes())
        .collect()
}

struct Device {
    /// Bytes de tiempo real recibidos (sin contar `?`), en orden.
    rt: Arc<Mutex<Vec<u8>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Device {
    fn rt(&self) -> Vec<u8> {
        self.rt.lock().unwrap().clone()
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Máquina que contesta con los bytes que devuelva `f` a cada entrada.
fn device(mut f: impl FnMut(Input) -> Vec<u8> + Send + 'static) -> (Device, MemoryTransport) {
    let (host, mut dev) = memory_pair();
    let rt = Arc::new(Mutex::new(Vec::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let thread = {
        let (rt, stop) = (rt.clone(), stop.clone());
        thread::spawn(move || {
            let mut partial = Vec::new();
            let mut buf = [0u8; 256];
            while !stop.load(Ordering::SeqCst) {
                let Ok(n) = dev.read_timeout(&mut buf, Duration::from_millis(1)) else {
                    return;
                };
                for &b in &buf[..n] {
                    let reply = if is_realtime(b) {
                        if b != b'?' {
                            rt.lock().unwrap().push(b);
                        }
                        f(Input::Byte(b))
                    } else if b == b'\n' {
                        let line = String::from_utf8_lossy(&std::mem::take(&mut partial))
                            .trim()
                            .to_string();
                        f(Input::Line(&line))
                    } else {
                        partial.push(b);
                        continue;
                    };
                    if !reply.is_empty() {
                        let _ = dev.write_all(&reply);
                    }
                }
            }
        })
    };
    (
        Device {
            rt,
            stop,
            thread: Some(thread),
        },
        host,
    )
}

fn config() -> StreamerConfig {
    StreamerConfig {
        status_interval: Some(Duration::from_millis(10)),
        cancel_delay: Duration::from_millis(5),
        hold_timeout: Duration::from_millis(400),
        hold_poll_interval: Duration::from_millis(10),
        reset_wait: Duration::from_millis(300),
        ..Default::default()
    }
}

fn gcode(n: usize) -> Vec<String> {
    (1..=n)
        .map(|i| format!("G1 X{i} Y{i} S100 F1000"))
        .collect()
}

fn run(transport: MemoryTransport, cfg: StreamerConfig, lines: &[String]) -> (Outcome, Vec<Event>) {
    let mut s = Streamer::new(transport, cfg);
    let mut events = Vec::new();
    let out = s.run(lines, |e| events.push(e)).unwrap();
    (out, events)
}

fn messages(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Message(m) => Some(m.clone()),
            _ => None,
        })
        .collect()
}

fn status(state: &str) -> Vec<u8> {
    say(&[&format!("<{state}|MPos:10.000,0.000,0.000|FS:0,0>")])
}

// ---------------------------------------------------------------------------
// L-3: bytes inválidos no hacen panic
// ---------------------------------------------------------------------------

#[test]
fn garbage_bytes_in_the_middle_of_a_job_do_not_panic() {
    // Líneas basura (UTF-8 inválido) entre las respuestas; una de ellas empieza por "gr".
    let garbage: [&[u8]; 6] = [
        b"ab\xff\xfe\r\n",
        b"\xff\xfe\xfd\xfc\xfb\r\n",
        b"gr\xff\r\n",
        b"grb\xe2\x82\r\n",
        b"\xc3\r\n",
        b"<\xff\xfe|MPos:0,0,0>\r\n",
    ];
    let mut n = 0usize;
    let (_dev, host) = device(move |inp| match inp {
        Input::Line(_) => {
            let mut out = garbage[n % garbage.len()].to_vec();
            out.extend_from_slice(b"ok\r\n");
            n += 1;
            out
        }
        Input::Byte(b'?') => status("Idle"),
        Input::Byte(_) => Vec::new(),
    });
    let (out, events) = run(host, config(), &gcode(12));
    assert_eq!(out, Outcome::Completed { errors: 0 });
    // Nada de la basura se tomó por un saludo: ningún reinicio.
    assert!(!events.iter().any(|e| matches!(e, Event::Reset(_))));
    assert_eq!(messages(&events).len(), 12);
}

#[test]
fn garbage_bytes_while_stopping_do_not_panic_either() {
    let (_dev, host) = device(|inp| match inp {
        Input::Line(_) => Vec::new(),
        Input::Byte(b'?') => {
            let mut v = b"ab\xff\xfe\r\n".to_vec();
            v.extend(status("Hold:0"));
            v
        }
        Input::Byte(0x18) => b"\xff\xfe\r\nGrbl 1.1h ['$' for help]\r\n".to_vec(),
        Input::Byte(_) => Vec::new(),
    });
    let mut s = Streamer::new(host, config());
    let job = s.control();
    let lines = gcode(3);
    let handle = thread::spawn(move || {
        let out = s.run(&lines, |_| {}).unwrap();
        (s, out)
    });
    thread::sleep(Duration::from_millis(60));
    job.cancel();
    let (_, out) = handle.join().unwrap();
    assert_eq!(out, Outcome::Cancelled);
}

// ---------------------------------------------------------------------------
// Creality: ERROR:NN durante el envío
// ---------------------------------------------------------------------------

/// Máquina que responde `ok` a dos líneas y luego `fault` en lugar del `ok`; después sigue
/// contestando `ok` a lo que ya tenía en el búfer.
fn faulty(fault: &'static str) -> (Device, MemoryTransport, Arc<AtomicBool>) {
    let hold0_seen = Arc::new(AtomicBool::new(false));
    let reset_after_hold0 = Arc::new(AtomicBool::new(false));
    let (flag_h, flag_r) = (hold0_seen.clone(), reset_after_hold0.clone());
    let (mut n, mut held, mut polls) = (0usize, false, 0usize);
    let (dev, host) = device(move |inp| match inp {
        Input::Line(_) => {
            n += 1;
            if n == 3 { say(&[fault]) } else { say(&["ok"]) }
        }
        Input::Byte(b'!') => {
            held = true;
            Vec::new()
        }
        Input::Byte(b'?') if held => {
            polls += 1;
            if polls < 3 {
                status("Hold:1")
            } else {
                flag_h.store(true, Ordering::SeqCst);
                status("Hold:0")
            }
        }
        Input::Byte(b'?') => status("Run"),
        Input::Byte(0x18) => {
            flag_r.store(flag_h.load(Ordering::SeqCst), Ordering::SeqCst);
            held = false;
            say(&["Grbl 1.1f ['$' for help]"])
        }
        Input::Byte(_) => Vec::new(),
    });
    (dev, host, reset_after_hold0)
}

#[test]
fn creality_errors_stop_the_job_safely_and_explain_themselves() {
    for (line, code, words) in [
        ("ERROR:05", 5, "llama"),
        ("ERROR:05.", 5, "llama"),
        ("ERROR:01", 1, "inclinación"),
        ("ERROR:06.", 6, "lente"),
        ("ERROR:24", 24, "tapa"),
        ("ERROR:04", 4, "aire"),
    ] {
        let (dev, host, reset_after_hold0) = faulty(line);
        let (out, events) = run(host, config(), &gcode(10));
        assert_eq!(out, Outcome::MachineFault(code), "{line}");
        // Se avisa con su significado, no como un `error:N` de GRBL.
        assert!(
            messages(&events).iter().any(|m| m.contains(words)),
            "{line}: {:?}",
            messages(&events)
        );
        assert!(
            !events.iter().any(|e| matches!(e, Event::Error { .. })),
            "{line}"
        );
        // Parada segura: feed hold, y el reset solo después de ver `Hold:0`.
        assert_eq!(dev.rt(), [b'!', 0x18], "{line}");
        assert!(reset_after_hold0.load(Ordering::SeqCst), "{line}");
        assert_eq!(events.last(), Some(&Event::Finished(out)));
    }
}

#[test]
fn grbl_lowercase_error_is_not_confused_with_creality_errors() {
    // `error:5` (GRBL: ajuste desactivado) y `ERROR:05` (Creality: llama) son distintos.
    let (_dev, host, _) = faulty("error:5");
    let (out, events) = run(host, config(), &gcode(10));
    assert!(matches!(out, Outcome::LineError { code: 5, .. }), "{out:?}");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Error { code: 5, .. }))
    );
}

#[test]
fn a_creality_fault_without_status_replies_still_resets_after_the_timeout() {
    // La máquina en falla no contesta `?`: se resetea de todos modos tras el tope.
    let (dev, host) = device(|inp| match inp {
        Input::Line(_) => say(&["ERROR:05"]),
        Input::Byte(0x18) => say(&["Grbl 1.1f ['$' for help]"]),
        Input::Byte(_) => Vec::new(),
    });
    let t0 = Instant::now();
    let mut s = Streamer::new(host, config());
    let out = s.run(&gcode(3), |_| {}).unwrap();
    assert_eq!(out, Outcome::MachineFault(5));
    assert!(t0.elapsed() >= Duration::from_millis(400));
    assert_eq!(dev.rt(), [b'!', 0x18]);
    // No se vio quieta: la posición puede haberse perdido.
    assert!(s.last_stop().unwrap().position_maybe_lost);
}

// ---------------------------------------------------------------------------
// error:N con la política Stop detiene la máquina de verdad
// ---------------------------------------------------------------------------

#[test]
fn error_with_stop_policy_holds_waits_and_resets_even_if_grblhal_repeats_it() {
    // E-1 + E-11: `error:20` en la tercera línea y grblHAL lo repite en las siguientes.
    let hold0_seen = Arc::new(AtomicBool::new(false));
    let reset_after_hold0 = Arc::new(AtomicBool::new(false));
    let (flag_h, flag_r) = (hold0_seen.clone(), reset_after_hold0.clone());
    let (mut n, mut held, mut polls) = (0usize, false, 0usize);
    let (dev, host) = device(move |inp| match inp {
        Input::Line(_) => {
            n += 1;
            say(&[if n < 3 { "ok" } else { "error:20" }])
        }
        Input::Byte(b'!') => {
            held = true;
            Vec::new()
        }
        Input::Byte(b'?') if held => {
            polls += 1;
            // Mientras frena sigue repitiendo el error de grblHAL.
            let mut v = say(&["error:20"]);
            if polls < 4 {
                v.extend(status("Hold:1"));
            } else {
                flag_h.store(true, Ordering::SeqCst);
                v.extend(status("Hold:0"));
            }
            v
        }
        Input::Byte(b'?') => status("Run"),
        Input::Byte(0x18) => {
            flag_r.store(flag_h.load(Ordering::SeqCst), Ordering::SeqCst);
            held = false;
            say(&["Grbl 1.1h ['$' for help]"])
        }
        Input::Byte(_) => Vec::new(),
    });
    let cfg = StreamerConfig {
        error_policy: ErrorPolicy::Stop,
        ..config()
    };
    let mut s = Streamer::new(host, cfg);
    let mut events = Vec::new();
    let out = s.run(&gcode(12), |e| events.push(e)).unwrap();
    assert_eq!(
        out,
        Outcome::LineError {
            code: 20,
            source_line: 3
        }
    );
    // Los errores repetidos son el mismo fallo: un solo `Event::Error`.
    let errors = events
        .iter()
        .filter(|e| matches!(e, Event::Error { .. }))
        .count();
    assert_eq!(errors, 1, "{events:?}");
    assert_eq!(dev.rt(), [b'!', 0x18]);
    assert!(reset_after_hold0.load(Ordering::SeqCst));
    // Se vio quieta antes del reset: no se avisa de pérdida de posición.
    let stop = s.last_stop().unwrap();
    assert!(stop.settled && !stop.position_maybe_lost, "{stop:?}");
    assert!(
        !messages(&events).iter().any(|m| m.contains("posición")),
        "{:?}",
        messages(&events)
    );
}

#[test]
fn error_with_continue_policy_does_not_stop_the_machine() {
    let mut n = 0usize;
    let (dev, host) = device(move |inp| match inp {
        Input::Line(_) => {
            n += 1;
            say(&[if n == 2 { "error:20" } else { "ok" }])
        }
        Input::Byte(b'?') => status("Idle"),
        Input::Byte(_) => Vec::new(),
    });
    let cfg = StreamerConfig {
        error_policy: ErrorPolicy::Continue,
        ..config()
    };
    let (out, _) = run(host, cfg, &gcode(5));
    assert_eq!(out, Outcome::Completed { errors: 1 });
    assert!(dev.rt().is_empty());
}

// ---------------------------------------------------------------------------
// Cancelación segura (Q-2, Q-3, E-8, E-9)
// ---------------------------------------------------------------------------

/// Máquina con un trabajo "ejecutándose" (no contesta `ok`) que, tras `!`, tarda `slow` en
/// informar `Hold:0` (o nunca, si `slow` es `None`). Al recibir 0x18 responde `reset_reply`.
fn cancellable(
    slow: Option<usize>,
    reset_reply: &'static [&'static str],
) -> (Device, MemoryTransport, Arc<AtomicBool>) {
    let reset_in_motion = Arc::new(AtomicBool::new(false));
    let flag = reset_in_motion.clone();
    let (mut held, mut polls) = (false, 0usize);
    let (dev, host) = device(move |inp| match inp {
        Input::Line(_) => Vec::new(),
        Input::Byte(b'!') => {
            held = true;
            Vec::new()
        }
        Input::Byte(b'?') if held => {
            polls += 1;
            match slow {
                Some(k) if polls > k => status("Hold:0"),
                _ => status("Hold:1"),
            }
        }
        Input::Byte(b'?') => status("Run"),
        Input::Byte(0x18) => {
            // ¿El reset llegó con la máquina aún frenando (o corriendo)?
            flag.store(!held || slow.is_none_or(|k| polls <= k), Ordering::SeqCst);
            held = false;
            say(reset_reply)
        }
        Input::Byte(_) => Vec::new(),
    });
    (dev, host, reset_in_motion)
}

fn cancel_after(
    mut s: Streamer<MemoryTransport>,
    lines: Vec<String>,
) -> (Streamer<MemoryTransport>, Outcome, Vec<Event>) {
    let control = s.control();
    let handle = thread::spawn(move || {
        let mut events = Vec::new();
        let out = s.run(&lines, |e| events.push(e)).unwrap();
        (s, out, events)
    });
    thread::sleep(Duration::from_millis(60));
    control.cancel();
    handle.join().unwrap()
}

#[test]
fn cancel_waits_for_hold0_before_resetting_so_there_is_no_alarm3() {
    // Q-2: `!`, el hold tarda varias consultas en llegar a Hold:0, y solo entonces 0x18.
    let (dev, host, reset_in_motion) = cancellable(Some(5), &["Grbl 1.1h ['$' for help]"]);
    let (s, out, events) = cancel_after(Streamer::new(host, config()), gcode(4));
    assert_eq!(out, Outcome::Cancelled);
    assert_eq!(dev.rt(), [b'!', 0x18]);
    assert!(!reset_in_motion.load(Ordering::SeqCst));
    let stop = s.last_stop().unwrap();
    assert!(stop.settled && !stop.position_maybe_lost && stop.alarm.is_none());
    // El saludo se registra en lugar de descartarse.
    assert!(messages(&events).iter().any(|m| m.starts_with("Grbl 1.1h")));
    // Y no se reporta como un reinicio inesperado.
    assert!(!events.iter().any(|e| matches!(e, Event::Reset(_))));
}

#[test]
fn cancel_resets_anyway_after_the_timeout_and_warns_about_the_position() {
    // El hold nunca llega a Hold:0 (Q-3): se resetea de todos modos y se avisa.
    let t0 = Instant::now();
    let (dev, host, reset_in_motion) = cancellable(None, &["ALARM:3", "Grbl 1.1h ['$' for help]"]);
    let (s, out, events) = cancel_after(Streamer::new(host, config()), gcode(4));
    assert_eq!(out, Outcome::Cancelled);
    assert!(t0.elapsed() >= Duration::from_millis(400));
    assert_eq!(dev.rt(), [b'!', 0x18]);
    assert!(reset_in_motion.load(Ordering::SeqCst));
    let stop = s.last_stop().unwrap();
    assert!(!stop.settled && stop.position_maybe_lost);
    assert_eq!(stop.alarm, Some(3));
    assert!(events.contains(&Event::Alarm(3)));
    assert!(
        messages(&events).iter().any(|m| m.contains("posición")),
        "{:?}",
        messages(&events)
    );
}

#[test]
fn alarm3_after_a_reset_marks_the_position_as_lost_even_if_hold0_was_seen() {
    // E-8: el orden de ALARM:3 y el saludo no está documentado; aquí llega primero la alarma.
    let (_dev, host, _) = cancellable(Some(1), &["ALARM:3", "Grbl 1.1f ['$' for help]"]);
    let (s, out, _) = cancel_after(Streamer::new(host, config()), gcode(4));
    assert_eq!(out, Outcome::Cancelled);
    let stop = s.last_stop().unwrap();
    assert!(stop.settled && stop.position_maybe_lost);
    assert_eq!(stop.alarm, Some(3));
}

#[test]
fn a_user_reset_during_motion_reports_that_the_position_may_be_lost() {
    let (_dev, host, _) = cancellable(Some(1), &["Grbl 1.1h ['$' for help]"]);
    let mut s = Streamer::new(host, config());
    let control = s.control();
    let lines = gcode(4);
    let handle = thread::spawn(move || {
        let mut events = Vec::new();
        let out = s.run(&lines, |e| events.push(e)).unwrap();
        (s, out, events)
    });
    // El sondeo ya vio `Run`.
    thread::sleep(Duration::from_millis(80));
    control.realtime(Realtime::SoftReset);
    let (s, out, events) = handle.join().unwrap();
    assert_eq!(out, Outcome::Cancelled);
    assert!(s.last_stop().unwrap().position_maybe_lost);
    assert!(messages(&events).iter().any(|m| m.contains("posición")));
}

// ---------------------------------------------------------------------------
// Alarmas y reinicios durante el envío
// ---------------------------------------------------------------------------

#[test]
fn alarm3_followed_by_the_falcon_banner_is_an_alarm_not_a_reset() {
    // Falcon2: tras ALARM:3 la placa se reinicia e imprime su banner completo.
    let mut n = 0usize;
    let (_dev, host) = device(move |inp| match inp {
        Input::Line(_) => {
            n += 1;
            if n == 4 {
                say(&[
                    "ALARM:3",
                    "usb connect succeed",
                    "[OK]system start-up CV50-Pro-MASTER-Release V1.0.1 20240325-1042",
                    "[MSG:Using cpu_map…esp32-s3]",
                    "Grbl 1.1f ['$' for help]",
                    "[OK] ILmp Link connect ok",
                ])
            } else {
                say(&["ok"])
            }
        }
        Input::Byte(b'?') => status("Run"),
        Input::Byte(_) => Vec::new(),
    });
    let (out, events) = run(host, config(), &gcode(10));
    assert_eq!(out, Outcome::Alarm(3));
    assert!(events.contains(&Event::Alarm(3)));
}

#[test]
fn a_banner_without_alarm_in_the_middle_of_a_job_is_a_reset() {
    // Q-4.
    let mut n = 0usize;
    let (_dev, host) = device(move |inp| match inp {
        Input::Line(_) => {
            n += 1;
            if n == 3 {
                say(&["Grbl 1.1h ['$' for help]"])
            } else {
                say(&["ok"])
            }
        }
        Input::Byte(b'?') => status("Run"),
        Input::Byte(_) => Vec::new(),
    });
    let (out, _) = run(host, config(), &gcode(10));
    assert_eq!(out, Outcome::Reset("Grbl 1.1h ['$' for help]".into()));
}

// ---------------------------------------------------------------------------
// Silencio: `$H` y "conexión perdida"
// ---------------------------------------------------------------------------

/// Máquina muda (ni `?` ni `ok`) durante `quiet` tras recibir la línea `trigger`; luego
/// contesta `ok` y vuelve a la normalidad.
fn mute_after(trigger: &'static str, quiet: Duration) -> (Device, MemoryTransport) {
    let mut until: Option<Instant> = None;
    let mut owed = 0usize;
    device(move |inp| {
        if let Some(t) = until {
            if Instant::now() < t {
                if let Input::Line(_) = inp {
                    owed += 1;
                }
                return Vec::new();
            }
            until = None;
            // Un `ok` por la línea que provocó el silencio, las que llegaron mientras tanto y,
            // si lo que llega ahora es una línea, esa también.
            let mut oks = std::mem::take(&mut owed) + 1;
            let mut v = Vec::new();
            match inp {
                Input::Line(_) => oks += 1,
                Input::Byte(b'?') => v.extend(status("Idle")),
                Input::Byte(_) => {}
            }
            let mut replies = say(&vec!["ok"; oks]);
            replies.extend(v);
            return replies;
        }
        match inp {
            Input::Line(l) if l == trigger => {
                until = Some(Instant::now() + quiet);
                Vec::new()
            }
            Input::Line(_) => say(&["ok"]),
            Input::Byte(b'?') => status("Idle"),
            Input::Byte(_) => Vec::new(),
        }
    })
}

#[test]
fn a_long_silence_is_never_a_lost_connection_and_only_warns_outside_homing() {
    let cfg = StreamerConfig {
        silence_notice: Some(Duration::from_millis(100)),
        ..config()
    };
    let silences = |events: &[Event]| {
        events
            .iter()
            .filter(|e| matches!(e, Event::Silence { .. }))
            .count()
    };
    // En el homing el silencio es lo esperado: el trabajo termina y no se avisa.
    let (_dev, host) = mute_after("$H", Duration::from_millis(500));
    let (out, events) = run(host, cfg.clone(), &["$H".to_string(), "G0 X1".to_string()]);
    assert_eq!(out, Outcome::Completed { errors: 0 });
    assert_eq!(silences(&events), 0, "{events:?}");

    // El mismo silencio sin homing tampoco da la conexión por perdida (se espera, como
    // LightBurn): el trabajo termina y la interfaz recibe el aviso y su retirada.
    let (_dev, host) = mute_after("G0X5", Duration::from_millis(500));
    let (out, events) = run(host, cfg, &["G0 X5".to_string(), "G0 X1".to_string()]);
    assert_eq!(out, Outcome::Completed { errors: 0 });
    assert!(silences(&events) >= 2, "{events:?}");
    assert_eq!(
        events.iter().rev().find_map(|e| match e {
            Event::Silence { ms } => Some(*ms),
            _ => None,
        }),
        Some(0)
    );
}

// ---------------------------------------------------------------------------
// Líneas que escriben en la EEPROM: se envían solas y se espera su `ok`
// ---------------------------------------------------------------------------

#[test]
fn eeprom_lines_are_sent_alone_and_acknowledged_before_the_next_line() {
    // La máquina responde con retraso (un `ok` por cada `?`) y vigila que, mientras una línea
    // que escribe en la EEPROM está sin responder, no llegue nada más.
    let queue = Arc::new(Mutex::new(Vec::<String>::new()));
    let violations = Arc::new(AtomicUsize::new(0));
    let (q, v) = (queue.clone(), violations.clone());
    let mut pending_sync = false;
    let (_dev, host) = device(move |inp| match inp {
        Input::Line(l) => {
            let sync = l.starts_with("G10") || l.starts_with("$100=") || l.starts_with("G28.1");
            let mut queue = q.lock().unwrap();
            if pending_sync {
                // Llegó otra línea con una de EEPROM aún sin `ok`.
                v.fetch_add(1, Ordering::SeqCst);
            }
            if sync {
                // Todo lo anterior debe estar respondido ya.
                if !queue.is_empty() {
                    v.fetch_add(1, Ordering::SeqCst);
                }
                pending_sync = true;
            }
            queue.push(l.to_string());
            Vec::new()
        }
        Input::Byte(b'?') => {
            let mut queue = q.lock().unwrap();
            let mut out = Vec::new();
            if !queue.is_empty() {
                let l = queue.remove(0);
                if l.starts_with("G10") || l.starts_with("$100=") || l.starts_with("G28.1") {
                    pending_sync = false;
                }
                out.extend(say(&["ok"]));
            }
            out.extend(status(if queue.is_empty() { "Idle" } else { "Run" }));
            out
        }
        Input::Byte(_) => Vec::new(),
    });
    let lines = [
        "G90",
        "G21",
        "G10 L2 P1 X0 Y0",
        "G1 X1 F100",
        "G1 X2",
        "$100=80",
        "G1 X3",
        "G28.1",
        "G1 X4",
    ]
    .map(String::from);
    let (out, _) = run(host, config(), &lines);
    assert_eq!(out, Outcome::Completed { errors: 0 });
    assert_eq!(violations.load(Ordering::SeqCst), 0);
}

// ---------------------------------------------------------------------------
// Informes de estado reales (Falcon2 y grblHAL)
// ---------------------------------------------------------------------------

#[test]
fn real_status_reports_parse_without_panicking() {
    // Falcon2 Pro (falcon2.md, sección 3).
    let r = StatusReport::parse(
        "<Idle|MPos:0.000,0.000,0.000|Bf:127,65535|FS:0,0|Pn:XY|WCO:0.000,0.000,0.000>",
    )
    .unwrap();
    assert_eq!(r.state, MachineState::Idle);
    assert_eq!(r.buffer.unwrap().rx_bytes, 65535);
    assert_eq!(r.buffer.unwrap().planner_blocks, 127);
    assert_eq!(r.pins.as_deref(), Some("XY"));
    let r = StatusReport::parse(
        "<Home|MPos:-0.363,-0.363,0.000|Bf:127,65535|FS:5657,0|WCO:0.000,0.000,0.000>",
    )
    .unwrap();
    assert_eq!(r.state, MachineState::Home);
    assert_eq!(r.mpos.unwrap().x, -0.363);
    assert_eq!(r.feed, Some(5657.0));
    let r = StatusReport::parse("<Run|MPos:17.225,17.862,0.000|Bf:126,65498|FS:3000,0>").unwrap();
    assert_eq!(r.state, MachineState::Run);
    assert_eq!(r.buffer.unwrap().rx_bytes, 65498);
    // Un WCO con nan (G54 dañado) se ignora, no llega como posición.
    let r = StatusReport::parse("<Idle|MPos:0.000,0.000,0.000|WCO:-nan,1.579,1.619>").unwrap();
    assert_eq!(r.wco, None);
    // grblHAL: estado Tool y FS de tres valores.
    let r =
        StatusReport::parse("<Tool|MPos:0.000,0.000,0.000|FS:0,0,0|WCS:G54|TLR:0|H:1>").unwrap();
    assert_eq!(r.state, MachineState::Other);
    let r = StatusReport::parse("<Run|MPos:1.000,1.000,0.000|FS:1500,255,240>").unwrap();
    assert_eq!((r.feed, r.spindle), (Some(1500.0), Some(255.0)));
}
