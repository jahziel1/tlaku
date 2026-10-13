//! Cáscara de la aplicación de escritorio. La interfaz (TypeScript) es dueña del documento;
//! el motor se conecta aquí en la oleada B siguiendo `docs/app-api.md`.

use serde::Serialize;

#[derive(Serialize)]
struct AppInfo {
    name: &'static str,
    version: &'static str,
}

/// Comando de prueba: confirma que la interfaz habla con Rust.
#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Datos de la app para la pantalla "Acerca de" (todavía sin usar desde la interfaz).
#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "LaserSoft",
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![app_version, app_info])
        .run(tauri::generate_context!())
        .expect("no se pudo iniciar la aplicación LaserSoft");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_coincide_con_el_paquete() {
        assert_eq!(app_version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn info_serializa_a_json() {
        let json = serde_json::to_value(app_info()).unwrap();
        assert_eq!(json["name"], "LaserSoft");
        assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
    }
}
