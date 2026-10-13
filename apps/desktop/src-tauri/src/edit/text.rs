//! Comandos del área `text` (fase 3): fuentes instaladas y contorno de un objeto de texto.
//!
//! La interfaz es dueña del documento; aquí solo se convierte el `TextObject` del contrato a
//! la `TextSpec` de `tlaku-text` y se devuelven las curvas con sus advertencias.

use serde::Serialize;
use tlaku_core::bezpath::BezPath;
use tlaku_doc::{TextAlign, TextObject};
use tlaku_text::{Align, FontFamily, FontLibrary, OnPathSpec, TextSpec};

/// Misma forma que `TextOutlineResult` de `src/api/types.ts`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TextOutlineResult {
    /// Un `BezPath` por glifo visible, en coordenadas locales del objeto (mm, Y hacia arriba).
    pub paths: Vec<BezPath>,
    /// Sustituciones de fuente, caracteres sin glifo, texto que no cabe en la trayectoria...
    pub warnings: Vec<String>,
}

/// `TextObject` del documento a la descripción que entiende el motor de texto. El contorno ya
/// generado (`outline`) no interviene: es justo lo que se va a recalcular.
pub fn to_spec(text: &TextObject) -> TextSpec {
    TextSpec {
        text: text.text.clone(),
        font_family: text.font_family.clone(),
        bold: text.bold,
        italic: text.italic,
        size_mm: text.size_mm,
        letter_spacing_mm: text.letter_spacing_mm,
        line_spacing: text.line_spacing,
        align: match text.align {
            TextAlign::Left => Align::Left,
            TextAlign::Center => Align::Center,
            TextAlign::Right => Align::Right,
        },
        on_path: text.on_path.as_ref().map(|p| OnPathSpec {
            offset_mm: p.offset_mm,
            flip: p.flip,
        }),
    }
}

/// Convierte el texto en curvas con las fuentes de `lib`.
pub fn run_outline(
    lib: &FontLibrary,
    text: &TextObject,
    path: Option<&BezPath>,
) -> Result<TextOutlineResult, String> {
    let out = tlaku_text::outline(lib, &to_spec(text), path)?;
    Ok(TextOutlineResult {
        paths: out.paths,
        warnings: out.warnings,
    })
}

/// Familias instaladas con sus estilos. La primera llamada carga las fuentes del sistema
/// (puede tardar); las siguientes reutilizan la biblioteca compartida.
#[tauri::command]
pub async fn list_fonts() -> Result<Vec<FontFamily>, String> {
    tauri::async_runtime::spawn_blocking(|| FontLibrary::system_shared().families())
        .await
        .map_err(|e| format!("La tarea se interrumpió: {e}"))
}

/// Contorno del texto; `path` es la trayectoria (en el mismo sistema local que el contorno)
/// cuando el texto va sobre una curva.
#[tauri::command]
pub async fn text_outline(
    text: TextObject,
    path: Option<BezPath>,
) -> Result<TextOutlineResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        run_outline(FontLibrary::system_shared(), &text, path.as_ref())
    })
    .await
    .map_err(|e| format!("La tarea se interrumpió: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tlaku_core::Point;
    use tlaku_core::bezpath::PathSeg;
    use tlaku_doc::TextOnPath;

    fn fixture_lib() -> FontLibrary {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../crates/tlaku-text/tests/fixtures/BebasNeue-Regular.ttf");
        let mut lib = FontLibrary::new();
        lib.load_font_file(&path)
            .expect("la fuente de prueba debe cargar");
        lib
    }

    fn text(s: &str) -> TextObject {
        TextObject {
            text: s.into(),
            font_family: "Bebas Neue".into(),
            bold: false,
            italic: false,
            size_mm: 10.0,
            letter_spacing_mm: 0.0,
            line_spacing: 1.2,
            align: TextAlign::Left,
            on_path: None,
            outline: vec![],
        }
    }

    fn line(len: f64) -> BezPath {
        BezPath(vec![
            PathSeg::MoveTo {
                to: Point::new(0.0, 0.0),
            },
            PathSeg::LineTo {
                to: Point::new(len, 0.0),
            },
        ])
    }

    #[test]
    fn conversion_copies_every_field() {
        let mut t = text("Hola\nmundo");
        t.font_family = "Arial".into();
        t.bold = true;
        t.italic = true;
        t.size_mm = 7.5;
        t.letter_spacing_mm = 0.4;
        t.line_spacing = 1.5;
        t.align = TextAlign::Right;
        t.on_path = Some(TextOnPath {
            object_id: "arco".into(),
            offset_mm: 3.0,
            flip: true,
        });
        let spec = to_spec(&t);
        assert_eq!(spec.text, "Hola\nmundo");
        assert_eq!(spec.font_family, "Arial");
        assert!(spec.bold && spec.italic);
        assert_eq!(spec.size_mm, 7.5);
        assert_eq!(spec.letter_spacing_mm, 0.4);
        assert_eq!(spec.line_spacing, 1.5);
        assert_eq!(spec.align, Align::Right);
        assert_eq!(
            spec.on_path,
            Some(OnPathSpec {
                offset_mm: 3.0,
                flip: true
            })
        );
        assert_eq!(to_spec(&text("x")).align, Align::Left);
        assert_eq!(to_spec(&text("x")).on_path, None);
    }

    #[test]
    fn conversion_maps_each_alignment() {
        for (from, to) in [
            (TextAlign::Left, Align::Left),
            (TextAlign::Center, Align::Center),
            (TextAlign::Right, Align::Right),
        ] {
            let mut t = text("x");
            t.align = from;
            assert_eq!(to_spec(&t).align, to);
        }
    }

    #[test]
    fn outline_has_one_path_per_visible_glyph() {
        let r = run_outline(&fixture_lib(), &text("AB"), None).unwrap();
        assert_eq!(r.paths.len(), 2);
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    }

    #[test]
    fn outline_reports_italic_substitution() {
        let mut t = text("A");
        t.italic = true;
        let r = run_outline(&fixture_lib(), &t, None).unwrap();
        assert_eq!(r.paths.len(), 1);
        assert!(
            r.warnings.iter().any(|w| w.contains("cursiva")),
            "{:?}",
            r.warnings
        );
    }

    #[test]
    fn outline_follows_a_path() {
        let mut t = text("AB");
        t.on_path = Some(TextOnPath {
            object_id: "guia".into(),
            offset_mm: 0.0,
            flip: false,
        });
        let r = run_outline(&fixture_lib(), &t, Some(&line(100.0))).unwrap();
        assert_eq!(r.paths.len(), 2);
    }

    #[test]
    fn missing_path_is_an_error() {
        let mut t = text("A");
        t.on_path = Some(TextOnPath {
            object_id: "guia".into(),
            offset_mm: 0.0,
            flip: false,
        });
        assert!(run_outline(&fixture_lib(), &t, None).is_err());
    }

    #[test]
    fn unknown_family_and_bad_size_are_errors() {
        let mut t = text("A");
        t.font_family = "Fuente Que No Existe".into();
        let e = run_outline(&fixture_lib(), &t, None).unwrap_err();
        assert!(e.contains("Fuente Que No Existe"), "{e}");
        let mut t = text("A");
        t.size_mm = 0.0;
        assert!(run_outline(&fixture_lib(), &t, None).is_err());
    }

    #[test]
    fn result_serializes_with_the_contract_shape() {
        let r = run_outline(&fixture_lib(), &text("A"), None).unwrap();
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["paths"][0][0]["type"], "move_to");
        assert!(v["warnings"].is_array());
    }

    #[test]
    fn command_deserializes_the_invoke_arguments() {
        // Lo que manda `invoke('text_outline', { text, path })` debe entrar en los tipos del comando.
        let json = serde_json::json!({
            "text": {
                "text": "A", "font_family": "Bebas Neue", "bold": false, "italic": false,
                "size_mm": 10.0, "letter_spacing_mm": 0.0, "line_spacing": 1.2,
                "align": "center", "on_path": null, "outline": []
            },
            "path": null
        });
        let t: TextObject = serde_json::from_value(json["text"].clone()).unwrap();
        let p: Option<BezPath> = serde_json::from_value(json["path"].clone()).unwrap();
        assert_eq!(t.align, TextAlign::Center);
        assert!(p.is_none());
    }

    #[test]
    fn list_fonts_command_runs_and_is_sorted() {
        let fonts = tauri::async_runtime::block_on(list_fonts()).unwrap();
        let names: Vec<String> = fonts.iter().map(|f| f.family.to_lowercase()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }
}
