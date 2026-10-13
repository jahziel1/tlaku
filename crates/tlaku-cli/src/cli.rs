//! Definición de la línea de comandos y ejecución de cada subcomando.

use crate::job::{self, DesignRequest};
use crate::link;
use crate::options::{
    Defaults, ImageModeArg, ImageParams, LayerMode, LayerOrder, LayerOverride, Overscan, parse_f64,
    parse_layer, parse_overscan, parse_pos, parse_positive, parse_size,
};
use crate::{report, sigint};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::time::Duration;
use tlaku_core::Rgb;
use tlaku_grbl::serial::{self, Grbl};
use tlaku_pipeline::doc::{DeviceProfile, Optimization, PowerMode};
use tlaku_sim::MachineProfile;

/// Búfer de recepción de GRBL en bytes.
const DEFAULT_RX_BUFFER: usize = 127;

#[derive(Parser)]
#[command(
    name = "tlaku",
    version,
    about = "Software abierto para cortadoras y grabadoras láser"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Convierte un SVG, DXF o imagen en G-code para GRBL y muestra el resumen del trabajo.
    Gcode {
        #[command(flatten)]
        design: DesignArgs,
        #[arg(short, long)]
        output: PathBuf,
        /// Valor de S para 100 % de potencia ($30 de la máquina)
        #[arg(long, default_value_t = 1000.0, value_parser = parse_positive)]
        s_max: f64,
        /// dynamic = M4 (la potencia sigue a la velocidad), constant = M3
        #[arg(long, value_enum, default_value_t = PowerModeArg::Dynamic)]
        power_mode: PowerModeArg,
        /// Archivo con G-code que se inserta antes del trabajo
        #[arg(long)]
        start_gcode: Option<PathBuf>,
        /// Archivo con G-code que se inserta al terminar
        #[arg(long)]
        end_gcode: Option<PathBuf>,
    },
    /// Imprime el orden de corte con la razón de cada paso, sin generar G-code.
    Plan {
        #[command(flatten)]
        design: DesignArgs,
    },
    /// Envía un archivo de G-code a la máquina con conteo de caracteres. Ctrl+C cancela.
    Send {
        file: PathBuf,
        #[command(flatten)]
        link: LinkArgs,
    },
    /// Recorre con el láser apagado el rectángulo que ocupa el diseño (incluido el overscan).
    Frame {
        #[command(flatten)]
        design: DesignArgs,
        #[command(flatten)]
        link: LinkArgs,
    },
    /// Envía comandos sueltos de GRBL ($X, $H, $I, G-code...) y muestra la respuesta completa.
    /// Los de solo lectura ($I, $$, $#, $G, $N, ?) se envían sin preguntar; `$X`, `$H` y todo lo
    /// que mueva la máquina, encienda el láser o cambie ajustes pide confirmación (`--yes` la
    /// omite). Tiempo real por nombre: status, hold, resume, reset. Escribe los comandos con
    /// `$` entre comillas simples: tlaku cmd --port /dev/cu.X '$H'
    Cmd {
        #[arg(short, long)]
        port: String,
        #[arg(long, default_value_t = serial::DEFAULT_BAUD)]
        baud: u32,
        /// No pedir confirmación antes de enviar
        #[arg(short, long)]
        yes: bool,
        /// Comandos en orden; cada argumento es un comando. Se detiene en el primer error
        #[arg(required = true, value_name = "COMANDO")]
        commands: Vec<String>,
    },
    /// Soporte experimental para controladoras Ruida (CO2): exporta archivos `.rd`.
    Ruida {
        #[command(subcommand)]
        action: crate::ruida::RuidaCommand,
    },
    /// Soporte experimental para K40 con placa Lihuiyu M2 Nano (USB): envía, estado y `.egv`.
    K40 {
        #[command(subcommand)]
        action: crate::k40::K40Command,
    },
    /// Lista los puertos serie disponibles
    Ports,
    /// Lee la versión ($I) y la configuración ($$) de una máquina GRBL sin moverla
    Info {
        #[arg(short, long)]
        port: String,
        #[arg(long, default_value_t = serial::DEFAULT_BAUD)]
        baud: u32,
    },
}

#[derive(Args)]
pub struct LinkArgs {
    /// Puerto serie (ver `tlaku ports`)
    #[arg(short, long)]
    pub port: String,
    #[arg(long, default_value_t = serial::DEFAULT_BAUD)]
    pub baud: u32,
    /// Bytes del búfer de recepción de la controladora (GRBL: 127)
    #[arg(long, default_value_t = DEFAULT_RX_BUFFER)]
    pub rx_buffer: usize,
    /// No pedir confirmación antes de mover la máquina
    #[arg(short, long)]
    pub yes: bool,
}

/// Opciones comunes a los subcomandos que leen un diseño (`gcode`, `plan`, `frame`).
#[derive(Args)]
pub struct DesignArgs {
    /// Diseño de entrada: .svg, .dxf o imagen (.png, .jpg, .jpeg, .bmp)
    pub input: PathBuf,
    /// Modo de las capas sin ajuste propio
    #[arg(long, value_enum)]
    mode: Option<LayerMode>,
    /// Velocidad por defecto en mm/min (valor del crate de cada modo si se omite). En `frame`
    /// es además la velocidad del recorrido (3000 si se omite)
    #[arg(long, value_parser = parse_positive)]
    pub speed: Option<f64>,
    /// Potencia por defecto en % (en imágenes, la potencia máxima)
    #[arg(long, value_parser = parse_f64)]
    power: Option<f64>,
    #[arg(long)]
    passes: Option<u32>,
    /// Activa la asistencia de aire (M8) por defecto
    #[arg(long)]
    air: bool,
    /// Ajustes de una capa: "#ff0000:mode=fill,speed=300,power=100,passes=2,air,interval=0.1,
    /// angle=0,crosshatch,nobidir,overscan=auto,kerf=0.1,order=2". Repetible. `order=N` fija la
    /// posición (desde 1) de la capa en el orden final y gana sobre --layer-order.
    #[arg(long = "layer", value_parser = parse_layer)]
    layers: Vec<(Rgb, LayerOverride)>,
    /// Orden de las capas: auto = grabados (relleno, imagen) primero, líneas después y cortes
    /// (kerf, o potencia >= 80 % con varias pasadas) al final; file = el del archivo
    #[arg(long, value_enum, default_value_t = LayerOrder::Auto)]
    layer_order: LayerOrder,
    /// Optimización: no cortar primero lo que está dentro de otra figura
    #[arg(long)]
    no_inner_first: bool,
    /// Optimización: conservar el orden del archivo en vez de acortar recorridos en vacío
    #[arg(long)]
    no_reduce_travel: bool,
    /// Optimización: las figuras cerradas empiezan en su primer vértice
    #[arg(long)]
    no_best_start: bool,
    /// Optimización: las figuras abiertas no se recorren al revés
    #[arg(long)]
    no_choose_direction: bool,
    /// Optimización: mezclar las capas de corte consecutivas con exactamente los mismos ajustes
    /// para acortar recorridos (las de ajustes distintos nunca se mezclan)
    #[arg(long)]
    mix_layers: bool,
    /// Optimización: quitar los tramos que se cortarían dos veces dentro de cada capa line;
    /// tolerancia en mm (0.01 si se omite). No se aplica con kerf
    #[arg(long, value_name = "MM", num_args = 0..=1, default_missing_value = "0.01", value_parser = parse_f64)]
    remove_overlaps: Option<f64>,
    /// Optimización: cerradas siempre en el mismo sentido y rellenos en un solo sentido, para
    /// esconder el juego mecánico (más lento)
    #[arg(long)]
    hide_backlash: bool,
    /// Separación entre líneas de relleno e imagen, en mm
    #[arg(long, value_parser = parse_positive)]
    interval: Option<f64>,
    /// Overscan de rellenos e imágenes: "auto" (por defecto, según la velocidad y la
    /// aceleración de la máquina) o mm
    #[arg(long, value_parser = parse_overscan)]
    overscan: Option<Overscan>,
    /// Área de trabajo en mm, ANCHOxALTO (Falcon2: 400x415)
    #[arg(long, default_value = "400x415", value_parser = parse_size)]
    bed: (f64, f64),
    /// Error máximo al convertir curvas en rectas, en mm
    #[arg(long, default_value_t = 0.02, value_parser = parse_positive)]
    tolerance: f64,
    /// SVG: no invertir el eje Y (por defecto el origen es abajo a la izquierda)
    #[arg(long)]
    no_flip: bool,
    /// Aceleración de la máquina en mm/s² para estimar el tiempo y el overscan (ambos ejes)
    #[arg(long, value_parser = parse_positive)]
    machine_accel: Option<f64>,
    /// Velocidad máxima de la máquina en mm/min para estimar el tiempo (ambos ejes)
    #[arg(long, value_parser = parse_positive)]
    machine_max_rate: Option<f64>,
    /// Imagen: tamaño en mm, ANCHOxALTO
    #[arg(long, value_parser = parse_size)]
    image_size: Option<(f64, f64)>,
    /// Imagen: esquina inferior izquierda en mm, X,Y
    #[arg(long, value_parser = parse_pos, allow_hyphen_values = true)]
    image_pos: Option<(f64, f64)>,
    /// Imagen: método de conversión a puntos (por defecto stucki)
    #[arg(long, value_enum)]
    image_mode: Option<ImageModeArg>,
    /// Imagen: umbral 0-1 para --image-mode threshold (0.5)
    #[arg(long, value_parser = parse_f64)]
    threshold: Option<f64>,
    /// Imagen: potencia mínima en % de los puntos encendidos (0)
    #[arg(long, value_parser = parse_f64)]
    min_power: Option<f64>,
    /// Imagen: brillo de -1 a 1
    #[arg(long, value_parser = parse_f64, allow_hyphen_values = true)]
    brightness: Option<f64>,
    /// Imagen: contraste de -1 a 1
    #[arg(long, value_parser = parse_f64, allow_hyphen_values = true)]
    contrast: Option<f64>,
    /// Imagen: gamma (1 = sin cambio)
    #[arg(long, value_parser = parse_f64)]
    gamma: Option<f64>,
    /// Imagen: invertir (negativo)
    #[arg(long)]
    invert: bool,
    /// Imagen: ancho del punto del haz en mm; acorta cada tramo encendido
    #[arg(long, value_parser = parse_f64)]
    dot_width: Option<f64>,
}

impl DesignArgs {
    pub fn into_request(self) -> Result<DesignRequest, String> {
        let mut profile = MachineProfile::default();
        if let Some(a) = self.machine_accel {
            profile.accel_mm_s2 = [a, a];
        }
        if let Some(r) = self.machine_max_rate {
            profile.max_rate_mm_min = [r, r];
            profile.rapid_rate_mm_min = r;
        }
        let given: Vec<&'static str> = [
            ("--image-size", self.image_size.is_some()),
            ("--image-pos", self.image_pos.is_some()),
            ("--image-mode", self.image_mode.is_some()),
            ("--threshold", self.threshold.is_some()),
            ("--min-power", self.min_power.is_some()),
            ("--brightness", self.brightness.is_some()),
            ("--contrast", self.contrast.is_some()),
            ("--gamma", self.gamma.is_some()),
            ("--invert", self.invert),
            ("--dot-width", self.dot_width.is_some()),
        ]
        .into_iter()
        .filter_map(|(name, on)| on.then_some(name))
        .collect();
        Ok(DesignRequest {
            input: self.input,
            defaults: Defaults {
                mode: self.mode,
                speed: self.speed,
                power: self.power,
                passes: self.passes,
                air: self.air.then_some(true),
                interval: self.interval,
                overscan: self.overscan,
            },
            overrides: self.layers,
            image: ImageParams {
                size: self.image_size,
                pos: self.image_pos,
                mode: self.image_mode,
                threshold: self.threshold,
                min_power: self.min_power,
                brightness: self.brightness,
                contrast: self.contrast,
                gamma: self.gamma,
                invert: self.invert,
                dot_width: self.dot_width,
                given,
            },
            tolerance_mm: self.tolerance,
            flip_y: !self.no_flip,
            profile,
            bed: self.bed,
            layer_order: self.layer_order,
            optimization: Optimization {
                inner_first: !self.no_inner_first,
                reduce_travel: !self.no_reduce_travel,
                best_start: !self.no_best_start,
                choose_direction: !self.no_choose_direction,
                by_layer: !self.mix_layers,
                remove_overlaps: self.remove_overlaps.is_some(),
                overlap_tolerance: self.remove_overlaps.unwrap_or(0.01),
                hide_backlash: self.hide_backlash,
                ..Optimization::default()
            },
        })
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub enum PowerModeArg {
    Dynamic,
    Constant,
}

impl From<PowerModeArg> for PowerMode {
    fn from(m: PowerModeArg) -> Self {
        match m {
            PowerModeArg::Dynamic => PowerMode::Dynamic,
            PowerModeArg::Constant => PowerMode::Constant,
        }
    }
}

fn read_script(path: Option<PathBuf>) -> Result<Option<String>, String> {
    path.map(|p| std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display())))
        .transpose()
}

/// Pide confirmación en la terminal salvo con `--yes`.
fn ask_to_proceed(yes: bool) -> Result<(), String> {
    if yes {
        return Ok(());
    }
    let ok = link::confirm(
        link::READY_PROMPT,
        &mut io::stdin().lock(),
        &mut io::stdout(),
    )
    .map_err(|e| e.to_string())?;
    if ok {
        Ok(())
    } else {
        Err("cancelado: no se movió la máquina".into())
    }
}

fn open_port(link: &LinkArgs) -> Result<Box<dyn serial::Transport>, String> {
    println!("Conectando a {} ({} baudios)...", link.port, link.baud);
    Ok(Grbl::open(&link.port, link.baud)
        .map_err(|e| format!("{}: {e}", link.port))?
        .into_transport())
}

fn stream(link: &LinkArgs, lines: &[String]) -> Result<(), String> {
    let transport = open_port(link)?;
    let cancel = sigint::install();
    link::send_job(
        transport,
        lines,
        link::streamer_config(link.rx_buffer),
        &link::PreflightConfig {
            port: link.port.clone(),
            ..link::PreflightConfig::default()
        },
        cancel,
        io::stdout().is_terminal(),
        &mut io::stdout(),
    )
}

pub fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Gcode {
            design,
            output,
            s_max,
            power_mode,
            start_gcode,
            end_gcode,
        } => {
            let start_gcode = read_script(start_gcode)?;
            let end_gcode = read_script(end_gcode)?;
            let request = design.into_request()?;
            let built = job::build(&request)?;
            tlaku_pipeline::check_bounds(&built, [request.bed.0, request.bed.1])?;
            let device = DeviceProfile {
                s_max,
                power_mode: power_mode.into(),
                start_gcode: start_gcode.unwrap_or_default(),
                end_gcode: end_gcode.unwrap_or_default(),
                ..request.device()
            };
            let g = tlaku_pipeline::gcode(&built, &device);
            std::fs::write(&output, g).map_err(|e| format!("{}: {e}", output.display()))?;
            print!("{}", report::summary(&built));
            println!("Escrito {}", output.display());
            Ok(())
        }
        Command::Plan { design } => {
            let built = job::build(&design.into_request()?)?;
            print!("{}", report::plan_text(&built));
            Ok(())
        }
        Command::Send { file, link } => {
            let text =
                std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            let count = link::validate_gcode(&text, link.rx_buffer)?;
            println!("{}: {count} líneas de G-code.", file.display());
            ask_to_proceed(link.yes)?;
            let lines: Vec<String> = text.lines().map(str::to_owned).collect();
            stream(&link, &lines)
        }
        Command::Frame { design, link } => {
            let speed = design.speed.unwrap_or(link::DEFAULT_FRAME_SPEED);
            let request = design.into_request()?;
            let built = job::build(&request)?;
            tlaku_pipeline::check_bounds(&built, [request.bed.0, request.bed.1])?;
            let bounds = built.bounds().ok_or("el diseño no tiene movimientos")?;
            print!("{}", report::order_line(&built));
            print!("{}", link::frame_description(bounds, speed));
            ask_to_proceed(link.yes)?;
            stream(&link, &link::frame_gcode(bounds, speed))
        }
        Command::Cmd {
            port,
            baud,
            yes,
            commands,
        } => {
            let cmds = commands
                .iter()
                .map(|c| link::parse_command(c, DEFAULT_RX_BUFFER))
                .collect::<Result<Vec<_>, _>>()?;
            link::confirm_commands(&cmds, yes, &mut io::stdin().lock(), &mut io::stdout())?;
            println!("Conectando a {port} ({baud} baudios)...");
            let mut transport = Grbl::open(&port, baud)
                .map_err(|e| format!("{port}: {e}"))?
                .into_transport();
            link::run_commands(
                &mut transport,
                &cmds,
                &link::CmdConfig::default(),
                &mut io::stdout(),
            )
        }
        Command::Ruida { action } => crate::ruida::run(action),
        Command::K40 { action } => crate::k40::run(action),
        Command::Ports => {
            for p in serial::list_ports().map_err(|e| e.to_string())? {
                println!("{p}");
            }
            Ok(())
        }
        Command::Info { port, baud } => {
            let mut grbl = Grbl::open(&port, baud).map_err(|e| format!("{port}: {e}"))?;
            for cmd in ["$I", "$$"] {
                println!("> {cmd}");
                for l in grbl
                    .command(cmd, Duration::from_secs(5))
                    .map_err(|e| e.to_string())?
                {
                    println!("{l}");
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("tlaku").chain(args.iter().copied()))
    }

    #[test]
    fn power_mode_arg_maps_to_core() {
        assert_eq!(PowerMode::from(PowerModeArg::Dynamic), PowerMode::Dynamic);
        assert_eq!(PowerMode::from(PowerModeArg::Constant), PowerMode::Constant);
    }

    #[test]
    fn missing_script_reports_path() {
        let err = read_script(Some(PathBuf::from("/no/existe.gcode"))).unwrap_err();
        assert!(err.contains("/no/existe.gcode"));
        assert_eq!(read_script(None), Ok(None));
    }

    #[test]
    fn gcode_accepts_all_options() {
        let cli = parse(&[
            "gcode",
            "a.svg",
            "-o",
            "a.gcode",
            "--power-mode",
            "constant",
            "--start-gcode",
            "s.txt",
            "--end-gcode",
            "e.txt",
            "--layer",
            "#ff0000:speed=300",
            "--layer",
            "#00ff00:mode=fill",
            "--machine-accel",
            "500",
            "--machine-max-rate",
            "6000",
            "--overscan",
            "auto",
            "--image-size",
            "10x20",
            "--image-pos",
            "-5,3",
            "--image-mode",
            "bayer4",
            "--invert",
            "--brightness",
            "-0.2",
        ])
        .unwrap();
        let Command::Gcode {
            design,
            power_mode,
            start_gcode,
            ..
        } = cli.command
        else {
            panic!("subcomando inesperado");
        };
        assert!(matches!(power_mode, PowerModeArg::Constant));
        assert_eq!(start_gcode, Some(PathBuf::from("s.txt")));
        let req = design.into_request().unwrap();
        assert_eq!(req.overrides.len(), 2);
        assert_eq!(req.profile.accel_mm_s2, [500.0, 500.0]);
        assert_eq!(req.profile.rapid_rate_mm_min, 6000.0);
        assert_eq!(req.image.pos, Some((-5.0, 3.0)));
        assert_eq!(req.image.mode, Some(ImageModeArg::Bayer4));
        assert_eq!(req.image.brightness, Some(-0.2));
        assert!(req.image.given.contains(&"--invert"));
        assert_eq!(req.defaults.overscan, Some(Overscan::Auto));
    }

    #[test]
    fn invalid_options_are_rejected_in_spanish() {
        let msg = |args: &[&str]| parse(args).err().expect("debía fallar").to_string();
        assert!(msg(&["gcode", "a.svg", "-o", "x", "--speed", "abc"]).contains("no es un número"));
        assert!(msg(&["gcode", "a.svg", "-o", "x", "--speed", "0"]).contains("mayor que 0"));
        assert!(msg(&["gcode", "a.svg", "-o", "x", "--bed", "400"]).contains("ANCHOxALTO"));
        assert!(
            msg(&["gcode", "a.svg", "-o", "x", "--layer", "#ff0000:foo=1"])
                .contains("clave desconocida")
        );
        assert!(msg(&["gcode", "a.svg", "-o", "x", "--power-mode", "otro"]).contains("otro"));
        assert!(msg(&["send", "a.gcode"]).contains("--port"));
    }
}
