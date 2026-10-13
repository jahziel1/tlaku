//! Subcomando `tlaku ruida`: exporta un trabajo a `.rd`, lo envía a una controladora Ruida por UDP
//! (`send`) y lee su estado sin tocar nada (`status`).
//! EXPERIMENTAL: no se ha probado con ninguna máquina real (ver docs/hardware/ruida.md).

use crate::cli::DesignArgs;
use crate::{job, link, report, sigint};
use clap::{Args, Subcommand};
use std::io::{self, BufRead, Write};
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tlaku_pipeline::ComputedJob;
use tlaku_ruida::cmd;
use tlaku_ruida::link::{LOCAL_PORT, PacketLink, REMOTE_PORT, UdpLink};
use tlaku_ruida::rd::{self, HomeCorner, LayerOptions, RuidaJobOptions};
use tlaku_ruida::safety::SafetyLimits;
use tlaku_ruida::session::{
    ProbeInfo, Progress, RuidaSession, SendMode, SessionConfig, SessionError,
};
use tlaku_ruida::swizzle::{MAGIC_DEFAULT, Magic};

/// Aviso que acompaña a todo lo que toca una máquina real.
const EXPERIMENTAL: &str = "EXPERIMENTAL: el soporte Ruida no se ha probado con una máquina real \
(docs/hardware/ruida.md). Prueba primero con el láser deshabilitado o a potencia 0.";

#[derive(Args, Clone)]
pub struct NetArgs {
    /// Clave del swizzle (hex, p. ej. 0x88). Si se omite se prueba 0x88 y, si la máquina no
    /// contesta, los demás magics conocidos (el ACK de la máquina delata el suyo)
    #[arg(long, value_parser = parse_magic)]
    magic: Option<u8>,
    /// Esquina de la cama donde está el home de la máquina: top-left, top-right, bottom-left o
    /// bottom-right (define el origen y el signo de X e Y del archivo)
    #[arg(long, default_value = "top-left", value_parser = parse_home)]
    home: HomeCorner,
    /// Espera del ACK de cada paquete, en milisegundos
    #[arg(long, default_value_t = 1000, value_parser = clap::value_parser!(u64).range(50..=60_000))]
    timeout_ms: u64,
    /// Puerto UDP de la controladora (solo para pruebas con un emulador)
    #[arg(long, default_value_t = REMOTE_PORT, hide = true)]
    port: u16,
    /// Puerto UDP local en el que contesta la controladora (0 = uno cualquiera)
    #[arg(long, default_value_t = LOCAL_PORT, hide = true)]
    local_port: u16,
}

#[derive(Subcommand)]
pub enum RuidaCommand {
    /// Convierte un SVG, DXF o imagen en un archivo `.rd` (experimental, sin probar en hardware).
    /// Una capa de la controladora por cada bloque del trabajo; los rellenos e imágenes llevan
    /// potencia variable (`C7`) con el overscan como saltos con el láser apagado.
    ExportRd {
        #[command(flatten)]
        design: DesignArgs,
        /// Archivo `.rd` de salida
        output: PathBuf,
        /// Clave del swizzle (hex, 0x88 por defecto; 0x11 para 634XG, 0x38 para RDL9635)
        #[arg(long, default_value = "0x88", value_parser = parse_magic)]
        magic: u8,
        /// Escribe el flujo en claro (sin swizzle), útil para depurar
        #[arg(long)]
        plain: bool,
        /// Imprime el desensamblado del archivo generado
        #[arg(long)]
        disasm: bool,
        /// Esquina de la cama donde está el home: top-left (por defecto), top-right,
        /// bottom-left o bottom-right
        #[arg(long, default_value = "top-left", value_parser = parse_home)]
        home: HomeCorner,
    },
    /// Envía un diseño a una controladora Ruida por UDP (experimental, sin probar en hardware).
    /// Sondea la máquina (solo lectura), muestra qué se va a hacer y pide confirmación. Por
    /// defecto el trabajo se guarda como documento y NO arranca (se inicia desde el panel de la
    /// máquina); con --run empieza a ejecutarse en cuanto llega. Si el envío falla a medias se
    /// manda Detener y se avisa. Ctrl+C cancela con el mismo cuidado.
    Send {
        /// IP de la controladora (en la misma red /24 que este equipo)
        ip: IpAddr,
        #[command(flatten)]
        design: DesignArgs,
        #[command(flatten)]
        net: NetArgs,
        /// Ejecutar el trabajo al recibirlo en vez de guardarlo como documento
        #[arg(long)]
        run: bool,
        /// Nombre del documento guardado (1 a 40 caracteres ASCII; por defecto el del archivo)
        #[arg(long)]
        name: Option<String>,
        /// Prepara y revisa el trabajo sin conectar con la máquina
        #[arg(long)]
        dry_run: bool,
        /// No pedir confirmación antes de enviar
        #[arg(short, long)]
        yes: bool,
    },
    /// Lee el estado de una controladora Ruida: modelo, cama, estado y posición. Solo lectura:
    /// no mueve nada, no enciende el láser ni escribe en la memoria de la máquina.
    Status {
        /// IP de la controladora (en la misma red /24 que este equipo)
        ip: IpAddr,
        #[command(flatten)]
        net: NetArgs,
    },
}

fn parse_magic(s: &str) -> Result<u8, String> {
    let t = s.trim().trim_start_matches("0x").trim_start_matches("0X");
    u8::from_str_radix(t, 16).map_err(|_| format!("'{s}' no es un byte en hexadecimal (ej. 0x88)"))
}

fn parse_home(s: &str) -> Result<HomeCorner, String> {
    HomeCorner::parse(s).ok_or_else(|| {
        format!("'{s}' no es una esquina: usa top-left, top-right, bottom-left o bottom-right")
    })
}

/// Genera los bytes del `.rd` (en claro) de un trabajo ya calculado. Usa el color de cada capa y
/// la cama indicada; los demás ajustes son los de [`RuidaJobOptions`] por defecto.
pub fn export_rd(
    built: &ComputedJob,
    bed: (f64, f64),
    home: HomeCorner,
) -> Result<Vec<u8>, String> {
    let opts = RuidaJobOptions {
        limits: SafetyLimits {
            bed_mm: [bed.0, bed.1],
            ..SafetyLimits::default()
        },
        layers: built
            .layers
            .iter()
            .map(|l| LayerOptions {
                color: [l.color.0, l.color.1, l.color.2],
                min_power_pct: None,
            })
            .collect(),
        home,
        ..RuidaJobOptions::default()
    };
    rd::build_rd(&built.toolpath, &opts).map_err(|e| e.to_string())
}

pub fn run(action: RuidaCommand) -> Result<(), String> {
    match action {
        RuidaCommand::ExportRd {
            design,
            output,
            magic,
            plain,
            disasm,
            home,
        } => {
            let request = design.into_request()?;
            let built = job::build(&request)?;
            tlaku_pipeline::check_bounds(&built, [request.bed.0, request.bed.1])?;
            let bytes = export_rd(&built, request.bed, home)?;
            if disasm {
                print!("{}", cmd::disassemble(&bytes).map_err(|e| e.to_string())?);
            }
            let out = if plain {
                bytes.clone()
            } else {
                rd::to_swizzled(&bytes, Magic(magic))
            };
            std::fs::write(&output, &out).map_err(|e| format!("{}: {e}", output.display()))?;
            print!("{}", report::summary(&built));
            let kind = if plain {
                "en claro".to_string()
            } else {
                format!("swizzle 0x{magic:02X}")
            };
            println!(
                "Escrito {} ({} bytes, {kind}, home {}). EXPERIMENTAL: sin probar con hardware.",
                output.display(),
                out.len(),
                home.name()
            );
            if magic != MAGIC_DEFAULT && !plain {
                println!("Aviso: se usó un magic distinto de 0x88.");
            }
            Ok(())
        }
        RuidaCommand::Send {
            ip,
            design,
            net,
            run,
            name,
            dry_run,
            yes,
        } => {
            let request = design.into_request()?;
            let built = job::build(&request)?;
            tlaku_pipeline::check_bounds(&built, [request.bed.0, request.bed.1])?;
            let plan = SendPlan::new(&request.input, request.bed, run, name, yes)?;
            let mut stdout = io::stdout();
            let bytes = export_rd(&built, request.bed, net.home)?;
            if dry_run {
                return dry_run_report(&built, &bytes, &plan, &net, &mut stdout);
            }
            let link = open_link(ip, &net)?;
            send_with(
                link,
                &built,
                &bytes,
                &plan,
                &net,
                sigint::install(),
                &mut io::stdin().lock(),
                &mut stdout,
            )
        }
        RuidaCommand::Status { ip, net } => {
            let link = open_link(ip, &net)?;
            status_with(link, &net, &mut io::stdout())
        }
    }
}

fn open_link(ip: IpAddr, net: &NetArgs) -> Result<UdpLink, String> {
    UdpLink::to_controller_ports(ip, net.port, net.local_port).map_err(|e| e.to_string())
}

/// Lo que el usuario pidió enviar y cómo.
pub struct SendPlan {
    pub bed: (f64, f64),
    pub mode: SendMode,
    pub yes: bool,
}

impl SendPlan {
    pub fn new(
        input: &std::path::Path,
        bed: (f64, f64),
        run: bool,
        name: Option<String>,
        yes: bool,
    ) -> Result<Self, String> {
        let mode = if run {
            SendMode::Run
        } else {
            SendMode::Store {
                name: match name {
                    Some(n) => n,
                    None => default_doc_name(input),
                },
            }
        };
        if let SendMode::Store { name } = &mode
            && (name.is_empty()
                || name.len() > 40
                || name.bytes().any(|b| !(0x20..0x7F).contains(&b)))
        {
            return Err(
                "el nombre del documento debe tener de 1 a 40 caracteres ASCII imprimibles".into(),
            );
        }
        Ok(SendPlan { bed, mode, yes })
    }
}

/// Nombre del documento a partir del archivo: mayúsculas ASCII y sin símbolos raros.
fn default_doc_name(input: &std::path::Path) -> String {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("TLAKU");
    let name: String = stem
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-' | '.'))
        .take(40)
        .collect::<String>()
        .to_ascii_uppercase();
    if name.is_empty() {
        "TLAKU".into()
    } else {
        name
    }
}

fn session_config(net: &NetArgs, bed: (f64, f64)) -> SessionConfig {
    SessionConfig {
        magic: Magic(net.magic.unwrap_or(MAGIC_DEFAULT)),
        // Con --magic explícito no se prueban otros: el usuario sabe cuál es.
        probe_all_magics: net.magic.is_none(),
        ack_timeout: Duration::from_millis(net.timeout_ms),
        reply_timeout: Duration::from_millis(net.timeout_ms),
        limits: SafetyLimits {
            bed_mm: [bed.0, bed.1],
            ..SafetyLimits::default()
        },
        home: net.home,
        ..SessionConfig::default()
    }
}

fn show<W: Write>(out: &mut W, text: &str) -> Result<(), String> {
    writeln!(out, "{text}").map_err(|e| e.to_string())
}

fn describe_probe(info: &ProbeInfo) -> String {
    format!(
        "Máquina: {} (tarjeta 0x{:08X}), magic 0x{:02X}{}, cama {}",
        info.model.unwrap_or("modelo desconocido"),
        info.card_id,
        info.magic.0,
        if info.magic_changed {
            " (detectado)"
        } else {
            ""
        },
        match info.bed_mm {
            Some([w, h]) => format!("{w:.1} x {h:.1} mm"),
            None => "no informada".into(),
        }
    )
}

fn session_error(e: SessionError) -> String {
    match &e {
        SessionError::Timeout { .. } => format!(
            "{e}. Comprueba la IP, que el equipo esté en la misma red /24 que la máquina, que \
             ningún otro programa de láser tenga abierto el puerto, y el magic (--magic)"
        ),
        _ => e.to_string(),
    }
}

/// `tlaku ruida status`: solo lectura (consultas `DA 00`).
pub fn status_with<L: PacketLink, W: Write>(
    link: L,
    net: &NetArgs,
    out: &mut W,
) -> Result<(), String> {
    let mut session = RuidaSession::new(link, session_config(net, (0.0, 0.0)));
    let info = session.probe().map_err(session_error)?;
    show(out, &describe_probe(&info))?;
    let st = session.status().map_err(session_error)?;
    show(out, &format!("Estado: {}", st.describe_state()))?;
    show(
        out,
        &format!(
            "Posición: X {:.3} mm, Y {:.3} mm (ejes de Tlaku con el home en {}); en la máquina {:.3}, {:.3} mm",
            st.pos_mm.x,
            st.pos_mm.y,
            net.home.name(),
            st.pos_um.0 as f64 / 1000.0,
            st.pos_um.1 as f64 / 1000.0
        ),
    )?;
    show(
        out,
        "Solo se enviaron lecturas. Nada se movió ni se escribió.",
    )?;
    Ok(())
}

fn dry_run_report<W: Write>(
    built: &ComputedJob,
    bytes: &[u8],
    plan: &SendPlan,
    net: &NetArgs,
    out: &mut W,
) -> Result<(), String> {
    show(out, report::summary(built).trim_end())?;
    let packets = tlaku_ruida::packet::split_into_packets(
        bytes,
        &tlaku_ruida::packet::PacketConfig::default(),
    )
    .map_err(|e| e.to_string())?;
    show(
        out,
        &format!(
            "Trabajo .rd de {} bytes en {} paquete(s), home {}, {}.",
            bytes.len(),
            packets.len(),
            net.home.name(),
            mode_text(&plan.mode)
        ),
    )?;
    show(out, "Simulacro (--dry-run): no se conectó con la máquina.")
}

fn mode_text(mode: &SendMode) -> String {
    match mode {
        SendMode::Run => "se ejecutará al recibirlo".into(),
        SendMode::Store { name } => format!("se guardará como documento \"{name}\" (no arranca)"),
    }
}

/// `tlaku ruida send` sobre un enlace ya abierto (UDP real o en memoria para las pruebas).
#[allow(clippy::too_many_arguments)]
pub fn send_with<L: PacketLink, R: BufRead, W: Write>(
    link: L,
    built: &ComputedJob,
    bytes: &[u8],
    plan: &SendPlan,
    net: &NetArgs,
    cancel: &AtomicBool,
    input: &mut R,
    out: &mut W,
) -> Result<(), String> {
    show(out, report::summary(built).trim_end())?;
    show(out, EXPERIMENTAL)?;
    let mut session = RuidaSession::new(link, session_config(net, plan.bed));
    let info = session.probe().map_err(session_error)?;
    show(out, &describe_probe(&info))?;
    if let Some([w, h]) = info.bed_mm
        && ((w - plan.bed.0).abs() > 1.0 || (h - plan.bed.1).abs() > 1.0)
    {
        return Err(format!(
            "la cama que informa la máquina ({w:.1} x {h:.1} mm) no coincide con --bed \
             ({:.1} x {:.1} mm): el origen del archivo depende de ella. Repite con --bed {w:.0}x{h:.0}",
            plan.bed.0, plan.bed.1
        ));
    }
    let st = session.status().map_err(session_error)?;
    show(out, &format!("Estado: {}", st.describe_state()))?;
    if !st.is_idle() {
        return Err(session_error(SessionError::Busy(st)));
    }
    show(
        out,
        &format!(
            "Se enviarán {} bytes de trabajo: {}. Home en {}.",
            bytes.len(),
            mode_text(&plan.mode),
            net.home.name()
        ),
    )?;
    if !plan.yes {
        let prompt = match plan.mode {
            SendMode::Run => {
                "El trabajo EMPEZARÁ a ejecutarse en cuanto llegue. ¿El láser está listo y el área \
                 despejada? [s/N] "
            }
            SendMode::Store { .. } => "¿Guardar el documento en la máquina? [s/N] ",
        };
        let ok = link::confirm(prompt, input, out).map_err(|e| e.to_string())?;
        if !ok {
            return Err("cancelado: no se envió nada".into());
        }
    }
    let mut last_step = usize::MAX;
    let mut progress = |p: Progress| {
        let pct = p.bytes_sent * 100 / p.bytes_total.max(1);
        let step = pct / 10;
        if step != last_step || p.packet == p.packets {
            last_step = step;
            let _ = writeln!(out, "  {pct:>3} %  (paquete {}/{})", p.packet, p.packets);
        }
    };
    match session.send_job(bytes, &plan.mode, cancel, &mut progress) {
        Ok(r) => {
            show(
                out,
                &format!(
                    "Enviado: {} paquete(s), {} bytes, {} reenvío(s).",
                    r.packets, r.bytes, r.resends
                ),
            )?;
            match plan.mode {
                SendMode::Run => show(
                    out,
                    "El trabajo se está ejecutando. Pausa/Stop desde el panel de la máquina; el \
                     paro de emergencia sigue siendo lo primero en caso de problema.",
                ),
                SendMode::Store { .. } => show(
                    out,
                    "Documento guardado. Selecciónalo e inícialo desde el panel de la máquina.",
                ),
            }
        }
        Err(e) => Err(format!("falló el envío: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use std::sync::atomic::AtomicBool;
    use tlaku_ruida::emu::{EmuConfig, EmuFaults, Emulator, UdpEmulator};
    use tlaku_ruida::packet::PacketConfig;

    #[derive(Parser)]
    struct Wrap {
        #[command(flatten)]
        design: DesignArgs,
    }

    #[derive(Parser)]
    struct WrapNet {
        #[command(flatten)]
        net: NetArgs,
    }

    fn design_file() -> String {
        format!("{}/tests/fixtures/design.svg", env!("CARGO_MANIFEST_DIR"))
    }

    fn built(bed: &str) -> (ComputedJob, (f64, f64), PathBuf) {
        let w = Wrap::parse_from(["t", &design_file(), "--mode", "line", "--bed", bed]);
        let req = w.design.into_request().unwrap();
        let b = job::build(&req).unwrap();
        (b, req.bed, req.input)
    }

    fn net(extra: &[&str]) -> NetArgs {
        let mut args = vec!["t", "--timeout-ms", "300"];
        args.extend(extra);
        WrapNet::parse_from(args).net
    }

    fn plan(run: bool, yes: bool, input: &std::path::Path) -> SendPlan {
        SendPlan::new(input, (600.0, 400.0), run, None, yes).unwrap()
    }

    #[test]
    fn send_to_a_udp_emulator_stores_the_document_after_confirmation() {
        let emu = UdpEmulator::spawn(EmuConfig::default()).unwrap();
        let n = net(&["--home", "bottom-left"]);
        let link = UdpLink::to_controller_ports(emu.addr().ip(), emu.addr().port(), 0).unwrap();
        let (b, bed, input) = built("600x400");
        let bytes = export_rd(&b, bed, n.home).unwrap();
        let mut out = Vec::new();
        send_with(
            link,
            &b,
            &bytes,
            &plan(false, false, &input),
            &n,
            &AtomicBool::new(false),
            &mut "s\n".as_bytes(),
            &mut out,
        )
        .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("RDC6442S"), "{text}");
        assert!(text.contains("DESIGN"), "{text}");
        assert!(text.contains("Documento guardado"), "{text}");
        assert!(text.contains("100 %"), "{text}");
        let e = emu.emulator();
        assert!(!e.running);
        assert_eq!(e.stored.len(), 1);
        assert_eq!(e.stored[0].name, "DESIGN");
        assert_eq!(e.stored[0].stream, bytes);
    }

    #[test]
    fn declining_the_confirmation_sends_nothing() {
        let (link, emu) = Emulator::new(EmuConfig::default()).into_link();
        let n = net(&["--home", "bottom-left"]);
        let (b, bed, input) = built("600x400");
        let bytes = export_rd(&b, bed, n.home).unwrap();
        let mut out = Vec::new();
        for answer in ["n\n", ""] {
            let e = send_with(
                link.clone(),
                &b,
                &bytes,
                &plan(true, false, &input),
                &n,
                &AtomicBool::new(false),
                &mut answer.as_bytes(),
                &mut out,
            )
            .unwrap_err();
            assert!(e.contains("cancelado"), "{e}");
        }
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("EMPEZARÁ a ejecutarse"), "{text}");
        let g = emu.lock().unwrap();
        // Solo lecturas (sondeo y estado): nada de D8, ni de trabajo.
        assert!(
            g.commands()
                .unwrap()
                .iter()
                .all(|c| matches!(c, tlaku_ruida::cmd::Command::MemRead { .. }))
        );
        assert!(!g.running && g.stored.is_empty());
    }

    #[test]
    fn run_mode_with_yes_starts_the_job_and_a_failure_stops_it() {
        let (link, emu) = Emulator::new(EmuConfig::default()).into_link();
        let n = net(&["--home", "bottom-left"]);
        let (b, bed, input) = built("600x400");
        let bytes = export_rd(&b, bed, n.home).unwrap();
        let mut out = Vec::new();
        send_with(
            link,
            &b,
            &bytes,
            &plan(true, true, &input),
            &n,
            &AtomicBool::new(false),
            &mut "".as_bytes(),
            &mut out,
        )
        .unwrap();
        assert!(emu.lock().unwrap().running);
        assert!(
            String::from_utf8(out)
                .unwrap()
                .contains("se está ejecutando")
        );

        // Se pierde un paquete: error con Detener confirmado y la máquina parada.
        let cfg = EmuConfig {
            faults: EmuFaults {
                // 0..2 sondeo, 3..5 y 6..8 las dos lecturas de estado; el 9 es el primer
                // paquete del trabajo.
                lose_packet: Some(9),
                ..Default::default()
            },
            packets: PacketConfig::default(),
            ..Default::default()
        };
        let (link, emu) = Emulator::new(cfg).into_link();
        let mut out = Vec::new();
        let mut n2 = n.clone();
        n2.timeout_ms = 200;
        let e = send_with(
            link,
            &b,
            &bytes,
            &plan(true, true, &input),
            &n2,
            &AtomicBool::new(false),
            &mut "".as_bytes(),
            &mut out,
        );
        let msg = e.unwrap_err();
        assert!(
            msg.contains("falló el envío") && msg.contains("Detener"),
            "{msg}"
        );
        let g = emu.lock().unwrap();
        assert!(!g.running);
        assert_eq!(g.stops, 1);
    }

    #[test]
    fn bed_mismatch_with_the_machine_is_refused_before_sending() {
        let (link, emu) = Emulator::new(EmuConfig::default()).into_link();
        let n = net(&["--home", "bottom-left"]);
        let (b, bed, input) = built("400x300");
        let bytes = export_rd(&b, bed, n.home).unwrap();
        let mut out = Vec::new();
        let e = send_with(
            link,
            &b,
            &bytes,
            &SendPlan::new(&input, (400.0, 300.0), false, None, true).unwrap(),
            &n,
            &AtomicBool::new(false),
            &mut "".as_bytes(),
            &mut out,
        )
        .unwrap_err();
        assert!(e.contains("--bed 600x400"), "{e}");
        assert!(emu.lock().unwrap().stored.is_empty());
    }

    #[test]
    fn busy_machine_is_refused() {
        let (link, emu) = Emulator::new(EmuConfig::default()).into_link();
        emu.lock().unwrap().running = true;
        let n = net(&["--home", "bottom-left"]);
        let (b, bed, input) = built("600x400");
        let bytes = export_rd(&b, bed, n.home).unwrap();
        let e = send_with(
            link,
            &b,
            &bytes,
            &plan(false, true, &input),
            &n,
            &AtomicBool::new(false),
            &mut "".as_bytes(),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(e.contains("no está en reposo"), "{e}");
    }

    #[test]
    fn status_is_read_only() {
        let emu = UdpEmulator::spawn(EmuConfig::default()).unwrap();
        emu.emulator().pos_um = (12_345, 100_000);
        let n = net(&["--home", "bottom-left"]);
        let link = UdpLink::to_controller_ports(emu.addr().ip(), emu.addr().port(), 0).unwrap();
        let mut out = Vec::new();
        status_with(link, &n, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.contains("RDC6442S") && text.contains("600.0 x 400.0"),
            "{text}"
        );
        assert!(text.contains("en reposo"), "{text}");
        assert!(text.contains("X 12.345 mm, Y 100.000 mm"), "{text}");
        let e = emu.emulator();
        assert!(
            e.commands()
                .unwrap()
                .iter()
                .all(|c| matches!(c, tlaku_ruida::cmd::Command::MemRead { .. }))
        );
        assert_eq!(e.pos_um, (12_345, 100_000));
    }

    #[test]
    fn status_without_answer_explains_what_to_check() {
        let link = tlaku_ruida::link::MemoryLink::new();
        let e = status_with(link, &net(&[]), &mut Vec::new()).unwrap_err();
        assert!(
            e.contains("sin respuesta") && e.contains("misma red /24"),
            "{e}"
        );
    }

    #[test]
    fn dry_run_needs_no_machine() {
        let n = net(&[]);
        let (b, bed, input) = built("600x400");
        let bytes = export_rd(&b, bed, n.home).unwrap();
        let mut out = Vec::new();
        dry_run_report(&b, &bytes, &plan(false, false, &input), &n, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("no se conectó"), "{text}");
        assert!(text.contains("documento \"DESIGN\""), "{text}");
    }

    #[test]
    fn document_names() {
        assert_eq!(
            default_doc_name(std::path::Path::new("a/mi diseño (1).svg")),
            "MI DISEO 1"
        );
        assert_eq!(default_doc_name(std::path::Path::new("ñ.svg")), "TLAKU");
        assert!(
            SendPlan::new(
                std::path::Path::new("x.svg"),
                (1.0, 1.0),
                false,
                Some("ñ".into()),
                true
            )
            .is_err()
        );
        assert!(
            SendPlan::new(
                std::path::Path::new("x.svg"),
                (1.0, 1.0),
                true,
                Some("ñ".into()),
                true
            )
            .is_ok()
        );
    }
}
