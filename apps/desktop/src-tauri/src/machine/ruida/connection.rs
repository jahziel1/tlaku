//! [`RuidaConnection`] (UDP real o virtual), configuración de la sesión y mensajes de error.

use super::*;

pub(super) use crate::machine::common::{frame_done_log, log, virtual_speed};
use tlaku_i18n::tr;

/// Conexión abierta: el mando y el hilo.
pub struct RuidaConnection {
    handle: RuidaHandle,
    thread: Option<JoinHandle<()>>,
}

impl RuidaConnection {
    pub fn handle(&self) -> RuidaHandle {
        self.handle.clone()
    }

    /// Cierra la conexión (deteniendo antes un trabajo en curso) y espera al hilo.
    pub fn close(mut self) {
        let _ = self.handle.tx.send(Request::Disconnect);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }

    /// Abre la conexión del perfil: la IP de `device.connection` por UDP o, con el puerto
    /// `virtual:ruida`, un emulador local. Sondea (solo lectura) y arranca el hilo.
    pub fn connect(
        device: &DeviceProfile,
        port: &str,
        sink: Arc<dyn Sink>,
        timing: RuidaTiming,
    ) -> Result<(Self, ConnectResult), String> {
        let cfg = session_config(device, &timing);
        if is_virtual(port) {
            let speed = virtual_speed(port)?;
            let server = UdpEmulator::spawn(EmuConfig {
                packets: PacketConfig {
                    magic: cfg.magic,
                    ..PacketConfig::default()
                },
                bed_um: [
                    (device.bed[0] * 1000.0).round() as i64,
                    (device.bed[1] * 1000.0).round() as i64,
                ],
                ..EmuConfig::default()
            })
            .map_err(|e| tr!("desktop.ruida.virtual_start_failed", err = e))?;
            let addr = server.addr();
            let link = UdpLink::to_controller_ports(addr.ip(), addr.port(), 0)
                .map_err(|e| tr!("desktop.ruida.virtual_link_failed", err = e))?;
            let sim = VirtualSim::new(server, speed);
            Self::start(link, cfg, port, sink, timing, Some(sim), device)
        } else {
            let ip = host_of(device)?;
            let link = UdpLink::to_controller(ip)
                .map_err(|e| tr!("desktop.ruida.udp_port_failed", err = e))?;
            Self::start(
                link,
                cfg,
                &format!("udp://{ip}"),
                sink,
                timing,
                None,
                device,
            )
        }
    }

    /// Sondea la máquina por `link` y arranca el hilo. `label` es lo que se muestra como puerto.
    pub fn start<L: PacketLink + 'static>(
        link: L,
        cfg: SessionConfig,
        label: &str,
        sink: Arc<dyn Sink>,
        timing: RuidaTiming,
        sim: Option<VirtualSim>,
        device: &DeviceProfile,
    ) -> Result<(Self, ConnectResult), String> {
        let auto_magic = device.ruida.magic.is_none();
        let mut session = RuidaSession::new(link, cfg);
        let probe = session.probe().map_err(connect_error)?;
        // Ya se conoce el magic: el resto de las lecturas no lo vuelven a buscar.
        {
            let c = session.config_mut();
            c.probe_all_magics = false;
            c.ack_timeout = timing.poll_timeout;
            c.reply_timeout = timing.poll_timeout;
            c.query_retries = 1;
        }
        log(sink.as_ref(), LogDir::Out, &tr!("desktop.ruida.log_probe"));
        log(sink.as_ref(), LogDir::In, &describe_probe(&probe));
        if probe.magic_changed || auto_magic {
            log(
                sink.as_ref(),
                LogDir::In,
                &tr!(
                    "desktop.ruida.log_magic_detected",
                    magic = format!("{:02X}", probe.magic.0)
                ),
            );
        }
        if let Some([w, h]) = probe.bed_mm
            && ((w - device.bed[0]).abs() > 1.0 || (h - device.bed[1]).abs() > 1.0)
        {
            log(
                sink.as_ref(),
                LogDir::In,
                &tr!(
                    "desktop.ruida.log_bed_mismatch",
                    w = format!("{w:.0}"),
                    h = format!("{h:.0}"),
                    pw = format!("{:.0}", device.bed[0]),
                    ph = format!("{:.0}", device.bed[1])
                ),
            );
        }
        if device.rotary.enabled {
            log(sink.as_ref(), LogDir::In, &format!("; {}", rotary_note()));
        }
        log(sink.as_ref(), LogDir::In, &format!("; {}", experimental()));
        if sim.is_some() {
            log(sink.as_ref(), LogDir::In, &tr!("desktop.ruida.log_virtual"));
        }
        let firmware = tr!(
            "desktop.ruida.firmware",
            model = probe
                .model
                .map_or_else(|| tr!("desktop.ruida.unknown_model"), str::to_string),
            card = format!("{:08X}", probe.card_id)
        );

        let shared = Arc::new(Shared::default());
        shared
            .virtual_machine
            .store(sim.is_some(), Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        let worker = Worker {
            session,
            rx,
            sink: sink.clone(),
            shared: shared.clone(),
            timing,
            port: label.to_string(),
            bed_mm: probe.bed_mm,
            job: None,
            frame: None,
            failures: 0,
            silent_logged: false,
            sim,
        };
        sink.emit(Emit::Connection(ConnectionEvent {
            connected: true,
            port: Some(label.to_string()),
            error: None,
        }));
        let thread = thread::Builder::new()
            .name("tlaku-ruida".into())
            .spawn(move || worker.run())
            .map_err(|e| tr!("desktop.machine.thread_failed", err = e))?;
        Ok((
            Self {
                handle: RuidaHandle { tx, shared },
                thread: Some(thread),
            },
            ConnectResult { firmware },
        ))
    }
}

impl Drop for RuidaConnection {
    fn drop(&mut self) {
        let _ = self.handle.tx.send(Request::Disconnect);
    }
}

/// Configuración de la sesión a partir del perfil: magic (o búsqueda automática), cama y home.
pub fn session_config(device: &DeviceProfile, timing: &RuidaTiming) -> SessionConfig {
    SessionConfig {
        magic: Magic(device.ruida.magic.unwrap_or(MAGIC_DEFAULT)),
        // Con el magic fijado en el perfil no se prueban otros: el usuario sabe cuál es.
        probe_all_magics: device.ruida.magic.is_none(),
        ack_timeout: timing.upload_ack,
        reply_timeout: timing.upload_ack,
        limits: SafetyLimits {
            bed_mm: device.bed,
            ..SafetyLimits::default()
        },
        home: tlaku_pipeline::home_for_origin(device.origin),
        ..SessionConfig::default()
    }
}

/// IP de la controladora del perfil (`connection = udp`). Acepta una IP o un nombre.
pub(super) fn host_of(device: &DeviceProfile) -> Result<IpAddr, String> {
    let host = match &device.connection {
        tlaku_doc::Connection::Udp { host } if !host.trim().is_empty() => host.trim(),
        _ => {
            return Err(tr!("desktop.ruida.needs_ip"));
        }
    };
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(ip);
    }
    (host, 0)
        .to_socket_addrs()
        .ok()
        .and_then(|mut it| it.find(|a| a.is_ipv4()))
        .map(|a| a.ip())
        .ok_or_else(|| tr!("desktop.ruida.bad_host", host = host))
}

pub(super) fn describe_probe(info: &ProbeInfo) -> String {
    let model = info
        .model
        .map_or_else(|| tr!("desktop.ruida.unknown_model"), str::to_string);
    let card = format!("{:08X}", info.card_id);
    let magic = format!("{:02X}", info.magic.0);
    let bed = match info.bed_mm {
        Some([w, h]) => format!("{w:.1} × {h:.1} mm"),
        None => tr!("desktop.ruida.not_reported"),
    };
    if info.magic_changed {
        tr!(
            "desktop.ruida.probe_line_detected",
            model = model,
            card = card,
            magic = magic,
            bed = bed
        )
    } else {
        tr!(
            "desktop.ruida.probe_line",
            model = model,
            card = card,
            magic = magic,
            bed = bed
        )
    }
}

/// Error del sondeo al conectar, con lo que conviene revisar.
pub(super) fn connect_error(e: SessionError) -> String {
    match &e {
        SessionError::Timeout { .. } => tr!("desktop.ruida.no_reply", err = e),
        _ => tr!("desktop.ruida.talk_failed", err = e),
    }
}

/// Error de una operación en marcha, con la pista de la red si es un silencio.
pub(super) fn op_error(e: SessionError) -> String {
    match &e {
        SessionError::Timeout { .. } => tr!("desktop.ruida.op_silent", err = e),
        SessionError::Busy(_) => tr!("desktop.ruida.op_busy", err = e),
        _ => e.to_string(),
    }
}
