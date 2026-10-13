//! Aplicación de escritorio de Tlaku (Tauri 2). La interfaz (TypeScript) es dueña del
//! documento; aquí viven el motor y la conexión con la máquina siguiendo `docs/app-api.md`.
//!
//! - `engine`: cálculo de trabajos, archivos y perfiles de dispositivo (sin Tauri).
//! - `machine`: conexión GRBL en un hilo dedicado, con eventos `machine://*` (sin Tauri).
//! - `commands`: manejadores de `invoke` que unen lo anterior con la ventana.

mod commands;
mod engine;
mod machine;

use commands::AppState;
use tauri::{Manager, RunEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::app_version,
            commands::import_file,
            commands::save_project,
            commands::load_project,
            commands::compute_job,
            commands::export_gcode,
            commands::list_devices,
            commands::save_device,
            commands::delete_device,
            commands::list_ports,
            commands::connect,
            commands::disconnect,
            commands::machine_command,
            commands::machine_realtime,
            commands::jog,
            commands::start_job,
            commands::pause_job,
            commands::resume_job,
            commands::cancel_job,
            commands::frame_job,
        ])
        .build(tauri::generate_context!())
        .expect("no se pudo iniciar la aplicación Tlaku");
    app.run(|app, event| {
        // Al salir se cancela un trabajo en curso (pausa y reinicio) antes de soltar el puerto.
        if let RunEvent::Exit = event {
            app.state::<AppState>().shutdown();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::commands::app_version;

    #[test]
    fn version_coincide_con_el_paquete() {
        assert_eq!(app_version(), env!("CARGO_PKG_VERSION"));
    }
}
