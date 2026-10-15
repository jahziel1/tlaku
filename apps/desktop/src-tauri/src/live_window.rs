//! Ventana aparte de la vista en vivo del trabajo (para ponerla en otro monitor).
//!
//! Es una ventana de SOLO LECTURA: no tiene documento ni máquina. La ventana que la abre (la
//! «dueña») le manda por `live_window_send` la ruta del trabajo y el avance, y la ventana aparte
//! solo escucha esos eventos (`live://route`, `live://state`). Su capacidad
//! (`capabilities/live.json`) no deja llamar a ningún comando de la app salvo `live_window_hello`
//! (pedir que le reenvíen el estado al abrirse) y `live_window_close` (cerrarse); ni un
//! comando de máquina. Solo hay una ventana `live` a la vez.

use std::sync::Mutex;

use tauri::{
    AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};

/// Etiqueta de la ventana; debe coincidir con la capacidad `live` (`capabilities/live.json`).
pub const LABEL: &str = "live";

/// Tope de un mensaje (la ruta de un trabajo grande en base64).
pub const MAX_MESSAGE_BYTES: usize = 48 * 1024 * 1024;

/// Qué ventana abrió la vista en vivo: a ella se le avisa que la ventana aparte está lista o se cerró.
#[derive(Default)]
pub struct LiveWindowState {
    owner: Mutex<Option<String>>,
}

impl LiveWindowState {
    fn set_owner(&self, label: &str) {
        *self.owner.lock().unwrap_or_else(|e| e.into_inner()) = Some(label.to_string());
    }
    fn owner(&self) -> Option<String> {
        self.owner.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// Canal de un mensaje a la ventana aparte (`live://<canal>`); `None` si el canal no existe.
pub fn event_name(kind: &str) -> Option<&'static str> {
    match kind {
        "route" => Some("live://route"),
        "state" => Some("live://state"),
        _ => None,
    }
}

/// Abre la ventana aparte (o la trae al frente si ya está abierta). La ventana que llama pasa a ser la dueña.
#[tauri::command]
pub async fn live_window_open(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, LiveWindowState>,
) -> Result<(), String> {
    state.set_owner(window.label());
    if let Some(existing) = app.get_webview_window(LABEL) {
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(());
    }
    let created = WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::App("index.html".into()))
        .title(tlaku_i18n::tr!("desktop.live_window.title"))
        .inner_size(760.0, 560.0)
        .min_inner_size(320.0, 240.0)
        .build()
        .map_err(|e| tlaku_i18n::tr!("desktop.window.open_failed", err = e))?;
    // Al cerrarla (por la X del sistema o por `live_window_close`) la dueña recupera la ventana flotante.
    let handle = app.clone();
    created.on_window_event(move |event| {
        if let WindowEvent::Destroyed = event
            && let Some(owner) = handle.state::<LiveWindowState>().owner()
        {
            let _ = handle.emit_to(owner, "live://closed", ());
        }
    });
    Ok(())
}

/// Cierra la ventana aparte si está abierta.
#[tauri::command]
pub async fn live_window_close(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(LABEL) {
        w.destroy().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Manda `data` (texto) a la ventana aparte como evento `live://<kind>` (`route` o `state`).
/// Devuelve si la ventana existe (si no, no se manda nada).
#[tauri::command]
pub async fn live_window_send(app: AppHandle, kind: String, data: String) -> Result<bool, String> {
    let Some(event) = event_name(&kind) else {
        return Err(tlaku_i18n::tr!("desktop.live_window.bad_kind", kind = kind));
    };
    if data.len() > MAX_MESSAGE_BYTES {
        return Err(tlaku_i18n::tr!("desktop.live_window.too_big"));
    }
    if app.get_webview_window(LABEL).is_none() {
        return Ok(false);
    }
    app.emit_to(LABEL, event, data).map_err(|e| e.to_string())?;
    Ok(true)
}

/// La ventana aparte ya escucha y pide que la dueña le reenvíe la ruta y el estado (`live://ready`).
/// Solo la ventana `live` puede llamarlo.
#[tauri::command]
pub async fn live_window_hello(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, LiveWindowState>,
) -> Result<(), String> {
    if window.label() != LABEL {
        return Err(tlaku_i18n::tr!("desktop.live_window.only_live"));
    }
    if let Some(owner) = state.owner() {
        app.emit_to(owner, "live://ready", ())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn read(name: &str) -> Value {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("capabilities")
            .join(name);
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn strings(v: &Value) -> Vec<String> {
        v.as_array()
            .unwrap()
            .iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect()
    }

    #[test]
    fn solo_hay_dos_canales() {
        assert_eq!(event_name("route"), Some("live://route"));
        assert_eq!(event_name("state"), Some("live://state"));
        assert_eq!(event_name("machine_command"), None);
    }

    #[test]
    fn la_capacidad_live_es_solo_de_la_ventana_live_y_no_deja_comandos_de_maquina() {
        let live = read("live.json");
        assert_eq!(strings(&live["windows"]), vec![LABEL.to_string()]);
        let perms: Vec<String> = live["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                p.as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| p["identifier"].as_str().unwrap().to_string())
            })
            .collect();
        // Lista cerrada: escuchar eventos y los dos comandos propios de la ventana aparte.
        let allowed = [
            "core:event:allow-listen",
            "core:event:allow-unlisten",
            "allow-live-window-hello",
            "allow-live-window-close",
        ];
        for p in &perms {
            assert!(
                allowed.contains(&p.as_str()),
                "permiso no previsto en la ventana aparte: {p}"
            );
        }
        for p in allowed {
            assert!(perms.iter().any(|x| x == p), "falta el permiso {p}");
        }
        // Nada de máquina, archivos ni el conjunto de todos los comandos.
        for p in &perms {
            for bad in [
                "main-window",
                "machine",
                "job",
                "jog",
                "connect",
                "fire",
                "dialog",
                "fs:",
                "shell",
                "opener",
                "updater",
            ] {
                assert!(!p.contains(bad), "la ventana aparte no debe tener «{p}»");
            }
        }
    }

    #[test]
    fn las_ventanas_principales_no_cubren_la_ventana_live_y_viceversa() {
        let main = strings(&read("default.json")["windows"]);
        assert!(
            !main
                .iter()
                .any(|w| w == LABEL || w == "*" || LABEL.starts_with(w.trim_end_matches('*')))
        );
        let live = strings(&read("live.json")["windows"]);
        assert!(
            !live
                .iter()
                .any(|w| w == "main" || w.starts_with("doc-") || w.contains('*'))
        );
        // La principal tiene el conjunto de todos los comandos de la app; la `live` no.
        let main_perms = read("default.json")["permissions"].to_string();
        assert!(main_perms.contains("\"main-window\""));
    }

    #[test]
    fn los_comandos_de_la_ventana_aparte_estan_registrados() {
        let lib = include_str!("lib.rs");
        for c in [
            "live_window_open",
            "live_window_close",
            "live_window_send",
            "live_window_hello",
        ] {
            assert!(
                lib.contains(&format!("live_window::{c}")),
                "{c} no está en generate_handler!"
            );
        }
    }
}
