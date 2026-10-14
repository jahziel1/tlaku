//! Mando de un trabajo en marcha, utilizable desde otro hilo.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// Estado del trabajo, visible desde otros hilos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    Idle,
    Running,
    Paused,
}

#[derive(Debug, Default)]
struct ControlInner {
    stop: AtomicBool,
    pause: AtomicBool,
    state: AtomicU8,
}

/// Mando de un trabajo en marcha: se puede clonar y usar desde otro hilo.
#[derive(Debug, Clone, Default)]
pub struct JobControl {
    inner: Arc<ControlInner>,
}

impl JobControl {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pide detener el trabajo: se cancela, se apaga el láser y la posición queda desconocida.
    pub fn request_stop(&self) {
        self.inner.stop.store(true, Ordering::SeqCst);
    }

    pub fn request_pause(&self) {
        self.inner.pause.store(true, Ordering::SeqCst);
    }

    pub fn request_resume(&self) {
        self.inner.pause.store(false, Ordering::SeqCst);
    }

    pub fn stop_requested(&self) -> bool {
        self.inner.stop.load(Ordering::SeqCst)
    }

    pub fn pause_requested(&self) -> bool {
        self.inner.pause.load(Ordering::SeqCst)
    }

    pub fn state(&self) -> JobState {
        match self.inner.state.load(Ordering::SeqCst) {
            1 => JobState::Running,
            2 => JobState::Paused,
            _ => JobState::Idle,
        }
    }

    pub(super) fn set_state(&self, s: JobState) {
        self.inner.state.store(
            match s {
                JobState::Idle => 0,
                JobState::Running => 1,
                JobState::Paused => 2,
            },
            Ordering::SeqCst,
        );
    }

    pub(super) fn reset(&self) {
        self.inner.stop.store(false, Ordering::SeqCst);
        self.inner.pause.store(false, Ordering::SeqCst);
    }
}
