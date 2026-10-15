//! Estado compartido, solicitudes al trabajador y el mando [`Handle`] de la conexión GRBL.

use super::*;
use tlaku_i18n::tr;

#[derive(Default)]
pub(super) struct Shared {
    /// Falso cuando el trabajador terminó (desconexión pedida o perdida).
    pub(super) dead: AtomicBool,
    /// Último reporte de estado.
    pub(super) status: Mutex<Option<StatusReport>>,
    /// Se hizo `$H` con éxito y la placa no se ha reiniciado ni perdido la posición desde
    /// entonces (condición para reanudar un trabajo en una máquina con homing).
    pub(super) homed: AtomicBool,
    /// Hay un trabajo (no un encuadre) transmitiéndose ahora mismo.
    pub(super) running: AtomicBool,
    /// Hay algo transmitiéndose (un trabajo o un encuadre): la consola y los movimientos se
    /// rechazan hasta que termine.
    pub(super) streaming: AtomicBool,
    /// Desde que se abrió esta conexión se hizo algo que mueve la cabeza (homing, jog, ir a una
    /// posición, sonda, encuadre, otro trabajo o una línea de G-code a mano). Ya no se puede
    /// suponer que la MPos de ahora es la que dejó un trabajo anterior (ver
    /// `resume::start::current_for`).
    pub(super) moved: AtomicBool,
    /// Con qué se comparó el punto de control en esta conexión (id del punto y la máquina vista,
    /// o `None` si no se compara); se fija en la primera evaluación y no cambia, así la
    /// sugerencia no se mueve por hacer homing o encuadrar. Ver `resume::start::current_for`.
    pub(super) resume_baseline: Mutex<Option<(String, Option<resume::assess::Now>)>>,
    /// Área de la cama [ancho, alto] en los ejes de la máquina (del perfil); la guarda del
    /// movimiento manual la usa junto con `$130`/`$131`.
    pub(super) bed: Mutex<Option<[f64; 2]>>,
}

pub(super) type Reply = Sender<Result<Vec<String>, String>>;

/// Qué se transmite: el trabajo o su encuadre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RunKind {
    Job,
    Frame,
}

pub(super) enum Request {
    Command {
        line: String,
        reply: Reply,
    },
    Realtime {
        cmd: RealtimeCommand,
        reply: Reply,
    },
    Jog {
        dx: f64,
        dy: f64,
        speed: f64,
        reply: Reply,
    },
    /// Movimiento absoluto en coordenadas de trabajo, con el láser apagado.
    JogTo {
        x: f64,
        y: f64,
        speed: f64,
        reply: Reply,
    },
    /// Sondeo de contacto con el cabezal (`G38.3`); ver `worker/probe.rs`.
    Probe {
        dx: f64,
        dy: f64,
        feed: f64,
        retract: f64,
        reply: Reply,
    },
    /// Ajusta un override en tiempo real (velocidad o potencia) al porcentaje pedido.
    Override {
        kind: OverrideKind,
        percent: u32,
        reply: Reply,
    },
    Start {
        lines: Vec<String>,
        link: Box<LinkProfile>,
        s_max: Option<f64>,
        kind: RunKind,
        /// Carga que el trabajo pone al enlace serie: si no da abasto, el registro lo avisa.
        load: Option<LinkLoad>,
        /// Punto de control para poder continuar el trabajo si se interrumpe (solo `RunKind::Job`).
        checkpoint: Option<Box<resume::CheckpointPlan>>,
        reply: Reply,
    },
    Pause(Reply),
    Resume(Reply),
    Cancel(Reply),
    Disconnect,
}

/// Mando de la conexión; se clona y se usa desde cualquier hilo. Las llamadas bloquean hasta que
/// el trabajador responde: desde Tauri se invocan dentro de `spawn_blocking`.
#[derive(Clone)]
pub struct Handle {
    pub(super) tx: Sender<Request>,
    pub(super) shared: Arc<Shared>,
    /// Mando del emulador si la conexión es una máquina virtual.
    pub(super) emu: Option<tlaku_emu::Handle>,
}

impl Handle {
    fn call(&self, build: impl FnOnce(Reply) -> Request) -> Result<Vec<String>, String> {
        if self.is_closed() {
            return Err(not_connected());
        }
        let (tx, rx) = mpsc::channel();
        self.tx.send(build(tx)).map_err(|_| not_connected())?;
        rx.recv()
            .map_err(|_| tr!("desktop.machine.connection_lost"))?
    }

    /// Fija el área de la cama [ancho, alto] (ejes de la máquina) con la que el trabajador
    /// revisa el movimiento manual. `None` = que valgan `$130`/`$131`.
    pub fn set_bed(&self, bed: Option<[f64; 2]>) {
        if let Ok(mut g) = self.shared.bed.lock() {
            *g = bed;
        }
    }

    /// `true` si la conexión terminó (a petición o por un fallo).
    pub fn is_closed(&self) -> bool {
        self.shared.dead.load(Ordering::SeqCst)
    }

    /// `true` si la conexión es una máquina virtual (nada se graba de verdad).
    pub fn is_virtual(&self) -> bool {
        self.emu.is_some()
    }

    /// Provoca una falla en la máquina virtual. Con una máquina real no hace nada y avisa.
    pub fn simulate_fault(&self, fault: SimulatedFault) -> Result<(), String> {
        if self.is_closed() {
            return Err(not_connected());
        }
        match &self.emu {
            Some(emu) if self.is_virtual() => {
                virtual_port::inject(emu, fault);
                Ok(())
            }
            _ => Err(tr!("desktop.machine.simulate_needs_virtual")),
        }
    }

    /// Posición de trabajo [x, y] del último reporte de estado.
    pub fn work_pos(&self) -> Option<[f64; 2]> {
        self.shared.status.lock().ok()?.as_ref().map(|s| s.work_pos)
    }

    /// Último reporte de estado completo (para la automatización: `STATUS`).
    pub fn status_report(&self) -> Option<StatusReport> {
        self.shared.status.lock().ok()?.clone()
    }

    /// Envía una línea y devuelve la respuesta, que termina en `ok`, `error:N` o `ALARM:N`.
    pub fn command(&self, line: &str) -> Result<Vec<String>, String> {
        self.call(|reply| Request::Command {
            line: line.into(),
            reply,
        })
    }

    pub fn realtime(&self, cmd: RealtimeCommand) -> Result<(), String> {
        self.call(|reply| Request::Realtime { cmd, reply })
            .map(drop)
    }

    /// Movimiento relativo en mm a `speed` mm/min.
    pub fn jog(&self, dx: f64, dy: f64, speed: f64) -> Result<(), String> {
        self.call(|reply| Request::Jog {
            dx,
            dy,
            speed,
            reply,
        })
        .map(drop)
    }

    /// Movimiento absoluto a (`x`, `y`) en coordenadas de trabajo (las de `work_pos`), en mm, a
    /// `speed` mm/min. Es un movimiento de posicionamiento: el láser no se enciende.
    pub fn jog_to(&self, x: f64, y: f64, speed: f64) -> Result<(), String> {
        self.call(|reply| Request::JogTo { x, y, speed, reply })
            .map(drop)
    }

    /// Un toque de sonda: el cabezal avanza `(dx, dy)` mm de trabajo como máximo a `feed` mm/min
    /// con el láser apagado, se detiene al tocar y retrocede `retract` mm. Solo GRBL en reposo.
    /// Los límites (recorrido, velocidad y retroceso) se comprueban aquí antes de enviar nada.
    pub fn probe(
        &self,
        dx: f64,
        dy: f64,
        feed: f64,
        retract: f64,
    ) -> Result<probe::ProbeTouch, String> {
        probe::check_args(dx, dy, feed, retract)?;
        let reply = self.call(|reply| Request::Probe {
            dx,
            dy,
            feed,
            retract,
            reply,
        })?;
        probe::decode(&reply).ok_or_else(|| tr!("desktop.worker.probe_no_prb"))
    }

    /// Lleva el override de `kind` (velocidad o potencia) a `percent` por ciento (10 a 200).
    /// Solo GRBL: Marlin y Smoothieware no tienen bytes en tiempo real. Vale con o sin trabajo.
    pub fn set_override(&self, kind: OverrideKind, percent: u32) -> Result<(), String> {
        self.call(|reply| Request::Override {
            kind,
            percent,
            reply,
        })
        .map(drop)
    }

    /// Comprueba el estado de la máquina y, solo si está en reposo y en modo láser (`$32=1`),
    /// transmite el G-code. No compara `$30` con la S máx del perfil; para eso usa
    /// [`start_job_for`](Self::start_job_for).
    #[cfg(test)]
    pub fn start_job(&self, lines: Vec<String>, rx_buffer: usize) -> Result<(), String> {
        self.start_job_for(lines, rx_buffer, None)
    }

    /// Como [`start_job`](Self::start_job), con la S máx del perfil: si `$30` de la controladora
    /// es distinto se avisa en el registro (la potencia saldría escalada).
    #[cfg(test)]
    pub fn start_job_for(
        &self,
        lines: Vec<String>,
        rx_buffer: usize,
        s_max: Option<f64>,
    ) -> Result<(), String> {
        self.start_job_link(lines, LinkProfile::grbl_with_buffer(rx_buffer), s_max)
    }

    /// Como [`start_job_for`](Self::start_job_for) con el enlace del perfil de la máquina
    /// ([`LinkProfile::for_device`]): GRBL con conteo de caracteres, o Marlin y Smoothieware con
    /// una línea por `ok`. Debe coincidir con la familia de la controladora conectada.
    #[cfg(test)]
    pub fn start_job_link(
        &self,
        lines: Vec<String>,
        link: LinkProfile,
        s_max: Option<f64>,
    ) -> Result<(), String> {
        self.start_job_with_load(lines, link, s_max, None)
    }

    /// Como [`start_job_link`](Self::start_job_link) con la carga del trabajo sobre el enlace
    /// serie: si supera lo que deja pasar el baudio, la comprobación previa lo avisa (no bloquea).
    #[cfg(test)]
    pub fn start_job_with_load(
        &self,
        lines: Vec<String>,
        link: LinkProfile,
        s_max: Option<f64>,
        load: Option<LinkLoad>,
    ) -> Result<(), String> {
        self.start(lines, link, s_max, RunKind::Job, load, None)
    }

    /// Como [`start_job_with_load`](Self::start_job_with_load), llevando el punto de control con
    /// el que se podrá continuar el trabajo si se interrumpe (ver [`resume`]).
    pub fn start_job_checkpointed(
        &self,
        lines: Vec<String>,
        link: LinkProfile,
        s_max: Option<f64>,
        load: Option<LinkLoad>,
        plan: Option<resume::CheckpointPlan>,
    ) -> Result<(), String> {
        self.start(lines, link, s_max, RunKind::Job, load, plan.map(Box::new))
    }

    /// Hay un trabajo transmitiéndose: su punto de control no es de uno «interrumpido».
    pub fn is_running(&self) -> bool {
        self.shared.running.load(Ordering::SeqCst)
    }

    /// Memoria de la comparación del punto de control (ver `resume::start::current_for`).
    pub fn resume_baseline(&self) -> &Mutex<Option<(String, Option<resume::assess::Now>)>> {
        &self.shared.resume_baseline
    }

    /// Hay un trabajo o un encuadre transmitiéndose.
    pub fn is_streaming(&self) -> bool {
        self.shared.streaming.load(Ordering::SeqCst)
    }

    /// Desde que se abrió la conexión se hizo algo que mueve la cabeza (homing, jog, encuadre,
    /// otro trabajo, ir a una posición, una línea de G-code a mano…). La marca no se quita.
    pub fn position_altered(&self) -> bool {
        self.shared.moved.load(Ordering::SeqCst)
    }

    /// Se hizo homing y la posición no se ha perdido desde entonces.
    pub fn is_homed(&self) -> bool {
        self.shared.homed.load(Ordering::SeqCst)
    }

    /// Como [`start_job`](Self::start_job) para el encuadre (sin eventos de progreso). El
    /// encuadre va con el láser apagado (M5, S0), así que no exige el modo láser.
    #[cfg(test)]
    pub fn frame(&self, lines: Vec<String>, rx_buffer: usize) -> Result<(), String> {
        self.frame_link(lines, LinkProfile::grbl_with_buffer(rx_buffer))
    }

    /// Como [`frame`](Self::frame) con el enlace del perfil de la máquina.
    pub fn frame_link(&self, lines: Vec<String>, link: LinkProfile) -> Result<(), String> {
        self.start(lines, link, None, RunKind::Frame, None, None)
    }

    fn start(
        &self,
        lines: Vec<String>,
        link: LinkProfile,
        s_max: Option<f64>,
        kind: RunKind,
        load: Option<LinkLoad>,
        checkpoint: Option<Box<resume::CheckpointPlan>>,
    ) -> Result<(), String> {
        self.call(|reply| Request::Start {
            lines,
            link: Box::new(link),
            s_max,
            kind,
            load,
            checkpoint,
            reply,
        })
        .map(drop)
    }

    pub fn pause(&self) -> Result<(), String> {
        self.call(Request::Pause).map(drop)
    }

    pub fn resume(&self) -> Result<(), String> {
        self.call(Request::Resume).map(drop)
    }

    /// Cancela el trabajo (feed hold y soft reset). Sin trabajo, cancela un jog (0x85).
    pub fn cancel(&self) -> Result<(), String> {
        self.call(Request::Cancel).map(drop)
    }
}
