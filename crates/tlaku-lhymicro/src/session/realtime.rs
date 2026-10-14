//! Pausa, reanudación y detención en tiempo real.

use super::{JobControl, JobState, LhySession, SessionError, StopOutcome};
use crate::packet::{Frame, laser_off_frame, pause_toggle_frame, stop_frames};
use crate::status::Status;
use crate::transport::{self, Transport};
use std::time::{Duration, Instant};

impl<T: Transport> LhySession<T> {
    // ------------------------------------------------------------------------------------
    // Pausa, reanudación y detención

    /// Orden en tiempo real: se escribe sin esperar a que haya sitio (con la placa pausada y el
    /// búfer lleno nunca lo habría) y vale con que no haya error de CRC.
    pub(super) fn send_realtime(&mut self, frame: &Frame) -> Result<(), SessionError> {
        let started = Instant::now();
        for _ in 0..=self.cfg.crc_retries {
            transport::write_frame(&mut self.transport, frame)?;
            let r = self.read_status()?;
            match r.status {
                Status::CrcError => {
                    self.resends += 1;
                    if started.elapsed() > self.cfg.realtime_timeout {
                        break;
                    }
                }
                Status::LowPower => return Err(SessionError::LowPower),
                _ => return Ok(()),
            }
        }
        Err(SessionError::CrcRepeated { packet: 0 })
    }

    /// Atiende las peticiones de pausa y detención. Si hay pausa, espera aquí. Devuelve `true`
    /// si estuvo en pausa (para no contar ese tiempo en las esperas).
    pub(super) fn service(&mut self, ctl: &JobControl) -> Result<bool, SessionError> {
        if ctl.stop_requested() {
            return Err(SessionError::Cancelled);
        }
        let mut paused = false;
        if ctl.pause_requested() && !self.pause_applied {
            self.send_realtime(&pause_toggle_frame())?;
            self.pause_applied = true;
            ctl.set_state(JobState::Paused);
        }
        let mut idle_polls = 0u32;
        while self.pause_applied {
            paused = true;
            if ctl.stop_requested() {
                return Err(SessionError::Cancelled);
            }
            if !ctl.pause_requested() {
                self.send_realtime(&pause_toggle_frame())?;
                self.pause_applied = false;
                ctl.set_state(JobState::Running);
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
            idle_polls += 1;
            if idle_polls.is_multiple_of(25) {
                // Vigila la conexión y la energía mientras está en pausa.
                let r = self.read_status()?;
                if r.status == Status::LowPower {
                    return Err(SessionError::LowPower);
                }
            }
        }
        Ok(paused)
    }

    /// Detiene la placa y apaga el láser: `I` y `IUS1P`. No exige ninguna precondición.
    pub fn stop(&mut self) -> StopOutcome {
        let mut ok = true;
        for f in stop_frames() {
            if self.send_realtime(&f).is_err() {
                ok = false;
            }
        }
        self.pause_applied = false;
        self.last_confirmed = None;
        self.pos = None;
        self.control.set_state(JobState::Idle);
        if ok {
            StopOutcome::Acked
        } else {
            StopOutcome::Unconfirmed
        }
    }

    /// Intenta apagar el láser sin cancelar nada más (`IUS1P`).
    pub fn laser_off(&mut self) -> Result<(), SessionError> {
        self.send_realtime(&laser_off_frame())
    }
}
