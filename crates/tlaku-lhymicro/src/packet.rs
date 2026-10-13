//! Del flujo de comandos LHYMICRO-GL a paquetes de 30 bytes.
//!
//! El flujo es el texto que escribe un `.egv`: comandos, magnitudes y saltos de línea. Cada línea
//! se corta en trozos de 30 bytes y el último se rellena con `F` hasta completar el paquete; un
//! `-` al final de la línea no se envía: es una orden al anfitrión de "esperar a que la placa
//! termine" antes de seguir. Las reglas salen de `lihuiyu/controller.py` de MeerK40t (MIT; ver
//! THIRD_PARTY.md):
//!
//! - un paquete nunca termina en `P` sin relleno (con el relleno `F` siempre hay al menos una `F`
//!   salvo que la línea mida justo 30 bytes; en ese caso se manda después un paquete de 30 `F`);
//! - los trozos pueden cortar un número a la mitad: el intérprete de la placa lee en flujo;
//! - el paquete lleva el CRC de [`crate::crc`] y viaja con los `0xA6` del CH341.

use crate::crc::{self, PAYLOAD_LEN};
use std::fmt;

/// Relleno de los paquetes.
pub const PAD: u8 = b'F';

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub payload: [u8; PAYLOAD_LEN],
    /// Tras este paquete hay que esperar a que la placa termine (el `-` del flujo).
    pub wait_finish: bool,
}

impl Frame {
    /// Paquete con `text` (hasta 30 bytes) y el resto de `F`.
    pub fn from_text(text: &[u8], wait_finish: bool) -> Frame {
        assert!(text.len() <= PAYLOAD_LEN, "más de 30 bytes en un paquete");
        let mut payload = [PAD; PAYLOAD_LEN];
        payload[..text.len()].copy_from_slice(text);
        Frame {
            payload,
            wait_finish,
        }
    }

    /// Bytes que se escriben en el endpoint de salida (34).
    pub fn wire(&self) -> Vec<u8> {
        crc::wire_packet(&self.payload)
    }

    /// Carga como texto (sin el relleno final), para registros y errores.
    pub fn text(&self) -> String {
        let end = self
            .payload
            .iter()
            .rposition(|&b| b != PAD)
            .map_or(0, |i| i + 1);
        String::from_utf8_lossy(&self.payload[..end]).into_owned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    /// Un byte que no es parte del lenguaje que genera Tlaku.
    BadByte { offset: usize, byte: u8 },
    /// Un `-` que no está al final de la línea.
    MisplacedWait { offset: usize },
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::BadByte { offset, byte } => {
                write!(
                    f,
                    "byte 0x{byte:02X} no válido en el flujo (posición {offset})"
                )
            }
            FrameError::MisplacedWait { offset } => {
                write!(
                    f,
                    "el '-' solo puede ir al final de una línea (posición {offset})"
                )
            }
        }
    }
}

impl std::error::Error for FrameError {}

fn allowed(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'|' || b == b'@'
}

/// Parte un flujo en paquetes. Una última línea sin `\n` se trata como si lo llevara.
pub fn frame_stream(stream: &[u8]) -> Result<Vec<Frame>, FrameError> {
    let mut frames: Vec<Frame> = Vec::new();
    let mut offset = 0usize;
    for line in stream.split_inclusive(|&b| b == b'\n') {
        let start = offset;
        offset += line.len();
        let mut content = line.strip_suffix(b"\n").unwrap_or(line);
        let mut wait = false;
        if let Some(rest) = content.strip_suffix(b"-") {
            content = rest;
            wait = true;
        }
        for (i, &b) in content.iter().enumerate() {
            if b == b'-' {
                return Err(FrameError::MisplacedWait { offset: start + i });
            }
            if !allowed(b) {
                return Err(FrameError::BadByte {
                    offset: start + i,
                    byte: b,
                });
            }
        }
        if content.is_empty() {
            // Línea con solo `-`: la espera se aplica al paquete anterior.
            if wait && let Some(last) = frames.last_mut() {
                last.wait_finish = true;
            }
            continue;
        }
        let n_chunks = content.len().div_ceil(PAYLOAD_LEN);
        for (i, chunk) in content.chunks(PAYLOAD_LEN).enumerate() {
            frames.push(Frame::from_text(chunk, wait && i + 1 == n_chunks));
        }
        // Línea de justo 30 bytes (o múltiplo) sin `-`: la placa necesita un paquete de relleno
        // que cierre la línea (MeerK40t manda 30 `F`).
        if !wait && content.len().is_multiple_of(PAYLOAD_LEN) {
            frames.push(Frame::from_text(b"", false));
        }
    }
    Ok(frames)
}

/// Paquete de pausa/reanudación en tiempo real: `PN`. MeerK40t manda el mismo paquete para las
/// dos órdenes (`~PN!` y `~PN&`, donde `!` y `&` solo cambian el estado del anfitrión): la placa
/// lo trata como un interruptor (inferido).
pub fn pause_toggle_frame() -> Frame {
    Frame::from_text(b"PN", false)
}

/// Paquetes que detienen la placa y apagan el láser: `I` (reinicia el intérprete y cancela el
/// programa en curso, como el `I*` de MeerK40t) e `IUS1P` (el apagado de láser en reposo de
/// MeerK40t, que aquí se repite por precaución).
pub fn stop_frames() -> Vec<Frame> {
    vec![Frame::from_text(b"I", false), laser_off_frame()]
}

/// `IUS1P`: láser apagado, en reposo.
pub fn laser_off_frame() -> Frame {
    Frame::from_text(b"IUS1P", false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(frames: &[Frame]) -> Vec<String> {
        frames.iter().map(Frame::text).collect()
    }

    #[test]
    fn short_line_is_padded_with_f_to_30_bytes() {
        let f = frame_stream(b"IBzzzvRzzzvS1P\n").unwrap();
        assert_eq!(f.len(), 1);
        assert_eq!(&f[0].payload[..14], b"IBzzzvRzzzvS1P");
        assert!(f[0].payload[14..].iter().all(|&b| b == b'F'));
        assert!(!f[0].wait_finish);
        assert_eq!(f[0].text(), "IBzzzvRzzzvS1P");
    }

    #[test]
    fn dash_at_line_end_marks_wait_and_is_not_sent() {
        let f = frame_stream(b"FNSE-\n").unwrap();
        assert_eq!(f.len(), 1);
        assert!(f[0].wait_finish);
        assert_eq!(f[0].text(), "FNSE");
    }

    #[test]
    fn long_line_is_split_in_30_byte_chunks() {
        let line: String = "a".repeat(70);
        let f = frame_stream(format!("{line}\n").as_bytes()).unwrap();
        assert_eq!(f.len(), 3);
        assert_eq!(f[0].payload, [b'a'; 30]);
        assert_eq!(f[1].payload, [b'a'; 30]);
        assert_eq!(&f[2].payload[..10], &[b'a'; 10]);
        assert_eq!(f[2].payload[10], b'F');
    }

    #[test]
    fn exact_30_byte_line_gets_an_extra_filler_packet() {
        let line: String = "b".repeat(30);
        let f = frame_stream(format!("{line}\n").as_bytes()).unwrap();
        assert_eq!(f.len(), 2);
        assert_eq!(f[1].payload, [b'F'; 30]);
        // Con `-` no hace falta el relleno.
        let f = frame_stream(format!("{line}-\n").as_bytes()).unwrap();
        assert_eq!(f.len(), 1);
        assert!(f[0].wait_finish);
        // Un `-` después de un múltiplo de 30: la espera va en el último trozo.
        let line: String = "c".repeat(60);
        let f = frame_stream(format!("{line}-\n").as_bytes()).unwrap();
        assert_eq!(f.len(), 2);
        assert!(!f[0].wait_finish && f[1].wait_finish);
    }

    #[test]
    fn the_egv_rectangle_of_meerk40t_becomes_three_packets() {
        let stream = b"IBzzzvRzzzvS1P\nICV2490731016000027CNLBS1EDz139Rz139Tz139Lz139FNSE-\n";
        let f = frame_stream(stream).unwrap();
        // 14 bytes -> 1 paquete; 52 bytes -> 2 paquetes (30 + 22).
        assert_eq!(f.len(), 3);
        assert_eq!(
            texts(&f),
            [
                "IBzzzvRzzzvS1P",
                "ICV2490731016000027CNLBS1EDz13",
                "9Rz139Tz139Lz139FNSE"
            ]
        );
        assert!(f[2].wait_finish && !f[1].wait_finish);
    }

    #[test]
    fn blank_and_dash_only_lines() {
        assert!(frame_stream(b"\n\n").unwrap().is_empty());
        let f = frame_stream(b"IPP\n-\n").unwrap();
        assert_eq!(f.len(), 1);
        assert!(f[0].wait_finish);
        // Sin salto final también cuenta como línea.
        assert_eq!(frame_stream(b"IPP").unwrap().len(), 1);
    }

    #[test]
    fn foreign_bytes_are_rejected() {
        assert!(matches!(
            frame_stream(b"IB zz\n"),
            Err(FrameError::BadByte {
                offset: 2,
                byte: b' '
            })
        ));
        assert!(matches!(
            frame_stream(b"I*\n"),
            Err(FrameError::BadByte { .. })
        ));
        assert!(matches!(
            frame_stream(b"IB-zz\n"),
            Err(FrameError::MisplacedWait { offset: 2 })
        ));
        assert!(frame_stream("Iñ\n".as_bytes()).is_err());
    }

    #[test]
    fn wire_of_a_frame_has_34_bytes() {
        let f = Frame::from_text(b"IPP", false);
        let w = f.wire();
        assert_eq!(w.len(), 34);
        assert_eq!((w[0], w[1], w[32]), (0xA6, 0x00, 0xA6));
        assert_eq!(w[33], crc::crc8(&f.payload));
    }

    #[test]
    fn realtime_frames() {
        assert_eq!(pause_toggle_frame().text(), "PN");
        let stop = stop_frames();
        assert_eq!(texts(&stop), ["I", "IUS1P"]);
        assert_eq!(laser_off_frame().text(), "IUS1P");
    }
}
