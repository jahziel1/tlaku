//! Solicitudes al hilo, estado compartido y el mando [`GalvoHandle`].

use super::*;
use tlaku_i18n::tr;

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
        /// Vueltas tras las cuales termina solo; `None` = hasta detener.
        loops: Option<u32>,
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
    /// Hay un encuadre en curso (admite mover y escalar en vivo).
    pub(super) framing: AtomicBool,
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
            return Err(crate::machine::not_connected());
        }
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(build(tx))
            .map_err(|_| crate::machine::not_connected())?;
        rx.recv()
            .map_err(|_| tr!("desktop.machine.connection_lost"))?
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
            Err(tr!("desktop.galvo.busy"))
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
            framing: sh.framing.load(Ordering::SeqCst),
            interlock: sh.cfg.inputs_required.map(|c| GalvoInterlock {
                mask: c.mask,
                value: c.value,
                met: inputs_known && inputs & c.mask == c.value,
            }),
            pointer_offset: sh.cfg.pointer_offset,
            overscan_mm: sh.cfg.device.overscan_mm,
        }
    }

    /// El perfil debe seguir siendo el de la conexión en lo que usa el driver (campo, fuente,
    /// límites, corrección, cama). El marcado dividido y repetido, el cilindro y el cono los
    /// calcula el pipeline y pueden cambiar sin reconectar.
    fn check_profile(&self, device: &DeviceProfile) -> Result<(), String> {
        if device.kind != DeviceKind::Galvo {
            return Err(tr!("desktop.galvo.profile_not_galvo"));
        }
        let now = (
            driver_part(&device.galvo),
            device.bed,
            device.pointer_offset,
        );
        let was = (
            driver_part(&self.shared.cfg.device),
            self.shared.bed,
            self.shared.cfg.pointer_offset,
        );
        if now != was {
            return Err(tr!("desktop.galvo.profile_changed"));
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
        let kind = if contour {
            FrameKind::Contour
        } else {
            FrameKind::Rect
        };
        self.frame(doc, device, options, kind, None, None)
    }

    /// Encuadre con el puntero rojo de la forma pedida (rectángulo, trazos, casco convexo u
    /// objetos sueltos). `loops`: vueltas tras las cuales termina solo (`None` = hasta detener).
    pub fn frame(
        &self,
        doc: &tlaku_doc::Document,
        device: &DeviceProfile,
        options: &tlaku_doc::JobOptions,
        kind: FrameKind,
        loops: Option<u32>,
        gap_mm: Option<f64>,
    ) -> Result<(), String> {
        self.check_profile(device)?;
        self.ensure_free()?;
        if loops == Some(0) {
            return Err(tr!("desktop.galvo.loops_min"));
        }
        if gap_mm.is_some_and(|g| !g.is_finite() || g < 0.0) {
            return Err(tr!("desktop.galvo.gap_invalid"));
        }
        let shape = light_shape(doc, device, options, kind, gap_mm)?;
        // Se valida el encuadre (campo, desfase del puntero) antes de tocar la tarjeta.
        compile::compile_light(
            &shape,
            &self.shared.cfg.device,
            &LightOptions {
                origin: self.shared.cfg.origin,
                pointer_offset: self.shared.cfg.pointer_offset,
            },
        )
        .map_err(|e| e.to_string())?;
        self.arm_request();
        self.call(|reply| Request::Light {
            shape: Box::new(shape),
            loops,
            reply,
        })
    }

    /// Mueve (mm) o escala el encuadre que está en curso, sin detenerlo. Se aplica en la vuelta
    /// siguiente; si el resultado no cabe en el campo se descarta y el encuadre sigue como estaba
    /// (el evento `light` lo avisa). `scale` es respecto al tamaño original; 1 = sin escalar.
    pub fn adjust_frame(&self, dx: f64, dy: f64, scale: f64) -> Result<(), String> {
        if self.is_closed() {
            return Err(crate::machine::not_connected());
        }
        if !self.shared.framing.load(Ordering::SeqCst) {
            return Err(tr!("desktop.galvo.no_frame_running"));
        }
        self.shared
            .control
            .set_light_adjust(LightAdjust { dx, dy, scale })
            .map(|_| ())
    }

    /// Enciende o apaga el puntero rojo en su sitio (la fuente queda deshabilitada).
    pub fn set_red_light(&self, on: bool) -> Result<(), String> {
        // Con algo en marcha no se encola (la llamada se quedaría esperando al final del trabajo y
        // borraría un «detener» en vuelo): para apagarlo todo está Detener.
        if self.working() {
            return Err(if on {
                tr!("desktop.galvo.busy")
            } else {
                tr!("desktop.galvo.job_running_press_stop")
            });
        }
        self.arm_request();
        self.call(|reply| Request::RedLight { on, reply })
    }

    /// Salto relativo del haz (mm) con la fuente desarmada. Parte de la última orden de salto, o
    /// del centro del campo si no hay ninguna.
    pub fn jog(&self, dx: f64, dy: f64, _speed_mm_min: f64) -> Result<(), String> {
        if !(dx.is_finite() && dy.is_finite()) {
            return Err(tr!("desktop.common.offset_invalid"));
        }
        let base = self
            .work_pos()
            .unwrap_or([self.shared.bed[0] / 2.0, self.shared.bed[1] / 2.0]);
        self.jog_to(base[0] + dx, base[1] + dy, _speed_mm_min)
    }

    /// Salto absoluto del haz (mm del documento) con la fuente desarmada. Debe caer en el campo.
    pub fn jog_to(&self, x: f64, y: f64, _speed_mm_min: f64) -> Result<(), String> {
        if !(x.is_finite() && y.is_finite()) {
            return Err(tr!("desktop.common.position_invalid"));
        }
        self.ensure_free()?;
        let f = field_bounds(self.shared.bed, self.shared.cfg.device.field_mm);
        if x < f.min.x || x > f.max.x || y < f.min.y || y > f.max.y {
            return Err(tr!(
                "desktop.galvo.point_outside_field",
                x = format!("{x:.2}"),
                y = format!("{y:.2}"),
                x0 = format!("{:.1}", f.min.x),
                x1 = format!("{:.1}", f.max.x),
                y0 = format!("{:.1}", f.min.y),
                y1 = format!("{:.1}", f.max.y)
            ));
        }
        self.arm_request();
        self.call(|reply| Request::JogTo { x, y, reply })
    }

    /// El usuario ya movió el material (o el rotativo): el trabajo sigue con la pieza siguiente.
    pub fn confirm_move(&self) -> Result<(), String> {
        if self.is_closed() {
            return Err(crate::machine::not_connected());
        }
        let mut g = self
            .shared
            .gate
            .lock()
            .map_err(|_| tr!("desktop.common.internal_state"))?;
        if g.waiting.is_none() {
            return Err(tr!("desktop.galvo.no_step_waiting"));
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
            return Err(crate::machine::not_connected());
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
