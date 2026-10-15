//! Inyección de fallas programadas.

use crate::fault::{Fault, Scheduled, Trigger};

use super::*;

impl Machine {
    // -----------------------------------------------------------------------
    // Fallas
    // -----------------------------------------------------------------------

    /// Programa una falla. `Trigger::Now` se aplica en el acto.
    pub fn inject(&mut self, trigger: Trigger, fault: Fault) {
        if trigger == Trigger::Now {
            self.apply_fault(fault);
        } else {
            self.faults.push(Scheduled { trigger, fault });
        }
    }

    pub(super) fn fire_due_faults(&mut self) {
        let now = self.now;
        let mut due = Vec::new();
        self.faults.retain(|f| match f.trigger {
            Trigger::At(t) if t <= now => {
                due.push(f.fault.clone());
                false
            }
            _ => true,
        });
        for f in due {
            self.apply_fault(f);
        }
    }

    pub(super) fn fire_line_faults(&mut self) {
        let n = self.lines_seen;
        let mut due = Vec::new();
        self.faults.retain(|f| match f.trigger {
            Trigger::Line(l) if l == n => {
                due.push(f.fault.clone());
                false
            }
            _ => true,
        });
        for f in due {
            self.apply_fault(f);
        }
    }

    /// Corte de luz: se apaga todo de golpe. La cabeza queda donde estaba (`pos`), pero la
    /// controladora ya no sabe dónde está (`position_lost`) y el cable deja de contestar.
    fn power_loss(&mut self) {
        self.stop_motion();
        self.do_reset(ResetKind::Quiet);
        self.homed = false;
        self.position_lost = true;
        // Sin luz no hay láser: lo que había quemado hasta aquí queda en el rastro; nada más.
        self.exec_spindle = (SpindleMode::Off, 0.0);
        if self.homing_lock() {
            self.alarm = Some(0);
        }
        self.connected = false;
    }

    fn apply_fault(&mut self, fault: Fault) {
        match fault {
            Fault::Noise(bytes) => self.emit_raw(&bytes),
            Fault::ErrorReply(c) => self.next_reply = Some(Reply::Error(c)),
            Fault::DropReply => self.next_reply = Some(Reply::Drop),
            Fault::TruncatedStatusBeforeReply => self.next_reply = Some(Reply::GluedStatus),
            Fault::CrealityError { code, halt } => {
                self.emit_line(&format!("ERROR:{code:02}"));
                if halt {
                    self.stop_motion();
                    self.halted = true;
                    self.alarm = Some(0);
                    self.pending = None;
                    self.line_ctx.bytes = 0;
                }
            }
            Fault::Alarm(c) => self.raise_alarm(c),
            Fault::Alarm3WithBanner => self.do_reset(ResetKind::Motion),
            Fault::Reboot => self.do_reset(ResetKind::Quiet),
            Fault::PowerLoss => self.power_loss(),
            Fault::Silence {
                duration,
                drop_output,
            } => self.silence = Some((self.now + duration, drop_output)),
            Fault::Disconnect => self.connected = false,
            Fault::NanOffsets { in_status } => self.nan = Some(in_status),
            Fault::LineEnding(le) => self.line_ending = le,
            Fault::CorruptLine => self.corrupt_next = true,
        }
    }
}
