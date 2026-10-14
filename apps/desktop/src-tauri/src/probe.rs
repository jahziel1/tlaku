//! Autodetección de la controladora: abre un puerto, escucha su saludo y pregunta quién es.
//!
//! # Solo lectura
//!
//! La detección **nunca mueve la máquina ni cambia su configuración**. Lo único que puede salir
//! hacia el puerto, y está fijado en [`send`] (no hay otro camino de escritura en este módulo):
//!
//! - `?` seguido de un salto de línea: informe de estado de GRBL (solo si no hubo saludo);
//! - `0x18` (reinicio suave): solo si ni el saludo ni `?` obtuvieron respuesta;
//! - `$I` (versión y opciones de GRBL), `$$` (lista de ajustes de GRBL) y `M115` (versión de
//!   Marlin y Smoothie). Los tres solo informan.
//!
//! Cualquier otra línea se rechaza antes de escribirla ([`is_read_only_command`]), de modo que ni
//! un error de programación puede mandar un movimiento, `$H`, `$X`, `$N=...`, `$x=valor`,
//! `$RST` ni G-code. Las pruebas de este módulo comprueban, contra el emulador, que lo recibido
//! por la controladora es siempre un subconjunto de esa lista.
//!
//! Abrir un puerto serie puede reiniciar la placa (señal DTR): por eso se espera el saludo antes
//! de preguntar nada. Marlin tarda en volver; se le da el tiempo de [`ProbeTiming`].
//!
//! El puerto puede ser real o virtual (`virtual:<modelo>`): los virtuales se atienden con el
//! emulador de `tlaku-emu`, así que el asistente se puede probar sin máquina.

use serde::Serialize;
use std::collections::BTreeMap;
use std::io;
use std::time::Duration;
use tlaku_doc::DeviceKind;
use tlaku_emu::{Profile, VirtualMachine};
use tlaku_grbl::serial::{LineReader, SerialTransport, Transport};
use tlaku_i18n::tr;

/// Prefijo de los puertos que atiende el emulador.
pub const VIRTUAL_PREFIX: &str = "virtual:";

/// Familia de firmware reconocida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Grbl,
    GrblHal,
    FluidNc,
    GrblLpc,
    Marlin,
    Smoothie,
    Unknown,
}

impl Family {
    fn is_grbl_like(self) -> bool {
        matches!(
            self,
            Self::Grbl | Self::GrblHal | Self::FluidNc | Self::GrblLpc
        )
    }
}

/// Lo que se averiguó del puerto.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProbeResult {
    pub family: Family,
    /// Nombre y versión legibles (`Grbl 1.1f (20220810)`, `Marlin 2.1.2.5`...); vacío si no se supo.
    pub firmware: String,
    /// Todo lo que la placa dijo al abrir el puerto, una línea por renglón.
    pub banner: String,
    /// Respuesta de `$I` o `M115`, sin el `ok` final.
    pub info: Vec<String>,
    /// Ajustes de `$$` (clave con el `$`, por ejemplo `"$30"` -> `"1000"`). Solo en la familia
    /// GRBL y solo si la placa devolvió alguno.
    pub settings: Option<BTreeMap<String, String>>,
    pub suggested_kind: DeviceKind,
}

/// Esperas de la detección. Se miden con el tiempo que se pasa leyendo, no con el reloj de pared,
/// así el emulador (que no duerme) y un puerto real se comportan igual.
#[derive(Debug, Clone)]
pub struct ProbeTiming {
    /// Cuánto se espera el primer byte del saludo (las placas que se reinician tardan 1 a 3 s).
    pub banner_first: Duration,
    /// Silencio que da por terminada una respuesta que ya empezó.
    pub quiet: Duration,
    pub status_wait: Duration,
    pub reset_wait: Duration,
    pub info_wait: Duration,
    pub settings_wait: Duration,
    /// Paso de lectura.
    pub step: Duration,
}

impl Default for ProbeTiming {
    fn default() -> Self {
        Self {
            banner_first: Duration::from_millis(3500),
            quiet: Duration::from_millis(600),
            status_wait: Duration::from_millis(1000),
            reset_wait: Duration::from_millis(3000),
            info_wait: Duration::from_millis(2500),
            settings_wait: Duration::from_millis(4000),
            step: Duration::from_millis(50),
        }
    }
}

// ---------------------------------------------------------------------------
// Lo único que se escribe
// ---------------------------------------------------------------------------

/// Líneas de solo lectura permitidas durante la detección.
const READ_ONLY_LINES: [&str; 3] = ["$I", "$$", "M115"];
const SOFT_RESET: u8 = 0x18;

/// `true` si `line` es un comando de solo lectura de la lista cerrada de la detección.
pub fn is_read_only_command(line: &str) -> bool {
    READ_ONLY_LINES.contains(&line)
}

enum Tx<'a> {
    /// `?` y salto de línea. El salto evita que, en una placa que no sea GRBL, el `?` se pegue
    /// al comando siguiente.
    Status,
    SoftReset,
    Line(&'a str),
}

/// Único punto de escritura del módulo.
fn send(t: &mut dyn Transport, tx: Tx<'_>) -> io::Result<()> {
    match tx {
        Tx::Status => t.write_all(b"?\n"),
        Tx::SoftReset => t.write_all(&[SOFT_RESET]),
        Tx::Line(line) => {
            if !is_read_only_command(line) {
                return Err(io::Error::other(tr!(
                    "desktop.probe.read_only",
                    line = line
                )));
            }
            t.write_all(format!("{line}\n").as_bytes())
        }
    }
}

// ---------------------------------------------------------------------------
// Lectura
// ---------------------------------------------------------------------------

fn is_ack(line: &str) -> bool {
    line == "ok"
        || line.starts_with("ok ")
        || line.to_ascii_lowercase().starts_with("error")
        || line.starts_with("ALARM")
}

/// Lee líneas hasta `max_wait`, o hasta el silencio `quiet` una vez que llegó algo, o (con
/// `stop_on_ack`) hasta el `ok`/`error`. Devuelve también si terminó por `ok`.
fn collect(
    t: &mut dyn Transport,
    reader: &mut LineReader,
    max_wait: Duration,
    quiet: Duration,
    step: Duration,
    stop_on_ack: bool,
) -> io::Result<(Vec<String>, bool)> {
    let mut lines = Vec::new();
    let mut waited = Duration::ZERO;
    let mut silent = Duration::ZERO;
    let mut buf = [0u8; 512];
    loop {
        while let Some(line) = reader.next_line() {
            let ack = is_ack(&line);
            if !(ack && stop_on_ack && line == "ok") {
                lines.push(line);
            }
            if ack && stop_on_ack {
                return Ok((lines, true));
            }
        }
        if waited >= max_wait || (!lines.is_empty() && silent >= quiet) {
            return Ok((lines, false));
        }
        let n = t.read_timeout(&mut buf, step)?;
        if n > 0 {
            reader.feed(&buf[..n]);
            silent = Duration::ZERO;
        } else {
            silent += step;
            waited += step;
        }
    }
}

// ---------------------------------------------------------------------------
// Reconocimiento
// ---------------------------------------------------------------------------

fn any_contains(lines: &[String], needle: &str) -> bool {
    let needle = needle.to_ascii_lowercase();
    lines
        .iter()
        .any(|l| l.to_ascii_lowercase().contains(&needle))
}

/// Familia que dicen unas líneas (saludo más información). Sin afinar con `$30`.
pub fn classify(lines: &[String]) -> Family {
    if any_contains(lines, "grblhal") {
        Family::GrblHal
    } else if any_contains(lines, "fluidnc") {
        Family::FluidNc
    } else if any_contains(lines, "smoothie") {
        Family::Smoothie
    } else if any_contains(lines, "marlin") {
        Family::Marlin
    } else if any_contains(lines, "lpc") && any_grbl_text(lines) {
        Family::GrblLpc
    } else if any_grbl_text(lines) {
        Family::Grbl
    } else {
        Family::Unknown
    }
}

fn any_grbl_text(lines: &[String]) -> bool {
    lines.iter().any(|l| {
        let low = l.to_ascii_lowercase();
        low.starts_with("grbl") || low.starts_with("[ver:") || l.starts_with('<')
    })
}

/// Versión `(mayor, menor, letra)` del saludo `Grbl 1.1f ['$' for help]`.
fn grbl_version(lines: &[String]) -> Option<(u32, u32, Option<char>)> {
    for l in lines {
        let Some(rest) = l.strip_prefix("Grbl ") else {
            continue;
        };
        let token = rest.split_whitespace().next()?;
        let (maj, rest) = token.split_once('.')?;
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        let letter = rest[digits.len()..]
            .chars()
            .next()
            .filter(char::is_ascii_alphabetic);
        return Some((maj.parse().ok()?, digits.parse().ok()?, letter));
    }
    None
}

/// GRBL 1.1e o anterior: sin `M4` ni modo láser.
fn is_pre_laser_grbl(lines: &[String]) -> bool {
    match grbl_version(lines) {
        Some((maj, min, letter)) => {
            maj < 1 || (maj == 1 && (min < 1 || (min == 1 && letter.is_some_and(|c| c < 'f'))))
        }
        None => false,
    }
}

/// Texto que sigue a `key` hasta uno de los `stops` (o el fin).
fn after<'a>(line: &'a str, key: &str, stops: &[&str]) -> Option<&'a str> {
    let start = line.find(key)? + key.len();
    let rest = &line[start..];
    let end = stops
        .iter()
        .filter_map(|s| rest.find(s))
        .min()
        .unwrap_or(rest.len());
    Some(rest[..end].trim().trim_end_matches(','))
}

fn firmware_string(family: Family, banner: &[String], info: &[String]) -> String {
    let all: Vec<&String> = banner.iter().chain(info.iter()).collect();
    let ver_date = || {
        info.iter()
            .find_map(|l| after(l, "[VER:", &["]"]))
            .and_then(|v| v.split(':').next())
            .and_then(|v| v.split_once('.'))
            .and_then(|(_, rest)| rest.split_once('.'))
            .map(|(_, date)| date.to_string())
            .filter(|d| !d.is_empty())
    };
    match family {
        Family::Grbl | Family::GrblLpc | Family::GrblHal => {
            let prefix = if family == Family::GrblHal {
                "GrblHAL "
            } else {
                "Grbl "
            };
            let version = all.iter().find_map(|l| {
                let low = l.to_ascii_lowercase();
                let rest = low.strip_prefix(&prefix.to_ascii_lowercase())?;
                // Respeta las mayúsculas originales de la versión.
                let token = rest.split_whitespace().next()?;
                Some(l[prefix.len()..prefix.len() + token.len()].to_string())
            });
            let base = match version {
                Some(v) => format!("{}{v}", prefix),
                None => prefix.trim().to_string(),
            };
            match ver_date() {
                Some(d) => format!("{base} ({d})"),
                None => base,
            }
        }
        Family::FluidNc => all
            .iter()
            .find_map(|l| {
                let start = l.find("FluidNC")?;
                let mut it = l[start..].split_whitespace();
                let clean = |w: &str| {
                    w.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.')
                        .to_string()
                };
                let name = clean(it.next()?);
                let ver = it.next().map(clean).filter(|v| v.starts_with('v'));
                Some(match ver {
                    Some(v) => format!("{name} {v}"),
                    None => name,
                })
            })
            .unwrap_or_else(|| "FluidNC".into()),
        Family::Marlin => all
            .iter()
            .find_map(|l| {
                after(
                    l,
                    "FIRMWARE_NAME:",
                    &[" SOURCE_CODE_URL", " PROTOCOL_VERSION", " ("],
                )
                .map(str::to_string)
                .or_else(|| {
                    l.strip_prefix("echo:Marlin ")
                        .map(|v| format!("Marlin {}", v.trim()))
                })
            })
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Marlin".into()),
        Family::Smoothie => {
            let version = all
                .iter()
                .find_map(|l| after(l, "FIRMWARE_VERSION:", &[",", " ("]))
                .filter(|v| !v.is_empty());
            match version {
                Some(v) => format!("Smoothieware {v}"),
                None => "Smoothie".into(),
            }
        }
        Family::Unknown => String::new(),
    }
}

/// `$N=valor` -> (`$N`, `valor`). Ignora lo que no tenga esa forma.
fn parse_setting(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix('$')?;
    let (id, value) = rest.split_once('=')?;
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let value = value
        .split(|c: char| c.is_whitespace() || c == '(')
        .next()?
        .trim();
    (!value.is_empty()).then(|| (format!("${id}"), value.to_string()))
}

fn suggested_kind(family: Family, banner: &[String]) -> DeviceKind {
    match family {
        Family::Grbl if is_pre_laser_grbl(banner) => DeviceKind::GrblM3,
        Family::Grbl => DeviceKind::Grbl,
        Family::GrblHal => DeviceKind::GrblHal,
        Family::FluidNc => DeviceKind::FluidNc,
        Family::GrblLpc => DeviceKind::GrblLpc,
        Family::Marlin => DeviceKind::Marlin,
        Family::Smoothie => DeviceKind::Smoothie,
        Family::Unknown => DeviceKind::CustomGcode,
    }
}

// ---------------------------------------------------------------------------
// Sondeo
// ---------------------------------------------------------------------------

/// Detecta la controladora en un canal ya abierto. No mueve ni configura nada (ver el módulo).
pub fn probe_transport(t: &mut dyn Transport, timing: &ProbeTiming) -> io::Result<ProbeResult> {
    let mut reader = LineReader::default();
    let step = timing.step;
    let mut banner = collect(
        t,
        &mut reader,
        timing.banner_first,
        timing.quiet,
        step,
        false,
    )?
    .0;

    if banner.is_empty() {
        // Sin saludo: la placa ya estaba encendida. `?` solo pregunta el estado a GRBL.
        send(t, Tx::Status)?;
        banner = collect(
            t,
            &mut reader,
            timing.status_wait,
            timing.quiet,
            step,
            false,
        )?
        .0;
        if banner.is_empty() {
            // Todavía nada: un reinicio suave hace que GRBL repita su saludo.
            send(t, Tx::SoftReset)?;
            banner = collect(t, &mut reader, timing.reset_wait, timing.quiet, step, false)?.0;
        }
        // Los restos de `?` (`ok`, informe) no son parte del saludo.
        banner.retain(|l| l != "ok");
    }

    let mut family = classify(&banner);
    let info = if family.is_grbl_like() {
        ask(t, &mut reader, timing, "$I", timing.info_wait)?
    } else if matches!(family, Family::Marlin | Family::Smoothie) {
        ask(t, &mut reader, timing, "M115", timing.info_wait)?
    } else {
        // Saludo irreconocible o ausente: probar los dos, primero GRBL.
        let grbl = ask(t, &mut reader, timing, "$I", timing.info_wait)?;
        if classify(&grbl) == Family::Unknown {
            ask(t, &mut reader, timing, "M115", timing.info_wait)?
        } else {
            grbl
        }
    };

    let mut all = banner.clone();
    all.extend(info.iter().cloned());
    family = classify(&all);

    // `$$` solo informa y solo existe en la familia GRBL.
    let mut settings = None;
    if family.is_grbl_like() {
        let lines = ask(t, &mut reader, timing, "$$", timing.settings_wait)?;
        let map: BTreeMap<String, String> = lines.iter().filter_map(|l| parse_setting(l)).collect();
        if !map.is_empty() {
            // GRBL-LPC trae `$30=1.0` de fábrica y casi nada más lo distingue del saludo.
            if family == Family::Grbl
                && map
                    .get("$30")
                    .and_then(|v| v.parse::<f64>().ok())
                    .is_some_and(|v| v > 0.0 && v <= 1.0)
            {
                family = Family::GrblLpc;
            }
            settings = Some(map);
        }
    }

    Ok(ProbeResult {
        family,
        firmware: firmware_string(family, &banner, &info),
        banner: banner.join("\n"),
        suggested_kind: suggested_kind(family, &banner),
        info,
        settings,
    })
}

/// Envía una línea de solo lectura y devuelve la respuesta hasta el `ok`. Un `error:N` se tolera
/// (la placa solo dijo que no entiende el comando) y no devuelve líneas.
fn ask(
    t: &mut dyn Transport,
    reader: &mut LineReader,
    timing: &ProbeTiming,
    cmd: &str,
    wait: Duration,
) -> io::Result<Vec<String>> {
    // Lo que sobró de antes (el `ok` de un `?`) no debe cortar esta respuesta.
    t.clear_input()?;
    reader.clear();
    send(t, Tx::Line(cmd))?;
    let (lines, _) = collect(t, reader, wait, timing.quiet, timing.step, true)?;
    Ok(lines
        .into_iter()
        .filter(|l| !l.to_ascii_lowercase().starts_with("error"))
        .collect())
}

// ---------------------------------------------------------------------------
// Puertos
// ---------------------------------------------------------------------------

/// Perfil del emulador para un puerto `virtual:<modelo>`. Lo que no se reconoce es un GRBL 1.1h.
fn virtual_profile(id: &str) -> Profile {
    let id = id.to_ascii_lowercase();
    if id.contains("falcon") {
        Profile::falcon2_pro()
    } else if id.contains("hal") {
        Profile::grbl_hal()
    } else if id.contains("fluid") {
        Profile::fluidnc()
    } else if id.contains("lpc") {
        Profile::grbl_lpc()
    } else if id.contains("marlin") {
        Profile::marlin()
    } else if id.contains("smoothie") {
        Profile::smoothie()
    } else {
        Profile::grbl_1_1h()
    }
}

fn open_transport(port: &str, baud: u32) -> io::Result<Box<dyn Transport>> {
    if let Some(id) = port.strip_prefix(VIRTUAL_PREFIX) {
        let (vm, _handle) = VirtualMachine::new(virtual_profile(id));
        return Ok(Box::new(vm));
    }
    if crate::netlink::is_network(port) {
        // Dirección de red (`tcp://`, `ws://`): el error ya viene explicado en español.
        return crate::netlink::open_endpoint(port).map_err(io::Error::other);
    }
    Ok(Box::new(SerialTransport::open(port, baud)?))
}

/// Detecta la controladora de `port` a `baud` baudios.
pub fn probe(port: &str, baud: u32, timing: &ProbeTiming) -> Result<ProbeResult, String> {
    if baud == 0 {
        return Err(tr!("desktop.common.baud_positive"));
    }
    let network = crate::netlink::is_network(port);
    let mut transport = open_transport(port, baud).map_err(|e| {
        if network {
            return e.to_string();
        }
        tr!("desktop.commands.open_port_failed", port = port, err = e)
    })?;
    probe_transport(&mut *transport, timing)
        .map_err(|e| tr!("desktop.probe.comm_lost", port = port, err = e))
}

/// Comando de Tauri: ver [`probe`]. Corre en un hilo aparte porque espera a la placa.
#[tauri::command]
pub async fn probe_port(port: String, baud: u32) -> Result<ProbeResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        // Por red no hay reinicio de la placa: basta una espera corta del saludo.
        let timing = if crate::netlink::is_network(&port) {
            ProbeTiming {
                banner_first: Duration::from_millis(800),
                ..ProbeTiming::default()
            }
        } else {
            ProbeTiming::default()
        };
        probe(&port, baud, &timing)
    })
    .await
    .map_err(|e| tr!("desktop.probe.interrupted", err = e))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_emu::Received;

    fn run(profile: Profile) -> (ProbeResult, Vec<String>, Vec<u8>) {
        let (mut vm, handle) = VirtualMachine::new(profile);
        let r = probe_transport(&mut vm, &ProbeTiming::default()).unwrap();
        (r, handle.received_lines(), handle.realtime_bytes())
    }

    /// Lo recibido es siempre de la lista cerrada: nunca movimiento, `$H`, `$X` ni escritura.
    fn assert_read_only(lines: &[String], realtime: &[u8]) {
        for l in lines {
            assert!(
                l.is_empty() || is_read_only_command(l) || l == "?",
                "la detección envió una línea no permitida: {l:?}"
            );
            for bad in [
                "$H", "$X", "G0", "G1", "$RST", "$N", "M3", "M4", "$32=", "$30=",
            ] {
                assert!(!l.starts_with(bad), "línea prohibida: {l:?}");
            }
        }
        for b in realtime {
            assert!(
                *b == b'?' || *b == SOFT_RESET,
                "byte en tiempo real no permitido: {b:#x}"
            );
        }
    }

    #[test]
    fn falcon2_pro_is_grbl_with_creality_banner_and_settings() {
        let (r, lines, rt) = run(Profile::falcon2_pro());
        assert_eq!(r.family, Family::Grbl);
        assert_eq!(r.suggested_kind, DeviceKind::Grbl);
        assert_eq!(r.firmware, "Grbl 1.1f (20220810)");
        assert!(r.banner.contains("CV50-Pro-MASTER"));
        assert!(r.info.iter().any(|l| l.starts_with("[VER:1.1f")));
        let s = r.settings.unwrap();
        assert_eq!(s["$30"], "1000");
        assert_eq!(s["$32"], "1");
        assert_eq!(s["$131"], "400.000");
        assert_eq!(s["$110"], "50000.000");
        assert_eq!(lines, ["$I", "$$"]);
        assert_read_only(&lines, &rt);
    }

    #[test]
    fn classic_grbl_reads_version_and_settings() {
        let (r, lines, rt) = run(Profile::grbl_1_1h());
        assert_eq!(r.family, Family::Grbl);
        assert_eq!(r.firmware, "Grbl 1.1h (20190825)");
        assert_eq!(r.settings.as_ref().unwrap()["$32"], "1");
        assert_read_only(&lines, &rt);
    }

    #[test]
    fn grbl_hal_and_fluidnc_and_lpc_are_told_apart() {
        let (hal, lines, rt) = run(Profile::grbl_hal());
        assert_eq!(hal.family, Family::GrblHal);
        assert_eq!(hal.suggested_kind, DeviceKind::GrblHal);
        assert!(hal.firmware.starts_with("GrblHAL 1.1f"));
        assert!(hal.settings.is_some());
        assert_read_only(&lines, &rt);

        let (fnc, lines, rt) = run(Profile::fluidnc());
        assert_eq!(fnc.family, Family::FluidNc);
        assert_eq!(fnc.suggested_kind, DeviceKind::FluidNc);
        assert_eq!(fnc.firmware, "FluidNC v3.9.7");
        assert_read_only(&lines, &rt);

        // GRBL-LPC se reconoce por su `$30=1.0` de fábrica (su saludo es el de GRBL).
        let (lpc, lines, rt) = run(Profile::grbl_lpc());
        assert_eq!(lpc.family, Family::GrblLpc);
        assert_eq!(lpc.suggested_kind, DeviceKind::GrblLpc);
        assert_read_only(&lines, &rt);
    }

    #[test]
    fn marlin_and_smoothie_use_m115_and_have_no_settings() {
        let (m, lines, rt) = run(Profile::marlin());
        assert_eq!(m.family, Family::Marlin);
        assert_eq!(m.suggested_kind, DeviceKind::Marlin);
        assert_eq!(m.firmware, "Marlin 2.1.2.5");
        assert!(m.settings.is_none());
        assert_eq!(lines, ["M115"]);
        assert_read_only(&lines, &rt);

        let (s, lines, rt) = run(Profile::smoothie());
        assert_eq!(s.family, Family::Smoothie);
        assert_eq!(s.suggested_kind, DeviceKind::Smoothie);
        assert!(s.firmware.starts_with("Smoothieware"));
        assert!(s.settings.is_none());
        assert_eq!(lines, ["M115"]);
        assert_read_only(&lines, &rt);
    }

    #[test]
    fn every_emulator_profile_is_probed_read_only() {
        for p in Profile::all() {
            let name = p.name.clone();
            let (r, lines, rt) = run(p);
            assert_ne!(r.family, Family::Unknown, "{name}");
            assert_read_only(&lines, &rt);
        }
    }

    #[test]
    fn nothing_but_the_allow_list_can_be_written() {
        let (mut vm, handle) = VirtualMachine::new(Profile::grbl_1_1h());
        for bad in [
            "$H",
            "$X",
            "G0 X10",
            "G1 X1 F100",
            "$32=0",
            "$RST=*",
            "M3 S1000",
            "$I=hola",
            "$N0=G0",
        ] {
            assert!(send(&mut vm, Tx::Line(bad)).is_err(), "{bad}");
            assert!(!is_read_only_command(bad), "{bad}");
        }
        assert!(handle.received().is_empty());
        for ok in ["$I", "$$", "M115"] {
            assert!(send(&mut vm, Tx::Line(ok)).is_ok());
        }
    }

    #[test]
    fn a_silent_board_gets_status_then_soft_reset_and_still_nothing_else() {
        // Banner de conexión apagado: la placa solo habla cuando se le pregunta.
        let (mut vm, handle) = VirtualMachine::builder(Profile::grbl_1_1h())
            .banner_on_connect(false)
            .build();
        let r = probe_transport(&mut vm, &ProbeTiming::default()).unwrap();
        assert_eq!(r.family, Family::Grbl);
        assert!(r.settings.is_some());
        let rt = handle.realtime_bytes();
        // El `\n` que acompaña a `?` llega como línea vacía.
        let sent: Vec<String> = handle
            .received_lines()
            .into_iter()
            .filter(|l| !l.is_empty())
            .collect();
        assert_eq!(sent, ["$I", "$$"]);
        assert_read_only(&handle.received_lines(), &rt);
        // Sí contestó a `?`, así que no hizo falta reiniciarla.
        assert!(!rt.contains(&SOFT_RESET), "{rt:?}");
        assert!(
            handle
                .received()
                .iter()
                .all(|e| matches!(&e.what, Received::Line(_) | Received::Realtime(_)))
        );
    }

    #[test]
    fn a_dead_port_is_unknown_custom_gcode_and_only_reads() {
        // Un canal en memoria cuyo otro extremo escucha sin contestar.
        let (mut a, mut b) = tlaku_grbl::serial::memory_pair();
        let timing = ProbeTiming {
            banner_first: Duration::from_millis(100),
            quiet: Duration::from_millis(50),
            status_wait: Duration::from_millis(100),
            reset_wait: Duration::from_millis(100),
            info_wait: Duration::from_millis(100),
            settings_wait: Duration::from_millis(100),
            step: Duration::from_millis(10),
        };
        let r = probe_transport(&mut a, &timing).unwrap();
        assert_eq!(r.family, Family::Unknown);
        assert_eq!(r.suggested_kind, DeviceKind::CustomGcode);
        assert!(r.settings.is_none() && r.firmware.is_empty());
        let mut got = Vec::new();
        let mut buf = [0u8; 64];
        loop {
            let n = b.read_timeout(&mut buf, Duration::from_millis(1)).unwrap();
            if n == 0 {
                break;
            }
            got.extend_from_slice(&buf[..n]);
        }
        // `?\n`, `0x18`, `$I\n`, `M115\n` y nada más.
        assert_eq!(got, b"?\n\x18$I\nM115\n");
    }

    #[test]
    fn a_pre_1_1f_grbl_suggests_the_m3_kind() {
        let lines = vec!["Grbl 0.9j ['$' for help]".to_string()];
        assert_eq!(suggested_kind(classify(&lines), &lines), DeviceKind::GrblM3);
        let lines = vec!["Grbl 1.1e ['$' for help]".to_string()];
        assert_eq!(suggested_kind(classify(&lines), &lines), DeviceKind::GrblM3);
        let lines = vec!["Grbl 1.1f ['$' for help]".to_string()];
        assert_eq!(suggested_kind(classify(&lines), &lines), DeviceKind::Grbl);
        let lines = vec!["Grbl 1.3a [ESP32]".to_string()];
        assert_eq!(suggested_kind(classify(&lines), &lines), DeviceKind::Grbl);
    }

    #[test]
    fn settings_lines_are_parsed_with_or_without_descriptions() {
        assert_eq!(
            parse_setting("$30=1000"),
            Some(("$30".into(), "1000".into()))
        );
        assert_eq!(
            parse_setting("$130=400.000 (x max travel, mm)"),
            Some(("$130".into(), "400.000".into()))
        );
        assert_eq!(parse_setting("$I"), None);
        assert_eq!(parse_setting("[VER:1.1f:]"), None);
        assert_eq!(parse_setting("$N0="), None);
    }

    #[test]
    fn virtual_ports_pick_their_profile_and_real_open_errors_are_spanish() {
        assert_eq!(
            virtual_profile("falcon2-pro").family,
            tlaku_emu::Family::Falcon2
        );
        assert_eq!(virtual_profile("marlin").family, tlaku_emu::Family::Marlin);
        assert_eq!(
            virtual_profile("lo-que-sea").family,
            tlaku_emu::Family::Grbl
        );
        let r = probe("virtual:grbl", 115_200, &ProbeTiming::default()).unwrap();
        assert_eq!(r.family, Family::Grbl);
        assert!(probe("virtual:grbl", 0, &ProbeTiming::default()).is_err());
        let e = probe("/dev/no-existe-tlaku", 115_200, &ProbeTiming::default()).unwrap_err();
        assert!(e.starts_with("No se pudo abrir"), "{e}");
    }
}
