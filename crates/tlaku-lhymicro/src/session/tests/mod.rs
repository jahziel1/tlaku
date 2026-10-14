//! Pruebas de la sesión contra el emulador, agrupadas por tema.

mod control;
mod faults;
mod job;
mod motion;

use super::*;
use crate::emu::{EmuBoard, EmuConfig, EmuFaults, EmuTransport, emulator};
use crate::encode::Warning;
use std::sync::{Arc, Mutex};
use tlaku_core::toolpath::{Move, ToolpathBlock};

type Shared = Arc<Mutex<EmuBoard>>;

fn fast_cfg() -> SessionConfig {
    SessionConfig {
        busy_timeout: Duration::from_secs(5),
        realtime_timeout: Duration::from_secs(1),
        poll_backoff_max: Duration::ZERO,
        home_timeout: Duration::from_secs(5),
        ..SessionConfig::default()
    }
}

fn session(emu: EmuConfig) -> (LhySession<EmuTransport>, Shared) {
    let (t, b) = emulator(emu);
    (LhySession::new(t, fast_cfg()), b)
}

fn square(x0: f64, y0: f64, side: f64, speed_mm_min: f64) -> Toolpath {
    let c = |x: f64, y: f64| Move::Cut {
        to: Point::new(x, y),
        power_pct: 50.0,
    };
    Toolpath {
        blocks: vec![ToolpathBlock {
            speed_mm_min,
            moves: vec![
                Move::Travel {
                    to: Point::new(x0, y0),
                },
                c(x0 + side, y0),
                c(x0 + side, y0 + side),
                c(x0, y0 + side),
                c(x0, y0),
            ],
            ..Default::default()
        }],
    }
}

fn no_progress() -> impl FnMut(Progress) {
    |_| {}
}

fn board(b: &Shared) -> std::sync::MutexGuard<'_, EmuBoard> {
    b.lock().unwrap()
}
