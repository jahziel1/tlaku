//! Tarjeta LMC real por USB con `nusb` (Rust puro, MIT o Apache-2.0; sin libusb).
//!
//! **Sin probar con una tarjeta real** (no hay hardware en el desarrollo). Plataformas:
//! - Linux: se desacopla el controlador del kernel si hace falta; los permisos se dan con una
//!   regla de udev (`SUBSYSTEM=="usb", ATTR{idVendor}=="9588", MODE="0666"`).
//! - macOS: no necesita controlador.
//! - Windows: hay que asociar la tarjeta al controlador **WinUSB** (por ejemplo con Zadig); con el
//!   controlador de EZCAD, `nusb` no puede abrirla.
//!
//! Seguridad: el listado muestra cualquier dispositivo del fabricante 9588 con su estado
//! (`Support`), pero [`UsbTransport::open`] solo abre un `9588:9899`, y exige un
//! [`LaserSafetyAck`]. No se carga firmware ni se escribe nada de configuración.

use crate::transport::{
    EP_IN, EP_OUT, LaserSafetyAck, PRODUCT_ID, Support, Transport, TransportError, TransportInfo,
    VENDOR_ID, identify,
};
use crate::wire::REPLY_LEN;
use nusb::transfer::{Buffer, Bulk, In, Out, TransferError};
use nusb::{Endpoint, MaybeFuture};
use std::fmt;
use std::time::Duration;

/// Esperas del USB.
#[derive(Debug, Clone, Copy)]
pub struct UsbTimeouts {
    pub write: Duration,
    pub read: Duration,
}

impl Default for UsbTimeouts {
    fn default() -> Self {
        UsbTimeouts {
            write: Duration::from_secs(2),
            read: Duration::from_millis(500),
        }
    }
}

/// Un dispositivo del fabricante BJJCZ (`9588:xxxx`) conectado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbCandidate {
    pub index: usize,
    pub bus_id: String,
    pub address: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub product: Option<String>,
    pub serial: Option<String>,
    /// Qué dice la tabla de tarjetas conocidas.
    pub support: Support,
    pub name: String,
}

impl fmt::Display for UsbCandidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "#{} {:04X}:{:04X} {} (bus {}, dirección {}) - ",
            self.index, self.vendor_id, self.product_id, self.name, self.bus_id, self.address
        )?;
        match self.support {
            Support::Supported => write!(f, "compatible (EXPERIMENTAL)"),
            Support::Rejected(why) => write!(f, "rechazada: {why}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenError {
    NoDevice,
    /// El VID:PID no es una tarjeta compatible.
    Unsupported {
        vendor_id: u16,
        product_id: u16,
        why: String,
    },
    Permission(String),
    Busy(String),
    Other(String),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::NoDevice => write!(
                f,
                "no se encontró ninguna tarjeta (USB {VENDOR_ID:04X}:{PRODUCT_ID:04X}). Comprueba \
                 el cable USB y que la máquina esté encendida"
            ),
            OpenError::Unsupported {
                vendor_id,
                product_id,
                why,
            } => write!(
                f,
                "la tarjeta {vendor_id:04X}:{product_id:04X} no es compatible: {why}"
            ),
            OpenError::Permission(m) => write!(
                f,
                "sin permiso para abrir la tarjeta: {m}. En Linux agrega una regla de udev; en \
                 Windows asocia la tarjeta al controlador WinUSB con Zadig"
            ),
            OpenError::Busy(m) => write!(
                f,
                "la tarjeta está en uso: {m}. Cierra EZCAD, LightBurn, MeerK40t o cualquier otro \
                 programa que la tenga abierta"
            ),
            OpenError::Other(m) => write!(f, "no se pudo abrir la tarjeta: {m}"),
        }
    }
}

impl std::error::Error for OpenError {}

fn classify(e: nusb::Error) -> OpenError {
    match e.kind() {
        nusb::ErrorKind::PermissionDenied => OpenError::Permission(e.to_string()),
        nusb::ErrorKind::Busy => OpenError::Busy(e.to_string()),
        nusb::ErrorKind::NotFound | nusb::ErrorKind::Disconnected => OpenError::NoDevice,
        _ => OpenError::Other(e.to_string()),
    }
}

fn is_bjjcz(d: &nusb::DeviceInfo) -> bool {
    d.vendor_id() == VENDOR_ID
}

/// Lista los dispositivos del fabricante BJJCZ conectados, con su compatibilidad. No abre nada.
pub fn list_boards() -> Result<Vec<UsbCandidate>, OpenError> {
    let devices = nusb::list_devices().wait().map_err(classify)?;
    Ok(devices
        .filter(is_bjjcz)
        .enumerate()
        .map(|(index, d)| {
            let known = identify(d.vendor_id(), d.product_id());
            UsbCandidate {
                index,
                bus_id: d.bus_id().to_string(),
                address: d.device_address(),
                vendor_id: d.vendor_id(),
                product_id: d.product_id(),
                product: d.product_string().map(str::to_string),
                serial: d.serial_number().map(str::to_string),
                support: known.map_or(
                    Support::Rejected("VID:PID desconocido: Tlaku no habla con él"),
                    |k| k.support,
                ),
                name: known.map_or_else(
                    || d.product_string().unwrap_or("desconocida").to_string(),
                    |k| k.name.to_string(),
                ),
            }
        })
        .collect())
}

pub struct UsbTransport {
    ep_out: Endpoint<Bulk, Out>,
    ep_in: Endpoint<Bulk, In>,
    timeouts: UsbTimeouts,
    info: TransportInfo,
}

impl UsbTransport {
    /// Abre la tarjeta número `index` de [`list_boards`]. Solo si es un `9588:9899`.
    pub fn open(
        index: usize,
        timeouts: UsbTimeouts,
        _ack: LaserSafetyAck,
    ) -> Result<Self, OpenError> {
        let info = nusb::list_devices()
            .wait()
            .map_err(classify)?
            .filter(is_bjjcz)
            .nth(index)
            .ok_or(OpenError::NoDevice)?;
        let (vid, pid) = (info.vendor_id(), info.product_id());
        match identify(vid, pid).map(|k| k.support) {
            Some(Support::Supported) => {}
            Some(Support::Rejected(why)) => {
                return Err(OpenError::Unsupported {
                    vendor_id: vid,
                    product_id: pid,
                    why: why.into(),
                });
            }
            None => {
                return Err(OpenError::Unsupported {
                    vendor_id: vid,
                    product_id: pid,
                    why: "VID:PID desconocido".into(),
                });
            }
        }
        let device = info.open().wait().map_err(classify)?;
        let interface = device
            .detach_and_claim_interface(0)
            .wait()
            .map_err(classify)?;
        let ep_out = interface
            .endpoint::<Bulk, Out>(EP_OUT)
            .map_err(|e| OpenError::Other(format!("endpoint {EP_OUT:#04x}: {e}")))?;
        let ep_in = interface
            .endpoint::<Bulk, In>(EP_IN)
            .map_err(|e| OpenError::Other(format!("endpoint {EP_IN:#04x}: {e}")))?;
        let mut t = UsbTransport {
            ep_out,
            ep_in,
            timeouts,
            info: TransportInfo {
                name: info.product_string().unwrap_or("LMC").to_string(),
                vendor_id: vid,
                product_id: pid,
                emulated: false,
            },
        };
        t.drain_input();
        Ok(t)
    }

    /// Descarta lo que haya quedado en el endpoint de entrada de una sesión anterior.
    fn drain_input(&mut self) {
        let mps = self.ep_in.max_packet_size().max(1);
        for _ in 0..8 {
            let c = self
                .ep_in
                .transfer_blocking(Buffer::new(mps), Duration::from_millis(30));
            if c.status.is_err() || c.actual_len == 0 {
                break;
            }
        }
    }
}

fn map_transfer(e: TransferError) -> TransportError {
    match e {
        TransferError::Cancelled => TransportError::Timeout,
        TransferError::Disconnected => TransportError::Disconnected,
        other => TransportError::Io(other.to_string()),
    }
}

impl Transport for UsbTransport {
    fn write(&mut self, data: &[u8]) -> Result<(), TransportError> {
        let c = self
            .ep_out
            .transfer_blocking(Buffer::from(data), self.timeouts.write);
        match c.status {
            Ok(()) if c.actual_len == data.len() => Ok(()),
            Ok(()) => Err(TransportError::Io(format!(
                "escritura parcial: {} de {} bytes",
                c.actual_len,
                data.len()
            ))),
            Err(TransferError::Stall) => {
                let _ = self.ep_out.clear_halt().wait();
                Err(TransportError::Io(
                    "endpoint de escritura detenido (stall)".into(),
                ))
            }
            Err(e) => Err(map_transfer(e)),
        }
    }

    fn read(&mut self) -> Result<[u8; REPLY_LEN], TransportError> {
        // Las lecturas deben pedirse en múltiplos del tamaño de paquete del endpoint.
        let mps = self.ep_in.max_packet_size().max(1);
        let req = REPLY_LEN.div_ceil(mps).max(1) * mps;
        let c = self
            .ep_in
            .transfer_blocking(Buffer::new(req), self.timeouts.read);
        match c.status {
            Ok(()) if c.buffer.len() >= REPLY_LEN => {
                let mut out = [0u8; REPLY_LEN];
                out.copy_from_slice(&c.buffer[..REPLY_LEN]);
                Ok(out)
            }
            Ok(()) => Err(TransportError::Io(format!(
                "respuesta de {} bytes (se esperaban {REPLY_LEN})",
                c.buffer.len()
            ))),
            Err(TransferError::Stall) => {
                let _ = self.ep_in.clear_halt().wait();
                Err(TransportError::Io(
                    "endpoint de lectura detenido (stall)".into(),
                ))
            }
            Err(e) => Err(map_transfer(e)),
        }
    }

    fn info(&self) -> TransportInfo {
        self.info.clone()
    }
}
