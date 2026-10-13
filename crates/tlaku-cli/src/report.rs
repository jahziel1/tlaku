//! Salida en pantalla: resumen del trabajo y plan de corte explicado.

use std::fmt::Write;
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

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn layer_line(n: usize, l: &LayerReport) -> String {
    let count = match (l.unit, l.count) {
        ("trayectorias", n) => plural(n, "trayectoria", "trayectorias"),
        (_, n) => plural(n, "línea", "líneas"),
    };
    let mut s = format!(
        "  {n}. {:<8} {:<5} {:<15} {} mm/min, {}, {} pasada(s)",
        l.name, l.mode, count, l.speed_mm_min, l.power, l.passes
    );
    if let Some(mm) = l.overscan_mm {
        let _ = write!(s, ", overscan {mm:.2} mm");
    }
    let _ = write!(s, " - {}", format_duration(l.seconds));
    if let Some(plan) = &l.plan {
        let _ = write!(
            s,
            "\n              recorrido en vacío: {:.1} mm (orden original {:.1} mm, ahorro {:.1} mm = {:.0} %)",
            plan.travel_mm,
            plan.baseline_travel_mm,
            plan.saved_mm(),
            plan.saved_pct()
        );
    }
    s
}

pub fn summary(built: &ComputedJob) -> String {
    let mut out = String::from("Capas (en el orden en que se ejecutan):\n");
    for (i, l) in built.layers.iter().enumerate() {
        out.push_str(&layer_line(i + 1, l));
        out.push('\n');
    }
    if let Some(note) = &built.preview.order_note {
        let _ = writeln!(out, "{}", capitalize(note));
    }
    let _ = writeln!(
        out,
        "Tiempo total estimado: {}",
        format_duration(built.preview.total_s)
    );
    if let Some(b) = built.bounds() {
        let _ = writeln!(
            out,
            "Área ocupada (con overscan): X {:.2} a {:.2} mm, Y {:.2} a {:.2} mm",
            b.min.x, b.max.x, b.min.y, b.max.y
        );
    }
    if !built.preview.warnings.is_empty() {
        out.push_str("Advertencias:\n");
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
        "Capas en orden: {}. {}\n",
        names.join(", "),
        capitalize(note)
    )
}

fn step_line(n: usize, s: &PlanStep) -> String {
    let p = s.path.start();
    format!(
        "  {:>3}. figura #{} ({}, entra por X{:.2} Y{:.2}): {}",
        n,
        s.item,
        if s.path.closed { "cerrada" } else { "abierta" },
        p.x,
        p.y,
        s.reason
    )
}

/// Orden de corte con la razón de cada paso. Las figuras se numeran desde 0 dentro de su capa,
/// en el orden del archivo (después de la compensación de kerf, si la hay).
pub fn plan_text(built: &ComputedJob) -> String {
    let mut out = String::new();
    if let Some(note) = &built.preview.order_note {
        let _ = writeln!(out, "{}", capitalize(note));
    }
    for (i, l) in built.layers.iter().enumerate() {
        let _ = writeln!(
            out,
            "Capa {} de {}: {} ({}, {} pasada(s), {})",
            i + 1,
            built.layers.len(),
            l.name,
            l.mode,
            l.passes,
            match l.unit {
                "trayectorias" => plural(l.count, "figura", "figuras"),
                _ => plural(l.count, "línea de barrido", "líneas de barrido"),
            }
        );
        match &l.plan {
            Some(plan) => {
                for (i, s) in plan.steps.iter().enumerate() {
                    let _ = writeln!(out, "{}", step_line(i + 1, s));
                }
                let _ = writeln!(
                    out,
                    "  Recorrido en vacío: {:.1} mm (orden original {:.1} mm, ahorro {:.1} mm = {:.0} %)",
                    plan.travel_mm,
                    plan.baseline_travel_mm,
                    plan.saved_mm(),
                    plan.saved_pct()
                );
            }
            None => {
                let _ = writeln!(
                    out,
                    "  Se barre línea por línea; no hay orden entre figuras que decidir."
                );
            }
        }
    }
    if !built.preview.warnings.is_empty() {
        out.push_str("Advertencias:\n");
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
