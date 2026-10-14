//! Sondeo de solo lectura: detecta el magic y lee el ID de tarjeta y la cama.

use super::*;
use crate::cmd::Command;
use crate::link::PacketLink;
use crate::packet::{self, Response};
use crate::swizzle::{Magic, probe_candidates};
use std::time::{Duration, Instant};
use tlaku_i18n::tr;

impl<L: PacketLink> RuidaSession<L> {
    // ------------------------------------------------------------------------------------
    // Sondeo

    /// Sondeo de solo lectura: `DA 00 05 7E`. Detecta el magic (el byte de ACK de la máquina va
    /// swizzleado con el suyo, así que un solo intercambio lo revela) y lee el ID de tarjeta y
    /// la cama. No envía nada que escriba ni mueva.
    ///
    /// El checksum no se sondea: la suma sobre los bytes del cable está confirmada por capturas
    /// reales (ruida.md sección 12).
    pub fn probe(&mut self) -> Result<ProbeInfo, SessionError> {
        let configured = self.cfg.magic;
        let mut candidates = vec![configured];
        if self.cfg.probe_all_magics {
            candidates.extend(probe_candidates().into_iter().filter(|m| *m != configured));
        }
        let mut last = SessionError::Timeout { what: "sondeo" };
        for (i, m) in candidates.into_iter().enumerate() {
            self.cfg.magic = m;
            // El primer magic con todos los reintentos; los demás con un solo intento corto
            // (una máquina que ignora el primero no suele contestar a los otros).
            let (attempts, wait) = if i == 0 {
                (
                    self.cfg.query_retries + 1,
                    self.cfg.ack_timeout + self.cfg.reply_timeout,
                )
            } else {
                (1, self.cfg.ack_timeout)
            };
            match self.probe_current(attempts, wait) {
                Ok(mut info) => {
                    info.magic_changed = info.magic != configured;
                    return Ok(info);
                }
                Err(e @ SessionError::Io(_)) => {
                    self.cfg.magic = configured;
                    return Err(e);
                }
                Err(e) => {
                    // Solo el primer intento informa de errores distintos de un timeout.
                    if i == 0 || matches!(e, SessionError::Timeout { .. }) {
                        last = e;
                    }
                }
            }
        }
        self.cfg.magic = configured;
        Err(last)
    }

    /// Un intercambio de sondeo con el magic actual; si el ACK delata otro magic, repite una
    /// vez con él.
    fn probe_current(&mut self, attempts: u32, wait: Duration) -> Result<ProbeInfo, SessionError> {
        let mut redirected = false;
        loop {
            let d = self.datagram(&[Command::MemRead {
                addr: addr::CARD_ID,
            }])?;
            let mut last = SessionError::Timeout { what: "sondeo" };
            let mut wire_ack: Option<u8> = None;
            for _ in 0..attempts {
                self.link.drain()?;
                self.link.send(&d)?;
                let deadline = Instant::now() + wait;
                loop {
                    let left = deadline.saturating_duration_since(Instant::now());
                    let Some(dg) = self.link.recv(left)? else {
                        break;
                    };
                    if dg.len() == 1 {
                        wire_ack = Some(dg[0]);
                        continue;
                    }
                    if let Some(v) = self.find_reply(&dg, addr::CARD_ID) {
                        let card_id = v as u32;
                        let model = model_name(card_id);
                        let bed_mm = self
                            .read_bed_um()
                            .ok()
                            .map(|[x, y]| [x as f64 / 1000.0, y as f64 / 1000.0]);
                        return Ok(ProbeInfo {
                            magic: self.cfg.magic,
                            magic_changed: false,
                            card_id,
                            model,
                            bed_mm,
                        });
                    }
                    last = SessionError::Protocol(tr!("ruida.session.probe_unreadable"));
                }
                // El ACK de otro magic delata a la máquina: no hace falta insistir con éste.
                if wire_ack.is_some_and(|b| Magic::from_wire_ack(b) != self.cfg.magic) {
                    break;
                }
            }
            // ¿Contestó con el ACK de otro magic?
            if let Some(b) = wire_ack {
                let theirs = Magic::from_wire_ack(b);
                if theirs != self.cfg.magic && !redirected {
                    redirected = true;
                    self.cfg.magic = theirs;
                    continue;
                }
                if packet::interpret_response(b, self.cfg.magic) == Response::Ack {
                    return Err(SessionError::Protocol(tr!(
                        "ruida.session.probe_no_card_id"
                    )));
                }
            }
            return Err(last);
        }
    }
}
