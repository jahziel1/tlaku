//! Comunicación con la máquina: envío de G-code, encuadre y confirmaciones.
//!
//! La lógica recibe un `Transport` ya abierto y un indicador de cancelación, de modo que se
//! prueba con `memory_pair` sin hardware.

use laser_core::Bounds;
use laser_grbl::serial::{LineReader, Transport};
use laser_grbl::streamer::{
    Event, MachineState, Outcome, Realtime, StatusReport, StreamError, Streamer, StreamerConfig,
    alarm_message, error_message, prepare_line,
};
use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Velocidad del encuadre cuando no se indica `--speed`.
pub const DEFAULT_FRAME_SPEED: f64 = 3000.0;

/// Pregunta de seguridad antes de mover la máquina.
pub const READY_PROMPT: &str = "¿El láser está listo y el área despejada? [s/N] ";

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

/// Líneas del archivo de G-code, validadas contra el búfer de la controladora antes de
/// abrir el puerto. Devuelve cuántas líneas con contenido se enviarán.
pub fn validate_gcode(text: &str, rx_buffer: usize) -> Result<usize, String> {
    let mut count = 0;
    for (i, raw) in text.lines().enumerate() {
        let source_line = i + 1;
        let line = prepare_line(raw)
            .map_err(|ch| StreamError::InvalidChar { source_line, ch }.to_string())?;
        if line.is_empty() {
            continue;
        }
        let len = line.len() + 1;
        if len > rx_buffer {
            return Err(StreamError::LineTooLong {
                source_line,
                len,
                max: rx_buffer,
            }
            .to_string());
        }
        count += 1;
    }
    if count == 0 {
        return Err("el archivo no contiene ninguna línea de G-code".into());
    }
    Ok(count)
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

    fn progress(&mut self, acked: usize, total: usize) {
        let pct = if total == 0 { 100 } else { acked * 100 / total };
        if self.tty {
            let due = self
                .last_draw
                .is_none_or(|t| t.elapsed() >= Duration::from_millis(100));
            if due || acked == total {
                let _ = write!(
                    self.out,
                    "\r  {pct:>3} %  ({acked}/{total} líneas)  estado: {}   ",
                    self.state
                );
                let _ = self.out.flush();
                self.last_draw = Some(Instant::now());
                self.drawn_inline = true;
            }
        } else if pct / 10 > self.last_step || (acked == total && self.last_step < 10) {
            self.last_step = pct / 10;
            let _ = writeln!(
                self.out,
                "  {pct:>3} %  ({acked}/{total} líneas)  estado: {}",
                self.state
            );
        }
    }

    fn status(&mut self, s: &StatusReport) {
        self.state = state_name(s.state);
    }
}

pub fn state_name(s: MachineState) -> String {
    match s {
        MachineState::Idle => "reposo".into(),
        MachineState::Run => "en marcha".into(),
        MachineState::Hold(_) => "en pausa".into(),
        MachineState::Jog => "jog".into(),
        MachineState::Alarm => "ALARMA".into(),
        MachineState::Door(_) => "puerta abierta".into(),
        MachineState::Check => "verificación".into(),
        MachineState::Home => "homing".into(),
        MachineState::Sleep => "suspendida".into(),
    }
}

/// Convierte el resultado del envío en un mensaje legible; `Ok` solo si todo salió bien.
pub fn outcome_result(outcome: &Outcome) -> Result<(), String> {
    match outcome {
        Outcome::Completed { errors: 0 } => Ok(()),
        Outcome::Completed { errors } => Err(format!("terminó con {errors} error(es)")),
        Outcome::Cancelled => Err(
            "trabajo cancelado: se envió feed hold y soft reset (el láser queda apagado)".into(),
        ),
        Outcome::LineError { code, source_line } => {
            let hint = if *code == 9 {
                "; si la máquina está en alarma, desbloquéala con $X o haz homing"
            } else {
                ""
            };
            Err(format!(
                "error:{code} en la línea {source_line} del archivo: {}{hint}",
                error_message(*code)
            ))
        }
        Outcome::Alarm(code) => Err(format!(
            "ALARM:{code}: {}. Revisa la máquina; para continuar desbloquea con $X",
            alarm_message(*code)
        )),
        Outcome::Reset(msg) => Err(format!(
            "la controladora se reinició a mitad del trabajo ({msg}); el trabajo quedó incompleto"
        )),
        Outcome::Disconnected(msg) => Err(format!("se perdió la conexión con la máquina: {msg}")),
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
        state: "iniciando".into(),
        last_draw: None,
        last_step: 0,
        drawn_inline: false,
    };
    let mut cancel_sent = false;
    loop {
        if cancel.load(Ordering::SeqCst) && !cancel_sent {
            cancel_sent = true;
            job.control.cancel();
            p.line("Cancelando: feed hold y soft reset...");
        }
        match job.events.recv_timeout(Duration::from_millis(50)) {
            Ok(Event::Progress { acked, total, .. }) => p.progress(acked, total),
            Ok(Event::Status(s)) => p.status(&s),
            Ok(Event::Error {
                code,
                source_line,
                text,
            }) => {
                let at = match (source_line, text) {
                    (Some(l), Some(t)) => format!(" en la línea {l} ({t})"),
                    _ => String::new(),
                };
                p.line(&format!("error:{code}{at}: {}", error_message(code)));
            }
            Ok(Event::Alarm(code)) => {
                p.line(&format!("ALARM:{code}: {}", alarm_message(code)));
            }
            Ok(Event::Message(m)) => p.line(&format!("máquina: {m}")),
            Ok(Event::Reset(m)) => p.line(&format!("la controladora se reinició: {m}")),
            Ok(Event::Finished(_)) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    p.end_inline();
    let (_, outcome) = job
        .join()
        .map_err(|_| "el hilo de envío terminó de forma inesperada".to_string())?;
    let result = outcome_result(&outcome);
    if result.is_ok() {
        writeln!(p.out, "Trabajo terminado.").map_err(|e| e.to_string())?;
    }
    result
}

// ---------------------------------------------------------------------------
// Comprobación del estado antes de transmitir
// ---------------------------------------------------------------------------

/// Esperas de la comprobación previa al envío.
#[derive(Debug, Clone)]
pub struct PreflightConfig {
    /// Cuánto esperar el reporte de estado (se reenvía `?` cada medio segundo).
    pub status_timeout: Duration,
    /// Cuánto esperar la respuesta a `$$` cuando la máquina está en alarma.
    pub settings_timeout: Duration,
    /// Puerto que se usa en los comandos sugeridos (`lasersoft cmd --port ...`).
    pub port: String,
}

impl Default for PreflightConfig {
    fn default() -> Self {
        Self {
            status_timeout: Duration::from_secs(3),
            settings_timeout: Duration::from_secs(2),
            port: "<puerto>".into(),
        }
    }
}

fn io_err(e: io::Error) -> String {
    format!("error al hablar con la máquina: {e}")
}

/// Envía `?` y espera el reporte de estado. Devuelve también las demás líneas recibidas
/// (por ejemplo un `ALARM:N`).
fn query_status(
    t: &mut impl Transport,
    timeout: Duration,
) -> Result<(StatusReport, Vec<String>), String> {
    t.clear_input().map_err(io_err)?;
    let deadline = Instant::now() + timeout;
    let mut next_poll = Instant::now();
    let mut reader = LineReader::default();
    let mut others = Vec::new();
    let mut buf = [0u8; 256];
    loop {
        let now = Instant::now();
        if now >= deadline {
            return Err(format!(
                "la máquina no respondió a la consulta de estado (?) en {:.0} s; revisa que sea \
                 el puerto correcto, que esté encendida y que use GRBL 1.1 (baudios: --baud)",
                timeout.as_secs_f64()
            ));
        }
        if now >= next_poll {
            t.write_all(b"?").map_err(io_err)?;
            next_poll = now + Duration::from_millis(500);
        }
        let n = t
            .read_timeout(&mut buf, Duration::from_millis(50))
            .map_err(io_err)?;
        reader.feed(&buf[..n]);
        while let Some(line) = reader.next_line() {
            match StatusReport::parse(&line) {
                Some(report) => return Ok((report, others)),
                None => others.push(line),
            }
        }
    }
}

/// Lee `$22` (homing activado) con `$$`; `None` si no responde o no aparece.
fn read_homing_setting(t: &mut impl Transport, timeout: Duration) -> Option<bool> {
    t.write_all(b"$$\n").ok()?;
    let deadline = Instant::now() + timeout;
    let mut reader = LineReader::default();
    let mut buf = [0u8; 256];
    let mut homing = None;
    while Instant::now() < deadline {
        let n = t.read_timeout(&mut buf, Duration::from_millis(50)).ok()?;
        reader.feed(&buf[..n]);
        while let Some(line) = reader.next_line() {
            if let Some(v) = line.strip_prefix("$22=") {
                homing = v.trim().parse::<f64>().ok().map(|v| v != 0.0);
            } else if line == "ok" || line.starts_with("error") {
                return homing;
            }
        }
    }
    homing
}

/// Argumento de shell para el puerto: tal cual si es seguro, entre comillas simples si no.
fn shell_arg(s: &str) -> String {
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | ':'))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// Comando exacto de `lasersoft cmd` para enviar `command` (`$X`, `$H`) a ese puerto.
pub fn cmd_hint(port: &str, command: &str) -> String {
    format!("lasersoft cmd --port {} '{command}'", shell_arg(port))
}

fn unlock_text(port: &str) -> String {
    format!(
        "$X desbloquea la alarma sin mover la máquina, pero no comprueba la posición: úsalo \
         solo si el cabezal está libre y sin obstáculos. Ejecútalo con: {}",
        cmd_hint(port, "$X")
    )
}

fn home_text(port: &str) -> String {
    format!(
        "$H hace homing (el cabezal se mueve hasta los finales de carrera) y recupera la \
         posición de origen. Ejecútalo con: {}",
        cmd_hint(port, "$H")
    )
}

/// Qué sugerir según la causa de la alarma (si se conoce) y si el homing está activado ($22).
fn alarm_advice(code: Option<u32>, homing: Option<bool>, port: &str) -> Vec<String> {
    let line = |s: &str| format!("  - {s}");
    let (unlock, home) = (unlock_text(port), home_text(port));
    let (unlock, home) = (unlock.as_str(), home.as_str());
    match (code, homing) {
        (Some(4 | 5), _) => vec![line(unlock)],
        (Some(6..=9), _) => vec![
            line("revisa la puerta, los finales de carrera y su cableado; después repite $H"),
            line(home),
            line(unlock),
        ],
        (Some(1 | 2), Some(false)) => {
            vec![line("aparta el cabezal del límite a mano"), line(unlock)]
        }
        (Some(1 | 2), _) => vec![
            line("aparta el cabezal del límite a mano; luego $X para desbloquear"),
            line("con homing activado, haz después $H para recuperar la posición"),
        ],
        (_, Some(false)) => vec![
            line("el homing está desactivado ($22=0): lo normal es desbloquear con $X"),
            line(unlock),
        ],
        (_, Some(true)) => vec![
            line("el homing está activado ($22=1): lo normal es hacer $H"),
            line(home),
            line(unlock),
        ],
        (_, None) => vec![
            line("si tu máquina tiene finales de carrera (homing): $H"),
            line(home),
            line("si no los tiene: $X"),
            line(unlock),
        ],
    }
}

/// `Ok` solo si la máquina está en reposo; en cualquier otro caso explica por qué no se
/// transmite y qué hacer. `code` es el `ALARM:N` recibido (si lo hubo) y `homing` el valor
/// de `$22` (si se pudo leer).
pub fn check_ready(
    report: &StatusReport,
    code: Option<u32>,
    homing: Option<bool>,
    port: &str,
) -> Result<(), String> {
    let why = match report.state {
        MachineState::Idle => return Ok(()),
        MachineState::Alarm => {
            let mut msg = String::from(
                "la máquina está en ALARMA: GRBL no acepta G-code hasta que se resuelva. \
                 No se transmitió nada.",
            );
            if let Some(c) = code {
                msg.push_str(&format!("\nCausa: ALARM:{c}, {}.", alarm_message(c)));
            }
            msg.push_str(
                "\nQué hacer (send nunca ejecuta estos comandos por su cuenta; lasersoft cmd \
                 pide confirmación antes de enviarlos):\n",
            );
            msg.push_str(&alarm_advice(code, homing, port).join("\n"));
            return Err(msg);
        }
        MachineState::Hold(_) => {
            "está en pausa (Hold): hay un movimiento detenido con feed hold. Reanúdalo con ~ \
             (cycle start) o cancélalo con un soft reset (Ctrl+X) antes de enviar otro trabajo"
        }
        MachineState::Door(_) => {
            "la puerta o tapa de seguridad está abierta (o se acaba de cerrar). Ciérrala y \
             reanuda con ~; si sigue igual, haz un soft reset (Ctrl+X)"
        }
        MachineState::Run => {
            "está ejecutando un movimiento: ya hay un trabajo en curso. Espera a que termine \
             (estado en reposo) o cancélalo antes de enviar otro"
        }
        MachineState::Jog => "está en jog (movimiento manual); espera a que termine",
        MachineState::Home => "está haciendo homing; espera a que termine",
        MachineState::Check => {
            "está en modo de verificación ($C): no ejecutaría nada. Sal de ese modo con $C o \
             con un soft reset (Ctrl+X)"
        }
        MachineState::Sleep => "está suspendida (sleep); despiértala con un soft reset (Ctrl+X)",
    };
    Err(format!(
        "la máquina no está en reposo, {why}. No se transmitió nada."
    ))
}

/// Consulta el estado con `?` y solo da el visto bueno si la máquina está en reposo.
/// No envía ningún comando que cambie el estado (`$X`, `$H`, `~`...).
pub fn preflight(
    t: &mut impl Transport,
    config: &PreflightConfig,
    out: &mut impl Write,
) -> Result<(), String> {
    let (report, others) = query_status(t, config.status_timeout)?;
    let _ = writeln!(out, "Estado de la máquina: {}.", state_name(report.state));
    let code = others
        .iter()
        .find_map(|l| l.strip_prefix("ALARM:")?.trim().parse::<u32>().ok());
    let homing = (report.state == MachineState::Alarm)
        .then(|| read_homing_setting(t, config.settings_timeout))
        .flatten();
    let result = check_ready(&report, code, homing, &config.port);
    // Lo que sobre (otro reporte, el `ok` de `$$`) no debe confundir al streamer.
    let _ = t.clear_input();
    result
}

/// Comprueba que la máquina esté en reposo y, solo entonces, transmite `lines`.
pub fn send_job<T: Transport + 'static, W: Write>(
    mut transport: T,
    lines: &[String],
    config: StreamerConfig,
    preflight_config: &PreflightConfig,
    cancel: &AtomicBool,
    tty: bool,
    out: &mut W,
) -> Result<(), String> {
    preflight(&mut transport, preflight_config, out)?;
    writeln!(out, "Enviando {} líneas. Ctrl+C cancela.", lines.len()).map_err(|e| e.to_string())?;
    stream_lines(transport, lines, config, cancel, tty, out)
}

/// Configuración del streamer con el búfer indicado.
pub fn streamer_config(rx_buffer: usize) -> StreamerConfig {
    StreamerConfig {
        rx_buffer,
        ..StreamerConfig::default()
    }
}

/// Esquinas del encuadre en el orden en que se recorren, con su nombre.
pub fn frame_corners(b: Bounds) -> [(&'static str, f64, f64); 4] {
    [
        ("inferior izquierda", b.min.x, b.min.y),
        ("inferior derecha", b.max.x, b.min.y),
        ("superior derecha", b.max.x, b.max.y),
        ("superior izquierda", b.min.x, b.max.y),
    ]
}

pub fn frame_description(b: Bounds, speed_mm_min: f64) -> String {
    let mut s = String::from("Encuadre (incluye el overscan), con el láser apagado:\n");
    for (name, x, y) in frame_corners(b) {
        s.push_str(&format!("  {name:<19} X{x:.2} Y{y:.2}\n"));
    }
    s.push_str(&format!(
        "  tamaño {:.2} x {:.2} mm, velocidad {} mm/min\n",
        b.max.x - b.min.x,
        b.max.y - b.min.y,
        speed_mm_min
    ));
    s
}

/// G-code del encuadre: va en G0 a la primera esquina y recorre el rectángulo con G1 S0 bajo
/// M5, porque GRBL ignora F en G0 y así sí se respeta la velocidad pedida.
pub fn frame_gcode(b: Bounds, speed_mm_min: f64) -> Vec<String> {
    let corners = frame_corners(b);
    let mut lines = vec!["G90".to_string(), "G21".into(), "M5".into()];
    let (_, x0, y0) = corners[0];
    lines.push(format!("G0 X{} Y{}", fmt(x0), fmt(y0)));
    for (_, x, y) in corners.iter().skip(1).chain(corners.iter().take(1)) {
        lines.push(format!(
            "G1 X{} Y{} S0 F{}",
            fmt(*x),
            fmt(*y),
            fmt(speed_mm_min)
        ));
    }
    lines.push("M5".into());
    lines
}

fn fmt(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s.into()
    }
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
    pub fn warning(self) -> Option<&'static str> {
        match self {
            Self::Safe => None,
            Self::Unlock => Some(
                "$X desbloquea la alarma sin hacer home: si la máquina tiene finales de \
                 carrera y no haces $H después, la posición puede quedar desconocida y el \
                 trabajo saldría desplazado.",
            ),
            Self::Home => Some(
                "$H hace homing: la máquina se moverá hasta los finales de carrera. \
                 Despeja el área.",
            ),
            Self::Moves => Some("Este comando mueve la máquina."),
            Self::Laser => Some("Este comando enciende el láser (y puede mover la máquina)."),
            Self::Setting => Some("Este comando cambia un ajuste guardado en la controladora."),
            Self::Other => Some(
                "No es un comando de solo lectura: puede mover la máquina, encender el láser \
                 o cambiar su configuración.",
            ),
        }
    }
}

/// Un comando pedido a `lasersoft cmd`.
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

/// Interpreta un argumento de `lasersoft cmd`: nombre de tiempo real o línea de GRBL.
pub fn parse_command(arg: &str, rx_buffer: usize) -> Result<Cmd, String> {
    if let Some((name, rt)) = realtime_by_name(arg.trim()) {
        return Ok(Cmd::Realtime { name, rt });
    }
    let text = prepare_line(arg)
        .map_err(|ch| {
            format!(
                "'{arg}' contiene '{}', un carácter de tiempo real; usa los nombres status, \
                 hold, resume o reset",
                ch.escape_default()
            )
        })?
        .to_uppercase();
    if text.is_empty() {
        return Err(format!("'{arg}' no contiene ningún comando"));
    }
    if text.len() + 1 > rx_buffer {
        return Err(format!(
            "'{text}' no cabe en el búfer de la controladora ({rx_buffer} bytes)"
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
        writeln!(out, "Se enviará exactamente: {}", cmd.shown()).map_err(|e| e.to_string())?;
        writeln!(out, "Advertencia: {warning}").map_err(|e| e.to_string())?;
        if !confirm("¿Enviarlo? [s/N] ", input, out).map_err(|e| e.to_string())? {
            return Err("cancelado: no se envió nada a la máquina".into());
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
            return Some(Err(format!("'{text}' respondió {msg}")));
        }
        let _ = writeln!(out, "{line}");
        None
    })?;
    match end {
        Some(Ok(())) => writeln!(out, "ok").map_err(w),
        Some(Err(e)) => Err(e),
        None => Err(format!(
            "sin respuesta a '{text}' en {:.0} s; revisa la máquina (puede seguir ejecutándolo)",
            timeout.as_secs_f64()
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
            writeln!(out, "Estado: {}", state_name(r.state)).map_err(|e| e.to_string())?;
            return Ok(());
        }
    }
    Err(format!(
        "la máquina no respondió a la consulta de estado (?) en {:.0} s",
        timeout.as_secs_f64()
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
                writeln!(out, "(enviado; la máquina no devolvió su estado)").map_err(w)?;
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
                    writeln!(out, "Aviso: el láser sigue encendido hasta que envíes M5.")
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
    use laser_core::Point;
    use laser_grbl::serial::{Transport, memory_pair};
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

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
            let r = confirm(READY_PROMPT, &mut a.as_bytes(), &mut out).unwrap();
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
        assert!(String::from_utf8(out).unwrap().contains("Cancelando"));
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
