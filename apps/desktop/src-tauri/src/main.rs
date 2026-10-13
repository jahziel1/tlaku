// En Windows, no abre una consola detrás de la app en las compilaciones de lanzamiento.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    lasersoft_desktop_lib::run();
}
