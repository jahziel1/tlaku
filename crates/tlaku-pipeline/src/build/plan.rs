//! Opciones del planificador y avisos de optimización.

use tlaku_core::Rgb;
use tlaku_doc::Optimization;
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
            "sin orden por capa: no hay capas de corte consecutivas con los mismos ajustes que \
             mezclar (las que tienen ajustes distintos no se mezclan)"
                .to_string()
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
            format!(
                "sin orden por capa: se mezclaron las capas con los mismos ajustes ({})",
                list.join("; ")
            )
        });
    }
    if o.remove_overlaps {
        parts.push(format!(
            "tramos repetidos quitados dentro de cada capa de corte (tolerancia {} mm)",
            o.overlap_tolerance
        ));
    }
    if o.hide_backlash {
        parts.push("hide_backlash: recorrido en un solo sentido, más lento".to_string());
    }
    format!("optimización: {}", parts.join("; "))
}

pub(super) fn nothing_to_produce(warnings: &[String]) -> String {
    format!(
        "no hay nada que producir: {}",
        if warnings.is_empty() {
            "el diseño está vacío (o ningún objeto tiene salida)".to_string()
        } else {
            warnings.join("; ")
        }
    )
}
