//! Captura de Ctrl+C sin dependencias externas.
//!
//! Se declara `signal` de la biblioteca C (que `std` ya enlaza en Unix y Windows) y el
//! manejador solo activa un indicador atómico, que es lo único seguro dentro de un manejador.
//! Limitación: solo se conoce la primera pulsación; una segunda no fuerza la salida, y en
//! plataformas que no sean Unix ni Windows no se captura nada.

use std::sync::atomic::{AtomicBool, Ordering};

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

#[cfg(any(unix, windows))]
mod imp {
    use super::*;

    const SIGINT: i32 = 2;

    unsafe extern "C" {
        fn signal(signum: i32, handler: usize) -> usize;
    }

    extern "C" fn on_sigint(_: i32) {
        INTERRUPTED.store(true, Ordering::SeqCst);
    }

    pub fn install() {
        // SAFETY: `on_sigint` solo toca un atómico y tiene la firma que espera `signal`.
        unsafe {
            signal(SIGINT, on_sigint as extern "C" fn(i32) as usize);
        }
    }
}

#[cfg(not(any(unix, windows)))]
mod imp {
    pub fn install() {}
}

/// Instala el manejador de Ctrl+C y devuelve el indicador que se activa al pulsarlo.
pub fn install() -> &'static AtomicBool {
    imp::install();
    &INTERRUPTED
}

#[cfg(all(test, any(unix, windows)))]
mod tests {
    use super::*;

    unsafe extern "C" {
        fn raise(sig: i32) -> i32;
    }

    #[test]
    fn ctrl_c_sets_the_flag_instead_of_killing_the_process() {
        let flag = install();
        assert!(!flag.load(Ordering::SeqCst));
        // SAFETY: `raise` solo envía la señal al propio proceso, que ya tiene manejador.
        unsafe { raise(2) };
        assert!(flag.load(Ordering::SeqCst));
    }
}
