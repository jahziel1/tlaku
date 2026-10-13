//! Utilidades de las pruebas: una "plataforma" con reloj manual para hablar con la
//! controladora emulada byte a byte y de forma determinista.
#![allow(dead_code)]

use std::time::Duration;

use tlaku_emu::{Builder, Handle, ManualClock, Profile, VirtualMachine};
use tlaku_grbl::serial::Transport;

pub struct Rig {
    pub vm: VirtualMachine,
    pub h: Handle,
    pub clock: ManualClock,
}

impl Rig {
    pub fn new(profile: Profile) -> Self {
        Self::with(profile, |b| b)
    }

    /// Como `new`, personalizando el constructor.
    pub fn with(profile: Profile, f: impl FnOnce(Builder) -> Builder) -> Self {
        let clock = ManualClock::new();
        let (vm, h) = f(VirtualMachine::builder(profile).clock(clock.clone())).build();
        Self { vm, h, clock }
    }

    /// Envía texto (sin añadir terminador).
    pub fn send(&mut self, s: &str) {
        self.vm.write_all(s.as_bytes()).unwrap();
    }

    /// Envía una línea con `\n`.
    pub fn line(&mut self, s: &str) {
        self.send(&format!("{s}\n"));
    }

    /// Envía un byte en tiempo real.
    pub fn rt(&mut self, b: u8) {
        self.vm.write_all(&[b]).unwrap();
    }

    /// Deja pasar tiempo simulado.
    pub fn ms(&mut self, n: u64) {
        self.clock.advance(Duration::from_millis(n));
    }

    pub fn secs(&mut self, n: u64) {
        self.clock.advance(Duration::from_secs(n));
    }

    /// Lo que haya pendiente de leer, en bruto.
    pub fn raw(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            match self.vm.read_timeout(&mut buf, Duration::ZERO) {
                Ok(0) | Err(_) => break,
                Ok(n) => out.extend_from_slice(&buf[..n]),
            }
        }
        out
    }

    /// Lo pendiente, como líneas no vacías.
    pub fn lines(&mut self) -> Vec<String> {
        String::from_utf8_lossy(&self.raw())
            .split(['\r', '\n'])
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect()
    }

    /// Descarta lo pendiente (p. ej. el banner).
    pub fn drain(&mut self) {
        let _ = self.raw();
    }

    /// Envía una línea, deja pasar 5 ms y devuelve la respuesta.
    pub fn ask(&mut self, cmd: &str) -> Vec<String> {
        self.line(cmd);
        self.ms(5);
        self.lines()
    }

    /// Envía `?` y devuelve el reporte.
    pub fn status(&mut self) -> String {
        self.rt(b'?');
        self.lines()
            .into_iter()
            .find(|l| l.starts_with('<'))
            .expect("la controladora no contestó a ?")
    }

    /// Hace pasar tiempo hasta que la máquina quede en reposo.
    pub fn settle(&mut self) {
        assert!(
            self.h.wait_idle(Duration::from_secs(600)),
            "la máquina no volvió a Idle: {}",
            self.h.status_line()
        );
    }
}

// ---------------------------------------------------------------------------
// Utilidades para las pruebas con el `Streamer` real
// ---------------------------------------------------------------------------

use std::time::Instant;

use tlaku_emu::ScaledClock;
use tlaku_grbl::streamer::{Control, Event, Outcome, Streamer, StreamerConfig};

/// Controladora con reloj escalado (`speed` segundos simulados por segundo real), banner ya
/// leído y descartado.
pub fn scaled_vm(
    profile: Profile,
    speed: f64,
    f: impl FnOnce(Builder) -> Builder,
) -> (VirtualMachine, Handle) {
    let (mut vm, h) = f(VirtualMachine::builder(profile).clock(ScaledClock::new(speed))).build();
    vm.clear_input().unwrap();
    (vm, h)
}

/// Configuración del streamer con sondeo rápido para que las pruebas duren poco.
pub fn fast_config() -> StreamerConfig {
    StreamerConfig {
        status_interval: Some(Duration::from_millis(5)),
        ..StreamerConfig::default()
    }
}

/// Espera (en tiempo real) a que `f` sea verdadero; `false` si vence el plazo.
pub fn wait_for(timeout: Duration, mut f: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    loop {
        if f() {
            return true;
        }
        if start.elapsed() > timeout {
            return false;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Resultado de un trabajo lanzado con [`run_job`].
pub struct JobRun {
    pub events: Vec<Event>,
    /// `None` si el trabajo no llegó a terminar (pánico o plazo vencido).
    pub outcome: Option<Outcome>,
    /// Venció el plazo y hubo que cancelar.
    pub timed_out: bool,
    /// El hilo del streamer entró en pánico.
    pub panicked: bool,
    pub streamer: Option<Streamer<VirtualMachine>>,
}

impl JobRun {
    pub fn max_buffer_used(&self) -> usize {
        self.events
            .iter()
            .filter_map(|e| match e {
                Event::Progress { buffer_used, .. } => Some(*buffer_used),
                _ => None,
            })
            .max()
            .unwrap_or(0)
    }
}

/// Lanza el trabajo en el hilo del streamer y espera el fin hasta `deadline` (tiempo real).
/// Si vence, cancela y lo anota en `timed_out`. `during` se llama en cada vuelta con el mando.
pub fn run_job_with(
    vm: VirtualMachine,
    cfg: StreamerConfig,
    lines: &[String],
    deadline: Duration,
    mut during: impl FnMut(&Control),
) -> JobRun {
    let streamer = Streamer::new(vm, cfg);
    let job = streamer.spawn(lines).expect("líneas válidas");
    let control = job.control.clone();
    let start = Instant::now();
    let mut events = Vec::new();
    let mut timed_out = false;
    let mut cancelled_at: Option<Instant> = None;
    loop {
        match job.events.recv_timeout(Duration::from_millis(2)) {
            Ok(ev) => {
                let done = matches!(ev, Event::Finished(_));
                events.push(ev);
                if done {
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                during(&control);
                if start.elapsed() > deadline && !timed_out {
                    timed_out = true;
                    control.cancel();
                    cancelled_at = Some(Instant::now());
                }
                if cancelled_at.is_some_and(|t| t.elapsed() > Duration::from_secs(3)) {
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let (streamer, panicked) = match job.join() {
        Ok((s, _)) => (Some(s), false),
        Err(_) => (None, true),
    };
    let outcome = events.iter().rev().find_map(|e| match e {
        Event::Finished(o) => Some(o.clone()),
        _ => None,
    });
    JobRun {
        events,
        outcome,
        timed_out,
        panicked,
        streamer,
    }
}

pub fn run_job(
    vm: VirtualMachine,
    cfg: StreamerConfig,
    lines: &[String],
    deadline: Duration,
) -> JobRun {
    run_job_with(vm, cfg, lines, deadline, |_| {})
}

/// Líneas de G-code a partir de literales.
pub fn gl(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|s| (*s).to_string()).collect()
}

/// Cuadrado de `side` mm con la esquina en (x0, y0), cortado con M4 y `F`/`S` dados.
pub fn square_job(x0: f64, y0: f64, side: f64, feed: u32, s: u32) -> Vec<String> {
    let (x1, y1) = (x0 + side, y0 + side);
    vec![
        "G90".into(),
        "G21".into(),
        "M4 S0".into(),
        format!("G0 X{x0} Y{y0}"),
        format!("G1 X{x1} S{s} F{feed}"),
        format!("G1 Y{y1}"),
        format!("G1 X{x0}"),
        format!("G1 Y{y0}"),
        "M5".into(),
        "G0 X0 Y0".into(),
    ]
}
