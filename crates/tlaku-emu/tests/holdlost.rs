//! Placa que deja de contestar a mitad de un trabajo (`docs/validacion-hardware.md` §2.8).
//!
//! Regla (igual que LightBurn, que simplemente espera): **el silencio nunca da la conexión por
//! perdida**. Declararla perdida no detiene nada (con la placa trabada no se le puede mandar
//! nada) y la Falcon2 se para 2–20 s y sigue sola. Solo la pierde un error real de E/S del
//! sistema o que el usuario cancele o desconecte. Lo que sí hay:
//!
//! 1. Un aviso temprano (`Event::Silence`) desde los 2 s, que se retira al volver la respuesta.
//! 2. Ningún `!` automático por silencio: solo el usuario detiene (feed hold y reset, como
//!    siempre).
//! 3. Detener y desconectar no se bloquean nunca, ni con una escritura colgada para siempre
//!    (`Transport::set_abort`).

mod common;

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::*;
use tlaku_emu::{Fault, Profile, Trigger, VirtualMachine};
use tlaku_grbl::serial::Transport;
use tlaku_grbl::streamer::{Event, Job, Outcome, Streamer, StreamerConfig};

const DEADLINE: Duration = Duration::from_secs(20);

/// Un corte largo y lento (X 100 mm a F600 = 10 s) con el láser encendido.
fn long_burn() -> Vec<String> {
    gl(&["G90", "G21", "M4 S0", "G1 X100 S500 F600", "M5", "G0 X0"])
}

/// Espera hasta `limit` a que llegue `Finished`; el resultado, si llegó.
fn finished_within<T: Transport>(job: &Job<T>, limit: Duration) -> Option<Outcome> {
    let end = Instant::now() + limit;
    while Instant::now() < end {
        if let Ok(Event::Finished(o)) = job.events.recv_timeout(Duration::from_millis(20)) {
            return Some(o);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Placa muda para siempre
// ---------------------------------------------------------------------------

#[test]
fn a_board_that_never_answers_keeps_the_job_waiting_and_only_the_user_stops_it() {
    // Tiempo real (x1). La placa recibe pero no dice nada, desde los 300 ms y por una hora.
    let (vm, h) = scaled_vm(Profile::falcon2_pro(), 1.0, |b| b.homed(true));
    h.inject(
        Trigger::At(Duration::from_millis(300)),
        Fault::Silence {
            duration: Duration::from_secs(3600),
            drop_output: true,
        },
    );
    let cfg = StreamerConfig {
        silence_notice: Some(Duration::from_millis(400)),
        hold_timeout: Duration::from_millis(300),
        ..fast_config()
    };
    let job = Streamer::new(vm, cfg).spawn(&long_burn()).unwrap();
    // Pasa más que el plazo de antes a esta escala: sigue esperando, con el aviso.
    let t = Instant::now();
    let (mut warned, mut last) = (false, 0);
    while t.elapsed() < Duration::from_millis(1800) {
        match job.events.recv_timeout(Duration::from_millis(20)) {
            Ok(Event::Finished(o)) => panic!("terminó solo: {o:?}"),
            Ok(Event::Silence { ms }) => {
                warned |= ms > 0;
                last = ms;
            }
            _ => {}
        }
    }
    assert!(warned && last >= 1000, "{warned} {last}");
    // Ningún `!` ni reset por su cuenta.
    assert!(
        h.realtime_bytes().iter().all(|&b| b == b'?'),
        "{:?}",
        h.realtime_bytes()
    );
    // El usuario detiene: feed hold y reset de siempre, al instante.
    let asked = Instant::now();
    job.control.cancel();
    assert_eq!(
        finished_within(&job, Duration::from_secs(5)),
        Some(Outcome::Cancelled)
    );
    assert!(
        asked.elapsed() < Duration::from_secs(3),
        "{:?}",
        asked.elapsed()
    );
    let rt = h.realtime_bytes();
    assert!(rt.contains(&b'!') && rt.contains(&0x18), "{rt:?}");
    let _ = job.join();
}

// ---------------------------------------------------------------------------
// Detener sin esperar una escritura colgada
// ---------------------------------------------------------------------------

/// Transporte cuya escritura de datos se queda esperando (la placa no lee) **indefinidamente**
/// hasta que se active el indicador de «abandonar», como el `SerialTransport` real.
struct BlockedWrites {
    inner: VirtualMachine,
    abort: Option<Arc<AtomicBool>>,
    blocked_since: Arc<Mutex<Option<Instant>>>,
}

impl Transport for BlockedWrites {
    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        // Los `?` (un byte) pasan; las líneas se quedan esperando.
        if data == b"?" {
            return self.inner.write_all(data);
        }
        self.blocked_since
            .lock()
            .unwrap()
            .get_or_insert_with(Instant::now);
        loop {
            if self
                .abort
                .as_ref()
                .is_some_and(|a| a.load(Ordering::SeqCst))
            {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "escritura abandonada",
                ));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        self.inner.read_timeout(buf, timeout)
    }

    fn set_abort(&mut self, abort: Arc<AtomicBool>) {
        self.abort = Some(abort);
    }
}

#[test]
fn a_write_stuck_forever_is_not_a_disconnect_and_cancel_never_waits_for_it() {
    let (vm, _h) = scaled_vm(Profile::falcon2_pro(), 1.0, |b| b.homed(true));
    let blocked_since = Arc::new(Mutex::new(None));
    let transport = BlockedWrites {
        inner: vm,
        abort: None,
        blocked_since: blocked_since.clone(),
    };
    let job = Streamer::new(transport, fast_config())
        .spawn(&long_burn())
        .unwrap();
    assert!(wait_for(Duration::from_secs(5), || blocked_since
        .lock()
        .unwrap()
        .is_some()));
    // No se rinde por su cuenta (antes, a los 30 s de escritura sin avanzar).
    assert_eq!(finished_within(&job, Duration::from_millis(1500)), None);
    let asked = Instant::now();
    job.control.cancel();
    // Cancelado (no «desconectado»): se intentó la parada de siempre y, al no avanzar el puerto,
    // se abandonó enseguida.
    assert_eq!(
        finished_within(&job, Duration::from_secs(10)),
        Some(Outcome::Cancelled)
    );
    let took = asked.elapsed();
    assert!(took < Duration::from_secs(4), "tardó {took:?} en cancelar");
    job.join().expect("sin pánico");
}

// ---------------------------------------------------------------------------
// Aviso temprano de silencio
// ---------------------------------------------------------------------------

fn silence_events(events: &[Event]) -> Vec<u64> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Silence { ms } => Some(*ms),
            _ => None,
        })
        .collect()
}

#[test]
fn the_early_silence_notice_comes_first_and_clears_when_the_board_answers() {
    let (vm, h) = scaled_vm(Profile::falcon2_pro(), 1.0, |b| b.homed(true));
    h.inject(
        Trigger::At(Duration::from_millis(300)),
        Fault::Silence {
            duration: Duration::from_millis(1200),
            // Lo dicho llega de golpe al terminar (como un búfer USB que se vacía).
            drop_output: false,
        },
    );
    let cfg = StreamerConfig {
        silence_notice: Some(Duration::from_millis(400)),
        ..fast_config()
    };
    let lines = gl(&["G90", "G21", "M4 S0", "G1 X6 S500 F600", "M5", "G0 X0"]);
    let run = run_job(vm, cfg, &lines, DEADLINE);
    assert_eq!(run.outcome, Some(Outcome::Completed { errors: 0 }));
    let ms = silence_events(&run.events);
    assert!(ms.len() >= 2, "{ms:?}");
    assert!((400..2000).contains(&ms[0]), "{ms:?}");
    // Al volver a contestar, el aviso se retira (y solo una vez).
    assert_eq!(ms.last(), Some(&0), "{ms:?}");
    assert_eq!(ms.iter().filter(|&&m| m == 0).count(), 1, "{ms:?}");
    // Ni `!` ni reset: la placa siguió sola.
    assert!(!h.realtime_bytes().contains(&b'!'));
}

#[test]
fn a_healthy_job_never_shows_the_silence_notice() {
    let (vm, _h) = scaled_vm(Profile::falcon2_pro(), 100.0, |b| b.homed(true));
    let run = run_job(
        vm,
        fast_config(),
        &square_job(5.0, 5.0, 10.0, 3000, 500),
        DEADLINE,
    );
    assert_eq!(run.outcome, Some(Outcome::Completed { errors: 0 }));
    assert!(silence_events(&run.events).is_empty());
}
