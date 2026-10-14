//! CH341 real por USB con `nusb` (Rust puro, MIT o Apache-2.0; sin libusb).
//!
//! La M2 Nano usa un CH341A/B en **modo EPP 1.9** (VID `1A86`, PID `5512`): escritura por el
//! endpoint `0x02`, lectura por el `0x82` y un solo control de fabricante, `0xB1` con
//! `wValue = 0x0102`, para poner el chip en modo EPP (MeerK40t `ch341/libusb.py`, MIT). La
//! Moshiboard comparte VID/PID: la sesión la distingue por los códigos de estado.
//!
//! **Sin probar con una placa real** (no hay hardware en el desarrollo). Plataformas:
//! - Linux: se desacopla el controlador del kernel si hace falta; los permisos se dan con una
//!   regla de udev (`SUBSYSTEM=="usb", ATTR{idVendor}=="1a86", ATTR{idProduct}=="5512", MODE="0666"`).
//! - macOS: no necesita controlador; si el sistema tiene instalado el controlador de WCH, hay
//!   que quitarlo.
//! - Windows: hay que asociar la placa al controlador **WinUSB** (por ejemplo con Zadig);
//!   con el controlador original de WCH `nusb` no puede abrirla.

use crate::transport::{Transport, TransportError, TransportInfo};
use nusb::transfer::{
    Buffer, Bulk, ControlIn, ControlOut, ControlType, In, Out, Recipient, TransferError,
};
use nusb::{Endpoint, MaybeFuture};
use std::fmt;
use std::time::Duration;
use tlaku_i18n::tr;

pub use crate::transport::{EP_IN, EP_OUT, PRODUCT_ID, VENDOR_ID};
/// Control de fabricante del CH341 que inicializa el puerto paralelo.
const REQ_PARA_INIT: u8 = 0xB1;
/// `wValue` de [`REQ_PARA_INIT`] para el modo EPP 1.9: `(1 << 8) | 2`.
const PARA_INIT_EPP19: u16 = 0x0102;
/// Control de fabricante que lee la versión del chip.
const REQ_GET_VERSION: u8 = 0x5F;

/// Esperas del USB. La de escritura es larga porque el CH341 detiene la escritura mientras la
/// placa tiene el búfer lleno (MeerK40t usa 60 s); la sesión solo escribe cuando la placa está
/// lista, así que aquí basta con un margen.
#[derive(Debug, Clone, Copy)]
pub struct UsbTimeouts {
    pub write: Duration,
    pub read: Duration,
    pub control: Duration,
}

impl Default for UsbTimeouts {
    fn default() -> Self {
        UsbTimeouts {
            write: Duration::from_secs(10),
            read: Duration::from_millis(1500),
            control: Duration::from_millis(1500),
        }
    }
}

/// Un CH341 `1A86:5512` conectado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbCandidate {
    pub index: usize,
    pub bus_id: String,
    pub address: u8,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
}

impl fmt::Display for UsbCandidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ids = format!("{VENDOR_ID:04X}:{PRODUCT_ID:04X}");
        f.write_str(&match self.product.as_deref() {
            Some(p) => tr!(
                "lhymicro.usb.candidate_product",
                index = self.index,
                ids = ids,
                bus = self.bus_id,
                address = self.address,
                product = p
            ),
            None => tr!(
                "lhymicro.usb.candidate",
                index = self.index,
                ids = ids,
                bus = self.bus_id,
                address = self.address
            ),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenError {
    /// No hay ningún `1A86:5512` conectado.
    NoDevice,
    /// El sistema no deja abrirlo o reclamarlo.
    Permission(String),
    /// Otro programa o controlador lo tiene tomado.
    Busy(String),
    Other(String),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::NoDevice => f.write_str(&tr!(
                "lhymicro.usb.no_device",
                ids = format!("{VENDOR_ID:04X}:{PRODUCT_ID:04X}")
            )),
            OpenError::Permission(m) => f.write_str(&tr!("lhymicro.usb.permission", m = m)),
            OpenError::Busy(m) => f.write_str(&tr!("lhymicro.usb.busy", m = m)),
            OpenError::Other(m) => f.write_str(&tr!("lhymicro.usb.other", m = m)),
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

fn is_board(d: &nusb::DeviceInfo) -> bool {
    d.vendor_id() == VENDOR_ID && d.product_id() == PRODUCT_ID
}

/// Lista los CH341 `1A86:5512` conectados (M2 Nano o Moshiboard).
pub fn list_boards() -> Result<Vec<UsbCandidate>, OpenError> {
    let devices = nusb::list_devices().wait().map_err(classify)?;
    Ok(devices
        .filter(is_board)
        .enumerate()
        .map(|(index, d)| UsbCandidate {
            index,
            bus_id: d.bus_id().to_string(),
            address: d.device_address(),
            manufacturer: d.manufacturer_string().map(str::to_string),
            product: d.product_string().map(str::to_string),
            serial: d.serial_number().map(str::to_string),
        })
        .collect())
}

pub struct UsbTransport {
    interface: nusb::Interface,
    ep_out: Endpoint<Bulk, Out>,
    ep_in: Endpoint<Bulk, In>,
    timeouts: UsbTimeouts,
    info: TransportInfo,
}

impl UsbTransport {
    /// Abre la placa número `index` de [`list_boards`], la pone en modo EPP y deja el
    /// transporte listo.
    pub fn open(index: usize, timeouts: UsbTimeouts) -> Result<Self, OpenError> {
        let info = nusb::list_devices()
            .wait()
            .map_err(classify)?
            .filter(is_board)
            .nth(index)
            .ok_or(OpenError::NoDevice)?;
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
            interface,
            ep_out,
            ep_in,
            timeouts,
            info: TransportInfo {
                name: info.product_string().unwrap_or("CH341").to_string(),
                vendor_id: VENDOR_ID,
                product_id: PRODUCT_ID,
                chip_version: None,
            },
        };
        t.init_epp()?;
        t.drain_input();
        t.info.chip_version = t.chip_version().ok();
        Ok(t)
    }

    /// Descarta lo que haya quedado en el endpoint de entrada de una sesión anterior (una
    /// respuesta de estado sin leer desfasaría todas las lecturas siguientes).
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

    /// Modo EPP 1.9 (`0x40 0xB1 0x0102 0x0000`).
    fn init_epp(&mut self) -> Result<(), OpenError> {
        self.interface
            .control_out(
                ControlOut {
                    control_type: ControlType::Vendor,
                    recipient: Recipient::Device,
                    request: REQ_PARA_INIT,
                    value: PARA_INIT_EPP19,
                    index: 0,
                    data: &[],
                },
                self.timeouts.control,
            )
            .wait()
            .map_err(|e| OpenError::Other(tr!("lhymicro.usb.epp_mode", e = e)))
    }

    fn chip_version(&mut self) -> Result<u16, TransportError> {
        let b = self
            .interface
            .control_in(
                ControlIn {
                    control_type: ControlType::Vendor,
                    recipient: Recipient::Device,
                    request: REQ_GET_VERSION,
                    value: 0,
                    index: 0,
                    length: 2,
                },
                self.timeouts.control,
            )
            .wait()
            .map_err(map_transfer)?;
        if b.len() < 2 {
            return Err(TransportError::Io(tr!("lhymicro.usb.short_version")));
        }
        Ok(((b[1] as u16) << 8) | b[0] as u16)
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
            Ok(()) => Err(TransportError::Io(tr!(
                "lhymicro.usb.partial_write",
                written = c.actual_len,
                total = data.len()
            ))),
            Err(TransferError::Stall) => {
                let _ = self.ep_out.clear_halt().wait();
                Err(TransportError::Io(tr!("lhymicro.usb.write_stall")))
            }
            Err(e) => Err(map_transfer(e)),
        }
    }

    fn read(&mut self, len: usize) -> Result<Vec<u8>, TransportError> {
        // Las lecturas deben pedirse en múltiplos del tamaño de paquete del endpoint.
        let mps = self.ep_in.max_packet_size().max(1);
        let req = len.div_ceil(mps).max(1) * mps;
        let c = self
            .ep_in
            .transfer_blocking(Buffer::new(req), self.timeouts.read);
        match c.status {
            Ok(()) => Ok(c.buffer[..].to_vec()),
            Err(TransferError::Stall) => {
                let _ = self.ep_in.clear_halt().wait();
                Err(TransportError::Io(tr!("lhymicro.usb.read_stall")))
            }
            Err(e) => Err(map_transfer(e)),
        }
    }

    fn info(&self) -> TransportInfo {
        self.info.clone()
    }
}
