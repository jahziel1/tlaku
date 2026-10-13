use crate::job::LayerReport;
use laser_core::toolpath::{Move, Toolpath};
use laser_doc::{JobPreview, PreviewLayer};

fn summary(l: &LayerReport) -> String {
    let count = match (l.unit, l.count) {
        ("trayectorias", 1) => "1 trayectoria".to_string(),
        ("trayectorias", n) => format!("{n} trayectorias"),
        (_, 1) => "1 línea".to_string(),
        (_, n) => format!("{n} líneas"),
    };
    let mut s = format!(
        "{count}, {} mm/min, {}, {} pasada(s)",
        l.speed_mm_min, l.power, l.passes
    );
    if let Some(mm) = l.overscan_mm {
        s.push_str(&format!(", overscan {mm:.2} mm"));
    }
    s
}

fn layer(l: &LayerReport) -> PreviewLayer {
    let explanations = l.plan.as_ref().map_or_else(Vec::new, |plan| {
        plan.steps
            .iter()
            .map(|s| {
                format!(
                    "figura #{} ({}): {}",
                    s.item,
                    if s.path.closed { "cerrada" } else { "abierta" },
                    s.reason
                )
            })
            .collect()
    });
    PreviewLayer {
        color: l.color,
        mode: l.mode.to_string(),
        seconds: l.seconds,
        summary: summary(l),
        travel_saved_pct: l.plan.as_ref().map(|p| p.saved_pct()),
        explanations,
    }
}

pub(crate) fn preview(
    toolpath: &Toolpath,
    layers: &[LayerReport],
    total_s: f64,
    move_times: Vec<f64>,
    order_note: Option<String>,
    warnings: Vec<String>,
) -> JobPreview {
    let count: usize = toolpath.blocks.iter().map(|b| b.moves.len()).sum();
    let mut moves = Vec::with_capacity(count * 4);
    for mv in toolpath.blocks.iter().flat_map(|b| &b.moves) {
        match *mv {
            Move::Travel { to } => moves.extend([0.0, to.x as f32, to.y as f32, 0.0]),
            Move::Cut { to, power_pct } => {
                moves.extend([1.0, to.x as f32, to.y as f32, power_pct as f32]);
            }
        }
    }
    let bounds = layers
        .iter()
        .filter_map(|l| l.bounds)
        .reduce(laser_doc::union)
        .map_or([[0.0; 2]; 2], |b| [[b.min.x, b.min.y], [b.max.x, b.max.y]]);
    JobPreview {
        layers: layers.iter().map(layer).collect(),
        moves,
        move_times,
        total_s,
        bounds,
        order_note,
        warnings,
    }
}
