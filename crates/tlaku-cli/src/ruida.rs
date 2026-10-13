//! Subcomando `tlaku ruida`: exporta un trabajo a un archivo `.rd` de controladoras Ruida.
//! EXPERIMENTAL: no se ha probado con ninguna máquina real (ver docs/hardware/ruida.md).

use crate::cli::DesignArgs;
use crate::{job, report};
use clap::Subcommand;
use std::path::PathBuf;
use tlaku_pipeline::ComputedJob;
use tlaku_ruida::cmd;
use tlaku_ruida::rd::{self, LayerOptions, RuidaJobOptions};
use tlaku_ruida::safety::SafetyLimits;
use tlaku_ruida::swizzle::{MAGIC_DEFAULT, Magic};

#[derive(Subcommand)]
pub enum RuidaCommand {
    /// Convierte un SVG, DXF o imagen en un archivo `.rd` (experimental, sin probar en hardware).
    /// Una capa de la controladora por cada bloque del trabajo, con potencia constante por capa.
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
        /// Invierte Y respecto a la altura de la cama (el origen de Ruida no está confirmado)
        #[arg(long)]
        flip_y: bool,
        /// No emite `D8 00` (iniciar proceso) al principio del archivo
        #[arg(long)]
        no_process_start: bool,
    },
}

fn parse_magic(s: &str) -> Result<u8, String> {
    let t = s.trim().trim_start_matches("0x").trim_start_matches("0X");
    u8::from_str_radix(t, 16).map_err(|_| format!("'{s}' no es un byte en hexadecimal (ej. 0x88)"))
}

/// Genera los bytes del `.rd` (en claro) de un trabajo ya calculado. Usa el color de cada capa y
/// la cama indicada; los demás ajustes son los de [`RuidaJobOptions`] por defecto.
pub fn export_rd(
    built: &ComputedJob,
    bed: (f64, f64),
    flip_y: bool,
    emit_process_start: bool,
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
        flip_y,
        emit_process_start,
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
            flip_y,
            no_process_start,
        } => {
            let request = design.into_request()?;
            let built = job::build(&request)?;
            tlaku_pipeline::check_bounds(&built, [request.bed.0, request.bed.1])?;
            let bytes = export_rd(&built, request.bed, flip_y, !no_process_start)?;
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
                "Escrito {} ({} bytes, {kind}). EXPERIMENTAL: sin probar con hardware.",
                output.display(),
                out.len()
            );
            if magic != MAGIC_DEFAULT && !plain {
                println!("Aviso: se usó un magic distinto de 0x88.");
            }
            Ok(())
        }
    }
}
