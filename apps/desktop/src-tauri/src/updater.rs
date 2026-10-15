//! Actualizaciones automáticas: registra el plugin del actualizador solo si la compilación lo
//! configuró (`plugins.updater` con `pubkey` y `endpoints` en `tauri.conf.json`; lo llena
//! `scripts/configure-updater.sh`, ver `docs/publicar.md`).
//!
//! El plugin exige `pubkey` al leer su configuración: sin ella falla al arrancar. Por eso no se
//! registra a ciegas. La interfaz decide con la misma regla (`features/help/updater.ts`) si ofrece
//! buscar actualizaciones o dice que esta compilación no las trae configuradas.
//!
//! Dentro de un sandbox de Flatpak el actualizador **nunca** se registra: las actualizaciones las
//! da Flathub y el programa no puede (ni debe) reemplazar sus propios archivos. La interfaz se
//! entera por `window.__TLAKU_SANDBOX__` (ver [`sandbox_init_script`]).

use std::path::Path;

use serde_json::Value;

/// Marcador que Flatpak crea dentro de todo sandbox.
const FLATPAK_INFO: &str = "/.flatpak-info";

/// ¿Corre dentro de un sandbox de Flatpak? `FLATPAK_ID` lo define Flatpak al lanzar la app y
/// `/.flatpak-info` existe en todo sandbox; basta con uno de los dos.
pub fn in_flatpak() -> bool {
    detect_flatpak(
        std::env::var("FLATPAK_ID").ok().as_deref(),
        Path::new(FLATPAK_INFO).exists(),
    )
}

fn detect_flatpak(flatpak_id: Option<&str>, info_file_exists: bool) -> bool {
    flatpak_id.is_some_and(|id| !id.trim().is_empty()) || info_file_exists
}

/// Script que se inyecta en cada ventana cuando hay sandbox, para que la interfaz oculte el
/// actualizador (`features/help/updater.ts`, `updatesManagedByStore`). Fuera de Flatpak no se
/// inyecta nada.
pub fn sandbox_init_script() -> Option<&'static str> {
    sandbox_script_for(in_flatpak())
}

fn sandbox_script_for(flatpak: bool) -> Option<&'static str> {
    flatpak.then_some("window.__TLAKU_SANDBOX__ = 'flatpak';")
}

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
        if !in_flatpak() && is_configured(app.config().plugins.0.get("updater")) {
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
    fn detecta_el_sandbox_de_flatpak() {
        assert!(detect_flatpak(Some("org.tlaku.app"), false));
        assert!(detect_flatpak(None, true));
        assert!(detect_flatpak(Some("org.tlaku.app"), true));
        assert!(!detect_flatpak(None, false));
        assert!(!detect_flatpak(Some(""), false));
        assert!(!detect_flatpak(Some("  "), false));
    }

    #[test]
    fn el_script_del_sandbox_solo_existe_en_flatpak() {
        assert_eq!(sandbox_script_for(false), None);
        let script = sandbox_script_for(true).expect("script");
        assert!(script.contains("__TLAKU_SANDBOX__"));
        assert!(script.contains("'flatpak'"));
    }

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
