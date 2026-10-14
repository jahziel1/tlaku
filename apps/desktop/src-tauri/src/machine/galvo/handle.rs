//! Solicitudes al hilo, estado compartido y el mando [`GalvoHandle`].

use super::*;

// ---------------------------------------------------------------------------
// Mando
// ---------------------------------------------------------------------------

pub(super) type Reply<T> = Sender<Result<T, String>>;

pub(super) enum Request {
    /// Marca los pasos. Responde cuando sale la primera tanda (o antes, si falla).
    Run {
        job: Box<PreparedJob>,
        reply: Reply<()>,
    },
    /// Encuadre con el puntero rojo en bucle. Responde al terminar la primera vuelta.
    Light {
        shape: Box<LightShape>,
        reply: Reply<()>,
    },
    RedLight {
        on: bool,
        reply: Reply<()>,
    },
    /// Salto de los espejos a un punto del documento (mm), con la fuente desarmada.
    JogTo {
        x: f64,
        y: f64,
        reply: Reply<()>,
    },
    /// Detener y apagar todo sin trabajo en marcha.
    Stop(Reply<()>),
    Disconnect,
}

/// Datos que la sonda de solo lectura y el sondeo van actualizando.
#[derive(Default)]
pub(super) struct Live {
    pub(super) status_raw: u16,
    pub(super) busy: bool,
    pub(super) ready: bool,
    pub(super) axis: bool,
    pub(super) inputs: u16,
    pub(super) inputs_known: bool,
    pub(super) position: Option<[f64; 2]>,
}

#[derive(Default)]
pub(super) struct Gate {
    pub(super) waiting: Option<StepInfo>,
    pub(super) confirmed: bool,
}

pub(super) struct Shared {
    /// El hilo terminó (desconexión pedida o perdida).
    pub(super) dead: AtomicBool,
    /// El hilo está ocupado con un trabajo, un encuadre o un salto.
    pub(super) busy: AtomicBool,
    /// Se pidió detener (también interrumpe la espera de un movimiento de material).
    pub(super) cancel: AtomicBool,
    /// El último apagado no se confirmó.
    pub(super) laser_may_be_on: AtomicBool,
    /// El puntero rojo quedó encendido en su sitio.
    pub(super) red_on: AtomicBool,
    pub(super) live: Mutex<Live>,
    pub(super) status: Mutex<Option<StatusReport>>,
    pub(super) gate: Mutex<Gate>,
    pub(super) gate_cv: Condvar,
    pub(super) control: JobControl,
    pub(super) virtual_machine: bool,
    pub(super) board: String,
    pub(super) transport: TransportInfo,
    pub(super) serial: String,
    pub(super) version: String,
    /// Lo que se usó al conectar: si el perfil cambia, hay que reconectar.
    pub(super) cfg: SessionConfig,
    pub(super) bed: [f64; 2],
}

/// Mando de la conexión; se clona y se usa desde cualquier hilo. Las llamadas bloquean hasta que
/// el hilo responde (en Tauri, dentro de `spawn_blocking`).
#[derive(Clone)]
pub struct GalvoHandle {
    pub(super) tx: Sender<Request>,
    pub(super) shared: Arc<Shared>,
}

impl GalvoHandle {
    fn call<T>(&self, build: impl FnOnce(Reply<T>) -> Request) -> Result<T, String> {
        if self.is_closed() {
            return Err(crate::machine::NOT_CONNECTED.into());
        }
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(build(tx))
            .map_err(|_| crate::machine::NOT_CONNECTED.to_string())?;
        rx.recv()
            .map_err(|_| "Se perdió la conexión con la máquina.".to_string())?
    }

    pub fn is_closed(&self) -> bool {
        self.shared.dead.load(Ordering::SeqCst)
    }

    #[cfg(test)]
    pub fn is_virtual(&self) -> bool {
        self.shared.virtual_machine
    }

    fn working(&self) -> bool {
        self.shared.busy.load(Ordering::SeqCst)
            || self.shared.control.state() != GalvoJobState::Idle
    }

    /// Falla si el hilo está ocupado: las órdenes de movimiento y de inicio solo valen en reposo.
    fn ensure_free(&self) -> Result<(), String> {
        if self.working() {
            Err(
                "La máquina está ocupada (trabajo, encuadre o salto en curso). Espera a que \
                 termine o detenla."
                    .into(),
            )
        } else {
            Ok(())
        }
    }

    /// Posición del haz según la última orden de salto (mm del documento), si se conoce.
    pub fn work_pos(&self) -> Option<[f64; 2]> {
        self.shared.live.lock().ok()?.position
    }

    pub fn status_report(&self) -> Option<StatusReport> {
        self.shared.status.lock().ok()?.clone()
    }

    pub fn state(&self) -> GalvoState {
        let sh = &self.shared;
        let (live_status, busy, ready, axis, inputs, inputs_known, pos) =
            sh.live
                .lock()
                .map_or((0, false, false, false, 0, false, None), |l| {
                    (
                        l.status_raw,
                        l.busy,
                        l.ready,
                        l.axis,
                        l.inputs,
                        l.inputs_known,
                        l.position,
                    )
                });
        let center = [sh.bed[0] / 2.0, sh.bed[1] / 2.0];
        GalvoState {
            board: sh.board.clone(),
            vendor_id: sh.transport.vendor_id,
            product_id: sh.transport.product_id,
            virtual_machine: sh.virtual_machine,
            serial: sh.serial.clone(),
            version: sh.version.clone(),
            status_raw: live_status,
            busy,
            ready,
            axis,
            inputs,
            inputs_known,
            working: self.working(),
            red_light_on: sh.red_on.load(Ordering::SeqCst),
            laser_may_be_on: sh.laser_may_be_on.load(Ordering::SeqCst),
            position: pos.unwrap_or(center),
            position_known: pos.is_some(),
            field_mm: sh.cfg.device.field_mm,
            source: sh.cfg.device.source,
            waiting_move: sh.gate.lock().ok().and_then(|g| g.waiting.clone()),
        }
    }

    /// El perfil debe seguir siendo el de la conexión en lo que usa el driver (campo, fuente,
    /// límites, corrección, cama). El marcado dividido y repetido, el cilindro y el cono los
    /// calcula el pipeline y pueden cambiar sin reconectar.
    fn check_profile(&self, device: &DeviceProfile) -> Result<(), String> {
        if device.kind != DeviceKind::Galvo {
            return Err("El perfil de la máquina no es de tipo galvo.".into());
        }
        let now = (driver_part(&device.galvo), device.bed);
        let was = (driver_part(&self.shared.cfg.device), self.shared.bed);
        if now != was {
            return Err(
                "El perfil de la máquina cambió (campo, fuente, límites, corrección o \
                        cama) desde que te conectaste. Desconecta y vuelve a conectar para \
                        aplicarlo."
                    .into(),
            );
        }
        Ok(())
    }

    fn arm_request(&self) {
        // Un trabajo nuevo no hereda un «detener» anterior.
        self.shared.control.clear();
        self.shared.cancel.store(false, Ordering::SeqCst);
    }

    /// Calcula el trabajo y lo marca. Vuelve cuando sale la primera tanda (o falla antes); el final
    /// llega como evento. `red_only`: recorrido con el láser deshabilitado y el puntero rojo.
    pub fn start_job(
        &self,
        doc: &tlaku_doc::Document,
        device: &DeviceProfile,
        options: &tlaku_doc::JobOptions,
        red_only: bool,
    ) -> Result<(), String> {
        self.check_profile(device)?;
        self.ensure_free()?;
        let job = prepare(doc, device, options, red_only)?;
        self.arm_request();
        self.call(|reply| Request::Run {
            job: Box::new(job),
            reply,
        })
    }

    /// Encuadre con el puntero rojo (rectángulo o contorno) en bucle hasta detener. Vuelve al
    /// terminar la primera vuelta; nunca habilita el láser.
    pub fn frame_job(
        &self,
        doc: &tlaku_doc::Document,
        device: &DeviceProfile,
        options: &tlaku_doc::JobOptions,
        contour: bool,
    ) -> Result<(), String> {
        self.check_profile(device)?;
        self.ensure_free()?;
        let shape = light_shape(doc, device, options, contour)?;
        // Se valida el encuadre (campo) antes de tocar la tarjeta.
        compile::compile_light(
            &shape,
            &self.shared.cfg.device,
            &LightOptions {
                origin: self.shared.cfg.origin,
            },
        )
        .map_err(|e| e.to_string())?;
        self.arm_request();
        self.call(|reply| Request::Light {
            shape: Box::new(shape),
            reply,
        })
    }

    /// Enciende o apaga el puntero rojo en su sitio (la fuente queda deshabilitada).
    pub fn set_red_light(&self, on: bool) -> Result<(), String> {
        // Con algo en marcha no se encola (la llamada se quedaría esperando al final del trabajo y
        // borraría un «detener» en vuelo): para apagarlo todo está Detener.
        if self.working() {
            return Err(if on {
                "La máquina está ocupada (trabajo, encuadre o salto en curso). Espera a que \
                 termine o detenla."
                    .into()
            } else {
                "Hay un trabajo o un encuadre en curso: pulsa Detener, que apaga la fuente y el \
                 puntero."
                    .into()
            });
        }
        self.arm_request();
        self.call(|reply| Request::RedLight { on, reply })
    }

    /// Salto relativo del haz (mm) con la fuente desarmada. Parte de la última orden de salto, o
    /// del centro del campo si no hay ninguna.
    pub fn jog(&self, dx: f64, dy: f64, _speed_mm_min: f64) -> Result<(), String> {
        if !(dx.is_finite() && dy.is_finite()) {
            return Err("El desplazamiento no es válido.".into());
        }
        let base = self
            .work_pos()
            .unwrap_or([self.shared.bed[0] / 2.0, self.shared.bed[1] / 2.0]);
        self.jog_to(base[0] + dx, base[1] + dy, _speed_mm_min)
    }

    /// Salto absoluto del haz (mm del documento) con la fuente desarmada. Debe caer en el campo.
    pub fn jog_to(&self, x: f64, y: f64, _speed_mm_min: f64) -> Result<(), String> {
        if !(x.is_finite() && y.is_finite()) {
            return Err("La posición no es válida.".into());
        }
        self.ensure_free()?;
        let f = field_bounds(self.shared.bed, self.shared.cfg.device.field_mm);
        if x < f.min.x || x > f.max.x || y < f.min.y || y > f.max.y {
            return Err(format!(
                "El punto ({x:.2}, {y:.2}) mm queda fuera del campo del galvo (X de {:.1} a \
                 {:.1}, Y de {:.1} a {:.1} mm).",
                f.min.x, f.max.x, f.min.y, f.max.y
            ));
        }
        self.arm_request();
        self.call(|reply| Request::JogTo { x, y, reply })
    }

    /// El usuario ya movió el material (o el rotativo): el trabajo sigue con la pieza siguiente.
    pub fn confirm_move(&self) -> Result<(), String> {
        if self.is_closed() {
            return Err(crate::machine::NOT_CONNECTED.into());
        }
        let mut g = self
            .shared
            .gate
            .lock()
            .map_err(|_| "estado interno dañado")?;
        if g.waiting.is_none() {
            return Err("Ningún paso del trabajo espera que muevas el material.".into());
        }
        g.confirmed = true;
        self.shared.gate_cv.notify_all();
        Ok(())
    }

    /// Detiene el trabajo, el encuadre o la espera y apaga la fuente y el puntero. Con algo en
    /// marcha vuelve en cuanto lo pide: el resultado (y si la tarjeta confirmó el apagado) llega
    /// como evento. En reposo detiene y apaga en el acto.
    pub fn cancel(&self) -> Result<(), String> {
        if self.is_closed() {
            return Err(crate::machine::NOT_CONNECTED.into());
        }
        self.shared.cancel.store(true, Ordering::SeqCst);
        self.shared.control.request_stop();
        self.shared.gate_cv.notify_all();
        if self.working() {
            return Ok(());
        }
        let r = self.call(Request::Stop);
        self.shared.cancel.store(false, Ordering::SeqCst);
        r
    }
}
