//! Controladora Ruida simulada, solo para pruebas (feature `emu` o `cargo test`).
//!
//! Recibe datagramas, verifica el checksum, responde un byte de ACK o NAK, des-swizzlea,
//! acumula el flujo, lo decodifica y reconstruye el trabajo para compararlo con el original.
//! No implementa red ni lectura de memoria: las consultas `DA 00` solo se registran.

use crate::cmd::{self, Command, DecodeError, Limit};
use crate::num;
use crate::packet::{self, PacketConfig};
use tlaku_core::Point;
use tlaku_core::toolpath::{Move, Toolpath, ToolpathBlock};

/// Bytes de ACK/NAK que responde el emulador. Las fuentes discrepan (ruida.md sección 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AckStyle {
    /// `C6` / `46`, según FabLab (bytes del cable).
    #[default]
    Wire,
    /// `CC` / `CF`, según EduTech.
    Plain,
}

impl AckStyle {
    pub fn ack(self) -> u8 {
        match self {
            AckStyle::Wire => 0xC6,
            AckStyle::Plain => 0xCC,
        }
    }

    pub fn nak(self) -> u8 {
        match self {
            AckStyle::Wire => 0x46,
            AckStyle::Plain => 0xCF,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EmuConfig {
    pub packets: PacketConfig,
    pub ack: AckStyle,
}

/// Tramo recorrido por el cabezal (µm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub from: (i64, i64),
    pub to: (i64, i64),
    pub laser_on: bool,
    pub layer: u8,
    pub speed_um_s: u64,
    /// Potencia máxima del láser 1 vigente (crudo de 14 bits).
    pub power_max: u16,
}

#[derive(Debug, Default)]
pub struct Emulator {
    cfg: EmuConfig,
    stream: Vec<u8>,
    /// Paquetes aceptados y rechazados.
    pub accepted: usize,
    pub rejected: usize,
}

impl Emulator {
    pub fn new(cfg: EmuConfig) -> Self {
        Emulator {
            cfg,
            ..Default::default()
        }
    }

    /// Procesa un datagrama y devuelve el byte de respuesta. Un paquete con checksum erróneo
    /// (o de tamaño inválido) no se acumula y recibe NAK.
    pub fn receive(&mut self, datagram: &[u8]) -> u8 {
        match packet::parse_packet(datagram, &self.cfg.packets) {
            Ok(plain) => {
                self.stream.extend(plain);
                self.accepted += 1;
                self.cfg.ack.ack()
            }
            Err(_) => {
                self.rejected += 1;
                self.cfg.ack.nak()
            }
        }
    }

    /// Flujo en claro recibido hasta ahora.
    pub fn stream(&self) -> &[u8] {
        &self.stream
    }

    /// `true` si el flujo recibido termina en `D7` (fin de archivo).
    pub fn job_complete(&self) -> bool {
        self.stream.last() == Some(&0xD7)
    }

    pub fn commands(&self) -> Result<Vec<Command>, DecodeError> {
        cmd::decode_stream(&self.stream)
    }

    /// Direcciones de memoria consultadas con `DA 00` (no se responde).
    pub fn read_requests(&self) -> Result<Vec<u16>, DecodeError> {
        Ok(self
            .commands()?
            .into_iter()
            .filter_map(|c| match c {
                Command::MemRead { addr } => Some(addr),
                _ => None,
            })
            .collect())
    }

    /// Recorre los comandos como lo haría el cabezal: posición (sumando relativos), capa,
    /// velocidad y potencia vigentes.
    pub fn replay(&self) -> Result<Vec<Segment>, DecodeError> {
        Ok(replay(&self.commands()?))
    }

    /// Reconstruye un `Toolpath` (un bloque por capa, en mm) para compararlo con el original.
    /// La velocidad sale de `C9 02` y la potencia de `C6 02` (láser 1).
    pub fn to_toolpath(&self) -> Result<Toolpath, DecodeError> {
        Ok(to_toolpath(&replay(&self.commands()?)))
    }
}

/// Ejecuta los comandos sobre un cabezal virtual que parte de (0, 0).
pub fn replay(cmds: &[Command]) -> Vec<Segment> {
    let mut out = Vec::new();
    let (mut x, mut y) = (0i64, 0i64);
    let (mut layer, mut speed, mut power) = (0u8, 0u64, 0u16);
    for c in cmds {
        let (nx, ny, on) = match *c {
            Command::MoveAbs { x, y } => (x, y, false),
            Command::CutAbs { x, y } => (x, y, true),
            Command::MoveRel { dx, dy } => (x + i64::from(dx), y + i64::from(dy), false),
            Command::CutRel { dx, dy } => (x + i64::from(dx), y + i64::from(dy), true),
            Command::MoveRelX(d) => (x + i64::from(d), y, false),
            Command::CutRelX(d) => (x + i64::from(d), y, true),
            Command::MoveRelY(d) => (x, y + i64::from(d), false),
            Command::CutRelY(d) => (x, y + i64::from(d), true),
            Command::LayerSelect(l) => {
                layer = l;
                continue;
            }
            Command::Speed(v) => {
                speed = v;
                continue;
            }
            Command::LaserPower {
                laser: 1,
                limit: Limit::Max,
                raw,
            } => {
                power = raw;
                continue;
            }
            _ => continue,
        };
        out.push(Segment {
            from: (x, y),
            to: (nx, ny),
            laser_on: on,
            layer,
            speed_um_s: speed,
            power_max: power,
        });
        (x, y) = (nx, ny);
    }
    out
}

fn to_toolpath(segments: &[Segment]) -> Toolpath {
    let mut blocks: Vec<(u8, ToolpathBlock)> = Vec::new();
    for s in segments {
        if blocks.last().map(|b| b.0) != Some(s.layer) {
            blocks.push((
                s.layer,
                ToolpathBlock {
                    speed_mm_min: num::speed_to_mm_min(s.speed_um_s),
                    ..Default::default()
                },
            ));
        }
        let to = Point::new(s.to.0 as f64 / 1000.0, s.to.1 as f64 / 1000.0);
        let mv = if s.laser_on {
            Move::Cut {
                to,
                power_pct: num::power_to_pct(s.power_max),
            }
        } else {
            Move::Travel { to }
        };
        blocks.last_mut().unwrap().1.moves.push(mv);
    }
    Toolpath {
        blocks: blocks.into_iter().map(|b| b.1).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::{ChecksumMode, split_into_packets};
    use crate::rd::{self, LayerOptions, RuidaJobOptions};
    use crate::swizzle::Magic;

    fn demo_toolpath() -> Toolpath {
        let mut blocks = Vec::new();
        for (i, (speed, power)) in [(6000.0, 40.0), (1800.0, 80.0)].into_iter().enumerate() {
            let o = 20.0 * i as f64;
            let c = |x: f64, y: f64| Move::Cut {
                to: Point::new(x + o, y + o),
                power_pct: power,
            };
            let mut moves = vec![Move::Travel {
                to: Point::new(10.0 + o, 10.0 + o),
            }];
            // Muchos segmentos para forzar varios paquetes.
            for k in 0..200 {
                let a = 1.0 + (k % 7) as f64 * 0.37;
                moves.push(c(10.0 + a, 10.0 + (k as f64) * 0.05));
                moves.push(c(30.0 - a, 10.0 + (k as f64) * 0.05 + 0.02));
            }
            blocks.push(ToolpathBlock {
                speed_mm_min: speed,
                air_assist: i == 0,
                moves,
                ..Default::default()
            });
        }
        Toolpath { blocks }
    }

    fn opts() -> RuidaJobOptions {
        RuidaJobOptions {
            layers: vec![LayerOptions::default(), LayerOptions::default()],
            ..Default::default()
        }
    }

    fn send_all(emu: &mut Emulator, packets: &[Vec<u8>]) {
        for p in packets {
            assert_eq!(emu.receive(p), emu.cfg.ack.ack());
        }
    }

    #[test]
    fn full_job_through_packets_is_reconstructed() {
        let tp = demo_toolpath();
        let plain = rd::build_rd(&tp, &opts()).unwrap();
        for mode in [ChecksumMode::PostSwizzle, ChecksumMode::PreSwizzle] {
            for ack in [AckStyle::Wire, AckStyle::Plain] {
                let packets_cfg = PacketConfig {
                    checksum: mode,
                    max_data_bytes: 300,
                    magic: Magic(0x88),
                };
                let packets = split_into_packets(&plain, &packets_cfg).unwrap();
                assert!(packets.len() > 3, "debe haber varios paquetes");
                let mut emu = Emulator::new(EmuConfig {
                    packets: packets_cfg,
                    ack,
                });
                send_all(&mut emu, &packets);
                assert_eq!(emu.stream(), &plain[..]);
                assert!(emu.job_complete());
                assert_eq!(emu.rejected, 0);
                // Reconstrucción geométrica: mismas posiciones en µm y misma cantidad de tramos.
                let back = emu.to_toolpath().unwrap();
                assert_eq!(back.blocks.len(), 2);
                for (orig, got) in tp.blocks.iter().zip(&back.blocks) {
                    assert!((orig.speed_mm_min - got.speed_mm_min).abs() < 0.01);
                    assert_eq!(orig.moves.len(), got.moves.len());
                    for (a, b) in orig.moves.iter().zip(&got.moves) {
                        assert!((a.to().x - b.to().x).abs() < 0.0006);
                        assert!((a.to().y - b.to().y).abs() < 0.0006);
                        assert_eq!(matches!(a, Move::Cut { .. }), matches!(b, Move::Cut { .. }));
                    }
                }
                let (Move::Cut { power_pct: p0, .. }, Move::Cut { power_pct: p1, .. }) =
                    (back.blocks[0].moves[1], back.blocks[1].moves[1])
                else {
                    panic!("se esperaban cortes");
                };
                assert!((p0 - 40.0).abs() < 0.01 && (p1 - 80.0).abs() < 0.01);
            }
        }
    }

    #[test]
    fn corrupted_or_wrong_mode_packet_gets_nak_and_is_not_stored() {
        let plain = rd::build_rd(&demo_toolpath(), &opts()).unwrap();
        let cfg = PacketConfig {
            max_data_bytes: 300,
            ..Default::default()
        };
        let packets = split_into_packets(&plain, &cfg).unwrap();
        let mut emu = Emulator::new(EmuConfig {
            packets: cfg,
            ack: AckStyle::Wire,
        });
        assert_eq!(emu.receive(&packets[0]), 0xC6);
        let mut bad = packets[1].clone();
        bad[5] ^= 0x10;
        assert_eq!(emu.receive(&bad), 0x46);
        assert_eq!(emu.rejected, 1);
        // Reintento del mismo paquete: aceptado.
        assert_eq!(emu.receive(&packets[1]), 0xC6);
        // Modo de checksum equivocado: NAK.
        let pre = PacketConfig {
            checksum: ChecksumMode::PreSwizzle,
            ..cfg
        };
        let other = split_into_packets(&plain, &pre).unwrap();
        assert_eq!(emu.receive(&other[2]), 0x46);
        // Datagrama demasiado grande: NAK.
        assert_eq!(emu.receive(&vec![0u8; 2000]), 0x46);
        assert_eq!(emu.accepted, 2);
        assert!(!emu.job_complete());
    }

    #[test]
    fn wrong_magic_is_rejected() {
        let plain = vec![0xDA, 0x00, 0x05, 0x7E];
        // Con checksum post-swizzle un magic equivocado pasaría sin detectarse (la suma se hace
        // sobre los bytes del cable), por eso aquí se usa el modo previo al swizzle.
        let pre = PacketConfig {
            checksum: ChecksumMode::PreSwizzle,
            ..Default::default()
        };
        let sender = PacketConfig {
            magic: Magic(0x11),
            ..pre
        };
        let mut emu = Emulator::new(EmuConfig {
            packets: pre,
            ..Default::default()
        });
        let p = split_into_packets(&plain, &sender).unwrap();
        // Con otro magic el checksum no cuadra casi seguro.
        assert_eq!(emu.receive(&p[0]), 0x46);
    }

    #[test]
    fn read_requests_are_recorded() {
        let q = vec![0xDA, 0x00, 0x05, 0x7E];
        let cfg = PacketConfig::default();
        let mut emu = Emulator::new(EmuConfig::default());
        for p in split_into_packets(&q, &cfg).unwrap() {
            emu.receive(&p);
        }
        assert_eq!(emu.read_requests().unwrap(), [0x2FE]);
    }
}
