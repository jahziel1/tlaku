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
pub mod hwtest;
mod id_migration;
mod job_history;
mod job_resume;
mod live_window;
mod machine;
mod netlink;
mod prefs;
mod probe;
mod remote;
mod settings_backup;
pub mod smoke;
mod templates;
mod updater;
mod versions;
mod window;

use commands::AppState;
use tauri::{Manager, RunEvent};

/// Contexto de Tauri (`tauri.conf.json` y los recursos de la interfaz).
fn app_context() -> tauri::Context {
    tauri::generate_context!()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = build_app(app_context());
    app.run(|app, event| {
        // Al salir se cancela un trabajo en curso (pausa y reinicio) antes de soltar el puerto.
        if let RunEvent::Exit = event {
            app.state::<AppState>().shutdown();
        }
    });
}

/// Quita de un menú los elementos «Cerrar ventana» (su atajo Cmd+W es de la pestaña) y los menús que queden vacíos.
#[cfg(target_os = "macos")]
fn without_close_window<R: tauri::Runtime>(menu: &tauri::menu::Menu<R>) -> tauri::Result<()> {
    use tauri::menu::MenuItemKind;
    for kind in menu.items()? {
        let MenuItemKind::Submenu(sub) = kind else {
            continue;
        };
        for item in sub.items()? {
            if let MenuItemKind::Predefined(p) = &item
                && p.text()? == "Close Window"
            {
                sub.remove(p)?;
            }
        }
        if sub.items()?.is_empty() {
            menu.remove(&sub)?;
        }
    }
    Ok(())
}

/// La aplicación con todos sus comandos registrados. La comparten `run` y la prueba de humo
/// (`smoke`), que le pasa un contexto sin ventanas.
fn build_app(context: tauri::Context) -> tauri::App {
    let builder = tauri::Builder::default();
    // En Flatpak la interfaz se entera de que las actualizaciones las da Flathub (`updater.rs`).
    let builder = match updater::sandbox_init_script() {
        Some(script) => builder.append_invoke_initialization_script(script),
        None => builder,
    };
    builder
        .plugin(tauri_plugin_dialog::init())
        // Reiniciar tras instalar una actualización (`features/help/updater.ts`).
        .plugin(tauri_plugin_process::init())
        // Abrir el navegador con los formularios del proyecto (`features/help/openExternal.ts`). El
        // permiso de `capabilities/default.json` limita las direcciones a las del repositorio y su sitio;
        // los enlaces `<a>` no se abren solos (`open_js_links_on_click(false)`): solo lo que la interfaz pide.
        .plugin(
            tauri_plugin_opener::Builder::new()
                .open_js_links_on_click(false)
                .build(),
        )
        // El actualizador solo se registra si tauri.conf.json trae su llave pública (ver `updater.rs`).
        .setup(|app| {
            // Antes que nada: copia los datos de la carpeta del id anterior (`id_migration.rs`).
            id_migration::run(app.handle());
            updater::setup(app)
        })
        // macOS: el menú por defecto trae «Cerrar ventana» con Cmd+W, que cerraría la ventana entera antes de que la
        // interfaz lo vea. Con pestañas, Cmd+W cierra la pestaña activa (comando `tabs.close`).
        .menu(|handle| {
            let menu = tauri::menu::Menu::default(handle)?;
            #[cfg(target_os = "macos")]
            without_close_window(&menu)?;
            Ok(menu)
        })
        .manage(AppState::default())
        .manage(automation::AutomationState::default())
        .manage(remote::RemoteState::default())
        .manage(live_window::LiveWindowState::default())
        .invoke_handler(tauri::generate_handler![
            commands::app_version,
            commands::set_engine_locale,
            commands::import_file,
            commands::save_project,
            commands::load_project,
            commands::compute_job,
            commands::export_gcode,
            commands::list_devices,
            commands::save_device,
            commands::delete_device,
            commands::reorder_devices,
            commands::job_link_warning,
            commands::list_ports,
            probe::probe_port,
            automation::automation_configure,
            remote::remote_configure,
            remote::remote_status,
            remote::remote_regenerate,
            remote::remote_kick,
            remote::remote_update,
            remote::remote_set_route,
            remote::remote_set_live,
            remote::remote_camera_frame,
            remote::remote_start_result,
            commands::connect,
            commands::disconnect,
            commands::machine_command,
            commands::probe_touch,
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
            commands::ruida_prepare_store,
            commands::ruida_file_delete,
            commands::ruida_file_run,
            commands::ruida_settings_read,
            commands::ruida_settings_preview,
            commands::ruida_settings_preview_file,
            commands::ruida_settings_apply,
            commands::ruida_settings_restore,
            commands::ruida_settings_export,
            commands::ruida_calibrate,
            commands::ruida_backups,
            commands::ruida_backup_text,
            commands::lhy_preview,
            commands::lhy_home,
            commands::lhy_state,
            commands::galvo_preview,
            commands::galvo_state,
            commands::galvo_red_run,
            commands::galvo_red_light,
            commands::galvo_confirm_move,
            job_history::job_history_load,
            job_history::job_history_append,
            job_history::job_history_update,
            job_history::job_history_remove,
            job_history::job_history_clear,
            job_history::job_history_import,
            job_history::job_history_export,
            job_resume::interrupted_job,
            job_resume::interrupted_job_view,
            job_resume::discard_interrupted_job,
            job_resume::continue_check,
            job_resume::continue_frame,
            job_resume::continue_job,
            commands::galvo_frame,
            commands::galvo_adjust_frame,
            commands::galvo_verify,
            edit::text::list_fonts,
            edit::text::text_outline,
            edit::text::font_capabilities,
            edit::text::import_shx_fonts,
            edit::variables::read_csv,
            edit::variables::probe_images,
            edit::codes::generate_code,
            edit::geometry::geometry_op,
            edit::nest::nest_start,
            edit::nest::nest_poll,
            edit::nest::nest_cancel,
            edit::remnants::load_remnants,
            edit::remnants::save_remnants,
            edit::shape_ops::shape_op,
            edit::review::review_design,
            edit::parts::list_generators,
            edit::parts::generate_part,
            edit::design::warp_objects,
            edit::design::hull_outline,
            edit::design::tangent_circles,
            edit::trace::trace_image,
            edit::trace::vectors_to_bitmap,
            edit::trace::save_processed_image,
            edit::imaging::preview_image,
            edit::printcut::fit_registration,
            edit::printcut::plan_tiles,
            edit::art::art_import_svg,
            edit::art::art_list_user,
            edit::art::art_add_user,
            edit::art::art_remove_user,
            edit::art::art_save_user,
            edit::camera::detect_board,
            edit::camera::calibrate_lens,
            edit::camera::camera_marks,
            edit::camera::detect_camera_marks,
            edit::camera::compose_bed_image,
            edit::camera::detect_circle_in_image,
            edit::camera::load_camera_setup,
            edit::camera::save_camera_setup,
            edit::camera::save_board_file,
            edit::camera::fetch_camera_frame,
            edit::materials::load_material_library,
            edit::materials::load_factory_materials,
            edit::materials::save_material_library,
            edit::materials::import_material_library,
            edit::materials::export_material_library,
            edit::materials::material_test,
            edit::devices::device_defaults,
            edit::devices::rotary_test,
            edit::devices::read_lbdev,
            edit::files::import_file_with,
            edit::files::export_design_file,
            edit::files::print_design,
            autosave::autosave_write,
            autosave::autosave_list,
            autosave::autosave_read,
            autosave::autosave_discard,
            versions::commands::versions_list,
            versions::commands::versions_thumb,
            versions::commands::versions_read,
            versions::commands::versions_capture,
            versions::commands::versions_snapshot,
            versions::commands::versions_delete,
            versions::commands::versions_clear,
            versions::commands::versions_projects,
            versions::commands::versions_config_get,
            versions::commands::versions_config_set,
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
            settings_backup::grbl_settings_autobackup,
            window::new_window,
            live_window::live_window_open,
            live_window::live_window_close,
            live_window::live_window_send,
            live_window::live_window_hello,
        ])
        .build(context)
        .expect("no se pudo iniciar la aplicación Tlaku")
}

#[cfg(test)]
mod tests {
    use super::commands::app_version;

    #[test]
    fn version_coincide_con_el_paquete() {
        assert_eq!(app_version(), env!("CARGO_PKG_VERSION"));
    }
}
