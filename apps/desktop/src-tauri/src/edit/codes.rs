//! Comandos del área `codes`: QR y códigos de barras como formas vectoriales cerradas.
//!
//! La interfaz inserta el resultado como un objeto vectorial; aquí solo se llama a `tlaku-codes`.

use tlaku_codes::{CodeOutput, CodeSpec};
use tlaku_text::FontLibrary;

/// Genera el código con las fuentes de `lib` (solo se usan para el texto legible).
pub fn run_generate(lib: &FontLibrary, spec: &CodeSpec) -> Result<CodeOutput, String> {
    tlaku_codes::generate(lib, spec)
}

/// Genera un código QR o de barras. Las formas salen en mm con la esquina inferior izquierda en
/// `(0, 0)` y Y hacia arriba, igual que el contorno de un texto.
#[tauri::command]
pub async fn generate_code(spec: CodeSpec) -> Result<CodeOutput, String> {
    tauri::async_runtime::spawn_blocking(move || {
        // El sistema de fuentes solo hace falta (y solo se carga) con texto legible.
        if spec.human_readable && spec.kind.is_linear() {
            run_generate(FontLibrary::system_shared(), &spec)
        } else {
            run_generate(&FontLibrary::new(), &spec)
        }
    })
    .await
    .map_err(crate::edit::task_interrupted)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tlaku_codes::{CodeKind, QrEcc};

    fn fixture_lib() -> FontLibrary {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../crates/tlaku-text/tests/fixtures/BebasNeue-Regular.ttf");
        let mut lib = FontLibrary::new();
        lib.load_font_file(&path)
            .expect("la fuente de prueba debe cargar");
        lib
    }

    #[test]
    fn spec_deserializes_what_the_ui_sends() {
        // Lo que manda `invoke('generate_code', { spec })`.
        let json = serde_json::json!({
            "kind": "qr", "data": "https://tlaku.example", "ecc": "q", "size_mm": 25.0,
            "height_mm": 15.0, "margin_mm": 2.0, "human_readable": false, "invert": true,
            "font_family": ""
        });
        let spec: CodeSpec = serde_json::from_value(json).unwrap();
        assert_eq!(spec.kind, CodeKind::Qr);
        assert_eq!(spec.ecc, QrEcc::Q);
        assert!(spec.invert);
    }

    #[test]
    fn result_has_the_shape_the_ui_expects() {
        let spec = CodeSpec {
            kind: CodeKind::Code128,
            data: "Tlaku".into(),
            size_mm: 40.0,
            height_mm: 10.0,
            human_readable: true,
            font_family: "Bebas Neue".into(),
            ..CodeSpec::default()
        };
        let out = run_generate(&fixture_lib(), &spec).unwrap();
        let v = serde_json::to_value(&out).unwrap();
        assert_eq!(v["paths"][0][0]["type"], "move_to");
        assert!(v["width_mm"].as_f64().unwrap() <= 40.0 + 1e-6);
        assert!(v["height_mm"].as_f64().unwrap() > 10.0);
        assert!(v["modules"].as_u64().unwrap() > 0);
        assert!(v["warnings"].is_array());
    }

    #[test]
    fn errors_reach_the_ui_as_text() {
        let spec = CodeSpec {
            kind: CodeKind::Ean13,
            data: "123".into(),
            ..CodeSpec::default()
        };
        let e = run_generate(&fixture_lib(), &spec).unwrap_err();
        assert!(e.contains("EAN-13"), "{e}");
    }

    #[test]
    fn command_runs_without_loading_system_fonts_for_qr() {
        let spec = CodeSpec {
            data: "hola".into(),
            ..CodeSpec::default()
        };
        let out = tauri::async_runtime::block_on(generate_code(spec)).unwrap();
        assert!(!out.paths.is_empty());
    }
}
