//! Generadores de piezas (cajas con dientes, bisagras flexibles, pieza de prueba de encaje).
//!
//! - `list_generators`: los generadores con el formulario de cada uno y sus valores de fábrica.
//! - `generate_part`: genera las piezas con unos parámetros JSON y las acomoda en filas de a lo
//!   más `max_width_mm` (el ancho de la cama). Los parámetros inválidos no son un error del
//!   comando: vuelven en `error` con el campo, para marcarlo mientras se edita.
//!
//! La geometría vive en `tlaku-parts`; aquí solo se expone. Insertar las piezas en el documento es
//! trabajo de la interfaz (objetos vectoriales normales en un grupo).

use serde_json::Value;
use tlaku_parts::{GenerateResult, GeneratorInfo};

/// Separación entre piezas, en mm, si la interfaz no pide otra.
const DEFAULT_GAP_MM: f64 = 5.0;

/// Generadores disponibles con su formulario.
#[tauri::command]
pub fn list_generators() -> Vec<GeneratorInfo> {
    tlaku_parts::list()
}

/// Núcleo de `generate_part` (sin Tauri).
pub fn generate(
    generator: &str,
    params: &Value,
    max_width_mm: Option<f64>,
    gap_mm: Option<f64>,
) -> Result<GenerateResult, String> {
    tlaku_parts::generate(
        generator,
        params,
        max_width_mm,
        gap_mm.unwrap_or(DEFAULT_GAP_MM),
    )
}

/// Genera las piezas de `generator` con `params`.
#[tauri::command(rename_all = "snake_case")]
pub fn generate_part(
    generator: String,
    params: Value,
    max_width_mm: Option<f64>,
    gap_mm: Option<f64>,
) -> Result<GenerateResult, String> {
    generate(&generator, &params, max_width_mm, gap_mm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tlaku_i18n::{Locale, with_locale};

    #[test]
    fn lists_every_generator() {
        let ids: Vec<String> = list_generators().into_iter().map(|g| g.id).collect();
        for id in ["box", "tray", "hinge", "fit_test"] {
            assert!(ids.iter().any(|i| i == id), "falta {id}");
        }
    }

    #[test]
    fn generates_a_box_that_fits_the_bed() {
        let r = generate(
            "box",
            &json!({"width_mm": 100, "depth_mm": 80, "height_mm": 60}),
            Some(300.0),
            None,
        )
        .unwrap();
        assert!(r.error.is_none());
        assert_eq!(r.parts.len(), 6);
        assert!(r.width <= 300.0 + 1e-9);
    }

    #[test]
    fn bad_params_come_back_as_data_in_the_active_language() {
        let p = json!({"thickness_mm": 0});
        let es = generate("box", &p, None, None).unwrap().error.unwrap();
        assert_eq!(es.field.as_deref(), Some("thickness_mm"));
        let en = with_locale(Locale::En, || {
            generate("box", &p, None, None).unwrap().error.unwrap()
        });
        assert_ne!(es.message, en.message);
        assert!(generate("nope", &p, None, None).is_err());
    }
}
