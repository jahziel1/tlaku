//! Ventanas adicionales: «Nueva ventana» abre otra ventana de la app con un documento propio.
//!
//! Cada ventana es una interfaz web aparte (con su documento, historial y selección
//! independientes); lo que comparten es el motor: la conexión con la máquina y el trabajo en
//! curso viven en `AppState`, y las preferencias son el mismo archivo. Los eventos de la máquina
//! llegan a todas las ventanas. Las ventanas nuevas se llaman `doc-N` y la capacidad por defecto
//! (`capabilities/default.json`) las cubre con el patrón `doc-*`.
//!
//! Dentro de cada ventana hay pestañas de documentos (`features/tabs`). «Mover a ventana nueva» pasa
//! una pestaña a otra ventana con el parámetro opcional `handoff` (ver `handoff_url`).

use std::sync::atomic::{AtomicU32, Ordering};
use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder};

/// Prefijo de la etiqueta de las ventanas adicionales; debe coincidir con la capacidad `doc-*`.
pub const LABEL_PREFIX: &str = "doc-";

static NEXT: AtomicU32 = AtomicU32::new(1);

/// Etiqueta de la n-ésima ventana adicional.
pub fn label_for(n: u32) -> String {
    format!("{LABEL_PREFIX}{n}")
}

/// Mayor longitud aceptada del parámetro `handoff` (un JSON corto codificado para una dirección).
const MAX_HANDOFF: usize = 2048;

/// Dirección de la interfaz para una ventana nueva. `handoff` llega de la interfaz ya codificado
/// (`encodeURIComponent`); solo se aceptan caracteres de una dirección sin sorpresas, para que nunca
/// pueda colar otro parámetro, un fragmento o una ruta distinta.
pub fn handoff_url(handoff: Option<&str>) -> Result<String, String> {
    let Some(h) = handoff.filter(|h| !h.is_empty()) else {
        return Ok("index.html".to_string());
    };
    let safe = |c: char| c.is_ascii_alphanumeric() || matches!(c, '%' | '-' | '_' | '.' | '~');
    if h.len() > MAX_HANDOFF || !h.chars().all(safe) {
        return Err(tlaku_i18n::tr!("desktop.window.bad_handoff"));
    }
    Ok(format!("index.html?handoff={h}"))
}

/// Abre una ventana nueva y devuelve su etiqueta. Es `async` para que Tauri la cree fuera del
/// hilo principal (en Windows, crear una ventana desde un comando síncrono puede bloquearse).
/// `handoff` (opcional): la pestaña que se mueve a esta ventana.
#[tauri::command]
pub async fn new_window(app: AppHandle, handoff: Option<String>) -> Result<String, String> {
    let url = handoff_url(handoff.as_deref())?;
    let label = label_for(NEXT.fetch_add(1, Ordering::Relaxed));
    WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(url.into()))
        .title("Tlaku")
        .inner_size(1440.0, 900.0)
        .min_inner_size(1100.0, 680.0)
        .build()
        .map_err(|e| tlaku_i18n::tr!("desktop.window.open_failed", err = e))?;
    Ok(label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn las_etiquetas_encajan_con_la_capacidad() {
        let caps = include_str!("../capabilities/default.json");
        let value: serde_json::Value = serde_json::from_str(caps).unwrap();
        let windows: Vec<&str> = value["windows"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(windows.contains(&"main"));
        // La capacidad cubre «doc-*»: toda etiqueta que generamos empieza por ese prefijo.
        assert!(windows.contains(&format!("{LABEL_PREFIX}*").as_str()));
        assert_eq!(label_for(3), "doc-3");
        assert!(label_for(1).starts_with(LABEL_PREFIX));
    }

    #[test]
    fn la_direccion_de_la_pestana_movida_solo_admite_texto_seguro() {
        assert_eq!(handoff_url(None).unwrap(), "index.html");
        assert_eq!(handoff_url(Some("")).unwrap(), "index.html");
        let ok = "%7B%22id%22%3A%22handoff-abc_1%22%7D";
        assert_eq!(
            handoff_url(Some(ok)).unwrap(),
            format!("index.html?handoff={ok}")
        );
        // Nada que abra otro parámetro, un fragmento, otra ruta o una URL distinta.
        for bad in [
            "a&b=1", "a#x", "../x", "a b", "http://x", "a/b", "a\\b", "a?b",
        ] {
            assert!(handoff_url(Some(bad)).is_err(), "{bad}");
        }
        assert!(handoff_url(Some(&"a".repeat(MAX_HANDOFF + 1))).is_err());
        assert!(handoff_url(Some(&"a".repeat(MAX_HANDOFF))).is_ok());
    }
}
