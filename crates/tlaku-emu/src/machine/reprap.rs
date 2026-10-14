//! Protocolo RepRap (Marlin, Smoothie): numeración, ping-pong, ventiladores, parada.

use std::collections::VecDeque;

use crate::model::{Action, Effect, Parsed, StateKind};
use crate::profile::Family;
use crate::trace::{MoveKind, SpindleMode};

use super::*;

impl Machine {
    // -----------------------------------------------------------------------
    // Protocolo RepRap (Marlin, Smoothie)
    // -----------------------------------------------------------------------

    /// Velocidad (0-255) del ventilador `idx` según el último `M106`/`M107`.
    pub fn fan_speed(&self, idx: u32) -> f64 {
        self.fans.get(&idx).copied().unwrap_or(0.0)
    }

    /// Paro total (`M112`) en curso: no contesta líneas hasta reiniciar (Smoothie: o `M999`).
    pub fn is_halted(&self) -> bool {
        self.halted
    }

    /// Modo inline de Marlin activo (`M3 S0 I`).
    pub fn is_inline(&self) -> bool {
        self.inline
    }

    /// Último número de línea aceptado (líneas numeradas de Marlin).
    pub fn last_line_number(&self) -> i64 {
        self.last_line
    }

    /// Orden sin su numeración (`N5 G1 X1*12` -> `G1X1`), en mayúsculas y sin espacios.
    fn strip_numbering(cleaned: &str) -> &str {
        let body = cleaned.split('*').next().unwrap_or("");
        match body.strip_prefix('N') {
            Some(rest) => rest.trim_start_matches(|c: char| c.is_ascii_digit()),
            None => body,
        }
    }

    /// Analizador de emergencia de Marlin: reconoce `M410`, `M112` y `M108` en cuanto llega
    /// la línea completa, sin esperar a que el intérprete llegue a ella.
    pub(super) fn emergency_scan(&mut self, b: u8) {
        if b == b'\n' || b == b'\r' {
            let line = String::from_utf8_lossy(&self.ep_buf).into_owned();
            self.ep_buf.clear();
            let cleaned = clean_line(&line);
            match Self::strip_numbering(&cleaned) {
                "M410" => self.quick_stop(),
                "M112" => self.kill(),
                "M108" => self.cycle_start(),
                _ => {}
            }
        } else if self.ep_buf.len() < MAX_RAW_LINE {
            self.ep_buf.push(b);
        }
    }

    /// `M410`: detiene los motores sin deceleración y descarta lo encolado (incluida la línea
    /// que esperaba sitio, que no recibe `ok`). **No apaga el láser**: el anfitrión manda `M5`.
    fn quick_stop(&mut self) {
        self.planner.clear();
        self.current = None;
        self.block_done = false;
        self.v = 0.0;
        self.hold = None;
        if self.pending.take().is_some() {
            self.in_flight = self.in_flight.saturating_sub(self.line_ctx.bytes);
            self.line_ctx.bytes = 0;
        }
        self.gc.position = self.pos;
        self.position_lost = true;
    }

    /// `M112`: paro total. Apaga el láser y deja de contestar hasta reiniciar (en Smoothie,
    /// hasta `M999`).
    fn kill(&mut self) {
        self.stop_motion();
        self.halted = true;
        self.alarm = Some(0);
        self.position_lost = true;
        self.line_ctx.bytes = 0;
        let msg = if self.profile.family == Family::Smoothie {
            // Inferido: Smoothie avisa con `!!` y pide reinicio o M999.
            "!! Kill button pressed - reset or M999 to continue"
        } else {
            "Error:Printer halted. kill() called!"
        };
        self.emit_line(msg);
    }

    /// Respuesta de Smoothie a `?`: `<Idle,MPos:x,y,z,WPos:x,y,z>` sin `ok` (formato por
    /// confirmar, ver [`Profile::smoothie`](crate::profile::Profile::smoothie)).
    pub(super) fn report_smoothie(&mut self) {
        let name = match self.state() {
            StateKind::Alarm(_) => "Alarm",
            StateKind::Home => "Home",
            StateKind::Run | StateKind::Jog => "Run",
            StateKind::Hold(_) | StateKind::Door(_) => "Hold",
            _ => "Idle",
        };
        let (m, w) = (self.mpos(), self.wpos());
        let line = format!(
            "<{name},MPos:{:.4},{:.4},{:.4},WPos:{:.4},{:.4},{:.4}>",
            m[0], m[1], m[2], w[0], w[1], w[2]
        );
        self.emit_line(&line);
    }

    /// Líneas numeradas de Marlin. Devuelve la línea sin su numeración si es válida o no
    /// estaba numerada; si falla contesta con `Error:` + `Resend:` + `ok` y devuelve `None`.
    fn check_numbering(&mut self, raw: &str) -> Option<String> {
        let mut line = raw.trim().to_string();
        if std::mem::take(&mut self.corrupt_next) {
            // Ruido en el cable: se altera el último byte antes del `*` (o el último).
            let at = line.rfind('*').unwrap_or(line.len()).saturating_sub(1);
            if at < line.len() && line.is_char_boundary(at) {
                let c = if line.as_bytes()[at] == b'#' {
                    "$"
                } else {
                    "#"
                };
                line.replace_range(at..=at, c);
            }
        }
        let (body, sum) = match line.rsplit_once('*') {
            Some((b, c)) => (b, Some(c)),
            None => (line.as_str(), None),
        };
        let numbered = body
            .strip_prefix('N')
            .is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit()));
        if !numbered && sum.is_none() {
            return Some(line);
        }
        let last = self.last_line;
        let fail = |m: &mut Self, why: &str| {
            m.emit_line(&format!("Error:{why}, Last Line: {last}"));
            m.emit_line(&format!("Resend: {}", last + 1));
            m.reply(Ok(()));
        };
        let Some(sum) = sum else {
            fail(self, "No Checksum with line number");
            return None;
        };
        if !numbered {
            fail(self, "No Line Number with checksum");
            return None;
        }
        let expect = body.bytes().fold(0u8, |a, b| a ^ b);
        if sum.trim().parse::<u8>().ok() != Some(expect) {
            fail(self, "checksum mismatch");
            return None;
        }
        let digits: String = body[1..].chars().take_while(char::is_ascii_digit).collect();
        let n: i64 = digits.parse().unwrap_or(-1);
        let rest = body[1 + digits.len()..].trim();
        // `M110 N<n>` fija el contador y se acepta con cualquier número.
        if !rest.to_ascii_uppercase().starts_with("M110") && n != last + 1 {
            fail(self, "Line Number is not Last Line Number+1");
            return None;
        }
        self.last_line = n;
        Some(rest.to_string())
    }

    pub(super) fn exec_pingpong(&mut self, raw: &str, text: &str) {
        let mut text = text.to_string();
        let mut raw = raw.to_string();
        if self.profile.family == Family::Marlin {
            match self.check_numbering(&raw) {
                Some(body) => {
                    text = clean_line(&body);
                    raw = body;
                }
                None => return,
            }
        }
        let raw = raw.as_str();
        let mut t = text;
        // `M3 S0 I` de Marlin: el modificador `I` no lleva valor.
        if t.ends_with('I') && (t.starts_with("M3") || t.starts_with("M4")) {
            t.pop();
            self.inline = true;
        }
        if t.starts_with("M5") {
            self.inline = false;
        }
        match t.as_str() {
            "M115" => {
                let info = self.profile.info.clone();
                self.reply_ok_with(&info);
            }
            "M114" => {
                let l = format!(
                    "X:{:.2} Y:{:.2} Z:{:.2} E:0.00 Count X: {} Y:{} Z:{}",
                    self.pos[0],
                    self.pos[1],
                    self.pos[2],
                    (self.pos[0] * 80.0).round() as i64,
                    (self.pos[1] * 80.0).round() as i64,
                    (self.pos[2] * 80.0).round() as i64,
                );
                self.reply_ok_with(&[l]);
            }
            "M400" => {
                self.pending = Some(Pending {
                    actions: VecDeque::from([Action::Sync]),
                    dwell_until: None,
                    awaiting_resume: false,
                    homing: false,
                });
            }
            // `ok` de lo que no hay que ejecutar (M110 fija el contador de líneas).
            _ if t.starts_with("M110") => {
                if let Some(n) = t.split_once('N').and_then(|(_, n)| n.parse::<i64>().ok()) {
                    self.last_line = n;
                }
                self.reply(Ok(()));
            }
            _ if t.starts_with("M105") || t.starts_with("M155") || t.starts_with("M999") => {
                self.reply(Ok(()));
            }
            "M410" => {
                // En la cola: llega aquí cuando el intérprete llega a ella.
                self.quick_stop();
                self.reply(Ok(()));
            }
            "M112" => self.kill(),
            // Sin LCD y sin `EMERGENCY_PARSER`, `M0` se ignora; con él, pausa hasta `M108`.
            "M108" => self.reply(Ok(())),
            _ if (t == "M0" || t.starts_with("M0P") || t.starts_with("M0S"))
                && !self.profile.emergency_parser =>
            {
                self.reply(Ok(()));
            }
            _ if t.starts_with("M106") || t.starts_with("M107") => self.exec_fan(&t),
            _ => match self.parse_gcode(&t) {
                Ok(mut parsed) => {
                    self.rapids_for_reprap(&mut parsed);
                    self.gc = parsed.gc;
                    for e in parsed.effects {
                        match e {
                            Effect::SetWcs(i, v) => self.wcs[i] = v,
                            Effect::SetG92(v) => self.g92 = v,
                            Effect::StoreG28(v) => self.g28 = v,
                            Effect::StoreG30(v) => self.g30 = v,
                        }
                    }
                    self.pending = Some(Pending {
                        actions: parsed.actions.into(),
                        dwell_until: None,
                        awaiting_resume: false,
                        homing: false,
                    });
                }
                Err(_) => {
                    self.emit_line(&format!("echo:Unknown command: \"{raw}\""));
                    self.reply(Ok(()));
                }
            },
        }
    }

    /// `M106 P<n> S<0-255>` y `M107 P<n>`. Con `laser_fan` el ventilador manda sobre el láser
    /// (con sincronización del planificador, como `LASER_SYNCHRONOUS_M106_M107`).
    fn exec_fan(&mut self, t: &str) {
        let Ok(words) = crate::parse::parse_words(&t[4..]) else {
            self.emit_line(&format!("echo:Unknown command: \"{t}\""));
            self.reply(Ok(()));
            return;
        };
        let get = |c: u8| words.iter().find(|w| w.0 == c).map(|w| w.1);
        let idx = get(b'P').unwrap_or(0.0).max(0.0) as u32;
        let speed = if t.starts_with("M107") {
            0.0
        } else {
            get(b'S').unwrap_or(255.0).clamp(0.0, 255.0)
        };
        self.fans.insert(idx, speed);
        if self.profile.laser_fan == Some(idx) {
            let mode = if speed > 0.0 {
                SpindleMode::Cw
            } else {
                SpindleMode::Off
            };
            self.pending = Some(Pending {
                actions: VecDeque::from([Action::Sync, Action::ApplySpindle(mode, speed)]),
                dwell_until: None,
                awaiting_resume: false,
                homing: false,
            });
        } else {
            self.reply(Ok(()));
        }
    }

    /// Particularidades de Marlin en los `G0`: comparten `F` con `G1` y, en modo estándar
    /// (`M3 S<n>` en línea aparte), apagan el láser hasta el siguiente `M3`.
    fn rapids_for_reprap(&self, parsed: &mut Parsed) {
        if self.profile.family != Family::Marlin {
            return;
        }
        let feed = parsed.gc.feed;
        let standard = self.profile.laser_fan.is_none() && !self.inline;
        let mut any = false;
        for a in &mut parsed.actions {
            if let Action::Move(b) = a
                && b.kind == MoveKind::Rapid
            {
                any = true;
                b.feed = feed;
                if standard {
                    b.spindle = SpindleMode::Off;
                }
            }
        }
        if any && standard {
            parsed.gc.spindle = SpindleMode::Off;
        }
    }
}
