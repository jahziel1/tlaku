//! Comunicación con GRBL. `Transport` abstrae el canal de bytes (puerto serie o
//! memoria) para poder probar el envío sin hardware.

use std::collections::VecDeque;
use std::io::{self, Write};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::time::{Duration, Instant};

pub const DEFAULT_BAUD: u32 = 115_200;

/// Canal de bytes bidireccional con espera máxima en la lectura.
pub trait Transport: Send {
    /// Escribe todos los bytes (y los vacía hacia el dispositivo).
    fn write_all(&mut self, data: &[u8]) -> io::Result<()>;

    /// Lee hasta `buf.len()` bytes esperando como máximo `timeout`. Devuelve
    /// `Ok(0)` si no llegó nada a tiempo; un canal cerrado es un `Err`.
    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize>;

    /// Descarta lo recibido y aún no leído.
    fn clear_input(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Transport for Box<dyn Transport> {
    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        (**self).write_all(data)
    }

    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        (**self).read_timeout(buf, timeout)
    }

    fn clear_input(&mut self) -> io::Result<()> {
        (**self).clear_input()
    }
}

/// Transporte sobre un puerto serie real.
pub struct SerialTransport {
    port: Box<dyn serialport::SerialPort>,
    timeout: Duration,
}

impl SerialTransport {
    pub fn open(path: &str, baud: u32) -> io::Result<Self> {
        let timeout = Duration::from_millis(100);
        let port = serialport::new(path, baud).timeout(timeout).open()?;
        Ok(Self { port, timeout })
    }
}

impl Transport for SerialTransport {
    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        Write::write_all(&mut self.port, data)?;
        self.port.flush()
    }

    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        // Un plazo de cero se interpretaría como "sin espera" según la plataforma.
        let timeout = timeout.max(Duration::from_millis(1));
        if timeout != self.timeout {
            self.port.set_timeout(timeout)?;
            self.timeout = timeout;
        }
        match io::Read::read(&mut self.port, buf) {
            Ok(n) => Ok(n),
            Err(e) if e.kind() == io::ErrorKind::TimedOut => Ok(0),
            Err(e) => Err(e),
        }
    }

    fn clear_input(&mut self) -> io::Result<()> {
        Ok(self.port.clear(serialport::ClearBuffer::Input)?)
    }
}

/// Un extremo de un canal en memoria (ver [`memory_pair`]). Si el otro extremo
/// se suelta, la lectura (tras agotar lo pendiente) y la escritura fallan con
/// `BrokenPipe`, igual que un cable desconectado.
pub struct MemoryTransport {
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
    leftover: VecDeque<u8>,
}

/// Par de extremos conectados entre sí: lo escrito en uno se lee en el otro.
pub fn memory_pair() -> (MemoryTransport, MemoryTransport) {
    let (tx_a, rx_b) = mpsc::channel();
    let (tx_b, rx_a) = mpsc::channel();
    let end = |tx, rx| MemoryTransport {
        tx,
        rx,
        leftover: VecDeque::new(),
    };
    (end(tx_a, rx_a), end(tx_b, rx_b))
}

impl Transport for MemoryTransport {
    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        if data.is_empty() {
            return Ok(());
        }
        self.tx
            .send(data.to_vec())
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "el otro extremo se cerró"))
    }

    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        if self.leftover.is_empty() {
            match self.rx.recv_timeout(timeout) {
                Ok(chunk) => self.leftover.extend(chunk),
                Err(RecvTimeoutError::Timeout) => return Ok(0),
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "el otro extremo se cerró",
                    ));
                }
            }
        }
        let n = buf.len().min(self.leftover.len());
        for (slot, byte) in buf.iter_mut().zip(self.leftover.drain(..n)) {
            *slot = byte;
        }
        Ok(n)
    }

    fn clear_input(&mut self) -> io::Result<()> {
        self.leftover.clear();
        loop {
            match self.rx.try_recv() {
                Ok(_) => {}
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return Ok(()),
            }
        }
    }
}

/// Máximo de bytes acumulados sin fin de línea antes de descartarlos como basura.
const MAX_PARTIAL_LINE: usize = 64 * 1024;

/// Separa el flujo recibido en líneas (`\n` o `\r` como terminador, vacías omitidas).
#[derive(Default)]
pub struct LineReader {
    partial: Vec<u8>,
    ready: VecDeque<String>,
}

impl LineReader {
    pub fn feed(&mut self, bytes: &[u8]) {
        for &b in bytes {
            if b == b'\n' || b == b'\r' {
                if !self.partial.is_empty() {
                    let line = String::from_utf8_lossy(&self.partial).trim().to_string();
                    self.partial.clear();
                    if !line.is_empty() {
                        self.ready.push_back(line);
                    }
                }
            } else {
                self.partial.push(b);
            }
        }
        if self.partial.len() > MAX_PARTIAL_LINE {
            self.partial.clear();
        }
    }

    pub fn next_line(&mut self) -> Option<String> {
        self.ready.pop_front()
    }

    pub fn clear(&mut self) {
        self.partial.clear();
        self.ready.clear();
    }
}

pub struct Grbl {
    transport: Box<dyn Transport>,
    reader: LineReader,
}

impl Grbl {
    pub fn open(path: &str, baud: u32) -> io::Result<Self> {
        let mut grbl = Self::with_transport(Box::new(SerialTransport::open(path, baud)?));
        // Muchas placas se reinician al abrir el puerto; se espera el saludo y se descarta.
        grbl.transport.write_all(b"\r\n\r\n")?;
        std::thread::sleep(Duration::from_secs(2));
        grbl.transport.clear_input()?;
        Ok(grbl)
    }

    /// Usa un transporte ya listo, sin saludo ni espera de reinicio.
    pub fn with_transport(transport: Box<dyn Transport>) -> Self {
        Self {
            transport,
            reader: LineReader::default(),
        }
    }

    /// Entrega el transporte (p. ej. a un `Streamer`); lo recibido a medias se pierde.
    pub fn into_transport(self) -> Box<dyn Transport> {
        self.transport
    }

    /// Envía un comando y devuelve las líneas de respuesta hasta `ok`.
    pub fn command(&mut self, cmd: &str, timeout: Duration) -> io::Result<Vec<String>> {
        self.transport.write_all(format!("{cmd}\n").as_bytes())?;
        let deadline = Instant::now() + timeout;
        let mut lines = Vec::new();
        let mut buf = [0u8; 256];
        loop {
            while let Some(line) = self.reader.next_line() {
                if line == "ok" {
                    return Ok(lines);
                }
                if line.starts_with("error") || line.starts_with("ALARM") {
                    return Err(io::Error::other(format!("'{cmd}' respondió {line}")));
                }
                lines.push(line);
            }
            let now = Instant::now();
            if now > deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("sin respuesta a '{cmd}'"),
                ));
            }
            let wait = (deadline - now).min(Duration::from_millis(100));
            let n = self.transport.read_timeout(&mut buf, wait)?;
            self.reader.feed(&buf[..n]);
        }
    }
}

/// Puertos serie disponibles (en macOS solo los `cu.*`, que son los de salida).
pub fn list_ports() -> io::Result<Vec<String>> {
    Ok(serialport::available_ports()?
        .into_iter()
        .map(|p| p.port_name)
        .filter(|n| !n.starts_with("/dev/tty."))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_reader_splits_crlf_and_partial() {
        let mut r = LineReader::default();
        r.feed(b"ok\r\n<Idle|MPos:0,0,0>\r");
        r.feed(b"\nerr");
        assert_eq!(r.next_line().as_deref(), Some("ok"));
        assert_eq!(r.next_line().as_deref(), Some("<Idle|MPos:0,0,0>"));
        assert_eq!(r.next_line(), None);
        r.feed(b"or:3\n\n\n");
        assert_eq!(r.next_line().as_deref(), Some("error:3"));
        assert_eq!(r.next_line(), None);
    }

    #[test]
    fn line_reader_drops_runaway_garbage() {
        let mut r = LineReader::default();
        r.feed(&vec![b'x'; MAX_PARTIAL_LINE + 1]);
        r.feed(b"ok\n");
        assert_eq!(r.next_line().as_deref(), Some("ok"));
    }

    #[test]
    fn memory_pair_roundtrip_timeout_and_close() {
        let (mut a, mut b) = memory_pair();
        let mut buf = [0u8; 4];
        assert_eq!(
            b.read_timeout(&mut buf, Duration::from_millis(1)).unwrap(),
            0
        );
        a.write_all(b"hello").unwrap();
        assert_eq!(
            b.read_timeout(&mut buf, Duration::from_millis(50)).unwrap(),
            4
        );
        assert_eq!(&buf, b"hell");
        assert_eq!(
            b.read_timeout(&mut buf, Duration::from_millis(50)).unwrap(),
            1
        );
        a.write_all(b"x").unwrap();
        b.clear_input().unwrap();
        assert_eq!(
            b.read_timeout(&mut buf, Duration::from_millis(1)).unwrap(),
            0
        );
        a.write_all(b"last").unwrap();
        drop(a);
        // Lo pendiente se entrega antes de reportar el cierre.
        assert_eq!(
            b.read_timeout(&mut buf, Duration::from_millis(50)).unwrap(),
            4
        );
        let err = b
            .read_timeout(&mut buf, Duration::from_millis(1))
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);
        assert!(b.write_all(b"x").is_err());
    }

    #[test]
    fn command_collects_lines_until_ok() {
        let (host, mut dev) = memory_pair();
        let mut grbl = Grbl::with_transport(Box::new(host));
        dev.write_all(b"[VER:1.1h:]\r\n[OPT:V,15,128]\r\nok\r\n")
            .unwrap();
        let lines = grbl.command("$I", Duration::from_secs(1)).unwrap();
        assert_eq!(lines, ["[VER:1.1h:]", "[OPT:V,15,128]"]);
        let mut buf = [0u8; 8];
        let n = dev
            .read_timeout(&mut buf, Duration::from_millis(50))
            .unwrap();
        assert_eq!(&buf[..n], b"$I\n");
    }

    #[test]
    fn command_reports_error_and_timeout() {
        let (host, mut dev) = memory_pair();
        let mut grbl = Grbl::with_transport(Box::new(host));
        dev.write_all(b"error:20\r\n").unwrap();
        let e = grbl.command("G99", Duration::from_secs(1)).unwrap_err();
        assert!(e.to_string().contains("error:20"));
        let e = grbl.command("$$", Duration::from_millis(30)).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::TimedOut);
    }
}
