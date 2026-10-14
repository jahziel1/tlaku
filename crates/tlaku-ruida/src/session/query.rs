//! Lecturas de memoria, estado, información del controlador y documentos guardados.

use super::types::{BIT_MOVING, BIT_PAUSED, BIT_RUNNING};
use super::*;
use crate::cmd::{self, Command};
use crate::link::PacketLink;
use crate::packet::{self, Response};
use std::time::Instant;

impl<L: PacketLink> RuidaSession<L> {
    // ------------------------------------------------------------------------------------
    // Lecturas

    /// Lee una dirección de memoria (`DA 00`, solo lectura) y devuelve el valor de 35 bits.
    pub fn read_mem(&mut self, address: u16) -> Result<u64, SessionError> {
        let d = self.datagram(&[Command::MemRead { addr: address }])?;
        self.query(&d, |s, dg| s.find_reply(dg, address))
    }

    /// Envía una consulta de solo lectura y espera su respuesta: el ACK llega primero y la
    /// respuesta en otro datagrama; `pick` extrae el valor de cada datagrama largo (`None` = no es
    /// la respuesta esperada). Es idempotente, así que se reintenta ante NAK o silencio.
    fn query<T>(
        &mut self,
        datagram: &[u8],
        mut pick: impl FnMut(&Self, &[u8]) -> Option<T>,
    ) -> Result<T, SessionError> {
        let mut last = SessionError::Timeout { what: "lectura" };
        for _ in 0..=self.cfg.query_retries {
            self.link.drain()?;
            self.link.send(datagram)?;
            let deadline = Instant::now() + self.cfg.ack_timeout + self.cfg.reply_timeout;
            let mut nak = false;
            while !nak {
                let left = deadline.saturating_duration_since(Instant::now());
                let Some(dg) = self.link.recv(left)? else {
                    break;
                };
                if dg.len() == 1 {
                    match packet::interpret_response(dg[0], self.cfg.magic) {
                        Response::Nak | Response::Err => {
                            last = SessionError::Rejected {
                                response: packet::interpret_response(dg[0], self.cfg.magic),
                                packet: 0,
                            };
                            nak = true;
                        }
                        Response::Unknown(b) => {
                            return Err(SessionError::Protocol(format!(
                                "respuesta inesperada 0x{b:02X}; ¿el magic (0x{:02X}) es el de \
                                 esta máquina?",
                                self.cfg.magic.0
                            )));
                        }
                        Response::Ack | Response::Enq => {}
                    }
                    continue;
                }
                if let Some(v) = pick(self, &dg) {
                    return Ok(v);
                }
            }
            if !nak {
                last = SessionError::Timeout {
                    what: "respuesta de lectura",
                };
            }
        }
        Err(last)
    }

    /// Busca la respuesta `DA 01 dirección valor` de `address` en un datagrama de respuesta.
    pub(super) fn find_reply(&self, datagram: &[u8], address: u16) -> Option<u64> {
        let plain = self.cfg.magic.unswizzle_all(datagram);
        let cmds = cmd::decode_stream(&plain).ok()?;
        cmds.into_iter().find_map(|c| match c {
            Command::MemWrite { addr, values } if addr == address && values.len() == 1 => {
                Some(values[0])
            }
            _ => None,
        })
    }

    /// Estado, movimiento y posición (cuatro lecturas de solo lectura).
    pub fn status(&mut self) -> Result<MachineStatus, SessionError> {
        let raw = self.read_mem(addr::STATUS)?;
        let x = signed35(self.read_mem(addr::POS_X)?);
        let y = signed35(self.read_mem(addr::POS_Y)?);
        let bed = self.bed_mm();
        let pos_mm = self
            .cfg
            .home
            .from_machine(x as f64 / 1000.0, y as f64 / 1000.0, bed);
        Ok(MachineStatus {
            raw,
            running: raw & BIT_RUNNING != 0,
            paused: raw & BIT_PAUSED != 0,
            moving: raw & BIT_MOVING != 0,
            pos_um: (x, y),
            pos_mm,
        })
    }

    /// Tamaño de la cama que informa la máquina (µm).
    pub fn read_bed_um(&mut self) -> Result<[i64; 2], SessionError> {
        let bx = signed35(self.read_mem(addr::BED_X)?);
        let by = signed35(self.read_mem(addr::BED_Y)?);
        self.bed_um = Some([bx, by]);
        Ok([bx, by])
    }

    /// Información de la controladora: sondeo más los ajustes y contadores que se conocen. Solo
    /// lecturas (`DA 00`). Un valor que no contesta queda en `None`; tras el primer silencio no
    /// se insiste con los demás.
    pub fn controller_info(&mut self) -> Result<ControllerInfo, SessionError> {
        let probe = self.probe()?;
        let mut silent = false;
        let mut read = |s: &mut Self, a: u16| -> Option<u64> {
            if silent {
                return None;
            }
            match s.read_mem(a) {
                Ok(v) => Some(v),
                Err(SessionError::Timeout { .. }) => {
                    silent = true;
                    None
                }
                Err(_) => None,
            }
        };
        // Un 0 es "la máquina no informa ese dato", no una velocidad de 0.
        let nonzero = |v: Option<u64>| v.filter(|&v| v > 0);
        let per_mille = |v: Option<u64>| nonzero(v).map(|v| v as f64 / 1000.0);
        let rapid_mm_s = per_mille(read(self, addr::G0_SPEED));
        let home_mm_s = per_mille(read(self, addr::HOME_SPEED));
        let axis_max_mm_s = [
            per_mille(read(self, addr::AXIS_MAX_SPEED_X)),
            per_mille(read(self, addr::AXIS_MAX_SPEED_Y)),
        ];
        let axis_accel_mm_s2 = [
            per_mille(read(self, addr::AXIS_MAX_ACCEL_X)),
            per_mille(read(self, addr::AXIS_MAX_ACCEL_Y)),
        ];
        let total_on_s = read(self, addr::TOTAL_ON_TIME);
        let total_work_s = read(self, addr::TOTAL_WORK_TIME);
        let total_jobs = read(self, addr::TOTAL_JOBS);
        let total_laser_s = read(self, addr::TOTAL_LASER_TIME);
        let travel_m = [read(self, addr::TRAVEL_X), read(self, addr::TRAVEL_Y)];
        let documents = read(self, addr::DOC_COUNT).map(|v| v.min(u64::from(u32::MAX)) as u32);
        Ok(ControllerInfo {
            probe,
            rapid_mm_s,
            home_mm_s,
            axis_max_mm_s,
            axis_accel_mm_s2,
            total_on_s,
            total_work_s,
            total_jobs,
            total_laser_s,
            travel_m,
            documents,
        })
    }

    /// Lista los documentos guardados en la memoria: cuenta (`0x205`) y nombre de cada uno con
    /// `E8 01 n`. Solo lectura. **EXPERIMENTAL**: la numeración (desde 1) y el formato de la
    /// respuesta (`E8 01 n nombre\0`) salen del emulador de MeerK40t, hecho a partir de capturas
    /// de RDWorks; no se han visto en una máquina real. No hay borrar ni ejecutar a propósito
    /// (`E8 00` con 0 borra todo y `E8 03` selecciona sin garantía de arrancar).
    pub fn list_documents(&mut self) -> Result<Vec<ControllerFile>, SessionError> {
        let count = self.read_mem(addr::DOC_COUNT)?;
        let n = count.min(u64::from(MAX_LISTED_DOCS)) as u32;
        let mut files = Vec::with_capacity(n as usize);
        let mut silent = false;
        for index in 1..=n {
            let name = if silent {
                None
            } else {
                match self.read_document_name(index) {
                    Ok(name) => Some(name),
                    Err(SessionError::Timeout { .. }) => {
                        silent = true;
                        None
                    }
                    Err(_) => None,
                }
            };
            files.push(ControllerFile { index, name });
        }
        Ok(files)
    }

    fn read_document_name(&mut self, index: u32) -> Result<String, SessionError> {
        let idx = [((index >> 7) & 0x7F) as u8, (index & 0x7F) as u8];
        let d = packet::build_packet(&[0xE8, 0x01, idx[0], idx[1]], &self.pcfg());
        self.query(&d, |s, dg| {
            let plain = s.cfg.magic.unswizzle_all(dg);
            let at = plain.iter().position(|&b| b == 0xE8)?;
            let body = plain.get(at..)?;
            if body.len() < 4 || body[1] != 0x01 || body[2] != idx[0] || body[3] != idx[1] {
                return None;
            }
            let name: String = body[4..]
                .iter()
                .take_while(|&&b| b != 0)
                .map(|&b| {
                    if (0x20..0x7F).contains(&b) {
                        b as char
                    } else {
                        '?'
                    }
                })
                .collect();
            Some(name)
        })
    }
}
