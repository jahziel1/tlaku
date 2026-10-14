//! Envío de un paquete con confirmación de estado.

use super::{Confirm, JobControl, LhySession, SessionError, Step};
use crate::packet::Frame;
use crate::status::{Status, StatusReport};
use crate::transport::{self, Transport};
use std::time::Duration;

impl<T: Transport> LhySession<T> {
    // ------------------------------------------------------------------------------------
    // Envío de paquetes

    fn wait_ready(&mut self, ctl: &JobControl) -> Result<StatusReport, SessionError> {
        let t = self.cfg.busy_timeout;
        self.poll_until(Some(ctl), t, "placa lista", |r| {
            if r.status.accepts_packets() || r.status == Status::CrcError {
                Step::Done
            } else {
                Step::Continue
            }
        })
    }

    fn confirm(&mut self, ctl: &JobControl) -> Result<Confirm, SessionError> {
        let t = self.cfg.busy_timeout;
        let r = self.poll_until(Some(ctl), t, "confirmación del paquete", |r| {
            if r.status.accepts_packets() || r.status == Status::CrcError {
                Step::Done
            } else {
                Step::Continue
            }
        })?;
        Ok(if r.status == Status::CrcError {
            Confirm::CrcRejected
        } else {
            Confirm::Accepted
        })
    }

    /// Un paquete: espera a que la placa esté lista, lo escribe y espera la confirmación. Un CRC
    /// malo se resuelve reenviando; si la placa avisa tarde de un CRC malo del paquete anterior,
    /// se reenvía ese antes de seguir.
    pub(super) fn send_frame(
        &mut self,
        frame: &Frame,
        index: usize,
        ctl: &JobControl,
    ) -> Result<(), SessionError> {
        let mut fails = 0u32;
        let mut current_failed = false;
        loop {
            let r = self.wait_ready(ctl)?;
            if r.status == Status::CrcError && !std::mem::take(&mut current_failed) {
                // Rechazo tardío del paquete anterior: se reenvía antes de seguir.
                fails += 1;
                if fails > self.cfg.crc_retries {
                    return Err(SessionError::CrcRepeated {
                        packet: index.saturating_sub(1),
                    });
                }
                if let Some(prev) = self.last_confirmed.clone() {
                    self.resends += 1;
                    transport::write_frame(&mut self.transport, &prev)?;
                    if let Confirm::CrcRejected = self.confirm(ctl)? {
                        continue;
                    }
                }
            }
            transport::write_frame(&mut self.transport, frame)?;
            match self.confirm(ctl)? {
                Confirm::Accepted => {
                    self.last_confirmed = Some(frame.clone());
                    return Ok(());
                }
                Confirm::CrcRejected => {
                    fails += 1;
                    self.resends += 1;
                    if fails > self.cfg.crc_retries {
                        return Err(SessionError::CrcRepeated { packet: index });
                    }
                    current_failed = true;
                }
            }
        }
    }

    /// Comprueba que la placa no haya rechazado tarde el último paquete confirmado; si lo hizo,
    /// lo reenvía. Se usa antes de dar un trabajo por terminado o de esperar su fin.
    pub(super) fn resolve_late_crc(
        &mut self,
        ctl: &JobControl,
        index: usize,
    ) -> Result<(), SessionError> {
        let mut fails = 0u32;
        loop {
            let r = self.read_status()?;
            if r.status != Status::CrcError {
                return Ok(());
            }
            let Some(prev) = self.last_confirmed.clone() else {
                return Ok(());
            };
            fails += 1;
            if fails > self.cfg.crc_retries {
                return Err(SessionError::CrcRepeated { packet: index });
            }
            self.resends += 1;
            transport::write_frame(&mut self.transport, &prev)?;
            self.confirm(ctl)?;
        }
    }

    /// Espera a que la placa vacíe su búfer (estado 236). Con `strict` agotar el tiempo es un
    /// error; si no, se da por terminado y se devuelve `false`.
    pub(super) fn wait_empty(
        &mut self,
        ctl: &JobControl,
        timeout: Duration,
        strict: bool,
    ) -> Result<bool, SessionError> {
        let r = self.poll_until(Some(ctl), timeout, "fin del movimiento", |r| {
            if r.status.is_empty() {
                Step::Done
            } else {
                Step::Continue
            }
        });
        match r {
            Ok(_) => Ok(true),
            Err(SessionError::Timeout { .. }) if !strict => Ok(false),
            Err(e) => Err(e),
        }
    }
}
