//! Opciones del planificador y avisos de optimización.

use tlaku_core::Rgb;
use tlaku_doc::{Document, Optimization};
use tlaku_i18n::tr;
use tlaku_plan::{PlanOptions, Winding};

/// Opciones del planificador que salen de los ajustes de optimización del trabajo.
/// `hide_backlash` fija el sentido de giro de las cerradas y deja las abiertas como vienen.
pub(super) fn plan_options(o: &Optimization) -> PlanOptions {
    PlanOptions {
        order_by_layer: o.by_layer,
        order_by_group: o.by_group,
        order_by_priority: o.by_priority,
        inner_first: o.inner_first,
        reduce_travel: o.reduce_travel,
        choose_start_point: o.best_start,
        choose_direction: o.choose_direction && !o.hide_backlash,
        closed_winding: o.hide_backlash.then_some(Winding::Ccw),
        ..PlanOptions::default()
    }
}

/// Frase para la vista previa con las opciones de optimización activas (se da solo cuando no
/// son las de por defecto). `merged` lista las capas que `by_layer = false` juntó.
pub(super) fn optimization_note(
    o: &Optimization,
    plan: &PlanOptions,
    merged: &[Vec<Rgb>],
) -> String {
    let mut parts = tlaku_plan::describe(plan);
    if !o.by_layer {
        parts.push(if merged.is_empty() {
            tr!("pipeline.build_plan.no_layer_order_nothing")
        } else {
            let list: Vec<String> = merged
                .iter()
                .map(|colors| {
                    colors
                        .iter()
                        .map(Rgb::to_string)
                        .collect::<Vec<_>>()
                        .join(" + ")
                })
                .collect();
            tr!(
                "pipeline.build_plan.no_layer_order_merged",
                list = list.join("; ")
            )
        });
    }
    if o.remove_overlaps {
        parts.push(tr!(
            "pipeline.build_plan.overlaps_removed",
            tolerance = o.overlap_tolerance
        ));
    }
    if o.hide_backlash {
        parts.push(tr!("pipeline.build_plan.hide_backlash"));
    }
    tr!("pipeline.build_plan.optimization", parts = parts.join("; "))
}

/// Mensaje cuando el trabajo no produce nada. Dice por qué si el documento lo deja claro: vacío,
/// objetos ocultos o capas sin salida (en `.lbrn2`, una capa con `doOutput 0`).
pub(super) fn nothing_to_produce(doc: &Document, warnings: &[String]) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.extend(empty_reason(doc));
    parts.extend(warnings.iter().cloned());
    let reason = if parts.is_empty() {
        tr!("pipeline.build_plan.empty_or_no_output")
    } else {
        parts.join("; ")
    };
    tlaku_i18n::coded(
        "nothing_to_send",
        tr!("pipeline.build_plan.nothing_to_produce", reason = reason),
    )
}

fn empty_reason(doc: &Document) -> Option<String> {
    if doc.objects.is_empty() {
        return Some(tr!("pipeline.build_plan.empty_design"));
    }
    let visible: Vec<_> = doc.objects.iter().filter(|o| !o.hidden).collect();
    if visible.is_empty() {
        return Some(tr!("pipeline.build_plan.all_hidden"));
    }
    let off = |color: Rgb| {
        doc.layers
            .iter()
            .find(|l| l.color == color)
            .is_some_and(|l| !l.output)
    };
    if !visible.iter().all(|o| off(o.layer)) {
        return None;
    }
    let mut names: Vec<String> = Vec::new();
    for o in &visible {
        let layer = doc.layers.iter().find(|l| l.color == o.layer)?;
        let name = if layer.name.trim().is_empty() {
            layer.color.to_string()
        } else {
            layer.name.clone()
        };
        if !names.contains(&name) {
            names.push(name);
        }
    }
    Some(if names.len() == 1 {
        tr!("pipeline.build_plan.layer_no_output", name = names[0])
    } else {
        tr!(
            "pipeline.build_plan.layers_no_output",
            names = names.join(", ")
        )
    })
}
