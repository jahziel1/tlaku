//! Envío de un trabajo `.rd` por paquetes con ACK y parada de seguridad.

use super::*;
use crate::cmd::{self, Command, ProcessOp};
use crate::link::PacketLink;
use crate::packet;
use crate::safety;
use std::sync::atomic::{AtomicBool, Ordering};
use tlaku_i18n::tr;

impl<L: PacketLink> RuidaSession<L> {
    // ------------------------------------------------------------------------------------
    // Envío de un trabajo

    /// Envía un flujo `.rd` en claro (sin swizzle, como lo genera `rd::build_rd`).
    ///
    /// Comprueba la seguridad del flujo, que la máquina esté en reposo y que termine en `D7`;
    /// lo parte en paquetes en límite de comando y espera el ACK de cada uno. `cancel` se
    /// consulta entre paquetes. Ver la nota de seguridad del módulo sobre fallos a medias.
    pub fn send_job(
        &mut self,
        plain: &[u8],
        mode: &SendMode,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<UploadReport, Box<UploadError>> {
        let early = |cause: SessionError| {
            Box::new(UploadError {
                cause,
                packets_sent: 0,
                packets_total: 0,
                stop: StopOutcome::NotNeeded,
                machine_may_be_running: false,
            })
        };
        let cmds = cmd::decode_stream(plain).map_err(|e| {
            early(SessionError::Protocol(tr!(
                "ruida.session.job_invalid",
                e = e
            )))
        })?;
        if cmds.last() != Some(&Command::EndOfFile) {
            return Err(early(SessionError::Protocol(tr!(
                "ruida.session.job_no_eof"
            ))));
        }
        safety::check_commands(&cmds, &self.cfg.limits, false).map_err(|e| early(e.into()))?;
        if let SendMode::Store { name } = mode
            && (name.is_empty()
                || name.len() > 40
                || name.bytes().any(|b| !(0x20..0x7F).contains(&b)))
        {
            return Err(early(SessionError::Protocol(tr!(
                "ruida.session.bad_doc_name"
            ))));
        }
        if self.cfg.require_idle {
            let st = self.status().map_err(early)?;
            if !st.is_idle() {
                return Err(early(SessionError::Busy(st)));
            }
        }

        let pcfg = self.pcfg();
        let mut datagrams = Vec::new();
        if let SendMode::Store { name } = mode {
            // Igual que la captura de Nova 35: `E8 02` y `E7 01 nombre` en paquetes propios.
            datagrams.push(packet::build_packet(&[0xE8, 0x02], &pcfg));
            let mut n = Vec::new();
            Command::FileName(name.clone())
                .encode(&mut n)
                .map_err(|e| early(SessionError::Protocol(e.to_string())))?;
            datagrams.push(packet::build_packet(&n, &pcfg));
        }
        let body = packet::split_into_packets(plain, &pcfg).map_err(|e| early(e.into()))?;
        datagrams.extend(body);
        let total = datagrams.len();
        let bytes_total: usize = datagrams.iter().map(Vec::len).sum();

        let mut resends = 0usize;
        let mut bytes_sent = 0usize;
        let mut stash = Vec::new();
        self.link.drain().map_err(|e| early(e.into()))?;
        for (i, d) in datagrams.iter().enumerate() {
            let result = if cancel.load(Ordering::SeqCst) {
                Err(SessionError::Cancelled)
            } else {
                // FabLab: ante ERR se reintenta solo el primer paquete.
                self.send_acked(d, i, self.cfg.ack_timeout, i == 0, &mut stash, &mut resends)
            };
            if let Err(cause) = result {
                return Err(self.fail_upload(cause, i, total, mode));
            }
            bytes_sent += d.len();
            progress(Progress {
                packet: i + 1,
                packets: total,
                bytes_sent,
                bytes_total,
            });
        }
        Ok(UploadReport {
            packets: total,
            bytes: bytes_total,
            resends,
            running: matches!(mode, SendMode::Run),
        })
    }

    /// Deja la máquina lo más segura posible tras un fallo a medias.
    fn fail_upload(
        &mut self,
        cause: SessionError,
        sent: usize,
        total: usize,
        mode: &SendMode,
    ) -> Box<UploadError> {
        let (stop, may_run) = match mode {
            // Nada se ejecuta: el documento incompleto no arranca solo.
            SendMode::Store { .. } => (StopOutcome::NotNeeded, false),
            SendMode::Run => {
                // Hasta el primer intento de enviar puede haber llegado el D8 00.
                let acked = self.try_stop();
                (
                    if acked {
                        StopOutcome::Acked
                    } else {
                        StopOutcome::Unconfirmed
                    },
                    !acked,
                )
            }
        };
        Box::new(UploadError {
            cause,
            packets_sent: sent,
            packets_total: total,
            stop,
            machine_may_be_running: may_run,
        })
    }

    /// `D8 01` con reintentos cortos; `true` si la máquina lo confirmó.
    fn try_stop(&mut self) -> bool {
        for _ in 0..3 {
            if self
                .command(&[Command::Process(ProcessOp::Stop)], self.cfg.ack_timeout)
                .is_ok()
            {
                return true;
            }
        }
        false
    }
}
