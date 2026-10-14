//! Enlace por paquetes con la controladora: un datagrama = un mensaje.
//!
//! GRBL es un flujo de bytes (`Transport` en `tlaku-grbl`); Ruida es un flujo de datagramas con
//! un ACK por paquete, así que el límite entre mensajes importa y el trait es distinto.
//!
//! - [`UdpLink`]: UDP real. La máquina escucha en el 50200 y **responde al 40200**, por lo que
//!   hay que enlazar el socket local a ese puerto (EduTech, ruida-pa y MeerK40t lo hacen igual).
//! - [`MemoryLink`]: en memoria, para pruebas, con un "respondedor" opcional (el emulador).

use std::collections::VecDeque;
use std::io;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tlaku_i18n::tr;

/// Puerto de la controladora al que se envían los datos.
pub const REMOTE_PORT: u16 = 50200;
/// Puerto local en el que la controladora contesta.
pub const LOCAL_PORT: u16 = 40200;
/// Tamaño del búfer de recepción (el datagrama más grande es de 1472 bytes).
const RECV_BUF: usize = 2048;

/// Canal de datagramas bidireccional con espera máxima en la lectura.
pub trait PacketLink: Send {
    /// Envía un datagrama completo.
    fn send(&mut self, datagram: &[u8]) -> io::Result<()>;

    /// Espera un datagrama como máximo `timeout`. `Ok(None)` = no llegó nada a tiempo; un enlace
    /// roto es un `Err`.
    fn recv(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>>;

    /// Descarta lo recibido y aún no leído (datagramas viejos que confundirían al protocolo).
    fn drain(&mut self) -> io::Result<()> {
        while self.recv(Duration::from_millis(1))?.is_some() {}
        Ok(())
    }
}

impl<T: PacketLink + ?Sized> PacketLink for Box<T> {
    fn send(&mut self, datagram: &[u8]) -> io::Result<()> {
        (**self).send(datagram)
    }

    fn recv(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        (**self).recv(timeout)
    }

    fn drain(&mut self) -> io::Result<()> {
        (**self).drain()
    }
}

fn diag_t() -> std::time::Duration {
    static T0: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    T0.get_or_init(std::time::Instant::now).elapsed()
}

/// UDP real contra una controladora.
pub struct UdpLink {
    socket: UdpSocket,
    remote: SocketAddr,
}

impl UdpLink {
    /// Enlaza `local` y fija `remote` como único interlocutor (el socket queda "conectado": los
    /// datagramas de otros orígenes se descartan).
    pub fn bind(local: SocketAddr, remote: SocketAddr) -> io::Result<Self> {
        let socket = UdpSocket::bind(local).map_err(|e| {
            io::Error::new(
                e.kind(),
                tr!("ruida.link.udp_bind", port = local.port(), e = e),
            )
        })?;
        socket.connect(remote)?;
        Ok(UdpLink { socket, remote })
    }

    /// Enlace estándar: escucha en el 40200 (todas las interfaces) y envía al `ip`:50200.
    pub fn to_controller(ip: IpAddr) -> io::Result<Self> {
        Self::to_controller_ports(ip, REMOTE_PORT, LOCAL_PORT)
    }

    /// Como [`UdpLink::to_controller`] con puertos propios (`local_port` 0 = uno efímero).
    pub fn to_controller_ports(ip: IpAddr, remote_port: u16, local_port: u16) -> io::Result<Self> {
        let any: IpAddr = if ip.is_ipv4() {
            [0, 0, 0, 0].into()
        } else {
            std::net::Ipv6Addr::UNSPECIFIED.into()
        };
        Self::bind(
            SocketAddr::new(any, local_port),
            SocketAddr::new(ip, remote_port),
        )
    }

    pub fn remote(&self) -> SocketAddr {
        self.remote
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }
}

impl PacketLink for UdpLink {
    fn send(&mut self, datagram: &[u8]) -> io::Result<()> {
        eprintln!(
            "DIAG {:?} send {} bytes {:02X?}",
            diag_t(),
            datagram.len(),
            &datagram[..datagram.len().min(8)]
        );
        match self.socket.send(datagram) {
            Ok(_) => Ok(()),
            // Un "puerto inalcanzable" de un envío anterior (ICMP) no es un fallo de este.
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::ConnectionRefused | io::ErrorKind::ConnectionReset
                ) =>
            {
                self.socket.send(datagram).map(|_| ())
            }
            Err(e) => Err(e),
        }
    }

    fn recv(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        let deadline = std::time::Instant::now() + timeout;
        let mut buf = [0u8; RECV_BUF];
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            // `set_read_timeout(Some(0))` es un error: mínimo 1 ms.
            self.socket
                .set_read_timeout(Some(left.max(Duration::from_millis(1))))?;
            let res = self.socket.recv(&mut buf);
            match &res {
                Ok(n) => eprintln!(
                    "DIAG {:?} recv ok {} {:02X?}",
                    diag_t(),
                    n,
                    &buf[..(*n).min(8)]
                ),
                Err(e) => eprintln!(
                    "DIAG {:?} recv err {:?} {e} left={left:?}",
                    diag_t(),
                    e.kind()
                ),
            }
            match res {
                Ok(n) => return Ok(Some(buf[..n].to_vec())),
                Err(e) => match e.kind() {
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => return Ok(None),
                    // ICMP "puerto inalcanzable" de un envío ANTERIOR (Windows: reset; Linux/macOS:
                    // refused). No es la respuesta a lo que esperamos: se sigue esperando lo que
                    // queda del plazo en vez de darlo por perdido al instante.
                    io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionRefused => {
                        if left.is_zero() {
                            return Ok(None);
                        }
                        // Con la máquina apagada el aviso se repite al instante: una pausa corta
                        // evita girar en vacío mientras se agota el plazo.
                        std::thread::sleep(left.min(Duration::from_millis(5)));
                    }
                    _ => return Err(e),
                },
            }
        }
    }
}

/// Quien responde a lo que se envía por un [`MemoryLink`]: devuelve los datagramas de respuesta.
pub type Responder = Box<dyn FnMut(&[u8]) -> Vec<Vec<u8>> + Send>;

#[derive(Default)]
struct MemState {
    inbox: VecDeque<Vec<u8>>,
    sent: Vec<Vec<u8>>,
    responder: Option<Responder>,
    fail_send: bool,
}

/// Enlace en memoria. Es `Clone` (comparte el estado): una copia va a la sesión y otra se queda
/// en la prueba para inspeccionar lo enviado o programar lo que "llega".
#[derive(Clone, Default)]
pub struct MemoryLink {
    state: Arc<Mutex<MemState>>,
}

impl MemoryLink {
    pub fn new() -> Self {
        Self::default()
    }

    fn st(&self) -> MutexGuard<'_, MemState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Enlace cuyas respuestas calcula `f` en cada envío.
    pub fn with_responder(f: impl FnMut(&[u8]) -> Vec<Vec<u8>> + Send + 'static) -> Self {
        let l = Self::new();
        l.st().responder = Some(Box::new(f));
        l
    }

    /// Deja un datagrama pendiente de leer.
    pub fn push_incoming(&self, datagram: Vec<u8>) {
        self.st().inbox.push_back(datagram);
    }

    /// Todo lo enviado hasta ahora.
    pub fn sent(&self) -> Vec<Vec<u8>> {
        self.st().sent.clone()
    }

    /// Datagramas recibidos que nadie ha leído todavía.
    pub fn pending(&self) -> usize {
        self.st().inbox.len()
    }

    /// Hace que los envíos fallen (cable desconectado).
    pub fn set_send_error(&self, fail: bool) {
        self.st().fail_send = fail;
    }
}

impl PacketLink for MemoryLink {
    fn send(&mut self, datagram: &[u8]) -> io::Result<()> {
        let mut st = self.st();
        if st.fail_send {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "el enlace en memoria está cortado",
            ));
        }
        st.sent.push(datagram.to_vec());
        if let Some(mut f) = st.responder.take() {
            let replies = f(datagram);
            st.responder = Some(f);
            st.inbox.extend(replies);
        }
        Ok(())
    }

    /// No espera: si no hay nada pendiente es un timeout inmediato.
    fn recv(&mut self, _timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        Ok(self.st().inbox.pop_front())
    }

    fn drain(&mut self) -> io::Result<()> {
        self.st().inbox.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_link_records_and_replies() {
        let link = MemoryLink::with_responder(|d| vec![vec![d.len() as u8], vec![0xAA]]);
        let mut l = link.clone();
        assert_eq!(l.recv(Duration::from_millis(1)).unwrap(), None);
        l.send(&[1, 2, 3]).unwrap();
        assert_eq!(link.sent(), [vec![1, 2, 3]]);
        assert_eq!(link.pending(), 2);
        assert_eq!(l.recv(Duration::ZERO).unwrap(), Some(vec![3]));
        l.drain().unwrap();
        assert_eq!(l.recv(Duration::ZERO).unwrap(), None);
        link.push_incoming(vec![9]);
        assert_eq!(l.recv(Duration::ZERO).unwrap(), Some(vec![9]));
        link.set_send_error(true);
        assert_eq!(l.send(&[0]).unwrap_err().kind(), io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn udp_link_roundtrip_on_localhost_with_ephemeral_ports() {
        let peer = UdpSocket::bind("127.0.0.1:0").unwrap();
        let mut link = UdpLink::to_controller_ports(
            "127.0.0.1".parse().unwrap(),
            peer.local_addr().unwrap().port(),
            0,
        )
        .unwrap();
        link.send(&[7, 8, 9]).unwrap();
        let mut buf = [0u8; 16];
        peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let (n, from) = peer.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..n], [7, 8, 9]);
        // Sin respuesta: timeout.
        assert_eq!(link.recv(Duration::from_millis(20)).unwrap(), None);
        peer.send_to(&[1, 2], from).unwrap();
        assert_eq!(link.recv(Duration::from_secs(2)).unwrap(), Some(vec![1, 2]));
        // Un tercero que escribe al socket local no se acepta (socket conectado).
        let other = UdpSocket::bind("127.0.0.1:0").unwrap();
        other.send_to(&[5], from).unwrap();
        assert_eq!(link.recv(Duration::from_millis(50)).unwrap(), None);
    }

    #[test]
    fn udp_link_reports_port_in_use_with_a_clear_message() {
        let first = UdpLink::to_controller_ports("127.0.0.1".parse().unwrap(), 50200, 0).unwrap();
        let port = first.local_addr().unwrap().port();
        let err = UdpLink::to_controller_ports("127.0.0.1".parse().unwrap(), 50200, port);
        // Algunos sistemas permiten compartir el puerto: entonces no hay error que comprobar.
        if let Err(e) = err {
            assert!(e.to_string().contains("puerto UDP local"), "{e}");
        }
    }
}
