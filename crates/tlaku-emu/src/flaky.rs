//! Un enlace «caprichoso» para pruebas de resistencia: envuelve una [`VirtualMachine`] (o cualquier
//! [`Transport`]) y le suma, con una semilla fija, las fallas del USB de una Creality Falcon2 Pro
//! real (`docs/hardware/falcon2.md` §10.4–10.6) que el emulador por sí solo no produce:
//!
//! - **Bloqueos de escritura**: la placa deja de leer el USB y el `write` del anfitrión se queda
//!   colgado de 2 a ~20 s (en el equipo real; aquí lo que se configure). Mientras dura, la placa
//!   está parada del todo: el reloj del emulador se congela con [`FreezeClock`].
//! - **Silencios de lectura**: la placa no dice nada un rato pero el anfitrión no está bloqueado
//!   (lo que dijo llega de golpe al terminar, como un búfer USB que se vacía).
//! - **Reportes de estado saltados**: el `?` se pierde (aislado o en ráfagas).
//! - **Lecturas fragmentadas**: el USB entrega los bytes en trozos de cualquier tamaño, también
//!   en mitad de una línea.
//!
//! El estado cortado pegado al `ok` ya lo inyecta el emulador (`Fault::TruncatedStatusBeforeReply`);
//! [`truncated_schedule`] reparte esas fallas con la misma semilla.
//!
//! No se usa fuera de las pruebas: el azar sale de un generador propio ([`Rng`]) para que la misma
//! semilla dé siempre las mismas fallas (el orden entre hilos sí depende del tiempo real).

use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tlaku_grbl::serial::Transport;

use crate::clock::Clock;

/// Generador pseudoaleatorio xorshift64* (sin dependencias; determinista por semilla).
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // Mezcla la semilla para que 0 o semillas vecinas no den secuencias parecidas.
        let mut s = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        s = (s ^ (s >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        s = (s ^ (s >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        s ^= s >> 31;
        Self(if s == 0 { 0x2545_F491_4F6C_DD1D } else { s })
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Número en `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Verdadero con probabilidad `p`.
    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    /// Entero en `lo..=hi`.
    pub fn range(&mut self, lo: usize, hi: usize) -> usize {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u64() % (hi - lo + 1) as u64) as usize
    }

    /// Duración uniforme en `[lo, hi]`.
    pub fn duration(&mut self, lo: Duration, hi: Duration) -> Duration {
        if hi <= lo {
            return lo;
        }
        lo + (hi - lo).mul_f64(self.unit())
    }
}

/// Números de línea (desde 1) en los que la respuesta llega pegada a un estado cortado: cada
/// línea tiene probabilidad `1 / every` (en la Falcon real se vio ~1 de cada 600).
pub fn truncated_schedule(seed: u64, lines: usize, every: usize) -> Vec<usize> {
    let mut rng = Rng::new(seed ^ 0x7255_4E43);
    let p = 1.0 / every.max(1) as f64;
    (1..=lines).filter(|_| rng.chance(p)).collect()
}

#[derive(Debug)]
struct ClockState {
    /// Tiempo simulado «perdido» en congelaciones ya terminadas.
    offset: Duration,
    /// Mientras está congelado: instante de pared del inicio y valor simulado fijado.
    frozen: Option<(Instant, Duration)>,
}

/// Reloj de pared multiplicado por `speed` que se puede congelar: mientras la placa «no lee el
/// USB» su tiempo no corre (no se mueve, no planifica). Al descongelar sigue donde se quedó.
#[derive(Clone)]
pub struct FreezeClock {
    start: Instant,
    speed: f64,
    state: Arc<Mutex<ClockState>>,
}

impl FreezeClock {
    pub fn new(speed: f64) -> Self {
        let speed = if speed.is_finite() && speed > 0.0 {
            speed
        } else {
            1.0
        };
        Self {
            start: Instant::now(),
            speed,
            state: Arc::new(Mutex::new(ClockState {
                offset: Duration::ZERO,
                frozen: None,
            })),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ClockState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn raw(&self) -> Duration {
        self.start.elapsed().mul_f64(self.speed)
    }

    /// Detiene el tiempo simulado (no hace nada si ya estaba detenido).
    pub fn freeze(&self) {
        let mut s = self.lock();
        if s.frozen.is_none() {
            let now = self.raw().saturating_sub(s.offset);
            s.frozen = Some((Instant::now(), now));
        }
    }

    /// Reanuda el tiempo simulado desde donde se detuvo.
    pub fn thaw(&self) {
        let mut s = self.lock();
        if let Some((since, _)) = s.frozen.take() {
            s.offset += since.elapsed().mul_f64(self.speed);
        }
    }
}

impl Clock for FreezeClock {
    fn now(&self) -> Duration {
        let s = self.lock();
        match s.frozen {
            Some((_, at)) => at,
            None => self.raw().saturating_sub(s.offset),
        }
    }

    fn pass(&self, d: Duration) {
        std::thread::sleep(d.div_f64(self.speed));
    }

    fn host_wait(&self, step: Duration) {
        std::thread::sleep(step);
    }
}

/// Qué fallas aplica el enlace y con qué frecuencia.
#[derive(Debug, Clone)]
pub struct FlakyConfig {
    pub seed: u64,
    /// Cada cuántas escrituras de datos (de media; entre la mitad y vez y media) la placa deja de
    /// leer; `0` = nunca.
    pub write_stall_every: usize,
    /// Duración (de pared) de cada bloqueo de escritura, uniforme en el rango.
    pub write_stall: (Duration, Duration),
    /// Plazo tras el cual una escritura colgada falla con `TimedOut`; `None` = nunca falla, solo
    /// tarda (como el `SerialTransport` real, que reintenta sin límite hasta un error del sistema
    /// o hasta que el usuario cancele).
    pub write_limit: Option<Duration>,
    /// Cada cuántas lecturas con datos (de media; entre la mitad y vez y media) la placa calla;
    /// `0` = nunca.
    pub read_mute_every: usize,
    /// Duración (de pared) de cada silencio de lectura.
    pub read_mute: (Duration, Duration),
    /// Bloqueo puntual: en la escritura número `n` (de datos, sin contar los `?`) la placa deja
    /// de leer durante `d`. Se suma a los aleatorios.
    pub scripted_write_stall: Option<(usize, Duration)>,
    /// Silencio puntual: tras la lectura con datos número `n` la placa calla durante `d`.
    pub scripted_read_mute: Option<(usize, Duration)>,
    /// Probabilidad de que un `?` suelto se pierda.
    pub skip_status: f64,
    /// Probabilidad de que, al perderse uno, se pierdan también los siguientes (ráfaga de 2 a 12).
    pub skip_burst: f64,
    /// Parte las lecturas en trozos de 1 a 16 bytes con esta probabilidad.
    pub fragment: f64,
}

impl Default for FlakyConfig {
    fn default() -> Self {
        Self {
            seed: 1,
            write_stall_every: 0,
            write_stall: (Duration::from_secs(2), Duration::from_secs(20)),
            write_limit: None,
            read_mute_every: 0,
            read_mute: (Duration::from_secs(1), Duration::from_secs(4)),
            scripted_write_stall: None,
            scripted_read_mute: None,
            skip_status: 0.0,
            skip_burst: 0.0,
            fragment: 0.0,
        }
    }
}

/// Cuenta de lo que el enlace inyectó (compartida: se puede leer mientras corre el trabajo).
#[derive(Debug)]
pub struct FlakyCounters {
    /// Mientras sea `false` el enlace no inyecta nada (para conectar e identificar la placa sin
    /// sobresaltos y armar las fallas al empezar el trabajo). Por omisión, armado.
    pub armed: AtomicBool,
    pub write_stalls: AtomicUsize,
    pub write_stall_nanos: AtomicU64,
    pub read_mutes: AtomicUsize,
    pub read_mute_nanos: AtomicU64,
    pub skipped_status: AtomicUsize,
    pub fragmented_reads: AtomicUsize,
    /// Escrituras de datos (sin contar los `?`) y lecturas con datos que pasaron por el enlace.
    pub data_writes: AtomicUsize,
    pub data_reads: AtomicUsize,
}

impl Default for FlakyCounters {
    fn default() -> Self {
        Self {
            armed: AtomicBool::new(true),
            write_stalls: AtomicUsize::default(),
            write_stall_nanos: AtomicU64::default(),
            read_mutes: AtomicUsize::default(),
            read_mute_nanos: AtomicU64::default(),
            skipped_status: AtomicUsize::default(),
            fragmented_reads: AtomicUsize::default(),
            data_writes: AtomicUsize::default(),
            data_reads: AtomicUsize::default(),
        }
    }
}

impl FlakyCounters {
    pub fn arm(&self, yes: bool) {
        self.armed.store(yes, Ordering::SeqCst);
    }

    pub fn write_stalls(&self) -> usize {
        self.write_stalls.load(Ordering::SeqCst)
    }

    pub fn write_stall_total(&self) -> Duration {
        Duration::from_nanos(self.write_stall_nanos.load(Ordering::SeqCst))
    }

    pub fn read_mutes(&self) -> usize {
        self.read_mutes.load(Ordering::SeqCst)
    }

    pub fn read_mute_total(&self) -> Duration {
        Duration::from_nanos(self.read_mute_nanos.load(Ordering::SeqCst))
    }

    pub fn skipped_status(&self) -> usize {
        self.skipped_status.load(Ordering::SeqCst)
    }

    pub fn fragmented_reads(&self) -> usize {
        self.fragmented_reads.load(Ordering::SeqCst)
    }

    pub fn data_writes(&self) -> usize {
        self.data_writes.load(Ordering::SeqCst)
    }

    pub fn data_reads(&self) -> usize {
        self.data_reads.load(Ordering::SeqCst)
    }
}

fn nanos(d: Duration) -> u64 {
    u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)
}

/// Transporte envuelto con las fallas de [`FlakyConfig`].
pub struct FlakyLink<T: Transport> {
    inner: T,
    clock: Option<FreezeClock>,
    cfg: FlakyConfig,
    /// Un generador por tipo de falla: lo que pasa en la escritura n no depende de cuántas
    /// lecturas o `?` hubo antes (la temporización real de cada hilo varía).
    rng_write: Rng,
    rng_read: Rng,
    rng_status: Rng,
    /// Número de escritura de datos / lectura con datos en que toca la siguiente falla aleatoria.
    next_stall: usize,
    next_mute: usize,
    counters: Arc<FlakyCounters>,
    /// Lo que la placa dijo durante un silencio y se entrega al terminar.
    held: Vec<u8>,
    mute_until: Option<Instant>,
    /// Escrituras de datos (sin `?`) y lecturas con datos hasta ahora.
    data_writes: usize,
    data_reads: usize,
    /// Lecturas vacías que siguen a un bloqueo de escritura: la placa aún no ha contestado.
    quiet_reads: usize,
    /// `?` que quedan por perder de la ráfaga en curso.
    skipping: usize,
    /// Indicador de «abandonar» del transporte (como el del puerto serie real).
    abort: Option<Arc<AtomicBool>>,
}

impl<T: Transport> FlakyLink<T> {
    /// `clock` es el reloj con el que se construyó la máquina envuelta (para congelarla durante
    /// los bloqueos de escritura); con `None` la máquina sigue corriendo mientras el anfitrión
    /// está colgado.
    pub fn new(
        inner: T,
        clock: Option<FreezeClock>,
        cfg: FlakyConfig,
    ) -> (Self, Arc<FlakyCounters>) {
        let counters = Arc::new(FlakyCounters::default());
        let (mut rng_write, mut rng_read) = (
            Rng::new(cfg.seed ^ 0x57_52_49_54),
            Rng::new(cfg.seed ^ 0x52_45_41_44),
        );
        let next_stall = gap(&mut rng_write, cfg.write_stall_every);
        let next_mute = gap(&mut rng_read, cfg.read_mute_every);
        let link = Self {
            rng_status: Rng::new(cfg.seed ^ 0x53_54_41_54),
            rng_write,
            rng_read,
            next_stall,
            next_mute,
            inner,
            clock,
            cfg,
            counters: counters.clone(),
            held: Vec::new(),
            mute_until: None,
            data_writes: 0,
            data_reads: 0,
            quiet_reads: 0,
            skipping: 0,
            abort: None,
        };
        (link, counters)
    }
}

/// Escrituras (o lecturas) hasta la siguiente falla: uniforme entre la mitad y una vez y media la
/// media, así nunca pasan más de `1,5 × every` sin falla y la cuenta mínima es predecible.
fn gap(rng: &mut Rng, every: usize) -> usize {
    if every == 0 {
        usize::MAX
    } else {
        rng.range(every.div_ceil(2), every + every / 2)
    }
}

impl<T: Transport> Transport for FlakyLink<T> {
    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        if !self.counters.armed.load(Ordering::SeqCst) {
            return self.inner.write_all(data);
        }
        // Un `?` suelto puede perderse (solo o en ráfaga); no llega a la placa.
        if data == b"?" {
            if self.skipping > 0 {
                self.skipping -= 1;
                self.counters.skipped_status.fetch_add(1, Ordering::SeqCst);
                return Ok(());
            }
            if self.rng_status.chance(self.cfg.skip_status) {
                if self.rng_status.chance(self.cfg.skip_burst) {
                    self.skipping = self.rng_status.range(1, 11);
                }
                self.counters.skipped_status.fetch_add(1, Ordering::SeqCst);
                return Ok(());
            }
        } else {
            self.data_writes += 1;
            self.counters.data_writes.fetch_add(1, Ordering::SeqCst);
            let scripted = self
                .cfg
                .scripted_write_stall
                .filter(|&(n, _)| n == self.data_writes)
                .map(|(_, d)| d);
            let random = (self.data_writes >= self.next_stall).then(|| {
                self.next_stall =
                    self.data_writes + gap(&mut self.rng_write, self.cfg.write_stall_every);
                self.rng_write
                    .duration(self.cfg.write_stall.0, self.cfg.write_stall.1)
            });
            if let Some(d) = scripted.or(random) {
                self.stall_write(d)?;
            }
        }
        self.inner.write_all(data)
    }

    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        self.read_inner(buf, timeout)
    }

    fn clear_input(&mut self) -> io::Result<()> {
        self.held.clear();
        self.inner.clear_input()
    }

    fn set_abort(&mut self, abort: Arc<AtomicBool>) {
        self.inner.set_abort(abort.clone());
        self.abort = Some(abort);
    }
}

impl<T: Transport> FlakyLink<T> {
    /// La placa deja de leer: el anfitrión se queda colgado en `write` y la placa, parada.
    fn stall_write(&mut self, d: Duration) -> io::Result<()> {
        self.counters.write_stalls.fetch_add(1, Ordering::SeqCst);
        self.counters
            .write_stall_nanos
            .fetch_add(nanos(d), Ordering::SeqCst);
        if let Some(c) = &self.clock {
            c.freeze();
        }
        // Un puerto real (`SerialTransport`) reintenta la escritura hasta su plazo y luego falla.
        let blocked = self.cfg.write_limit.map_or(d, |limit| d.min(limit));
        // Como el puerto real: con el indicador de «abandonar» activo, la espera se corta tras la
        // gracia (ver `tlaku_grbl::serial::ABORT_GRACE`).
        let started = Instant::now();
        let mut aborted = false;
        while started.elapsed() < blocked {
            if started.elapsed() >= tlaku_grbl::serial::ABORT_GRACE
                && self
                    .abort
                    .as_ref()
                    .is_some_and(|a| a.load(Ordering::SeqCst))
            {
                aborted = true;
                break;
            }
            std::thread::sleep(
                blocked
                    .saturating_sub(started.elapsed())
                    .min(Duration::from_millis(10)),
            );
        }
        if let Some(c) = &self.clock {
            c.thaw();
        }
        if aborted {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "la escritura se abandonó: se canceló o se cerró la conexión",
            ));
        }
        if blocked < d {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "la escritura no avanzó: la placa dejó de leer el USB",
            ));
        }
        self.quiet_reads = 2;
        Ok(())
    }

    fn mute_for(&mut self, d: Duration) {
        self.counters.read_mutes.fetch_add(1, Ordering::SeqCst);
        self.counters
            .read_mute_nanos
            .fetch_add(nanos(d), Ordering::SeqCst);
        self.mute_until = Some(Instant::now() + d);
    }

    fn read_inner(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        if !self.counters.armed.load(Ordering::SeqCst) && self.held.is_empty() {
            return self.inner.read_timeout(buf, timeout);
        }
        if self.quiet_reads > 0 {
            self.quiet_reads -= 1;
            std::thread::sleep(timeout);
            return Ok(0);
        }
        // Silencio en curso: se oye a la placa pero no se entrega nada hasta que termine.
        if let Some(until) = self.mute_until {
            if Instant::now() < until {
                let mut tmp = [0u8; 512];
                let n = self.inner.read_timeout(&mut tmp, timeout)?;
                self.held.extend_from_slice(&tmp[..n]);
                return Ok(0);
            }
            self.mute_until = None;
        }
        if !self.held.is_empty() {
            let n = self.held.len().min(buf.len());
            buf[..n].copy_from_slice(&self.held[..n]);
            self.held.drain(..n);
            return Ok(n);
        }
        let limit = if self.rng_read.chance(self.cfg.fragment) {
            self.counters
                .fragmented_reads
                .fetch_add(1, Ordering::SeqCst);
            self.rng_read.range(1, 16).min(buf.len())
        } else {
            buf.len()
        };
        let n = self.inner.read_timeout(&mut buf[..limit], timeout)?;
        if n > 0 {
            self.data_reads += 1;
            self.counters.data_reads.fetch_add(1, Ordering::SeqCst);
            let scripted = self
                .cfg
                .scripted_read_mute
                .filter(|&(k, _)| k == self.data_reads)
                .map(|(_, d)| d);
            let random = (self.data_reads >= self.next_mute).then(|| {
                self.next_mute =
                    self.data_reads + gap(&mut self.rng_read, self.cfg.read_mute_every);
                self.rng_read
                    .duration(self.cfg.read_mute.0, self.cfg.read_mute.1)
            });
            if let Some(d) = scripted.or(random) {
                self.mute_for(d);
            }
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_deterministic_per_seed() {
        let a: Vec<u64> = (0..8)
            .scan(Rng::new(7), |r, _| Some(r.next_u64()))
            .collect();
        let b: Vec<u64> = (0..8)
            .scan(Rng::new(7), |r, _| Some(r.next_u64()))
            .collect();
        let c: Vec<u64> = (0..8)
            .scan(Rng::new(8), |r, _| Some(r.next_u64()))
            .collect();
        assert_eq!(a, b);
        assert_ne!(a, c);
        let mut r = Rng::new(1);
        assert!((0..1000).all(|_| (0.0..1.0).contains(&r.unit())));
        assert!((0..1000).all(|_| (3..=5).contains(&r.range(3, 5))));
    }

    #[test]
    fn truncated_schedule_has_about_one_per_every() {
        let s = truncated_schedule(3, 60_000, 600);
        assert!((60..=140).contains(&s.len()), "{}", s.len());
        assert_eq!(s, truncated_schedule(3, 60_000, 600));
        assert!(s.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn freeze_clock_stops_and_resumes_where_it_left() {
        let c = FreezeClock::new(1000.0);
        std::thread::sleep(Duration::from_millis(5));
        c.freeze();
        let at = c.now();
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(c.now(), at);
        c.thaw();
        let after = c.now();
        // Sigue desde donde quedó: el hueco de 20 ms de pared (20 s simulados) no cuenta.
        assert!(
            after >= at && after < at + Duration::from_secs(5),
            "{after:?} {at:?}"
        );
        c.thaw();
        c.freeze();
        c.freeze();
        c.thaw();
    }

    use tlaku_grbl::serial::memory_pair;

    fn read_all(t: &mut impl Transport) -> Vec<u8> {
        let mut out = Vec::new();
        let mut buf = [0u8; 64];
        loop {
            match t.read_timeout(&mut buf, Duration::from_millis(5)) {
                Ok(0) | Err(_) => return out,
                Ok(n) => out.extend_from_slice(&buf[..n]),
            }
        }
    }

    #[test]
    fn stalls_come_at_bounded_gaps_and_a_limit_turns_them_into_errors() {
        let (host, mut dev) = memory_pair();
        let cfg = FlakyConfig {
            write_stall_every: 4,
            write_stall: (Duration::from_millis(1), Duration::from_millis(2)),
            ..FlakyConfig::default()
        };
        let (mut link, counters) = FlakyLink::new(host, None, cfg);
        for _ in 0..30 {
            link.write_all(b"G0X1\n").unwrap();
        }
        // Huecos de 2 a 6 escrituras: entre 5 y 15 bloqueos en 30 escrituras.
        assert!(
            (5..=15).contains(&counters.write_stalls()),
            "{}",
            counters.write_stalls()
        );
        assert_eq!(read_all(&mut dev).len(), 30 * 5);

        let cfg = FlakyConfig {
            scripted_write_stall: Some((1, Duration::from_millis(50))),
            write_limit: Some(Duration::from_millis(5)),
            ..FlakyConfig::default()
        };
        let (host, mut dev) = memory_pair();
        let (mut link, _) = FlakyLink::new(host, None, cfg);
        let started = Instant::now();
        let e = link.write_all(b"G0X1\n").unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_millis(40));
        // La escritura fallida no llegó a la placa.
        assert!(read_all(&mut dev).is_empty());
    }

    #[test]
    fn status_queries_can_be_skipped_and_reads_fragmented() {
        let (host, mut dev) = memory_pair();
        let cfg = FlakyConfig {
            skip_status: 1.0,
            fragment: 1.0,
            ..FlakyConfig::default()
        };
        let (mut link, counters) = FlakyLink::new(host, None, cfg);
        link.write_all(b"?").unwrap();
        link.write_all(b"?").unwrap();
        assert!(read_all(&mut dev).is_empty());
        assert_eq!(counters.skipped_status(), 2);
        // Desarmado, el `?` pasa.
        counters.arm(false);
        link.write_all(b"?").unwrap();
        assert_eq!(read_all(&mut dev), b"?");
        counters.arm(true);

        dev.write_all(&[b'x'; 200]).unwrap();
        let mut buf = [0u8; 256];
        let n = link
            .read_timeout(&mut buf, Duration::from_millis(5))
            .unwrap();
        assert!((1..=16).contains(&n), "{n}");
        assert!(counters.fragmented_reads() >= 1);
    }

    #[test]
    fn a_read_silence_holds_what_the_board_said_and_delivers_it_afterwards() {
        let (host, mut dev) = memory_pair();
        let cfg = FlakyConfig {
            scripted_read_mute: Some((1, Duration::from_millis(60))),
            ..FlakyConfig::default()
        };
        let (mut link, counters) = FlakyLink::new(host, None, cfg);
        dev.write_all(b"ok\r\n").unwrap();
        let mut buf = [0u8; 64];
        // La primera lectura con datos dispara el silencio (y los entrega, ya leídos).
        let first = link
            .read_timeout(&mut buf, Duration::from_millis(5))
            .unwrap();
        assert_eq!(&buf[..first], b"ok\r\n");
        dev.write_all(b"ok\r\n").unwrap();
        assert_eq!(
            link.read_timeout(&mut buf, Duration::from_millis(5))
                .unwrap(),
            0
        );
        assert_eq!(counters.read_mutes(), 1);
        std::thread::sleep(Duration::from_millis(70));
        let n = link
            .read_timeout(&mut buf, Duration::from_millis(5))
            .unwrap();
        assert_eq!(&buf[..n], b"ok\r\n");
    }
}
