//! Aplicación de escritorio de Tlaku (Tauri 2). La interfaz (TypeScript) es dueña del
//! documento; aquí viven el motor y la conexión con la máquina siguiendo `docs/app-api.md`.
//!
//! - `engine`: cálculo de trabajos, archivos y perfiles de dispositivo (sin Tauri).
//! - `machine`: conexión con la máquina (GRBL o Ruida, ver `machine::active`) en un hilo dedicado,
//!   con eventos `machine://*` (sin Tauri).
//! - `commands`: manejadores de `invoke` que unen lo anterior con la ventana.
//! - `edit`: texto, geometría, vectorizado, cámara (fase 3) y materiales (fase 4), un módulo
//!   por área.

mod automation;
mod autosave;
mod commands;
mod edit;
mod engine;
mod machine;
mod netlink;
mod prefs;
mod probe;
mod settings_backup;
mod templates;
mod window;

use commands::AppState;
use tauri::{Manager, RunEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .manage(automation::AutomationState::default())
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
            probe::probe_port,
            automation::automation_configure,
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
            commands::frame_hull,
            commands::jog_to,
            commands::jog_axis,
            commands::jog_axis_to,
            commands::fire_test,
            commands::set_override,
            commands::simulate_fault,
            commands::ruida_info,
            commands::ruida_files,
            commands::lhy_preview,
            commands::lhy_home,
            commands::lhy_state,
            edit::text::list_fonts,
            edit::text::text_outline,
            edit::text::import_shx_fonts,
            edit::variables::read_csv,
            edit::codes::generate_code,
            edit::geometry::geometry_op,
            edit::nest::nest_start,
            edit::nest::nest_poll,
            edit::nest::nest_cancel,
            edit::shape_ops::shape_op,
            edit::design::warp_objects,
            edit::design::hull_outline,
            edit::design::tangent_circles,
            edit::trace::trace_image,
            edit::trace::vectors_to_bitmap,
            edit::trace::save_processed_image,
            edit::art::art_import_svg,
            edit::art::art_list_user,
            edit::art::art_add_user,
            edit::art::art_remove_user,
            edit::camera::detect_board,
            edit::camera::calibrate_lens,
            edit::camera::camera_marks,
            edit::camera::detect_camera_marks,
            edit::camera::compose_bed_image,
            edit::camera::detect_circle_in_image,
            edit::camera::load_camera_setup,
            edit::camera::save_camera_setup,
            edit::camera::save_board_file,
            edit::materials::load_material_library,
            edit::materials::save_material_library,
            edit::materials::import_material_library,
            edit::materials::export_material_library,
            edit::materials::material_test,
            edit::devices::device_defaults,
            edit::devices::rotary_test,
            edit::files::import_file_with,
            edit::files::export_design_file,
            edit::files::print_design,
            autosave::autosave_write,
            autosave::autosave_list,
            autosave::autosave_read,
            autosave::autosave_discard,
            prefs::load_preferences,
            prefs::save_preferences,
            prefs::export_preferences,
            prefs::import_preferences,
            prefs::paths_exist,
            templates::list_templates,
            templates::save_template,
            templates::load_template,
            templates::delete_template,
            prefs::export_user_bundle,
            prefs::import_user_bundle,
            settings_backup::write_settings_backup,
            settings_backup::read_settings_backup,
            window::new_window,
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
