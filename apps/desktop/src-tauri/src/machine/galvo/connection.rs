//! [`GalvoConnection`] (conexión USB o virtual), transporte con ritmo y mensajes de error.

use super::*;

pub(super) use crate::machine::common::{Paced, log, virtual_speed};
use tlaku_i18n::tr;

/// Conexión abierta: el mando y el hilo.
pub struct GalvoConnection {
    handle: GalvoHandle,
    thread: Option<JoinHandle<()>>,
}

impl GalvoConnection {
    pub fn handle(&self) -> GalvoHandle {
        self.handle.clone()
    }

    fn request_close(&self) {
        // Un trabajo en curso se detiene (y la fuente se apaga) antes de cerrar.
        let sh = &self.handle.shared;
        sh.cancel.store(true, Ordering::SeqCst);
        sh.control.request_stop();
        sh.gate_cv.notify_all();
        let _ = self.handle.tx.send(Request::Disconnect);
    }

    /// Cierra la conexión (deteniendo antes un trabajo en curso) y espera al hilo.
    pub fn close(mut self) {
        self.request_close();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }

    /// Abre la tarjeta del puerto (`usb:galvo:<n>`, con el reconocimiento de seguridad) o la
    /// máquina virtual (`virtual:galvo`), la sondea (solo lectura) y arranca el hilo.
    ///
    /// Con una tarjeta real, sin `ack` completo falla ANTES de tocar el USB.
    pub fn connect(
        device: &DeviceProfile,
        port: &str,
        ack: Option<&SafetyAck>,
        sink: Arc<dyn Sink>,
        timing: GalvoTimingCfg,
    ) -> Result<(Self, ConnectResult), String> {
        let mut cfg = session_config(device)?;
        if is_virtual(port) {
            let (transport, _state) = emulator(EmuConfig {
                source: device.galvo.source,
                field_mm: device.galvo.field_mm,
                ..EmuConfig::default()
            });
            // El tiempo ya lo marca el paso de cada lectura: no hace falta esperar más.
            cfg.poll_interval = Duration::ZERO;
            let step = timing.virtual_pace.div_f64(virtual_speed(port)?);
            Self::start(
                Box::new(Paced {
                    inner: transport,
                    step,
                }),
                cfg,
                device.bed,
                port,
                sink,
                timing,
                true,
            )
        } else if let Some(index) = usb_index(port) {
            // El reconocimiento de seguridad se exige antes de abrir nada.
            let token = ack.ok_or_else(ack_required)?.token()?;
            let transport = UsbTransport::open(index, UsbTimeouts::default(), token)
                .map_err(|e| tr!("desktop.galvo.open_failed", err = e))?;
            Self::start(
                Box::new(transport),
                cfg,
                device.bed,
                port,
                sink,
                timing,
                false,
            )
        } else {
            Err(tr!("desktop.galvo.not_galvo_port", port = port))
        }
    }

    /// Sondea la tarjeta por `transport` (solo lectura) y arranca el hilo. `label` es lo que se
    /// muestra como puerto.
    pub fn start(
        transport: Box<dyn Transport>,
        cfg: SessionConfig,
        bed: [f64; 2],
        label: &str,
        sink: Arc<dyn Sink>,
        timing: GalvoTimingCfg,
        virtual_machine: bool,
    ) -> Result<(Self, ConnectResult), String> {
        let mut session = GalvoSession::new(transport, cfg.clone()).map_err(connect_error)?;
        log(sink.as_ref(), LogDir::Out, &tr!("desktop.galvo.log_probe"));
        let info = session.probe().map_err(connect_error)?;
        for line in describe_info(&info) {
            log(sink.as_ref(), LogDir::In, &line);
        }
        log(sink.as_ref(), LogDir::In, &format!("; {}", experimental()));
        if virtual_machine {
            log(sink.as_ref(), LogDir::In, &tr!("desktop.galvo.log_virtual"));
        }
        let serial = words_hex(&info.serial);
        let version = words_hex(&info.version);
        let usb_id = format!(
            "{:04X}:{:04X}",
            info.transport.vendor_id, info.transport.product_id
        );
        let firmware = if virtual_machine {
            tr!(
                "desktop.galvo.firmware_virtual",
                board = info.board_name,
                usb = usb_id
            )
        } else {
            tr!(
                "desktop.galvo.firmware",
                board = info.board_name,
                usb = usb_id
            )
        };
        let live = Live {
            status_raw: info.status.raw,
            busy: info.status.busy,
            ready: info.status.ready,
            axis: info.status.axis,
            inputs: info.inputs,
            inputs_known: true,
            position: None,
        };
        let shared = Arc::new(Shared {
            dead: AtomicBool::new(false),
            busy: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            laser_may_be_on: AtomicBool::new(false),
            red_on: AtomicBool::new(false),
            framing: AtomicBool::new(false),
            live: Mutex::new(live),
            status: Mutex::new(None),
            gate: Mutex::new(Gate::default()),
            gate_cv: Condvar::new(),
            control: session.control(),
            virtual_machine,
            board: info.board_name.clone(),
            transport: info.transport.clone(),
            serial,
            version,
            cfg: cfg.clone(),
            bed,
        });
        let (tx, rx) = mpsc::channel();
        let worker = Worker {
            session,
            rx,
            shared: shared.clone(),
            sink: sink.clone(),
            timing,
            port: label.to_string(),
            bed,
            failures: 0,
            last_state: MachineState::Idle,
        };
        sink.emit(Emit::Connection(ConnectionEvent {
            connected: true,
            port: Some(label.to_string()),
            error: None,
        }));
        let thread = thread::Builder::new()
            .name("tlaku-galvo".into())
            .spawn(move || worker.run())
            .map_err(|e| tr!("desktop.machine.thread_failed", err = e))?;
        Ok((
            Self {
                handle: GalvoHandle { tx, shared },
                thread: Some(thread),
            },
            ConnectResult { firmware },
        ))
    }
}

impl Drop for GalvoConnection {
    fn drop(&mut self) {
        self.request_close();
    }
}

// ---------------------------------------------------------------------------
// Verificación sin láser (asistente de alta, M41)
// ---------------------------------------------------------------------------

/// Una comprobación de la verificación.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VerifyCheck {
    /// `board`, `idle`, `profile`, `frame` o `interlock`.
    pub id: String,
    pub ok: bool,
    pub detail: String,
}

/// Resultado de [`verify`]: lo que dijo la tarjeta y las comprobaciones del perfil.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GalvoVerifyReport {
    pub board: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub virtual_machine: bool,
    pub serial: String,
    pub version: String,
    pub status_raw: u16,
    pub busy: bool,
    pub ready: bool,
    pub inputs: u16,
    pub interlock: Option<GalvoInterlock>,
    pub checks: Vec<VerifyCheck>,
}

impl GalvoVerifyReport {
    /// Todas las comprobaciones salieron bien.
    #[cfg(test)]
    pub fn all_ok(&self) -> bool {
        self.checks.iter().all(|c| c.ok)
    }
}

/// Verificación SIN LÁSER de un perfil galvo: abre la tarjeta (la real exige el reconocimiento de
/// seguridad de ESTA verificación; la virtual no), solo la LEE (serie, versión, estado, entradas) y
/// comprueba, sin enviar nada, que el perfil se compila: la lista del encuadre del campo. No mueve
/// los espejos, no enciende el puntero ni la fuente, y la cierra al terminar. Una tarjeta con un
/// VID:PID desconocido se rechaza antes de enviar nada.
pub fn verify(
    device: &DeviceProfile,
    port: &str,
    ack: Option<&SafetyAck>,
) -> Result<GalvoVerifyReport, String> {
    let cfg = session_config(device)?;
    let (transport, virtual_machine): (Box<dyn Transport>, bool) = if is_virtual(port) {
        let (t, _state) = emulator(EmuConfig {
            source: device.galvo.source,
            field_mm: device.galvo.field_mm,
            ..EmuConfig::default()
        });
        virtual_speed(port)?;
        (Box::new(t), true)
    } else if let Some(index) = usb_index(port) {
        let token = ack.ok_or_else(ack_required)?.token()?;
        let t = UsbTransport::open(index, UsbTimeouts::default(), token)
            .map_err(|e| tr!("desktop.galvo.open_failed", err = e))?;
        (Box::new(t), false)
    } else {
        return Err(tr!("desktop.galvo.not_galvo_port", port = port));
    };
    verify_with(transport, cfg, device, virtual_machine)
}

/// El cuerpo de [`verify`] con el transporte ya abierto: aquí se rechaza una tarjeta con un VID:PID
/// desconocido (`GalvoSession::new`) antes de enviar nada, y solo se LEE.
pub(super) fn verify_with(
    transport: Box<dyn Transport>,
    cfg: SessionConfig,
    device: &DeviceProfile,
    virtual_machine: bool,
) -> Result<GalvoVerifyReport, String> {
    let mut session = GalvoSession::new(transport, cfg.clone()).map_err(connect_error)?;
    let info = session.probe().map_err(connect_error)?;
    // La sesión solo leyó: al soltarla no hay nada que apagar.
    drop(session);
    let mut checks = vec![VerifyCheck {
        id: "board".into(),
        ok: true,
        detail: format!(
            "{} (USB {:04X}:{:04X})",
            info.board_name, info.transport.vendor_id, info.transport.product_id
        ),
    }];
    checks.push(VerifyCheck {
        id: "idle".into(),
        ok: !info.status.busy,
        detail: if info.status.busy {
            tr!("desktop.galvo.verify_busy")
        } else {
            tr!("desktop.galvo.verify_idle")
        },
    });
    // El perfil: el encuadre del campo (95 % del lado, centrado en la cama) debe compilar, caber
    // en el campo y pasar el validador de listas, con el desfase del puntero del perfil.
    let side = device.galvo.field_mm * 0.95;
    let c = [device.bed[0] / 2.0, device.bed[1] / 2.0];
    let shape = LightShape::Rect {
        min: Point::new(c[0] - side / 2.0, c[1] - side / 2.0),
        max: Point::new(c[0] + side / 2.0, c[1] + side / 2.0),
    };
    let frame = compile::compile_light(
        &shape,
        &cfg.device,
        &LightOptions {
            origin: cfg.origin,
            pointer_offset: cfg.pointer_offset,
        },
    );
    checks.push(match frame {
        Ok(l) => VerifyCheck {
            id: "frame".into(),
            ok: true,
            detail: tr!(
                "desktop.galvo.verify_frame_ok",
                side = format!("{side:.0}"),
                commands = l.stats.commands
            ),
        },
        Err(e) => VerifyCheck {
            id: "frame".into(),
            ok: false,
            detail: e.to_string(),
        },
    });
    let interlock = cfg.inputs_required.map(|r| GalvoInterlock {
        mask: r.mask,
        value: r.value,
        met: info.inputs & r.mask == r.value,
    });
    if let Some(i) = interlock {
        checks.push(VerifyCheck {
            id: "interlock".into(),
            ok: i.met,
            detail: tr!(
                "desktop.galvo.verify_interlock",
                inputs = format!("{:#06x}", info.inputs),
                mask = format!("{:#06x}", i.mask),
                value = format!("{:#06x}", i.value)
            ),
        });
    }
    Ok(GalvoVerifyReport {
        board: info.board_name,
        vendor_id: info.transport.vendor_id,
        product_id: info.transport.product_id,
        virtual_machine,
        serial: words_hex(&info.serial),
        version: words_hex(&info.version),
        status_raw: info.status.raw,
        busy: info.status.busy,
        ready: info.status.ready,
        inputs: info.inputs,
        interlock,
        checks,
    })
}

impl<T: Transport> Transport for Paced<T> {
    fn write(&mut self, data: &[u8]) -> Result<(), TransportError> {
        self.inner.write(data)
    }

    fn read(&mut self) -> Result<[u8; tlaku_galvo::wire::REPLY_LEN], TransportError> {
        self.pace();
        self.inner.read()
    }

    fn info(&self) -> TransportInfo {
        self.inner.info()
    }
}

pub(super) fn words_hex(w: &[u16; 4]) -> String {
    format!("{:04X}-{:04X}-{:04X}-{:04X}", w[0], w[1], w[2], w[3])
}

pub(super) fn describe_info(info: &BoardInfo) -> Vec<String> {
    let t = &info.transport;
    let yes_no = |v: bool| {
        if v {
            tr!("desktop.galvo.yes")
        } else {
            tr!("desktop.galvo.no")
        }
    };
    let mut out = vec![
        tr!(
            "desktop.galvo.board_line",
            board = info.board_name,
            usb = format!("{:04X}:{:04X}", t.vendor_id, t.product_id),
            serial = words_hex(&info.serial),
            version = words_hex(&info.version)
        ),
        tr!(
            "desktop.galvo.status_line",
            raw = format!("{:#06x}", info.status.raw),
            busy = yes_no(info.status.busy),
            ready = yes_no(info.status.ready),
            inputs = format!("{:#06x}", info.inputs)
        ),
    ];
    out.extend(
        info.notes
            .iter()
            .map(|n| tr!("desktop.lhymicro.log_note", note = n)),
    );
    out
}

/// Error del sondeo al conectar, con lo que conviene revisar.
pub(super) fn connect_error(e: SessionError) -> String {
    match &e {
        SessionError::UnknownBoard { .. } => tr!("desktop.galvo.unknown_board", err = e),
        _ if usb_silence(&e) => tr!("desktop.galvo.talk_failed_usb", err = e),
        _ => tr!("desktop.galvo.talk_failed", err = e),
    }
}

/// ¿El fallo es un silencio del USB (cable, máquina apagada o tarjeta ocupada por otro programa)?
pub(super) fn usb_silence(e: &SessionError) -> bool {
    matches!(e, SessionError::Transport(_) | SessionError::Timeout { .. })
}

/// Error de una operación en marcha, con la pista del USB si es un silencio.
pub(super) fn op_error(e: &SessionError) -> String {
    if usb_silence(e) {
        tr!("desktop.galvo.op_failed_usb", err = e)
    } else {
        e.to_string()
    }
}

/// Texto de un fallo de trabajo, con la pista del USB si es un silencio. Si no se confirmó el
/// apagado de la fuente lleva el código estable `stop_unconfirmed` (ver [`STOP_UNCONFIRMED`]).
pub(super) fn job_error_text(e: &JobError) -> String {
    let text = if usb_silence(&e.cause) {
        tr!("desktop.galvo.op_failed_usb", err = e)
    } else {
        e.to_string()
    };
    match e.code() {
        Some(code) => tlaku_i18n::coded(code, text),
        None => text,
    }
}

pub(super) fn emit_job(sink: &dyn Sink, state: JobState, message: &str) {
    sink.emit(Emit::Job(JobEvent {
        state,
        message: message.to_string(),
        // El galvo no pierde posición: no hay carro que haya quedado a medio camino.
        position_lost: false,
    }));
}
