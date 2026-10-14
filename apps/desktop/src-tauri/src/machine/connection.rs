//! [`Connection`] (hilo del trabajador sobre un puerto) e identificación del firmware (`$I`, `M115`).

use super::*;
use tlaku_i18n::tr;

/// Conexión abierta: el mando y el hilo del trabajador.
pub struct Connection {
    handle: Handle,
    thread: Option<JoinHandle<()>>,
}

impl Connection {
    /// Identifica la controladora con `$I` y arranca el trabajador. `transport` es un puerto
    /// ya abierto (con la espera de reinicio de la placa cumplida).
    pub fn start(
        transport: Box<dyn Transport>,
        port: &str,
        sink: Arc<dyn Sink>,
        timing: Timing,
    ) -> Result<(Self, ConnectResult), String> {
        Self::start_with(transport, port, sink, timing, None)
    }

    /// Abre la máquina virtual de un puerto `virtual:...` (ver [`virtual_port`]).
    pub fn start_virtual(
        port: &str,
        sink: Arc<dyn Sink>,
        timing: Timing,
    ) -> Result<(Self, ConnectResult), String> {
        let (transport, emu) = virtual_port::open(port)?;
        Self::start_with(transport, port, sink, timing, Some(emu))
    }

    /// Como [`start`](Self::start), con el mando del emulador si `transport` es uno.
    pub fn start_with(
        transport: Box<dyn Transport>,
        port: &str,
        sink: Arc<dyn Sink>,
        timing: Timing,
        emu: Option<tlaku_emu::Handle>,
    ) -> Result<(Self, ConnectResult), String> {
        let mut grbl = Grbl::with_transport(transport);
        let reply = grbl.command("$I", timing.identify_timeout);
        log(sink.as_ref(), LogDir::Out, "$I");
        // Marlin y Smoothieware no conocen `$I` (`echo:Unknown command` o `error`): se prueba
        // `M115`. Si tampoco contesta, vale el mensaje de siempre.
        let answered_like_grbl = matches!(&reply, Ok(l) if !looks_like_reprap(l));
        let reprap = if answered_like_grbl {
            None
        } else {
            if let Ok(l) = &reply {
                for l in l {
                    log(sink.as_ref(), LogDir::In, l);
                }
            }
            probe_reprap(&mut grbl, sink.as_ref(), timing.identify_timeout)
        };
        let (firmware, family, link_family, emergency_parser) = match reprap {
            Some(r) => (r.name, Firmware::Unknown, r.family, r.emergency_parser),
            None => {
                let lines = reply.map_err(|e| tr!("desktop.machine.identify_failed", err = e))?;
                for l in &lines {
                    log(sink.as_ref(), LogDir::In, l);
                }
                log(sink.as_ref(), LogDir::In, "ok");
                let (name, fw) = identify(&lines);
                (name, fw, LinkFamily::Grbl, false)
            }
        };
        if emu.is_some() {
            log(
                sink.as_ref(),
                LogDir::In,
                &tr!("desktop.machine.virtual_note"),
            );
        }

        let shared = Arc::new(Shared::default());
        let (tx, rx) = mpsc::channel();
        let mut worker = Worker::new(rx, sink.clone(), shared.clone(), timing, port.to_string());
        worker.fw = family;
        worker.link_family = link_family;
        worker.emergency_parser = emergency_parser;
        let transport = grbl.into_transport();
        sink.emit(Emit::Connection(ConnectionEvent {
            connected: true,
            port: Some(port.to_string()),
            error: None,
        }));
        let thread = thread::Builder::new()
            .name("tlaku-machine".into())
            .spawn(move || worker.run(transport))
            .map_err(|e| tr!("desktop.machine.thread_failed", err = e))?;
        Ok((
            Self {
                handle: Handle { tx, shared, emu },
                thread: Some(thread),
            },
            ConnectResult { firmware },
        ))
    }

    pub fn handle(&self) -> Handle {
        self.handle.clone()
    }

    /// Cierra la conexión (cancelando antes un trabajo en curso) y espera al hilo.
    pub fn close(mut self) {
        let _ = self.handle.tx.send(Request::Disconnect);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // Si no se llamó a `close`, al menos se le pide al trabajador que termine.
        let _ = self.handle.tx.send(Request::Disconnect);
    }
}

/// Nombre legible del firmware (`Grbl 1.1h`, `FluidNC v3.9.7`, `grblHAL 1.1f`) y su familia, a
/// partir de las líneas de `$I`. La familia decide cómo se leen los códigos `error:N` y
/// `ALARM:N` extendidos. Se toleran líneas ajenas (`[software versions: ...]`,
/// `inquire_info...` de Creality) y la respuesta vacía.
pub(super) fn identify(lines: &[String]) -> (String, Firmware) {
    let fw = Firmware::detect(lines);
    let generic = match fw {
        Firmware::GrblHal => "grblHAL",
        Firmware::FluidNc => "FluidNC",
        Firmware::Grbl | Firmware::Unknown => "GRBL",
    };
    let Some(ver) = lines.iter().find_map(|l| l.trim().strip_prefix("[VER:")) else {
        return (generic.into(), fw);
    };
    let ver = ver
        .trim_end_matches(']')
        .split(':')
        .next()
        .unwrap_or("")
        .trim();
    let ver = match ver.rsplit_once('.') {
        Some((head, build)) if build.len() == 8 && build.bytes().all(|b| b.is_ascii_digit()) => {
            head
        }
        _ => ver,
    };
    let name = match fw {
        // `3.9 FluidNC v3.9.7` -> `FluidNC v3.9.7`.
        Firmware::FluidNc => {
            let rest = ver
                .to_ascii_lowercase()
                .find("fluidnc")
                .map_or(ver, |i| &ver[i + "fluidnc".len()..])
                .trim();
            if rest.is_empty() {
                generic.to_string()
            } else {
                format!("{generic} {rest}")
            }
        }
        _ if ver.is_empty() => generic.to_string(),
        Firmware::GrblHal => format!("grblHAL {ver}"),
        Firmware::Grbl | Firmware::Unknown => format!("Grbl {ver}"),
    };
    (name, fw)
}

/// `Grbl 1.1h` a partir de la línea `[VER:1.1h.20190825:]` de `$I`.
#[cfg(test)]
pub(super) fn firmware_name(lines: &[String]) -> String {
    identify(lines).0
}

/// ¿La respuesta a `$I` es la de un firmware RepRap (Marlin, Smoothieware) que no lo conoce?
pub(super) fn looks_like_reprap(lines: &[String]) -> bool {
    !lines.iter().any(|l| l.trim_start().starts_with("[VER:"))
        && lines.iter().any(|l| l.starts_with("echo:"))
}

/// Lo que se averigua de Marlin o Smoothieware con `M115`.
pub(super) struct Reprap {
    name: String,
    family: LinkFamily,
    /// Informa `Cap:EMERGENCY_PARSER:1`: `M0` y `M108` actúan al instante.
    emergency_parser: bool,
}

/// Pregunta `M115` y reconoce Marlin o Smoothieware por su `FIRMWARE_NAME`.
pub(super) fn probe_reprap(grbl: &mut Grbl, sink: &dyn Sink, timeout: Duration) -> Option<Reprap> {
    log(sink, LogDir::Out, "M115");
    let lines = grbl.command("M115", timeout).ok()?;
    for l in &lines {
        log(sink, LogDir::In, l);
    }
    log(sink, LogDir::In, "ok");
    let info = ReprapInfo::parse(&lines);
    let name = info.firmware_name.clone()?;
    let low = name.to_ascii_lowercase();
    let family = if low.contains("smoothie") {
        LinkFamily::Smoothie
    } else if low.contains("marlin") {
        LinkFamily::Marlin
    } else {
        return None;
    };
    Some(Reprap {
        name,
        family,
        emergency_parser: info.caps.get("EMERGENCY_PARSER").copied().unwrap_or(false),
    })
}

/// `X:12.00 Y:3.50 Z:0.00 E:0.00 Count ...` (respuesta de `M114`) -> [x, y].
pub(super) fn parse_m114(line: &str) -> Option<[f64; 2]> {
    let field = |key: &str| -> Option<f64> {
        let i = line.find(key)? + key.len();
        let rest = &line[i..];
        let end = rest
            .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+')))
            .unwrap_or(rest.len());
        rest[..end].parse().ok()
    };
    // `Count X:` también contiene "X:": solo vale la primera aparición, que va antes.
    Some([field("X:")?, field("Y:")?])
}

/// ¿Es la confirmación de una línea? GRBL: `ok`. Marlin puede añadir texto (`ok T:20`).
pub(super) fn is_ack(family: LinkFamily, line: &str) -> bool {
    line == "ok" || (family != LinkFamily::Grbl && line.starts_with("ok "))
}
