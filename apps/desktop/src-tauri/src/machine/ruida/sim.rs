//! Máquina virtual: el emulador local con el que "se ejecuta" un trabajo.

use super::*;

// ---------------------------------------------------------------------------
// Máquina virtual
// ---------------------------------------------------------------------------

/// Emulador local con el que "se ejecuta" un trabajo durante su tiempo estimado.
pub struct VirtualSim {
    /// Mantiene vivo el emulador UDP (se detiene al soltarlo).
    pub(super) server: UdpEmulator,
    pub(super) speed: f64,
    /// Segundos simulados que le quedan al trabajo en marcha.
    pub(super) remaining: Option<f64>,
    pub(super) last_tick: Instant,
}

impl VirtualSim {
    pub fn new(server: UdpEmulator, speed: f64) -> Self {
        Self {
            server,
            speed,
            remaining: None,
            last_tick: Instant::now(),
        }
    }

    pub(super) fn start(&mut self, estimate_s: f64) {
        let s = if estimate_s.is_finite() {
            estimate_s.max(1.0)
        } else {
            1.0
        };
        self.remaining = Some(s);
        self.last_tick = Instant::now();
    }

    /// Hace avanzar el reloj del trabajo; al agotarse, la máquina lo da por terminado.
    pub(super) fn tick(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f64() * self.speed;
        self.last_tick = now;
        let Some(rem) = self.remaining.as_mut() else {
            return;
        };
        let mut emu = self.server.emulator();
        if !emu.running {
            // Se detuvo desde fuera (Detener).
            self.remaining = None;
        } else if !emu.paused {
            *rem -= dt;
            if *rem <= 0.0 {
                emu.finish_job();
                self.remaining = None;
            }
        }
    }
}
