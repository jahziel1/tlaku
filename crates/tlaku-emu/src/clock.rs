//! Relojes inyectables: deciden cómo transcurre el tiempo simulado de la controladora.
//!
//! El emulador no usa hilos propios: avanza su estado "a demanda" hasta el instante que dice
//! el reloj cada vez que el anfitrión escribe, lee o consulta. Así el mismo código sirve para
//! pruebas deterministas (reloj manual), rápidas (reloj automático) y de tiempo real (reloj
//! escalado).

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Fuente de tiempo simulado.
pub trait Clock: Send + Sync {
    /// Tiempo simulado transcurrido desde que se encendió la controladora.
    fn now(&self) -> Duration;

    /// Deja pasar `d` desde el código de prueba (`Handle::advance`). Los relojes controlables lo
    /// suman al tiempo simulado; los de pared duermen `d` de tiempo simulado.
    fn pass(&self, d: Duration);

    /// El anfitrión espera `step` de lectura sin recibir datos. El reloj automático lo trata
    /// como tiempo simulado transcurrido (sin dormir de verdad); los demás duermen un poco.
    fn host_wait(&self, step: Duration);
}

fn nanos(d: Duration) -> u64 {
    u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)
}

/// Reloj manual: el tiempo solo avanza con [`ManualClock::advance`]. Es el más determinista:
/// una lectura sin datos no hace pasar tiempo.
#[derive(Clone, Default)]
pub struct ManualClock(Arc<AtomicU64>);

impl ManualClock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Suma `d` al tiempo simulado.
    pub fn advance(&self, d: Duration) {
        self.0.fetch_add(nanos(d), Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Duration {
        Duration::from_nanos(self.0.load(Ordering::SeqCst))
    }

    fn pass(&self, d: Duration) {
        self.advance(d);
    }

    fn host_wait(&self, step: Duration) {
        // No hace pasar tiempo simulado; solo evita un bucle caliente del anfitrión.
        std::thread::sleep(step.min(Duration::from_micros(200)));
    }
}

/// Reloj automático: cada espera sin datos del anfitrión es tiempo simulado que transcurre, sin
/// dormir. Un `read_timeout(5 ms)` vacío equivale a 5 ms simulados. Los trabajos se ejecutan a
/// la velocidad de la CPU, pero los temporizadores reales del anfitrión (sondeo `?`, silencio)
/// siguen en tiempo real, así que conviene reducirlos en las pruebas.
#[derive(Clone, Default)]
pub struct AutoClock(Arc<AtomicU64>);

impl AutoClock {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Clock for AutoClock {
    fn now(&self) -> Duration {
        Duration::from_nanos(self.0.load(Ordering::SeqCst))
    }

    fn pass(&self, d: Duration) {
        self.0.fetch_add(nanos(d), Ordering::SeqCst);
    }

    fn host_wait(&self, step: Duration) {
        self.pass(step);
        std::thread::yield_now();
    }
}

/// Reloj de pared multiplicado por `speed`: con `speed = 1.0` es tiempo real; con 20.0 un
/// segundo simulado dura 50 ms. Es el adecuado cuando el anfitrión usa sus propios plazos
/// reales (pausas, cancelaciones, silencio).
#[derive(Clone)]
pub struct ScaledClock {
    start: Instant,
    speed: f64,
}

impl ScaledClock {
    pub fn new(speed: f64) -> Self {
        let speed = if speed.is_finite() && speed > 0.0 {
            speed
        } else {
            1.0
        };
        Self {
            start: Instant::now(),
            speed,
        }
    }

    pub fn realtime() -> Self {
        Self::new(1.0)
    }
}

impl Clock for ScaledClock {
    fn now(&self) -> Duration {
        self.start.elapsed().mul_f64(self.speed)
    }

    fn pass(&self, d: Duration) {
        std::thread::sleep(d.div_f64(self.speed));
    }

    fn host_wait(&self, step: Duration) {
        std::thread::sleep(step);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_only_moves_when_told() {
        let c = ManualClock::new();
        assert_eq!(c.now(), Duration::ZERO);
        c.host_wait(Duration::from_millis(1));
        assert_eq!(c.now(), Duration::ZERO);
        c.pass(Duration::from_secs(2));
        assert_eq!(c.now(), Duration::from_secs(2));
    }

    #[test]
    fn auto_counts_host_waits() {
        let c = AutoClock::new();
        c.host_wait(Duration::from_millis(5));
        c.host_wait(Duration::from_millis(5));
        assert_eq!(c.now(), Duration::from_millis(10));
    }

    #[test]
    fn scaled_runs_faster_than_wall_clock() {
        let c = ScaledClock::new(100.0);
        std::thread::sleep(Duration::from_millis(10));
        assert!(c.now() >= Duration::from_millis(900));
        assert_eq!(ScaledClock::new(-3.0).speed, 1.0);
    }
}
