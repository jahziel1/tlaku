//! [`VirtualMachine`]: la controladora emulada como `Transport` del anfitrión, y [`Handle`],
//! el mando de las pruebas para inyectar fallas y consultar el estado.

use std::io;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use tlaku_grbl::serial::Transport;

use crate::clock::{AutoClock, Clock};
use crate::fault::{Fault, Trigger};
use crate::machine::{LogEntry, Machine, Options, Received};
use crate::model::StateKind;
use crate::profile::{LineEnding, Profile};
use crate::trace::LaserTrace;

struct Inner {
    machine: Machine,
}

struct Shared {
    inner: Mutex<Inner>,
    clock: Arc<dyn Clock>,
}

impl Shared {
    /// Toma el cerrojo y deja la simulación al día con el reloj.
    fn lock(&self) -> MutexGuard<'_, Inner> {
        let mut g = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let t = self.clock.now();
        g.machine.advance_to(t);
        g
    }
}

/// Paso con el que el lector bloqueante consulta al reloj.
const WAIT_STEP: Duration = Duration::from_millis(1);

/// Controladora emulada conectada como transporte. Se crea con [`VirtualMachine::new`] o
/// [`VirtualMachine::builder`], que devuelven también el [`Handle`] de las pruebas.
pub struct VirtualMachine {
    shared: Arc<Shared>,
}

/// Constructor con las opciones de la máquina emulada.
pub struct Builder {
    profile: Profile,
    clock: Arc<dyn Clock>,
    opts: Options,
    faults: Vec<(Trigger, Fault)>,
    line_ending: Option<LineEnding>,
}

impl Builder {
    /// Reloj inyectado (por defecto [`AutoClock`]).
    pub fn clock(mut self, clock: impl Clock + 'static) -> Self {
        self.clock = Arc::new(clock);
        self
    }

    /// Si imprime el banner al crearse (por defecto sí).
    pub fn banner_on_connect(mut self, yes: bool) -> Self {
        self.opts.banner_on_connect = yes;
        self
    }

    /// Arranca referenciada y sin bloqueo de alarma aunque `$22=1`.
    pub fn homed(mut self, yes: bool) -> Self {
        self.opts.homed = yes;
        self
    }

    /// Posición de máquina inicial.
    pub fn position(mut self, p: [f64; 3]) -> Self {
        self.opts.start_position = p;
        self
    }

    /// Valor inicial de un ajuste `$N`.
    pub fn setting(mut self, id: u32, value: f64) -> Self {
        self.profile = self.profile.with_setting(id, value);
        self
    }

    /// Duración simulada de `$H`.
    pub fn homing_duration(mut self, d: Duration) -> Self {
        self.profile.homing_duration = d;
        self
    }

    /// Si `?` se contesta durante el homing.
    pub fn status_during_home(mut self, yes: bool) -> Self {
        self.profile.status_during_home = yes;
        self
    }

    /// Cambia el tamaño del búfer RX declarado (el utilizable es uno menos).
    pub fn rx_size(mut self, bytes: usize) -> Self {
        self.profile.rx_size = bytes;
        self
    }

    /// Cambia el número de bloques del planificador.
    pub fn planner_blocks(mut self, blocks: usize) -> Self {
        self.profile.planner_blocks = blocks;
        self
    }

    /// Terminador de las respuestas.
    pub fn line_ending(mut self, le: LineEnding) -> Self {
        self.line_ending = Some(le);
        self
    }

    /// Falla programada desde el arranque.
    pub fn fault(mut self, trigger: Trigger, fault: Fault) -> Self {
        self.faults.push((trigger, fault));
        self
    }

    pub fn build(mut self) -> (VirtualMachine, Handle) {
        if let Some(le) = self.line_ending {
            self.profile.line_ending = le;
        }
        let mut machine = Machine::new(self.profile, self.opts);
        for (t, f) in self.faults {
            machine.inject(t, f);
        }
        let shared = Arc::new(Shared {
            inner: Mutex::new(Inner { machine }),
            clock: self.clock,
        });
        (
            VirtualMachine {
                shared: shared.clone(),
            },
            Handle { shared },
        )
    }
}

impl VirtualMachine {
    /// Punto de partida con opciones.
    pub fn builder(profile: Profile) -> Builder {
        Builder {
            profile,
            clock: Arc::new(AutoClock::new()),
            opts: Options::default(),
            faults: Vec::new(),
            line_ending: None,
        }
    }

    /// Controladora con el reloj automático (el tiempo simulado corre mientras el anfitrión
    /// espera).
    pub fn new(profile: Profile) -> (VirtualMachine, Handle) {
        Self::builder(profile).build()
    }

    /// Otro mando sobre la misma controladora.
    pub fn handle(&self) -> Handle {
        Handle {
            shared: self.shared.clone(),
        }
    }
}

impl Transport for VirtualMachine {
    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        let mut g = self.shared.lock();
        if !g.machine.is_connected() {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "la controladora emulada se desconectó",
            ));
        }
        g.machine.receive(data);
        Ok(())
    }

    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        let mut waited = Duration::ZERO;
        loop {
            {
                let mut g = self.shared.lock();
                let n = g.machine.take_output(buf);
                if n > 0 || buf.is_empty() {
                    return Ok(n);
                }
                if !g.machine.is_connected() {
                    return Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "la controladora emulada se desconectó",
                    ));
                }
            }
            if waited >= timeout {
                return Ok(0);
            }
            let step = (timeout - waited).min(WAIT_STEP);
            self.shared.clock.host_wait(step);
            waited += step;
        }
    }

    fn clear_input(&mut self) -> io::Result<()> {
        self.shared.lock().machine.clear_output();
        Ok(())
    }
}

/// Mando de las pruebas sobre una [`VirtualMachine`]: consulta y fallas. Se puede clonar y
/// usar desde otros hilos mientras el anfitrión opera el transporte.
#[derive(Clone)]
pub struct Handle {
    shared: Arc<Shared>,
}

impl Handle {
    /// Ejecuta `f` con la máquina al día. Para consultas no cubiertas por los métodos de abajo.
    pub fn with<R>(&self, f: impl FnOnce(&mut Machine) -> R) -> R {
        let mut g = self.shared.lock();
        f(&mut g.machine)
    }

    // --- Consultas ---------------------------------------------------------

    pub fn state(&self) -> StateKind {
        self.with(|m| m.state())
    }

    /// El reporte que daría `?` ahora (sin campos periódicos como WCO).
    pub fn status_line(&self) -> String {
        self.with(|m| m.status_line())
    }

    /// Posición de máquina.
    pub fn mpos(&self) -> [f64; 3] {
        self.with(|m| m.mpos())
    }

    /// Posición de trabajo.
    pub fn wpos(&self) -> [f64; 3] {
        self.with(|m| m.wpos())
    }

    /// Tiempo simulado de la controladora.
    pub fn now(&self) -> Duration {
        self.with(|m| m.now())
    }

    /// Copia del rastro del láser.
    pub fn trace(&self) -> LaserTrace {
        self.with(|m| m.trace().clone())
    }

    pub fn clear_trace(&self) {
        self.with(Machine::clear_trace);
    }

    /// Potencia efectiva del láser en este instante (0 = apagado).
    pub fn laser_power(&self) -> f64 {
        self.with(|m| m.laser_power())
    }

    pub fn laser_on(&self) -> bool {
        self.with(|m| m.laser_on())
    }

    /// Registro de todo lo recibido (líneas y bytes en tiempo real).
    pub fn received(&self) -> Vec<LogEntry> {
        self.with(|m| m.log().to_vec())
    }

    /// Líneas recibidas tal como llegaron, sin terminador.
    pub fn received_lines(&self) -> Vec<String> {
        self.with(|m| {
            m.log()
                .iter()
                .filter_map(|e| match &e.what {
                    Received::Line(l) => Some(l.clone()),
                    Received::Realtime(_) => None,
                })
                .collect()
        })
    }

    /// Bytes en tiempo real recibidos, en orden.
    pub fn realtime_bytes(&self) -> Vec<u8> {
        self.with(|m| {
            m.log()
                .iter()
                .filter_map(|e| match e.what {
                    Received::Realtime(b) => Some(b),
                    Received::Line(_) => None,
                })
                .collect()
        })
    }

    /// Bytes perdidos por desborde del búfer RX (0 si el anfitrión respetó el conteo).
    pub fn rx_overflow_bytes(&self) -> usize {
        self.with(|m| m.rx_overflow_bytes())
    }

    /// Máximo de bytes de líneas enviadas sin respuesta que se llegó a tener.
    pub fn max_in_flight(&self) -> usize {
        self.with(|m| m.max_in_flight())
    }

    pub fn in_flight(&self) -> usize {
        self.with(|m| m.in_flight())
    }

    pub fn lines_received(&self) -> usize {
        self.with(|m| m.lines_received())
    }

    pub fn planner_used(&self) -> usize {
        self.with(|m| m.planner_used())
    }

    pub fn alarm(&self) -> Option<u32> {
        self.with(|m| m.alarm())
    }

    pub fn position_lost(&self) -> bool {
        self.with(|m| m.position_lost())
    }

    /// Cuántas veces se reinició la controladora.
    pub fn resets(&self) -> usize {
        self.with(|m| m.resets())
    }

    pub fn setting(&self, id: u32) -> f64 {
        self.with(|m| m.setting(id))
    }

    pub fn set_setting(&self, id: u32, value: f64) {
        self.with(|m| m.set_setting(id, value));
    }

    pub fn is_connected(&self) -> bool {
        self.with(|m| m.is_connected())
    }

    // --- Mando -------------------------------------------------------------

    /// Programa una falla.
    pub fn inject(&self, trigger: Trigger, fault: Fault) {
        self.with(|m| m.inject(trigger, fault));
    }

    /// Desconecta el cable ahora.
    pub fn disconnect(&self) {
        self.inject(Trigger::Now, Fault::Disconnect);
    }

    /// Cierra (`true`) o abre la puerta de seguridad.
    pub fn set_door_closed(&self, closed: bool) {
        self.with(|m| m.set_door_closed(closed));
    }

    /// Deja pasar `d` de tiempo simulado: los relojes manual y automático lo suman; el de
    /// pared duerme.
    pub fn advance(&self, d: Duration) {
        self.shared.clock.pass(d);
        drop(self.shared.lock());
    }

    /// Hace pasar tiempo en pasos de 1 ms simulado hasta que `pred` se cumpla o transcurran
    /// `timeout` simulados. Devuelve si se cumplió.
    pub fn run_until(&self, timeout: Duration, mut pred: impl FnMut(&mut Machine) -> bool) -> bool {
        let mut elapsed = Duration::ZERO;
        loop {
            if self.with(&mut pred) {
                return true;
            }
            if elapsed >= timeout {
                return false;
            }
            self.shared.clock.pass(WAIT_STEP);
            elapsed += WAIT_STEP;
        }
    }

    /// Espera (en tiempo simulado) a que el estado sea `Idle`.
    pub fn wait_idle(&self, timeout: Duration) -> bool {
        self.run_until(timeout, |m| m.state() == StateKind::Idle)
    }

    /// Espera a que el estado sea el dado.
    pub fn wait_state(&self, state: StateKind, timeout: Duration) -> bool {
        self.run_until(timeout, |m| m.state() == state)
    }
}
