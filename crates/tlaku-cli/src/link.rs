//! Comunicación con la máquina: envío de G-code, encuadre y confirmaciones.
//!
//! La lógica recibe un `Transport` ya abierto y un indicador de cancelación, de modo que se
//! prueba con `memory_pair` sin hardware.

use tlaku_core::Bounds;
use tlaku_grbl::serial::{LineReader, Transport};

// La comprobación previa y los mensajes del resultado viven en tlaku-grbl (los usa también la app).
use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
pub use tlaku_grbl::frame::{frame_corners, frame_gcode};
use tlaku_grbl::preflight::{Advice, JobExpect, preflight_checked};
pub use tlaku_grbl::preflight::{
    PreflightConfig, check_ready, cmd_hint, io_err, outcome_result, preflight, state_name,
    validate_gcode,
};
use tlaku_grbl::streamer::{
    Event, Realtime, StatusReport, StopInfo, Streamer, StreamerConfig, alarm_message,
    error_message, prepare_line,
};
use tlaku_i18n::tr;

/// Velocidad del encuadre cuando no se indica `--speed`.
pub const DEFAULT_FRAME_SPEED: f64 = 3000.0;

/// Pregunta de seguridad antes de mover la máquina.
pub fn ready_prompt() -> String {
    format!("{} ", tr!("cli.link.ready_prompt"))
}

/// Muestra `prompt` y lee una línea; solo "s", "si", "sí", "y" o "yes" confirman.
/// Sin entrada (fin de archivo) o con cualquier otra cosa, se rechaza.
pub fn confirm(prompt: &str, input: &mut impl BufRead, out: &mut impl Write) -> io::Result<bool> {
    write!(out, "{prompt}")?;
    out.flush()?;
    let mut line = String::new();
    input.read_line(&mut line)?;
    Ok(matches!(
        line.trim().to_lowercase().as_str(),
        "s" | "si" | "sí" | "y" | "yes"
    ))
}

/// Muestra el avance. En una terminal reescribe una sola línea; si no, imprime cada 10 %.
struct Progress<'a, W: Write> {
    out: &'a mut W,
    tty: bool,
    state: String,
    last_draw: Option<Instant>,
    last_step: usize,
    drawn_inline: bool,
}

impl<W: Write> Progress<'_, W> {
    fn line(&mut self, text: &str) {
        if self.drawn_inline {
            let _ = writeln!(self.out);
            self.drawn_inline = false;
        }
        let _ = writeln!(self.out, "{text}");
    }

    fn end_inline(&mut self) {
        if self.drawn_inline {
            let _ = writeln!(self.out);
            self.drawn_inline = false;
        }
    }

    fn progress_text(&self, pct: usize, acked: usize, total: usize) -> String {
        tr!(
            "cli.link.progress",
            pct = format!("{pct:>3}"),
            acked = acked,
            total = total,
            state = self.state
        )
    }

    fn progress(&mut self, acked: usize, total: usize) {
        let pct = (acked * 100).checked_div(total).unwrap_or(100);
        if self.tty {
            let due = self
                .last_draw
                .is_none_or(|t| t.elapsed() >= Duration::from_millis(100));
            if due || acked == total {
                let _ = write!(self.out, "\r  {}   ", self.progress_text(pct, acked, total));
                let _ = self.out.flush();
                self.last_draw = Some(Instant::now());
                self.drawn_inline = true;
            }
        } else if pct / 10 > self.last_step || (acked == total && self.last_step < 10) {
            self.last_step = pct / 10;
            let _ = writeln!(self.out, "  {}", self.progress_text(pct, acked, total));
        }
    }

    fn status(&mut self, s: &StatusReport) {
        self.state = state_name(s.state);
    }
}

/// Envía `lines` por el transporte con conteo de caracteres y muestra el avance en `out`.
/// Si `cancel` pasa a verdadero (Ctrl+C) se hace feed hold y soft reset.
pub fn stream_lines<T: Transport + 'static, W: Write>(
    transport: T,
    lines: &[String],
    config: StreamerConfig,
    cancel: &AtomicBool,
    tty: bool,
    out: &mut W,
) -> Result<(), String> {
    let job = Streamer::new(transport, config)
        .spawn(lines)
        .map_err(|e| e.to_string())?;
    let mut p = Progress {
        out,
        tty,
        state: tr!("cli.link.state_starting"),
        last_draw: None,
        last_step: 0,
        drawn_inline: false,
    };
    let mut cancel_sent = false;
    loop {
        if cancel.load(Ordering::SeqCst) && !cancel_sent {
            cancel_sent = true;
            job.control.cancel();
            p.line(&tr!("cli.link.cancelling"));
        }
        match job.events.recv_timeout(Duration::from_millis(50)) {
            Ok(Event::Progress { acked, total, .. }) => p.progress(acked, total),
            Ok(Event::Status(s)) => p.status(&s),
            Ok(Event::Error {
                code,
                source_line,
                text,
            }) => {
                let msg = error_message(code);
                p.line(&match (source_line, text) {
                    (Some(l), Some(t)) => {
                        tr!(
                            "cli.link.error_at_line",
                            code = code,
                            line = l,
                            text = t,
                            msg = msg
                        )
                    }
                    _ => tr!("cli.link.error", code = code, msg = msg),
                });
            }
            Ok(Event::Alarm(code)) => {
                p.line(&tr!(
                    "cli.link.alarm",
                    code = code,
                    msg = alarm_message(code)
                ));
            }
            Ok(Event::Message(m)) => p.line(&tr!("cli.link.machine_message", msg = m)),
            Ok(Event::Reset(m)) => p.line(&tr!("cli.link.controller_reset", msg = m)),
            Ok(Event::Finished(_)) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    p.end_inline();
    let (streamer, outcome) = job.join().map_err(|_| tr!("cli.link.send_thread_died"))?;
    let result = outcome_result(&outcome);
    // Si la máquina se detuvo (cancelación, error, falla de Creality), cómo quedó.
    if let Some(stop) = streamer.last_stop() {
        for note in stop_notes(stop) {
            writeln!(p.out, "{note}").map_err(|e| e.to_string())?;
        }
    }
    if result.is_ok() {
        writeln!(p.out, "{}", tr!("cli.link.job_done")).map_err(|e| e.to_string())?;
    }
    result
}

/// Líneas para el usuario sobre cómo quedó la máquina tras detenerla: si la posición puede
/// haberse perdido (y qué hacer), la alarma que llegó durante la parada o, si frenó por
/// completo antes del reset, que la posición se conserva.
pub fn stop_notes(stop: &StopInfo) -> Vec<String> {
    let mut notes = Vec::new();
    if stop.position_maybe_lost {
        notes.push(tlaku_grbl::streamer::position_lost_note());
    } else if stop.settled {
        notes.push(tr!("cli.link.settled"));
    }
    if let Some(code) = stop.alarm {
        notes.push(tr!(
            "cli.link.alarm_during_stop",
            code = code,
            msg = alarm_message(code)
        ));
    }
    notes
}

/// Comprueba que la máquina esté en reposo y, solo entonces, transmite `lines`. No compara
/// `$30` con la S máx del trabajo; para eso usa [`send_job_for`].
pub fn send_job<T: Transport + 'static, W: Write>(
    transport: T,
    lines: &[String],
    config: StreamerConfig,
    preflight_config: &PreflightConfig,
    cancel: &AtomicBool,
    tty: bool,
    out: &mut W,
) -> Result<(), String> {
    send_job_for(
        transport,
        lines,
        config,
        preflight_config,
        None,
        cancel,
        tty,
        out,
    )
}

/// Como [`send_job`], con la S máx del trabajo: además de exigir el modo láser (`$32=1`), se
/// avisa si `$30` de la controladora es distinto (la potencia saldría escalada) o si `$#` trae
/// `nan` en los desplazamientos.
#[allow(clippy::too_many_arguments)]
pub fn send_job_for<T: Transport + 'static, W: Write>(
    mut transport: T,
    lines: &[String],
    config: StreamerConfig,
    preflight_config: &PreflightConfig,
    s_max: Option<f64>,
    cancel: &AtomicBool,
    tty: bool,
    out: &mut W,
) -> Result<(), String> {
    let expect = JobExpect {
        require_laser_mode: true,
        s_max,
        check_parameters: true,
    };
    preflight_checked(
        &mut transport,
        preflight_config,
        Advice::Cli {
            port: &preflight_config.port,
        },
        &expect,
        out,
    )?;
    writeln!(out, "{}", tr!("cli.link.sending", n = lines.len())).map_err(|e| e.to_string())?;
    stream_lines(transport, lines, config, cancel, tty, out)
}

/// Configuración del streamer con el búfer indicado.
pub fn streamer_config(rx_buffer: usize) -> StreamerConfig {
    StreamerConfig {
        rx_buffer,
        ..StreamerConfig::default()
    }
}

pub fn frame_description(b: Bounds, speed_mm_min: f64) -> String {
    let mut s = tr!("cli.link.frame_header");
    s.push('\n');
    for (name, x, y) in frame_corners(b) {
        s.push_str(&format!("  {name:<19} X{x:.2} Y{y:.2}\n"));
    }
    s.push_str(&format!(
        "  {}\n",
        tr!(
            "cli.link.frame_size",
            w = format!("{:.2}", b.max.x - b.min.x),
            h = format!("{:.2}", b.max.y - b.min.y),
            speed = speed_mm_min
        )
    ));
    s
}

// ---------------------------------------------------------------------------
// `cmd`: comandos sueltos de GRBL con reglas de seguridad
// ---------------------------------------------------------------------------

/// Qué tan delicado es un comando; decide si se pide confirmación y qué se advierte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Risk {
    /// Solo lectura (o apaga el láser): se envía sin preguntar.
    Safe,
    /// `$X`: desbloquea sin hacer home.
    Unlock,
    /// `$H`: la máquina se mueve hasta los finales de carrera.
    Home,
    /// G0/G1/G2/G3, `$J=`, G28...: mueve la máquina.
    Moves,
    /// M3/M4 con potencia: enciende el láser.
    Laser,
    /// `$N=valor`: cambia un ajuste guardado en la controladora.
    Setting,
    /// Cualquier otro: no está en la lista de comandos de solo lectura.
    Other,
}

impl Risk {
    /// Advertencia que se muestra antes de confirmar; `None` si no hace falta confirmar.
    pub fn warning(self) -> Option<String> {
        match self {
            Self::Safe => None,
            Self::Unlock => Some(tr!("cli.link.risk_unlock")),
            Self::Home => Some(tr!("cli.link.risk_home")),
            Self::Moves => Some(tr!("cli.link.risk_moves")),
            Self::Laser => Some(tr!("cli.link.risk_laser")),
            Self::Setting => Some(tr!("cli.link.risk_setting")),
            Self::Other => Some(tr!("cli.link.risk_other")),
        }
    }
}

/// Un comando pedido a `tlaku cmd`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cmd {
    /// Byte de tiempo real, pedido por nombre (`status`, `hold`, `resume`, `reset`).
    Realtime { name: &'static str, rt: Realtime },
    /// Línea de GRBL en mayúsculas y sin espacios, tal como se enviará.
    Line { text: String, risk: Risk },
}

impl Cmd {
    pub fn risk(&self) -> Risk {
        match self {
            Self::Realtime { .. } => Risk::Safe,
            Self::Line { risk, .. } => *risk,
        }
    }

    /// Texto que se muestra al usuario.
    pub fn shown(&self) -> String {
        match self {
            Self::Realtime { name, rt } => format!("{name} (byte 0x{:02X})", rt.byte()),
            Self::Line { text, .. } => text.clone(),
        }
    }
}

/// Palabras G-code (letra y valor) de una línea ya sin espacios.
fn gcode_words(text: &str) -> Vec<(char, f64)> {
    let mut words = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(letter) = chars.next() {
        if !letter.is_ascii_alphabetic() {
            continue;
        }
        let mut num = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() || matches!(c, '.' | '-' | '+') {
                num.push(c);
                chars.next();
            } else {
                break;
            }
        }
        if let Ok(v) = num.parse::<f64>() {
            words.push((letter, v));
        }
    }
    words
}

/// Clasifica una línea ya normalizada (mayúsculas, sin espacios ni comentarios).
pub fn classify(text: &str) -> Risk {
    match text {
        "$I" | "$$" | "$#" | "$G" | "$N" | "M5" => return Risk::Safe,
        "$X" => return Risk::Unlock,
        "$H" => return Risk::Home,
        _ => {}
    }
    if text.starts_with("$J=") {
        return Risk::Moves;
    }
    if let Some(rest) = text.strip_prefix('$') {
        let rest = rest.strip_prefix('N').unwrap_or(rest);
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        return if digits > 0 && rest[digits..].starts_with('=') {
            Risk::Setting
        } else {
            Risk::Other
        };
    }
    let words = gcode_words(text);
    let spindle_on = words
        .iter()
        .any(|&(l, v)| l == 'M' && (v == 3.0 || v == 4.0))
        && words
            .iter()
            .filter(|&&(l, _)| l == 'S')
            .all(|&(_, s)| s > 0.0);
    let moves = words.iter().any(|&(l, v)| {
        l == 'G' && (matches!(v, 0.0 | 1.0 | 2.0 | 3.0 | 28.0 | 30.0) || (38.0..39.0).contains(&v))
    });
    if spindle_on {
        Risk::Laser
    } else if moves {
        Risk::Moves
    } else {
        Risk::Other
    }
}

/// Nombre de un comando en tiempo real (`status`, `hold`, `resume`, `reset`).
fn realtime_by_name(name: &str) -> Option<(&'static str, Realtime)> {
    Some(match name.to_lowercase().as_str() {
        "status" | "?" => ("status", Realtime::StatusQuery),
        "hold" => ("hold", Realtime::FeedHold),
        "resume" => ("resume", Realtime::CycleStart),
        "reset" => ("reset", Realtime::SoftReset),
        _ => return None,
    })
}

/// Interpreta un argumento de `tlaku cmd`: nombre de tiempo real o línea de GRBL.
pub fn parse_command(arg: &str, rx_buffer: usize) -> Result<Cmd, String> {
    if let Some((name, rt)) = realtime_by_name(arg.trim()) {
        return Ok(Cmd::Realtime { name, rt });
    }
    let text = prepare_line(arg)
        .map_err(|ch| {
            tr!(
                "cli.link.realtime_char",
                arg = arg,
                ch = ch.escape_default()
            )
        })?
        .to_uppercase();
    if text.is_empty() {
        return Err(tr!("cli.link.empty_command", arg = arg));
    }
    if text.len() + 1 > rx_buffer {
        return Err(tr!(
            "cli.link.command_too_long",
            text = text,
            rx_buffer = rx_buffer
        ));
    }
    let risk = classify(&text);
    Ok(Cmd::Line { text, risk })
}

/// Pide confirmación de cada comando delicado (todas antes de enviar nada). Muestra
/// exactamente qué se enviará. Sin entrada (stdin cerrado) se rechaza.
pub fn confirm_commands(
    cmds: &[Cmd],
    yes: bool,
    input: &mut impl BufRead,
    out: &mut impl Write,
) -> Result<(), String> {
    if yes {
        return Ok(());
    }
    for cmd in cmds {
        let Some(warning) = cmd.risk().warning() else {
            continue;
        };
        writeln!(out, "{}", tr!("cli.link.will_send", cmd = cmd.shown()))
            .map_err(|e| e.to_string())?;
        writeln!(out, "{}", tr!("cli.link.warning", warning = warning))
            .map_err(|e| e.to_string())?;
        if !confirm(&format!("{} ", tr!("cli.link.send_prompt")), input, out)
            .map_err(|e| e.to_string())?
        {
            return Err(tr!("cli.link.nothing_sent"));
        }
    }
    Ok(())
}

/// Esperas de `cmd`.
#[derive(Debug, Clone, Copy)]
pub struct CmdConfig {
    /// Respuesta a un comando normal.
    pub timeout: Duration,
    /// Respuesta a `$H`: GRBL contesta `ok` al terminar el homing.
    pub home_timeout: Duration,
    /// Respuesta a `?`.
    pub status_timeout: Duration,
    /// Cuánto escuchar tras un soft reset (saludo y alarmas).
    pub reset_wait: Duration,
}

impl Default for CmdConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            home_timeout: Duration::from_secs(60),
            status_timeout: Duration::from_secs(3),
            reset_wait: Duration::from_millis(1500),
        }
    }
}

/// Línea `ALARM:N` o `error:N` traducida; `None` si la línea es otra cosa.
fn translate(line: &str) -> Option<String> {
    if let Some(code) = line
        .strip_prefix("error:")
        .and_then(|c| c.trim().parse().ok())
    {
        return Some(format!("error:{code}: {}", error_message(code)));
    }
    let code = line.strip_prefix("ALARM:")?.trim().parse().ok()?;
    Some(format!("ALARM:{code}: {}", alarm_message(code)))
}

/// Lee líneas hasta que `done` devuelva `Some` o venza `timeout`.
fn read_until<T>(
    t: &mut impl Transport,
    reader: &mut LineReader,
    timeout: Duration,
    mut on_line: impl FnMut(String) -> Option<T>,
) -> Result<Option<T>, String> {
    let deadline = Instant::now() + timeout;
    let mut buf = [0u8; 256];
    loop {
        while let Some(line) = reader.next_line() {
            if let Some(r) = on_line(line) {
                return Ok(Some(r));
            }
        }
        let now = Instant::now();
        if now >= deadline {
            return Ok(None);
        }
        let wait = (deadline - now).min(Duration::from_millis(50));
        let n = t.read_timeout(&mut buf, wait).map_err(io_err)?;
        reader.feed(&buf[..n]);
    }
}

/// Envía una línea y muestra la respuesta completa hasta `ok`. Un `error:N` o `ALARM:N`
/// termina con el mensaje traducido.
fn exchange(
    t: &mut impl Transport,
    reader: &mut LineReader,
    text: &str,
    timeout: Duration,
    out: &mut impl Write,
) -> Result<(), String> {
    let w = |e: io::Error| e.to_string();
    writeln!(out, "> {text}").map_err(w)?;
    t.write_all(format!("{text}\n").as_bytes())
        .map_err(io_err)?;
    let end = read_until(t, reader, timeout, |line| {
        if line == "ok" {
            return Some(Ok(()));
        }
        if let Some(msg) = translate(&line) {
            return Some(Err(tr!("cli.link.command_replied", text = text, msg = msg)));
        }
        let _ = writeln!(out, "{line}");
        None
    })?;
    match end {
        Some(Ok(())) => writeln!(out, "ok").map_err(w),
        Some(Err(e)) => Err(e),
        None => Err(tr!(
            "cli.link.no_response",
            text = text,
            secs = format!("{:.0}", timeout.as_secs_f64())
        )),
    }
}

/// Pide el estado con `?` (se repite cada medio segundo) y lo muestra.
fn show_status(
    t: &mut impl Transport,
    reader: &mut LineReader,
    timeout: Duration,
    out: &mut impl Write,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        t.write_all(b"?").map_err(io_err)?;
        let step = Duration::from_millis(500).min(deadline - Instant::now());
        let found = read_until(t, reader, step, |line| {
            StatusReport::parse(&line).map(|r| (line, r))
        })?;
        if let Some((line, r)) = found {
            writeln!(out, "{line}").map_err(|e| e.to_string())?;
            writeln!(
                out,
                "{}",
                tr!("cli.link.state_line", state = state_name(r.state))
            )
            .map_err(|e| e.to_string())?;
            return Ok(());
        }
    }
    Err(tr!(
        "cli.link.status_timeout",
        secs = format!("{:.0}", timeout.as_secs_f64())
    ))
}

fn run_realtime(
    t: &mut impl Transport,
    reader: &mut LineReader,
    name: &str,
    rt: Realtime,
    config: &CmdConfig,
    out: &mut impl Write,
) -> Result<(), String> {
    let w = |e: io::Error| e.to_string();
    writeln!(out, "> {name}").map_err(w)?;
    match rt {
        Realtime::StatusQuery => show_status(t, reader, config.status_timeout, out),
        Realtime::SoftReset => {
            t.write_all(&[rt.byte()]).map_err(io_err)?;
            read_until::<()>(t, reader, config.reset_wait, |line| {
                let _ = writeln!(out, "{}", translate(&line).unwrap_or(line));
                None
            })?;
            Ok(())
        }
        _ => {
            t.write_all(&[rt.byte()]).map_err(io_err)?;
            // El reporte confirma el efecto; si no llega, la orden ya salió.
            if show_status(t, reader, config.status_timeout, out).is_err() {
                writeln!(out, "{}", tr!("cli.link.sent_no_state")).map_err(w)?;
            }
            Ok(())
        }
    }
}

/// Ejecuta los comandos en orden y muestra la respuesta de cada uno. Se detiene en el
/// primer error, alarma o falta de respuesta, sin enviar los siguientes.
pub fn run_commands(
    t: &mut impl Transport,
    cmds: &[Cmd],
    config: &CmdConfig,
    out: &mut impl Write,
) -> Result<(), String> {
    t.clear_input().map_err(io_err)?;
    let mut reader = LineReader::default();
    for cmd in cmds {
        match cmd {
            Cmd::Realtime { name, rt } => {
                run_realtime(t, &mut reader, name, *rt, config, out)?;
            }
            Cmd::Line { text, risk } => {
                let timeout = if *risk == Risk::Home {
                    config.home_timeout
                } else {
                    config.timeout
                };
                exchange(t, &mut reader, text, timeout, out)?;
                if *risk == Risk::Laser {
                    writeln!(out, "{}", tr!("cli.link.laser_still_on"))
                        .map_err(|e| e.to_string())?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use tlaku_core::Point;
    use tlaku_grbl::serial::{Transport, memory_pair};
    use tlaku_grbl::streamer::Outcome;

    /// Simula una GRBL mínima: responde `ok` a cada línea y `Idle` a cada `?`.
    fn fake_device(
        mut dev: impl Transport + 'static,
        error_on: Option<usize>,
    ) -> std::thread::JoinHandle<Vec<String>> {
        std::thread::spawn(move || {
            let mut seen = Vec::new();
            let mut partial = String::new();
            let mut buf = [0u8; 256];
            let mut idle_after = 0;
            loop {
                let n = match dev.read_timeout(&mut buf, Duration::from_millis(20)) {
                    Ok(n) => n,
                    Err(_) => return seen,
                };
                for &b in &buf[..n] {
                    match b {
                        b'?' => {
                            let _ = dev.write_all(b"<Idle|MPos:0.000,0.000,0.000|FS:0,0>\r\n");
                        }
                        0x18 => seen.push("<RESET>".into()),
                        b'!' => seen.push("<HOLD>".into()),
                        b'\n' => {
                            let line = std::mem::take(&mut partial);
                            seen.push(line);
                            let reply = if error_on == Some(seen.len()) {
                                "error:20\r\n"
                            } else {
                                "ok\r\n"
                            };
                            let _ = dev.write_all(reply.as_bytes());
                            idle_after += 1;
                        }
                        c => partial.push(c as char),
                    }
                }
                let _ = idle_after;
            }
        })
    }

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn confirmation_answers() {
        let ask = |a: &str| {
            let mut out = Vec::new();
            let r = confirm(&ready_prompt(), &mut a.as_bytes(), &mut out).unwrap();
            assert!(
                String::from_utf8(out)
                    .unwrap()
                    .contains("¿El láser está listo")
            );
            r
        };
        assert!(ask("s\n"));
        assert!(ask("  Sí \n"));
        assert!(ask("yes\n"));
        assert!(!ask("n\n"));
        assert!(!ask("\n"));
        assert!(!ask(""));
        assert!(!ask("claro\n"));
    }

    #[test]
    fn gcode_validation() {
        assert_eq!(validate_gcode("G90\n; nada\n\nG1 X1", 127), Ok(2));
        assert!(
            validate_gcode("; solo comentario\n", 127)
                .unwrap_err()
                .contains("ninguna línea")
        );
        assert!(
            validate_gcode("G1 X1 ?", 127)
                .unwrap_err()
                .contains("tiempo real")
        );
        let long = format!("G1 X{}", "1".repeat(200));
        assert!(validate_gcode(&long, 127).unwrap_err().contains("búfer"));
    }

    #[test]
    fn frame_lines_are_laser_off_and_closed() {
        let b = Bounds {
            min: Point::new(10.0, 20.0),
            max: Point::new(30.0, 25.5),
        };
        let g = frame_gcode(b, 1500.0);
        assert_eq!(g[2], "M5");
        assert_eq!(g[3], "G0 X10 Y20");
        assert_eq!(g[4], "G1 X30 Y20 S0 F1500");
        assert_eq!(g[5], "G1 X30 Y25.5 S0 F1500");
        assert_eq!(g[6], "G1 X10 Y25.5 S0 F1500");
        assert_eq!(g[7], "G1 X10 Y20 S0 F1500");
        assert!(g.iter().all(|l| !l.contains("M3") && !l.contains("M4")));
        let d = frame_description(b, 1500.0);
        assert!(d.contains("inferior izquierda") && d.contains("X30.00 Y25.50"));
    }

    #[test]
    fn streams_a_job_through_memory_transport() {
        let (host, dev) = memory_pair();
        let device = fake_device(dev, None);
        let cancel = AtomicBool::new(false);
        let mut out = Vec::new();
        let l = lines(&["G90", "G21", "M5", "G0 X1 Y1", "G1 X2 Y2 S0 F100"]);
        stream_lines(host, &l, streamer_config(127), &cancel, false, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.contains("100 %") && text.contains("Trabajo terminado"),
            "{text}"
        );
        let seen = device.join().unwrap();
        // GRBL recibe las líneas sin espacios.
        let stripped: Vec<String> = l.iter().map(|s| s.replace(' ', "")).collect();
        assert_eq!(seen, stripped);
    }

    #[test]
    fn line_error_is_reported_readably() {
        let (host, dev) = memory_pair();
        let device = fake_device(dev, Some(2));
        let cancel = AtomicBool::new(false);
        let mut out = Vec::new();
        let l = lines(&["G90", "G99", "M5"]);
        let e = stream_lines(host, &l, streamer_config(127), &cancel, false, &mut out).unwrap_err();
        assert!(e.contains("error:20") && e.contains("línea 2"), "{e}");
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("error:20 en la línea 2 (G99)"), "{text}");
        drop(device);
    }

    #[test]
    fn ctrl_c_flag_sends_hold_and_reset() {
        let (host, dev) = memory_pair();
        // Un dispositivo mudo: nunca responde `ok`, así que el trabajo queda en curso.
        let device = std::thread::spawn(move || {
            let mut dev = dev;
            let mut seen = Vec::new();
            let mut buf = [0u8; 256];
            let end = Instant::now() + Duration::from_secs(3);
            while Instant::now() < end {
                if let Ok(n) = dev.read_timeout(&mut buf, Duration::from_millis(20)) {
                    seen.extend_from_slice(&buf[..n]);
                    if seen.contains(&0x18) {
                        break;
                    }
                }
            }
            seen
        });
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancel);
        let trigger = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            flag.store(true, Ordering::SeqCst);
        });
        let mut out = Vec::new();
        let l = lines(&["G90", "G1 X5 Y5 S0 F100"]);
        let cfg = StreamerConfig {
            status_interval: None,
            ..streamer_config(127)
        };
        let e = stream_lines(host, &l, cfg, &cancel, false, &mut out).unwrap_err();
        assert!(e.contains("cancelado"), "{e}");
        trigger.join().unwrap();
        let seen = device.join().unwrap();
        let hold = seen.iter().position(|&b| b == b'!').expect("sin feed hold");
        let reset = seen
            .iter()
            .position(|&b| b == 0x18)
            .expect("sin soft reset");
        assert!(hold < reset);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("Cancelando"), "{text}");
        // Sin ver la máquina quieta antes del reset, avisa de que la posición puede haberse
        // perdido y qué hacer.
        assert!(text.contains("posición puede haberse perdido"), "{text}");
        assert!(text.contains("homing"), "{text}");
    }

    #[test]
    fn stop_notes_say_what_happened_to_the_machine() {
        let lost = StopInfo {
            settled: false,
            position_maybe_lost: true,
            alarm: Some(3),
        };
        let n = stop_notes(&lost);
        assert_eq!(n.len(), 2);
        assert!(n[0].contains("posición puede haberse perdido"), "{n:?}");
        assert!(
            n[1].contains("ALARM:3") && n[1].contains("perdió la posición"),
            "{n:?}"
        );

        let calm = StopInfo {
            settled: true,
            position_maybe_lost: false,
            alarm: None,
        };
        let n = stop_notes(&calm);
        assert_eq!(n.len(), 1);
        assert!(n[0].contains("la posición se conserva"), "{n:?}");
    }

    /// GRBL mínima que contesta `?` y `$$` (con `$32=1` y el `$30` dado) y `ok` a lo demás.
    fn device_with_settings(
        mut dev: impl Transport + 'static,
        s30: u32,
    ) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            let mut partial = String::new();
            let mut buf = [0u8; 256];
            while let Ok(n) = dev.read_timeout(&mut buf, Duration::from_millis(20)) {
                for &b in &buf[..n] {
                    match b {
                        b'?' => {
                            let _ = dev.write_all(b"<Idle|MPos:0.000,0.000,0.000|FS:0,0>\r\n");
                        }
                        b'\n' => {
                            let line = std::mem::take(&mut partial);
                            let reply = match line.as_str() {
                                "$$" => format!("$30={s30}\r\n$32=1\r\nok\r\n"),
                                "$#" => "[G54:0.000,0.000,0.000]\r\nok\r\n".to_string(),
                                _ => "ok\r\n".to_string(),
                            };
                            let _ = dev.write_all(reply.as_bytes());
                        }
                        c => partial.push(c as char),
                    }
                }
            }
        })
    }

    #[test]
    fn send_job_for_warns_when_s_max_differs_from_the_controller() {
        let (host, dev) = memory_pair();
        let device = device_with_settings(dev, 1000);
        let cancel = AtomicBool::new(false);
        let mut out = Vec::new();
        let l = lines(&["G90", "G1 X2 Y2 S0 F100"]);
        send_job_for(
            host,
            &l,
            streamer_config(127),
            &PreflightConfig::default(),
            Some(255.0),
            &cancel,
            false,
            &mut out,
        )
        .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.contains("Aviso: $30=1000") && text.contains("S máx = 255"),
            "{text}"
        );
        assert!(text.contains("Trabajo terminado"), "{text}");
        drop(device);
    }

    #[test]
    fn send_job_for_is_quiet_when_s_max_matches() {
        let (host, dev) = memory_pair();
        let device = device_with_settings(dev, 255);
        let cancel = AtomicBool::new(false);
        let mut out = Vec::new();
        let l = lines(&["G90", "G1 X2 Y2 S0 F100"]);
        send_job_for(
            host,
            &l,
            streamer_config(127),
            &PreflightConfig::default(),
            Some(255.0),
            &cancel,
            false,
            &mut out,
        )
        .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(!text.contains("Aviso"), "{text}");
        drop(device);
    }

    fn report(line: &str) -> StatusReport {
        StatusReport::parse(line).unwrap()
    }

    #[test]
    fn check_ready_only_accepts_idle() {
        assert!(check_ready(&report("<Idle|MPos:0,0,0>"), None, None, "/dev/x").is_ok());
        for line in [
            "<Run|MPos:0,0,0>",
            "<Hold:0|MPos:0,0,0>",
            "<Door:1|MPos:0,0,0>",
            "<Jog|MPos:0,0,0>",
            "<Home|MPos:0,0,0>",
            "<Check|MPos:0,0,0>",
            "<Sleep|MPos:0,0,0>",
        ] {
            let e = check_ready(&report(line), None, None, "/dev/x").unwrap_err();
            assert!(e.contains("No se transmitió nada"), "{line}: {e}");
        }
    }

    #[test]
    fn alarm_message_uses_cause_and_homing() {
        let alarm = report("<Alarm|MPos:0,0,0>");
        let e = check_ready(&alarm, Some(1), Some(true), "/dev/x").unwrap_err();
        assert!(e.contains("ALARM:1") && e.contains("límite físico"), "{e}");
        assert!(e.contains("aparta el cabezal") && e.contains("$H"), "{e}");
        // Fallo de sonda: solo tiene sentido desbloquear.
        let e = check_ready(&alarm, Some(5), Some(true), "/dev/x").unwrap_err();
        assert!(e.contains("$X") && !e.contains("$H"), "{e}");
        let e = check_ready(&alarm, Some(9), None, "/dev/x").unwrap_err();
        assert!(
            e.contains("finales de carrera") && e.contains("repite $H"),
            "{e}"
        );
    }

    #[test]
    fn alarm_and_disconnect_messages() {
        let e = outcome_result(&Outcome::Alarm(1)).unwrap_err();
        assert!(e.contains("ALARM:1") && e.contains("límite físico"));
        let e = outcome_result(&Outcome::LineError {
            code: 9,
            source_line: 3,
        })
        .unwrap_err();
        assert!(e.contains("$X"));
        assert!(outcome_result(&Outcome::Disconnected("x".into())).is_err());
        assert!(outcome_result(&Outcome::Completed { errors: 0 }).is_ok());
    }
}
