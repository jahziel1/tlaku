//! GRBL por red: transportes [`TcpTransport`] (Telnet o TCP crudo, como grblHAL Ethernet y el
//! servidor Telnet de FluidNC) y [`WebSocketTransport`] (FluidNC y ESP3D). Los dos implementan
//! [`Transport`], así que el `Streamer` y la conexión de la app no saben que no hay un cable.
//!
//! Diferencias con el puerto serie que se tienen en cuenta:
//!
//! - No hay reinicio al abrir: la controladora ya estaba encendida. Quien conecta puede esperar
//!   un saludo con [`drain_greeting`].
//! - Un `ok` puede llegar partido en varios paquetes (o pegado a otras líneas): el
//!   [`LineReader`](crate::serial::LineReader) trabaja byte a byte y no depende de los límites de
//!   cada lectura.
//! - Una conexión cerrada por el otro extremo es un `Err` (nunca un `Ok(0)`, que significa
//!   "nada a tiempo").
//! - Los errores de conexión se explican en español ([`explain_connect_error`]).
//!
//! Referencias públicas: Telnet (RFC 854/855) para el filtrado de negociación; WebSocket
//! (RFC 6455) a través de la biblioteca `tungstenite` (MIT/Apache-2.0).

use crate::serial::{LineReader, Transport};
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};
use tungstenite::protocol::Message;
use tungstenite::{Error as WsError, WebSocket};

/// Puerto Telnet, el que usan grblHAL Ethernet y FluidNC por omisión.
pub const DEFAULT_TCP_PORT: u16 = 23;

/// Puerto de WebSocket cuando la URL no lo indica.
pub const DEFAULT_WS_PORT: u16 = 80;

/// Espera máxima para abrir la conexión (resolver, conectar y, en WebSocket, el saludo HTTP).
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Plazo de escritura: si la máquina desapareció de la red, no se bloquea para siempre.
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

/// Prefijo de la dirección TCP en el identificador de conexión (`tcp://host:puerto`).
pub const TCP_SCHEME: &str = "tcp://";

// ---------------------------------------------------------------------------
// Errores en español
// ---------------------------------------------------------------------------

/// Explica un fallo al abrir la conexión con `target` (dirección legible) para mostrárselo al
/// usuario: qué pasó y qué revisar.
pub fn explain_connect_error(target: &str, e: &io::Error) -> String {
    use io::ErrorKind as K;
    match e.kind() {
        K::ConnectionRefused => format!(
            "La máquina en {target} rechazó la conexión. El puerto está cerrado o el servicio de \
             red de la controladora está apagado (Telnet suele usar el puerto 23)."
        ),
        K::TimedOut | K::WouldBlock => format!(
            "{target} no contestó a tiempo. Revisa que la máquina esté encendida, conectada a la \
             misma red que este equipo y que la dirección sea la correcta."
        ),
        K::HostUnreachable | K::NetworkUnreachable | K::NetworkDown => format!(
            "No se puede llegar a {target}: el equipo no es alcanzable desde esta red. Revisa el \
             Wi-Fi o el cable de red y la dirección IP."
        ),
        K::ConnectionReset | K::ConnectionAborted => {
            format!("{target} cerró la conexión al abrirla: ¿hay otro programa conectado a ella?")
        }
        _ => format!("No se pudo conectar con {target}: {e}."),
    }
}

/// Error de E/S con el mismo `kind` que `e` y el texto de [`explain_connect_error`].
fn connect_error(target: &str, e: &io::Error) -> io::Error {
    io::Error::new(e.kind(), explain_connect_error(target, e))
}

fn closed(by: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::ConnectionAborted,
        format!("{by} cerró la conexión de red."),
    )
}

/// Abre un `TcpStream` probando cada dirección a la que resuelva `host`.
fn open_stream(host: &str, port: u16, timeout: Duration) -> io::Result<TcpStream> {
    let target = if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    let addrs: Vec<SocketAddr> = (host, port)
        .to_socket_addrs()
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "No se encontró '{host}' en la red. Revisa que el nombre esté bien escrito \
                     o usa la dirección IP de la máquina."
                ),
            )
        })?
        .collect();
    let mut last = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(stream) => {
                // Comandos cortos (`?`, `!`): sin esperar a juntar paquetes.
                let _ = stream.set_nodelay(true);
                stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
                return Ok(stream);
            }
            Err(e) => last = Some(e),
        }
    }
    Err(match last {
        Some(e) => connect_error(&target, &e),
        None => io::Error::new(
            io::ErrorKind::NotFound,
            format!("'{host}' no tiene ninguna dirección de red."),
        ),
    })
}

// ---------------------------------------------------------------------------
// Telnet
// ---------------------------------------------------------------------------

const IAC: u8 = 255;
const SB: u8 = 250;
const SE: u8 = 240;
const WILL: u8 = 251;
const WONT: u8 = 252;
const DO: u8 = 253;
const DONT: u8 = 254;

/// Quita la negociación Telnet del flujo recibido y rechaza lo que el servidor proponga.
/// Un servidor de TCP crudo no manda nada de esto y el flujo pasa intacto (salvo los NUL, que
/// Telnet añade tras un `\r` suelto).
#[derive(Debug, Default)]
struct TelnetFilter {
    state: TelnetState,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum TelnetState {
    #[default]
    Data,
    Iac,
    /// Tras `WILL/WONT/DO/DONT`: falta la opción.
    Verb(u8),
    /// Dentro de una subnegociación (`IAC SB ... IAC SE`).
    Sub,
    SubIac,
}

impl TelnetFilter {
    /// Filtra `data` en el sitio y devuelve cuántos bytes de datos quedan al principio. Las
    /// respuestas de negociación (rechazos) se añaden a `replies`.
    fn process(&mut self, data: &mut [u8], replies: &mut Vec<u8>) -> usize {
        let mut out = 0;
        for i in 0..data.len() {
            let b = data[i];
            match self.state {
                TelnetState::Data => match b {
                    IAC => self.state = TelnetState::Iac,
                    0 => {}
                    _ => {
                        data[out] = b;
                        out += 1;
                    }
                },
                TelnetState::Iac => match b {
                    IAC => {
                        data[out] = IAC;
                        out += 1;
                        self.state = TelnetState::Data;
                    }
                    WILL | WONT | DO | DONT => self.state = TelnetState::Verb(b),
                    SB => self.state = TelnetState::Sub,
                    // NOP, GA y demás comandos de un byte: se ignoran.
                    _ => self.state = TelnetState::Data,
                },
                TelnetState::Verb(verb) => {
                    match verb {
                        WILL => replies.extend_from_slice(&[IAC, DONT, b]),
                        DO => replies.extend_from_slice(&[IAC, WONT, b]),
                        _ => {}
                    }
                    self.state = TelnetState::Data;
                }
                TelnetState::Sub => {
                    if b == IAC {
                        self.state = TelnetState::SubIac;
                    }
                }
                TelnetState::SubIac => {
                    self.state = match b {
                        SE => TelnetState::Data,
                        _ => TelnetState::Sub,
                    };
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// TCP
// ---------------------------------------------------------------------------

/// Transporte sobre un socket TCP: Telnet (puerto 23) o TCP crudo. La negociación Telnet, si el
/// servidor la manda, se rechaza y se descarta; los `0xFF` que se escriban se duplican como pide
/// el protocolo (GRBL no usa ese byte).
pub struct TcpTransport {
    stream: TcpStream,
    telnet: TelnetFilter,
    /// Último plazo de lectura puesto en el socket (para no repetir la llamada).
    read_timeout: Option<Duration>,
    peer: String,
}

impl TcpTransport {
    /// Abre `host:port` esperando como máximo `timeout`.
    pub fn connect(host: &str, port: u16, timeout: Duration) -> io::Result<Self> {
        let stream = open_stream(host, port, timeout)?;
        Ok(Self::from_stream(stream, format!("{host}:{port}")))
    }

    /// Usa un socket ya abierto (`peer` es el nombre que sale en los mensajes de error).
    pub fn from_stream(stream: TcpStream, peer: String) -> Self {
        Self {
            stream,
            telnet: TelnetFilter::default(),
            read_timeout: None,
            peer,
        }
    }

    fn set_read_timeout(&mut self, timeout: Duration) -> io::Result<()> {
        // Un plazo de cero significa "sin plazo" en `TcpStream`; se pide al menos 1 ms.
        let timeout = timeout.max(Duration::from_millis(1));
        if self.read_timeout != Some(timeout) {
            self.stream.set_read_timeout(Some(timeout))?;
            self.read_timeout = Some(timeout);
        }
        Ok(())
    }
}

impl Transport for TcpTransport {
    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        if data.contains(&IAC) {
            let mut escaped = Vec::with_capacity(data.len() + 4);
            for &b in data {
                escaped.push(b);
                if b == IAC {
                    escaped.push(IAC);
                }
            }
            self.stream.write_all(&escaped)?;
        } else {
            self.stream.write_all(data)?;
        }
        self.stream.flush()
    }

    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let deadline = Instant::now() + timeout;
        loop {
            self.set_read_timeout(deadline.saturating_duration_since(Instant::now()))?;
            match self.stream.read(buf) {
                Ok(0) => return Err(closed(&self.peer)),
                Ok(n) => {
                    let mut replies = Vec::new();
                    let kept = self.telnet.process(&mut buf[..n], &mut replies);
                    if !replies.is_empty() {
                        self.stream.write_all(&replies)?;
                    }
                    if kept > 0 {
                        return Ok(kept);
                    }
                    // Solo era negociación: se sigue esperando datos hasta el plazo.
                    if Instant::now() >= deadline {
                        return Ok(0);
                    }
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    return Ok(0);
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }

    fn clear_input(&mut self) -> io::Result<()> {
        self.stream.set_nonblocking(true)?;
        let mut scratch = [0u8; 1024];
        let result = loop {
            match self.stream.read(&mut scratch) {
                Ok(0) => break Ok(()),
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break Ok(()),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => break Err(e),
            }
        };
        self.stream.set_nonblocking(false)?;
        self.telnet = TelnetFilter::default();
        result
    }
}

impl Drop for TcpTransport {
    fn drop(&mut self) {
        // Cierre ordenado: el otro extremo ve el fin de la conexión y no un reinicio.
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

// ---------------------------------------------------------------------------
// WebSocket
// ---------------------------------------------------------------------------

/// Mensajes de texto de la interfaz web de ESP3D y FluidNC que no son parte del flujo de GRBL
/// (identificadores de sesión y latido). Se descartan para no confundir al lector de líneas.
/// (Inferido de la interfaz web de FluidNC/ESP3D; no verificado con hardware.)
const WS_CONTROL_PREFIXES: [&str; 3] = ["currentID:", "activeID:", "PING:"];

/// Transporte sobre un WebSocket `ws://`. El flujo de la controladora viaja en mensajes de
/// texto o binarios, que se concatenan; lo que se escribe va como texto, o como binario si
/// contiene bytes que no son UTF-8 (los comandos en tiempo real `0x80..`).
pub struct WebSocketTransport {
    ws: WebSocket<TcpStream>,
    pending: VecDeque<u8>,
    read_timeout: Option<Duration>,
    peer: String,
}

/// Separa `ws://host[:puerto][/ruta]` en (host, puerto). `wss://` no está soportado.
pub fn parse_ws_url(url: &str) -> Result<(String, u16), String> {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("wss://") {
        return Err(
            "Las conexiones cifradas (wss://) no están soportadas; usa una dirección \
             ws:// (los módulos de las controladoras no suelen cifrar)."
                .into(),
        );
    }
    if !lower.starts_with("ws://") {
        return Err(
            "La dirección WebSocket debe empezar por ws://, por ejemplo \
             ws://192.168.1.50:81"
                .into(),
        );
    }
    let rest = &url["ws://".len()..];
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let (host, port) = split_host_port(authority, DEFAULT_WS_PORT)?;
    Ok((host, port))
}

/// Separa `host[:puerto]` (con `[::1]` para IPv6). Sin puerto vale `default_port`.
pub fn split_host_port(text: &str, default_port: u16) -> Result<(String, u16), String> {
    let text = text.trim();
    let (host, port_text) = if let Some(rest) = text.strip_prefix('[') {
        let (host, after) = rest
            .split_once(']')
            .ok_or("Falta el corchete de cierre de la dirección IPv6.")?;
        (host, after.strip_prefix(':'))
    } else if let Some((host, port)) = text.rsplit_once(':') {
        (host, Some(port))
    } else {
        (text, None)
    };
    if host.is_empty() || host.chars().any(|c| c.is_whitespace() || c == '/') {
        return Err("Falta la dirección de la máquina (nombre o IP).".into());
    }
    let port =
        match port_text {
            None => default_port,
            Some(p) => p.parse::<u16>().ok().filter(|p| *p != 0).ok_or_else(|| {
                format!("El puerto '{p}' no es válido (usa un número de 1 a 65535).")
            })?,
        };
    Ok((host.to_string(), port))
}

fn ws_error(peer: &str, e: WsError) -> io::Error {
    match e {
        WsError::Io(e) => e,
        WsError::ConnectionClosed | WsError::AlreadyClosed => closed(peer),
        other => io::Error::other(format!("Error de WebSocket con {peer}: {other}")),
    }
}

impl WebSocketTransport {
    /// Abre `url` (`ws://host:puerto/ruta`) y completa el saludo HTTP, todo dentro de `timeout`.
    pub fn connect(url: &str, timeout: Duration) -> io::Result<Self> {
        let invalid = |m: String| io::Error::new(io::ErrorKind::InvalidInput, m);
        let (host, port) = parse_ws_url(url).map_err(invalid)?;
        let stream = open_stream(&host, port, timeout)?;
        stream.set_read_timeout(Some(timeout))?;
        let peer = url.trim().to_string();
        let handshake = tungstenite::client(peer.as_str(), stream);
        let (ws, _response) = match handshake {
            Ok(done) => done,
            Err(tungstenite::HandshakeError::Failure(e)) => {
                return Err(match e {
                    WsError::Io(e) => connect_error(&peer, &e),
                    other => io::Error::other(format!(
                        "{peer} no aceptó la conexión WebSocket: {other}. Revisa que sea la \
                         dirección del WebSocket de la máquina (FluidNC usa el puerto 81)."
                    )),
                });
            }
            Err(tungstenite::HandshakeError::Interrupted(_)) => {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    explain_connect_error(&peer, &io::ErrorKind::TimedOut.into()),
                ));
            }
        };
        let transport = Self {
            ws,
            pending: VecDeque::new(),
            read_timeout: None,
            peer,
        };
        transport
            .ws
            .get_ref()
            .set_write_timeout(Some(WRITE_TIMEOUT))?;
        Ok(transport)
    }

    fn set_read_timeout(&mut self, timeout: Duration) -> io::Result<()> {
        let timeout = timeout.max(Duration::from_millis(1));
        if self.read_timeout != Some(timeout) {
            self.ws.get_ref().set_read_timeout(Some(timeout))?;
            self.read_timeout = Some(timeout);
        }
        Ok(())
    }

    /// Guarda el contenido de un mensaje recibido (los de control de ESP3D se descartan).
    fn accept(&mut self, msg: Message) -> io::Result<()> {
        match msg {
            Message::Text(text) => {
                let s: &str = text.as_ref();
                if !WS_CONTROL_PREFIXES.iter().any(|p| s.starts_with(p)) {
                    self.pending.extend(s.as_bytes());
                }
            }
            Message::Binary(bytes) => self.pending.extend(bytes.iter()),
            Message::Close(_) => return Err(closed(&self.peer)),
            // Ping/Pong los atiende la biblioteca (el Pong sale con la siguiente escritura).
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
        }
        Ok(())
    }
}

impl Transport for WebSocketTransport {
    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        if data.is_empty() {
            return Ok(());
        }
        let msg = match std::str::from_utf8(data) {
            Ok(text) => Message::text(text),
            Err(_) => Message::binary(data.to_vec()),
        };
        self.ws.send(msg).map_err(|e| ws_error(&self.peer, e))
    }

    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let deadline = Instant::now() + timeout;
        loop {
            if !self.pending.is_empty() {
                let n = buf.len().min(self.pending.len());
                for (slot, byte) in buf.iter_mut().zip(self.pending.drain(..n)) {
                    *slot = byte;
                }
                return Ok(n);
            }
            self.set_read_timeout(deadline.saturating_duration_since(Instant::now()))?;
            match self.ws.read() {
                Ok(msg) => self.accept(msg)?,
                Err(WsError::Io(e))
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    return Ok(0);
                }
                Err(e) => return Err(ws_error(&self.peer, e)),
            }
            if self.pending.is_empty() && Instant::now() >= deadline {
                return Ok(0);
            }
        }
    }

    fn clear_input(&mut self) -> io::Result<()> {
        self.pending.clear();
        self.ws.get_ref().set_nonblocking(true)?;
        let result = loop {
            match self.ws.read() {
                Ok(Message::Close(_)) => break Ok(()),
                Ok(_) => {}
                Err(WsError::Io(e)) if e.kind() == io::ErrorKind::WouldBlock => break Ok(()),
                Err(WsError::ConnectionClosed | WsError::AlreadyClosed) => break Ok(()),
                Err(e) => break Err(ws_error(&self.peer, e)),
            }
        };
        self.ws.get_ref().set_nonblocking(false)?;
        self.pending.clear();
        result
    }
}

impl Drop for WebSocketTransport {
    fn drop(&mut self) {
        // Cierre ordenado: trama Close y luego el socket. Sin esperar la respuesta.
        let _ = self.ws.close(None);
        let _ = self.ws.flush();
        let _ = self.ws.get_ref().shutdown(Shutdown::Both);
    }
}

// ---------------------------------------------------------------------------
// Direcciones de red
// ---------------------------------------------------------------------------

/// Destino de red de una conexión, tal como lo guarda el perfil de la máquina.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Endpoint {
    Tcp { host: String, port: u16 },
    WebSocket { url: String },
}

impl Endpoint {
    /// Interpreta el identificador de conexión que usa la app en lugar del puerto serie:
    /// `tcp://host[:puerto]` o `ws://host[:puerto][/ruta]`. `None` si no es una dirección de red
    /// (un puerto serie); `Some(Err)` si lo parece pero está mal escrita.
    pub fn parse(id: &str) -> Option<Result<Self, String>> {
        let lower = id.trim().to_ascii_lowercase();
        if lower.starts_with(TCP_SCHEME) {
            let rest = &id.trim()[TCP_SCHEME.len()..];
            Some(
                split_host_port(rest.trim_end_matches('/'), DEFAULT_TCP_PORT)
                    .map(|(host, port)| Self::Tcp { host, port }),
            )
        } else if lower.starts_with("ws://") || lower.starts_with("wss://") {
            Some(parse_ws_url(id).map(|_| Self::WebSocket {
                url: id.trim().to_string(),
            }))
        } else {
            None
        }
    }

    /// Identificador de conexión (el inverso de [`parse`](Self::parse)).
    pub fn id(&self) -> String {
        match self {
            Self::Tcp { host, port } if host.contains(':') => {
                format!("{TCP_SCHEME}[{host}]:{port}")
            }
            Self::Tcp { host, port } => format!("{TCP_SCHEME}{host}:{port}"),
            Self::WebSocket { url } => url.clone(),
        }
    }

    /// Dirección para mostrar al usuario (`192.168.1.50:23` o la URL).
    pub fn label(&self) -> String {
        match self {
            Self::Tcp { .. } => self.id()[TCP_SCHEME.len()..].to_string(),
            Self::WebSocket { url } => url.clone(),
        }
    }

    /// Abre el transporte. Los errores ya vienen explicados en español.
    pub fn open(&self, timeout: Duration) -> io::Result<Box<dyn Transport>> {
        Ok(match self {
            Self::Tcp { host, port } => Box::new(TcpTransport::connect(host, *port, timeout)?),
            Self::WebSocket { url } => Box::new(WebSocketTransport::connect(url, timeout)?),
        })
    }
}

/// Espera el saludo que algunas controladoras mandan al conectar por red (`Grbl 1.1f ['$' for
/// help]`) y lo descarta: lee hasta `quiet` de silencio o `max` en total. Devuelve las líneas
/// recibidas, por si sirven (p. ej. para registrarlas).
pub fn drain_greeting(
    t: &mut dyn Transport,
    quiet: Duration,
    max: Duration,
) -> io::Result<Vec<String>> {
    let deadline = Instant::now() + max;
    let mut reader = LineReader::default();
    let mut lines = Vec::new();
    let mut buf = [0u8; 512];
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        let n = t.read_timeout(&mut buf, quiet.min(left))?;
        if n == 0 {
            break;
        }
        reader.feed(&buf[..n]);
        while let Some(line) = reader.next_line() {
            lines.push(line);
        }
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serial::Grbl;
    use std::net::TcpListener;
    use std::thread;

    fn listener() -> (TcpListener, u16) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        (l, port)
    }

    fn connect(port: u16) -> TcpTransport {
        TcpTransport::connect("127.0.0.1", port, Duration::from_secs(2)).unwrap()
    }

    #[test]
    fn telnet_filter_strips_negotiation_and_refuses_options() {
        let mut f = TelnetFilter::default();
        let mut replies = Vec::new();
        let mut data = vec![
            IAC, DO, 1, b'o', IAC, WILL, 3, b'k', 0, b'\r', b'\n', IAC, SB, 24, 1, IAC, SE, b'!',
            IAC, IAC, IAC, 241,
        ];
        let n = f.process(&mut data, &mut replies);
        assert_eq!(&data[..n], [b'o', b'k', b'\r', b'\n', b'!', 255]);
        assert_eq!(replies, [IAC, WONT, 1, IAC, DONT, 3]);
    }

    #[test]
    fn telnet_filter_keeps_state_across_packets() {
        let mut f = TelnetFilter::default();
        let mut replies = Vec::new();
        let mut all = Vec::new();
        for chunk in [&[b'a', IAC][..], &[DO][..], &[1, b'b'][..]] {
            let mut c = chunk.to_vec();
            let n = f.process(&mut c, &mut replies);
            all.extend_from_slice(&c[..n]);
        }
        assert_eq!(all, b"ab");
        assert_eq!(replies, [IAC, WONT, 1]);
    }

    #[test]
    fn tcp_roundtrip_timeout_and_clean_close() {
        let (l, port) = listener();
        let server = thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut got = [0u8; 8];
            let n = s.read(&mut got).unwrap();
            s.write_all(&got[..n]).unwrap();
            // Al soltar el socket el cliente debe ver el cierre.
        });
        let mut t = connect(port);
        let mut buf = [0u8; 16];
        assert_eq!(
            t.read_timeout(&mut buf, Duration::from_millis(20)).unwrap(),
            0
        );
        t.write_all(b"$I\n").unwrap();
        let n = t.read_timeout(&mut buf, Duration::from_secs(2)).unwrap();
        assert_eq!(&buf[..n], b"$I\n");
        server.join().unwrap();
        let e = t
            .read_timeout(&mut buf, Duration::from_secs(2))
            .unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::ConnectionAborted);
        assert!(e.to_string().contains("cerró la conexión"));
    }

    #[test]
    fn tcp_read_with_zero_timeout_does_not_block() {
        let (l, port) = listener();
        let server = thread::spawn(move || {
            let (_s, _) = l.accept().unwrap();
            thread::sleep(Duration::from_millis(300));
        });
        let mut t = connect(port);
        let start = Instant::now();
        let mut buf = [0u8; 4];
        assert_eq!(t.read_timeout(&mut buf, Duration::ZERO).unwrap(), 0);
        assert!(start.elapsed() < Duration::from_millis(200));
        drop(t);
        server.join().unwrap();
    }

    #[test]
    fn tcp_clear_input_discards_the_pending_greeting() {
        let (l, port) = listener();
        let server = thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            s.write_all(b"Grbl 1.1f ['$' for help]\r\n").unwrap();
            thread::sleep(Duration::from_millis(400));
        });
        let mut t = connect(port);
        thread::sleep(Duration::from_millis(150));
        t.clear_input().unwrap();
        let mut buf = [0u8; 64];
        assert_eq!(
            t.read_timeout(&mut buf, Duration::from_millis(30)).unwrap(),
            0
        );
        server.join().unwrap();
    }

    #[test]
    fn tcp_ok_split_in_several_packets_is_still_one_ok() {
        // La respuesta llega byte a byte, con pausas entre paquetes y el `\r\n` partido.
        let (l, port) = listener();
        let server = thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            s.set_nodelay(true).unwrap();
            let mut got = [0u8; 16];
            let n = s.read(&mut got).unwrap();
            assert_eq!(&got[..n], b"$I\n");
            for chunk in [
                &b"[VER:1.1f"[..],
                b".20220810:]\r",
                b"\n[OPT:V",
                b"]\r\no",
                b"k",
                b"\r",
                b"\n",
            ] {
                s.write_all(chunk).unwrap();
                thread::sleep(Duration::from_millis(15));
            }
        });
        let mut grbl = Grbl::with_transport(Box::new(connect(port)));
        let lines = grbl.command("$I", Duration::from_secs(3)).unwrap();
        assert_eq!(lines, ["[VER:1.1f.20220810:]", "[OPT:V]"]);
        server.join().unwrap();
    }

    #[test]
    fn tcp_telnet_negotiation_is_refused_and_hidden_from_the_reader() {
        let (l, port) = listener();
        let server = thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            s.write_all(&[IAC, DO, 1, IAC, WILL, 3]).unwrap();
            let mut got = [0u8; 16];
            // Primero llegan los rechazos y después el comando.
            let mut all = Vec::new();
            while all.len() < 6 + 4 {
                let n = s.read(&mut got).unwrap();
                all.extend_from_slice(&got[..n]);
            }
            s.write_all(b"ok\r\n").unwrap();
            all
        });
        let mut grbl = Grbl::with_transport(Box::new(connect(port)));
        // El cliente debe leer (y así contestar) antes de que el servidor espere los rechazos.
        let h = thread::spawn(move || {
            grbl.command("G90", Duration::from_secs(3)).unwrap();
        });
        let got = server.join().unwrap();
        h.join().unwrap();
        // El orden entre el comando y los rechazos no está fijado; deben llegar los dos.
        let has = |needle: &[u8]| got.windows(needle.len()).any(|w| w == needle);
        assert!(has(&[IAC, WONT, 1]) && has(&[IAC, DONT, 3]), "{got:?}");
        assert!(has(b"G90\n"), "{got:?}");
        assert_eq!(got.len(), 10);
    }

    #[test]
    fn tcp_refused_has_a_clear_message() {
        let (l, port) = listener();
        drop(l);
        let e = TcpTransport::connect("127.0.0.1", port, Duration::from_secs(2))
            .err()
            .unwrap();
        assert_eq!(e.kind(), io::ErrorKind::ConnectionRefused);
        assert!(e.to_string().contains("rechazó la conexión"), "{e}");
        assert!(e.to_string().contains(&port.to_string()), "{e}");
    }

    #[test]
    fn tcp_unknown_host_has_a_clear_message() {
        let e = TcpTransport::connect("host-que-no-existe.invalid", 23, Duration::from_secs(2))
            .err()
            .unwrap();
        assert!(e.to_string().contains("No se encontró"), "{e}");
    }

    #[test]
    fn explain_covers_timeouts_and_unreachable() {
        let t = explain_connect_error("10.0.0.9:23", &io::ErrorKind::TimedOut.into());
        assert!(t.contains("no contestó a tiempo"));
        let u = explain_connect_error("10.0.0.9:23", &io::ErrorKind::HostUnreachable.into());
        assert!(u.contains("no es alcanzable"));
    }

    #[test]
    fn host_and_port_parsing() {
        assert_eq!(
            split_host_port("192.168.1.5", 23),
            Ok(("192.168.1.5".into(), 23))
        );
        assert_eq!(
            split_host_port("fluidnc.local:8080", 23),
            Ok(("fluidnc.local".into(), 8080))
        );
        assert_eq!(split_host_port("[::1]:99", 23), Ok(("::1".into(), 99)));
        assert_eq!(split_host_port("[::1]", 23), Ok(("::1".into(), 23)));
        assert!(split_host_port("", 23).is_err());
        assert!(split_host_port("host:0", 23).is_err());
        assert!(split_host_port("host:abc", 23).is_err());
        assert!(split_host_port("host:70000", 23).is_err());
        assert!(split_host_port("a b", 23).is_err());
    }

    #[test]
    fn ws_url_parsing() {
        assert_eq!(
            parse_ws_url("ws://fluidnc.local:81/"),
            Ok(("fluidnc.local".into(), 81))
        );
        assert_eq!(
            parse_ws_url("ws://10.0.0.2/ws"),
            Ok(("10.0.0.2".into(), 80))
        );
        assert!(
            parse_ws_url("wss://x")
                .unwrap_err()
                .contains("no están soportadas")
        );
        assert!(parse_ws_url("http://x").is_err());
        assert!(parse_ws_url("ws://").is_err());
    }

    #[test]
    fn endpoint_parse_and_id() {
        assert_eq!(Endpoint::parse("/dev/cu.usbserial"), None);
        assert_eq!(Endpoint::parse("COM3"), None);
        assert_eq!(Endpoint::parse("virtual:falcon2"), None);
        let tcp = Endpoint::parse("tcp://192.168.1.5").unwrap().unwrap();
        assert_eq!(
            tcp,
            Endpoint::Tcp {
                host: "192.168.1.5".into(),
                port: 23
            }
        );
        assert_eq!(tcp.id(), "tcp://192.168.1.5:23");
        assert_eq!(tcp.label(), "192.168.1.5:23");
        let v6 = Endpoint::parse("tcp://[::1]:99").unwrap().unwrap();
        assert_eq!(v6.id(), "tcp://[::1]:99");
        let ws = Endpoint::parse("ws://h:81/").unwrap().unwrap();
        assert_eq!(ws.label(), "ws://h:81/");
        assert!(Endpoint::parse("tcp://").unwrap().is_err());
        assert!(Endpoint::parse("wss://h").unwrap().is_err());
    }

    #[test]
    fn drain_greeting_returns_lines_and_stops_on_silence() {
        let (l, port) = listener();
        let server = thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            s.write_all(b"Grbl 1.1f ['$' for help]\r\n").unwrap();
            thread::sleep(Duration::from_millis(300));
        });
        let mut t = connect(port);
        let lines =
            drain_greeting(&mut t, Duration::from_millis(80), Duration::from_secs(2)).unwrap();
        assert_eq!(lines, ["Grbl 1.1f ['$' for help]"]);
        server.join().unwrap();
    }

    // --- WebSocket ---

    /// Servidor WebSocket de una sola conexión; `script` recibe el socket ya negociado.
    fn ws_server(
        script: impl FnOnce(WebSocket<TcpStream>) + Send + 'static,
    ) -> (String, thread::JoinHandle<()>) {
        let (l, port) = listener();
        let h = thread::spawn(move || {
            let (s, _) = l.accept().unwrap();
            let ws = tungstenite::accept(s).unwrap();
            script(ws);
        });
        (format!("ws://127.0.0.1:{port}/"), h)
    }

    #[test]
    fn ws_roundtrip_ignores_control_text_and_joins_messages() {
        let (url, server) = ws_server(|mut ws| {
            ws.send(Message::text("currentID:123")).unwrap();
            ws.send(Message::text("activeID:123")).unwrap();
            ws.send(Message::text("PING:60:60")).unwrap();
            let Message::Text(cmd) = ws.read().unwrap() else {
                panic!("se esperaba texto");
            };
            assert_eq!(cmd.as_str(), "$I\n");
            // Un `ok` partido en dos mensajes, uno de ellos binario.
            ws.send(Message::text("[VER:1.1f:]\r\no")).unwrap();
            ws.send(Message::binary(b"k\r\n".to_vec())).unwrap();
            // Un byte en tiempo real debe llegar como binario.
            let msg = ws.read().unwrap();
            assert_eq!(msg.into_data().as_ref(), [0x90u8]);
            let _ = ws.read();
        });
        let mut grbl = Grbl::with_transport(Box::new(
            WebSocketTransport::connect(&url, Duration::from_secs(2)).unwrap(),
        ));
        let lines = grbl.command("$I", Duration::from_secs(3)).unwrap();
        assert_eq!(lines, ["[VER:1.1f:]"]);
        let mut t = grbl.into_transport();
        t.write_all(&[0x90]).unwrap();
        drop(t);
        server.join().unwrap();
    }

    #[test]
    fn ws_timeout_and_server_close() {
        let (url, server) = ws_server(|mut ws| {
            thread::sleep(Duration::from_millis(150));
            let _ = ws.close(None);
            let _ = ws.flush();
        });
        let mut t = WebSocketTransport::connect(&url, Duration::from_secs(2)).unwrap();
        let mut buf = [0u8; 8];
        assert_eq!(
            t.read_timeout(&mut buf, Duration::from_millis(30)).unwrap(),
            0
        );
        let e = t
            .read_timeout(&mut buf, Duration::from_secs(2))
            .unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::ConnectionAborted);
        server.join().unwrap();
    }

    #[test]
    fn ws_errors_are_clear() {
        let (l, port) = listener();
        drop(l);
        let e =
            WebSocketTransport::connect(&format!("ws://127.0.0.1:{port}"), Duration::from_secs(2))
                .err()
                .unwrap();
        assert!(e.to_string().contains("rechazó la conexión"), "{e}");
        let e = WebSocketTransport::connect("wss://127.0.0.1:81", Duration::from_secs(2))
            .err()
            .unwrap();
        assert_eq!(e.kind(), io::ErrorKind::InvalidInput);
        // Un servidor que no habla WebSocket (responde otra cosa) tampoco debe colgar.
        let (l, port) = listener();
        let server = thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut buf = [0u8; 1024];
            let _ = s.read(&mut buf);
            let _ = s.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
        });
        let e =
            WebSocketTransport::connect(&format!("ws://127.0.0.1:{port}"), Duration::from_secs(2))
                .err()
                .unwrap();
        assert!(
            e.to_string().contains("no aceptó la conexión WebSocket"),
            "{e}"
        );
        server.join().unwrap();
    }
}
