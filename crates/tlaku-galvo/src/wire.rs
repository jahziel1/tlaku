//! Formato del cable de las tarjetas LMC (JCZ/BJJCZ): comandos de 12 bytes y tandas de 256.
//!
//! - Todo comando mide **12 bytes**: seis enteros de 16 bits little-endian (código y cinco
//!   parámetros); los no usados van a cero.
//! - Código con byte alto `0x80`: comando de **lista** (se guarda y se ejecuta cuando la lista
//!   corre). Byte alto `0x00`: comando **inmediato** (se ejecuta al llegar y la tarjeta contesta
//!   8 bytes).
//! - Las listas viajan en **tandas de 256 comandos** (3072 bytes) en una sola escritura; la última
//!   tanda se rellena con `listEndOfList`.
//!
//! Fuentes: galvoplotter (MIT, `galvo/controller.py`: `empty = nop * 0x100`, escritura de 0xC00
//! bytes) y MeerK40t `balormk/controller.py` (MIT); edutechwiki y Bryce Schroeder en prosa
//! (docs/hardware/galvo.md, sección 2.1). Confianza: alta para el formato, sin hardware.

use std::fmt;

/// Bytes por comando.
pub const CMD_LEN: usize = 12;
/// Comandos por tanda de lista.
pub const BATCH_CMDS: usize = 256;
/// Bytes por tanda (256 x 12 = 3072).
pub const BATCH_BYTES: usize = CMD_LEN * BATCH_CMDS;
/// Bytes de la respuesta a un comando inmediato (cuatro enteros de 16 bits).
pub const REPLY_LEN: usize = 8;
/// Bit alto del código que marca un comando de lista.
pub const LIST_FLAG: u16 = 0x8000;
/// Código con el que se rellena el final de la última tanda (`listEndOfList`, como galvoplotter).
pub const PAD_CODE: u16 = 0x8002;

/// Tipo de un comando según el byte alto de su código.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Byte alto `0x80`.
    List,
    /// Byte alto `0x00`.
    Immediate,
    /// Cualquier otro byte alto: no se conoce y nunca se envía.
    Unknown,
}

/// Un comando de 12 bytes sin interpretar: código y cinco parámetros.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Raw {
    pub code: u16,
    pub p: [u16; 5],
}

impl Raw {
    pub const fn new(code: u16, p: [u16; 5]) -> Self {
        Raw { code, p }
    }

    /// Comando sin parámetros.
    pub const fn bare(code: u16) -> Self {
        Raw { code, p: [0; 5] }
    }

    pub fn kind(&self) -> Kind {
        match self.code >> 8 {
            0x80 => Kind::List,
            0x00 => Kind::Immediate,
            _ => Kind::Unknown,
        }
    }

    pub fn to_bytes(&self) -> [u8; CMD_LEN] {
        let mut out = [0u8; CMD_LEN];
        out[0..2].copy_from_slice(&self.code.to_le_bytes());
        for (i, v) in self.p.iter().enumerate() {
            out[2 + 2 * i..4 + 2 * i].copy_from_slice(&v.to_le_bytes());
        }
        out
    }

    pub fn from_bytes(b: &[u8]) -> Result<Raw, WireError> {
        if b.len() != CMD_LEN {
            return Err(WireError::BadLength { got: b.len() });
        }
        let w = |i: usize| u16::from_le_bytes([b[2 * i], b[2 * i + 1]]);
        Ok(Raw {
            code: w(0),
            p: [w(1), w(2), w(3), w(4), w(5)],
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireError {
    /// Un comando debe medir exactamente 12 bytes.
    BadLength { got: usize },
    /// Los bytes de una tanda no son múltiplo de 12.
    NotMultipleOf12 { got: usize },
    /// Una lista no puede llevar comandos inmediatos ni de tipo desconocido.
    NotAListCommand { index: usize, code: u16 },
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WireError::BadLength { got } => {
                write!(f, "un comando mide {CMD_LEN} bytes y llegaron {got}")
            }
            WireError::NotMultipleOf12 { got } => {
                write!(f, "{got} bytes no son múltiplo de {CMD_LEN}")
            }
            WireError::NotAListCommand { index, code } => {
                write!(f, "el comando {index} (código {code:#06x}) no es de lista")
            }
        }
    }
}

impl std::error::Error for WireError {}

/// Cuántas tandas hacen falta para `n` comandos.
pub fn batch_count(n: usize) -> usize {
    n.div_ceil(BATCH_CMDS)
}

/// Parte una lista en tandas de 3072 bytes. La última se rellena con `listEndOfList`; una lista
/// vacía no genera ninguna tanda. Falla si algún comando no es de lista.
pub fn pack_batches(cmds: &[Raw]) -> Result<Vec<Vec<u8>>, WireError> {
    if let Some((index, c)) = cmds
        .iter()
        .enumerate()
        .find(|(_, c)| c.kind() != Kind::List)
    {
        return Err(WireError::NotAListCommand {
            index,
            code: c.code,
        });
    }
    let pad = Raw::bare(PAD_CODE).to_bytes();
    Ok(cmds
        .chunks(BATCH_CMDS)
        .map(|chunk| {
            let mut buf = Vec::with_capacity(BATCH_BYTES);
            for c in chunk {
                buf.extend_from_slice(&c.to_bytes());
            }
            while buf.len() < BATCH_BYTES {
                buf.extend_from_slice(&pad);
            }
            buf
        })
        .collect())
}

/// Lee una o varias tandas (múltiplo de 12 bytes). Nunca entra en pánico con bytes arbitrarios.
pub fn unpack(bytes: &[u8]) -> Result<Vec<Raw>, WireError> {
    if !bytes.len().is_multiple_of(CMD_LEN) {
        return Err(WireError::NotMultipleOf12 { got: bytes.len() });
    }
    bytes.chunks_exact(CMD_LEN).map(Raw::from_bytes).collect()
}

/// Respuesta de 8 bytes de un comando inmediato como cuatro enteros de 16 bits.
pub fn parse_reply(b: &[u8]) -> Option<[u16; 4]> {
    if b.len() < REPLY_LEN {
        return None;
    }
    let w = |i: usize| u16::from_le_bytes([b[2 * i], b[2 * i + 1]]);
    Some([w(0), w(1), w(2), w(3)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_is_twelve_little_endian_bytes() {
        let c = Raw::new(0x8005, [0x8000, 0x1234, 0, 0xFFFF, 7]);
        assert_eq!(
            c.to_bytes(),
            [0x05, 0x80, 0x00, 0x80, 0x34, 0x12, 0, 0, 0xFF, 0xFF, 7, 0]
        );
        assert_eq!(Raw::from_bytes(&c.to_bytes()).unwrap(), c);
        assert_eq!(c.kind(), Kind::List);
        assert_eq!(Raw::bare(0x000D).kind(), Kind::Immediate);
        assert_eq!(Raw::bare(0x4001).kind(), Kind::Unknown);
        assert_eq!(Raw::bare(0x8100).kind(), Kind::Unknown);
    }

    #[test]
    fn wrong_lengths_are_rejected() {
        assert_eq!(
            Raw::from_bytes(&[0; 11]),
            Err(WireError::BadLength { got: 11 })
        );
        assert_eq!(
            unpack(&[0; 25]),
            Err(WireError::NotMultipleOf12 { got: 25 })
        );
    }

    #[test]
    fn batches_are_3072_bytes_padded_with_end_of_list() {
        assert_eq!(BATCH_BYTES, 3072);
        let cmds: Vec<Raw> = (0..300u16)
            .map(|i| Raw::new(0x8001, [i, 0, 0, 0, 0]))
            .collect();
        let batches = pack_batches(&cmds).unwrap();
        assert_eq!(batches.len(), 2);
        assert!(batches.iter().all(|b| b.len() == BATCH_BYTES));
        let second = unpack(&batches[1]).unwrap();
        assert_eq!(second.len(), BATCH_CMDS);
        assert_eq!(second[43], cmds[299]);
        assert!(second[44..].iter().all(|c| *c == Raw::bare(PAD_CODE)));
        // Una tanda llena no lleva relleno.
        let full = pack_batches(&cmds[..256]).unwrap();
        assert_eq!(full.len(), 1);
        assert_eq!(unpack(&full[0]).unwrap(), cmds[..256]);
        assert!(pack_batches(&[]).unwrap().is_empty());
        assert_eq!(batch_count(0), 0);
        assert_eq!(batch_count(256), 1);
        assert_eq!(batch_count(257), 2);
    }

    #[test]
    fn immediates_cannot_go_in_a_list() {
        let err = pack_batches(&[Raw::bare(0x8002), Raw::bare(0x0004)]).unwrap_err();
        assert_eq!(
            err,
            WireError::NotAListCommand {
                index: 1,
                code: 0x0004
            }
        );
    }

    #[test]
    fn arbitrary_bytes_never_panic_and_round_trip() {
        // Generador congruencial: bytes pseudoaleatorios deterministas.
        let mut s = 0x1234_5678_9abc_def0u64;
        let mut next = || {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (s >> 33) as u8
        };
        for len in [0usize, 1, 11, 12, 13, 24, 3072, 3073] {
            let bytes: Vec<u8> = (0..len).map(|_| next()).collect();
            match unpack(&bytes) {
                Ok(cmds) => {
                    let back: Vec<u8> = cmds.iter().flat_map(|c| c.to_bytes()).collect();
                    assert_eq!(back, bytes);
                }
                Err(e) => assert!(!len.is_multiple_of(12), "{e}"),
            }
        }
    }

    #[test]
    fn replies_are_four_little_endian_words() {
        assert_eq!(
            parse_reply(&[1, 0, 2, 0, 3, 0, 0x24, 0x00]),
            Some([1, 2, 3, 0x24])
        );
        assert_eq!(parse_reply(&[0; 7]), None);
    }
}
