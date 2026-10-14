//! Estado de la placa: lectura, sondeo y esperas.

use super::{BoardInfo, JobControl, LhySession, SessionError, Step};
use crate::status::{Status, StatusReport, looks_like_moshiboard};
use crate::transport::{self, Transport, TransportError};
use std::time::{Duration, Instant};
use tlaku_i18n::tr;

impl<T: Transport> LhySession<T> {
    // ------------------------------------------------------------------------------------
    // Estado

    fn backoff(&self, n: u32) {
        if n > 10 && !self.cfg.poll_backoff_max.is_zero() {
            let ms = u64::from(n - 10).min(1000);
            std::thread::sleep(Duration::from_millis(ms).min(self.cfg.poll_backoff_max));
        }
    }

    /// Una lectura de estado con reintentos cortos ante un tiempo agotado del USB.
    pub(super) fn read_status(&mut self) -> Result<StatusReport, SessionError> {
        let mut last = TransportError::Timeout;
        for _ in 0..3 {
            match transport::read_status(&mut self.transport) {
                Ok(r) => return Ok(r),
                Err(TransportError::Timeout) => last = TransportError::Timeout,
                Err(e) => return Err(e.into()),
            }
        }
        Err(last.into())
    }

    /// Lee el estado de la placa (solo lectura: no mueve nada).
    pub fn status(&mut self) -> Result<StatusReport, SessionError> {
        let r = self.read_status()?;
        if looks_like_moshiboard(r.code()) {
            return Err(SessionError::NotM2Nano { code: r.code() });
        }
        Ok(r)
    }

    /// Sondeo de solo lectura: una consulta de estado. Detecta una Moshiboard.
    pub fn probe(&mut self) -> Result<BoardInfo, SessionError> {
        let status = self.status()?;
        let mut notes = Vec::new();
        match status.status {
            Status::LowPower => notes.push(tr!("lhymicro.session.note_low_power")),
            Status::CrcError => notes.push(tr!("lhymicro.session.note_crc")),
            Status::NoReply => notes.push(tr!("lhymicro.session.note_no_reply")),
            Status::Unknown(c) => notes.push(tr!("lhymicro.session.note_unknown", code = c)),
            _ => {}
        }
        Ok(BoardInfo {
            ready: status.status.accepts_packets(),
            status,
            transport: self.transport.info(),
            notes,
        })
    }

    /// Sondea el estado hasta que `pred` diga `Done`. Atiende pausa y detención si hay `ctl`.
    pub(super) fn poll_until(
        &mut self,
        ctl: Option<&JobControl>,
        timeout: Duration,
        what: &'static str,
        mut pred: impl FnMut(&StatusReport) -> Step,
    ) -> Result<StatusReport, SessionError> {
        let mut start = Instant::now();
        let mut n = 0u32;
        let mut anomalies = 0u32;
        loop {
            if let Some(c) = ctl
                && self.service(c)?
            {
                // El tiempo en pausa no cuenta para el tiempo máximo de espera.
                start = Instant::now();
            }
            let r = self.read_status()?;
            if looks_like_moshiboard(r.code()) {
                return Err(SessionError::NotM2Nano { code: r.code() });
            }
            match r.status {
                Status::LowPower => return Err(SessionError::LowPower),
                Status::NoReply | Status::Unknown(_) => {
                    anomalies += 1;
                    if anomalies > self.cfg.anomaly_polls {
                        return Err(SessionError::NoStatus(r.status));
                    }
                }
                _ => anomalies = 0,
            }
            if let Step::Done = pred(&r) {
                return Ok(r);
            }
            if start.elapsed() > timeout {
                return Err(SessionError::Timeout { what });
            }
            n += 1;
            self.backoff(n);
        }
    }
}
