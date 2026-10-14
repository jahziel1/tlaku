//! Cámaras de red (M35): el motor pide un cuadro a una cámara IP por HTTP y lo entrega a la
//! interfaz. El navegador de Tauri no deja leer ese cuadro (CORS y lienzo «contaminado» al subirlo
//! a WebGL), así que la petición sale de aquí, como un proxy de un solo cuadro por llamada.
//!
//! Qué admite:
//! - `http://` (sin TLS: las cámaras de la red local casi nunca lo traen; `https://` se rechaza
//!   con un mensaje claro).
//! - Snapshot (`image/jpeg`, `image/png`, `image/bmp`, con `Content-Length` o por fragmentos) y
//!   MJPEG (`multipart/x-mixed-replace`, o JPEG sin fin): se devuelve el primer cuadro completo y
//!   se cierra la conexión.
//! - Usuario y clave en la URL (`http://usuario:clave@host/...`) como autenticación Basic. La clave
//!   nunca aparece en un mensaje de error ni se registra.
//!
//! Solo se conecta a direcciones de la red local (privadas, de enlace local o de bucle): la URL la
//! pone el usuario, pero la app no debe servir de puente hacia internet.

use super::*;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const IO_TIMEOUT: Duration = Duration::from_secs(5);
/// Tiempo total para obtener un cuadro (una cámara lenta o muda no cuelga la interfaz).
const TOTAL_TIMEOUT: Duration = Duration::from_secs(10);
/// Tamaño máximo de un cuadro.
pub(super) const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
const MAX_HEADER_BYTES: usize = 16 * 1024;

/// Un cuadro de la cámara de red, listo para que la interfaz lo decodifique.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NetworkFrame {
    /// `image/jpeg`, `image/png` o `image/bmp`, según los bytes (no según el encabezado).
    pub mime: String,
    pub data_base64: String,
}

/// Partes de una URL `http://`.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct HttpUrl {
    pub host: String,
    pub port: u16,
    /// Ruta y consulta, siempre con `/` al inicio.
    pub path: String,
    /// `usuario:clave` ya decodificado, si la URL lo traía.
    pub credentials: Option<String>,
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let Some(hex) = s.get(i + 1..i + 3)
            && let Ok(v) = u8::from_str_radix(hex, 16)
        {
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Separa una URL `http://[usuario:clave@]host[:puerto][/ruta]`.
pub(super) fn parse_http_url(url: &str) -> Result<HttpUrl, String> {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("https://") {
        return Err(
            "Las cámaras de red se piden por http:// (https no se admite en la red local).".into(),
        );
    }
    let Some(rest) = lower
        .starts_with("http://")
        .then(|| &url["http://".len()..])
    else {
        return Err("La URL de la cámara debe empezar con http://".into());
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    let tail = tail.split('#').next().unwrap_or("");
    let path = if tail.is_empty() {
        "/".to_string()
    } else if tail.starts_with('?') {
        format!("/{tail}")
    } else {
        tail.to_string()
    };
    let (credentials, hostport) = match authority.rsplit_once('@') {
        Some((user, hp)) => (Some(percent_decode(user)), hp),
        None => (None, authority),
    };
    let (host, port) = if let Some(v6) = hostport.strip_prefix('[') {
        let (h, after) = v6
            .split_once(']')
            .ok_or("La dirección IPv6 de la URL no es válida.")?;
        let port = match after.strip_prefix(':') {
            Some(p) => p
                .parse::<u16>()
                .map_err(|_| "El puerto de la URL no es válido.")?,
            None if after.is_empty() => 80,
            None => return Err("La URL de la cámara no es válida.".into()),
        };
        (h.to_string(), port)
    } else {
        match hostport.rsplit_once(':') {
            Some((h, p)) => (
                h.to_string(),
                p.parse::<u16>()
                    .map_err(|_| "El puerto de la URL no es válido.")?,
            ),
            None => (hostport.to_string(), 80),
        }
    };
    if host.is_empty() || port == 0 {
        return Err("La URL de la cámara no tiene dirección.".into());
    }
    Ok(HttpUrl {
        host,
        port,
        path,
        credentials: credentials.filter(|c| !c.is_empty()),
    })
}

/// ¿Es una dirección de la red local (privada, de enlace local o de bucle)?
pub(super) fn is_local_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => v.is_private() || v.is_loopback() || v.is_link_local(),
        IpAddr::V6(v) => {
            if let Some(v4) = v.to_ipv4_mapped() {
                return is_local_ip(IpAddr::V4(v4));
            }
            let seg0 = v.segments()[0];
            v.is_loopback() || (seg0 & 0xfe00) == 0xfc00 || (seg0 & 0xffc0) == 0xfe80
        }
    }
}

/// Resuelve el host y comprueba que **todas** sus direcciones sean locales.
fn local_addresses(u: &HttpUrl) -> Result<Vec<SocketAddr>, String> {
    let addrs: Vec<SocketAddr> = (u.host.as_str(), u.port)
        .to_socket_addrs()
        .map_err(|_| format!("No se encontró la cámara «{}».", u.host))?
        .collect();
    if addrs.is_empty() {
        return Err(format!("No se encontró la cámara «{}».", u.host));
    }
    if addrs.iter().any(|a| !is_local_ip(a.ip())) {
        return Err(format!(
            "«{}» no está en la red local: la cámara debe ser de tu red (192.168.x.x, 10.x.x.x...).",
            u.host
        ));
    }
    Ok(addrs)
}

fn connect(addrs: &[SocketAddr], host: &str) -> Result<TcpStream, String> {
    let mut last = None;
    for a in addrs {
        match TcpStream::connect_timeout(a, CONNECT_TIMEOUT) {
            Ok(s) => return Ok(s),
            Err(e) => last = Some(e),
        }
    }
    Err(format!(
        "No se pudo conectar con la cámara «{host}»: {}.",
        last.map_or_else(|| "sin respuesta".into(), |e| e.to_string())
    ))
}

/// Codificación Base64 de las credenciales para `Authorization: Basic`.
fn basic_header(credentials: &str) -> String {
    format!("Basic {}", STANDARD.encode(credentials.as_bytes()))
}

fn build_request(u: &HttpUrl) -> String {
    let host = if u.host.contains(':') {
        format!("[{}]", u.host)
    } else {
        u.host.clone()
    };
    let hostport = if u.port == 80 {
        host
    } else {
        format!("{host}:{}", u.port)
    };
    let mut req = format!(
        "GET {} HTTP/1.1\r\nHost: {hostport}\r\nUser-Agent: Tlaku\r\n\
         Accept: image/jpeg, image/png, multipart/x-mixed-replace, */*;q=0.5\r\n\
         Connection: close\r\n",
        u.path
    );
    if let Some(c) = &u.credentials {
        req.push_str(&format!("Authorization: {}\r\n", basic_header(c)));
    }
    req.push_str("\r\n");
    req
}

/// Encabezados de la respuesta que importan.
#[derive(Debug, Default)]
struct Head {
    status: u16,
    content_type: String,
    content_length: Option<usize>,
    chunked: bool,
    www_authenticate: String,
}

fn read_head<R: BufRead>(r: &mut R) -> Result<Head, String> {
    let mut line = String::new();
    let io_err = |e: io::Error| format!("La cámara no respondió: {e}.");
    r.read_line(&mut line).map_err(io_err)?;
    let status = line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or("La respuesta de la cámara no es HTTP.")?;
    let mut head = Head {
        status,
        ..Head::default()
    };
    let mut total = line.len();
    loop {
        line.clear();
        if r.read_line(&mut line).map_err(io_err)? == 0 {
            break;
        }
        total += line.len();
        if total > MAX_HEADER_BYTES {
            return Err("Los encabezados de la cámara son demasiado grandes.".into());
        }
        let l = line.trim_end();
        if l.is_empty() {
            break;
        }
        if let Some((k, v)) = l.split_once(':') {
            let v = v.trim();
            match k.trim().to_ascii_lowercase().as_str() {
                "content-type" => head.content_type = v.to_ascii_lowercase(),
                "content-length" => head.content_length = v.parse().ok(),
                "transfer-encoding" => head.chunked = v.to_ascii_lowercase().contains("chunked"),
                "www-authenticate" => head.www_authenticate = v.to_ascii_lowercase(),
                _ => {}
            }
        }
    }
    Ok(head)
}

/// Lector del cuerpo con `Transfer-Encoding: chunked`.
struct Chunked<R: BufRead> {
    inner: R,
    left: usize,
    done: bool,
}

impl<R: BufRead> Read for Chunked<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.done || buf.is_empty() {
            return Ok(0);
        }
        if self.left == 0 {
            let mut line = String::new();
            loop {
                line.clear();
                if self.inner.read_line(&mut line)? == 0 {
                    self.done = true;
                    return Ok(0);
                }
                // La línea en blanco que cierra el fragmento anterior se salta.
                if !line.trim().is_empty() {
                    break;
                }
            }
            let size = line.split(';').next().unwrap_or("").trim();
            let size = usize::from_str_radix(size, 16)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "fragmento inválido"))?;
            if size == 0 {
                self.done = true;
                return Ok(0);
            }
            self.left = size;
        }
        let n = buf.len().min(self.left);
        let got = self.inner.read(&mut buf[..n])?;
        if got == 0 {
            self.done = true;
        }
        self.left -= got;
        Ok(got)
    }
}

const SOI: [u8; 3] = [0xFF, 0xD8, 0xFF];

/// Busca un JPEG completo en lo leído: usa el `Content-Length` de la parte si la hay y, si no, el
/// primer marcador de fin de imagen (`FF D9`).
pub(super) fn extract_jpeg(buf: &[u8]) -> Option<Vec<u8>> {
    let soi = buf.windows(3).position(|w| w == SOI)?;
    // Encabezado de la parte multipart: los últimos bytes antes del inicio de la imagen.
    let before = String::from_utf8_lossy(&buf[soi.saturating_sub(512)..soi]).to_ascii_lowercase();
    if let Some(pos) = before.rfind("content-length:") {
        let digits: String = before[pos + "content-length:".len()..]
            .trim_start()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if let Ok(n) = digits.parse::<usize>()
            && n >= 4
        {
            return (buf.len() >= soi + n).then(|| buf[soi..soi + n].to_vec());
        }
    }
    let eoi = buf[soi + 2..].windows(2).position(|w| w == [0xFF, 0xD9])?;
    Some(buf[soi..soi + 2 + eoi + 2].to_vec())
}

/// Lee hasta tener un JPEG completo (MJPEG o JPEG sin fin).
fn read_first_jpeg<R: Read>(r: &mut R, started: Instant) -> Result<Vec<u8>, String> {
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        if started.elapsed() > TOTAL_TIMEOUT {
            return Err("La cámara tardó demasiado en enviar un cuadro.".into());
        }
        let n = r
            .read(&mut chunk)
            .map_err(|e| format!("Se cortó la conexión con la cámara: {e}."))?;
        if n == 0 {
            return extract_jpeg(&buf)
                .ok_or_else(|| "La cámara cerró la conexión sin enviar un cuadro.".into());
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(jpeg) = extract_jpeg(&buf) {
            return Ok(jpeg);
        }
        if buf.len() > MAX_FRAME_BYTES + 65_536 {
            return Err("El cuadro de la cámara es demasiado grande.".into());
        }
        if buf.len() > 65_536 && !buf.windows(3).any(|w| w == SOI) {
            return Err("La cámara no está enviando imágenes JPEG.".into());
        }
    }
}

/// Tipo de imagen según sus primeros bytes.
pub(super) fn sniff_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&SOI) {
        Some("image/jpeg")
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if bytes.starts_with(b"BM") && bytes.len() > 26 {
        Some("image/bmp")
    } else {
        None
    }
}

/// Pide un cuadro a la cámara de `url` (snapshot o MJPEG) y lo devuelve en base64.
pub fn grab_network_frame(url: &str) -> Result<NetworkFrame, String> {
    let started = Instant::now();
    let u = parse_http_url(url)?;
    let addrs = local_addresses(&u)?;
    let mut stream = connect(&addrs, &u.host)?;
    let io = |e: io::Error| format!("La cámara no respondió: {e}.");
    stream.set_read_timeout(Some(IO_TIMEOUT)).map_err(io)?;
    stream.set_write_timeout(Some(IO_TIMEOUT)).map_err(io)?;
    stream.write_all(build_request(&u).as_bytes()).map_err(io)?;
    let mut reader = BufReader::new(stream);
    let head = read_head(&mut reader)?;
    match head.status {
        200 => {}
        401 | 403 => {
            let digest = head.www_authenticate.starts_with("digest");
            return Err(if digest {
                "La cámara pide autenticación Digest, que no se admite: usa Basic o una URL de snapshot sin clave.".into()
            } else if u.credentials.is_some() {
                "La cámara rechazó el usuario o la clave.".into()
            } else {
                "La cámara pide usuario y clave: ponlos en la URL (http://usuario:clave@host/...)."
                    .into()
            });
        }
        301 | 302 | 303 | 307 | 308 => {
            return Err("La cámara redirige a otra dirección; pon la URL final.".into());
        }
        code => return Err(format!("La cámara respondió con el error {code}.")),
    }
    let bytes = if head.chunked {
        read_body(
            &mut Chunked {
                inner: reader,
                left: 0,
                done: false,
            },
            &head,
            started,
        )?
    } else if let Some(n) = head.content_length {
        read_body(&mut reader.take(n as u64), &head, started)?
    } else {
        read_body(&mut reader, &head, started)?
    };
    let mime = sniff_mime(&bytes).ok_or_else(|| {
        let kind = if head.content_type.is_empty() {
            "desconocido"
        } else {
            head.content_type.as_str()
        };
        format!("La respuesta de la cámara no es una imagen (tipo {kind}).")
    })?;
    Ok(NetworkFrame {
        mime: mime.into(),
        data_base64: STANDARD.encode(&bytes),
    })
}

/// Cuerpo de la respuesta: un cuadro de un flujo (MJPEG) o la imagen completa.
fn read_body<R: Read>(r: &mut R, head: &Head, started: Instant) -> Result<Vec<u8>, String> {
    // MJPEG (multipart) o JPEG sin largo conocido (puede no terminar nunca): solo el primer cuadro.
    let stream_like = head.content_type.starts_with("multipart/")
        || (head.content_type.contains("jpeg") && head.content_length.is_none());
    if stream_like {
        return read_first_jpeg(r, started);
    }
    let mut bytes = Vec::new();
    r.take(MAX_FRAME_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Se cortó la conexión con la cámara: {e}."))?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err("El cuadro de la cámara es demasiado grande.".into());
    }
    if bytes.is_empty() {
        return Err("La cámara respondió sin imagen.".into());
    }
    Ok(bytes)
}

/// Pide un cuadro a una cámara de red. La URL la pone el usuario y debe ser de la red local.
#[tauri::command]
pub async fn fetch_camera_frame(url: String) -> Result<NetworkFrame, String> {
    tauri::async_runtime::spawn_blocking(move || grab_network_frame(&url))
        .await
        .map_err(blocking_error)?
}
