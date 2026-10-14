//! Transporte hacia la tarjeta: dos endpoints de bulto y nada más.
//!
//! - Escritura por el endpoint `0x02`: un comando inmediato de 12 bytes o una tanda de lista de
//!   3072 bytes (galvoplotter y MeerK40t, MIT; edutechwiki y Bryce en prosa).
//! - Lectura por el endpoint `0x88`: 8 bytes (cuatro enteros de 16 bits) tras cada inmediato.
//! - El endpoint de dongle (`0x01`/`0x81`) no se usa: el protocolo no lo exige.
//!
//! Implementaciones: [`MemoryTransport`] (en memoria, con respuestas programadas), `usb`
//! (feature `usb`, `nusb`) y el emulador de tarjeta de `emu`.

use crate::cmd::Imm;
use crate::wire::{self, REPLY_LEN};
use std::collections::VecDeque;
use std::fmt;
use tlaku_i18n::tr;

/// Vendor ID de BJJCZ/JCZ.
pub const VENDOR_ID: u16 = 0x9588;
/// Product ID de la LMCV4-FIBER-M ya enumerada ("BJJCZ USBLMCV4").
pub const PRODUCT_ID: u16 = 0x9899;
/// Endpoint de escritura (bulk OUT) de órdenes y listas.
pub const EP_OUT: u8 = 0x02;
/// Endpoint de lectura (bulk IN) de estado.
pub const EP_IN: u8 = 0x88;

/// ¿Se puede hablar con esta tarjeta?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    /// Identificada y con protocolo descrito (aun sin verificar con hardware propio).
    Supported,
    /// Conocida pero rechazada, con el motivo.
    Rejected(Reject),
}

/// Por qué se rechaza una tarjeta. El texto sale en el idioma activo ([`Reject::message`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reject {
    /// Clon que se vio en un foro; no se sabe si habla el mismo protocolo.
    UnverifiedClone,
    /// Necesita cargar el firmware del FPGA, que Tlaku no hace.
    NeedsFirmware,
    /// Posible tarjeta con carga de firmware; no verificado.
    UnverifiedFirmwareBoard,
    /// VID:PID que no está en la tabla de tarjetas conocidas.
    UnknownIds,
}

impl Reject {
    /// El motivo en el idioma activo.
    pub fn message(self) -> String {
        match self {
            Reject::UnverifiedClone => tr!("galvo.transport.reject_clone"),
            Reject::NeedsFirmware => tr!("galvo.transport.reject_needs_firmware"),
            Reject::UnverifiedFirmwareBoard => tr!("galvo.transport.reject_firmware_board"),
            Reject::UnknownIds => tr!("galvo.transport.reject_unknown_ids"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnownBoard {
    pub vendor_id: u16,
    pub product_id: u16,
    pub name: &'static str,
    pub support: Support,
    pub source: &'static str,
}

impl KnownBoard {
    /// El nombre de la tarjeta en el idioma activo (`name` es el de la tabla, en español).
    pub fn display_name(&self) -> String {
        let ids = format!("{:04X}:{:04X}", self.vendor_id, self.product_id);
        match self.product_id {
            0x9980 => tr!("galvo.transport.name_clone", ids = ids),
            0x9880 => tr!("galvo.transport.name_no_firmware", ids = ids),
            0x9990 | 0x9999 => tr!("galvo.transport.name_inf", ids = ids),
            _ => self.name.to_string(),
        }
    }
}

/// Las tarjetas conocidas. Todo VID:PID que no esté aquí se rechaza.
pub static KNOWN_BOARDS: &[KnownBoard] = &[
    KnownBoard {
        vendor_id: VENDOR_ID,
        product_id: PRODUCT_ID,
        name: "BJJCZ USBLMCV4 (LMCV4-FIBER-M, EZCAD2)",
        support: Support::Supported,
        source: "galvoplotter y MeerK40t balormk (MIT); docs/hardware/galvo.md sección 1.1",
    },
    KnownBoard {
        vendor_id: VENDOR_ID,
        product_id: 0x9980,
        name: "clon BJJCZ (9588:9980)",
        support: Support::Rejected(Reject::UnverifiedClone),
        source: "foro de LightBurn (INF del driver de LightBurn)",
    },
    KnownBoard {
        vendor_id: VENDOR_ID,
        product_id: 0x9880,
        name: "tarjeta FIBER/CO2-V sin firmware cargado (9588:9880)",
        support: Support::Rejected(Reject::NeedsFirmware),
        source: "lmcv2u-loader README",
    },
    KnownBoard {
        vendor_id: VENDOR_ID,
        product_id: 0x9990,
        name: "INF de EZCAD2 (9588:9990)",
        support: Support::Rejected(Reject::UnverifiedFirmwareBoard),
        source: "foro de LightBurn (INF del driver de EZCAD2)",
    },
    KnownBoard {
        vendor_id: VENDOR_ID,
        product_id: 0x9999,
        name: "INF de EZCAD2 (9588:9999)",
        support: Support::Rejected(Reject::UnverifiedFirmwareBoard),
        source: "foro de LightBurn (INF del driver de EZCAD2)",
    },
];

pub fn identify(vendor_id: u16, product_id: u16) -> Option<&'static KnownBoard> {
    KNOWN_BOARDS
        .iter()
        .find(|b| b.vendor_id == vendor_id && b.product_id == product_id)
}

/// `true` solo para tarjetas con `Support::Supported`.
pub fn is_supported(vendor_id: u16, product_id: u16) -> bool {
    identify(vendor_id, product_id).is_some_and(|b| b.support == Support::Supported)
}

/// Aceptación explícita de que el láser puede ser peligroso. Sin ella no se abre el USB real.
/// Solo se construye llamando a [`LaserSafetyAck::i_understand_laser_safety`], que el CLI hace
/// únicamente con `--i-understand-laser-safety`.
#[derive(Debug, Clone, Copy)]
pub struct LaserSafetyAck(());

impl LaserSafetyAck {
    pub fn i_understand_laser_safety() -> Self {
        LaserSafetyAck(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// No hubo respuesta a tiempo.
    Timeout,
    /// La tarjeta ya no está (USB desenchufado).
    Disconnected,
    /// Otro fallo de E/S, con el mensaje del sistema.
    Io(String),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportError::Timeout => write!(f, "{}", tr!("galvo.transport.timeout")),
            TransportError::Disconnected => write!(f, "{}", tr!("galvo.transport.disconnected")),
            TransportError::Io(m) => write!(f, "{}", tr!("galvo.transport.io", msg = m)),
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
    /// Tarjeta simulada: no hay hardware detrás.
    pub emulated: bool,
}

pub trait Transport: Send {
    /// Escribe `data` entero en el endpoint de salida.
    fn write(&mut self, data: &[u8]) -> Result<(), TransportError>;

    /// Lee la respuesta de 8 bytes del endpoint de entrada.
    fn read(&mut self) -> Result<[u8; REPLY_LEN], TransportError>;

    fn info(&self) -> TransportInfo;
}

impl<T: Transport + ?Sized> Transport for Box<T> {
    fn write(&mut self, data: &[u8]) -> Result<(), TransportError> {
        (**self).write(data)
    }

    fn read(&mut self) -> Result<[u8; REPLY_LEN], TransportError> {
        (**self).read()
    }

    fn info(&self) -> TransportInfo {
        (**self).info()
    }
}

/// Envía un inmediato y lee su respuesta (cuatro palabras).
pub fn exchange<T: Transport + ?Sized>(t: &mut T, imm: &Imm) -> Result<[u16; 4], TransportError> {
    t.write(&imm.to_raw().to_bytes())?;
    let reply = t.read()?;
    wire::parse_reply(&reply).ok_or_else(|| TransportError::Io("respuesta incompleta".into()))
}

/// Transporte en memoria: guarda lo escrito y devuelve las respuestas que se le programen.
#[derive(Debug)]
pub struct MemoryTransport {
    /// Cada llamada a `write`, en orden.
    pub written: Vec<Vec<u8>>,
    replies: VecDeque<Result<[u8; REPLY_LEN], TransportError>>,
    /// Respuesta cuando no queda ninguna programada (`None`: tiempo agotado).
    pub default_reply: Option<[u8; REPLY_LEN]>,
    pub info: TransportInfo,
}

impl Default for MemoryTransport {
    fn default() -> Self {
        MemoryTransport {
            written: Vec::new(),
            replies: VecDeque::new(),
            default_reply: None,
            info: TransportInfo {
                name: "memoria".into(),
                vendor_id: VENDOR_ID,
                product_id: PRODUCT_ID,
                emulated: false,
            },
        }
    }
}

impl MemoryTransport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_reply(&mut self, reply: Result<[u8; REPLY_LEN], TransportError>) {
        self.replies.push_back(reply);
    }

    /// Respuesta de cuatro palabras.
    pub fn push_words(&mut self, w: [u16; 4]) {
        let mut b = [0u8; REPLY_LEN];
        for (i, v) in w.iter().enumerate() {
            b[2 * i..2 * i + 2].copy_from_slice(&v.to_le_bytes());
        }
        self.replies.push_back(Ok(b));
    }
}

impl Transport for MemoryTransport {
    fn write(&mut self, data: &[u8]) -> Result<(), TransportError> {
        self.written.push(data.to_vec());
        Ok(())
    }

    fn read(&mut self) -> Result<[u8; REPLY_LEN], TransportError> {
        match self.replies.pop_front() {
            Some(r) => r,
            None => self.default_reply.ok_or(TransportError::Timeout),
        }
    }

    fn info(&self) -> TransportInfo {
        self.info.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_documented_board_is_supported() {
        assert!(is_supported(0x9588, 0x9899));
        for pid in [0x9980u16, 0x9880, 0x9990, 0x9999] {
            assert!(identify(0x9588, pid).is_some());
            assert!(!is_supported(0x9588, pid), "{pid:#06x}");
        }
        // CH341 de los K40, el de GRBL por USB y lo desconocido.
        assert!(!is_supported(0x1A86, 0x5512));
        assert!(!is_supported(0x9588, 0x0001));
        assert!(!is_supported(0, 0));
        assert!(identify(0x1A86, 0x7523).is_none());
    }

    #[test]
    fn exchange_writes_twelve_bytes_and_parses_the_reply() {
        let mut t = MemoryTransport::new();
        t.push_words([1, 2, 3, 0x24]);
        let r = exchange(&mut t, &Imm::GetVersion).unwrap();
        assert_eq!(r, [1, 2, 3, 0x24]);
        assert_eq!(t.written.len(), 1);
        assert_eq!(t.written[0].len(), 12);
        assert_eq!(t.written[0][..2], [0x07, 0x00]);
        // Sin respuesta programada: tiempo agotado.
        assert_eq!(
            exchange(&mut t, &Imm::GetVersion),
            Err(TransportError::Timeout)
        );
        t.push_reply(Err(TransportError::Disconnected));
        assert_eq!(
            exchange(&mut t, &Imm::ReadPort),
            Err(TransportError::Disconnected)
        );
    }

    #[test]
    fn boxed_transports_work() {
        let mut t: Box<dyn Transport> = Box::new(MemoryTransport::new());
        t.write(&[0; 12]).unwrap();
        assert_eq!(t.info().vendor_id, 0x9588);
    }
}
