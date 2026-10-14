//! Mandos del usuario (pausa, reanudar, cancelar) y parada segura de la máquina.

use std::io;
use std::time::{Duration, Instant};

use crate::link::{CancelStyle, ControlAction, LinkFamily};
use crate::serial::Transport;

use super::*;

/// Nota cuando se detuvo con una parada rápida (Marlin, Smoothie): no hay reinicio en caliente
/// ni desaceleración controlada.
pub fn quick_stop_note() -> String {
    tr!("grbl.stream.quick_stop_note")
}

/// Silencio tras el que, al cancelar en Marlin o Smoothie, se sigue con la siguiente orden de
/// la secuencia aunque la anterior no haya contestado (`M112` no contesta).
const SEQUENCE_ACK_WAIT: Duration = Duration::from_millis(300);

impl<T: Transport> Streamer<T> {
    /// Detiene la máquina como corresponde al firmware: GRBL con feed hold y soft reset;
    /// Marlin y Smoothie con su secuencia de parada rápida.
    pub(super) fn stop_machine(
        &mut self,
        st: &mut RunState,
        emit: &mut dyn FnMut(Event),
    ) -> io::Result<()> {
        match self.link.cancel.clone() {
            CancelStyle::SafeStopReset => self.safe_stop(st, emit),
            CancelStyle::Sequence(actions) => self.sequence_stop(&actions, st, emit),
        }
    }

    pub(super) fn handle_command(
        &mut self,
        cmd: Command,
        st: &mut RunState,
        emit: &mut dyn FnMut(Event),
    ) -> io::Result<Option<Outcome>> {
        match cmd {
            Command::Pause if !st.paused => {
                let actions = self.link.pause.clone();
                self.run_actions(&actions, st)?;
                st.paused = true;
            }
            Command::Resume if st.paused => {
                let actions = self.link.resume.clone();
                self.run_actions(&actions, st)?;
                st.paused = false;
            }
            Command::Pause | Command::Resume => {}
            Command::Cancel => {
                self.stop_machine(st, emit)?;
                return Ok(Some(Outcome::Cancelled));
            }
            // Reset pedido a propósito por el usuario: sin esperar el fin del hold.
            Command::Realtime(Realtime::SoftReset) if self.link.family == LinkFamily::Grbl => {
                let stationary = st.last_state.is_some_and(MachineState::is_stationary);
                self.reset(st, stationary, emit)?;
                return Ok(Some(Outcome::Cancelled));
            }
            // Marlin y Smoothie no tienen reinicio en caliente: se cancela con su secuencia.
            Command::Realtime(Realtime::SoftReset) => {
                self.stop_machine(st, emit)?;
                return Ok(Some(Outcome::Cancelled));
            }
            // Overrides y demás bytes son de GRBL: en otro firmware serían basura en la línea.
            Command::Realtime(_) if self.link.family != LinkFamily::Grbl => {}
            Command::Realtime(r) => self.transport.write_all(&[r.byte()])?,
        }
        Ok(None)
    }

    /// Parada de Marlin y Smoothie: se deja de enviar, se descarta lo pendiente y se manda la
    /// secuencia del enlace (`M410` + apagado del láser; `M112` + `M999` + apagado). Tras cada
    /// línea se espera un `ok` (el de lo pendiente, el suyo o ninguno si el firmware no
    /// contesta, como tras `M112`): como mucho [`SEQUENCE_ACK_WAIT`] de silencio, y en total
    /// [`StreamerConfig::hold_timeout`].
    ///
    /// Como no hay reinicio en caliente y las órdenes son líneas encoladas, el láser se apaga
    /// con el comando del dialecto, no al detenerse el movimiento. La posición se da por
    /// perdida: la parada rápida frena sin desaceleración.
    fn sequence_stop(
        &mut self,
        actions: &[ControlAction],
        st: &mut RunState,
        emit: &mut dyn FnMut(Event),
    ) -> io::Result<()> {
        st.pending.clear();
        st.used = 0;
        st.last_state = None;
        let deadline = Instant::now() + self.config.hold_timeout;
        let mut buf = [0u8; 256];
        let mut waiting = false;
        let mut quiet_since = Instant::now();
        let mut acked = false;
        // Un paso más que las acciones: tras la última línea también se espera su respuesta.
        for step in 0..=actions.len() {
            if waiting {
                // Espera un `ok` (o silencio) antes de mandar la siguiente.
                while !acked
                    && Instant::now() < deadline
                    && quiet_since.elapsed() < SEQUENCE_ACK_WAIT
                {
                    let Ok(n) = self.transport.read_timeout(&mut buf, TICK) else {
                        break;
                    };
                    if n > 0 {
                        quiet_since = Instant::now();
                    }
                    self.reader.feed(&buf[..n]);
                    while let Some(line) = self.reader.next_line() {
                        if self.link.is_ack(&line) {
                            acked = true;
                        } else {
                            emit(Event::Message(line));
                        }
                    }
                }
            }
            let Some(a) = actions.get(step) else { break };
            match a {
                ControlAction::Realtime(b) => self.transport.write_all(&[*b])?,
                ControlAction::Line(l) => {
                    self.transport.write_all(format!("{l}\n").as_bytes())?;
                    waiting = true;
                    acked = false;
                    quiet_since = Instant::now();
                }
                ControlAction::HostSide => {}
            }
        }
        self.reader.clear();
        let _ = self.transport.clear_input();
        emit(Event::Message(format!("; {}", quick_stop_note())));
        self.stop = Some(StopInfo {
            settled: false,
            position_maybe_lost: true,
            alarm: None,
        });
        Ok(())
    }

    /// Detiene la máquina sin perder la posición: feed hold, consulta `?` hasta ver que
    /// frenó (`Hold:0`, reposo...) con un tope de [`StreamerConfig::hold_timeout`] y solo
    /// entonces soft reset. Un reset mientras aún frena (`Hold:1`) lanza `ALARM:3`. Después
    /// del reset se lee el saludo y cualquier alarma en lugar de descartarlos.
    ///
    /// grblHAL repite el último `error:N` en las líneas siguientes (y Creality puede seguir
    /// hablando): todo lo que llega durante la espera se absorbe sin contarlo como fallo nuevo.
    fn safe_stop(&mut self, st: &mut RunState, emit: &mut dyn FnMut(Event)) -> io::Result<()> {
        self.transport.write_all(&[Realtime::FeedHold.byte()])?;
        // Los informes anteriores al hold ya no valen.
        st.last_state = None;
        // Sin sondeo no se puede saber cuándo frenó: una espera corta y a resetear.
        let polling = self.config.status_interval.is_some();
        let deadline = Instant::now()
            + if polling {
                self.config.hold_timeout
            } else {
                self.config.cancel_delay
            };
        let mut next_poll = Instant::now();
        let mut settled = false;
        let mut alarm = None;
        let mut buf = [0u8; 256];
        while !settled && Instant::now() < deadline {
            if polling && Instant::now() >= next_poll {
                self.transport.write_all(&[Realtime::StatusQuery.byte()])?;
                next_poll = Instant::now() + self.config.hold_poll_interval;
            }
            let n = self.transport.read_timeout(&mut buf, TICK)?;
            self.reader.feed(&buf[..n]);
            while let Some(line) = self.reader.next_line() {
                match absorb_line(&line, emit) {
                    Absorbed::State(state) => {
                        st.last_state = Some(state);
                        settled |= state.is_stationary();
                    }
                    Absorbed::Alarm(code) => {
                        alarm = Some(code);
                        // Una alarma ya detuvo la máquina.
                        settled = true;
                    }
                    Absorbed::Greeting => settled = true,
                    Absorbed::Other => {}
                }
            }
        }
        self.reset(st, settled, emit)?;
        if let Some(info) = self.stop.as_mut()
            && info.alarm.is_none()
        {
            info.alarm = alarm;
            info.position_maybe_lost |= alarm == Some(3);
        }
        Ok(())
    }

    /// Soft reset y lectura de lo que responde la controladora (saludo, `ALARM:3`...).
    /// `stationary` indica si se sabía la máquina quieta al resetear.
    fn reset(
        &mut self,
        st: &mut RunState,
        stationary: bool,
        emit: &mut dyn FnMut(Event),
    ) -> io::Result<()> {
        self.transport.write_all(&[Realtime::SoftReset.byte()])?;
        let started = Instant::now();
        let mut greeted: Option<Instant> = None;
        let mut alarm = None;
        let mut buf = [0u8; 256];
        loop {
            let over = match greeted {
                Some(t) => t.elapsed() >= self.config.cancel_delay,
                None => started.elapsed() >= self.config.reset_wait,
            };
            if over {
                break;
            }
            // El reset ya salió: si el canal se cae ahora, el siguiente uso del puerto lo
            // dirá; la parada en sí se hizo.
            let Ok(n) = self.transport.read_timeout(&mut buf, TICK) else {
                break;
            };
            self.reader.feed(&buf[..n]);
            while let Some(line) = self.reader.next_line() {
                match absorb_line(&line, emit) {
                    Absorbed::Alarm(code) => alarm = Some(code),
                    Absorbed::Greeting => {
                        greeted.get_or_insert_with(Instant::now);
                    }
                    Absorbed::State(_) | Absorbed::Other => {}
                }
            }
        }
        // Lo que quede (otro informe, un `ok` rezagado) no debe confundirse con futuras respuestas.
        self.reader.clear();
        let _ = self.transport.clear_input();
        st.last_state = None;
        let position_maybe_lost = !stationary || alarm == Some(3);
        if position_maybe_lost {
            emit(Event::Message(format!("; {}", position_lost_note())));
        }
        self.stop = Some(StopInfo {
            settled: stationary,
            position_maybe_lost,
            alarm,
        });
        Ok(())
    }
}

/// Lo que importa de una línea recibida mientras se detiene la máquina.
enum Absorbed {
    State(MachineState),
    Alarm(u32),
    Greeting,
    Other,
}

/// Clasifica una línea recibida durante la parada, emite el evento que corresponda y
/// descarta el resto (`ok`, `error:N` repetidos de grblHAL...).
fn absorb_line(line: &str, emit: &mut dyn FnMut(Event)) -> Absorbed {
    if line == "ok" || line.starts_with("error:") {
        return Absorbed::Other;
    }
    if let Some(code) = parse_creality_error(line) {
        emit(Event::Message(format!(
            "ERROR:{code:02}: {}",
            creality_error_message(code)
        )));
        return Absorbed::Other;
    }
    if let Some(code) = line
        .strip_prefix("ALARM:")
        .and_then(|c| c.trim().parse::<u32>().ok())
    {
        emit(Event::Alarm(code));
        return Absorbed::Alarm(code);
    }
    if line.starts_with('<') {
        return match StatusReport::parse(line) {
            Some(report) => {
                let state = report.state;
                emit(Event::Status(Box::new(report)));
                Absorbed::State(state)
            }
            None => {
                emit(Event::Message(line.to_string()));
                Absorbed::Other
            }
        };
    }
    // El saludo tras nuestro propio reset es esperado: se registra como mensaje, no como
    // un reinicio inesperado.
    if is_greeting(line) {
        emit(Event::Message(line.to_string()));
        return Absorbed::Greeting;
    }
    emit(Event::Message(line.to_string()));
    Absorbed::Other
}
