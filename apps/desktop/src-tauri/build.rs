//! Genera la lista de comandos de la app para el control de acceso (ACL) de Tauri.
//!
//! Sin una lista de comandos propios, Tauri deja que CUALQUIER ventana llame a CUALQUIER comando
//! de la app, y entonces una capacidad «mínima» no limitaría nada. Aquí se leen los comandos de
//! `generate_handler![...]` (en `src/lib.rs`, la única fuente) y se les crea un permiso
//! `allow-<comando>` a cada uno; además se arma el conjunto `main-window` con todos. La capacidad
//! de las ventanas principales (`capabilities/default.json`) incluye ese conjunto; la de la
//! ventana aparte de la vista en vivo (`capabilities/live.json`) solo nombra los que necesita.
//! Un comando nuevo queda disponible para las ventanas principales sin tocar nada más.

use std::{fs, path::Path};

/// Comandos registrados en `tauri::generate_handler![...]` de `src/lib.rs`, sin la ruta del módulo.
fn registered_commands(lib_rs: &str) -> Vec<String> {
    let start = lib_rs
        .find("generate_handler![")
        .expect("lib.rs no tiene generate_handler![")
        + "generate_handler![".len();
    let end = start
        + lib_rs[start..]
            .find(']')
            .expect("generate_handler! sin cierre");
    lib_rs[start..end]
        .lines()
        .map(|l| l.split("//").next().unwrap_or("").trim())
        .flat_map(|l| l.split(','))
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(|c| c.rsplit("::").next().unwrap_or(c).to_string())
        .collect()
}

fn write_if_changed(path: &Path, text: &str) {
    if fs::read_to_string(path).ok().as_deref() != Some(text) {
        fs::create_dir_all(path.parent().expect("ruta con carpeta")).expect("crear permissions/");
        fs::write(path, text).expect("escribir el conjunto de permisos");
    }
}

fn main() {
    println!("cargo:rerun-if-changed=src/lib.rs");
    let lib_rs = fs::read_to_string("src/lib.rs").expect("leer src/lib.rs");
    let commands = registered_commands(&lib_rs);
    assert!(
        commands.len() > 100,
        "se esperaban más de 100 comandos y se leyeron {}",
        commands.len()
    );
    let mut set = String::from(
        "# Generado por build.rs a partir de `generate_handler!` de src/lib.rs. No editar.\n\n[[set]]\nidentifier = \"main-window\"\ndescription = \"Todos los comandos de la app: lo usan las ventanas principales y las de documento. La ventana aparte de la vista en vivo NO lo incluye.\"\npermissions = [\n",
    );
    for c in &commands {
        set.push_str(&format!("  \"allow-{}\",\n", c.replace('_', "-")));
    }
    set.push_str("]\n");
    write_if_changed(Path::new("permissions/main-window.toml"), &set);
    let leaked: &'static [&'static str] = Box::leak(
        commands
            .into_iter()
            .map(|c| &*Box::leak(c.into_boxed_str()))
            .collect::<Vec<&'static str>>()
            .into_boxed_slice(),
    );
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(leaked)),
    )
    .expect("tauri-build falló");
}
