//! Pruebas de la sesión, agrupadas por tema.

mod control;
mod docs;
mod probe;
mod query;
mod send;
mod settings;

use super::*;
use crate::emu::{EmuConfig, EmuFaults, Emulator, UdpEmulator};
use crate::link::{MemoryLink, UdpLink};
use crate::rd::HomeCorner;
use crate::rd::{self, RuidaJobOptions};
use crate::safety::SafetyError;
use crate::swizzle::probe_candidates;
use std::sync::atomic::{AtomicBool, Ordering};
use tlaku_core::Point;
use tlaku_core::toolpath::{Move, Toolpath, ToolpathBlock};

fn fast_cfg() -> SessionConfig {
    SessionConfig {
        ack_timeout: Duration::from_millis(300),
        reply_timeout: Duration::from_millis(300),
        long_timeout: Duration::from_secs(2),
        home: HomeCorner::BottomLeft,
        ..Default::default()
    }
}

fn square_job() -> Vec<u8> {
    let c = |x: f64, y: f64| Move::Cut {
        to: Point::new(x, y),
        power_pct: 20.0,
    };
    let tp = Toolpath {
        blocks: vec![ToolpathBlock {
            speed_mm_min: 6000.0,
            moves: vec![
                Move::Travel {
                    to: Point::new(10.0, 10.0),
                },
                c(30.0, 10.0),
                c(30.0, 30.0),
                c(10.0, 30.0),
                c(10.0, 10.0),
            ],
            ..Default::default()
        }],
    };
    let o = RuidaJobOptions {
        home: HomeCorner::BottomLeft,
        ..Default::default()
    };
    rd::build_rd(&tp, &o).unwrap()
}

fn mem_session(
    emu_cfg: EmuConfig,
    cfg: SessionConfig,
) -> (
    RuidaSession<MemoryLink>,
    MemoryLink,
    std::sync::Arc<std::sync::Mutex<Emulator>>,
) {
    let (link, shared) = Emulator::new(emu_cfg).into_link();
    (RuidaSession::new(link.clone(), cfg), link, shared)
}

fn no_progress() -> impl FnMut(Progress) {
    |_| {}
}
