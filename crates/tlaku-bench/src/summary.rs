//! Resumen en Markdown de una corrida.

use crate::report::Report;
use std::fmt::Write;

fn mb(b: u64) -> f64 {
    b as f64 / 1e6
}

fn short(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

/// Problemas de un informe aunque el estado sea `ok`.
pub fn problems(r: &Report) -> Vec<String> {
    let mut v = Vec::new();
    if r.status != "ok" && r.status != "na" {
        v.push(format!("{} en {}", r.status, r.error_phase));
    }
    if !r.slow.is_empty() {
        v.push(format!("lento: {}", r.slow));
    }
    if r.emu_status.starts_with("fallo") {
        v.push(format!("emulador: {}", r.emu_status));
    }
    if r.emu_rapid_burns > 0 {
        v.push(format!("láser encendido en {} rápidos", r.emu_rapid_burns));
    }
    v
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i]
}

pub fn render(rows: &[Report], threshold_s: f64) -> String {
    let mut s = String::new();
    let n = rows.len();
    let count = |st: &str| rows.iter().filter(|r| r.status == st).count();
    let _ = writeln!(s, "# Resumen de la corrida\n");
    let _ = writeln!(
        s,
        "- Ejecuciones: {n} (ok {}, sin nada que producir en relleno {}, error {}, pánico {}, crash {}, timeout {})",
        count("ok"),
        count("na"),
        count("error"),
        count("panic"),
        count("crash"),
        count("timeout")
    );
    let slow: Vec<_> = rows.iter().filter(|r| !r.slow.is_empty()).collect();
    let _ = writeln!(
        s,
        "- Con alguna fase de más de {threshold_s:.0} s: {}",
        slow.len()
    );
    let rapid: Vec<_> = rows.iter().filter(|r| r.emu_rapid_burns > 0).collect();
    let _ = writeln!(
        s,
        "- Archivos donde el láser se encendió en un rápido (emulador): {}",
        rapid.len()
    );
    let emu_ok: Vec<&Report> = rows.iter().filter(|r| r.emu_status == "ok").collect();
    let _ = writeln!(s, "- Pasaron por el emulador: {}", emu_ok.len());
    let mut diffs: Vec<f64> = emu_ok
        .iter()
        .filter(|r| r.est_s >= 20.0)
        .map(|r| r.emu_diff_pct)
        .collect();
    diffs.sort_by(f64::total_cmp);
    if !diffs.is_empty() {
        let mut abs: Vec<f64> = diffs.iter().map(|d| d.abs()).collect();
        abs.sort_by(f64::total_cmp);
        let _ = writeln!(
            s,
            "- Diferencia simulado vs estimado (trabajos de >= 20 s, {} casos): mediana {:+.2} %, |p90| {:.2} %, |máx| {:.2} %",
            diffs.len(),
            percentile(&diffs, 0.5),
            percentile(&abs, 0.9),
            percentile(&abs, 1.0)
        );
    }
    let _ = writeln!(s);

    let _ = writeln!(s, "## Problemas\n");
    let mut any = false;
    for r in rows {
        let p = problems(r);
        if p.is_empty() {
            continue;
        }
        any = true;
        let _ = writeln!(
            s,
            "- `{}` ({}): {}{}",
            short(&r.file),
            r.mode,
            p.join("; "),
            if r.error.is_empty() {
                String::new()
            } else {
                format!(" - {}", r.error.chars().take(200).collect::<String>())
            }
        );
    }
    if !any {
        let _ = writeln!(s, "Ninguno.");
    }
    let _ = writeln!(s);

    let mut top = |title: &str, key: fn(&Report) -> f64, unit: &str| {
        let mut v: Vec<&Report> = rows.iter().collect();
        v.sort_by(|a, b| key(b).total_cmp(&key(a)));
        let _ = writeln!(s, "## {title}\n");
        let _ = writeln!(s, "| archivo | modo | valor |\n|---|---|---|");
        for r in v.iter().take(8) {
            let _ = writeln!(
                s,
                "| {} | {} | {:.2} {unit} |",
                short(&r.file),
                r.mode,
                key(r)
            );
        }
        let _ = writeln!(s);
    };
    top("Más lentos al importar", |r| r.import_s, "s");
    top("Más lentos al calcular", |r| r.compute_s, "s");
    top("Más lentos al generar G-code", |r| r.gcode_s, "s");
    top("Más lentos al exportar .rd", |r| r.rd_s, "s");
    top("Más memoria (pico)", |r| r.peak_rss_mb, "MB");
    let _ = mb(0);
    s
}
