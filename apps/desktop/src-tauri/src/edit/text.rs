//! Comandos del área `text` (fase 3): fuentes instaladas y contorno de un objeto de texto.
//!
//! La interfaz es dueña del documento; aquí solo se convierte el `TextObject` del contrato a
//! la `TextSpec` de `tlaku-text` y se devuelven las curvas con sus advertencias.
//!
//! `list_fonts` devuelve también las fuentes de una línea (Hershey incluidas y las SHX que el
//! usuario haya importado), marcadas con `single_line`.

use serde::Serialize;
use std::path::{Path, PathBuf};
use tlaku_core::bezpath::BezPath;
use tlaku_doc::{TextAlign, TextObject, TextVAlign};
use tlaku_i18n::tr;
use tlaku_text::{
    Align, FontAxis, FontCapabilities, FontFamily, FontFeature, FontLibrary, OnPathSpec, TextSpec,
    VAlign,
};

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
        bend_deg: text.bend_deg.unwrap_or(0.0),
        welded: text.welded.unwrap_or(false),
        features: text
            .features
            .iter()
            .flatten()
            .map(|f| FontFeature {
                tag: f.tag.clone(),
                value: f.value,
            })
            .collect(),
        variations: text
            .variations
            .iter()
            .flatten()
            .map(|a| FontAxis {
                tag: a.tag.clone(),
                value: a.value,
            })
            .collect(),
        uppercase: text.uppercase.unwrap_or(false),
        valign: match text.valign {
            None | Some(TextVAlign::Baseline) => VAlign::Baseline,
            Some(TextVAlign::Top) => VAlign::Top,
            Some(TextVAlign::Middle) => VAlign::Middle,
            Some(TextVAlign::Bottom) => VAlign::Bottom,
        },
        word_spacing_mm: text.word_spacing_mm.unwrap_or(0.0),
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

/// Familias instaladas con sus estilos, seguidas de las fuentes de una línea. La primera llamada
/// carga las fuentes del sistema (puede tardar); las siguientes reutilizan la biblioteca
/// compartida.
#[tauri::command]
pub async fn list_fonts() -> Result<Vec<FontFamily>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let lib = FontLibrary::system_shared();
        load_saved_shx(lib);
        let mut all = lib.families();
        all.extend(lib.line_families());
        all
    })
    .await
    .map_err(crate::edit::task_interrupted)
}

/// Carpeta donde se guardan las fuentes SHX importadas (se pueden fijar en las pruebas).
fn shx_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("TLAKU_SHX_DIR") {
        return Some(PathBuf::from(dir));
    }
    #[cfg(target_os = "macos")]
    let base =
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"));
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")));
    base.map(|b| b.join("tlaku").join("fonts-shx"))
}

/// Carga una sola vez las SHX que el usuario importó en sesiones anteriores.
fn load_saved_shx(lib: &FontLibrary) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let Some(dir) = shx_dir() else { return };
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("shx"))
            {
                // Un archivo dañado no impide cargar los demás.
                let _ = lib.load_shx_file(&path);
            }
        }
    });
}

/// Importa fuentes SHX de AutoCAD: las valida, las copia a la carpeta de la aplicación (para
/// que sigan disponibles) y devuelve los nombres registrados.
pub fn run_import_shx(
    lib: &FontLibrary,
    paths: &[PathBuf],
    dir: Option<&Path>,
) -> Result<Vec<String>, String> {
    if paths.is_empty() {
        return Err(tr!("desktop.edit.text.no_file"));
    }
    let mut names = Vec::new();
    let mut errors = Vec::new();
    for path in paths {
        match lib.load_shx_file(path) {
            Ok(name) => {
                if let (Some(dir), Some(file)) = (dir, path.file_name()) {
                    // Que no se pueda guardar no invalida la importación de esta sesión.
                    if std::fs::create_dir_all(dir).is_ok() {
                        let _ = std::fs::copy(path, dir.join(file));
                    }
                }
                names.push(name);
            }
            Err(e) => errors.push(e),
        }
    }
    if names.is_empty() {
        return Err(errors.join("; "));
    }
    Ok(names)
}

/// Importa archivos `.shx`. Devuelve los nombres de las fuentes añadidas; si algún archivo no se
/// pudo leer pero otros sí, solo se informan los buenos.
#[tauri::command]
pub async fn import_shx_fonts(paths: Vec<String>) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        run_import_shx(FontLibrary::system_shared(), &paths, shx_dir().as_deref())
    })
    .await
    .map_err(crate::edit::task_interrupted)?
}

/// Funciones OpenType y ejes variables de la fuente del texto (familia, negrita y cursiva), con
/// una vista previa de cada función. Solo lista lo que la fuente declara.
pub fn run_capabilities(lib: &FontLibrary, text: &TextObject) -> Result<FontCapabilities, String> {
    tlaku_text::font_capabilities(lib, &to_spec(text))
}

/// Qué ofrece la fuente del texto: funciones OpenType (con vista previa) y ejes variables.
#[tauri::command]
pub async fn font_capabilities(text: TextObject) -> Result<FontCapabilities, String> {
    tauri::async_runtime::spawn_blocking(move || {
        run_capabilities(FontLibrary::system_shared(), &text)
    })
    .await
    .map_err(crate::edit::task_interrupted)?
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
    .map_err(crate::edit::task_interrupted)?
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
            bend_deg: None,
            welded: None,
            features: None,
            variations: None,
            uppercase: None,
            valign: None,
            word_spacing_mm: None,
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
        // Primero las del sistema (ordenadas) y al final las de una línea.
        let names: Vec<String> = fonts
            .iter()
            .filter(|f| !f.single_line)
            .map(|f| f.family.to_lowercase())
            .collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn conversion_copies_bend_and_welded() {
        let mut t = text("x");
        assert_eq!(to_spec(&t).bend_deg, 0.0);
        assert!(!to_spec(&t).welded);
        t.bend_deg = Some(-45.0);
        t.welded = Some(true);
        let s = to_spec(&t);
        assert_eq!(s.bend_deg, -45.0);
        assert!(s.welded);
    }

    #[test]
    fn hershey_text_gives_open_paths() {
        let mut t = text("Hola");
        t.font_family = "Hershey Roman Simplex".into();
        let r = run_outline(&fixture_lib(), &t, None).unwrap();
        assert_eq!(r.paths.len(), 4);
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
        assert!(
            r.paths
                .iter()
                .all(|p| !p.0.iter().any(|s| matches!(s, PathSeg::Close)))
        );
    }

    #[test]
    fn bend_and_weld_change_the_outline() {
        let mut t = text("AV");
        t.letter_spacing_mm = -2.0;
        let flat = run_outline(&fixture_lib(), &t, None).unwrap();
        t.welded = Some(true);
        let welded = run_outline(&fixture_lib(), &t, None).unwrap();
        assert_eq!(flat.paths.len(), 2);
        assert_eq!(welded.paths.len(), 1);
        t.bend_deg = Some(90.0);
        let bent = run_outline(&fixture_lib(), &t, None).unwrap();
        assert_ne!(bent.paths, welded.paths);
    }

    #[test]
    fn old_documents_without_bend_fields_still_load() {
        let json = serde_json::json!({
            "text": "A", "font_family": "Bebas Neue", "bold": false, "italic": false,
            "size_mm": 10.0, "letter_spacing_mm": 0.0, "line_spacing": 1.2,
            "align": "left", "on_path": null, "outline": []
        });
        let t: TextObject = serde_json::from_value(json).unwrap();
        assert_eq!(t.bend_deg, None);
        assert_eq!(t.welded, None);
        // Sin valores no se escriben.
        let back = serde_json::to_value(&t).unwrap();
        assert!(back.get("bend_deg").is_none() && back.get("welded").is_none());
    }

    fn test_font_lib() -> FontLibrary {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../crates/tlaku-text/tests/fixtures/TlakuTest-Var.ttf");
        let mut lib = FontLibrary::new();
        lib.load_font_file(&path)
            .expect("la fuente de prueba debe cargar");
        lib
    }

    #[test]
    fn conversion_copies_opentype_options() {
        use tlaku_doc::{TextAxis, TextFeature};
        let mut t = text("x");
        let plain = to_spec(&t);
        assert!(plain.features.is_empty() && plain.variations.is_empty());
        assert!(!plain.uppercase && plain.valign == VAlign::Baseline);
        assert_eq!(plain.word_spacing_mm, 0.0);
        t.features = Some(vec![TextFeature {
            tag: "ss01".into(),
            value: 1,
        }]);
        t.variations = Some(vec![TextAxis {
            tag: "wght".into(),
            value: 650.0,
        }]);
        t.uppercase = Some(true);
        t.valign = Some(TextVAlign::Middle);
        t.word_spacing_mm = Some(1.5);
        let s = to_spec(&t);
        assert_eq!(
            (s.features[0].tag.as_str(), s.features[0].value),
            ("ss01", 1)
        );
        assert_eq!(
            (s.variations[0].tag.as_str(), s.variations[0].value),
            ("wght", 650.0)
        );
        assert!(s.uppercase);
        assert_eq!(s.valign, VAlign::Middle);
        assert_eq!(s.word_spacing_mm, 1.5);
    }

    #[test]
    fn opentype_options_change_the_outline_through_the_command_path() {
        use tlaku_doc::{TextAxis, TextFeature};
        let lib = test_font_lib();
        let mut t = text("fi");
        t.font_family = "Tlaku Test".into();
        let lig = run_outline(&lib, &t, None).unwrap();
        t.features = Some(vec![TextFeature {
            tag: "liga".into(),
            value: 0,
        }]);
        let apart = run_outline(&lib, &t, None).unwrap();
        assert_eq!((lig.paths.len(), apart.paths.len()), (1, 2));
        let mut v = text("A");
        v.font_family = "Tlaku Test".into();
        let light = run_outline(&lib, &v, None).unwrap();
        v.variations = Some(vec![TextAxis {
            tag: "wght".into(),
            value: 900.0,
        }]);
        assert_ne!(run_outline(&lib, &v, None).unwrap().paths, light.paths);
    }

    #[test]
    fn capabilities_come_from_the_font() {
        let lib = test_font_lib();
        let mut t = text("");
        t.font_family = "Tlaku Test".into();
        let caps = run_capabilities(&lib, &t).unwrap();
        assert!(caps.features.iter().any(|f| f.tag == "smcp"));
        assert_eq!(caps.axes.len(), 5);
        let json = serde_json::to_value(&caps).unwrap();
        assert!(json["features"][0]["on"].is_array() && json["axes"][0]["tag"].is_string());
        assert!(
            run_capabilities(&lib, &text("")).is_err(),
            "Bebas no está cargada"
        );
    }

    #[test]
    fn documents_without_the_new_fields_still_load_and_omit_them() {
        let json = serde_json::json!({
            "text": "A", "font_family": "Bebas Neue", "bold": false, "italic": false,
            "size_mm": 10.0, "letter_spacing_mm": 0.0, "line_spacing": 1.2,
            "align": "left", "on_path": null, "outline": [], "bend_deg": 20.0
        });
        let t: TextObject = serde_json::from_value(json).unwrap();
        assert!(t.features.is_none() && t.variations.is_none() && t.valign.is_none());
        assert!(t.uppercase.is_none() && t.word_spacing_mm.is_none());
        let back = serde_json::to_value(&t).unwrap();
        for key in [
            "features",
            "variations",
            "uppercase",
            "valign",
            "word_spacing_mm",
        ] {
            assert!(back.get(key).is_none(), "{key} no debería escribirse vacío");
        }
        // Y con valores se guardan y se leen igual.
        let mut t = t;
        t.valign = Some(TextVAlign::Top);
        t.uppercase = Some(true);
        let again: TextObject = serde_json::from_value(serde_json::to_value(&t).unwrap()).unwrap();
        assert_eq!(again, t);
    }

    #[test]
    fn list_fonts_ends_with_the_single_line_section() {
        let fonts = tauri::async_runtime::block_on(list_fonts()).unwrap();
        let first = fonts.iter().position(|f| f.single_line).unwrap();
        assert!(fonts[first..].iter().all(|f| f.single_line));
        assert!(fonts[..first].iter().all(|f| !f.single_line));
        assert!(fonts.iter().any(|f| f.family == "Hershey Roman Simplex"));
    }

    /// SHX `unifont` mínimo con una 'L'.
    fn tiny_shx() -> Vec<u8> {
        let mut shx = b"AutoCAD-86 unifont 1.0\r\n\x1a".to_vec();
        shx.extend([2, 0, 0, 0, 20, 0]);
        shx.extend(b"Prueba\0");
        shx.extend([4, 1, 0, 0, 0, 0]);
        let codes = [0x44u8, 0x4C, 0x30, 0];
        shx.extend(u16::from(b'L').to_le_bytes());
        shx.extend(((codes.len() + 2) as u16).to_le_bytes());
        shx.extend([b'x', 0]);
        shx.extend(codes);
        shx
    }

    #[test]
    fn imports_shx_fonts_and_keeps_a_copy() {
        let tmp = std::env::temp_dir().join(format!("tlaku-shx-{}", std::process::id()));
        let src = tmp.join("origen");
        let dst = tmp.join("guardadas");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("mia.shx"), tiny_shx()).unwrap();
        std::fs::write(src.join("rota.shx"), b"no es una fuente").unwrap();

        let lib = fixture_lib();
        let names = run_import_shx(
            &lib,
            &[src.join("mia.shx"), src.join("rota.shx")],
            Some(&dst),
        )
        .unwrap();
        assert_eq!(names, vec!["mia".to_string()]);
        assert!(dst.join("mia.shx").exists() && !dst.join("rota.shx").exists());

        let mut t = text("LL");
        t.font_family = "mia".into();
        let r = run_outline(&lib, &t, None).unwrap();
        assert_eq!(r.paths.len(), 2);

        // Si todos fallan, es un error con el motivo.
        let e = run_import_shx(&lib, &[src.join("rota.shx")], None).unwrap_err();
        assert!(e.contains("SHX"), "{e}");
        assert!(run_import_shx(&lib, &[], None).is_err());
        let _ = std::fs::remove_dir_all(tmp);
    }
}
