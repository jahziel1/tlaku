//! Explicación en palabras de las opciones activas del planificador.

use crate::{PlanOptions, Winding};

/// Frase por cada regla activa o desactivada que cambia el resultado, para mostrarla junto al
/// plan. Va de la regla de mayor precedencia a la de menor (ver la documentación del crate).
pub fn describe(options: &PlanOptions) -> Vec<String> {
    let mut out = Vec::new();
    let levels: Vec<&str> = [
        (options.order_by_layer, "capa"),
        (options.order_by_group, "grupo"),
        (options.order_by_priority, "prioridad"),
    ]
    .into_iter()
    .filter_map(|(on, name)| on.then_some(name))
    .collect();
    out.push(if levels.is_empty() {
        "sin niveles de orden: todas las figuras compiten por igual".to_string()
    } else {
        format!(
            "orden por {} (de mayor a menor precedencia)",
            levels.join(" > ")
        )
    });
    out.push(if options.inner_first {
        "interiores primero: lo que está dentro de otra figura se corta antes".to_string()
    } else {
        "interiores primero apagado: una figura puede cortarse antes que las que contiene"
            .to_string()
    });
    out.push(if options.reduce_travel {
        "recorrido corto: vecino más cercano con mejora local".to_string()
    } else {
        "recorrido corto apagado: se conserva el orden original".to_string()
    });
    if options.choose_start_point {
        out.push("las figuras cerradas empiezan por el punto más cómodo".to_string());
    } else {
        out.push("las figuras cerradas empiezan donde fueron dibujadas".to_string());
    }
    if options.choose_direction {
        out.push("las figuras abiertas pueden recorrerse al revés".to_string());
    } else {
        out.push("las figuras abiertas conservan su sentido".to_string());
    }
    if let Some(w) = options.closed_winding {
        out.push(format!(
            "las figuras cerradas giran siempre en sentido {}",
            match w {
                Winding::Ccw => "antihorario",
                Winding::Cw => "horario",
            }
        ));
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
