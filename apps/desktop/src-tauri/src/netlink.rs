//! Conexión por red de la familia GRBL (Telnet/TCP y WebSocket) y la máquina virtual por red.
//!
//! La interfaz identifica el destino con el mismo texto que antes era el puerto serie:
//!
//! - `tcp://host[:puerto]` (puerto 23 por omisión) y `ws://host[:puerto][/ruta]`: máquinas reales
//!   (grblHAL Ethernet, FluidNC, ESP3D). Ver [`tlaku_grbl::net`].
//! - `virtualnet:<modelo>[@<velocidad>]`: el emulador de `tlaku-emu` detrás de un socket TCP
//!   local, para probar la conexión por red de verdad sin hardware. Se conecta como a cualquier
//!   máquina de red; solo que el otro extremo es un hilo de este mismo programa.

use crate::machine::virtual_port;
use std::io;
use std::time::Duration;
use tlaku_emu::net::{NetServer, serve_local};
use tlaku_grbl::net::{CONNECT_TIMEOUT, Endpoint, TcpTransport, drain_greeting};
use tlaku_grbl::serial::Transport;

/// Prefijo de las máquinas virtuales por red.
pub const VIRTUAL_NET_PREFIX: &str = "virtualnet:";

/// Transporte listo y, si es una máquina virtual, el mando del emulador (para simular fallas).
pub type Opened = (Box<dyn Transport>, Option<tlaku_emu::Handle>);

/// Conexión TCP a una máquina virtual que es dueña de su servidor: al soltar la conexión (cerrar,
/// desconectar o perderla) se cierra también el socket local y se detiene el hilo del emulador.
struct VirtualNetTransport {
    // El orden importa: primero se cierra el cliente y después se detiene el servidor.
    client: TcpTransport,
    _server: NetServer,
}

impl Transport for VirtualNetTransport {
    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        self.client.write_all(data)
    }

    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        self.client.read_timeout(buf, timeout)
    }

    fn clear_input(&mut self) -> io::Result<()> {
        self.client.clear_input()
    }
}

/// Identificadores de las máquinas virtuales por red, para la lista de puertos (los mismos
/// modelos que las virtuales locales).
pub fn virtual_ports() -> Vec<String> {
    virtual_port::ports()
        .into_iter()
        .map(|p| format!("{VIRTUAL_NET_PREFIX}{}", &p[virtual_port::PREFIX.len()..]))
        .collect()
}

/// ¿`port` es una dirección de red real (`tcp://` o `ws://`)?
pub fn is_network(port: &str) -> bool {
    Endpoint::parse(port).is_some()
}

/// ¿`port` es una máquina virtual por red?
pub fn is_virtual_net(port: &str) -> bool {
    port.starts_with(VIRTUAL_NET_PREFIX)
}

/// Dirección para mostrar al usuario (sin el esquema `tcp://`). Para lo que no es de red,
/// el texto tal cual.
pub fn display_address(port: &str) -> String {
    match Endpoint::parse(port) {
        Some(Ok(e)) => e.label(),
        _ => port.to_string(),
    }
}

/// Esperas del saludo al conectar: algunas controladoras lo mandan al abrir el socket (no hay
/// reinicio de la placa como con el puerto serie, así que es corto).
const GREETING_QUIET: Duration = Duration::from_millis(250);
const GREETING_MAX: Duration = Duration::from_millis(1500);

/// Abre una dirección de red real. Los errores salen ya explicados en español.
pub fn open_endpoint(port: &str) -> Result<Box<dyn Transport>, String> {
    let endpoint = match Endpoint::parse(port) {
        Some(Ok(e)) => e,
        Some(Err(msg)) => return Err(msg),
        None => return Err(format!("'{port}' no es una dirección de red.")),
    };
    endpoint.open(CONNECT_TIMEOUT).map_err(|e| e.to_string())
}

/// Abre el destino de red de `port` para conectar: red real o máquina virtual por red. `None`
/// si `port` es otra cosa (un puerto serie o una máquina virtual local).
pub fn open(port: &str) -> Option<Result<Opened, String>> {
    if is_virtual_net(port) {
        return Some(open_virtual(port));
    }
    if !is_network(port) {
        return None;
    }
    Some(open_endpoint(port).and_then(|mut t| {
        // Sin reinicio al abrir: solo se descarta el saludo, si lo hay.
        drain_greeting(&mut *t, GREETING_QUIET, GREETING_MAX)
            .map_err(|e| format!("Se perdió la conexión con {}: {e}", display_address(port)))?;
        Ok((t, None))
    }))
}

/// Levanta el emulador de `virtualnet:<modelo>[@v]` en un puerto local libre y se conecta a él
/// por TCP, como a una máquina de red.
fn open_virtual(port: &str) -> Result<Opened, String> {
    let rest = &port[VIRTUAL_NET_PREFIX.len()..];
    let (vm, handle) = virtual_port::build(&format!("{}{rest}", virtual_port::PREFIX))?;
    let server = serve_local(vm)
        .map_err(|e| format!("No se pudo abrir el socket local de la máquina virtual: {e}"))?;
    let client = TcpTransport::connect("127.0.0.1", server.port(), CONNECT_TIMEOUT)
        .map_err(|e| e.to_string())?;
    Ok((
        Box::new(VirtualNetTransport {
            client,
            _server: server,
        }),
        Some(handle),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::time::Instant;
    use tlaku_grbl::serial::Grbl;

    #[test]
    fn classifies_ports() {
        assert!(is_network("tcp://192.168.1.5:23"));
        assert!(is_network("ws://fluidnc.local:81/"));
        assert!(!is_network("/dev/cu.usbserial-1"));
        assert!(!is_network("virtual:falcon2"));
        assert!(!is_network("virtualnet:falcon2"));
        assert!(is_virtual_net("virtualnet:falcon2@5"));
        assert!(virtual_ports().contains(&"virtualnet:falcon2".to_string()));
        assert!(open("COM3").is_none());
        assert!(open("virtual:falcon2").is_none());
        assert_eq!(display_address("tcp://10.0.0.2:23"), "10.0.0.2:23");
        assert_eq!(display_address("COM3"), "COM3");
    }

    #[test]
    fn real_network_errors_are_in_spanish() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        drop(l);
        let err = open(&format!("tcp://127.0.0.1:{port}"))
            .unwrap()
            .err()
            .unwrap();
        assert!(err.contains("rechazó la conexión"), "{err}");
        let err = open("tcp://").unwrap().err().unwrap();
        assert!(err.contains("Falta la dirección"), "{err}");
    }

    #[test]
    fn virtual_machine_over_a_real_socket_answers_and_stops_with_the_connection() {
        let (t, emu) = open("virtualnet:falcon2@20").unwrap().unwrap();
        let emu = emu.expect("el mando del emulador");
        let mut grbl = Grbl::with_transport(t);
        let info = grbl.command("$I", Duration::from_secs(3)).unwrap();
        assert!(info.iter().any(|l| l.starts_with("[VER:")), "{info:?}");
        assert_eq!(emu.received_lines(), ["$I"]);
        // Soltar la conexión cierra el socket y detiene el servidor sin colgarse.
        let start = Instant::now();
        drop(grbl);
        assert!(start.elapsed() < Duration::from_secs(5));
        assert!(open("virtualnet:no-existe").unwrap().is_err());
    }
}
