//! Salida en pantalla: resumen del trabajo y plan de corte explicado.

use std::fmt::Write;
use tlaku_i18n::tr;
use tlaku_pipeline::{ComputedJob, LayerReport};
use tlaku_plan::PlanStep;

/// `83 s` -> "1 min 23 s"; `3725 s` -> "1 h 02 min 05 s"; menos de 10 s lleva un decimal.
pub fn format_duration(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "?".into();
    }
    if seconds < 10.0 {
        return format!("{seconds:.1} s");
    }
    let total = seconds.round() as u64;
    let (h, m, s) = (total / 3600, total % 3600 / 60, total % 60);
    match (h, m) {
        (0, 0) => format!("{s} s"),
        (0, _) => format!("{m} min {s:02} s"),
        _ => format!("{h} h {m:02} min {s:02} s"),
    }
}

/// "1 trayectoria" / "3 trayectorias" (cada idioma tiene su par de claves).
fn paths_count(n: usize) -> String {
    if n == 1 {
        tr!("cli.report.paths_one", n = n)
    } else {
        tr!("cli.report.paths_other", n = n)
    }
}

fn lines_count(n: usize) -> String {
    if n == 1 {
        tr!("cli.report.lines_one", n = n)
    } else {
        tr!("cli.report.lines_other", n = n)
    }
}

fn shapes_count(n: usize) -> String {
    if n == 1 {
        tr!("cli.report.shapes_one", n = n)
    } else {
        tr!("cli.report.shapes_other", n = n)
    }
}

fn sweep_lines_count(n: usize) -> String {
    if n == 1 {
        tr!("cli.report.sweep_lines_one", n = n)
    } else {
        tr!("cli.report.sweep_lines_other", n = n)
    }
}

fn layer_line(n: usize, l: &LayerReport) -> String {
    let count = match (l.unit, l.count) {
        ("trayectorias", n) => paths_count(n),
        (_, n) => lines_count(n),
    };
    let mut s = format!(
        "  {}",
        tr!(
            "cli.report.layer_line",
            n = n,
            name = format!("{:<8}", l.name),
            mode = format!("{:<5}", l.mode),
            count = format!("{count:<15}"),
            speed = l.speed_mm_min,
            power = l.power,
            passes = l.passes
        )
    );
    if let Some(mm) = l.overscan_mm {
        let _ = write!(s, ", overscan {mm:.2} mm");
    }
    let _ = write!(s, " - {}", format_duration(l.seconds));
    if let Some(plan) = &l.plan {
        let _ = write!(
            s,
            "\n              {}",
            tr!(
                "cli.report.layer_travel",
                travel = format!("{:.1}", plan.travel_mm),
                baseline = format!("{:.1}", plan.baseline_travel_mm),
                saved = format!("{:.1}", plan.saved_mm()),
                pct = format!("{:.0}", plan.saved_pct())
            )
        );
    }
    s
}

pub fn summary(built: &ComputedJob) -> String {
    let mut out = tr!("cli.report.layers_header");
    out.push('\n');
    for (i, l) in built.layers.iter().enumerate() {
        out.push_str(&layer_line(i + 1, l));
        out.push('\n');
    }
    if let Some(note) = &built.preview.order_note {
        let _ = writeln!(out, "{}", capitalize(note));
    }
    let _ = writeln!(
        out,
        "{}",
        tr!(
            "cli.report.total_time",
            time = format_duration(built.preview.total_s)
        )
    );
    if let Some(b) = built.bounds() {
        let _ = writeln!(
            out,
            "{}",
            tr!(
                "cli.report.occupied_area",
                x0 = format!("{:.2}", b.min.x),
                x1 = format!("{:.2}", b.max.x),
                y0 = format!("{:.2}", b.min.y),
                y1 = format!("{:.2}", b.max.y)
            )
        );
    }
    if !built.preview.warnings.is_empty() {
        let _ = writeln!(out, "{}", tr!("cli.report.warnings_header"));
        for w in &built.preview.warnings {
            let _ = writeln!(out, "  - {w}");
        }
    }
    out
}

/// Primera letra en mayúscula ("orden automático: ..." -> "Orden automático: ...").
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// Orden de ejecución en una línea y su explicación; vacío si solo hay una capa.
pub fn order_line(built: &ComputedJob) -> String {
    let Some(note) = &built.preview.order_note else {
        return String::new();
    };
    let names: Vec<String> = built
        .layers
        .iter()
        .enumerate()
        .map(|(i, l)| format!("{}. {} ({})", i + 1, l.name, l.mode))
        .collect();
    format!(
        "{}\n",
        tr!(
            "cli.report.layers_in_order",
            names = names.join(", "),
            note = capitalize(note)
        )
    )
}

fn step_line(n: usize, s: &PlanStep) -> String {
    let p = s.path.start();
    let (x, y) = (format!("{:.2}", p.x), format!("{:.2}", p.y));
    let text = if s.path.closed {
        tr!(
            "cli.report.step_closed",
            item = s.item,
            x = x,
            y = y,
            reason = s.reason
        )
    } else {
        tr!(
            "cli.report.step_open",
            item = s.item,
            x = x,
            y = y,
            reason = s.reason
        )
    };
    format!("  {n:>3}. {text}")
}

/// Orden de corte con la razón de cada paso. Las figuras se numeran desde 0 dentro de su capa,
/// en el orden del archivo (después de la compensación de kerf, si la hay).
pub fn plan_text(built: &ComputedJob) -> String {
    let mut out = String::new();
    if let Some(note) = &built.preview.order_note {
        let _ = writeln!(out, "{}", capitalize(note));
    }
    for (i, l) in built.layers.iter().enumerate() {
        let count = match l.unit {
            "trayectorias" => shapes_count(l.count),
            _ => sweep_lines_count(l.count),
        };
        let _ = writeln!(
            out,
            "{}",
            tr!(
                "cli.report.plan_layer",
                n = i + 1,
                total = built.layers.len(),
                name = l.name,
                mode = l.mode,
                passes = l.passes,
                count = count
            )
        );
        match &l.plan {
            Some(plan) => {
                for (i, s) in plan.steps.iter().enumerate() {
                    let _ = writeln!(out, "{}", step_line(i + 1, s));
                }
                let _ = writeln!(
                    out,
                    "  {}",
                    tr!(
                        "cli.report.plan_travel",
                        travel = format!("{:.1}", plan.travel_mm),
                        baseline = format!("{:.1}", plan.baseline_travel_mm),
                        saved = format!("{:.1}", plan.saved_mm()),
                        pct = format!("{:.0}", plan.saved_pct())
                    )
                );
            }
            None => {
                let _ = writeln!(out, "  {}", tr!("cli.report.plan_sweep"));
            }
        }
    }
    if !built.preview.warnings.is_empty() {
        let _ = writeln!(out, "{}", tr!("cli.report.warnings_header"));
        for w in &built.preview.warnings {
            let _ = writeln!(out, "  - {w}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(format_duration(0.0), "0.0 s");
        assert_eq!(format_duration(9.96), "10.0 s");
        assert_eq!(format_duration(42.4), "42 s");
        assert_eq!(format_duration(83.0), "1 min 23 s");
        assert_eq!(format_duration(3725.0), "1 h 02 min 05 s");
        assert_eq!(format_duration(f64::NAN), "?");
    }
}
