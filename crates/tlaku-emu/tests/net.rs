//! El `Streamer` y `Grbl` reales de `tlaku-grbl` hablando por red (TCP y WebSocket) con la
//! controladora emulada detrás del socket.

mod common;

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use common::{fast_config, scaled_vm, square_job};
use tlaku_emu::net::serve_local;
use tlaku_emu::{Fault, Profile, StateKind, Trigger};
use tlaku_grbl::net::{TcpTransport, WebSocketTransport, drain_greeting};
use tlaku_grbl::serial::{Grbl, Transport};
use tlaku_grbl::streamer::{Event, Outcome, Streamer};
use tungstenite::Message;

const DEADLINE: Duration = Duration::from_secs(30);

/// Corre el trabajo con el streamer real sobre `transport` hasta que termine.
fn run_streamer<T: Transport + 'static>(transport: T, lines: &[String]) -> Outcome {
    let streamer = Streamer::new(transport, fast_config());
    let job = streamer.spawn(lines).expect("líneas válidas");
    let start = Instant::now();
    loop {
        match job.events.recv_timeout(Duration::from_millis(20)) {
            Ok(Event::Finished(outcome)) => {
                let _ = job.join();
                return outcome;
            }
            Ok(_) => {}
            Err(_) => assert!(start.elapsed() < DEADLINE, "el trabajo no terminó"),
        }
    }
}

#[test]
fn grbl_identifies_and_streamer_runs_a_job_over_tcp() {
    let (vm, h) = scaled_vm(Profile::grbl_1_1h(), 50.0, |b| b.homed(true));
    let server = serve_local(vm).unwrap();
    let mut t = TcpTransport::connect("127.0.0.1", server.port(), Duration::from_secs(2)).unwrap();
    // Sin reinicio al abrir: se espera un instante el saludo (el servidor no manda ninguno).
    assert!(
        drain_greeting(
            &mut t,
            Duration::from_millis(50),
            Duration::from_millis(300)
        )
        .unwrap()
        .is_empty()
    );
    let mut grbl = Grbl::with_transport(Box::new(t));
    let info = grbl.command("$I", Duration::from_secs(3)).unwrap();
    assert!(info.iter().any(|l| l.starts_with("[VER:")), "{info:?}");

    let lines = square_job(10.0, 10.0, 20.0, 3000, 500);
    let outcome = run_streamer(grbl.into_transport(), &lines);
    assert_eq!(outcome, Outcome::Completed { errors: 0 });
    assert_eq!(h.state(), StateKind::Idle);
    assert_eq!(h.rx_overflow_bytes(), 0);
    assert!(h.max_in_flight() <= 127, "{}", h.max_in_flight());
    let t = h.trace();
    assert!(
        (t.total_length() - 80.0).abs() < 0.1,
        "{}",
        t.total_length()
    );
}

#[test]
fn the_machine_survives_a_client_and_accepts_the_next_one() {
    let (vm, h) = scaled_vm(Profile::grbl_1_1h(), 50.0, |b| b);
    let server = serve_local(vm).unwrap();
    for _ in 0..2 {
        let t = TcpTransport::connect("127.0.0.1", server.port(), Duration::from_secs(2)).unwrap();
        let mut grbl = Grbl::with_transport(Box::new(t));
        grbl.command("G90", Duration::from_secs(3)).unwrap();
    }
    assert_eq!(h.received_lines(), ["G90", "G90"]);
}

#[test]
fn a_dropped_machine_is_reported_as_closed_connection() {
    let (vm, h) = scaled_vm(Profile::grbl_1_1h(), 1.0, |b| b);
    let server = serve_local(vm).unwrap();
    let t = TcpTransport::connect("127.0.0.1", server.port(), Duration::from_secs(2)).unwrap();
    let mut grbl = Grbl::with_transport(Box::new(t));
    grbl.command("G90", Duration::from_secs(3)).unwrap();
    h.inject(Trigger::Now, Fault::Disconnect);
    // Para el anfitrión es un cierre de la conexión de red: error, no silencio.
    let err = grbl.command("G90", Duration::from_secs(3)).unwrap_err();
    assert!(
        matches!(
            err.kind(),
            io::ErrorKind::ConnectionAborted
                | io::ErrorKind::BrokenPipe
                | io::ErrorKind::ConnectionReset
        ),
        "{err}"
    );
}

/// Puente WebSocket <-> emulador de una conexión: cada mensaje (texto o binario) se escribe en
/// la máquina y lo que ella dice sale como mensajes de texto, trozeado en paquetes pequeños para
/// que el `ok` llegue partido.
fn serve_ws(vm: tlaku_emu::VirtualMachine) -> (u16, Arc<AtomicBool>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let handle = thread::spawn(move || {
        let mut vm = vm;
        let (stream, _) = listener.accept().unwrap();
        let mut ws = tungstenite::accept(stream).unwrap();
        ws.get_ref()
            .set_read_timeout(Some(Duration::from_millis(2)))
            .unwrap();
        let _ = ws.send(Message::text("currentID:7"));
        let mut out = [0u8; 64];
        while !flag.load(Ordering::SeqCst) {
            match ws.read() {
                Ok(Message::Text(t)) => vm.write_all(t.as_bytes()).unwrap(),
                Ok(Message::Binary(b)) => vm.write_all(&b).unwrap(),
                Ok(Message::Close(_)) | Err(tungstenite::Error::ConnectionClosed) => return,
                Ok(_) => {}
                Err(tungstenite::Error::Io(e))
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) => {}
                Err(_) => return,
            }
            // Trozos de 5 bytes: parte los `ok\r\n` y las líneas largas entre mensajes.
            match vm.read_timeout(&mut out[..5], Duration::ZERO) {
                Ok(0) => {}
                Ok(n) => {
                    let text = String::from_utf8_lossy(&out[..n]).into_owned();
                    if ws.send(Message::text(text)).is_err() {
                        return;
                    }
                }
                Err(_) => return,
            }
        }
    });
    (port, stop, handle)
}

#[test]
fn grbl_and_streamer_work_over_websocket_with_split_replies() {
    let (mut vm, h) = scaled_vm(Profile::fluidnc(), 50.0, |b| b.homed(true));
    vm.clear_input().unwrap();
    let (port, stop, server) = serve_ws(vm);
    let url = format!("ws://127.0.0.1:{port}/");
    let t = WebSocketTransport::connect(&url, Duration::from_secs(2)).unwrap();
    let mut grbl = Grbl::with_transport(Box::new(t));
    let info = grbl.command("$I", Duration::from_secs(3)).unwrap();
    assert!(info.iter().any(|l| l.contains("FluidNC")), "{info:?}");
    // Los mensajes de sesión de ESP3D no se cuelan en la respuesta.
    assert!(info.iter().all(|l| !l.starts_with("currentID")), "{info:?}");

    let lines = square_job(5.0, 5.0, 10.0, 3000, 400);
    let outcome = run_streamer(grbl.into_transport(), &lines);
    assert_eq!(outcome, Outcome::Completed { errors: 0 });
    assert_eq!(h.state(), StateKind::Idle);
    assert_eq!(h.rx_overflow_bytes(), 0);
    stop.store(true, Ordering::SeqCst);
    server.join().unwrap();
}

#[test]
fn tcp_transport_closes_cleanly_so_the_server_sees_eof() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut buf = [0u8; 8];
        // `Ok(0)` es el fin ordenado; un reinicio de conexión sería un `Err`.
        s.read(&mut buf).unwrap()
    });
    let t = TcpTransport::connect("127.0.0.1", port, Duration::from_secs(2)).unwrap();
    drop(t);
    assert_eq!(server.join().unwrap(), 0);
    let _ = TcpStream::connect(("127.0.0.1", port)).map(|mut s| s.flush());
}
