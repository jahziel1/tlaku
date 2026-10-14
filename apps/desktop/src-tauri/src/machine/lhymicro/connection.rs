//! [`LhyConnection`] (USB o virtual), transporte con ritmo y mensajes de error.

use super::*;

pub(super) use crate::machine::common::{Paced, frame_done_log, log, virtual_speed};
use tlaku_i18n::tr;

/// Conexión abierta: el mando y el hilo.
pub struct LhyConnection {
    handle: LhyHandle,
    thread: Option<JoinHandle<()>>,
}

impl LhyConnection {
    pub fn handle(&self) -> LhyHandle {
        self.handle.clone()
    }

    fn request_close(&self) {
        // Un trabajo en curso se detiene (y la placa también) antes de cerrar.
        self.handle.shared.cancel.store(true, Ordering::SeqCst);
        self.handle.shared.control.request_stop();
        let _ = self.handle.tx.send(Request::Disconnect);
    }

    /// Cierra la conexión (deteniendo antes un trabajo en curso) y espera al hilo.
    pub fn close(mut self) {
        self.request_close();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }

    /// Abre la placa del puerto (`usb:k40:<n>`) o la máquina virtual (`virtual:k40`), la sondea
    /// (solo lectura) y arranca el hilo.
    pub fn connect(
        device: &DeviceProfile,
        port: &str,
        sink: Arc<dyn Sink>,
        timing: LhyTiming,
    ) -> Result<(Self, ConnectResult), String> {
        let mut cfg = session_config(device)?;
        if is_virtual(port) {
            let speed = virtual_speed(port)?;
            let emu = EmuConfig {
                board: cfg.board,
                ..EmuConfig::default()
            };
            let step = Duration::from_secs_f64(emu.poll_dt / speed);
            let (transport, _board) = emulator(emu);
            // El tiempo ya lo marca el paso de cada lectura: no hace falta esperar más.
            cfg.poll_backoff_max = Duration::ZERO;
            Self::start(
                Box::new(Paced {
                    inner: transport,
                    step,
                }),
                cfg,
                port,
                sink,
                timing,
                true,
            )
        } else if let Some(index) = usb_index(port) {
            let transport = UsbTransport::open(index, UsbTimeouts::default())
                .map_err(|e| tr!("desktop.lhymicro.open_failed", err = e))?;
            Self::start(Box::new(transport), cfg, port, sink, timing, false)
        } else {
            Err(tr!("desktop.lhymicro.not_k40_port", port = port))
        }
    }

    /// Sondea la placa por `transport` y arranca el hilo. `label` es lo que se muestra como puerto.
    pub fn start(
        transport: Box<dyn Transport>,
        cfg: SessionConfig,
        label: &str,
        sink: Arc<dyn Sink>,
        timing: LhyTiming,
        virtual_machine: bool,
    ) -> Result<(Self, ConnectResult), String> {
        let mut session = LhySession::new(transport, cfg.clone());
        log(
            sink.as_ref(),
            LogDir::Out,
            &tr!("desktop.lhymicro.log_probe"),
        );
        let info = session.probe().map_err(connect_error)?;
        for line in describe_info(&info) {
            log(sink.as_ref(), LogDir::In, &line);
        }
        if matches!(info.status.status, Status::NoReply) {
            // El USB abre pero la placa no contesta: no hay con qué hablar.
            return Err(tr!("desktop.lhymicro.no_status"));
        }
        log(sink.as_ref(), LogDir::In, &format!("; {}", experimental()));
        log(
            sink.as_ref(),
            LogDir::In,
            &tr!("desktop.lhymicro.log_position_unknown"),
        );
        if virtual_machine {
            log(
                sink.as_ref(),
                LogDir::In,
                &tr!("desktop.lhymicro.log_virtual"),
            );
        }
        let firmware = tr!(
            "desktop.lhymicro.firmware",
            board = cfg.board.name(),
            transport = if info.transport.name.is_empty() {
                "CH341"
            } else {
                &info.transport.name
            },
            status = info.status.status.describe()
        );

        let shared = Arc::new(Shared {
            dead: AtomicBool::new(false),
            busy: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            pos_known: AtomicBool::new(false),
            status_code: Mutex::new(info.status.code()),
            status: Mutex::new(None),
            control: session.control(),
            sink: sink.clone(),
            virtual_machine,
            board: cfg.board,
            cfg: cfg.clone(),
            timing: timing.clone(),
        });
        let (tx, rx) = mpsc::channel();
        let worker = Worker {
            session,
            rx,
            shared: shared.clone(),
            sink: sink.clone(),
            timing,
            port: label.to_string(),
            home: cfg.home,
            bed: cfg.bed_mm,
            failures: 0,
            last_state: MachineState::Idle,
            last_pos: [0.0, 0.0],
        };
        sink.emit(Emit::Connection(ConnectionEvent {
            connected: true,
            port: Some(label.to_string()),
            error: None,
        }));
        let thread = thread::Builder::new()
            .name("tlaku-lhymicro".into())
            .spawn(move || worker.run())
            .map_err(|e| tr!("desktop.machine.thread_failed", err = e))?;
        Ok((
            Self {
                handle: LhyHandle { tx, shared },
                thread: Some(thread),
            },
            ConnectResult { firmware },
        ))
    }
}

impl Drop for LhyConnection {
    fn drop(&mut self) {
        self.request_close();
    }
}

impl<T: Transport> Transport for Paced<T> {
    fn write(&mut self, data: &[u8]) -> Result<(), TransportError> {
        self.inner.write(data)
    }

    fn read(&mut self, len: usize) -> Result<Vec<u8>, TransportError> {
        self.pace();
        self.inner.read(len)
    }

    fn info(&self) -> TransportInfo {
        self.inner.info()
    }
}

pub(super) fn describe_info(info: &BoardInfo) -> Vec<String> {
    let t = &info.transport;
    let name = if t.name.is_empty() { "CH341" } else { &t.name };
    let vid_pid = format!("{:04X}:{:04X}", t.vendor_id, t.product_id);
    let status = info.status.status.describe();
    let first = match t.chip_version {
        Some(v) => tr!(
            "desktop.lhymicro.board_line_chip",
            name = name,
            usb = vid_pid,
            chip = format!("0x{v:02X}"),
            status = status
        ),
        None => tr!(
            "desktop.lhymicro.board_line",
            name = name,
            usb = vid_pid,
            status = status
        ),
    };
    let mut out = vec![first];
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
        SessionError::NotM2Nano { .. } => tr!("desktop.lhymicro.not_m2nano", err = e),
        SessionError::Transport(_) | SessionError::Timeout { .. } => {
            tr!("desktop.lhymicro.talk_failed_usb", err = e)
        }
        _ => tr!("desktop.lhymicro.talk_failed", err = e),
    }
}

/// Error de una operación en marcha, con la pista del USB si es un silencio.
pub(super) fn op_error(e: SessionError) -> String {
    match &e {
        SessionError::UnknownPosition => unknown_position(),
        SessionError::Transport(_) | SessionError::Timeout { .. } => {
            tr!("desktop.lhymicro.op_failed_usb", err = e)
        }
        SessionError::Busy(_) => tr!("desktop.lhymicro.op_busy", err = e),
        _ => e.to_string(),
    }
}

/// Texto de un trabajo que falló o se detuvo. Si la placa no confirmó detenerse (el láser pudo
/// quedar encendido), lleva el código estable [`STOP_UNCONFIRMED`] al inicio: la interfaz lo
/// reconoce por el código —que se decide por el tipo de error, no por su texto— en cualquier idioma.
pub(super) fn job_error_text(e: &tlaku_lhymicro::session::JobError) -> String {
    let text = e.to_string();
    if e.laser_may_be_on() {
        tlaku_i18n::coded(STOP_UNCONFIRMED, text)
    } else {
        text
    }
}

pub(super) fn emit_job(sink: &dyn Sink, state: JobState, message: &str, position_lost: bool) {
    sink.emit(Emit::Job(JobEvent {
        state,
        message: message.to_string(),
        position_lost,
    }));
}
