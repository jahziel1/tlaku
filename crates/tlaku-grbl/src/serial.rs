//! Comunicación con GRBL. `Transport` abstrae el canal de bytes (puerto serie o
//! memoria) para poder probar el envío sin hardware.

use std::collections::VecDeque;
use std::io::{self, Write};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::time::{Duration, Instant};
use tlaku_i18n::tr;

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

/// Cuánto se insiste en escribir a un puerto cuya cola no avanza (la placa dejó de leer) antes
/// de darlo por fallido.
const WRITE_STALL_LIMIT: Duration = Duration::from_secs(30);

/// Transporte sobre un puerto serie real.
pub struct SerialTransport {
    port: Box<dyn serialport::SerialPort>,
    timeout: Duration,
}

/// Cómo se abre el puerto serie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SerialOptions {
    /// Abrir sin subir DTR: muchas placas (Arduino con GRBL) se reinician cuando DTR sube, y
    /// con esto no. Depende del sistema: en Linux el controlador sube DTR al abrir de todos
    /// modos, y algunos adaptadores lo suben igual.
    pub no_dtr_reset: bool,
}

impl SerialTransport {
    pub fn open(path: &str, baud: u32) -> io::Result<Self> {
        Self::open_with(path, baud, SerialOptions::default())
    }

    pub fn open_with(path: &str, baud: u32, options: SerialOptions) -> io::Result<Self> {
        let timeout = Duration::from_millis(100);
        let mut builder = serialport::new(path, baud).timeout(timeout);
        if options.no_dtr_reset {
            builder = builder.dtr_on_open(false);
        }
        let port = builder.open()?;
        Ok(Self { port, timeout })
    }
}

impl Transport for SerialTransport {
    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        // Sin `flush()`: en POSIX es `tcdrain`, que espera sin límite a que la placa lea todo
        // (en un USB CDC cuya placa se detiene, el hilo queda colgado y ni siquiera puede
        // cancelar). `write` ya entrega al controlador del puerto; si su cola está llena
        // (`TimedOut` tras el plazo de lectura) se reintenta hasta `WRITE_STALL_LIMIT`.
        let started = Instant::now();
        let mut sent = 0;
        while sent < data.len() {
            match Write::write(&mut self.port, &data[sent..]) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(n) => sent += n,
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::Interrupted
                            | io::ErrorKind::TimedOut
                            | io::ErrorKind::WouldBlock
                    ) && started.elapsed() < WRITE_STALL_LIMIT => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
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
    split_glued: bool,
}

impl LineReader {
    /// Lector de la familia GRBL: además separa las respuestas pegadas en una línea
    /// (`<Run|...|Ov:100,100,10ok`, `okok`, ver [`crate::glued`]). El resto de firmwares usa
    /// [`LineReader::default`].
    pub fn grbl() -> Self {
        Self {
            split_glued: true,
            ..Self::default()
        }
    }

    /// Activa o desactiva la separación de respuestas pegadas (solo familia GRBL).
    pub fn set_split_glued(&mut self, on: bool) {
        self.split_glued = on;
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        for &b in bytes {
            if b == b'\n' || b == b'\r' {
                if !self.partial.is_empty() {
                    let line = String::from_utf8_lossy(&self.partial).trim().to_string();
                    self.partial.clear();
                    if line.is_empty() {
                        continue;
                    }
                    match self.split_glued.then(|| crate::glued::split_glued(&line)) {
                        Some(Some(pieces)) => {
                            self.ready.extend(pieces.into_iter().map(str::to_string));
                        }
                        _ => self.ready.push_back(line),
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
        Self::open_with(path, baud, SerialOptions::default())
    }

    /// Como [`open`](Self::open) con opciones del puerto. Sin reinicio por DTR
    /// (`no_dtr_reset`) no hay saludo que esperar y la espera baja de 2 s a 0,3 s.
    pub fn open_with(path: &str, baud: u32, options: SerialOptions) -> io::Result<Self> {
        let mut grbl =
            Self::with_transport(Box::new(SerialTransport::open_with(path, baud, options)?));
        // Muchas placas se reinician al abrir el puerto; se espera el saludo y se descarta.
        grbl.transport.write_all(b"\r\n\r\n")?;
        std::thread::sleep(if options.no_dtr_reset {
            Duration::from_millis(300)
        } else {
            Duration::from_secs(2)
        });
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

    /// Pide un reporte de estado con `?` (tiempo real: no espera turno detrás de las líneas que
    /// la controladora tenga en su búfer) y devuelve el primero que llegue (`<Run|MPos:...>`),
    /// o `None` si no contesta a tiempo. Lo demás que llegue entretanto (los `ok` de un trabajo
    /// en marcha) se descarta.
    pub fn status_query(&mut self, timeout: Duration) -> io::Result<Option<String>> {
        self.transport.write_all(b"?")?;
        let deadline = Instant::now() + timeout;
        let mut buf = [0u8; 256];
        loop {
            while let Some(line) = self.reader.next_line() {
                if line.starts_with('<') && line.ends_with('>') {
                    return Ok(Some(line));
                }
            }
            let now = Instant::now();
            if now > deadline {
                return Ok(None);
            }
            let wait = (deadline - now).min(Duration::from_millis(100));
            let n = self.transport.read_timeout(&mut buf, wait)?;
            self.reader.feed(&buf[..n]);
        }
    }

    /// Envía un comando y devuelve las líneas de respuesta hasta `ok`.
    pub fn command(&mut self, cmd: &str, timeout: Duration) -> io::Result<Vec<String>> {
        self.transport.write_all(format!("{cmd}\n").as_bytes())?;
        let deadline = Instant::now() + timeout;
        let mut lines = Vec::new();
        let mut buf = [0u8; 256];
        loop {
            while let Some(line) = self.reader.next_line() {
                // `ok` solo en GRBL; Marlin puede añadir texto (`ok T:20`, `ok N3 P15 B3`).
                if line == "ok" || line.starts_with("ok ") {
                    return Ok(lines);
                }
                if line.starts_with("error")
                    || line.starts_with("Error:")
                    || line.starts_with("ALARM")
                {
                    return Err(io::Error::other(tr!(
                        "grbl.serial.command_reply",
                        cmd = cmd,
                        line = line
                    )));
                }
                lines.push(line);
            }
            let now = Instant::now();
            if now > deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    tr!("grbl.serial.command_timeout", cmd = cmd),
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
    let mut ports: Vec<(u8, String)> = serialport::available_ports()?
        .into_iter()
        .filter(|p| !p.port_name.starts_with("/dev/tty."))
        .map(|p| {
            let usb = matches!(p.port_type, serialport::SerialPortType::UsbPort(_));
            (port_rank(&p.port_name, usb), p.port_name)
        })
        .collect();
    // Orden estable: primero lo que parece una máquina, al final los puertos del sistema.
    ports.sort_by_key(|(rank, _)| *rank);
    Ok(ports.into_iter().map(|(_, name)| name).collect())
}

/// Qué tan probable es que el puerto sea una máquina: 0 USB, 1 otro serie, 2 puerto del sistema.
/// En macOS el primero de la lista solía ser `/dev/cu.debug-console`, y la interfaz lo elegía por
/// omisión: «no se puede conectar» con la máquina enchufada.
pub fn port_rank(name: &str, usb: bool) -> u8 {
    if is_system_port(name) {
        2
    } else if usb || looks_like_usb_serial(name) {
        0
    } else {
        1
    }
}

/// Puertos que el sistema operativo crea y nunca son una máquina láser.
pub fn is_system_port(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    [
        "bluetooth",
        "debug-console",
        "wlan-debug",
        "/dev/cu.airpods",
        "-wireless",
        "rfcomm",
    ]
    .iter()
    .any(|k| n.contains(k))
}

/// Nombres típicos de un adaptador USB-serie o un USB nativo (CDC).
fn looks_like_usb_serial(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    [
        "usbmodem",
        "usbserial",
        "wchusbserial",
        "slab_usbtouart",
        "ttyusb",
        "ttyacm",
    ]
    .iter()
    .any(|k| n.contains(k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_like_ports_go_first_and_system_ports_last() {
        assert_eq!(port_rank("/dev/cu.debug-console", false), 2);
        assert_eq!(port_rank("/dev/cu.Bluetooth-Incoming-Port", false), 2);
        assert_eq!(port_rank("/dev/cu.wlan-debug", false), 2);
        assert_eq!(port_rank("/dev/cu.usbmodem1234561", false), 0);
        assert_eq!(port_rank("/dev/cu.wchusbserial1410", false), 0);
        assert_eq!(port_rank("/dev/ttyUSB0", false), 0);
        assert_eq!(port_rank("/dev/ttyACM0", false), 0);
        assert_eq!(port_rank("COM3", true), 0);
        assert_eq!(port_rank("COM1", false), 1);
        assert!(is_system_port("/dev/cu.debug-console"));
        assert!(!is_system_port("/dev/cu.usbmodem1234561"));
    }

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
    fn line_reader_survives_invalid_utf8() {
        // L-3: bytes inválidos (CH340, cables con ruido) no hacen panic ni rompen lo siguiente.
        let mut r = LineReader::default();
        r.feed(b"ab\xff\xfe\r\nok\r\n");
        assert_eq!(r.next_line().as_deref(), Some("ab\u{fffd}\u{fffd}"));
        assert_eq!(r.next_line().as_deref(), Some("ok"));
        r.feed(b"\xff\xfe\xfd\n\xc3\n\xe2\x82\ngr\xffok\n");
        let lines: Vec<_> = std::iter::from_fn(|| r.next_line()).collect();
        assert_eq!(lines.len(), 4);
        assert!(lines.iter().all(|l| !l.is_empty()));
    }

    #[test]
    fn command_ignores_the_falcon2_banner_lines_around_the_greeting() {
        // Banner real de la Falcon2 Pro (falcon2.md, sección 3): ninguna línea que no sea
        // `ok` termina un comando, y `$I` sigue devolviendo sus líneas.
        let (host, mut dev) = memory_pair();
        let mut grbl = Grbl::with_transport(Box::new(host));
        dev.write_all(
            b"usb connect succeed\r\n\
              [OK]system start-up CV50-Pro-MASTER-Release V1.0.1 20240325-1042\r\n\
              [OK] ILmp_app_init OK\r\n[OK] gas_init OK\r\n\
              [MSG:Using cpu_map\xe2\x80\xa6esp32-s3]\r\n\
              Grbl 1.1f ['$' for help]\r\n[OK] ILmp Link connect ok\r\n\
              [VER:1.1f.20220810:]\r\n[OPT:VHL,127,65536]\r\n\
              [software versions: CV50-Pro-MASTER-Release V1.0.0 20231129-1024]\r\n\
              inquire_info->power:22W ,CV50-LASER-22W-Release V3.0.5 20231129-1001\r\nok\r\n",
        )
        .unwrap();
        let lines = grbl.command("$I", Duration::from_secs(1)).unwrap();
        assert!(lines.contains(&"[VER:1.1f.20220810:]".to_string()));
        assert!(lines.contains(&"[OPT:VHL,127,65536]".to_string()));
        assert!(lines.iter().any(|l| l.starts_with("inquire_info")));
        assert_eq!(lines.len(), 11);
    }

    #[test]
    fn command_accepts_an_info_reply_that_is_only_ok() {
        // Un hilo real mostró `$I` devolviendo solo `ok`: lista vacía, no un error.
        let (host, mut dev) = memory_pair();
        let mut grbl = Grbl::with_transport(Box::new(host));
        dev.write_all(b"ok\r\n").unwrap();
        assert_eq!(
            grbl.command("$I", Duration::from_secs(1)).unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn command_with_garbage_before_the_reply_does_not_panic() {
        let (host, mut dev) = memory_pair();
        let mut grbl = Grbl::with_transport(Box::new(host));
        dev.write_all(b"ab\xff\xfe\r\n[VER:1.1h:]\r\nok\r\n")
            .unwrap();
        let lines = grbl.command("$I", Duration::from_secs(1)).unwrap();
        assert_eq!(lines, ["ab\u{fffd}\u{fffd}", "[VER:1.1h:]"]);
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
