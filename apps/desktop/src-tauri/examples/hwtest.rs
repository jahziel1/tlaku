//! Prueba con hardware real a través de la app (ver `src/hwtest.rs`). Nunca enciende el láser.
fn main() {
    let mut args = std::env::args().skip(1);
    let port = args.next().expect("uso: hwtest <puerto> <paso>...");
    std::process::exit(tlaku_desktop_lib::hwtest::run(port, args.collect()));
}
