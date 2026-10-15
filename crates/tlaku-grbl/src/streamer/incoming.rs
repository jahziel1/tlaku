//! Interpretación de las líneas que llegan de la controladora durante el trabajo.

use std::io;
use std::time::{Duration, Instant};

use crate::link::LinkFamily;
use crate::serial::Transport;

use super::*;

impl<T: Transport> Streamer<T> {
    pub(super) fn handle_line(
        &mut self,
        line: &str,
        job: &[Prepared],
        st: &mut RunState,
        emit: &mut dyn FnMut(Event),
    ) -> io::Result<Option<Outcome>> {
        // Las líneas propias del streamer (`M110 N0`) no cuentan en el avance.
        let hidden = st.hidden;
        let progress = |st: &RunState| Event::Progress {
            acked: st.acked.saturating_sub(hidden),
            sent: st.next.saturating_sub(hidden),
            total: job.len() - hidden,
            buffer_used: st.used,
        };
        if self.link.is_ack(line) {
            // El `ok` que acompaña a un `Resend:` no confirma ninguna línea.
            if st.resend_until.take().is_some() {
                return Ok(None);
            }
            st.resends = 0;
            if let Some((idx, len)) = st.pending.pop_front() {
                st.used -= len;
                if idx != EXTRA {
                    st.acked += 1;
                    if !job[idx].silent {
                        emit(progress(st));
                    }
                }
            }
        } else if self.link.family == LinkFamily::Marlin
            && let Some(outcome) = self.handle_marlin_line(line, job, st, emit)
        {
            return Ok(outcome);
        } else if let Some(code) = line.strip_prefix("error:") {
            // Minúsculas: el `error:N` de GRBL. El `ERROR:NN` de Creality va más abajo.
            let Ok(code) = code.trim().parse::<u32>() else {
                emit(Event::Message(line.to_string()));
                return Ok(None);
            };
            let Some((idx, len)) = st.pending.pop_front() else {
                emit(Event::Error {
                    code,
                    source_line: None,
                    text: None,
                });
                return Ok(None);
            };
            st.used -= len;
            if idx == EXTRA {
                // Una línea auxiliar (`M0`, `M400`...) rechazada: no es del trabajo.
                emit(Event::Error {
                    code,
                    source_line: None,
                    text: None,
                });
                return Ok(None);
            }
            st.acked += 1;
            st.errors += 1;
            let source_line = job[idx].source_line;
            emit(Event::Error {
                code,
                source_line: Some(source_line),
                text: Some(job[idx].text.clone()),
            });
            emit(progress(st));
            if self.config.error_policy == ErrorPolicy::Stop {
                // La máquina sigue ejecutando lo que ya tiene en el búfer: se detiene.
                self.stop_machine(st, emit)?;
                return Ok(Some(Outcome::LineError { code, source_line }));
            }
        } else if let Some(code) = parse_creality_error(line) {
            emit(Event::Message(format!(
                "ERROR:{code:02}: {}",
                creality_error_message(code)
            )));
            self.stop_machine(st, emit)?;
            return Ok(Some(Outcome::MachineFault(code)));
        } else if let Some(code) = line.strip_prefix("ALARM:") {
            if let Ok(code) = code.trim().parse::<u32>() {
                emit(Event::Alarm(code));
                return Ok(Some(Outcome::Alarm(code)));
            }
            emit(Event::Message(line.to_string()));
        } else if line.starts_with('<') {
            match StatusReport::parse(line) {
                Some(report) => {
                    st.last_state = Some(report.state);
                    if st.acked == job.len() && report.state == MachineState::Idle {
                        st.idle_seen = true;
                    }
                    emit(Event::Status(Box::new(report)));
                }
                None => emit(Event::Message(line.to_string())),
            }
        } else if self.link.is_reset_banner(line) {
            emit(Event::Reset(line.to_string()));
            // La placa se reinició sola: lo que se haya enviado después del reinicio no debe
            // seguir ejecutándose. Parada segura (hold y reset) y la posición se da por perdida.
            self.stop_machine(st, emit)?;
            if let Some(info) = self.stop.as_mut() {
                info.position_maybe_lost = true;
            }
            return Ok(Some(Outcome::Reset(line.to_string())));
        } else {
            emit(Event::Message(line.to_string()));
        }
        Ok(None)
    }

    /// Líneas propias de Marlin: `Resend: N` (reenvía desde la línea N) y `Error:...`.
    /// `None` = no es una de ellas; `Some(resultado)` = atendida (con el resultado de
    /// `handle_line`).
    fn handle_marlin_line(
        &mut self,
        line: &str,
        job: &[Prepared],
        st: &mut RunState,
        emit: &mut dyn FnMut(Event),
    ) -> Option<Option<Outcome>> {
        let low = line.to_ascii_lowercase();
        if let Some(rest) = low.strip_prefix("resend:") {
            let Some(n) = rest
                .trim()
                .parse::<usize>()
                .ok()
                .filter(|&n| self.link.line_numbering.is_some() && n < job.len())
            else {
                emit(Event::Message(line.to_string()));
                return Some(None);
            };
            st.resends += 1;
            if st.resends > MAX_RESENDS {
                return Some(Some(Outcome::Disconnected(tr!(
                    "grbl.stream.resend_limit",
                    n = n,
                    max = MAX_RESENDS
                ))));
            }
            emit(Event::Message(line.to_string()));
            // Marlin descarta lo que tuviera en cola desde esa línea: se vuelve a enviar todo.
            st.pending.clear();
            st.used = 0;
            st.next = n;
            st.acked = st.acked.min(n);
            // El `ok` que sigue al `Resend:` no confirma nada; si no llega, se sigue pasado un rato.
            st.resend_until = Some(Instant::now() + RESEND_WAIT);
            return Some(None);
        }
        if low.starts_with("error:") {
            // Una parada total (`M112`, `kill()`) no se recupera sin reiniciar la placa.
            if low.contains("halted") || low.contains("kill()") {
                emit(Event::Message(line.to_string()));
                return Some(Some(Outcome::Reset(line.to_string())));
            }
            // Los demás (`checksum mismatch`, `Line Number is not Last Line Number+1`) van
            // seguidos de `Resend:`; se informan sin más.
            emit(Event::Message(line.to_string()));
            return Some(None);
        }
        None
    }
}

/// Reenvíos seguidos de una línea (sin un `ok` entre medias) tras los que se abandona.
const MAX_RESENDS: u32 = 8;

/// Cuánto se espera el `ok` posterior a un `Resend:` antes de reenviar de todos modos.
const RESEND_WAIT: Duration = Duration::from_millis(500);

/// ¿Es el saludo de la controladora (`Grbl 1.1h [...]`, `GrblHAL ...`)? Compara bytes: el
/// texto puede traer `U+FFFD` por errores de transmisión (CH340, cables con ruido) y cortarlo
/// por índice de bytes haría panic dentro de un carácter.
pub fn is_greeting(line: &str) -> bool {
    line.as_bytes()
        .get(..4)
        .is_some_and(|p| p.eq_ignore_ascii_case(b"grbl"))
}

/// ¿Es una línea con la que arranca una placa GRBL? El saludo (`Grbl 1.1h [...]`, `GrblHAL`,
/// FluidNC) o el `[MSG:Using cpu_map` de Creality, que sale antes. Sirve para notar un reinicio
/// fuera de un trabajo.
pub fn is_boot_banner(line: &str) -> bool {
    is_greeting(line)
        || line
            .as_bytes()
            .get(..crate::link::CPU_MAP_BANNER.len())
            .is_some_and(|p| p.eq_ignore_ascii_case(crate::link::CPU_MAP_BANNER.as_bytes()))
}
