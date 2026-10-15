//! Comprobación de seguridad antes de transmitir un trabajo y mensajes legibles del resultado.
//!
//! La comparten el CLI (`tlaku send`) y la app de escritorio: solo se transmite con la
//! máquina en reposo (`Idle`); en cualquier otro estado se explica qué hacer. Nunca se envía
//! `$X`, `$H` ni `~` por cuenta propia. Recibe un `Transport` ya abierto, de modo que se
//! prueba con `memory_pair` sin hardware.

use crate::link::{LinkFamily, LinkProfile};
use crate::serial::{LineReader, Transport};
use crate::streamer::{
    Firmware, MachineState, Outcome, StatusReport, StreamError, alarm_message_for,
    creality_error_message, creality_recovery, error_message_for, parse_parameter_line,
    prepare_line_with,
};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::time::{Duration, Instant};
use tlaku_i18n::tr;

/// ¿La línea de la salida del preflight es un aviso (empieza con «Aviso» o «Warning»)? Los avisos
/// no bloquean el envío; las apps los dejan en el registro. No depende del idioma activo.
pub fn is_warning_line(line: &str) -> bool {
    tlaku_i18n::Locale::ALL.iter().any(|l| {
        line.starts_with(&tlaku_i18n::translate_in(
            *l,
            "grbl.preflight.warning_prefix",
            &[],
        ))
    })
}

pub fn state_name(s: MachineState) -> String {
    match s {
        MachineState::Idle => tr!("grbl.preflight.state_idle"),
        MachineState::Run => tr!("grbl.preflight.state_run"),
        MachineState::Hold(_) => tr!("grbl.preflight.state_hold"),
        MachineState::Jog => tr!("grbl.preflight.state_jog"),
        MachineState::Alarm => tr!("grbl.preflight.state_alarm"),
        MachineState::Door(_) => tr!("grbl.preflight.state_door"),
        MachineState::Check => tr!("grbl.preflight.state_check"),
        MachineState::Home => tr!("grbl.preflight.state_home"),
        MachineState::Sleep => tr!("grbl.preflight.state_sleep"),
        MachineState::Other => tr!("grbl.preflight.state_other"),
    }
}

/// Convierte el resultado del envío en un mensaje legible; `Ok` solo si todo salió bien.
/// Sin conocer el firmware (ver [`outcome_result_for`]).
pub fn outcome_result(outcome: &Outcome) -> Result<(), String> {
    outcome_result_for(Firmware::Unknown, outcome)
}

/// Como [`outcome_result`], con los textos de error y alarma de la familia del firmware.
pub fn outcome_result_for(fw: Firmware, outcome: &Outcome) -> Result<(), String> {
    match outcome {
        Outcome::Completed { errors: 0 } => Ok(()),
        Outcome::Completed { errors } => {
            Err(tr!("grbl.preflight.finished_with_errors", errors = errors))
        }
        Outcome::Cancelled => Err(tr!("grbl.preflight.cancelled")),
        Outcome::LineError { code, source_line } => {
            let msg = error_message_for(fw, *code);
            Err(if *code == 9 {
                tr!(
                    "grbl.preflight.line_error_locked",
                    code = code,
                    line = source_line,
                    msg = msg
                )
            } else {
                tr!(
                    "grbl.preflight.line_error",
                    code = code,
                    line = source_line,
                    msg = msg
                )
            })
        }
        Outcome::MachineFault(code) => Err(tr!(
            "grbl.preflight.machine_fault",
            code = format!("{code:02}"),
            msg = creality_error_message(*code),
            recovery = creality_recovery()
        )),
        Outcome::Alarm(code) => Err(tr!(
            "grbl.preflight.alarm_outcome",
            code = code,
            msg = alarm_message_for(fw, *code),
            hint = alarm_outcome_hint(fw, *code)
        )),
        Outcome::Reset(msg) => Err(tr!("grbl.preflight.reset_midjob", msg = msg)),
        Outcome::Disconnected(msg) => Err(tr!("grbl.preflight.disconnected", msg = msg)),
    }
}

/// Qué hacer tras un `ALARM:N` durante el envío.
fn alarm_outcome_hint(fw: Firmware, code: u32) -> String {
    match (fw, code) {
        (Firmware::GrblHal, 10) => tr!("grbl.preflight.alarm_hint_hal_10"),
        (Firmware::GrblHal, 11) => tr!("grbl.preflight.alarm_hint_hal_11"),
        (_, 3) => tr!("grbl.preflight.alarm_hint_3"),
        _ => tr!("grbl.preflight.alarm_hint_default"),
    }
}

/// Líneas del archivo de G-code, validadas contra el búfer de la controladora antes de
/// abrir el puerto. Devuelve cuántas líneas con contenido se enviarán.
pub fn validate_gcode(text: &str, rx_buffer: usize) -> Result<usize, String> {
    validate_gcode_for(text, &LinkProfile::grbl_with_buffer(rx_buffer))
}

/// Como [`validate_gcode`] con las reglas del enlace de cada firmware: espacios conservados
/// en Marlin y Smoothie, `?`/`!`/`~` admitidos en Marlin, línea máxima de su dialecto (con el
/// `N…*checksum` si se numeran las líneas).
pub fn validate_gcode_for(text: &str, link: &LinkProfile) -> Result<usize, String> {
    let max = link.flow.max_line_len();
    let mut count = 0;
    for (i, raw) in text.lines().enumerate() {
        let source_line = i + 1;
        let line = prepare_line_with(raw, link.strip_spaces, link.reject_realtime_chars)
            .map_err(|ch| StreamError::InvalidChar { source_line, ch }.to_string())?;
        if line.is_empty() {
            continue;
        }
        // La primera línea numerada es `M110 N0`: el trabajo empieza en la N1.
        let len = match link.line_numbering {
            Some(n) => crate::link::numbered_line(count + 1, &line, n).len() + 1,
            None => line.len() + 1,
        };
        if len > max {
            return Err(StreamError::LineTooLong {
                source_line,
                len,
                max,
            }
            .to_string());
        }
        count += 1;
    }
    if count == 0 {
        return Err(tr!("grbl.preflight.no_gcode_lines"));
    }
    Ok(count)
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
    /// Puerto que se usa en los comandos sugeridos (`tlaku cmd --port ...`).
    pub port: String,
}

impl Default for PreflightConfig {
    fn default() -> Self {
        Self {
            status_timeout: Duration::from_secs(3),
            settings_timeout: Duration::from_secs(2),
            port: tr!("grbl.preflight.port_placeholder"),
        }
    }
}

/// Texto de un fallo de E/S con la máquina.
pub fn io_err(e: io::Error) -> String {
    tr!("grbl.preflight.io_error", e = e)
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
            return Err(tr!(
                "grbl.preflight.status_timeout",
                secs = format!("{:.0}", timeout.as_secs_f64())
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

/// Ajustes `$N=valor` leídos con `$$`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Settings(BTreeMap<u32, f64>);

impl Settings {
    /// Interpreta las líneas de la respuesta a `$$`. Lo que no sea `$N=valor` numérico y
    /// finito se ignora (grblHAL antiguo añade comentarios tras el valor).
    pub fn parse<S: AsRef<str>>(lines: &[S]) -> Self {
        Self(
            lines
                .iter()
                .filter_map(|l| parse_setting_line(l.as_ref()))
                .collect(),
        )
    }

    pub fn get(&self, n: u32) -> Option<f64> {
        self.0.get(&n).copied()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// `$22` (homing activado).
    pub fn homing(&self) -> Option<bool> {
        self.get(22).map(|v| v != 0.0)
    }
}

fn parse_setting_line(line: &str) -> Option<(u32, f64)> {
    let (n, v) = line.trim().strip_prefix('$')?.split_once('=')?;
    let v = v.split_whitespace().next()?.parse::<f64>().ok()?;
    v.is_finite().then_some((n.parse().ok()?, v))
}

/// Lee los ajustes con `$$`. `None` si no se pudo escribir o no llegó nada en `timeout`.
fn read_settings(t: &mut impl Transport, timeout: Duration) -> Option<Settings> {
    t.write_all(b"$$\n").ok()?;
    let deadline = Instant::now() + timeout;
    let mut reader = LineReader::default();
    let mut buf = [0u8; 256];
    let mut lines = Vec::new();
    while Instant::now() < deadline {
        let n = t.read_timeout(&mut buf, Duration::from_millis(50)).ok()?;
        reader.feed(&buf[..n]);
        while let Some(line) = reader.next_line() {
            if line == "ok" || line.starts_with("error") {
                return Some(Settings::parse(&lines));
            }
            lines.push(line);
        }
    }
    let settings = Settings::parse(&lines);
    (!settings.is_empty()).then_some(settings)
}

/// Lee `$22` (homing activado) con `$$`; `None` si no responde o no aparece.
#[cfg(test)]
fn read_homing_setting(t: &mut impl Transport, timeout: Duration) -> Option<bool> {
    read_settings(t, timeout)?.homing()
}

/// Lee los parámetros con `$#`: las líneas `[G54:...]`, etc. hasta el `ok`.
fn read_parameters(t: &mut impl Transport, timeout: Duration) -> Option<Vec<String>> {
    t.write_all(b"$#\n").ok()?;
    let deadline = Instant::now() + timeout;
    let mut reader = LineReader::default();
    let mut buf = [0u8; 256];
    let mut lines = Vec::new();
    while Instant::now() < deadline {
        let n = t.read_timeout(&mut buf, Duration::from_millis(50)).ok()?;
        reader.feed(&buf[..n]);
        while let Some(line) = reader.next_line() {
            if line == "ok" || line.starts_with("error") {
                return Some(lines);
            }
            lines.push(line);
        }
    }
    (!lines.is_empty()).then_some(lines)
}

/// Qué espera el trabajo de la configuración de la controladora.
#[derive(Debug, Clone, PartialEq)]
pub struct JobExpect {
    /// Exige el modo láser (`$32=1`). Con `$32=0` GRBL trata M3/M4 como un husillo normal:
    /// el láser quedaría **encendido durante los G0** y el movimiento se detendría en cada
    /// cambio de S. Es lo que debe pedir un trabajo que dispara el láser; un encuadre (todo
    /// bajo M5/S0) no lo necesita.
    pub require_laser_mode: bool,
    /// `S máx` del perfil, para compararlo con `$30` (avisa si no coinciden).
    pub s_max: Option<f64>,
    /// Lee también `$#` y avisa de valores `nan` en los desplazamientos de trabajo.
    pub check_parameters: bool,
}

impl Default for JobExpect {
    fn default() -> Self {
        Self {
            require_laser_mode: true,
            s_max: None,
            check_parameters: false,
        }
    }
}

/// Compara `$$` con lo que espera el trabajo. `Err` bloquea el envío (con el porqué y cómo
/// arreglarlo); `Ok` trae avisos que no lo bloquean.
pub fn check_settings(
    settings: &Settings,
    expect: &JobExpect,
    advice: Advice,
) -> Result<Vec<String>, String> {
    let mut warnings = Vec::new();
    if expect.require_laser_mode {
        let fix = tr!("grbl.preflight.enable_laser_fix", cmd = advice.run("$32=1"));
        match settings.get(32) {
            Some(1.0) => {}
            Some(0.0) => {
                return Err(tr!("grbl.preflight.laser_mode_off", fix = fix));
            }
            Some(2.0) => {
                return Err(tr!("grbl.preflight.laser_mode_lathe", fix = fix));
            }
            Some(v) => {
                return Err(tr!("grbl.preflight.laser_mode_other", v = v, fix = fix));
            }
            None => warnings.push(tr!("grbl.preflight.warn_no_32")),
        }
    }
    if let Some(expected) = expect.s_max {
        match settings.get(30) {
            Some(v) if (v - expected).abs() > 1e-6 => {
                let pct = if v > 0.0 { expected / v * 100.0 } else { 0.0 };
                warnings.push(tr!(
                    "grbl.preflight.warn_s_max_mismatch",
                    v = v,
                    expected = expected,
                    pct = format!("{pct:.0}"),
                    cmd = advice.run(&format!("$30={expected}"))
                ));
            }
            Some(_) => {}
            None => warnings.push(tr!("grbl.preflight.warn_no_30")),
        }
    }
    Ok(warnings)
}

/// Avisos sobre las líneas de `$#`: desplazamientos con valores `nan` (el firmware de Creality
/// llega a imprimir `[G54:-nan,1.579,1.619]`). Tlaku no usa G54-G59, así que no bloquea.
pub fn parameter_warnings<S: AsRef<str>>(lines: &[S]) -> Vec<String> {
    let broken: Vec<String> = lines
        .iter()
        .filter_map(|l| parse_parameter_line(l.as_ref()))
        .filter(|(name, values)| {
            values.is_none() && (name.starts_with('G') || name == "TLO" || name == "PRB")
        })
        .map(|(name, _)| name)
        .collect();
    if broken.is_empty() {
        return Vec::new();
    }
    vec![tr!(
        "grbl.preflight.warn_nan_offsets",
        names = broken.join(", ")
    )]
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

/// Comando exacto de `tlaku cmd` para enviar `command` (`$X`, `$H`) a ese puerto.
pub fn cmd_hint(port: &str, command: &str) -> String {
    format!("tlaku cmd --port {} '{command}'", shell_arg(port))
}

/// Dónde se le sugiere al usuario ejecutar `$X`/`$H`: el CLI o la consola de la app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advice<'a> {
    /// `tlaku cmd --port <puerto> '<cmd>'`.
    Cli { port: &'a str },
    /// La consola de la pestaña Máquina de la app.
    Console,
}

impl Advice<'_> {
    fn run(self, command: &str) -> String {
        match self {
            Self::Cli { port } => cmd_hint(port, command),
            Self::Console => tr!("grbl.preflight.advice_console", command = command),
        }
    }

    /// Quién envía los comandos de recuperación (nunca el envío de trabajos).
    fn who(self) -> String {
        match self {
            Self::Cli { .. } => tr!("grbl.preflight.advice_who_cli"),
            Self::Console => tr!("grbl.preflight.advice_who_console"),
        }
    }
}

fn unlock_text(advice: Advice) -> String {
    tr!("grbl.preflight.unlock_text", cmd = advice.run("$X"))
}

fn home_text(advice: Advice) -> String {
    tr!("grbl.preflight.home_text", cmd = advice.run("$H"))
}

/// Qué sugerir según la causa de la alarma (si se conoce) y si el homing está activado ($22).
fn alarm_advice(code: Option<u32>, homing: Option<bool>, advice: Advice) -> Vec<String> {
    let line = |s: &str| format!("  - {s}");
    let (unlock, home) = (unlock_text(advice), home_text(advice));
    let (unlock, home) = (unlock.as_str(), home.as_str());
    let by_homing = |lost: Vec<String>| -> Vec<String> {
        let rest = match homing {
            Some(false) => vec![line(&tr!("grbl.preflight.advice_homing_off")), line(unlock)],
            Some(true) => vec![
                line(&tr!("grbl.preflight.advice_homing_on")),
                line(home),
                line(unlock),
            ],
            None => vec![
                line(&tr!("grbl.preflight.advice_if_limits")),
                line(home),
                line(&tr!("grbl.preflight.advice_if_no_limits")),
                line(unlock),
            ],
        };
        lost.into_iter().chain(rest).collect()
    };
    match (code, homing) {
        (Some(4 | 5), _) => vec![line(unlock)],
        (Some(6..=9), _) => vec![
            line(&tr!("grbl.preflight.advice_check_door")),
            line(home),
            line(unlock),
        ],
        (Some(1 | 2), Some(false)) => {
            vec![line(&tr!("grbl.preflight.advice_move_head")), line(unlock)]
        }
        (Some(1 | 2), _) => vec![
            line(&tr!("grbl.preflight.advice_move_head_unlock")),
            line(&tr!("grbl.preflight.advice_homing_after")),
        ],
        // Reinicio durante un movimiento: la posición se perdió. En la Falcon2 es lo más
        // frecuente en los foros y se atribuye a la alimentación, el USB o un sensor.
        (Some(3), _) => by_homing(vec![
            line(&tr!("grbl.preflight.advice_pos_lost")),
            line(&tr!("grbl.preflight.advice_pos_lost_cause")),
        ]),
        // ALARM:10 cambia de significado según el firmware; ALARM:11 es solo de grblHAL.
        (Some(10), _) => vec![
            line(&tr!("grbl.preflight.advice_hal_estop")),
            line(&tr!("grbl.preflight.advice_grbl_square")),
            line(unlock),
        ],
        (Some(11), _) => vec![line(&tr!("grbl.preflight.advice_hal_homing")), line(home)],
        _ => by_homing(Vec::new()),
    }
}

/// `Ok` solo si la máquina está en reposo; en cualquier otro caso explica por qué no se
/// transmite y qué hacer. `code` es el `ALARM:N` recibido (si lo hubo) y `homing` el valor
/// de `$22` (si se pudo leer). Con los consejos del CLI (`tlaku cmd --port <port>`).
pub fn check_ready(
    report: &StatusReport,
    code: Option<u32>,
    homing: Option<bool>,
    port: &str,
) -> Result<(), String> {
    check_ready_with(report, code, homing, Advice::Cli { port })
}

/// Como [`check_ready`], eligiendo dónde se sugiere ejecutar `$X`/`$H`.
pub fn check_ready_with(
    report: &StatusReport,
    code: Option<u32>,
    homing: Option<bool>,
    advice: Advice,
) -> Result<(), String> {
    let key = match report.state {
        MachineState::Idle => return Ok(()),
        MachineState::Alarm => {
            let mut msg = tr!("grbl.preflight.alarm_state");
            if let Some(c) = code {
                msg.push('\n');
                msg.push_str(&tr!(
                    "grbl.preflight.alarm_cause",
                    code = c,
                    msg = alarm_message_for(Firmware::Unknown, c)
                ));
            }
            msg.push('\n');
            msg.push_str(&tr!("grbl.preflight.alarm_what_to_do", who = advice.who()));
            msg.push('\n');
            msg.push_str(&alarm_advice(code, homing, advice).join("\n"));
            return Err(msg);
        }
        MachineState::Hold(_) => tr!("grbl.preflight.not_idle_hold"),
        MachineState::Door(_) => tr!("grbl.preflight.not_idle_door"),
        MachineState::Run => tr!("grbl.preflight.not_idle_run"),
        MachineState::Jog => tr!("grbl.preflight.not_idle_jog"),
        MachineState::Home => tr!("grbl.preflight.not_idle_home"),
        MachineState::Check => tr!("grbl.preflight.not_idle_check"),
        MachineState::Sleep => tr!("grbl.preflight.not_idle_sleep"),
        MachineState::Other => tr!("grbl.preflight.not_idle_other"),
    };
    Err(key)
}

/// Consulta el estado con `?` y solo da el visto bueno si la máquina está en reposo.
/// No envía ningún comando que cambie el estado (`$X`, `$H`, `~`...). Los consejos son los
/// del CLI; ver [`preflight_with`].
pub fn preflight(
    t: &mut impl Transport,
    config: &PreflightConfig,
    out: &mut impl Write,
) -> Result<(), String> {
    preflight_with(t, config, Advice::Cli { port: &config.port }, out)
}

/// Como [`preflight`], eligiendo dónde se sugiere ejecutar `$X`/`$H`. Exige el modo láser
/// (ver [`JobExpect::default`]).
pub fn preflight_with(
    t: &mut impl Transport,
    config: &PreflightConfig,
    advice: Advice,
    out: &mut impl Write,
) -> Result<(), String> {
    preflight_checked(t, config, advice, &JobExpect::default(), out)
}

/// Como [`preflight_with`], comprobando además la configuración de la controladora contra
/// `expect`: con la máquina en reposo se lee `$$` y se **bloquea** el envío si `$32` no es 1
/// (láser encendido durante los G0), y se **avisa** (en `out`, líneas que empiezan por
/// "Aviso") si `$30` no coincide con `S máx` o si `$#` trae `nan`. Si `$$` no se puede leer
/// solo se avisa: no todos los firmwares lo contestan.
pub fn preflight_checked(
    t: &mut impl Transport,
    config: &PreflightConfig,
    advice: Advice,
    expect: &JobExpect,
    out: &mut impl Write,
) -> Result<(), String> {
    let (report, others) = query_status(t, config.status_timeout)?;
    let _ = writeln!(
        out,
        "{}",
        tr!(
            "grbl.preflight.machine_state",
            state = state_name(report.state)
        )
    );
    let code = others
        .iter()
        .find_map(|l| l.strip_prefix("ALARM:")?.trim().parse::<u32>().ok());
    let wants_settings = report.state == MachineState::Alarm
        || (report.state == MachineState::Idle
            && (expect.require_laser_mode || expect.s_max.is_some()));
    let settings = wants_settings
        .then(|| read_settings(t, config.settings_timeout))
        .flatten();
    let homing = settings.as_ref().and_then(Settings::homing);
    let mut result = check_ready_with(&report, code, homing, advice);
    if result.is_ok() && wants_settings {
        match &settings {
            Some(s) => match check_settings(s, expect, advice) {
                Ok(warnings) => {
                    for w in warnings {
                        let _ = writeln!(out, "{w}");
                    }
                }
                Err(e) => result = Err(e),
            },
            None => {
                let _ = writeln!(out, "{}", tr!("grbl.preflight.warn_no_settings"));
            }
        }
    }
    if result.is_ok()
        && expect.check_parameters
        && let Some(lines) = read_parameters(t, config.settings_timeout)
    {
        for w in parameter_warnings(&lines) {
            let _ = writeln!(out, "{w}");
        }
    }
    // Lo que sobre (otro reporte, el `ok` de `$$`) no debe confundir al streamer.
    let _ = t.clear_input();
    result
}

// ---------------------------------------------------------------------------
// Comprobación por familia de firmware
// ---------------------------------------------------------------------------

/// Lo que se averigua de Marlin o Smoothieware con `M115`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReprapInfo {
    /// `FIRMWARE_NAME:` (hasta el siguiente campo, sin recortar versiones).
    pub firmware_name: Option<String>,
    /// Líneas `Cap:NOMBRE:0/1` (con `EXTENDED_CAPABILITIES_REPORT`).
    pub caps: BTreeMap<String, bool>,
    /// Todas las líneas de la respuesta, sin el `ok`.
    pub lines: Vec<String>,
}

impl ReprapInfo {
    /// Interpreta las líneas de la respuesta a `M115`.
    pub fn parse<S: AsRef<str>>(lines: &[S]) -> Self {
        let mut info = Self::default();
        for l in lines {
            let l = l.as_ref().trim();
            info.lines.push(l.to_string());
            if let Some(i) = l.find("FIRMWARE_NAME:") {
                let rest = &l[i + "FIRMWARE_NAME:".len()..];
                // Marlin separa los campos con espacios (`... PROTOCOL_VERSION:1.0`); Smoothie
                // con comas. El nombre llega hasta el siguiente `CLAVE:`.
                let end = rest
                    .match_indices(" ")
                    .find(|(j, _)| {
                        let word = rest[j + 1..].split_whitespace().next().unwrap_or("");
                        word.split_once(':').is_some_and(|(k, _)| {
                            !k.is_empty() && k.bytes().all(|b| b.is_ascii_uppercase() || b == b'_')
                        })
                    })
                    .map_or(rest.len(), |(j, _)| j);
                info.firmware_name = Some(rest[..end].trim().trim_end_matches(',').to_string());
            } else if let Some(cap) = l.strip_prefix("Cap:")
                && let Some((name, v)) = cap.rsplit_once(':')
                && let Ok(v) = v.trim().parse::<u8>()
            {
                info.caps.insert(name.to_string(), v != 0);
            }
        }
        info
    }

    fn name_lower(&self) -> String {
        self.firmware_name
            .as_deref()
            .unwrap_or("")
            .to_ascii_lowercase()
    }
}

/// Envía `M115` y espera su `ok` (se reenvía cada segundo). Devuelve las líneas de la
/// respuesta. `Err` con el motivo si no contesta o si contesta como GRBL.
fn query_m115(
    t: &mut impl Transport,
    link: &LinkProfile,
    timeout: Duration,
) -> Result<Vec<String>, String> {
    t.clear_input().map_err(io_err)?;
    let deadline = Instant::now() + timeout;
    let mut next_send = Instant::now();
    let mut reader = LineReader::default();
    let mut lines = Vec::new();
    let mut buf = [0u8; 256];
    loop {
        let now = Instant::now();
        if now >= deadline {
            return Err(tr!(
                "grbl.preflight.m115_timeout",
                secs = format!("{:.0}", timeout.as_secs_f64()),
                family = link.family_name()
            ));
        }
        if now >= next_send {
            t.write_all(b"M115\n").map_err(io_err)?;
            next_send = now + Duration::from_secs(1);
        }
        let n = t
            .read_timeout(&mut buf, Duration::from_millis(50))
            .map_err(io_err)?;
        reader.feed(&buf[..n]);
        while let Some(line) = reader.next_line() {
            let low = line.to_ascii_lowercase();
            if low.starts_with("grbl") || low.starts_with("[ver:") || low.starts_with("<") {
                return Err(tr!(
                    "grbl.preflight.m115_is_grbl",
                    line = line,
                    family = link.family_name()
                ));
            }
            if link.is_ack(&line) {
                return Ok(lines);
            }
            if low.starts_with("error") {
                return Err(tr!(
                    "grbl.preflight.m115_rejected",
                    line = line,
                    family = link.family_name()
                ));
            }
            lines.push(line);
        }
    }
}

/// Lo que el trabajo exige al enlace serie: líneas por segundo en su tramo más denso contra
/// lo que el baudio deja pasar. Con 115 200 baudios (8N1, 10 bits por byte) pasan 11 520 bytes
/// por segundo: con líneas de 19 bytes son unas 600 líneas por segundo. Si el trabajo pide más
/// durante un tramo, la máquina vacía su planificador, se queda esperando líneas y hace pausas
/// (con el láser encendido en M3 puede dejar marcas).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinkLoad {
    /// Velocidad del puerto serie en baudios.
    pub baud: u32,
    /// Líneas por segundo que pide el trabajo en su tramo más denso (no el promedio); ver
    /// `tlaku_sim::line_rate`, con el trabajo ya compactado (`gcode::compact_toolpath`).
    pub peak_lines_per_s: f64,
    /// Largo medio de línea en bytes, con su salto de línea.
    pub mean_line_bytes: f64,
}

impl LinkLoad {
    /// Carga de un G-code de `lines` líneas y `bytes` bytes (con saltos de línea).
    pub fn new(baud: u32, peak_lines_per_s: f64, lines: usize, bytes: usize) -> Self {
        Self {
            baud,
            peak_lines_per_s,
            mean_line_bytes: if lines > 0 {
                bytes as f64 / lines as f64
            } else {
                0.0
            },
        }
    }

    /// Líneas por segundo que deja pasar el enlace: `baud / 10` bytes entre el largo medio.
    pub fn capacity_lines_per_s(&self) -> f64 {
        if self.baud == 0 || !(self.mean_line_bytes.is_finite() && self.mean_line_bytes > 0.0) {
            return f64::INFINITY;
        }
        f64::from(self.baud) / 10.0 / self.mean_line_bytes
    }

    /// Aviso si el tramo más denso pide más líneas de las que caben en el enlace.
    pub fn warning(&self) -> Option<String> {
        let capacity = self.capacity_lines_per_s();
        if !(self.peak_lines_per_s.is_finite() && self.peak_lines_per_s > capacity) {
            return None;
        }
        let keep = (capacity / self.peak_lines_per_s * 100.0).floor().max(1.0);
        Some(tr!(
            "grbl.preflight.warn_link_load",
            peak = format!("{:.0}", self.peak_lines_per_s),
            baud = self.baud,
            capacity = format!("{capacity:.0}"),
            bytes = format!("{:.0}", self.mean_line_bytes),
            keep = format!("{keep:.0}")
        ))
    }
}

/// Como [`preflight_link`], y si el trabajo supera lo que permite el enlace (ver [`LinkLoad`])
/// añade su aviso a `out` (línea "Aviso"). El aviso no bloquea el envío.
pub fn preflight_link_with_load(
    t: &mut impl Transport,
    config: &PreflightConfig,
    advice: Advice,
    expect: &JobExpect,
    link: &LinkProfile,
    load: Option<&LinkLoad>,
    out: &mut impl Write,
) -> Result<(), String> {
    preflight_link(t, config, advice, expect, link, out)?;
    if let Some(w) = load.and_then(LinkLoad::warning) {
        let _ = writeln!(out, "{w}");
    }
    Ok(())
}

/// Comprobación previa para el enlace de `link`. Con GRBL es exactamente [`preflight_checked`]
/// (estado con `?`, `$32`, `$30`). Con Marlin y Smoothieware comprueba que la máquina responde
/// a `M115` y que es del firmware esperado, y **avisa** (líneas "Aviso" en `out`) de lo que no
/// se puede verificar: no hay `?` para saber si está en reposo o en alarma, ni forma de leer
/// la escala de `S` (`CUTTER_POWER_UNIT`, `Maximum S-value`), el modo inline o el módulo láser.
pub fn preflight_link(
    t: &mut impl Transport,
    config: &PreflightConfig,
    advice: Advice,
    expect: &JobExpect,
    link: &LinkProfile,
    out: &mut impl Write,
) -> Result<(), String> {
    if link.family == LinkFamily::Grbl {
        return preflight_checked(t, config, advice, expect, out);
    }
    let lines = query_m115(t, link, config.status_timeout)?;
    // Respuestas repetidas por los reenvíos de M115 que aún lleguen.
    std::thread::sleep(Duration::from_millis(50));
    let _ = t.clear_input();
    let info = ReprapInfo::parse(&lines);
    let me = link.family_name();
    let name = info.firmware_name.clone();
    let low = info.name_lower();
    match (link.family, name.as_deref()) {
        (LinkFamily::Marlin, Some(n)) if low.contains("smoothie") => {
            return Err(tr!("grbl.preflight.not_marlin", name = n));
        }
        (LinkFamily::Smoothie, Some(n)) if low.contains("marlin") => {
            return Err(tr!("grbl.preflight.not_smoothie", name = n));
        }
        _ => {}
    }
    match &name {
        Some(n) => {
            let _ = writeln!(out, "{}", tr!("grbl.preflight.firmware_line", name = n));
        }
        None => {
            let _ = writeln!(
                out,
                "{}",
                tr!("grbl.preflight.warn_no_firmware_name", me = me)
            );
        }
    }
    if name.is_some() && !low.contains(&me.to_ascii_lowercase()[..6]) {
        let _ = writeln!(
            out,
            "{}",
            tr!(
                "grbl.preflight.warn_firmware_mismatch",
                me = me,
                name = name.as_deref().unwrap_or("")
            )
        );
    }
    let _ = writeln!(
        out,
        "{}",
        tr!("grbl.preflight.warn_no_status_query", me = me)
    );
    if expect.require_laser_mode {
        let msg = match link.family {
            LinkFamily::Marlin => tr!("grbl.preflight.warn_laser_unchecked_marlin", me = me),
            _ => tr!("grbl.preflight.warn_laser_unchecked_smoothie", me = me),
        };
        let _ = writeln!(out, "{msg}");
    }
    if let Some(s_max) = expect.s_max {
        let msg = match link.family {
            LinkFamily::Marlin => tr!("grbl.preflight.warn_s_scale_marlin", s_max = s_max),
            _ => tr!("grbl.preflight.warn_s_scale_smoothie", s_max = s_max),
        };
        let _ = writeln!(out, "{msg}");
    }
    if link.family == LinkFamily::Marlin {
        match info.caps.get("EMERGENCY_PARSER") {
            Some(true) => {
                if !link.emergency_parser {
                    let _ = writeln!(out, "{}", tr!("grbl.preflight.warn_emergency_unused"));
                }
            }
            Some(false) => {
                let _ = writeln!(out, "{}", tr!("grbl.preflight.warn_no_emergency"));
            }
            None => {
                let _ = writeln!(out, "{}", tr!("grbl.preflight.warn_emergency_unknown"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serial::memory_pair;

    #[test]
    fn link_load_warns_only_when_the_dense_stretch_exceeds_the_baud() {
        // 115 200 baudios y líneas de 19 bytes: 11 520 / 19 = 606 líneas por segundo.
        let ok = LinkLoad::new(115_200, 500.0, 1000, 19_000);
        assert!((ok.capacity_lines_per_s() - 606.3).abs() < 0.1);
        assert_eq!(ok.warning(), None);
        let bad = LinkLoad::new(115_200, 1200.0, 1000, 19_000);
        let w = bad.warning().unwrap();
        assert!(w.starts_with("Aviso:"), "{w}");
        assert!(w.contains("1200 líneas por segundo"), "{w}");
        assert!(w.contains("606"), "{w}");
        assert!(w.contains("pausas") && w.contains("marcas"), "{w}");
        assert!(
            w.contains("Baja la velocidad") && w.contains("simplifica"),
            "{w}"
        );
        assert!(w.contains("a menos del 50 %"), "{w}");
        // El mismo trabajo a 921 600 baudios cabe de sobra.
        assert_eq!(LinkLoad::new(921_600, 1200.0, 1000, 19_000).warning(), None);
        // Sin datos no hay aviso.
        assert_eq!(LinkLoad::new(0, 1200.0, 1000, 19_000).warning(), None);
        assert_eq!(LinkLoad::new(115_200, 1200.0, 0, 0).warning(), None);
    }

    #[test]
    fn preflight_with_load_adds_the_warning_after_the_normal_checks() {
        let (mut host, dev) = memory_pair();
        let machine = fake_machine(dev, IDLE, Some("$30=1000\r\n$32=1\r\n"), None);
        let load = LinkLoad::new(115_200, 1500.0, 100, 1900);
        let mut out = Vec::new();
        let r = preflight_link_with_load(
            &mut host,
            &PreflightConfig::default(),
            Advice::Console,
            &JobExpect::default(),
            &LinkProfile::grbl(),
            Some(&load),
            &mut out,
        );
        drop(host);
        let _ = machine.join();
        let text = String::from_utf8_lossy(&out).to_string();
        assert!(r.is_ok(), "{r:?} {text}");
        assert!(text.contains("1500 líneas por segundo"), "{text}");
        // Sin carga no hay aviso de enlace.
        let (mut host, dev) = memory_pair();
        let machine = fake_machine(dev, IDLE, Some("$30=1000\r\n$32=1\r\n"), None);
        let mut out = Vec::new();
        preflight_link_with_load(
            &mut host,
            &PreflightConfig::default(),
            Advice::Console,
            &JobExpect::default(),
            &LinkProfile::grbl(),
            None,
            &mut out,
        )
        .unwrap();
        drop(host);
        let _ = machine.join();
        assert!(!String::from_utf8_lossy(&out).contains("líneas por segundo"));
    }

    fn report(line: &str) -> StatusReport {
        StatusReport::parse(line).unwrap()
    }

    #[test]
    fn console_advice_points_to_the_console_not_the_cli() {
        let r = report("<Alarm|MPos:0.000,0.000,0.000|FS:0,0>");
        let e = check_ready_with(&r, None, Some(true), Advice::Console).unwrap_err();
        assert!(e.contains("consola de la pestaña Máquina"), "{e}");
        assert!(e.contains("$H") && e.contains("$X"), "{e}");
        assert!(!e.contains("tlaku"), "{e}");
        let cli = check_ready(&r, None, Some(true), "/dev/cu.x").unwrap_err();
        assert!(cli.contains("tlaku cmd --port /dev/cu.x '$H'"), "{cli}");
    }

    #[test]
    fn preflight_with_accepts_idle_and_refuses_hold() {
        for (status, ok) in [
            ("<Idle|MPos:0,0,0|FS:0,0>", true),
            ("<Hold:0|MPos:0,0,0>", false),
        ] {
            let (mut host, mut dev) = memory_pair();
            // Máquina falsa: contesta con el reporte cuando le llega un `?`.
            let device = std::thread::spawn(move || {
                let mut buf = [0u8; 16];
                loop {
                    match dev.read_timeout(&mut buf, Duration::from_millis(50)) {
                        Ok(n) if buf[..n].contains(&b'?') => {
                            dev.write_all(format!("{status}\r\n").as_bytes()).unwrap();
                            return;
                        }
                        Ok(_) => {}
                        Err(_) => return,
                    }
                }
            });
            let mut out = Vec::new();
            let r = preflight_with(
                &mut host,
                &PreflightConfig::default(),
                Advice::Console,
                &mut out,
            );
            device.join().unwrap();
            assert_eq!(r.is_ok(), ok, "{r:?}");
            if !ok {
                assert!(r.unwrap_err().contains("No se transmitió nada"));
            }
        }
    }

    // ---- configuración de la controladora ($32, $30, $#) --------------------------------

    use crate::serial::MemoryTransport;
    use std::thread::JoinHandle;

    /// Volcado `$$` A de la Falcon2 22 W/40 W (falcon2.md, sección 2), usuario con rotativo.
    const FALCON_A: &str = "$0=10\r\n$1=250\r\n$2=0\r\n$3=4\r\n$10=3\r\n$13=0\r\n$20=0\r\n$21=1\r\n\
        $22=1\r\n$23=3\r\n$24=2000\r\n$25=4000\r\n$26=20\r\n$27=5.000\r\n$30=1000\r\n$31=0\r\n\
        $32=1\r\n$100=80\r\n$101=60.375\r\n$110=25000\r\n$111=8000\r\n$120=1000\r\n$121=1000\r\n\
        $130=400\r\n$131=415\r\n$150=1\r\n$151=4\r\n$152=30\r\n$153=0\r\n$154=1\r\n$155=1\r\n\
        $156=0\r\n$157=1\r\n$158=1\r\n$160=1\r\n$161=13.00\r\n";
    /// Volcado `$$` B (Falcon2 Pro 22 W).
    const FALCON_B: &str = "$0=10\r\n$10=3\r\n$21=1\r\n$22=1\r\n$23=3\r\n$30=1000\r\n$31=0\r\n$32=1\r\n\
        $100=80\r\n$101=80\r\n$110=50000\r\n$111=50000\r\n$120=5000\r\n$121=5000\r\n$130=400\r\n\
        $131=400\r\n$150=36\r\n$151=100\r\n$152=30\r\n$153=0\r\n$154=1\r\n$155=1\r\n$161=13.00\r\n";
    const IDLE: &str = "<Idle|MPos:0.000,0.000,0.000|Bf:127,65535|FS:0,0|Pn:XY>";

    /// Máquina falsa: contesta `?` con `status`, `$$` con `settings` y `$#` con `params` (cada
    /// uno solo si se da) y `ok` al terminar. Termina cuando se suelta el otro extremo.
    fn fake_machine(
        mut dev: MemoryTransport,
        status: &'static str,
        settings: Option<&'static str>,
        params: Option<&'static str>,
    ) -> JoinHandle<Vec<String>> {
        std::thread::spawn(move || {
            let (mut seen, mut partial) = (Vec::new(), String::new());
            let mut buf = [0u8; 64];
            while let Ok(n) = dev.read_timeout(&mut buf, Duration::from_millis(20)) {
                for &b in &buf[..n] {
                    match b {
                        b'?' => {
                            let _ = dev.write_all(format!("{status}\r\n").as_bytes());
                        }
                        b'\n' => {
                            let line = std::mem::take(&mut partial);
                            let body = match line.as_str() {
                                "$$" => settings,
                                "$#" => params,
                                _ => None,
                            };
                            if let Some(text) = body {
                                let _ = dev.write_all(text.as_bytes());
                                let _ = dev.write_all(b"ok\r\n");
                            }
                            seen.push(line);
                        }
                        c => partial.push(c as char),
                    }
                }
            }
            seen
        })
    }

    fn check(
        status: &'static str,
        settings: Option<&'static str>,
        params: Option<&'static str>,
        expect: &JobExpect,
    ) -> (Result<(), String>, String, Vec<String>) {
        let (mut host, dev) = memory_pair();
        let machine = fake_machine(dev, status, settings, params);
        let config = PreflightConfig {
            status_timeout: Duration::from_secs(2),
            settings_timeout: Duration::from_millis(200),
            port: "fake".into(),
        };
        let mut out = Vec::new();
        let r = preflight_checked(&mut host, &config, Advice::Console, expect, &mut out);
        drop(host);
        (r, String::from_utf8(out).unwrap(), machine.join().unwrap())
    }

    #[test]
    fn laser_mode_off_blocks_the_job_and_says_why_and_how_to_fix_it() {
        let (r, out, seen) = check(
            IDLE,
            Some("$30=1000\r\n$32=0\r\n"),
            None,
            &JobExpect::default(),
        );
        let e = r.unwrap_err();
        assert!(e.contains("$32=0"), "{e}");
        assert!(
            e.contains("encendido durante los movimientos rápidos (G0)"),
            "{e}"
        );
        assert!(
            e.contains("$32=1") && e.contains("consola de la pestaña Máquina"),
            "{e}"
        );
        assert!(e.contains("No se transmitió nada"), "{e}");
        assert!(out.contains("Estado de la máquina: reposo"), "{out}");
        // Solo consultas: nada que cambie el estado.
        assert!(seen.iter().all(|l| l == "$$"), "{seen:?}");
        // Con el CLI, el comando exacto.
        let (mut host, dev) = memory_pair();
        let m = fake_machine(dev, IDLE, Some("$32=0\r\n"), None);
        let e = preflight(
            &mut host,
            &PreflightConfig {
                port: "/dev/cu.x".into(),
                settings_timeout: Duration::from_millis(200),
                ..PreflightConfig::default()
            },
            &mut Vec::new(),
        )
        .unwrap_err();
        drop(host);
        m.join().unwrap();
        assert!(e.contains("tlaku cmd --port /dev/cu.x '$32=1'"), "{e}");
    }

    #[test]
    fn laser_mode_other_than_one_blocks_too() {
        let (r, ..) = check(IDLE, Some("$32=2\r\n"), None, &JobExpect::default());
        assert!(r.unwrap_err().contains("modo torno"));
        let (r, ..) = check(IDLE, Some("$32=7\r\n"), None, &JobExpect::default());
        assert!(r.unwrap_err().contains("$32=7"));
    }

    #[test]
    fn laser_mode_on_with_matching_power_scale_goes_ahead_without_warnings() {
        let expect = JobExpect {
            s_max: Some(1000.0),
            ..JobExpect::default()
        };
        for dump in [FALCON_A, FALCON_B] {
            let (r, out, _) = check(IDLE, Some(dump), None, &expect);
            r.unwrap();
            assert!(!out.contains("Aviso"), "{out}");
        }
    }

    #[test]
    fn s_max_mismatch_warns_but_does_not_block() {
        let expect = JobExpect {
            s_max: Some(1000.0),
            ..JobExpect::default()
        };
        // GRBL-LPC: $30=1.
        let (r, out, _) = check(IDLE, Some("$30=1.000\r\n$32=1\r\n"), None, &expect);
        r.unwrap();
        assert!(out.contains("Aviso: $30=1 "), "{out}");
        assert!(
            out.contains("S máx = 1000") && out.contains("GRBL-LPC"),
            "{out}"
        );
        // $30=255 con un perfil de 1000: S máx del perfil sería el 392 % de la controladora.
        let (r, out, _) = check(IDLE, Some("$30=255\r\n$32=1\r\n"), None, &expect);
        r.unwrap();
        assert!(out.contains("392 %") && out.contains("$30=1000"), "{out}");
        // Sin S máx del perfil no hay comparación.
        let (r, out, _) = check(
            IDLE,
            Some("$30=255\r\n$32=1\r\n"),
            None,
            &JobExpect::default(),
        );
        r.unwrap();
        assert!(!out.contains("Aviso"), "{out}");
    }

    #[test]
    fn a_frame_does_not_require_laser_mode() {
        let expect = JobExpect {
            require_laser_mode: false,
            ..JobExpect::default()
        };
        let (r, ..) = check(IDLE, Some("$32=0\r\n"), None, &expect);
        r.unwrap();
    }

    #[test]
    fn unreadable_settings_only_warn() {
        // La máquina no contesta a `$$` (p. ej. un firmware que no lo admite).
        let (r, out, _) = check(IDLE, None, None, &JobExpect::default());
        r.unwrap();
        assert!(out.contains("Aviso") && out.contains("$$"), "{out}");
        // Contesta pero sin $32.
        let (r, out, _) = check(IDLE, Some("$22=0\r\n"), None, &JobExpect::default());
        r.unwrap();
        assert!(out.contains("no informó $32"), "{out}");
    }

    #[test]
    fn nan_in_the_parameters_is_reported_not_trusted() {
        let expect = JobExpect {
            check_parameters: true,
            ..JobExpect::default()
        };
        let params = "[G54:-nan,1.579,1.619]\r\n[G55:0.000,0.000,0.000]\r\n[G92:0.000,0.000,0.000]\r\n\
                      [TLO:0.000]\r\n[PRB:0.000,0.000,0.000:0]\r\n";
        let (r, out, seen) = check(IDLE, Some("$32=1\r\n"), Some(params), &expect);
        r.unwrap();
        assert!(seen.contains(&"$#".to_string()), "{seen:?}");
        assert!(out.contains("(nan) en G54 y"), "{out}");
        assert!(!out.contains("G55") && out.contains("$RST=#"), "{out}");
        // Sin nan, ni aviso; y sin pedirlo, ni siquiera se consulta `$#`.
        let clean = "[G54:0.000,0.000,0.000]\r\n[TLO:0.000]\r\n";
        let (_, out, _) = check(IDLE, Some("$32=1\r\n"), Some(clean), &expect);
        assert!(!out.contains("nan"), "{out}");
        let (_, _, seen) = check(IDLE, Some("$32=1\r\n"), Some(params), &JobExpect::default());
        assert!(!seen.contains(&"$#".to_string()), "{seen:?}");
    }

    #[test]
    fn real_falcon_dumps_parse_into_settings() {
        for dump in [FALCON_A, FALCON_B] {
            let lines: Vec<&str> = dump.lines().collect();
            let s = Settings::parse(&lines);
            assert_eq!(s.homing(), Some(true));
            assert_eq!(
                (s.get(30), s.get(32), s.get(161)),
                (Some(1000.0), Some(1.0), Some(13.0))
            );
        }
        let b = Settings::parse(&FALCON_B.lines().collect::<Vec<_>>());
        assert_eq!(
            (b.get(110), b.get(121), b.get(131)),
            (Some(50000.0), Some(5000.0), Some(400.0))
        );
        // Comentario tras el valor (grblHAL antiguo) y basura.
        let s = Settings::parse(&["$30=1000 (rpm máx)", "$x=1", "$22=nan", "ok", "[MSG:x]"]);
        assert_eq!((s.get(30), s.get(22)), (Some(1000.0), None));
    }

    #[test]
    fn homing_is_read_from_the_real_dumps() {
        for dump in [FALCON_A, FALCON_B] {
            let (mut host, dev) = memory_pair();
            let m = fake_machine(dev, IDLE, Some(dump), None);
            assert_eq!(
                read_homing_setting(&mut host, Duration::from_secs(1)),
                Some(true)
            );
            drop(host);
            m.join().unwrap();
        }
    }

    // ---- consejos, alarmas y mensajes del resultado -------------------------------------

    #[test]
    fn alarm3_advice_blames_power_usb_and_sensors_and_still_offers_home() {
        let r = report("<Alarm|MPos:0.000,0.000,0.000|FS:0,0>");
        let e = check_ready_with(&r, Some(3), Some(true), Advice::Console).unwrap_err();
        assert!(e.contains("la posición se perdió"), "{e}");
        assert!(
            e.contains("alimentación") && e.contains("USB") && e.contains("sensor"),
            "{e}"
        );
        assert!(e.contains("$H"), "{e}");
    }

    #[test]
    fn grblhal_alarms_10_and_11_have_their_own_advice() {
        let r = report("<Alarm|MPos:0.000,0.000,0.000|FS:0,0>");
        let e = check_ready_with(&r, Some(10), None, Advice::Console).unwrap_err();
        assert!(
            e.contains("parada de emergencia") && e.contains("suelta el botón"),
            "{e}"
        );
        let e = check_ready_with(&r, Some(11), None, Advice::Console).unwrap_err();
        assert!(e.contains("homing") && e.contains("$H"), "{e}");
    }

    #[test]
    fn outcome_messages_explain_the_safe_stop() {
        let e = outcome_result(&Outcome::MachineFault(5)).unwrap_err();
        assert!(e.contains("ERROR:05") && e.contains("llama"), "{e}");
        assert!(
            e.contains("feed hold") && e.contains("Reset") && e.contains("no la desbloquea"),
            "{e}"
        );
        let e = outcome_result(&Outcome::LineError {
            code: 20,
            source_line: 4,
        })
        .unwrap_err();
        assert!(
            e.contains("error:20") && e.contains("línea 4") && e.contains("se detuvo"),
            "{e}"
        );
        let e = outcome_result_for(Firmware::GrblHal, &Outcome::Alarm(10)).unwrap_err();
        assert!(e.contains("emergencia") && e.contains("Suelta"), "{e}");
        let e = outcome_result_for(Firmware::Grbl, &Outcome::Alarm(10)).unwrap_err();
        assert!(e.contains("autoescuadre"), "{e}");
        let e = outcome_result(&Outcome::Alarm(3)).unwrap_err();
        assert!(e.contains("USB"), "{e}");
        assert_eq!(state_name(MachineState::Other), "otro estado");
        let r = report("<Tool|MPos:0,0,0|FS:0,0,0>");
        let e = check_ready_with(&r, None, None, Advice::Console).unwrap_err();
        assert!(e.contains("no reconoce"), "{e}");
    }

    #[test]
    fn reprap_info_reads_the_firmware_name_and_capabilities() {
        let i = ReprapInfo::parse(&[
            "FIRMWARE_NAME:Marlin 2.1.2.5 (Tlaku emulador) PROTOCOL_VERSION:1.0 MACHINE_TYPE:Laser",
            "Cap:EMERGENCY_PARSER:1",
            "Cap:AUTOREPORT_POS:0",
        ]);
        assert_eq!(
            i.firmware_name.as_deref(),
            Some("Marlin 2.1.2.5 (Tlaku emulador)")
        );
        assert_eq!(i.caps.get("EMERGENCY_PARSER"), Some(&true));
        assert_eq!(i.caps.get("AUTOREPORT_POS"), Some(&false));
        let s = ReprapInfo::parse(&["FIRMWARE_NAME:Smoothieware, FIRMWARE_VERSION:1.1 (x)"]);
        assert_eq!(s.firmware_name.as_deref(), Some("Smoothieware"));
        assert_eq!(ReprapInfo::parse(&["ok?"]).firmware_name, None);
    }

    #[test]
    fn validation_follows_the_link_rules() {
        use tlaku_doc::GcodeDialect;
        let marlin = LinkProfile::marlin(&GcodeDialect::marlin_inline());
        // Espacios conservados; `!` admitido en Marlin.
        assert_eq!(validate_gcode_for("M3 S0 I\nM117 hola!\n", &marlin), Ok(2));
        assert!(validate_gcode_for("M117 hola!", &LinkProfile::grbl()).is_err());
        // Línea de más de 96 bytes.
        let long = format!("M117 {}", "x".repeat(100));
        assert!(
            validate_gcode_for(&long, &marlin)
                .unwrap_err()
                .contains("96")
        );
        // Con líneas numeradas el prefijo y el checksum cuentan.
        let numbered = marlin.clone().with_line_numbering();
        let near = format!("M117 {}", "x".repeat(88));
        assert!(validate_gcode_for(&near, &marlin).is_ok());
        assert!(validate_gcode_for(&near, &numbered).is_err());
        // Smoothie no tiene dato de largo máximo.
        let smoothie =
            LinkProfile::smoothie(&GcodeDialect::for_kind(tlaku_doc::DeviceKind::Smoothie));
        assert!(validate_gcode_for(&long, &smoothie).is_ok());
        // GRBL, igual que siempre.
        assert_eq!(
            validate_gcode("G1 X1\n", 127),
            validate_gcode_for("G1 X1\n", &LinkProfile::grbl())
        );
    }

    #[test]
    fn gcode_validation_counts_lines_and_rejects_bad_ones() {
        assert_eq!(validate_gcode("G90\n; nada\n\nG0 X1", 127), Ok(2));
        assert!(validate_gcode("; solo comentario", 127).is_err());
        assert!(
            validate_gcode("G1 X1 ?", 127)
                .unwrap_err()
                .contains("tiempo real")
        );
        assert!(
            validate_gcode(&"G1 X1 ".repeat(30), 20)
                .unwrap_err()
                .contains("búfer")
        );
    }
}
