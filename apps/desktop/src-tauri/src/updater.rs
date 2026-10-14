//! Actualizaciones automáticas: registra el plugin del actualizador solo si la compilación lo
//! configuró (`plugins.updater` con `pubkey` y `endpoints` en `tauri.conf.json`; lo llena
//! `scripts/configure-updater.sh`, ver `docs/publicar.md`).
//!
//! El plugin exige `pubkey` al leer su configuración: sin ella falla al arrancar. Por eso no se
//! registra a ciegas. La interfaz decide con la misma regla (`features/help/updater.ts`) si ofrece
//! buscar actualizaciones o dice que esta compilación no las trae configuradas.

use serde_json::Value;

/// ¿La configuración del actualizador trae llave pública y al menos un endpoint?
pub fn is_configured(plugins: Option<&Value>) -> bool {
    let Some(cfg) = plugins else { return false };
    let pubkey = cfg
        .get("pubkey")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let endpoints = cfg
        .get("endpoints")
        .and_then(Value::as_array)
        .is_some_and(|list| {
            list.iter()
                .any(|e| e.as_str().is_some_and(|s| !s.trim().is_empty()))
        });
    !pubkey.is_empty() && endpoints
}

/// `setup` de la aplicación: añade el actualizador si está configurado.
pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(desktop)]
    {
        if is_configured(app.config().plugins.0.get("updater")) {
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
        }
    }
    #[cfg(not(desktop))]
    let _ = app;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sin_configuracion_no_esta_configurado() {
        assert!(!is_configured(None));
        assert!(!is_configured(Some(&json!({}))));
        assert!(!is_configured(Some(
            &json!({ "pubkey": "", "endpoints": [] })
        )));
    }

    #[test]
    fn necesita_llave_y_endpoint() {
        let url = "https://example.org/latest.json";
        assert!(!is_configured(Some(&json!({ "pubkey": "abc" }))));
        assert!(!is_configured(Some(
            &json!({ "pubkey": " ", "endpoints": [url] })
        )));
        assert!(!is_configured(Some(
            &json!({ "pubkey": "abc", "endpoints": [""] })
        )));
        assert!(is_configured(Some(
            &json!({ "pubkey": "abc", "endpoints": [url] })
        )));
    }
}
