//! Prueba de humo de la aplicación real (ver `src/smoke.rs`). Código de salida 0 = correcta.
fn main() {
    std::process::exit(tlaku_desktop_lib::smoke::run());
}
