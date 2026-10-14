//! Mensajes de usuario del motor (avisos y errores) en varios idiomas.
//!
//! - Los textos viven en `locales/{es,en}/<crate>.txt` como `clave = plantilla` y se incluyen al
//!   compilar (ver [`catalog`]).
//! - [`tr!`] busca una clave en el idioma activo y rellena los marcadores con nombre:
//!   `tr!("pipeline.import.no_paths", path = p.display())`.
//! - El idioma activo es global y seguro entre hilos ([`set_locale`]); por omisión es español.
//!   [`with_locale`] lo cambia solo en el hilo actual (para pruebas y para respuestas por petición).
//! - Si falta una clave en un idioma se usa el otro; si falta en ambos se devuelve la clave. Nunca
//!   hay pánico. [`take_fallbacks`] devuelve lo que cayó en el respaldo (las pruebas lo vigilan).
//! - Un error que la interfaz debe reconocer lleva un código estable: [`coded`] y [`split_code`].
//!
//! Guía para convertir mensajes: `crates/tlaku-i18n/README.md`.

pub mod catalog;
mod locale;
mod render;

pub use locale::{Locale, init_from_env, locale, locale_from_env, set_locale, with_locale};
pub use render::{
    Arg, coded, msg_is, msg_matches, placeholders, split_code, take_fallbacks, translate,
    translate_in,
};

/// Texto del catálogo en el idioma activo, con los marcadores `{nombre}` reemplazados.
///
/// `tr!("pipeline.import.no_paths")` o `tr!("pipeline.import.bad_svg", path = p.display(), err = e)`.
/// Cada valor se escribe con `Display`; para un formato (`{:.1}`) pásalo ya formateado:
/// `tr!("...", mm = format!("{x:.1}"))`. La clave debe ser un literal (una prueba del crate
/// comprueba que existe en el catálogo).
#[macro_export]
macro_rules! tr {
    ($key:literal $(,)?) => {
        $crate::translate($key, &[])
    };
    ($key:literal, $($name:ident = $val:expr),+ $(,)?) => {
        $crate::translate($key, &[$((stringify!($name), $crate::Arg::new(&$val))),+])
    };
}
