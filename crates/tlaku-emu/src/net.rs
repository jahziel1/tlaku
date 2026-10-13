//! La controladora emulada detrás de un socket TCP: sirve para probar la conexión por red
//! (Telnet de grblHAL y FluidNC) sin una máquina, tanto en las pruebas como en la app
//! ("máquina virtual por red").
//!
//! [`serve_tcp`] atiende **un cliente a la vez**; al irse uno, la máquina sigue viva (con su
//! estado) esperando al siguiente, como una controladora real. Al soltar el [`NetServer`] se
//! detiene el hilo y se cierra el socket.

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tlaku_grbl::serial::Transport;

use crate::vm::VirtualMachine;

/// Servidor TCP de una máquina emulada. Soltarlo lo detiene.
pub struct NetServer {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl NetServer {
    /// Dirección en la que escucha.
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Puerto en el que escucha.
    pub fn port(&self) -> u16 {
        self.addr.port()
    }
}

impl Drop for NetServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Escucha en `127.0.0.1` en un puerto libre y sirve `vm`.
pub fn serve_local(vm: VirtualMachine) -> io::Result<NetServer> {
    serve_tcp(TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?, vm)
}

/// Sirve `vm` en `listener`, un cliente a la vez.
pub fn serve_tcp(listener: TcpListener, vm: VirtualMachine) -> io::Result<NetServer> {
    let addr = listener.local_addr()?;
    listener.set_nonblocking(true)?;
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let thread = thread::Builder::new()
        .name("tlaku-emu-tcp".into())
        .spawn(move || run(listener, vm, &flag))?;
    Ok(NetServer {
        addr,
        stop,
        thread: Some(thread),
    })
}

fn run(listener: TcpListener, mut vm: VirtualMachine, stop: &AtomicBool) {
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                // Lo que la máquina dijo sin nadie conectado (su saludo de arranque) se pierde.
                let _ = vm.clear_input();
                let _ = session(stream, &mut vm, stop);
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// Pasa bytes entre el socket y la máquina hasta que el cliente se va o la máquina se
/// desconecta.
fn session(mut stream: TcpStream, vm: &mut VirtualMachine, stop: &AtomicBool) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(Duration::from_millis(2)))?;
    let mut inbound = [0u8; 1024];
    let mut outbound = [0u8; 1024];
    while !stop.load(Ordering::SeqCst) {
        match stream.read(&mut inbound) {
            Ok(0) => return Ok(()),
            Ok(n) => vm.write_all(&inbound[..n])?,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(e) => return Err(e),
        }
        let n = vm.read_timeout(&mut outbound, Duration::ZERO)?;
        if n > 0 {
            stream.write_all(&outbound[..n])?;
        }
    }
    Ok(())
}
