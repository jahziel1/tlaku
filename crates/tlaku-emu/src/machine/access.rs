//! Construcción de la máquina y consultas (posición, estado, contadores, salida).

use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

use crate::model::{Gc, StateKind};
use crate::profile::{Profile, Setting};
use crate::trace::{LaserTrace, MoveKind, SpindleMode};

use super::*;

impl Machine {
    pub fn new(profile: Profile, opts: Options) -> Self {
        let settings: BTreeMap<u32, Setting> =
            profile.settings.iter().map(|s| (s.id, s.clone())).collect();
        let pos = opts.start_position;
        let line_ending = profile.line_ending;
        let mut m = Self {
            profile,
            now: Duration::ZERO,
            out: VecDeque::new(),
            held: Vec::new(),
            silence: None,
            connected: true,
            line_ending,
            rx: VecDeque::new(),
            line_buf: Vec::new(),
            line_bytes: 0,
            in_flight: 0,
            max_in_flight: 0,
            overflow_bytes: 0,
            log: Vec::new(),
            lines_seen: 0,
            line_ctx: LineCtx {
                bytes: 0,
                reply: Reply::Normal,
            },
            next_reply: None,
            last_error: None,
            settings,
            user_info: String::new(),
            startup: [String::new(), String::new()],
            wcs: [[0.0; 3]; 6],
            g28: [0.0; 3],
            g30: [0.0; 3],
            g92: [0.0; 3],
            gc: Gc::new(pos),
            alarm: None,
            hold: None,
            door: None,
            door_open: false,
            door_resume_at: None,
            check: false,
            sleeping: false,
            homing: None,
            halted: false,
            tool_pending: false,
            limit_lock: false,
            position_lost: false,
            homed: opts.homed,
            jog_cancel: false,
            block_done: false,
            resets: 0,
            pos,
            v: 0.0,
            planner: VecDeque::new(),
            current: None,
            block_seq: 0,
            pending: None,
            exec_spindle: (SpindleMode::Off, 0.0),
            spindle_stop_ovr: false,
            ovr_feed: 100,
            ovr_rapid: 100,
            ovr_spindle: 100,
            trace: LaserTrace::default(),
            open_seg: None,
            rep: ReportState::default(),
            faults: Vec::new(),
            nan: None,
            fans: BTreeMap::new(),
            inline: false,
            last_line: 0,
            corrupt_next: false,
            ep_buf: Vec::new(),
            opts,
        };
        if m.homing_lock() && !m.opts.homed {
            m.alarm = Some(0);
        }
        if m.opts.banner_on_connect {
            let banner = m.profile.banner_connect.clone();
            for l in banner {
                m.emit_line(&l);
            }
            if m.alarm.is_some() && m.profile.unlock_message {
                m.emit_line("[MSG:'$H'|'$X' to unlock]");
            }
        }
        m
    }

    // -----------------------------------------------------------------------
    // Acceso para pruebas
    // -----------------------------------------------------------------------

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// Tiempo simulado actual de la controladora.
    pub fn now(&self) -> Duration {
        self.now
    }

    /// Posición de máquina.
    pub fn mpos(&self) -> [f64; 3] {
        self.pos
    }

    /// Posición de trabajo (MPos - WCO).
    pub fn wpos(&self) -> [f64; 3] {
        let w = self.wco();
        [self.pos[0] - w[0], self.pos[1] - w[1], self.pos[2] - w[2]]
    }

    /// Desplazamiento de coordenadas de trabajo activo.
    pub fn wco(&self) -> [f64; 3] {
        self.wco_for(self.gc.wcs)
    }

    pub(crate) fn wco_for(&self, wcs: usize) -> [f64; 3] {
        let o = self.wcs[wcs.min(5)];
        [o[0] + self.g92[0], o[1] + self.g92[1], o[2] + self.g92[2]]
    }

    pub fn state(&self) -> StateKind {
        if let Some(c) = self.alarm {
            return StateKind::Alarm(c);
        }
        if self.sleeping {
            return StateKind::Sleep;
        }
        if self.homing.is_some() {
            return StateKind::Home;
        }
        if self.check {
            return StateKind::Check;
        }
        if let Some(d) = self.door {
            return StateKind::Door(d);
        }
        if self.tool_pending {
            return StateKind::Tool;
        }
        if let Some(done) = self.hold {
            return StateKind::Hold(u8::from(!done));
        }
        let jog = self.current.as_ref().map(|e| e.block.kind) == Some(MoveKind::Jog)
            || self.planner.front().map(|b| b.kind) == Some(MoveKind::Jog);
        if jog {
            return StateKind::Jog;
        }
        if self.current.is_some() || !self.planner.is_empty() {
            return StateKind::Run;
        }
        StateKind::Idle
    }

    pub fn trace(&self) -> &LaserTrace {
        &self.trace
    }

    pub fn clear_trace(&mut self) {
        self.trace.clear();
        self.open_seg = None;
    }

    /// Potencia efectiva del láser ahora (0 si está apagado).
    pub fn laser_power(&self) -> f64 {
        self.current_power()
    }

    pub(super) fn current_power(&self) -> f64 {
        self.laser_sample().map_or(0.0, |s| s.0)
    }

    pub fn laser_on(&self) -> bool {
        self.current_power() > 0.0
    }

    pub fn log(&self) -> &[LogEntry] {
        &self.log
    }

    /// Bytes que no cupieron en el búfer RX y se perdieron.
    pub fn rx_overflow_bytes(&self) -> usize {
        self.overflow_bytes
    }

    /// Mayor cantidad de bytes de líneas enviadas y sin respuesta que se llegó a tener.
    /// Con conteo de caracteres correcto nunca pasa de `rx_size - 1`.
    pub fn max_in_flight(&self) -> usize {
        self.max_in_flight
    }

    /// Bytes de líneas enviadas aún sin respuesta.
    pub fn in_flight(&self) -> usize {
        self.in_flight
    }

    /// Líneas no vacías recibidas hasta ahora.
    pub fn lines_received(&self) -> usize {
        self.lines_seen
    }

    pub fn planner_used(&self) -> usize {
        self.planner.len() + usize::from(self.current.is_some())
    }

    pub fn alarm(&self) -> Option<u32> {
        self.alarm.filter(|&c| c > 0)
    }

    /// `true` si la posición se declaró perdida (ALARM:3 o ALARM:6) y no se ha vuelto a
    /// referenciar.
    pub fn position_lost(&self) -> bool {
        self.position_lost
    }

    pub fn resets(&self) -> usize {
        self.resets
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn setting(&self, id: u32) -> f64 {
        self.settings.get(&id).map_or(0.0, |s| s.value)
    }

    pub fn set_setting(&mut self, id: u32, value: f64) {
        match self.settings.get_mut(&id) {
            Some(s) => s.value = value,
            None => {
                self.settings.insert(id, Setting::new(id, value, 0));
            }
        }
    }

    /// Cierra o abre la puerta de seguridad (`true` = cerrada).
    pub fn set_door_closed(&mut self, closed: bool) {
        if closed {
            self.door_open = false;
            if self.door == Some(1) {
                self.door = Some(0);
            }
        } else if self.door.is_none() || self.door == Some(0) {
            self.door_open = true;
            self.safety_door();
        }
    }

    /// Bytes pendientes de leer por el anfitrión.
    pub fn output_len(&self) -> usize {
        self.out.len()
    }

    /// Copia en `buf` lo que haya para el anfitrión y devuelve cuántos bytes.
    pub fn take_output(&mut self, buf: &mut [u8]) -> usize {
        let n = buf.len().min(self.out.len());
        for (slot, b) in buf.iter_mut().zip(self.out.drain(..n)) {
            *slot = b;
        }
        n
    }

    /// Descarta lo no leído (equivale a `clear_input` del anfitrión).
    pub fn clear_output(&mut self) {
        self.out.clear();
    }

    /// Todo lo pendiente como texto (para pruebas sin transporte).
    pub fn drain_output_string(&mut self) -> String {
        let v: Vec<u8> = self.out.drain(..).collect();
        String::from_utf8_lossy(&v).into_owned()
    }

    /// Reporte de estado tal como lo daría `?` ahora, sin tocar los contadores de WCO/Ov.
    pub fn status_line(&self) -> String {
        let mut tmp = self.rep;
        self.status_with(&mut tmp, false, true)
    }
}
