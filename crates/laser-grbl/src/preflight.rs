//! Comprobación de seguridad antes de transmitir un trabajo y mensajes legibles del resultado.
//!
//! La comparten el CLI (`lasersoft send`) y la app de escritorio: solo se transmite con la
//! máquina en reposo (`Idle`); en cualquier otro estado se explica qué hacer. Nunca se envía
//! `$X`, `$H` ni `~` por cuenta propia. Recibe un `Transport` ya abierto, de modo que se
//! prueba con `memory_pair` sin hardware.

use crate::serial::{LineReader, Transport};
use crate::streamer::{
    MachineState, Outcome, StatusReport, StreamError, alarm_message, error_message, prepare_line,
};
use std::io::{self, Write};
use std::time::{Duration, Instant};

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

/// Texto de un fallo de E/S con la máquina.
pub fn io_err(e: io::Error) -> String {
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

/// Dónde se le sugiere al usuario ejecutar `$X`/`$H`: el CLI o la consola de la app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advice<'a> {
    /// `lasersoft cmd --port <puerto> '<cmd>'`.
    Cli { port: &'a str },
    /// La consola de la pestaña Máquina de la app.
    Console,
}

impl Advice<'_> {
    fn run(self, command: &str) -> String {
        match self {
            Self::Cli { port } => cmd_hint(port, command),
            Self::Console => format!("escribe {command} en la consola de la pestaña Máquina"),
        }
    }

    /// Quién envía los comandos de recuperación (nunca el envío de trabajos).
    fn who(self) -> &'static str {
        match self {
            Self::Cli { .. } => {
                "send nunca ejecuta estos comandos por su cuenta; lasersoft cmd pide \
                 confirmación antes de enviarlos"
            }
            Self::Console => {
                "la app nunca ejecuta estos comandos por su cuenta; los envías tú desde la \
                 consola"
            }
        }
    }
}

fn unlock_text(advice: Advice) -> String {
    format!(
        "$X desbloquea la alarma sin mover la máquina, pero no comprueba la posición: úsalo \
         solo si el cabezal está libre y sin obstáculos. Ejecútalo con: {}",
        advice.run("$X")
    )
}

fn home_text(advice: Advice) -> String {
    format!(
        "$H hace homing (el cabezal se mueve hasta los finales de carrera) y recupera la \
         posición de origen. Ejecútalo con: {}",
        advice.run("$H")
    )
}

/// Qué sugerir según la causa de la alarma (si se conoce) y si el homing está activado ($22).
fn alarm_advice(code: Option<u32>, homing: Option<bool>, advice: Advice) -> Vec<String> {
    let line = |s: &str| format!("  - {s}");
    let (unlock, home) = (unlock_text(advice), home_text(advice));
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
/// de `$22` (si se pudo leer). Con los consejos del CLI (`lasersoft cmd --port <port>`).
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
            msg.push_str(&format!("\nQué hacer ({}):\n", advice.who()));
            msg.push_str(&alarm_advice(code, homing, advice).join("\n"));
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
/// No envía ningún comando que cambie el estado (`$X`, `$H`, `~`...). Los consejos son los
/// del CLI; ver [`preflight_with`].
pub fn preflight(
    t: &mut impl Transport,
    config: &PreflightConfig,
    out: &mut impl Write,
) -> Result<(), String> {
    preflight_with(t, config, Advice::Cli { port: &config.port }, out)
}

/// Como [`preflight`], eligiendo dónde se sugiere ejecutar `$X`/`$H`.
pub fn preflight_with(
    t: &mut impl Transport,
    config: &PreflightConfig,
    advice: Advice,
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
    let result = check_ready_with(&report, code, homing, advice);
    // Lo que sobre (otro reporte, el `ok` de `$$`) no debe confundir al streamer.
    let _ = t.clear_input();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serial::memory_pair;

    fn report(line: &str) -> StatusReport {
        StatusReport::parse(line).unwrap()
    }

    #[test]
    fn console_advice_points_to_the_console_not_the_cli() {
        let r = report("<Alarm|MPos:0.000,0.000,0.000|FS:0,0>");
        let e = check_ready_with(&r, None, Some(true), Advice::Console).unwrap_err();
        assert!(e.contains("consola de la pestaña Máquina"), "{e}");
        assert!(e.contains("$H") && e.contains("$X"), "{e}");
        assert!(!e.contains("lasersoft"), "{e}");
        let cli = check_ready(&r, None, Some(true), "/dev/cu.x").unwrap_err();
        assert!(cli.contains("lasersoft cmd --port /dev/cu.x '$H'"), "{cli}");
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
