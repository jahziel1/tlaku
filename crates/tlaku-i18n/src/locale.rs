use std::cell::Cell;
use std::sync::atomic::{AtomicU8, Ordering};

/// Idioma de los mensajes del motor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Locale {
    Es,
    En,
}

impl Locale {
    pub const ALL: [Locale; 2] = [Locale::Es, Locale::En];

    /// Código corto: `"es"` o `"en"` (también el nombre de la carpeta en `locales/`).
    pub fn code(self) -> &'static str {
        match self {
            Locale::Es => "es",
            Locale::En => "en",
        }
    }

    /// El otro idioma, que sirve de respaldo.
    pub fn other(self) -> Locale {
        match self {
            Locale::Es => Locale::En,
            Locale::En => Locale::Es,
        }
    }

    /// Reconoce `es`, `en`, `es_MX.UTF-8`, `en-US`, `English`... Cualquier otro valor, `None`.
    pub fn parse(s: &str) -> Option<Locale> {
        let s = s.trim().to_ascii_lowercase();
        if s == "en" || s.starts_with("en_") || s.starts_with("en-") || s.starts_with("en.") {
            Some(Locale::En)
        } else if s == "es" || s.starts_with("es_") || s.starts_with("es-") || s.starts_with("es.")
        {
            Some(Locale::Es)
        } else {
            None
        }
    }

    fn to_u8(self) -> u8 {
        match self {
            Locale::Es => 0,
            Locale::En => 1,
        }
    }

    fn from_u8(n: u8) -> Locale {
        if n == 1 { Locale::En } else { Locale::Es }
    }
}

static GLOBAL: AtomicU8 = AtomicU8::new(0);
const NO_OVERRIDE: u8 = u8::MAX;

thread_local! {
    static OVERRIDE: Cell<u8> = const { Cell::new(NO_OVERRIDE) };
}

/// Idioma activo: el del hilo si hay un [`with_locale`] en curso, si no el global.
pub fn locale() -> Locale {
    let o = OVERRIDE.with(Cell::get);
    if o != NO_OVERRIDE {
        return Locale::from_u8(o);
    }
    Locale::from_u8(GLOBAL.load(Ordering::Relaxed))
}

/// Fija el idioma global (la app lo llama al arrancar y al cambiar la preferencia).
pub fn set_locale(l: Locale) {
    GLOBAL.store(l.to_u8(), Ordering::Relaxed);
}

/// Ejecuta `f` con otro idioma solo en este hilo; no afecta a los demás hilos ni a las pruebas
/// que corren en paralelo. Restaura el anterior al terminar, incluso si `f` entra en pánico.
pub fn with_locale<T>(l: Locale, f: impl FnOnce() -> T) -> T {
    struct Restore(u8);
    impl Drop for Restore {
        fn drop(&mut self) {
            OVERRIDE.with(|c| c.set(self.0));
        }
    }
    let _restore = Restore(OVERRIDE.with(|c| c.replace(l.to_u8())));
    f()
}

/// Idioma pedido por el entorno: `TLAKU_LANG` primero, luego `LANG`. `None` si no dicen es/en.
pub fn locale_from_env() -> Option<Locale> {
    ["TLAKU_LANG", "LANG"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find_map(|v| Locale::parse(&v))
}

/// Fija el idioma global según el entorno (para el CLI). Sin variable reconocida, español.
pub fn init_from_env() -> Locale {
    let l = locale_from_env().unwrap_or(Locale::Es);
    set_locale(l);
    l
}
