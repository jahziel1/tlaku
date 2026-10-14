//! Por red (Telnet/TCP): la conexión real contra el emulador detrás de un socket.

use super::*;

#[test]
fn network_connection_runs_a_job_through_a_real_tcp_socket() {
    let port = "virtualnet:falcon2@50";
    let (transport, emu) = crate::netlink::open(port).unwrap().unwrap();
    let sink = Arc::new(Collect::default());
    let (conn, info) =
        Connection::start_with(transport, port, sink.clone(), Timing::default(), emu).unwrap();
    assert!(info.firmware.contains("Grbl"), "{}", info.firmware);
    let handle = conn.handle();
    assert!(handle.is_virtual());
    sink.wait("estado", |s| !s.statuses().is_empty());
    handle.start_job(job_lines(), 127).unwrap();
    sink.wait("trabajo terminado", |s| {
        s.jobs().contains(&JobState::Finished)
    });
    assert_eq!(sink.jobs(), [JobState::Started, JobState::Finished]);
    let last = sink.progress().last().copied().unwrap();
    assert_eq!((last.acked, last.total), (6, 6));
    // La dirección que ve la interfaz es la de la conexión.
    assert_eq!(
        sink.connections().first().and_then(|c| c.port.clone()),
        Some(port.to_string())
    );
    // Cerrar la conexión cierra el socket sin colgarse y avisa a la interfaz.
    conn.close();
}

#[test]
fn network_connection_loss_is_reported_as_a_lost_connection() {
    let port = "virtualnet:grbl11@50";
    let (transport, emu) = crate::netlink::open(port).unwrap().unwrap();
    let sink = Arc::new(Collect::default());
    let (conn, _) =
        Connection::start_with(transport, port, sink.clone(), Timing::default(), emu).unwrap();
    sink.wait("estado", |s| !s.statuses().is_empty());
    // La máquina "se apaga": el servidor cierra el socket.
    conn.handle()
        .simulate_fault(tlaku_doc::SimulatedFault::Disconnect)
        .unwrap();
    sink.wait("conexión perdida", |s| {
        s.connections().iter().any(|c| !c.connected)
    });
    let lost = sink
        .connections()
        .into_iter()
        .find(|c| !c.connected)
        .unwrap();
    assert!(
        lost.error.as_deref().unwrap_or("").contains("conexión"),
        "{lost:?}"
    );
    conn.close();
}
