//! Opciones del planificador y avisos de optimización.

use tlaku_core::Rgb;
use tlaku_doc::{Document, Optimization};
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

/// Mensaje cuando el trabajo no produce nada. Dice por qué si el documento lo deja claro: vacío,
/// objetos ocultos o capas sin salida (en `.lbrn2`, una capa con `doOutput 0`).
pub(super) fn nothing_to_produce(doc: &Document, warnings: &[String]) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.extend(empty_reason(doc));
    parts.extend(warnings.iter().cloned());
    format!(
        "no hay nada que producir: {}",
        if parts.is_empty() {
            "el diseño está vacío (o ningún objeto tiene salida)".to_string()
        } else {
            parts.join("; ")
        }
    )
}

fn empty_reason(doc: &Document) -> Option<String> {
    if doc.objects.is_empty() {
        return Some("el diseño está vacío".into());
    }
    let visible: Vec<_> = doc.objects.iter().filter(|o| !o.hidden).collect();
    if visible.is_empty() {
        return Some("todos los objetos están ocultos".into());
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
        format!(
            "la capa \"{}\" está sin salida (la salida de la capa está desactivada); \
             actívala para que se corte o grabe",
            names[0]
        )
    } else {
        format!(
            "las capas con objetos ({}) están sin salida (la salida está desactivada); \
             activa la salida de al menos una para que se corte o grabe",
            names.join(", ")
        )
    })
}
