//! Reúne los aportes de la comunidad (`community/materials/*.json`, un archivo por aporte) en una
//! lista JSON que `src/community.rs` incluye en el programa. No valida nada: eso lo hacen la
//! prueba `community::tests::los_archivos_de_la_comunidad_son_validos` y la expansión al arrancar.

use std::path::PathBuf;
use std::{env, fs};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let dir = manifest.join("../../community/materials");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", dir.display());

    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "json"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();

    let mut parts = Vec::new();
    for file in &files {
        println!("cargo:rerun-if-changed={}", file.display());
        let text = fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", file.display()));
        parts.push(text.trim().to_string());
    }
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("community.json");
    fs::write(&out, format!("[{}]", parts.join(",\n"))).expect("escribir community.json");
}
