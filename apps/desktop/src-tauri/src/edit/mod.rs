//! Comandos de edición, cámara (fase 3) y materiales (fase 4), un archivo por área.

pub mod art;
pub mod camera;
pub mod codes;
pub mod design;
pub mod devices;
pub mod files;
pub mod geometry;
pub mod imaging;
pub mod materials;
pub mod nest;
pub mod parts;
pub mod printcut;
pub mod review;
pub mod shape_ops;
pub mod text;
pub mod trace;
pub mod variables;

use tlaku_i18n::tr;

/// Error de un comando que corre en un hilo aparte (`spawn_blocking`) y no terminó.
pub(crate) fn task_interrupted(e: impl std::fmt::Display) -> String {
    tr!("desktop.edit.task_interrupted", err = e)
}

/// No se pudo ubicar la carpeta de configuración de la app.
pub(crate) fn config_dir_missing(e: impl std::fmt::Display) -> String {
    tr!("desktop.edit.config_dir_missing", err = e)
}

/// Nombre o identificador entre comillas, para listas de objetos en los mensajes.
pub(crate) fn quoted(name: impl std::fmt::Display) -> String {
    tr!("desktop.edit.quoted_name", name = name)
}

#[cfg(test)]
mod i18n_tests {
    use super::{art, camera, geometry, materials, variables};
    use tlaku_i18n::{Locale, with_locale};

    #[test]
    fn camera_url_error_in_both_languages() {
        let es = camera::grab_network_frame("https://camara.local/").unwrap_err();
        assert!(es.starts_with("Las cámaras de red se piden por http://"));
        let en = with_locale(Locale::En, || {
            camera::grab_network_frame("https://camara.local/").unwrap_err()
        });
        assert!(
            en.starts_with("Network cameras are requested over http://"),
            "{en}"
        );
    }

    #[test]
    fn csv_and_art_errors_in_english() {
        let en = with_locale(Locale::En, || variables::parse_csv(b"").unwrap_err());
        assert_eq!(en, "The CSV is empty.");
        let en = with_locale(Locale::En, || {
            art::remove_art(std::path::Path::new("x"), "../a").unwrap_err()
        });
        assert_eq!(en, "Invalid artwork name: \"../a\".");
    }

    #[test]
    fn geometry_without_objects_in_english() {
        let en = with_locale(Locale::En, || {
            geometry::run(geometry::GeometryOp::Union, &[], None, None).unwrap_err()
        });
        assert_eq!(en, "There are no objects to operate on.");
    }

    #[test]
    fn unsupported_library_version_is_recognized_in_any_language() {
        let text = r#"{"version": 999, "presets": []}"#;
        for locale in Locale::ALL {
            let e = with_locale(locale, || materials::parse_library(text).unwrap_err());
            assert!(
                tlaku_i18n::msg_is(&e, "desktop.edit.materials.unsupported_version"),
                "{e}"
            );
        }
        let en = with_locale(Locale::En, || materials::parse_library(text).unwrap_err());
        assert!(en.starts_with("Unsupported library version: 999"), "{en}");
    }
}
