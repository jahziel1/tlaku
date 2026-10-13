//! Subcomando `tlaku k40`: envía un diseño a un K40 con placa Lihuiyu M2 Nano por USB (`send`),
//! lee su estado sin tocar nada (`status`), lista las placas conectadas (`list`) y exporta el
//! trabajo como `.egv` (`export-egv`). Con `--emu` todo corre contra una placa simulada, sin USB.
//!
//! EXPERIMENTAL: no se ha probado con ninguna placa real (ver `crates/tlaku-lhymicro`).

use crate::cli::DesignArgs;
use crate::options::parse_positive;
use crate::{job, link, report, sigint};
use clap::{Args, Subcommand};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tlaku_lhymicro::emu::{EmuBoard, EmuConfig, emulator};
use tlaku_lhymicro::encode::{EncodeConfig, HomeCorner, Program, encode};
use tlaku_lhymicro::session::{
    BoardInfo, JobControl, LhySession, Progress, SessionConfig, SessionError,
};
use tlaku_lhymicro::speed::Board;
use tlaku_lhymicro::transport::Transport;
use tlaku_lhymicro::usb::{self, UsbTimeouts, UsbTransport};

/// Aviso que acompaña a todo lo que toca una máquina real.
const EXPERIMENTAL: &str = "EXPERIMENTAL: el soporte K40 (M2 Nano) no se ha probado con una placa \
real. La M2 Nano no controla la potencia por software: ajústala en el panel de la máquina. Ten el \
paro de emergencia al alcance y prueba primero con el láser deshabilitado o muy débil.";

/// Cama que usa `--bed` si no se indica otra: el valor por defecto de `DesignArgs` es el del
/// Falcon2 (400x415), que no corresponde a un K40 (área útil habitual de 300x200).
const GRBL_DEFAULT_BED: (f64, f64) = (400.0, 415.0);
const K40_DEFAULT_BED: (f64, f64) = (300.0, 200.0);

/// Conexión: USB real o la placa simulada.
#[derive(Args, Clone, Debug)]
pub struct ConnectArgs {
    /// Usar una placa simulada en vez del USB (para probar sin máquina)
    #[arg(long)]
    emu: bool,
    /// Simular una Moshiboard (mismo USB, otros códigos de estado) para probar su detección
    #[arg(long, hide = true)]
    emu_moshiboard: bool,
    /// Número de la placa entre las conectadas (ver `tlaku k40 list`)
    #[arg(long, default_value_t = 0)]
    device: usize,
}

/// Cómo se codifica el trabajo.
#[derive(Args, Clone, Debug)]
pub struct EncodeArgs {
    /// Modelo de la placa: A, B, B1, B2, M, M1, M2 (la de serie) o M3. Cambia las ecuaciones de
    /// velocidad
    #[arg(long, default_value = "M2", value_parser = parse_board)]
    board: Board,
    /// Esquina de la cama donde está el home (los finales de carrera): top-left (K40 de serie),
    /// top-right, bottom-left o bottom-right
    #[arg(long, default_value = "top-left", value_parser = parse_home)]
    home: HomeCorner,
    /// Velocidad máxima de corte en mm/s; una capa más rápida es un error
    #[arg(long, default_value_t = 140.0, value_parser = parse_positive)]
    max_speed: f64,
    /// Sin rampa de aceleración en todas las velocidades (sufijo C). Por defecto se usa el
    /// engranaje del software chino para cada velocidad
    #[arg(long)]
    constant_speed: bool,
    /// Usar la pendiente de velocidad medida por MeerK40t (la placa va ~8 % más lenta que lo que
    /// dice el software chino)
    #[arg(long)]
    fix_speeds: bool,
    /// Desbloquear el riel tras cada salto (por defecto queda bloqueado)
    #[arg(long)]
    no_autolock: bool,
}

#[derive(Subcommand)]
pub enum K40Command {
    /// Envía un diseño a un K40 con placa M2 Nano (experimental, sin probar en hardware). Muestra
    /// qué va a hacer, pide confirmación y, salvo --no-home, lleva primero el cabezal al home
    /// (la placa no informa su posición). La potencia de las capas se ignora (la placa no la
    /// controla). Ctrl+C detiene la máquina y apaga el láser.
    Send {
        #[command(flatten)]
        design: DesignArgs,
        #[command(flatten)]
        connect: ConnectArgs,
        #[command(flatten)]
        encoding: EncodeArgs,
        /// No hacer home: se asume que el cabezal ya está en el home (esquina de los finales de
        /// carrera)
        #[arg(long)]
        no_home: bool,
        /// No esperar el estado «terminó» de la placa al cambiar de modo (si tu placa no lo
        /// informa y el envío se queda esperando)
        #[arg(long)]
        no_wait_finish: bool,
        /// Prepara y revisa el trabajo sin conectar con la máquina
        #[arg(long)]
        dry_run: bool,
        /// No pedir confirmación antes de enviar
        #[arg(short, long)]
        yes: bool,
    },
    /// Lee el estado de la placa (modelo del chip y código de estado). Solo lectura: no mueve
    /// nada ni enciende el láser. Avisa si la placa es una Moshiboard.
    Status {
        #[command(flatten)]
        connect: ConnectArgs,
        /// Repite la consulta N veces más y muestra cada código (sirve para ver cómo cambia
        /// el estado mientras se usan los botones de la máquina)
        #[arg(long, default_value_t = 0)]
        repeat: u32,
        /// Pausa entre consultas repetidas, en milisegundos
        #[arg(long, default_value_t = 200)]
        interval_ms: u64,
    },
    /// Lista las placas CH341 (USB 1A86:5512) conectadas.
    List,
    /// Convierte un diseño en un archivo `.egv` (LHYMICRO-GL) sin conectar con la máquina.
    ExportEgv {
        #[command(flatten)]
        design: DesignArgs,
        #[command(flatten)]
        encoding: EncodeArgs,
        /// Archivo `.egv` de salida
        output: PathBuf,
    },
}

fn parse_board(s: &str) -> Result<Board, String> {
    Board::parse(s)
        .ok_or_else(|| format!("'{s}' no es una placa: usa A, B, B1, B2, M, M1, M2 o M3"))
}

fn parse_home(s: &str) -> Result<HomeCorner, String> {
    HomeCorner::parse(s).ok_or_else(|| {
        format!("'{s}' no es una esquina: usa top-left, top-right, bottom-left o bottom-right")
    })
}

fn show<W: Write>(out: &mut W, text: &str) -> Result<(), String> {
    writeln!(out, "{text}").map_err(|e| e.to_string())
}

/// El diseño calculado y codificado.
struct Planned {
    built: tlaku_pipeline::ComputedJob,
    program: Program,
    bed: (f64, f64),
}

fn plan(design: DesignArgs, enc: &EncodeArgs) -> Result<Planned, String> {
    let mut request = design.into_request()?;
    if request.bed == GRBL_DEFAULT_BED {
        request.bed = K40_DEFAULT_BED;
    }
    let built = job::build(&request)?;
    tlaku_pipeline::check_bounds(&built, [request.bed.0, request.bed.1])?;
    let cfg = EncodeConfig {
        board: enc.board,
        bed_mm: [request.bed.0, request.bed.1],
        home: enc.home,
        autolock: !enc.no_autolock,
        max_speed_mm_s: enc.max_speed,
        constant_speed: enc.constant_speed,
        fix_speeds: enc.fix_speeds,
        ..EncodeConfig::default()
    };
    let program = encode(&built.toolpath, &cfg).map_err(|e| e.to_string())?;
    Ok(Planned {
        built,
        program,
        bed: request.bed,
    })
}

fn describe_program<W: Write>(p: &Planned, out: &mut W) -> Result<(), String> {
    let frames = p.program.frames().map_err(|e| e.to_string())?;
    let s = &p.program.stats;
    show(
        out,
        &format!(
            "Trabajo LHYMICRO-GL (placa {}): {} bytes en {} paquetes de 30 bytes; {:.1} mm \
             grabados, {:.1} mm de viajes con el láser apagado, {} saltos rápidos, {} entradas \
             al modo de corte; tiempo estimado {}.",
            p.program.board,
            p.program.stream.len(),
            frames.len(),
            s.burn_mm,
            s.travel_mm,
            s.rapids,
            s.programs,
            report::format_duration(s.est_seconds),
        ),
    )?;
    if !p.program.warnings.is_empty() {
        show(out, "Avisos de la placa:")?;
        for w in &p.program.warnings {
            show(out, &format!("  - {w}"))?;
        }
    }
    Ok(())
}

fn session_config(
    enc: &EncodeArgs,
    bed: (f64, f64),
    no_wait_finish: bool,
    emulated: bool,
) -> SessionConfig {
    SessionConfig {
        // El tiempo de la placa simulada es virtual: no hay motivo para esperar entre consultas.
        poll_backoff_max: if emulated {
            Duration::ZERO
        } else {
            SessionConfig::default().poll_backoff_max
        },
        board: enc.board,
        bed_mm: [bed.0, bed.1],
        home: enc.home,
        autolock: !enc.no_autolock,
        max_speed_mm_s: enc.max_speed,
        constant_speed: enc.constant_speed,
        fix_speeds: enc.fix_speeds,
        strict_finish: !no_wait_finish,
        ..SessionConfig::default()
    }
}

fn session_error(e: &SessionError) -> String {
    match e {
        SessionError::Transport(_) | SessionError::Timeout { .. } => format!(
            "{e}. Comprueba el cable USB, que la máquina esté encendida y que ningún otro \
             programa de láser (K40 Whisperer, MeerK40t, LaserDRW) tenga la placa abierta"
        ),
        _ => e.to_string(),
    }
}

fn describe_info<W: Write>(info: &BoardInfo, out: &mut W) -> Result<(), String> {
    let t = &info.transport;
    show(
        out,
        &format!(
            "Placa: {} (USB {:04X}:{:04X}{})",
            if t.name.is_empty() { "CH341" } else { &t.name },
            t.vendor_id,
            t.product_id,
            t.chip_version
                .map(|v| format!(", chip CH341 versión 0x{v:02X}"))
                .unwrap_or_default()
        ),
    )?;
    show(out, &format!("Estado: {}", info.status.status.describe()))?;
    for n in &info.notes {
        show(out, &format!("Nota: {n}"))?;
    }
    Ok(())
}

/// Resumen de lo que hizo la placa simulada.
fn describe_emulator<W: Write>(board: &mut EmuBoard, out: &mut W) -> Result<(), String> {
    board.run_to_idle();
    let burn = board.burn_length_mm();
    let bbox = board
        .burn_bbox_mm()
        .map(|(lo, hi)| {
            format!(
                "X {:.2} a {:.2} mm, Y {:.2} a {:.2} mm desde el home",
                lo.0, hi.0, lo.1, hi.1
            )
        })
        .unwrap_or_else(|| "nada".into());
    show(
        out,
        &format!(
            "Emulador: la placa simulada aceptó {} paquetes ({} rechazados, {} con el búfer \
             lleno), quemó {burn:.1} mm ({bbox}), tardó {:.1} s virtuales y terminó con el láser \
             {}.",
            board.accepted,
            board.rejected,
            board.overruns,
            board.elapsed_s(),
            if board.laser_on() {
                "ENCENDIDO"
            } else {
                "apagado"
            },
        ),
    )
}

pub fn run(action: K40Command) -> Result<(), String> {
    let mut out = io::stdout();
    match action {
        K40Command::List => list(&mut out),
        K40Command::Status {
            connect,
            repeat,
            interval_ms,
        } => status(&connect, repeat, interval_ms, &mut out),
        K40Command::ExportEgv {
            design,
            encoding,
            output,
        } => export_egv(design, &encoding, &output, &mut out),
        K40Command::Send {
            design,
            connect,
            encoding,
            no_home,
            no_wait_finish,
            dry_run,
            yes,
        } => send(
            design,
            SendRequest {
                connect,
                encoding,
                no_home,
                no_wait_finish,
                dry_run,
                yes,
            },
            sigint::install(),
            &mut io::stdin().lock(),
            &mut out,
        ),
    }
}

fn list<W: Write>(out: &mut W) -> Result<(), String> {
    let boards = usb::list_boards().map_err(|e| e.to_string())?;
    if boards.is_empty() {
        return show(
            out,
            "No hay placas CH341 (USB 1A86:5512) conectadas. Comprueba el cable y que la máquina \
             esté encendida.",
        );
    }
    for b in boards {
        show(out, &b.to_string())?;
    }
    show(
        out,
        "Una Moshiboard comparte este USB: `tlaku k40 status` distingue una M2 Nano.",
    )
}

fn status<W: Write>(
    connect: &ConnectArgs,
    repeat: u32,
    interval_ms: u64,
    out: &mut W,
) -> Result<(), String> {
    if connect.emu || connect.emu_moshiboard {
        let (t, _b) = emulator(EmuConfig {
            moshiboard: connect.emu_moshiboard,
            ..EmuConfig::default()
        });
        return status_with(t, repeat, interval_ms, out);
    }
    let t = open_usb(connect.device)?;
    status_with(t, repeat, interval_ms, out)
}

fn open_usb(device: usize) -> Result<UsbTransport, String> {
    UsbTransport::open(device, UsbTimeouts::default()).map_err(|e| e.to_string())
}

/// `tlaku k40 status` sobre un transporte ya abierto: solo lectura.
pub fn status_with<T: Transport, W: Write>(
    t: T,
    repeat: u32,
    interval_ms: u64,
    out: &mut W,
) -> Result<(), String> {
    let mut session = LhySession::new(t, SessionConfig::default());
    let info = session.probe().map_err(|e| session_error(&e))?;
    describe_info(&info, out)?;
    for i in 0..repeat {
        std::thread::sleep(Duration::from_millis(interval_ms));
        let r = session.status().map_err(|e| session_error(&e))?;
        show(
            out,
            &format!(
                "  consulta {}: código {} ({}), bytes {:?}",
                i + 1,
                r.code(),
                r.status.describe(),
                r.raw
            ),
        )?;
    }
    show(
        out,
        "Solo se envió la consulta de estado. Nada se movió ni se escribió.",
    )
}

fn export_egv<W: Write>(
    design: DesignArgs,
    enc: &EncodeArgs,
    output: &std::path::Path,
    out: &mut W,
) -> Result<(), String> {
    let planned = plan(design, enc)?;
    let bytes = planned
        .program
        .to_egv_file(&format!("Tlaku {}", env!("CARGO_PKG_VERSION")));
    std::fs::write(output, &bytes).map_err(|e| format!("{}: {e}", output.display()))?;
    show(out, report::summary(&planned.built).trim_end())?;
    describe_program(&planned, out)?;
    show(
        out,
        &format!(
            "Escrito {} ({} bytes). Supone el cabezal en el home al empezar. EXPERIMENTAL: sin \
             probar con hardware.",
            output.display(),
            bytes.len()
        ),
    )
}

/// Todo lo que pide `tlaku k40 send`.
pub struct SendRequest {
    pub connect: ConnectArgs,
    pub encoding: EncodeArgs,
    pub no_home: bool,
    pub no_wait_finish: bool,
    pub dry_run: bool,
    pub yes: bool,
}

/// `tlaku k40 send`. `cancel` es el indicador de Ctrl+C.
pub fn send<R: BufRead, W: Write>(
    design: DesignArgs,
    req: SendRequest,
    cancel: &AtomicBool,
    input: &mut R,
    out: &mut W,
) -> Result<(), String> {
    let planned = plan(design, &req.encoding)?;
    show(out, report::summary(&planned.built).trim_end())?;
    describe_program(&planned, out)?;
    if req.dry_run {
        return show(out, "Simulacro (--dry-run): no se conectó con la máquina.");
    }
    show(out, EXPERIMENTAL)?;
    let emulated = req.connect.emu || req.connect.emu_moshiboard;
    let cfg = session_config(&req.encoding, planned.bed, req.no_wait_finish, emulated);
    if emulated {
        let (t, board) = emulator(EmuConfig {
            moshiboard: req.connect.emu_moshiboard,
            ..EmuConfig::default()
        });
        show(out, "Usando la placa simulada (--emu): no se toca el USB.")?;
        let result = send_with(t, cfg, &planned, &req, cancel, input, out);
        let mut b = board.lock().unwrap_or_else(|e| e.into_inner());
        describe_emulator(&mut b, out)?;
        return result;
    }
    let t = open_usb(req.connect.device)?;
    send_with(t, cfg, &planned, &req, cancel, input, out)
}

fn send_with<T: Transport, R: BufRead, W: Write>(
    t: T,
    cfg: SessionConfig,
    planned: &Planned,
    req: &SendRequest,
    cancel: &AtomicBool,
    input: &mut R,
    out: &mut W,
) -> Result<(), String> {
    let mut session = LhySession::new(t, cfg);
    let info = session.probe().map_err(|e| session_error(&e))?;
    describe_info(&info, out)?;
    if !info.ready {
        return Err(format!(
            "la placa no está lista: {}",
            info.status.status.describe()
        ));
    }
    let home_text = if req.no_home {
        "Se asume que el cabezal ya está en el home (esquina de los finales de carrera)."
    } else {
        "Primero el cabezal irá al home (finales de carrera)."
    };
    show(
        out,
        &format!(
            "{home_text} Después se grabará el diseño: {:.0} x {:.0} mm de cama, home en {}.",
            planned.bed.0,
            planned.bed.1,
            req.encoding.home.name()
        ),
    )?;
    if !req.yes {
        let ok = link::confirm(
            "El láser DISPARARÁ. ¿La tapa está cerrada, el área despejada y el paro de \
             emergencia al alcance? [s/N] ",
            input,
            out,
        )
        .map_err(|e| e.to_string())?;
        if !ok {
            return Err("cancelado: no se envió nada".into());
        }
    }
    if req.no_home {
        session.assume_at_home();
    } else {
        show(out, "Haciendo home...")?;
        session.home().map_err(|e| session_error(&e))?;
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

    // Ctrl+C: otro hilo traslada el indicador al mando del trabajo.
    let control: JobControl = session.control();
    let done = AtomicBool::new(false);
    let result = std::thread::scope(|scope| {
        scope.spawn(|| {
            while !done.load(Ordering::SeqCst) {
                if cancel.load(Ordering::SeqCst) {
                    control.request_stop();
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        let r = session.send_job(&planned.program, &mut progress);
        done.store(true, Ordering::SeqCst);
        r
    });
    match result {
        Ok(r) => {
            show(
                out,
                &format!(
                    "Enviado: {} paquetes, {} bytes, {} reenvíos por CRC, en {:.1} s.{}",
                    r.packets,
                    r.bytes,
                    r.resends,
                    r.elapsed.as_secs_f64(),
                    if r.finish_confirmed {
                        " La placa confirmó el final del trabajo."
                    } else {
                        " La placa no confirmó el final del trabajo (estado 236)."
                    }
                ),
            )?;
            show(
                out,
                &format!(
                    "El cabezal quedó en X {:.2}, Y {:.2} mm.",
                    r.end_mm.x, r.end_mm.y
                ),
            )
        }
        Err(e) => Err(format!("falló el envío: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Wrap {
        #[command(subcommand)]
        action: K40Command,
    }

    fn parse(args: &[&str]) -> K40Command {
        Wrap::try_parse_from(std::iter::once("k40").chain(args.iter().copied()))
            .unwrap_or_else(|e| panic!("{e}"))
            .action
    }

    fn design_file() -> String {
        format!("{}/tests/fixtures/design.svg", env!("CARGO_MANIFEST_DIR"))
    }

    fn send_cmd(extra: &[&str]) -> (DesignArgs, SendRequest) {
        let design = design_file();
        let mut args = vec!["send", design.as_str(), "--mode", "line"];
        args.extend(extra);
        match parse(&args) {
            K40Command::Send {
                design,
                connect,
                encoding,
                no_home,
                no_wait_finish,
                dry_run,
                yes,
            } => (
                design,
                SendRequest {
                    connect,
                    encoding,
                    no_home,
                    no_wait_finish,
                    dry_run,
                    yes,
                },
            ),
            _ => panic!("no es send"),
        }
    }

    fn run_send(extra: &[&str], stdin: &str) -> (Result<(), String>, String) {
        let mut out = Vec::new();
        let cancel = AtomicBool::new(false);
        let (design, req) = send_cmd(extra);
        let r = send(
            design,
            req,
            &cancel,
            &mut io::Cursor::new(stdin.as_bytes().to_vec()),
            &mut out,
        );
        (r, String::from_utf8(out).unwrap())
    }

    #[test]
    fn send_against_the_emulator_burns_the_design_and_homes_first() {
        let (r, out) = run_send(&["--emu", "--yes"], "");
        r.unwrap_or_else(|e| panic!("{e}\n{out}"));
        assert!(out.contains("Usando la placa simulada"), "{out}");
        assert!(out.contains("Haciendo home"), "{out}");
        assert!(out.contains("Enviado:"), "{out}");
        assert!(out.contains("confirmó el final del trabajo"), "{out}");
        assert!(out.contains("terminó con el láser apagado"), "{out}");
        assert!(out.contains("0 rechazados, 0 con el búfer lleno"), "{out}");
        // La potencia de la capa se ignora con aviso.
        assert!(out.contains("potencia"), "{out}");
        assert!(out.contains("EXPERIMENTAL"), "{out}");
    }

    #[test]
    fn declining_the_confirmation_sends_nothing() {
        let (r, out) = run_send(&["--emu"], "n\n");
        assert!(r.unwrap_err().contains("cancelado"));
        assert!(out.contains("El láser DISPARARÁ"), "{out}");
        assert!(
            out.contains("0 paquetes") || out.contains("aceptó 0"),
            "{out}"
        );
        // Con "s" sí envía.
        let (r, out) = run_send(&["--emu"], "s\n");
        r.unwrap_or_else(|e| panic!("{e}\n{out}"));
    }

    #[test]
    fn no_home_assumes_the_head_is_at_home() {
        let (r, out) = run_send(&["--emu", "--yes", "--no-home"], "");
        r.unwrap_or_else(|e| panic!("{e}\n{out}"));
        assert!(!out.contains("Haciendo home"), "{out}");
        assert!(
            out.contains("Se asume que el cabezal ya está en el home"),
            "{out}"
        );
    }

    #[test]
    fn dry_run_does_not_connect() {
        let (r, out) = run_send(&["--dry-run"], "");
        r.unwrap();
        assert!(out.contains("Simulacro"), "{out}");
        assert!(out.contains("paquetes de 30 bytes"), "{out}");
        assert!(!out.contains("Placa:"), "no se conectó: {out}");
    }

    #[test]
    fn a_design_bigger_than_the_bed_is_refused_before_connecting() {
        let (r, out) = run_send(&["--emu", "--yes", "--bed", "50x50"], "");
        let e = r.unwrap_err();
        assert!(e.contains("área de trabajo"), "{e}");
        assert!(!out.contains("Placa:"), "{out}");
    }

    #[test]
    fn the_default_bed_is_a_k40_not_the_falcon() {
        // 100x60 mm cabe en 300x200 sin pasar --bed (el valor por defecto de GRBL es 400x415).
        let (r, out) = run_send(&["--emu", "--yes", "--dry-run"], "");
        r.unwrap_or_else(|e| panic!("{e}\n{out}"));
        assert!(out.contains("Simulacro"));
    }

    #[test]
    fn a_moshiboard_is_reported_not_driven() {
        let (r, out) = run_send(&["--emu-moshiboard", "--yes"], "");
        let e = r.unwrap_err();
        assert!(e.contains("Moshiboard"), "{e}");
        assert!(!out.contains("Enviado:"), "{out}");
    }

    #[test]
    fn status_is_read_only_and_names_the_board() {
        let mut out = Vec::new();
        let cmd = parse(&["status", "--emu"]);
        let K40Command::Status { connect, .. } = cmd else {
            panic!()
        };
        status(&connect, 0, 0, &mut out).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("M2 Nano simulada"), "{s}");
        assert!(s.contains("1A86:5512"), "{s}");
        assert!(s.contains("lista (206)"), "{s}");
        assert!(s.contains("Nada se movió"), "{s}");
        let mut out = Vec::new();
        let K40Command::Status { connect, .. } = parse(&["status", "--emu-moshiboard"]) else {
            panic!()
        };
        let e = status(&connect, 0, 0, &mut out).unwrap_err();
        assert!(e.contains("Moshiboard"), "{e}");
    }

    #[test]
    fn status_can_repeat_the_query_and_print_the_raw_codes() {
        let K40Command::Status {
            connect,
            repeat,
            interval_ms,
        } = parse(&["status", "--emu", "--repeat", "3", "--interval-ms", "1"])
        else {
            panic!()
        };
        let mut out = Vec::new();
        status(&connect, repeat, interval_ms, &mut out).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert_eq!(s.matches("  consulta ").count(), 3, "{s}");
        assert!(s.contains("código 206"), "{s}");
    }

    #[test]
    fn export_egv_writes_the_header_and_the_stream() {
        let dir = std::env::temp_dir().join(format!("tlaku-k40-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.egv");
        let design = design_file();
        let path_s = path.to_string_lossy().to_string();
        let K40Command::ExportEgv {
            design,
            encoding,
            output,
        } = parse(&["export-egv", &design, &path_s, "--mode", "line"])
        else {
            panic!()
        };
        let mut out = Vec::new();
        export_egv(design, &encoding, &output, &mut out).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.starts_with("Document type : LHYMICRO-GL file\n"),
            "{text}"
        );
        assert!(text.contains("%0%0%0%0%\nI"));
        assert!(text.contains("CV") && text.contains("S1E") && text.contains("FNSE-"));
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("Escrito"), "{s}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn options_are_validated_in_spanish() {
        let err = |args: &[&str]| {
            Wrap::try_parse_from(std::iter::once("k40").chain(args.iter().copied()))
                .err()
                .expect("debía fallar")
                .to_string()
        };
        let design = design_file();
        assert!(err(&["send", &design, "--board", "Z9"]).contains("no es una placa"));
        assert!(err(&["send", &design, "--home", "centro"]).contains("no es una esquina"));
        assert!(err(&["send", &design, "--max-speed", "0"]).contains("mayor que 0"));
    }

    #[test]
    fn ctrl_c_flag_stops_the_job_and_reports_the_laser_off() {
        // El indicador ya está activo antes de empezar: el hilo lo traslada y el trabajo se
        // cancela en cuanto la sesión lo atiende, apagando el láser.
        let mut out = Vec::new();
        let cancel = AtomicBool::new(true);
        let (design, req) = send_cmd(&["--emu", "--yes"]);
        let r = send(
            design,
            req,
            &cancel,
            &mut io::Cursor::new(Vec::new()),
            &mut out,
        );
        let out = String::from_utf8(out).unwrap();
        match r {
            // Cancelado a tiempo (lo normal) o terminó antes de que el hilo lo viera.
            Err(e) => {
                assert!(e.contains("cancelado"), "{e}");
                assert!(e.contains("apagó el láser"), "{e}");
            }
            Ok(()) => assert!(out.contains("Enviado:")),
        }
        assert!(out.contains("terminó con el láser apagado"), "{out}");
    }
}
