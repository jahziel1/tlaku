//! Transporte hacia la placa: dos endpoints de bulto del CH341 y nada más.
//!
//! El trait no sabe de paquetes ni de estados: solo escribe bytes en el endpoint de salida
//! (`0x02`) y lee del de entrada (`0x82`). Las funciones de este módulo ponen encima el protocolo
//! del CH341 en modo EPP: el paquete de 34 bytes y la consulta de estado (`0xA0` y 6 bytes).
//!
//! - [`MemoryTransport`]: en memoria, con respuestas programadas; para pruebas de bajo nivel.
//! - `UsbTransport` (feature `usb`): el CH341 real con `nusb`.
//! - `EmuTransport` (feature `emu`): una placa simulada completa.

use crate::crc::EPP_STATUS;
use crate::packet::Frame;
use crate::status::{STATUS_LEN, StatusReport};
use std::collections::VecDeque;
use std::fmt;
use tlaku_i18n::tr;

/// VID/PID del CH341 de la M2 Nano (la Moshiboard usa los mismos).
pub const VENDOR_ID: u16 = 0x1A86;
pub const PRODUCT_ID: u16 = 0x5512;
/// Endpoint de escritura (bulk OUT) y de lectura (bulk IN) del CH341.
pub const EP_OUT: u8 = 0x02;
pub const EP_IN: u8 = 0x82;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// No hubo respuesta a tiempo.
    Timeout,
    /// El dispositivo ya no está (USB desenchufado).
    Disconnected,
    /// Otro fallo de E/S, con el mensaje del sistema.
    Io(String),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportError::Timeout => f.write_str(&tr!("lhymicro.transport.timeout")),
            TransportError::Disconnected => f.write_str(&tr!("lhymicro.transport.disconnected")),
            TransportError::Io(m) => f.write_str(&tr!("lhymicro.transport.io", m = m)),
        }
    }
}

impl std::error::Error for TransportError {}

/// Qué se sabe del dispositivo conectado.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransportInfo {
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    /// Versión del chip CH341 (control `0x5F`), si se pudo leer.
    pub chip_version: Option<u16>,
}

pub trait Transport: Send {
    /// Escribe `data` entero en el endpoint de salida.
    fn write(&mut self, data: &[u8]) -> Result<(), TransportError>;

    /// Lee hasta `len` bytes del endpoint de entrada (puede devolver menos).
    fn read(&mut self, len: usize) -> Result<Vec<u8>, TransportError>;

    fn info(&self) -> TransportInfo {
        TransportInfo::default()
    }
}

impl<T: Transport + ?Sized> Transport for Box<T> {
    fn write(&mut self, data: &[u8]) -> Result<(), TransportError> {
        (**self).write(data)
    }

    fn read(&mut self, len: usize) -> Result<Vec<u8>, TransportError> {
        (**self).read(len)
    }

    fn info(&self) -> TransportInfo {
        (**self).info()
    }
}

/// Escribe un paquete (34 bytes en el cable).
pub fn write_frame<T: Transport + ?Sized>(t: &mut T, frame: &Frame) -> Result<(), TransportError> {
    t.write(&frame.wire())
}

/// Consulta de estado: escribe `0xA0` y lee 6 bytes.
///
/// Si la lectura no llega a tiempo se lee una vez más sin volver a preguntar: una respuesta tardía
/// que se quedara en el endpoint desfasaría todas las lecturas siguientes (cada consulta
/// recibiría la respuesta de la anterior).
pub fn read_status<T: Transport + ?Sized>(t: &mut T) -> Result<StatusReport, TransportError> {
    t.write(&[EPP_STATUS])?;
    let bytes = match t.read(STATUS_LEN) {
        Ok(b) => b,
        Err(TransportError::Timeout) => t.read(STATUS_LEN)?,
        Err(e) => return Err(e),
    };
    Ok(StatusReport::parse(&bytes))
}

/// Transporte en memoria: guarda lo escrito y devuelve las respuestas que se le programen.
#[derive(Debug, Default)]
pub struct MemoryTransport {
    /// Cada llamada a `write`, en orden.
    pub written: Vec<Vec<u8>>,
    replies: VecDeque<Result<Vec<u8>, TransportError>>,
    /// Respuesta de estado por defecto cuando no queda ninguna programada.
    pub default_reply: Option<Vec<u8>>,
}

impl MemoryTransport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Siempre contesta con ese estado (código en el byte 1).
    pub fn always_status(code: u8) -> Self {
        MemoryTransport {
            default_reply: Some(vec![255, code, 0, 0, 0, 1]),
            ..Default::default()
        }
    }

    pub fn push_reply(&mut self, reply: Result<Vec<u8>, TransportError>) {
        self.replies.push_back(reply);
    }

    pub fn push_status(&mut self, code: u8) {
        self.replies.push_back(Ok(vec![255, code, 0, 0, 0, 1]));
    }
}

impl Transport for MemoryTransport {
    fn write(&mut self, data: &[u8]) -> Result<(), TransportError> {
        self.written.push(data.to_vec());
        Ok(())
    }

    fn read(&mut self, _len: usize) -> Result<Vec<u8>, TransportError> {
        match self.replies.pop_front() {
            Some(r) => r,
            None => self.default_reply.clone().ok_or(TransportError::Timeout),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::Status;

    #[test]
    fn status_query_writes_a0_and_parses_six_bytes() {
        let mut t = MemoryTransport::new();
        t.push_status(206);
        let r = read_status(&mut t).unwrap();
        assert_eq!(r.status, Status::Ok);
        assert_eq!(t.written, vec![vec![0xA0]]);
    }

    #[test]
    fn frames_are_written_as_one_34_byte_transfer() {
        let mut t = MemoryTransport::new();
        write_frame(&mut t, &Frame::from_text(b"IPP", false)).unwrap();
        assert_eq!(t.written.len(), 1);
        assert_eq!(t.written[0].len(), 34);
    }

    #[test]
    fn missing_replies_time_out_and_errors_pass_through() {
        let mut t = MemoryTransport::new();
        assert_eq!(read_status(&mut t), Err(TransportError::Timeout));
        t.push_reply(Err(TransportError::Disconnected));
        assert_eq!(read_status(&mut t), Err(TransportError::Disconnected));
        let mut t = MemoryTransport::always_status(238);
        assert_eq!(read_status(&mut t).unwrap().status, Status::Busy);
        assert_eq!(read_status(&mut t).unwrap().status, Status::Busy);
    }

    #[test]
    fn a_late_reply_is_read_instead_of_asking_again() {
        let mut t = MemoryTransport::new();
        t.push_reply(Err(TransportError::Timeout));
        t.push_status(236);
        let r = read_status(&mut t).unwrap();
        assert_eq!(r.status, Status::Finished);
        assert_eq!(t.written.len(), 1, "una sola consulta");
    }

    #[test]
    fn boxed_transports_work() {
        let mut t: Box<dyn Transport> = Box::new(MemoryTransport::always_status(236));
        assert_eq!(read_status(&mut t).unwrap().status, Status::Finished);
        assert_eq!(t.info(), TransportInfo::default());
    }
}
