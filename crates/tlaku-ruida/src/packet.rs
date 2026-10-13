//! Paquetes UDP del puerto 50200: 2 bytes de checksum (big endian) y datos swizzleados.
//!
//! Un paquete es un datagrama. La longitud de los datos sale del tamaño del datagrama.
//! Los paquetes se cortan siempre en límite de comando (ruida.md sección 1.1).
//!
//! Dudas de las fuentes, todas configurables aquí:
//! - el checksum puede ir sobre los bytes antes o después del swizzle ([`ChecksumMode`]);
//! - el tamaño máximo del datagrama (1472 o 1474 B según la fuente; en la práctica ~1000 B de
//!   datos);
//! - los bytes de ACK y NAK (`C6`/`46` en el cable según FabLab, `CC`/`CF` según EduTech).

use crate::cmd::{self, DecodeError};
use crate::swizzle::Magic;
use std::fmt;

/// Máximo de bytes de datos por paquete que permite este módulo: 1472 B del datagrama (la cifra
/// menor de las fuentes) menos los 2 del checksum.
pub const MAX_DATA_BYTES_LIMIT: usize = 1470;
/// Tamaño por defecto de los datos de un paquete (recomendación de ruida.md: ~1000 B).
pub const DEFAULT_MAX_DATA_BYTES: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChecksumMode {
    /// Suma de los bytes ya swizzleados ([FAB], [JNW]).
    #[default]
    PostSwizzle,
    /// Suma de los bytes antes del swizzle ([EDU]).
    PreSwizzle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PacketConfig {
    pub magic: Magic,
    pub checksum: ChecksumMode,
    /// Bytes de datos por paquete (sin los 2 del checksum), entre 1 y [`MAX_DATA_BYTES_LIMIT`].
    pub max_data_bytes: usize,
}

impl Default for PacketConfig {
    fn default() -> Self {
        PacketConfig {
            magic: Magic::default(),
            checksum: ChecksumMode::default(),
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
            PacketError::BadConfig(w) => write!(f, "configuración de paquetes inválida: {w}"),
            PacketError::CommandTooLarge { offset, len, max } => write!(
                f,
                "el comando en el byte {offset} ocupa {len} bytes y el paquete admite {max}"
            ),
            PacketError::Decode(e) => write!(f, "{e}"),
            PacketError::TooShort(n) => write!(f, "datagrama de {n} bytes: falta el checksum"),
            PacketError::TooLong { len, max } => {
                write!(f, "datagrama de {len} bytes, el máximo es {max}")
            }
            PacketError::ChecksumMismatch { expected, found } => write!(
                f,
                "checksum 0x{found:04X} en el paquete y 0x{expected:04X} calculado"
            ),
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

/// Checksum de un paquete según el modo, a partir de los datos en claro.
pub fn packet_checksum(plain: &[u8], cfg: &PacketConfig) -> u16 {
    match cfg.checksum {
        ChecksumMode::PreSwizzle => checksum16(plain),
        ChecksumMode::PostSwizzle => checksum16(&cfg.magic.swizzle_all(plain)),
    }
}

/// Construye un datagrama con un trozo de flujo en claro.
pub fn build_packet(plain: &[u8], cfg: &PacketConfig) -> Vec<u8> {
    let mut out = Vec::with_capacity(plain.len() + 2);
    out.extend(packet_checksum(plain, cfg).to_be_bytes());
    out.extend(cfg.magic.swizzle_all(plain));
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
    let plain = cfg.magic.unswizzle_all(&datagram[2..]);
    let expected = packet_checksum(&plain, cfg);
    if expected != found {
        return Err(PacketError::ChecksumMismatch { expected, found });
    }
    Ok(plain)
}

/// Interpretación de un byte de respuesta de la controladora.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Response {
    /// Paquete aceptado: enviar el siguiente.
    Ack,
    /// Checksum rechazado: reintentar (el primer paquete) o abortar.
    Nak,
    /// Cualquier otro valor.
    Unknown(u8),
}

/// ACK: `C6` (cable según FabLab) y `CC` (EduTech). NAK: `46`, `48`, `CD`, `CF` (`48` y `46` son
/// `CF` y `CD` swizzleados con magic 0x88). SIN CONFIRMAR con hardware.
pub fn interpret_response(b: u8) -> Response {
    match b {
        0xC6 | 0xCC => Response::Ack,
        0x46 | 0x48 | 0xCD | 0xCF => Response::Nak,
        other => Response::Unknown(other),
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
    fn checksum_vectors_v7_v8() {
        // V7: consulta del ID de tarjeta; los dos modos dan sumas distintas (el conflicto).
        let q = [0xDA, 0x00, 0x05, 0x7E];
        let pre = PacketConfig {
            checksum: ChecksumMode::PreSwizzle,
            ..Default::default()
        };
        let post = PacketConfig::default();
        assert_eq!(packet_checksum(&q, &pre), 0x015D);
        assert_eq!(packet_checksum(&q, &post), 0x0261);
        assert_eq!(build_packet(&q, &pre), [0x01, 0x5D, 0xD4, 0x89, 0x0D, 0xF7]);
        assert_eq!(
            build_packet(&q, &post),
            [0x02, 0x61, 0xD4, 0x89, 0x0D, 0xF7]
        );
        // V8: cabecera del ejemplo 1.
        let h = hex("d810 f0 e601 f10200 d800");
        assert_eq!(packet_checksum(&h, &pre), 0x048A);
        assert_eq!(packet_checksum(&h, &post), 0x0647);
    }

    #[test]
    fn split_is_on_command_boundaries_and_reassembles() {
        let stream =
            hex("d810 f0 e601 f10200 d800 8800001116500000021256 a800000b51120000021256 d7");
        for mode in [ChecksumMode::PreSwizzle, ChecksumMode::PostSwizzle] {
            for max in [11, 12, 16, 25, 1000] {
                let cfg = PacketConfig {
                    checksum: mode,
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
    fn parse_detects_corruption_and_wrong_mode() {
        let stream = hex("d810 f0 d7");
        let post = PacketConfig::default();
        let pre = PacketConfig {
            checksum: ChecksumMode::PreSwizzle,
            ..Default::default()
        };
        let p = build_packet(&stream, &post);
        assert!(parse_packet(&p, &post).is_ok());
        assert!(matches!(
            parse_packet(&p, &pre),
            Err(PacketError::ChecksumMismatch { .. })
        ));
        let mut bad = p.clone();
        bad[3] ^= 0x01;
        assert!(matches!(
            parse_packet(&bad, &post),
            Err(PacketError::ChecksumMismatch { .. })
        ));
        assert!(matches!(
            parse_packet(&[1], &post),
            Err(PacketError::TooShort(1))
        ));
        let long = vec![0u8; 2000];
        assert!(matches!(
            parse_packet(&long, &post),
            Err(PacketError::TooLong { .. })
        ));
    }

    #[test]
    fn checksum_wraps_at_16_bits() {
        let data = vec![0xFFu8; 1000];
        assert_eq!(checksum16(&data), (255u32 * 1000 % 65536) as u16);
    }

    #[test]
    fn responses() {
        for b in [0xC6, 0xCC] {
            assert_eq!(interpret_response(b), Response::Ack);
        }
        for b in [0x46, 0x48, 0xCD, 0xCF] {
            assert_eq!(interpret_response(b), Response::Nak);
        }
        assert_eq!(interpret_response(0x00), Response::Unknown(0));
    }
}
