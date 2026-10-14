//! Subcomando `tlaku galvo`: láseres galvo (tarjetas JCZ/BJJCZ LMC, EZCAD2) por USB.
//!
//! - `list`: dispositivos del fabricante 9588 conectados y si son compatibles (no abre nada).
//! - `status`: sondeo de solo lectura de la tarjeta.
//! - `light`: encuadre con el **puntero rojo** (rectángulo o contorno) en bucle hasta Ctrl+C.
//! - `mark`: marca el diseño. Con `--red-only` hace todo el recorrido con el láser deshabilitado y
//!   el puntero rojo; con `--dry-run` solo compila.
//!
//! `--emu` usa una tarjeta simulada, sin USB. Con hardware real todo menos `list` exige
//! `--i-understand-laser-safety`.
//!
//! EXPERIMENTAL: no se ha probado con ninguna tarjeta real (docs/hardware/galvo.md, sección 9).

use crate::cli::DesignArgs;
use crate::options::{parse_f64, parse_positive};
use crate::{job, sigint};
use clap::{Args, Subcommand, ValueEnum};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tlaku_core::Point;
use tlaku_core::toolpath::Toolpath;
use tlaku_galvo::cmd::Describe;
use tlaku_galvo::compile::{self, CompileOptions, CompiledList, FieldOrigin, LightShape};
use tlaku_galvo::emu::{self, EmuConfig, EmuHandle};
use tlaku_galvo::safety::ListKind;
use tlaku_galvo::session::{
    BoardInfo, GalvoSession, InputCheck, JobControl, JobError, LightProgress, Progress,
    SessionConfig,
};
use tlaku_galvo::transport::{LaserSafetyAck, Support, Transport};
use tlaku_galvo::usb::{self, UsbTimeouts, UsbTransport};
use tlaku_pipeline::doc::{GalvoDevice, LaserSource};

/// Aviso que acompaña a todo lo que toca una tarjeta real.
const EXPERIMENTAL: &str = "EXPERIMENTAL: el soporte galvo no se ha probado con ninguna tarjeta \
real. Una fibra es láser clase 4 y su haz es INVISIBLE: usa gafas con la longitud de onda y la DO \
correctas, recinto cerrado con enclavamiento y un obturador, y ten el paro de emergencia al \
alcance. Primero prueba sin la fuente conectada (docs/hardware/galvo.md, sección 9).";

/// La cama de GRBL por defecto de `DesignArgs` no sirve para un galvo: se usa el campo.
const GRBL_DEFAULT_BED: (f64, f64) = (400.0, 415.0);
/// Potencia por defecto si no se indica: baja a propósito.
const DEFAULT_POWER_PCT: f64 = 10.0;
/// Velocidad de marcado por defecto, mm/s.
const DEFAULT_MARK_SPEED: f64 = 1000.0;

/// Conexión: USB real o la tarjeta simulada.
#[derive(Args, Clone, Debug)]
pub struct ConnectArgs {
    /// Usar una tarjeta simulada en vez del USB (para probar sin máquina)
    #[arg(long)]
    emu: bool,
    /// Número de la tarjeta entre las listadas (ver `tlaku galvo list`)
    #[arg(long, default_value_t = 0)]
    device: usize,
    /// Obligatorio con hardware real: confirmas que entiendes que el láser puede dañar a las
    /// personas y que has leído docs/hardware/galvo.md (gafas, recinto, obturador, paro de
    /// emergencia). Sin esta bandera no se abre el USB
    #[arg(long)]
    i_understand_laser_safety: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum SourceArg {
    /// Fibra Q-switched: frecuencia, sin ancho de pulso
    FiberQ,
    /// Fibra MOPA: frecuencia y ancho de pulso de la lista
    FiberMopa,
    Co2,
    Uv,
}

impl From<SourceArg> for LaserSource {
    fn from(s: SourceArg) -> Self {
        match s {
            SourceArg::FiberQ => LaserSource::FiberQ,
            SourceArg::FiberMopa => LaserSource::FiberMopa,
            SourceArg::Co2 => LaserSource::Co2,
            SourceArg::Uv => LaserSource::Uv,
        }
    }
}

fn parse_range(s: &str) -> Result<(f64, f64), String> {
    let (a, b) = s
        .split_once('-')
        .ok_or_else(|| format!("'{s}': usa MIN-MAX, por ejemplo 20-80"))?;
    let (a, b) = (parse_positive(a)?, parse_positive(b)?);
    if a > b {
        return Err(format!("'{s}': el mínimo es mayor que el máximo"));
    }
    Ok((a, b))
}

/// `MASK=VALOR` en hexadecimal (`0x`) o decimal.
fn parse_input_check(s: &str) -> Result<InputCheck, String> {
    let num = |t: &str| {
        let t = t.trim();
        match t.strip_prefix("0x") {
            Some(h) => u16::from_str_radix(h, 16),
            None => t.parse::<u16>(),
        }
        .map_err(|_| format!("'{t}' no es un entero de 16 bits"))
    };
    let (m, v) = s
        .split_once('=')
        .ok_or_else(|| format!("'{s}': usa MASK=VALOR, por ejemplo 0x0008=0x0008"))?;
    let (mask, value) = (num(m)?, num(v)?);
    if value & !mask != 0 {
        return Err(format!("'{s}': VALOR tiene bits fuera de MASK"));
    }
    Ok(InputCheck { mask, value })
}

/// El perfil de la máquina galvo.
#[derive(Args, Clone, Debug)]
pub struct ProfileArgs {
    /// Lado del campo de la lente F-theta en mm (110, 150, 200, 300...)
    #[arg(long, default_value_t = 110.0, value_parser = parse_positive)]
    field: f64,
    /// Tipo de fuente
    #[arg(long, value_enum, default_value_t = SourceArg::FiberQ)]
    source: SourceArg,
    /// Rango de frecuencia de la fuente en kHz, MIN-MAX. Fuera de él no se envía nada
    #[arg(long, default_value = "20-80", value_parser = parse_range)]
    freq_range: (f64, f64),
    /// Anchos de pulso permitidos en ns, separados por comas (obligatorio con --source fiber-mopa)
    #[arg(long, value_delimiter = ',')]
    pulse_widths: Vec<u32>,
    /// Potencia máxima que permite el perfil, en %. Una capa que la supere es un error
    #[arg(long, default_value_t = 100.0, value_parser = parse_positive)]
    max_power: f64,
    /// Velocidad del encuadre con el puntero rojo, mm/s
    #[arg(long, default_value_t = 2000.0, value_parser = parse_positive)]
    light_speed: f64,
    /// Velocidad de los saltos, mm/s
    #[arg(long, default_value_t = 4000.0, value_parser = parse_positive)]
    jump_speed: f64,
    /// Negar el eje X del galvo
    #[arg(long)]
    invert_x: bool,
    /// Negar el eje Y del galvo
    #[arg(long)]
    invert_y: bool,
    /// Intercambiar los ejes X e Y
    #[arg(long)]
    swap_xy: bool,
    /// Giro del campo en grados (antihorario)
    #[arg(long, default_value_t = 0.0, value_parser = parse_f64, allow_hyphen_values = true)]
    rotate: f64,
    /// Enclavamiento por software: antes de cada trabajo se exige (entradas & MASK) == VALOR
    /// (por ejemplo 0x0008=0x0008). Los bits de EMSTOP y alarmas no están documentados: mídelos
    #[arg(long, value_name = "MASK=VALOR", value_parser = parse_input_check)]
    require_input: Option<InputCheck>,
}

impl ProfileArgs {
    fn device(&self) -> GalvoDevice {
        let mut d = GalvoDevice {
            field_mm: self.field,
            source: self.source.into(),
            freq_min_khz: self.freq_range.0,
            freq_max_khz: self.freq_range.1,
            pulse_widths_ns: self.pulse_widths.clone(),
            max_power_pct: self.max_power,
            light_speed: self.light_speed,
            ..GalvoDevice::default()
        };
        d.jump.speed = self.jump_speed;
        d.correction.invert = [self.invert_x, self.invert_y];
        d.correction.swap_xy = self.swap_xy;
        d.correction.rotation_deg = self.rotate;
        d
    }
}

/// Parámetros de marcado de los bloques.
#[derive(Args, Clone, Debug)]
pub struct MarkParams {
    /// Frecuencia en kHz (obligatoria: debe estar dentro del rango del perfil)
    #[arg(long, value_parser = parse_positive)]
    frequency: f64,
    /// Ancho de pulso en ns (fuentes MOPA y UV; debe estar en --pulse-widths)
    #[arg(long)]
    pulse_width: Option<u32>,
    /// Velocidad de marcado en mm/s (si no pasas --speed en mm/min)
    #[arg(long, default_value_t = DEFAULT_MARK_SPEED, value_parser = parse_positive)]
    mark_speed: f64,
    /// Retardo de encendido del láser en us (admite negativos)
    #[arg(long, value_parser = parse_f64, allow_hyphen_values = true)]
    laser_on: Option<f64>,
    /// Retardo de apagado del láser en us (admite negativos)
    #[arg(long, value_parser = parse_f64, allow_hyphen_values = true)]
    laser_off: Option<f64>,
    /// Retardo en las esquinas en us
    #[arg(long, value_parser = parse_f64)]
    polygon: Option<f64>,
    /// Pausa al terminar cada trazo en us
    #[arg(long, value_parser = parse_f64)]
    end_delay: Option<f64>,
    /// Repeticiones del trabajo dentro de la tarjeta
    #[arg(long, default_value_t = 1)]
    repeat: u32,
}

#[derive(Subcommand)]
pub enum GalvoCommand {
    /// Lista los dispositivos BJJCZ (USB 9588:xxxx) conectados y si son compatibles. No abre
    /// nada ni necesita --i-understand-laser-safety.
    List,
    /// Lee el estado de la tarjeta (serie, versión, estado, entradas). Solo lectura: no mueve
    /// nada ni enciende nada.
    Status {
        #[command(flatten)]
        connect: ConnectArgs,
        #[command(flatten)]
        profile: ProfileArgs,
    },
    /// Encuadre con el puntero rojo: dibuja el rectángulo (o el contorno) del diseño en bucle
    /// hasta Ctrl+C. Nunca habilita el láser. Experimental.
    Light {
        #[command(flatten)]
        design: DesignArgs,
        #[command(flatten)]
        connect: ConnectArgs,
        #[command(flatten)]
        profile: ProfileArgs,
        /// Dibujar los trazos del diseño en vez de su rectángulo
        #[arg(long)]
        contour: bool,
        /// Parar tras N vueltas (por defecto, hasta Ctrl+C)
        #[arg(long)]
        loops: Option<u32>,
    },
    /// Marca el diseño con el láser (experimental, sin probar en hardware). Muestra qué va a
    /// hacer, pide confirmación y arma la fuente solo durante el trabajo. Ctrl+C detiene y apaga.
    Mark {
        #[command(flatten)]
        design: DesignArgs,
        #[command(flatten)]
        connect: ConnectArgs,
        #[command(flatten)]
        profile: ProfileArgs,
        #[command(flatten)]
        params: MarkParams,
        /// Recorre todo el trabajo con el láser DESHABILITADO y el puntero rojo encendido
        #[arg(long)]
        red_only: bool,
        /// Compila y revisa el trabajo sin conectar con la tarjeta
        #[arg(long)]
        dry_run: bool,
        /// Escribe la lista de comandos decodificada en este archivo
        #[arg(long)]
        dump: Option<PathBuf>,
        /// No pedir confirmación antes de marcar
        #[arg(short, long)]
        yes: bool,
    },
}

fn show<W: Write>(out: &mut W, text: &str) -> Result<(), String> {
    writeln!(out, "{text}").map_err(|e| e.to_string())
}

/// Un trabajo preparado: el diseño convertido y el perfil.
struct Planned {
    toolpath: Toolpath,
    device: GalvoDevice,
    origin: FieldOrigin,
    warnings: Vec<String>,
}

/// Importa el diseño, ajusta la cama al campo y deja los bloques con sus `GalvoParams`.
fn plan(
    design: DesignArgs,
    profile: &ProfileArgs,
    params: Option<&MarkParams>,
) -> Result<Planned, String> {
    let device = profile.device();
    let mut request = design.into_request()?;
    if request.bed == GRBL_DEFAULT_BED {
        request.bed = (device.field_mm, device.field_mm);
    }
    request.defaults.power.get_or_insert(DEFAULT_POWER_PCT);
    request
        .defaults
        .speed
        .get_or_insert(params.map_or(DEFAULT_MARK_SPEED, |p| p.mark_speed) * 60.0);
    let built = job::build(&request)?;
    tlaku_pipeline::check_bounds(&built, [request.bed.0, request.bed.1])?;
    let mut toolpath = built.toolpath.clone();
    if let Some(p) = params {
        let mut gp = compile::params_from_device(&device, p.frequency, p.pulse_width);
        gp.laser_on_us = p.laser_on.unwrap_or(gp.laser_on_us);
        gp.laser_off_us = p.laser_off.unwrap_or(gp.laser_off_us);
        gp.polygon_us = p.polygon.unwrap_or(gp.polygon_us);
        gp.end_us = p.end_delay.unwrap_or(gp.end_us);
        gp.repeat = p.repeat;
        for b in &mut toolpath.blocks {
            b.galvo = Some(gp.clone());
        }
    }
    Ok(Planned {
        toolpath,
        device,
        // Convención del pipeline: el campo está centrado en la cama.
        origin: FieldOrigin::CenterAt(Point::new(request.bed.0 / 2.0, request.bed.1 / 2.0)),
        warnings: built.preview.warnings,
    })
}

fn session_config(
    p: &Planned,
    profile: &ProfileArgs,
    emulated: bool,
) -> Result<SessionConfig, String> {
    let mut cfg = SessionConfig::new(p.device.clone()).map_err(|e| e.to_string())?;
    cfg.origin = p.origin;
    cfg.inputs_required = profile.require_input;
    if emulated {
        // El tiempo de la tarjeta simulada es virtual: no hay motivo para esperar.
        cfg.poll_interval = Duration::ZERO;
    }
    Ok(cfg)
}

fn job_error(e: &JobError) -> String {
    let mut s = e.to_string();
    if matches!(
        e.cause,
        tlaku_galvo::session::SessionError::Transport(_)
            | tlaku_galvo::session::SessionError::Timeout { .. }
    ) {
        s.push_str(
            ". Comprueba el cable USB, que la máquina esté encendida y que EZCAD, LightBurn o \
             MeerK40t no tengan la tarjeta abierta",
        );
    }
    s
}

fn require_ack(connect: &ConnectArgs) -> Result<LaserSafetyAck, String> {
    if connect.i_understand_laser_safety {
        Ok(LaserSafetyAck::i_understand_laser_safety())
    } else {
        Err(
            "con hardware real hace falta --i-understand-laser-safety: un láser de fibra es clase \
             4 y su haz es invisible. Lee docs/hardware/galvo.md (gafas con la DO correcta, \
             recinto, obturador, paro de emergencia) y prueba primero sin la fuente conectada. \
             Para probar sin máquina usa --emu"
                .into(),
        )
    }
}

fn open_usb(connect: &ConnectArgs) -> Result<UsbTransport, String> {
    let ack = require_ack(connect)?;
    UsbTransport::open(connect.device, UsbTimeouts::default(), ack).map_err(|e| e.to_string())
}

fn describe_info<W: Write>(info: &BoardInfo, out: &mut W) -> Result<(), String> {
    let t = &info.transport;
    show(
        out,
        &format!(
            "Tarjeta: {} (USB {:04X}:{:04X})",
            info.board_name, t.vendor_id, t.product_id
        ),
    )?;
    show(
        out,
        &format!(
            "Serie: {:04x}{:04x}{:04x}{:04x}  Versión: {:04x}:{:04x}:{:04x}",
            info.serial[0],
            info.serial[1],
            info.serial[2],
            info.serial[3],
            info.version[0],
            info.version[1],
            info.version[2]
        ),
    )?;
    show(
        out,
        &format!(
            "Estado: {} (bits {:#06x}: ocupada={}, lista={})  Entradas: {:#06x}",
            if info.status.busy {
                "ocupada"
            } else {
                "en reposo"
            },
            info.status.raw,
            info.status.busy,
            info.status.ready,
            info.inputs
        ),
    )?;
    for n in &info.notes {
        show(out, &format!("Nota: {n}"))?;
    }
    Ok(())
}

fn describe_list<W: Write>(l: &CompiledList, out: &mut W) -> Result<(), String> {
    let s = &l.stats;
    show(
        out,
        &format!(
            "Lista de la tarjeta: {} comandos en {} tandas de 256; {} marcas ({:.1} mm), {} saltos \
             ({:.1} mm), {} disparos en un punto, {} cortes con potencia 0 emitidos como salto; \
             tiempo estimado {} (sin retardos de láser).",
            s.commands,
            s.batches,
            s.marks,
            s.mark_mm,
            s.jumps,
            s.jump_mm,
            s.dots,
            s.zero_power_cuts,
            crate::report::format_duration(s.est_seconds),
        ),
    )?;
    for w in &l.warnings {
        show(out, &format!("Aviso: {w}"))?;
    }
    Ok(())
}

fn describe_emulator<W: Write>(h: &EmuHandle, out: &mut W) -> Result<(), String> {
    let s = emu::lock(h);
    show(
        out,
        &format!(
            "Emulador: {} tandas recibidas, {} tramos con el haz saliendo, {} saltos (ninguno \
             disparó), {:.2} s virtuales; la fuente terminó {}; {} violaciones del protocolo.",
            s.batches_received,
            s.emitted().len(),
            s.jumps().len(),
            s.elapsed_s(),
            if s.laser_safe() {
                "apagada"
            } else {
                "HABILITADA"
            },
            s.violations.len(),
        ),
    )
}

pub fn run(action: GalvoCommand) -> Result<(), String> {
    let mut out = io::stdout();
    match action {
        GalvoCommand::List => list(&mut out),
        GalvoCommand::Status { connect, profile } => status(&connect, &profile, &mut out),
        GalvoCommand::Light {
            design,
            connect,
            profile,
            contour,
            loops,
        } => light(
            design,
            LightRequest {
                connect,
                profile,
                contour,
                loops,
            },
            sigint::install(),
            &mut out,
        ),
        GalvoCommand::Mark {
            design,
            connect,
            profile,
            params,
            red_only,
            dry_run,
            dump,
            yes,
        } => mark(
            design,
            MarkRequest {
                connect,
                profile,
                params,
                red_only,
                dry_run,
                dump,
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
            "No hay dispositivos BJJCZ (USB 9588:xxxx) conectados. Comprueba el cable y que la \
             máquina esté encendida.",
        );
    }
    for b in &boards {
        show(out, &b.to_string())?;
    }
    if boards.iter().all(|b| b.support != Support::Supported) {
        show(
            out,
            "Ninguno es compatible: Tlaku solo habla con la LMCV4-FIBER-M (9588:9899).",
        )?;
    }
    Ok(())
}

fn status<W: Write>(
    connect: &ConnectArgs,
    profile: &ProfileArgs,
    out: &mut W,
) -> Result<(), String> {
    let device = profile.device();
    let planned = Planned {
        toolpath: Toolpath::default(),
        origin: FieldOrigin::Center,
        device,
        warnings: Vec::new(),
    };
    if connect.emu {
        let (t, _h) = emu::emulator(EmuConfig::default());
        return status_with(t, session_config(&planned, profile, true)?, out);
    }
    let t = open_usb(connect)?;
    status_with(t, session_config(&planned, profile, false)?, out)
}

fn status_with<T: Transport, W: Write>(
    t: T,
    cfg: SessionConfig,
    out: &mut W,
) -> Result<(), String> {
    let mut s = GalvoSession::new(t, cfg).map_err(|e| e.to_string())?;
    let info = s.probe().map_err(|e| e.to_string())?;
    describe_info(&info, out)?;
    show(
        out,
        "Solo se enviaron consultas. Nada se movió ni se escribió.",
    )
}

/// Todo lo que pide `tlaku galvo light`.
pub struct LightRequest {
    pub connect: ConnectArgs,
    pub profile: ProfileArgs,
    pub contour: bool,
    pub loops: Option<u32>,
}

pub fn light<W: Write>(
    design: DesignArgs,
    req: LightRequest,
    cancel: &AtomicBool,
    out: &mut W,
) -> Result<(), String> {
    let planned = plan(design, &req.profile, None)?;
    let shape = if req.contour {
        LightShape::contour_of(&planned.toolpath)
    } else {
        LightShape::bounds_of(&planned.toolpath)
    }
    .ok_or("el diseño no tiene movimientos que encuadrar")?;
    for w in &planned.warnings {
        show(out, &format!("Aviso: {w}"))?;
    }
    if let LightShape::Rect { min, max } = &shape {
        show(
            out,
            &format!(
                "Encuadre: X {:.2} a {:.2} mm, Y {:.2} a {:.2} mm (campo de {} mm).",
                min.x, max.x, min.y, max.y, planned.device.field_mm
            ),
        )?;
    }
    show(out, EXPERIMENTAL)?;
    let emulated = req.connect.emu;
    let cfg = session_config(&planned, &req.profile, emulated)?;
    if emulated {
        let (t, h) = emu::emulator(EmuConfig {
            source: planned.device.source,
            field_mm: planned.device.field_mm,
            ..EmuConfig::default()
        });
        show(
            out,
            "Usando la tarjeta simulada (--emu): no se toca el USB.",
        )?;
        let r = light_with(t, cfg, &shape, req.loops, cancel, out);
        describe_emulator(&h, out)?;
        return r;
    }
    let t = open_usb(&req.connect)?;
    light_with(t, cfg, &shape, req.loops, cancel, out)
}

/// Traslada el indicador de Ctrl+C al mando del trabajo mientras dura `f`.
fn with_ctrl_c<R>(control: &JobControl, cancel: &AtomicBool, f: impl FnOnce() -> R) -> R {
    let done = AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            while !done.load(Ordering::SeqCst) {
                if cancel.load(Ordering::SeqCst) {
                    control.request_stop();
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        let r = f();
        done.store(true, Ordering::SeqCst);
        r
    })
}

fn light_with<T: Transport, W: Write>(
    t: T,
    cfg: SessionConfig,
    shape: &LightShape,
    loops: Option<u32>,
    cancel: &AtomicBool,
    out: &mut W,
) -> Result<(), String> {
    let mut session = GalvoSession::new(t, cfg).map_err(|e| e.to_string())?;
    let info = session.probe().map_err(|e| e.to_string())?;
    describe_info(&info, out)?;
    show(
        out,
        "Dibujando con el puntero rojo; el láser NO se habilita. Ctrl+C para detener.",
    )?;
    let control = session.control();
    let mut progress = |p: LightProgress| {
        if p.loops == 1 || p.loops.is_multiple_of(10) {
            let _ = writeln!(out, "  vuelta {}", p.loops);
        }
    };
    let result = with_ctrl_c(&control, cancel, || {
        session.light(shape, &control, loops, &mut progress)
    });
    match result {
        Ok(r) => show(
            out,
            &format!(
                "Encuadre terminado: {} vueltas{}. Puntero rojo apagado.",
                r.loops,
                if r.stopped { " (detenido)" } else { "" }
            ),
        ),
        Err(e) => Err(format!("falló el encuadre: {}", job_error(&e))),
    }
}

/// Todo lo que pide `tlaku galvo mark`.
pub struct MarkRequest {
    pub connect: ConnectArgs,
    pub profile: ProfileArgs,
    pub params: MarkParams,
    pub red_only: bool,
    pub dry_run: bool,
    pub dump: Option<PathBuf>,
    pub yes: bool,
}

pub fn mark<R: BufRead, W: Write>(
    design: DesignArgs,
    req: MarkRequest,
    cancel: &AtomicBool,
    input: &mut R,
    out: &mut W,
) -> Result<(), String> {
    let planned = plan(design, &req.profile, Some(&req.params))?;
    let opts = CompileOptions {
        origin: planned.origin,
        kind: if req.red_only {
            ListKind::DryRun
        } else {
            ListKind::Mark
        },
        ..CompileOptions::default()
    };
    let list =
        compile::compile(&planned.toolpath, &planned.device, &opts).map_err(|e| e.to_string())?;
    for w in &planned.warnings {
        show(out, &format!("Aviso: {w}"))?;
    }
    describe_list(&list, out)?;
    if let Some(path) = &req.dump {
        let mut text = String::new();
        for (i, c) in list.commands.iter().enumerate() {
            text.push_str(&format!("{i:06} {}\n", Describe(c)));
        }
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        show(out, &format!("Lista escrita en {}", path.display()))?;
    }
    if req.dry_run {
        return show(out, "Simulacro (--dry-run): no se conectó con la tarjeta.");
    }
    show(out, EXPERIMENTAL)?;
    let emulated = req.connect.emu;
    let cfg = session_config(&planned, &req.profile, emulated)?;
    if emulated {
        let (t, h) = emu::emulator(EmuConfig {
            source: planned.device.source,
            field_mm: planned.device.field_mm,
            ..EmuConfig::default()
        });
        show(
            out,
            "Usando la tarjeta simulada (--emu): no se toca el USB.",
        )?;
        let r = mark_with(t, cfg, &list, &req, cancel, input, out);
        describe_emulator(&h, out)?;
        return r;
    }
    // Sin la bandera no se abre ni el USB.
    require_ack(&req.connect)?;
    let t = open_usb(&req.connect)?;
    mark_with(t, cfg, &list, &req, cancel, input, out)
}

fn mark_with<T: Transport, R: BufRead, W: Write>(
    t: T,
    cfg: SessionConfig,
    list: &CompiledList,
    req: &MarkRequest,
    cancel: &AtomicBool,
    input: &mut R,
    out: &mut W,
) -> Result<(), String> {
    let mut session = GalvoSession::new(t, cfg).map_err(|e| e.to_string())?;
    let info = session.probe().map_err(|e| e.to_string())?;
    describe_info(&info, out)?;
    if info.status.busy {
        return Err("la tarjeta está ocupada con otro trabajo".into());
    }
    let simulated = info.transport.emulated;
    if req.red_only {
        show(
            out,
            "--red-only: el trabajo se recorre con el láser DESHABILITADO y el puntero rojo.",
        )?;
    } else if !req.yes && !simulated {
        let ok = crate::link::confirm(
            "El láser DISPARARÁ. ¿Gafas puestas, recinto cerrado, obturador y paro de emergencia \
             listos, y el material correcto bajo el haz? [s/N] ",
            input,
            out,
        )
        .map_err(|e| e.to_string())?;
        if !ok {
            return Err("cancelado: no se envió nada".into());
        }
    }
    let mut last_step = usize::MAX;
    let mut progress = |p: Progress| {
        let pct = p.bytes_sent * 100 / p.bytes_total.max(1);
        let step = pct / 10;
        if step != last_step || p.batch == p.batches {
            last_step = step;
            let _ = writeln!(out, "  {pct:>3} %  (tanda {}/{})", p.batch, p.batches);
        }
    };
    let control = session.control();
    let result = with_ctrl_c(&control, cancel, || {
        session.send_job(list, &control, &mut progress)
    });
    match result {
        Ok(r) => show(
            out,
            &format!(
                "Enviado: {} tandas, {} comandos, {} bytes en {:.1} s. {} La fuente se apagó y la \
                 lista se vació.",
                r.batches,
                r.commands,
                r.bytes,
                r.elapsed.as_secs_f64(),
                if r.armed {
                    "La fuente estuvo armada durante el trabajo."
                } else {
                    "La fuente NO se armó."
                }
            ),
        ),
        Err(e) => Err(format!("falló el marcado: {}", job_error(&e))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Wrap {
        #[command(subcommand)]
        action: GalvoCommand,
    }

    fn parse(args: &[&str]) -> GalvoCommand {
        Wrap::try_parse_from(std::iter::once("galvo").chain(args.iter().copied()))
            .unwrap_or_else(|e| panic!("{e}"))
            .action
    }

    fn try_parse(args: &[&str]) -> Result<GalvoCommand, String> {
        Wrap::try_parse_from(std::iter::once("galvo").chain(args.iter().copied()))
            .map(|w| w.action)
            .map_err(|e| e.to_string())
    }

    fn design_file() -> String {
        format!("{}/tests/fixtures/design.svg", env!("CARGO_MANIFEST_DIR"))
    }

    fn run_mark(extra: &[&str], stdin: &str) -> (Result<(), String>, String) {
        let design = design_file();
        let mut args = vec![
            "mark",
            design.as_str(),
            "--mode",
            "line",
            "--frequency",
            "30",
        ];
        args.extend(extra);
        let GalvoCommand::Mark {
            design,
            connect,
            profile,
            params,
            red_only,
            dry_run,
            dump,
            yes,
        } = parse(&args)
        else {
            panic!("no es mark")
        };
        let mut out = Vec::new();
        let r = mark(
            design,
            MarkRequest {
                connect,
                profile,
                params,
                red_only,
                dry_run,
                dump,
                yes,
            },
            &AtomicBool::new(false),
            &mut io::Cursor::new(stdin.as_bytes().to_vec()),
            &mut out,
        );
        (r, String::from_utf8(out).unwrap())
    }

    #[test]
    fn mark_against_the_emulator_runs_and_leaves_the_source_off() {
        let (r, out) = run_mark(&["--emu"], "");
        r.unwrap_or_else(|e| panic!("{e}\n{out}"));
        assert!(out.contains("EXPERIMENTAL"), "{out}");
        assert!(out.contains("tarjeta simulada"), "{out}");
        assert!(out.contains("Enviado:"), "{out}");
        assert!(out.contains("estuvo armada durante el trabajo"), "{out}");
        assert!(out.contains("la fuente terminó apagada"), "{out}");
        assert!(out.contains("0 violaciones"), "{out}");
        assert!(out.contains("ninguno disparó"), "{out}");
    }

    #[test]
    fn real_hardware_requires_the_safety_flag_before_touching_usb() {
        let (r, out) = run_mark(&[], "s\n");
        let e = r.unwrap_err();
        assert!(e.contains("--i-understand-laser-safety"), "{e}");
        assert!(!out.contains("Tarjeta:"), "no se sondeó nada: {out}");
        // light y status igual.
        let design = design_file();
        let GalvoCommand::Light {
            design,
            connect,
            profile,
            contour,
            loops,
        } = parse(&["light", design.as_str(), "--mode", "line"])
        else {
            panic!()
        };
        let e = light(
            design,
            LightRequest {
                connect,
                profile,
                contour,
                loops,
            },
            &AtomicBool::new(false),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(e.contains("--i-understand-laser-safety"), "{e}");
        let GalvoCommand::Status { connect, profile } = parse(&["status"]) else {
            panic!()
        };
        let e = status(&connect, &profile, &mut Vec::new()).unwrap_err();
        assert!(e.contains("--i-understand-laser-safety"), "{e}");
    }

    #[test]
    fn dry_run_compiles_without_connecting_or_the_flag() {
        let (r, out) = run_mark(&["--dry-run"], "");
        r.unwrap_or_else(|e| panic!("{e}\n{out}"));
        assert!(out.contains("Simulacro"), "{out}");
        assert!(out.contains("tandas de 256"), "{out}");
        assert!(!out.contains("Tarjeta:"), "{out}");
    }

    #[test]
    fn declining_the_confirmation_sends_nothing_on_non_simulated_boards() {
        // La confirmación solo se pide con hardware; con --emu no hay nada que arriesgar.
        let (r, out) = run_mark(&["--emu"], "n\n");
        r.unwrap_or_else(|e| panic!("{e}\n{out}"));
    }

    #[test]
    fn red_only_never_arms_the_source() {
        let (r, out) = run_mark(&["--emu", "--red-only"], "");
        r.unwrap_or_else(|e| panic!("{e}\n{out}"));
        assert!(out.contains("La fuente NO se armó"), "{out}");
        assert!(out.contains("0 tramos con el haz saliendo"), "{out}");
    }

    #[test]
    fn power_above_the_profile_is_refused_before_connecting() {
        let (r, out) = run_mark(&["--emu", "--power", "50", "--max-power", "40"], "");
        let e = r.unwrap_err();
        assert!(e.contains("potencia"), "{e}");
        assert!(!out.contains("Tarjeta:"), "{out}");
    }

    #[test]
    fn frequency_outside_the_profile_range_is_refused() {
        // 30 kHz está fuera de 5-25.
        let (r, out) = run_mark(&["--emu", "--freq-range", "5-25"], "");
        let e = r.unwrap_err();
        assert!(e.contains("frecuencia"), "{e}");
        assert!(!out.contains("Tarjeta:"), "{out}");
    }

    #[test]
    fn mopa_needs_a_pulse_width_from_the_list() {
        let (r, _) = run_mark(&["--emu", "--source", "fiber-mopa"], "");
        assert!(r.is_err());
        let (r, out) = run_mark(
            &[
                "--emu",
                "--source",
                "fiber-mopa",
                "--pulse-widths",
                "4,9,13",
                "--pulse-width",
                "9",
            ],
            "",
        );
        r.unwrap_or_else(|e| panic!("{e}\n{out}"));
        let (r, _) = run_mark(
            &[
                "--emu",
                "--source",
                "fiber-mopa",
                "--pulse-widths",
                "4,9,13",
                "--pulse-width",
                "10",
            ],
            "",
        );
        assert!(r.is_err());
    }

    #[test]
    fn dump_writes_the_decoded_list() {
        let path = std::env::temp_dir().join(format!("tlaku-galvo-{}.txt", std::process::id()));
        let p = path.to_string_lossy().to_string();
        let (r, out) = run_mark(&["--dry-run", "--dump", &p], "");
        r.unwrap_or_else(|e| panic!("{e}\n{out}"));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("listReadyMark"), "{text}");
        assert!(text.contains("listMarkTo"), "{text}");
        assert!(text.contains("listEndOfList"), "{text}");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn light_against_the_emulator_draws_with_the_red_pointer_only() {
        let design = design_file();
        let GalvoCommand::Light {
            design,
            connect,
            profile,
            contour,
            loops,
        } = parse(&[
            "light",
            design.as_str(),
            "--mode",
            "line",
            "--emu",
            "--loops",
            "2",
        ])
        else {
            panic!()
        };
        let mut out = Vec::new();
        light(
            design,
            LightRequest {
                connect,
                profile,
                contour,
                loops,
            },
            &AtomicBool::new(false),
            &mut out,
        )
        .unwrap();
        let out = String::from_utf8(out).unwrap();
        assert!(out.contains("el láser NO se habilita"), "{out}");
        assert!(out.contains("2 vueltas"), "{out}");
        assert!(out.contains("0 tramos con el haz saliendo"), "{out}");
        assert!(out.contains("terminó apagada"), "{out}");
    }

    #[test]
    fn contour_frames_follow_the_design() {
        let design = design_file();
        let GalvoCommand::Light {
            design,
            connect,
            profile,
            contour,
            loops,
        } = parse(&[
            "light",
            design.as_str(),
            "--mode",
            "line",
            "--emu",
            "--loops",
            "1",
            "--contour",
        ])
        else {
            panic!()
        };
        let mut out = Vec::new();
        light(
            design,
            LightRequest {
                connect,
                profile,
                contour,
                loops,
            },
            &AtomicBool::new(false),
            &mut out,
        )
        .unwrap();
        assert!(String::from_utf8(out).unwrap().contains("1 vueltas"));
    }

    #[test]
    fn ctrl_c_stops_the_job_and_reports_the_source_off() {
        let design = design_file();
        let GalvoCommand::Mark {
            design,
            connect,
            profile,
            params,
            red_only,
            dry_run,
            dump,
            yes,
        } = parse(&[
            "mark",
            design.as_str(),
            "--mode",
            "line",
            "--frequency",
            "30",
            "--emu",
        ])
        else {
            panic!()
        };
        let mut out = Vec::new();
        let r = mark(
            design,
            MarkRequest {
                connect,
                profile,
                params,
                red_only,
                dry_run,
                dump,
                yes,
            },
            // Ya activo: el hilo lo traslada y el trabajo se cancela en cuanto la sesión lo ve.
            &AtomicBool::new(true),
            &mut io::Cursor::new(Vec::new()),
            &mut out,
        );
        let out = String::from_utf8(out).unwrap();
        match r {
            Err(e) => assert!(e.contains("cancelado"), "{e}"),
            Ok(()) => assert!(out.contains("Enviado:")),
        }
        assert!(out.contains("terminó apagada"), "{out}");
    }

    #[test]
    fn status_with_the_emulator_is_read_only() {
        let GalvoCommand::Status { connect, profile } = parse(&["status", "--emu"]) else {
            panic!()
        };
        let mut out = Vec::new();
        status(&connect, &profile, &mut out).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("9588:9899"), "{s}");
        assert!(s.contains("SIMULADA"), "{s}");
        assert!(s.contains("Nada se movió"), "{s}");
    }

    #[test]
    fn options_are_validated_in_spanish() {
        let design = design_file();
        let err = |args: &[&str]| try_parse(args).err().expect("debía fallar");
        assert!(err(&["mark", &design, "--frequency", "0"]).contains("mayor que 0"));
        assert!(err(&["mark", &design]).contains("--frequency"));
        assert!(
            err(&[
                "mark",
                &design,
                "--frequency",
                "30",
                "--freq-range",
                "80-20"
            ])
            .contains("mínimo")
        );
        assert!(
            err(&["mark", &design, "--frequency", "30", "--freq-range", "20"]).contains("MIN-MAX")
        );
        assert!(err(&["status", "--require-input", "0x0001=0x0002"]).contains("fuera de MASK"));
    }

    #[test]
    fn input_checks_parse_hex_and_decimal() {
        assert_eq!(
            parse_input_check("0x0008=0x0008").unwrap(),
            InputCheck { mask: 8, value: 8 }
        );
        assert_eq!(
            parse_input_check("12=4").unwrap(),
            InputCheck { mask: 12, value: 4 }
        );
        assert!(parse_input_check("8").is_err());
    }
}
