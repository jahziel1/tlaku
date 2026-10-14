//! Envío de un trabajo completo con progreso.

use super::{JobError, JobReport, JobState, LhySession, Progress, SessionError, StopOutcome, secs};
use crate::encode::Program;
use crate::packet::pause_toggle_frame;
use crate::status::Status;
use crate::transport::Transport;
use std::time::Instant;

impl<T: Transport> LhySession<T> {
    // ------------------------------------------------------------------------------------
    // Trabajo

    /// Envía un trabajo y espera a que termine. `progress` se llama tras cada paquete. Detener,
    /// pausar y reanudar se hacen con [`LhySession::control`].
    ///
    /// Exige que se conozca la posición del cabezal y que coincida con la que se usó al
    /// codificar. Si algo falla, el trabajo se aborta, se intenta apagar el láser y la posición
    /// queda desconocida.
    pub fn send_job(
        &mut self,
        program: &Program,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<JobReport, Box<JobError>> {
        let early = |cause: SessionError| {
            Box::new(JobError {
                cause,
                packets_sent: 0,
                packets_total: 0,
                stop: StopOutcome::Acked,
            })
        };
        if self.control.state() != JobState::Idle {
            return Err(early(SessionError::JobActive));
        }
        if program.board != self.cfg.board {
            return Err(early(SessionError::BadProgram(
                "el trabajo se codificó para otra placa",
            )));
        }
        let Some(pos) = self.pos else {
            return Err(early(SessionError::UnknownPosition));
        };
        if pos != program.start_mil {
            return Err(early(SessionError::StartMismatch {
                expected: program.start_mil,
                found: pos,
            }));
        }
        let frames = program.frames().map_err(|e| early(e.into()))?;
        // La placa debe estar en reposo y lista antes de empezar.
        let st = self.status().map_err(early)?;
        match st.status {
            Status::LowPower => return Err(early(SessionError::LowPower)),
            s @ (Status::NoReply | Status::Unknown(_)) => {
                return Err(early(SessionError::NoStatus(s)));
            }
            s if !s.accepts_packets() && s != Status::CrcError => {
                return Err(early(SessionError::Busy(s)));
            }
            _ => {}
        }

        let total = frames.len();
        let bytes_total = total * crate::crc::WIRE_LEN;
        let ctl = self.control.clone();
        ctl.reset();
        ctl.set_state(JobState::Running);
        self.pause_applied = false;
        self.last_confirmed = None;
        self.resends = 0;
        let started = Instant::now();
        let mut sent = 0usize;
        let mut finish_confirmed = false;

        let run = (|| -> Result<(), SessionError> {
            for (i, frame) in frames.iter().enumerate() {
                self.service(&ctl)?;
                self.send_frame(frame, i, &ctl)?;
                sent = i + 1;
                if frame.wait_finish || sent == total {
                    self.resolve_late_crc(&ctl, i)?;
                }
                progress(Progress {
                    packet: sent,
                    packets: total,
                    bytes_sent: sent * crate::crc::WIRE_LEN,
                    bytes_total,
                });
                if frame.wait_finish && (self.cfg.strict_finish || i + 1 == total) {
                    let strict = self.cfg.strict_finish;
                    // Sin espera estricta, el final del trabajo se espera lo que estima el
                    // codificador (con margen), no los diez minutos del tiempo máximo.
                    let limit = if strict {
                        self.cfg.busy_timeout
                    } else {
                        secs(
                            program.stats.est_seconds * 1.5 + self.cfg.motion_margin_s,
                            self.cfg.busy_timeout,
                        )
                        .min(self.cfg.busy_timeout)
                    };
                    let ok = self.wait_empty(&ctl, limit, strict)?;
                    if i + 1 == total {
                        finish_confirmed = ok;
                    }
                }
            }
            if total == 0 {
                finish_confirmed = true;
            }
            Ok(())
        })();

        match run {
            Ok(()) => {
                // Una pausa pedida justo al final se cancela: ya no hay nada que pausar.
                if self.pause_applied {
                    let _ = self.send_realtime(&pause_toggle_frame());
                    self.pause_applied = false;
                }
                ctl.set_state(JobState::Idle);
                self.pos = Some(program.end_mil);
                Ok(JobReport {
                    packets: total,
                    bytes: bytes_total,
                    resends: self.resends,
                    finish_confirmed,
                    elapsed: started.elapsed(),
                    warnings: program.warnings.clone(),
                    end_mm: program.end_mm(),
                })
            }
            Err(cause) => {
                let stop = self.stop();
                Err(Box::new(JobError {
                    cause,
                    packets_sent: sent,
                    packets_total: total,
                    stop,
                }))
            }
        }
    }
}
