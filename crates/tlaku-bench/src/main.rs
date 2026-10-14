//! `tlaku-bench`: prueba de estrés con archivos reales.
//!
//! ```text
//! tlaku-bench gen DIR                       # genera los archivos sintéticos
//! tlaku-bench run --corpus DIR --csv o.csv  # mide todo el corpus (un proceso por archivo)
//! tlaku-bench worker FILE                   # mide un archivo en este proceso (JSON por fase)
//! ```

mod driver;
mod report;
mod summary;
mod synth;
mod worker;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "tlaku-bench", about = "Prueba de estrés del motor de Tlaku")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Mide archivos: importar, calcular, G-code, .rd y emulador GRBL.
    Run {
        /// Carpetas (recursivo) o archivos.
        paths: Vec<PathBuf>,
        /// Solo archivos cuya ruta contenga este texto.
        #[arg(long)]
        filter: Option<String>,
        /// CSV de salida.
        #[arg(long)]
        csv: Option<PathBuf>,
        /// Resumen en Markdown de salida.
        #[arg(long)]
        summary: Option<PathBuf>,
        /// Modos: `line`, `fill` (separados por coma).
        #[arg(long, default_value = "line")]
        modes: String,
        /// Umbral (s) para marcar una fase como lenta.
        #[arg(long, default_value_t = 10.0)]
        threshold: f64,
        /// Plazo duro (s) por archivo y modo; pasado ese tiempo se mata el proceso.
        #[arg(long, default_value_t = 300.0)]
        hard_timeout: f64,
        #[arg(long)]
        no_emu: bool,
        /// No se manda al emulador un G-code de más líneas que éstas.
        #[arg(long, default_value_t = 250_000)]
        emu_max_lines: usize,
        /// Aceleración máxima del reloj del emulador.
        #[arg(long, default_value_t = 400.0)]
        emu_max_speed: f64,
        /// Plazo (s reales) del envío al emulador.
        #[arg(long, default_value_t = 240.0)]
        emu_deadline: f64,
    },
    /// Mide un archivo en este proceso; imprime una línea JSON por fase (uso interno).
    Worker {
        file: PathBuf,
        #[arg(long, default_value = "line")]
        mode: String,
        #[arg(long, default_value_t = 10.0)]
        threshold: f64,
        #[arg(long)]
        no_emu: bool,
        #[arg(long, default_value_t = 250_000)]
        emu_max_lines: usize,
        #[arg(long, default_value_t = 400.0)]
        emu_max_speed: f64,
        #[arg(long, default_value_t = 240.0)]
        emu_deadline: f64,
    },
    /// Genera los archivos sintéticos (50 000 rectángulos, 1 M de nodos, texto denso...).
    Gen {
        dir: PathBuf,
        /// Solo los que contengan este texto en el nombre.
        #[arg(long)]
        only: Option<String>,
        /// Genera los extremos (4 M de nodos, 200 000 rectángulos...) en lugar del conjunto normal.
        #[arg(long)]
        xl: bool,
    },
}

fn parse_mode(s: &str) -> Result<worker::Mode, String> {
    match s {
        "line" => Ok(worker::Mode::Line),
        "fill" => Ok(worker::Mode::Fill),
        other => Err(format!("modo desconocido: {other} (usa line o fill)")),
    }
}

fn main() -> ExitCode {
    match real_main(Cli::parse()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn real_main(cli: Cli) -> Result<ExitCode, String> {
    match cli.cmd {
        Cmd::Gen { dir, only, xl } => {
            synth::run(&dir, only.as_deref(), xl)?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Worker {
            file,
            mode,
            threshold,
            no_emu,
            emu_max_lines,
            emu_max_speed,
            emu_deadline,
        } => {
            worker::run(
                &file,
                &worker::WorkerArgs {
                    mode: parse_mode(&mode)?,
                    threshold_s: threshold,
                    emu: !no_emu,
                    emu_max_lines,
                    emu_max_speed,
                    emu_deadline_s: emu_deadline,
                },
            );
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Run {
            paths,
            filter,
            csv,
            summary,
            modes,
            threshold,
            hard_timeout,
            no_emu,
            emu_max_lines,
            emu_max_speed,
            emu_deadline,
        } => {
            let mode_list: Vec<String> = modes.split(',').map(|m| m.trim().to_string()).collect();
            for m in &mode_list {
                parse_mode(m)?;
            }
            let mut files = Vec::new();
            for p in &paths {
                if p.is_dir() {
                    driver::collect(p, &mut files);
                } else {
                    files.push(p.clone());
                }
            }
            if let Some(f) = &filter {
                files.retain(|p| p.to_string_lossy().contains(f.as_str()));
            }
            if files.is_empty() {
                return Err("no hay archivos que medir".into());
            }
            let opts = driver::RunOptions {
                modes: mode_list,
                threshold_s: threshold,
                hard_timeout_s: hard_timeout,
                emu: !no_emu,
                emu_max_lines,
                emu_max_speed,
                emu_deadline_s: emu_deadline,
            };
            let total = files.len() * opts.modes.len();
            let mut rows = Vec::new();
            for (i, f) in files.iter().enumerate() {
                for m in &opts.modes {
                    let r = driver::run_one(f, m, &opts);
                    let p = summary::problems(&r);
                    eprintln!(
                        "[{}/{}] {} ({m}): {} total {:.1}s pico {:.0} MB{}",
                        rows.len() + 1,
                        total,
                        f.file_name()
                            .map(|n| n.to_string_lossy())
                            .unwrap_or_default(),
                        r.status,
                        r.total_s,
                        r.peak_rss_mb,
                        if p.is_empty() {
                            String::new()
                        } else {
                            format!(" !! {}", p.join("; "))
                        }
                    );
                    rows.push(r);
                    let _ = i;
                    if let Some(c) = &csv {
                        std::fs::write(c, driver::csv_text(&rows))
                            .map_err(|e| format!("{}: {e}", c.display()))?;
                    }
                }
            }
            let text = summary::render(&rows, threshold);
            if let Some(s) = &summary {
                std::fs::write(s, &text).map_err(|e| format!("{}: {e}", s.display()))?;
            }
            println!("{text}");
            let bad = rows.iter().any(|r| !summary::problems(r).is_empty());
            Ok(if bad {
                ExitCode::from(2)
            } else {
                ExitCode::SUCCESS
            })
        }
    }
}
