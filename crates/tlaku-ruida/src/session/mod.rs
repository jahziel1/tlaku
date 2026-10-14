//! Sesión con una controladora Ruida por paquetes (UDP 50200): sondeo, envío de un trabajo,
//! estado, control del proceso, saltos y encuadre.
//!
//! Protocolo (ruida.md sección 12, fuentes ruida-pa y MeerK40t, ambas MIT y probadas en una
//! RDC6442S):
//! - cada mensaje es un datagrama `checksum(2) + datos swizzleados` y la máquina responde con
//!   **un byte de ACK** por paquete; las lecturas (`DA 00`) añaden un segundo datagrama
//!   `DA 01 dirección valor(5)` sin checksum, también swizzleado;
//! - la máquina "habla solo cuando se le habla": no hay mensajes espontáneos;
//! - sondeo: `DA 00 05 7E` (ID de tarjeta). Solo lectura. Es lo que usa ruida-pa de ping;
//! - pausar `D8 02`, reanudar `D8 03`, detener `D8 01`, home `D8 2A`, saltos `D9` con las
//!   opciones `00`/`02` (láser apagado). Las cuatro de control las mandan MeerK40t y ruida-pa
//!   por el mismo canal incluso con un trabajo en marcha.
//!
//! **Seguridad.** Un trabajo en modo [`SendMode::Run`] empieza con `D8 00` y la máquina puede
//! arrancar antes de recibir el final; ninguna fuente documenta un "abortar" distinto de
//! `D8 01`. Si el envío falla a medias se intenta ese `D8 01` y se avisa al usuario de que
//! puede que no llegue (la máquina ignora la red con un trabajo en marcha, [FOR3]). El modo
//! [`SendMode::Store`] guarda el trabajo como documento (`E8 02` + `E7 01 nombre`, igual que la
//! captura de Nova 35) y no arranca nada hasta que el usuario lo inicia en el panel: es el
//! predeterminado de la línea de comandos.

use crate::cmd::{self, Command};
use crate::link::PacketLink;
use crate::num;
use crate::packet::{self, PacketConfig, Response};
use crate::swizzle::Magic;
use std::time::{Duration, Instant};
use tlaku_i18n::tr;

mod config;
mod control;
mod probe;
mod query;
mod send;
#[cfg(test)]
mod tests;
mod types;

pub use config::{MAX_LISTED_DOCS, SessionConfig, SessionError, addr, model_name};
pub use types::{
    ControllerFile, ControllerInfo, MachineStatus, ProbeInfo, Progress, SendMode, StopOutcome,
    UploadError, UploadReport,
};

pub struct RuidaSession<L: PacketLink> {
    link: L,
    cfg: SessionConfig,
    /// Cama informada por la máquina (µm), si ya se leyó.
    bed_um: Option<[i64; 2]>,
}

impl<L: PacketLink> RuidaSession<L> {
    pub fn new(link: L, cfg: SessionConfig) -> Self {
        RuidaSession {
            link,
            cfg,
            bed_um: None,
        }
    }

    pub fn config(&self) -> &SessionConfig {
        &self.cfg
    }

    pub fn magic(&self) -> Magic {
        self.cfg.magic
    }

    /// Acceso para ajustar tiempos o límites entre operaciones (p. ej. esperas cortas al
    /// sondear el estado y largas al subir un trabajo). No cambies el magic: usa el sondeo.
    pub fn config_mut(&mut self) -> &mut SessionConfig {
        &mut self.cfg
    }

    fn pcfg(&self) -> PacketConfig {
        PacketConfig {
            magic: self.cfg.magic,
            max_data_bytes: self.cfg.max_data_bytes,
        }
    }

    fn datagram(&self, cmds: &[Command]) -> Result<Vec<u8>, SessionError> {
        let plain = cmd::encode_stream(cmds)
            .map_err(|e| SessionError::Protocol(tr!("ruida.session.invalid_command", e = e)))?;
        Ok(packet::build_packet(&plain, &self.pcfg()))
    }

    fn bed_mm(&self) -> [f64; 2] {
        match self.bed_um {
            Some([x, y]) => [x as f64 / 1000.0, y as f64 / 1000.0],
            None => self.cfg.limits.bed_mm,
        }
    }

    // ------------------------------------------------------------------------------------
    // Envío con ACK

    /// Espera el byte de respuesta de un paquete. Los datagramas más largos (respuestas de
    /// lectura) se guardan en `stash`.
    fn wait_response(
        &mut self,
        timeout: Duration,
        stash: &mut Vec<Vec<u8>>,
    ) -> Result<Option<Response>, SessionError> {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.link.recv(left)? {
                None => return Ok(None),
                Some(d) if d.len() == 1 => {
                    return Ok(Some(packet::interpret_response(d[0], self.cfg.magic)));
                }
                Some(d) if d.is_empty() => {}
                Some(d) => stash.push(d),
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
        }
    }

    /// Envía un datagrama y espera su ACK. `allow_err_retry`: reenviar una vez ante `ERR`
    /// (FabLab: solo el primer paquete de un trabajo).
    fn send_acked(
        &mut self,
        datagram: &[u8],
        index: usize,
        timeout: Duration,
        allow_err_retry: bool,
        stash: &mut Vec<Vec<u8>>,
        resends: &mut usize,
    ) -> Result<(), SessionError> {
        let (mut naks, mut errs, mut timeouts) = (0u32, 0u32, 0u32);
        loop {
            self.link.send(datagram)?;
            match self.wait_response(timeout, stash)? {
                Some(Response::Ack) => return Ok(()),
                Some(Response::Nak) => {
                    naks += 1;
                    if naks > self.cfg.nak_retries {
                        return Err(SessionError::Rejected {
                            response: Response::Nak,
                            packet: index,
                        });
                    }
                    *resends += 1;
                }
                Some(Response::Err) => {
                    errs += 1;
                    if !allow_err_retry || errs > 1 {
                        return Err(SessionError::Rejected {
                            response: Response::Err,
                            packet: index,
                        });
                    }
                    *resends += 1;
                }
                // Keep-alive: no es la respuesta a este paquete; se vuelve a esperar sin reenviar.
                Some(Response::Enq) => {
                    if let Some(Response::Ack) = self.wait_response(timeout, stash)? {
                        return Ok(());
                    }
                    return Err(SessionError::Timeout { what: "ACK" });
                }
                Some(Response::Unknown(b)) => {
                    return Err(SessionError::Protocol(tr!(
                        "ruida.session.unexpected_reply_packet",
                        byte = format!("{b:02X}"),
                        packet = index + 1,
                        magic = format!("{:02X}", self.cfg.magic.0)
                    )));
                }
                None => {
                    timeouts += 1;
                    if timeouts > self.cfg.timeout_retries {
                        return Err(SessionError::Timeout { what: "ACK" });
                    }
                    *resends += 1;
                }
            }
        }
    }

    /// Envía un mensaje corto (un solo paquete) y espera el ACK.
    fn command(&mut self, cmds: &[Command], timeout: Duration) -> Result<(), SessionError> {
        let d = self.datagram(cmds)?;
        self.link.drain()?;
        let (mut stash, mut resends) = (Vec::new(), 0);
        self.send_acked(&d, 0, timeout, false, &mut stash, &mut resends)
    }
}

fn num_err(e: num::NumError) -> SessionError {
    SessionError::Protocol(e.to_string())
}

/// Valor de 35 bits con signo.
fn signed35(v: u64) -> i64 {
    if v & (1 << 34) != 0 {
        v as i64 - (1 << 35)
    } else {
        v as i64
    }
}
