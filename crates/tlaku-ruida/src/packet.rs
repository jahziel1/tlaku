//! Paquetes UDP del puerto 50200: 2 bytes de checksum (big endian) y datos swizzleados.
//!
//! Un paquete es un datagrama. La longitud de los datos sale del tamaño del datagrama.
//! Los paquetes se cortan siempre en límite de comando (ruida.md sección 1.1).
//!
//! Resuelto con las fuentes (ruida.md sección 12):
//! - el checksum es la suma de los bytes **ya swizzleados** (capturas reales de Nova 35 y de
//!   LightBurn; MeerK40t y ruida-pa lo calculan así). No hay modo alternativo;
//! - la respuesta de la máquina es un byte lógico (`CC` ACK, `CF` checksum erróneo, `CD` error,
//!   `CE` keep-alive) que viaja swizzleado con el magic de la máquina: `C6` para 0x88. EduTech
//!   da el byte lógico y FabLab el del cable;
//! - tamaño: MeerK40t corta en ~1000 B en límite de comando y la máquina acepta hasta 1472 B de
//!   datagrama (1470 de datos). Aquí 1000 por defecto y 1470 como tope.

use crate::cmd::{self, DecodeError};
use crate::swizzle::Magic;
use crate::text;
use std::fmt;
use tlaku_i18n::tr;

/// Máximo de bytes de datos por paquete que permite este módulo: 1472 B del datagrama menos los
/// 2 del checksum.
pub const MAX_DATA_BYTES_LIMIT: usize = 1470;
/// Tamaño por defecto de los datos de un paquete (MeerK40t y ruida-pa: ~1000 B).
pub const DEFAULT_MAX_DATA_BYTES: usize = 1000;

/// ACK: el paquete llegó bien.
pub const ACK: u8 = 0xCC;
/// ERR: la máquina detectó un problema con el mensaje.
pub const ERR: u8 = 0xCD;
/// ENQ: keep-alive.
pub const ENQ: u8 = 0xCE;
/// NAK: el checksum no coincide; reenviar el paquete.
pub const NAK: u8 = 0xCF;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PacketConfig {
    pub magic: Magic,
    /// Bytes de datos por paquete (sin los 2 del checksum), entre 1 y [`MAX_DATA_BYTES_LIMIT`].
    pub max_data_bytes: usize,
}

impl Default for PacketConfig {
    fn default() -> Self {
        PacketConfig {
            magic: Magic::default(),
            max_data_bytes: DEFAULT_MAX_DATA_BYTES,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PacketError {
    BadConfig(&'static str),
    /// Un solo comando no cabe en un paquete.
    CommandTooLarge {
        offset: usize,
        len: usize,
        max: usize,
    },
    Decode(DecodeError),
    /// El datagrama no tiene ni los 2 bytes del checksum.
    TooShort(usize),
    /// El datagrama supera el tamaño máximo.
    TooLong {
        len: usize,
        max: usize,
    },
    ChecksumMismatch {
        expected: u16,
        found: u16,
    },
}

impl fmt::Display for PacketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PacketError::BadConfig(w) => {
                f.write_str(&tr!("ruida.packet.bad_config", what = text::what(w)))
            }
            PacketError::CommandTooLarge { offset, len, max } => f.write_str(&tr!(
                "ruida.packet.command_too_large",
                offset = offset,
                len = len,
                max = max
            )),
            PacketError::Decode(e) => write!(f, "{e}"),
            PacketError::TooShort(n) => f.write_str(&tr!("ruida.packet.too_short", n = n)),
            PacketError::TooLong { len, max } => {
                f.write_str(&tr!("ruida.packet.too_long", len = len, max = max))
            }
            PacketError::ChecksumMismatch { expected, found } => f.write_str(&tr!(
                "ruida.packet.checksum",
                found = format!("{found:04X}"),
                expected = format!("{expected:04X}")
            )),
        }
    }
}

impl std::error::Error for PacketError {}

impl From<DecodeError> for PacketError {
    fn from(e: DecodeError) -> Self {
        PacketError::Decode(e)
    }
}

impl PacketConfig {
    pub fn validate(&self) -> Result<(), PacketError> {
        if self.max_data_bytes == 0 || self.max_data_bytes > MAX_DATA_BYTES_LIMIT {
            return Err(PacketError::BadConfig(
                "max_data_bytes debe estar entre 1 y 1470",
            ));
        }
        Ok(())
    }

    /// Tamaño máximo del datagrama completo.
    pub fn max_datagram(&self) -> usize {
        self.max_data_bytes + 2
    }
}

/// Los 16 bits bajos de la suma de los valores de los bytes.
pub fn checksum16(data: &[u8]) -> u16 {
    data.iter()
        .fold(0u16, |acc, &b| acc.wrapping_add(u16::from(b)))
}

/// Checksum de un paquete a partir de los datos en claro: suma de los bytes swizzleados.
pub fn packet_checksum(plain: &[u8], cfg: &PacketConfig) -> u16 {
    checksum16(&cfg.magic.swizzle_all(plain))
}

/// Construye un datagrama con un trozo de flujo en claro.
pub fn build_packet(plain: &[u8], cfg: &PacketConfig) -> Vec<u8> {
    let wire = cfg.magic.swizzle_all(plain);
    let mut out = Vec::with_capacity(wire.len() + 2);
    out.extend(checksum16(&wire).to_be_bytes());
    out.extend(wire);
    out
}

/// Parte un flujo en claro en datagramas, cortando solo en límite de comando.
pub fn split_into_packets(stream: &[u8], cfg: &PacketConfig) -> Result<Vec<Vec<u8>>, PacketError> {
    cfg.validate()?;
    let mut packets = Vec::new();
    let mut start = 0usize;
    let mut end = 0usize;
    for r in cmd::command_ranges(stream)? {
        if r.len() > cfg.max_data_bytes {
            return Err(PacketError::CommandTooLarge {
                offset: r.start,
                len: r.len(),
                max: cfg.max_data_bytes,
            });
        }
        if r.end - start > cfg.max_data_bytes {
            packets.push(build_packet(&stream[start..end], cfg));
            start = end;
        }
        end = r.end;
    }
    if end > start {
        packets.push(build_packet(&stream[start..end], cfg));
    }
    Ok(packets)
}

/// Verifica el checksum de un datagrama y devuelve sus datos en claro.
pub fn parse_packet(datagram: &[u8], cfg: &PacketConfig) -> Result<Vec<u8>, PacketError> {
    if datagram.len() < 2 {
        return Err(PacketError::TooShort(datagram.len()));
    }
    if datagram.len() > cfg.max_datagram() {
        return Err(PacketError::TooLong {
            len: datagram.len(),
            max: cfg.max_datagram(),
        });
    }
    let found = u16::from_be_bytes([datagram[0], datagram[1]]);
    let expected = checksum16(&datagram[2..]);
    if expected != found {
        return Err(PacketError::ChecksumMismatch { expected, found });
    }
    Ok(cfg.magic.unswizzle_all(&datagram[2..]))
}

/// Interpretación de un byte de respuesta de la controladora.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Response {
    /// Paquete aceptado: enviar el siguiente.
    Ack,
    /// Checksum rechazado: el paquete no se aplicó, se puede reenviar tal cual.
    Nak,
    /// La máquina detectó un problema con el mensaje (FabLab: reintentar solo el primer
    /// paquete; en los siguientes, abortar).
    Err,
    /// Keep-alive.
    Enq,
    /// Cualquier otro valor (también un magic equivocado).
    Unknown(u8),
}

impl Response {
    /// Byte lógico de la respuesta (`CC`, `CF`, `CD`, `CE`).
    pub fn logical(self) -> Option<u8> {
        match self {
            Response::Ack => Some(ACK),
            Response::Nak => Some(NAK),
            Response::Err => Some(ERR),
            Response::Enq => Some(ENQ),
            Response::Unknown(_) => None,
        }
    }

    /// Byte tal como viaja por el cable con el magic dado (`C6` para ACK con 0x88).
    pub fn wire(self, magic: Magic) -> Option<u8> {
        self.logical().map(|b| magic.swizzle(b))
    }
}

/// Interpreta el byte de respuesta **del cable**: se des-swizzlea con el magic y se compara con
/// los bytes lógicos. ACK `C6`, ERR `46`, NAK `48` y ENQ `47` con el magic 0x88.
pub fn interpret_response(wire: u8, magic: Magic) -> Response {
    match magic.unswizzle(wire) {
        ACK => Response::Ack,
        NAK => Response::Nak,
        ERR => Response::Err,
        ENQ => Response::Enq,
        _ => Response::Unknown(wire),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        let s: String = s.split_whitespace().collect();
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn real_capture_checksum_is_over_swizzled_bytes() {
        // Captura de LightBurn publicada en el README de ruida-pa
        // (https://github.com/StevenIsaacs/ruida-pa, licencia MIT, Copyright (c) 2025 Steve
        // Isaacs; ver THIRD_PARTY.md):
        // el datagrama `0261 d4890df7` es `DA 00 05 7E` con magic 0x88. El checksum es la suma
        // de los bytes del cable (0xD4+0x89+0x0D+0xF7 = 0x0261), no de los datos en claro
        // (0x015D). Con eso queda resuelto el conflicto entre EduTech y FabLab.
        let q = [0xDA, 0x00, 0x05, 0x7E];
        let cfg = PacketConfig::default();
        assert_eq!(build_packet(&q, &cfg), hex("0261 d4890df7"));
        assert_eq!(packet_checksum(&q, &cfg), 0x0261);
        assert_eq!(checksum16(&q), 0x015D);
        assert_eq!(parse_packet(&hex("0261 d4890df7"), &cfg).unwrap(), q);
        // V8: cabecera del ejemplo 1.
        let h = hex("d810 f0 e601 f10200 d800");
        assert_eq!(packet_checksum(&h, &cfg), 0x0647);
    }

    #[test]
    fn split_is_on_command_boundaries_and_reassembles() {
        let stream =
            hex("d810 f0 e601 f10200 d800 8800001116500000021256 a800000b51120000021256 d7");
        for max in [11, 12, 16, 25, 1000] {
            let cfg = PacketConfig {
                max_data_bytes: max,
                ..Default::default()
            };
            let packets = split_into_packets(&stream, &cfg).unwrap();
            let mut joined = Vec::new();
            for p in &packets {
                assert!(p.len() <= cfg.max_datagram());
                let plain = parse_packet(p, &cfg).unwrap();
                // Cada paquete es un flujo de comandos completo.
                cmd::decode_stream(&plain).unwrap();
                joined.extend(plain);
            }
            assert_eq!(joined, stream, "max {max}");
        }
        let one = split_into_packets(&stream, &PacketConfig::default()).unwrap();
        assert_eq!(one.len(), 1);
    }

    #[test]
    fn command_larger_than_packet_is_an_error() {
        let stream = hex("8800001116500000021256");
        let cfg = PacketConfig {
            max_data_bytes: 10,
            ..Default::default()
        };
        assert!(matches!(
            split_into_packets(&stream, &cfg),
            Err(PacketError::CommandTooLarge {
                len: 11,
                max: 10,
                ..
            })
        ));
    }

    #[test]
    fn config_is_validated() {
        for bad in [0, 1471, 5000] {
            let cfg = PacketConfig {
                max_data_bytes: bad,
                ..Default::default()
            };
            assert!(matches!(
                split_into_packets(&[0xD7], &cfg),
                Err(PacketError::BadConfig(_))
            ));
        }
        let ok = PacketConfig {
            max_data_bytes: 1470,
            ..Default::default()
        };
        assert!(split_into_packets(&[0xD7], &ok).is_ok());
    }

    #[test]
    fn parse_detects_corruption() {
        let stream = hex("d810 f0 d7");
        let cfg = PacketConfig::default();
        let p = build_packet(&stream, &cfg);
        assert!(parse_packet(&p, &cfg).is_ok());
        let mut bad = p.clone();
        bad[3] ^= 0x01;
        assert!(matches!(
            parse_packet(&bad, &cfg),
            Err(PacketError::ChecksumMismatch { .. })
        ));
        assert!(matches!(
            parse_packet(&[1], &cfg),
            Err(PacketError::TooShort(1))
        ));
        let long = vec![0u8; 2000];
        assert!(matches!(
            parse_packet(&long, &cfg),
            Err(PacketError::TooLong { .. })
        ));
    }

    #[test]
    fn a_wrong_magic_is_not_caught_by_the_checksum() {
        // La suma se hace sobre los bytes del cable, así que con otro magic el paquete se
        // acepta y lo que cambia es la interpretación (por eso el sondeo usa el ACK).
        let q = [0xDA, 0x00, 0x05, 0x7E];
        let sender = PacketConfig {
            magic: Magic(0x11),
            ..Default::default()
        };
        let p = build_packet(&q, &sender);
        let receiver = PacketConfig::default();
        let got = parse_packet(&p, &receiver).unwrap();
        assert_ne!(got, q);
    }

    #[test]
    fn checksum_wraps_at_16_bits() {
        let data = vec![0xFFu8; 1000];
        assert_eq!(checksum16(&data), (255u32 * 1000 % 65536) as u16);
    }

    #[test]
    fn responses_are_logical_bytes_swizzled_with_the_magic() {
        let m = Magic::default();
        // Cable con 0x88: C6 = ACK (FabLab), 46 = ERR, 48 = NAK, C8 = ENQ.
        assert_eq!(interpret_response(0xC6, m), Response::Ack);
        assert_eq!(interpret_response(0x46, m), Response::Err);
        assert_eq!(interpret_response(0x48, m), Response::Nak);
        assert_eq!(interpret_response(0xC8, m), Response::Enq);
        assert_eq!(interpret_response(0x00, m), Response::Unknown(0));
        // Con otro magic cambian los bytes del cable pero no el significado.
        for magic in [Magic(0x11), Magic(0x38), Magic(0x77)] {
            for r in [Response::Ack, Response::Nak, Response::Err, Response::Enq] {
                let w = r.wire(magic).unwrap();
                assert_eq!(interpret_response(w, magic), r);
            }
        }
        assert_eq!(Response::Ack.wire(m), Some(0xC6));
        assert_eq!(Response::Unknown(3).wire(m), None);
    }
}
