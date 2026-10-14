//! [`GalvoConnection`] (conexión USB o virtual), transporte con ritmo y mensajes de error.

use super::*;

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
            let token = ack.ok_or_else(|| ACK_REQUIRED.to_string())?.token()?;
            let transport = UsbTransport::open(index, UsbTimeouts::default(), token)
                .map_err(|e| format!("No se pudo abrir la tarjeta galvo: {e}"))?;
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
            Err(format!(
                "«{port}» no es un puerto galvo. Elige una tarjeta USB de la lista o la máquina \
                 virtual."
            ))
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
        log(sink.as_ref(), LogDir::Out, "; sondeo galvo (solo lectura)");
        let info = session.probe().map_err(connect_error)?;
        for line in describe_info(&info) {
            log(sink.as_ref(), LogDir::In, &line);
        }
        log(sink.as_ref(), LogDir::In, &format!("; {EXPERIMENTAL}"));
        if virtual_machine {
            log(
                sink.as_ref(),
                LogDir::In,
                "; Máquina virtual (tarjeta simulada): no hay láser ni hardware reales.",
            );
        }
        let serial = words_hex(&info.serial);
        let version = words_hex(&info.version);
        let firmware = format!(
            "Galvo · {} · {:04X}:{:04X}{}",
            info.board_name,
            info.transport.vendor_id,
            info.transport.product_id,
            if virtual_machine { " · virtual" } else { "" }
        );
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
            .map_err(|e| format!("No se pudo iniciar el hilo de la conexión: {e}"))?;
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

/// Transporte que duerme `step` en cada lectura: marca el paso del reloj virtual del emulador
/// (que avanza una vez por consulta de estado) contra el reloj de pared.
pub(super) struct Paced<T> {
    pub(super) inner: T,
    pub(super) step: Duration,
}

impl<T: Transport> Transport for Paced<T> {
    fn write(&mut self, data: &[u8]) -> Result<(), TransportError> {
        self.inner.write(data)
    }

    fn read(&mut self) -> Result<[u8; tlaku_galvo::wire::REPLY_LEN], TransportError> {
        thread::sleep(self.step);
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
    let mut out = vec![
        format!(
            "Tarjeta: {} (USB {:04X}:{:04X}); serie {}; versión {}",
            info.board_name,
            t.vendor_id,
            t.product_id,
            words_hex(&info.serial),
            words_hex(&info.version)
        ),
        format!(
            "Estado: {:#06x} (ocupada: {}, lista: {}); entradas: {:#06x}",
            info.status.raw,
            if info.status.busy { "sí" } else { "no" },
            if info.status.ready { "sí" } else { "no" },
            info.inputs
        ),
    ];
    out.extend(info.notes.iter().map(|n| format!("; AVISO: {n}")));
    out
}

/// Error del sondeo al conectar, con lo que conviene revisar.
pub(super) fn connect_error(e: SessionError) -> String {
    match &e {
        SessionError::UnknownBoard { .. } => {
            format!("{e}. Tlaku solo habla con la LMCV4-FIBER-M (USB 9588:9899).")
        }
        _ => format!(
            "No se pudo hablar con la tarjeta galvo: {e}{}",
            usb_hint(&e)
        ),
    }
}

pub(super) fn usb_hint(e: &SessionError) -> &'static str {
    match e {
        SessionError::Transport(_) | SessionError::Timeout { .. } => {
            ". Comprueba el cable USB, que la máquina esté encendida y que EZCAD, LightBurn o \
             MeerK40t no tengan la tarjeta abierta"
        }
        _ => "",
    }
}

/// Error de una operación en marcha, con la pista del USB si es un silencio.
pub(super) fn op_error(e: &SessionError) -> String {
    format!("{e}{}", usb_hint(e))
}

pub(super) fn job_error_text(e: &JobError) -> String {
    format!("{e}{}", usb_hint(&e.cause))
}

pub(super) fn log(sink: &dyn Sink, dir: LogDir, text: &str) {
    sink.emit(Emit::Log(LogEvent {
        dir,
        text: text.to_string(),
    }));
}

pub(super) fn emit_job(sink: &dyn Sink, state: JobState, message: &str) {
    sink.emit(Emit::Job(JobEvent {
        state,
        message: message.to_string(),
        // El galvo no pierde posición: no hay carro que haya quedado a medio camino.
        position_lost: false,
    }));
}
