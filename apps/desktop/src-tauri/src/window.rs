//! Ventanas adicionales: «Nueva ventana» abre otra ventana de la app con un documento propio.
//!
//! Cada ventana es una interfaz web aparte (con su documento, historial y selección
//! independientes); lo que comparten es el motor: la conexión con la máquina y el trabajo en
//! curso viven en `AppState`, y las preferencias son el mismo archivo. Los eventos de la máquina
//! llegan a todas las ventanas. Las ventanas nuevas se llaman `doc-N` y la capacidad por defecto
//! (`capabilities/default.json`) las cubre con el patrón `doc-*`.

use std::sync::atomic::{AtomicU32, Ordering};
use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder};

/// Prefijo de la etiqueta de las ventanas adicionales; debe coincidir con la capacidad `doc-*`.
pub const LABEL_PREFIX: &str = "doc-";

static NEXT: AtomicU32 = AtomicU32::new(1);

/// Etiqueta de la n-ésima ventana adicional.
pub fn label_for(n: u32) -> String {
    format!("{LABEL_PREFIX}{n}")
}

/// Abre una ventana nueva y devuelve su etiqueta. Es `async` para que Tauri la cree fuera del
/// hilo principal (en Windows, crear una ventana desde un comando síncrono puede bloquearse).
#[tauri::command]
pub async fn new_window(app: AppHandle) -> Result<String, String> {
    let label = label_for(NEXT.fetch_add(1, Ordering::Relaxed));
    WebviewWindowBuilder::new(&app, &label, WebviewUrl::App("index.html".into()))
        .title("Tlaku")
        .inner_size(1440.0, 900.0)
        .min_inner_size(1100.0, 680.0)
        .build()
        .map_err(|e| format!("No se pudo abrir la ventana nueva: {e}"))?;
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
}
