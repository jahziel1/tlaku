//! Compila `community/materials/*.json` al índice del catálogo (`catalog/v1/index.json`).
//!
//! ```sh
//! cargo run -q -p tlaku-doc --example community_catalog -- --out site-dist/catalog/v1/index.json
//! cargo run -q -p tlaku-doc --example community_catalog -- --check     # solo valida
//! ```
//!
//! Lo corre el flujo del sitio (`.github/workflows/pages.yml`) y el de CI. Valida cada aporte con las
//! reglas de `tlaku_doc::community` y el índice resultante con las mismas que aplicará la app al
//! descargarlo: si algo no pasa, falla y no se publica nada. Opciones: `--dir` (por omisión
//! `community/materials`), `--out` (archivo; crea las carpetas) y `--date AAAA-MM-DD` (por omisión, hoy en UTC).

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

/// Día de hoy en UTC como `AAAA-MM-DD` (calendario gregoriano; algoritmo de días civiles).
fn today() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

fn run() -> Result<(), String> {
    let mut dir = PathBuf::from("community/materials");
    let mut out: Option<PathBuf> = None;
    let mut date = today();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| {
            args.next()
                .ok_or_else(|| format!("{name} necesita un valor"))
        };
        match arg.as_str() {
            "--dir" => dir = PathBuf::from(value("--dir")?),
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--date" => date = value("--date")?,
            "--check" => {}
            other => return Err(format!("argumento desconocido: {other}")),
        }
    }
    let index = tlaku_doc::build_index_from_dir(&dir, &date)?;
    let text = serde_json::to_string(&index).map_err(|e| e.to_string())?;
    if let Some(out) = out {
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&out, &text).map_err(|e| format!("{}: {e}", out.display()))?;
        eprintln!(
            "catálogo: {} ajustes, {} bytes -> {}",
            index.entries.len(),
            text.len(),
            out.display()
        );
    } else {
        eprintln!("catálogo válido: {} ajustes", index.entries.len());
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
