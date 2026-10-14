//! Explicación en palabras de las opciones activas del planificador.

use crate::{PlanOptions, Winding};
use tlaku_i18n::tr;

/// Frase por cada regla activa o desactivada que cambia el resultado, para mostrarla junto al
/// plan. Va de la regla de mayor precedencia a la de menor (ver la documentación del crate).
pub fn describe(options: &PlanOptions) -> Vec<String> {
    let mut out = Vec::new();
    let levels: Vec<String> = [
        (options.order_by_layer, tr!("plan.describe.level_layer")),
        (options.order_by_group, tr!("plan.describe.level_group")),
        (
            options.order_by_priority,
            tr!("plan.describe.level_priority"),
        ),
    ]
    .into_iter()
    .filter_map(|(on, name)| on.then_some(name))
    .collect();
    out.push(if levels.is_empty() {
        tr!("plan.describe.no_levels")
    } else {
        tr!("plan.describe.levels", levels = levels.join(" > "))
    });
    out.push(if options.inner_first {
        tr!("plan.describe.inner_first")
    } else {
        tr!("plan.describe.inner_first_off")
    });
    out.push(if options.reduce_travel {
        tr!("plan.describe.reduce_travel")
    } else {
        tr!("plan.describe.reduce_travel_off")
    });
    if options.choose_start_point {
        out.push(tr!("plan.describe.start_chosen"));
    } else {
        out.push(tr!("plan.describe.start_drawn"));
    }
    if options.choose_direction {
        out.push(tr!("plan.describe.direction_free"));
    } else {
        out.push(tr!("plan.describe.direction_kept"));
    }
    if let Some(w) = options.closed_winding {
        out.push(match w {
            Winding::Ccw => tr!("plan.describe.winding_ccw"),
            Winding::Cw => tr!("plan.describe.winding_cw"),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_options_are_described_in_precedence_order() {
        let text = describe(&PlanOptions::default());
        assert!(text[0].contains("capa > prioridad"), "{text:?}");
        assert!(text.iter().any(|t| t.contains("interiores primero:")));
        assert!(!text.iter().any(|t| t.contains("siempre en sentido")));
    }

    #[test]
    fn off_options_and_winding_are_explained() {
        let text = describe(&PlanOptions {
            order_by_layer: false,
            order_by_priority: false,
            inner_first: false,
            reduce_travel: false,
            choose_direction: false,
            closed_winding: Some(Winding::Cw),
            ..PlanOptions::default()
        });
        let all = text.join(" | ");
        assert!(all.contains("sin niveles"), "{all}");
        assert!(all.contains("interiores primero apagado"), "{all}");
        assert!(all.contains("orden original"), "{all}");
        assert!(all.contains("conservan su sentido"), "{all}");
        assert!(all.contains("horario"), "{all}");
    }
}
