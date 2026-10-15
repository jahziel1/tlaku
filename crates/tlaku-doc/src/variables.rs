//! Datos variables (mejora 14): fuentes de datos con nombre, imágenes y códigos cuyo contenido
//! viene de una columna, y las rutas relativas al archivo del proyecto.
//!
//! La resolución de las plantillas (`{csv.Fuente:Columna}`) ocurre en la interfaz; aquí solo
//! viven los datos que se guardan en el `.tlaku`, su migración desde el CSV único antiguo
//! (`csv_path`) y el paso entre rutas absolutas (en memoria) y relativas (en el archivo).

use crate::cut::VariableText;
use crate::document::{Document, ObjectKind};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use ts_rs::TS;

/// Qué pasa cuando una fuente secundaria tiene menos filas que la principal.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ShortRows {
    /// Vuelve a empezar por la primera fila (la fila 4 de una fuente de 3 filas es la fila 1).
    #[default]
    Repeat,
    /// Se detiene: el envío se bloquea con un aviso que nombra la fuente y la fila.
    Block,
}

/// Un archivo de datos (CSV) del proyecto. La primera fuente de la lista es la principal: manda
/// el número de filas y la fila actual (`csv_row`); las demás se leen con el mismo índice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct DataSource {
    /// Nombre corto con el que las plantillas la eligen: `{csv.nombre:Columna}`. Letras, números,
    /// `_` y `-`; no se distingue mayúsculas de minúsculas.
    pub name: String,
    /// Ruta del CSV. En memoria es absoluta; en el archivo es relativa al proyecto si se puede.
    pub path: String,
    /// Qué hacer si esta fuente tiene menos filas que la principal.
    #[serde(default)]
    pub on_short: ShortRows,
}

/// Cómo se ajusta una imagen variable a la caja de la imagen marcada.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ImageFit {
    /// Entra entera dentro de la caja, centrada (puede sobrar espacio).
    #[default]
    Contain,
    /// Llena la caja y recorta lo que sobra (con una máscara).
    Cover,
}

/// Una imagen marcada como variable: al enviar se sustituye su archivo por el que dice la plantilla.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub struct VariableImage {
    /// Plantilla de la ruta del archivo, por ejemplo `{csv:Logo}`. Una ruta relativa se busca en
    /// la carpeta del CSV de donde sale la columna.
    pub source: String,
    #[serde(default)]
    pub fit: ImageFit,
}

/// Un código QR o de barras cuyo contenido puede llevar variables. Lleva los mismos ajustes que
/// `CodeSpec` de `tlaku-codes` (con `kind` y `ecc` en texto, porque este crate no depende de
/// aquél); usa los mismos nombres (`qr`, `code128`..., `l`/`m`/`q`/`h`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", default)]
#[ts(export)]
pub struct VariableCode {
    pub kind: String,
    /// Plantilla del contenido, por ejemplo `https://tienda.example/{csv:Id}`.
    pub data: String,
    pub ecc: String,
    pub size_mm: f64,
    pub height_mm: f64,
    pub margin_mm: f64,
    pub human_readable: bool,
    pub invert: bool,
    pub font_family: String,
}

impl Default for VariableCode {
    fn default() -> Self {
        Self {
            kind: "qr".into(),
            data: String::new(),
            ecc: "m".into(),
            size_mm: 30.0,
            height_mm: 15.0,
            margin_mm: 2.0,
            human_readable: false,
            invert: false,
            font_family: String::new(),
        }
    }
}

/// Nombre de fuente válido: letras, números, `_` y `-`.
pub fn clean_source_name(raw: &str) -> String {
    raw.trim()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect()
}

impl VariableText {
    /// Deja las fuentes en forma canónica y migra el CSV único antiguo:
    /// - sin `sources` y con `csv_path`, la fuente principal sale del archivo (nombre = el del archivo);
    /// - con `sources`, `csv_path` repite la ruta de la principal (para versiones anteriores);
    /// - los nombres quedan válidos, sin repetir (sin distinguir mayúsculas) y sin vacíos.
    pub fn normalize(&mut self) {
        if self.sources.is_empty()
            && let Some(path) = self.csv_path.clone().filter(|p| !p.trim().is_empty())
        {
            let stem = Path::new(&path)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.sources.push(DataSource {
                name: stem,
                path,
                on_short: ShortRows::Repeat,
            });
        }
        let mut seen: Vec<String> = Vec::new();
        for (i, s) in self.sources.iter_mut().enumerate() {
            let mut name = clean_source_name(&s.name);
            if name.is_empty() {
                name = if i == 0 {
                    "datos".into()
                } else {
                    format!("datos{}", i + 1)
                };
            }
            let base = name.clone();
            let mut n = 2;
            while seen.contains(&name.to_lowercase()) {
                name = format!("{base}{n}");
                n += 1;
            }
            seen.push(name.to_lowercase());
            s.name = name;
        }
        self.sync_legacy_path();
    }

    /// `csv_path` repite la ruta de la fuente principal (lo lee una versión anterior de la app).
    fn sync_legacy_path(&mut self) {
        self.csv_path = self
            .sources
            .first()
            .map(|s| s.path.clone())
            .filter(|p| !p.trim().is_empty());
    }
}

/// Resuelve `.` y `..` sin tocar el disco.
fn lexical_clean(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Cuántos niveles hacia arriba se permiten en una ruta relativa: más allá se prefiere la ruta
/// absoluta (un `../../../../x` ya no es "junto al proyecto").
const MAX_UP_LEVELS: usize = 3;

/// Ruta de `path` relativa a la carpeta `base`, con `/` como separador, si el archivo queda en
/// esa carpeta o cerca (hasta [`MAX_UP_LEVELS`] niveles arriba). Si no, o si `path` ya es relativa,
/// la devuelve igual.
pub fn relativize(path: &str, base: &Path) -> String {
    let p = Path::new(path);
    if path.trim().is_empty() || !p.is_absolute() || !base.is_absolute() {
        return path.to_string();
    }
    let a_clean = lexical_clean(p);
    let b_clean = lexical_clean(base);
    let a: Vec<Component> = a_clean.components().collect();
    let b: Vec<Component> = b_clean.components().collect();
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    // Sin ancestro común más allá de la raíz (o de la unidad en Windows), no hay relación.
    let root_len = a
        .iter()
        .take_while(|c| matches!(c, Component::Prefix(_) | Component::RootDir))
        .count();
    if common <= root_len || common == a.len() {
        return path.to_string();
    }
    let ups = b.len() - common;
    if ups > MAX_UP_LEVELS {
        return path.to_string();
    }
    let mut parts: Vec<String> = vec!["..".into(); ups];
    parts.extend(
        a[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    parts.join("/")
}

/// Ruta absoluta de `path` partiendo de la carpeta `base` (si `path` es relativa).
pub fn absolutize(path: &str, base: &Path) -> String {
    let p = Path::new(path);
    if path.trim().is_empty() || p.is_absolute() || !base.is_absolute() {
        return path.to_string();
    }
    lexical_clean(&base.join(p)).to_string_lossy().into_owned()
}

impl Document {
    /// Ajusta las rutas de datos al leer: las relativas se vuelven absolutas respecto de la
    /// carpeta del proyecto `dir`, y las fuentes se normalizan.
    pub fn absolutize_data_paths(&mut self, dir: &Path) {
        let vt = &mut self.variable_text;
        vt.normalize();
        for s in &mut vt.sources {
            s.path = absolutize(&s.path, dir);
        }
        vt.sync_legacy_path();
    }

    /// Ajusta las rutas de datos al guardar en la carpeta `dir`: las absolutas cercanas pasan a relativas.
    pub fn relativize_data_paths(&mut self, dir: &Path) {
        let vt = &mut self.variable_text;
        vt.normalize();
        for s in &mut vt.sources {
            s.path = relativize(&s.path, dir);
        }
        vt.sync_legacy_path();
    }

    /// ¿Algún objeto es una imagen o un código variable?
    pub fn has_variable_objects(&self) -> bool {
        self.objects.iter().any(|o| match &o.kind {
            ObjectKind::Image(i) => i.variable.is_some(),
            ObjectKind::Vector(v) => v.code.is_some(),
            ObjectKind::Text(_) => false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ruta absoluta del sistema para las pruebas: en Windows le falta la unidad a `/home/...`.
    fn abs(p: &str) -> String {
        if cfg!(windows) {
            format!("C:{}", p.replace('/', "\\"))
        } else {
            p.to_string()
        }
    }

    #[test]
    fn old_document_with_single_csv_migrates_to_a_source() {
        let json = r#"{"version":2,"objects":[],"layers":[],"device_id":null,
            "variable_text":{"serial":5,"csv_path":"/datos/Etiquetas 2026.csv","csv_row":3}}"#;
        let doc = Document::from_json(json).unwrap();
        let vt = &doc.variable_text;
        assert_eq!(vt.sources.len(), 1);
        assert_eq!(vt.sources[0].name, "Etiquetas2026");
        assert_eq!(vt.sources[0].path, "/datos/Etiquetas 2026.csv");
        assert_eq!(vt.sources[0].on_short, ShortRows::Repeat);
        assert_eq!(vt.csv_path.as_deref(), Some("/datos/Etiquetas 2026.csv"));
        assert_eq!(vt.csv_row, 3);
        assert_eq!(vt.serial, 5);
    }

    #[test]
    fn document_without_variable_text_has_no_sources() {
        let json = r#"{"version":2,"objects":[],"layers":[],"device_id":null}"#;
        let doc = Document::from_json(json).unwrap();
        assert!(doc.variable_text.sources.is_empty());
        assert!(doc.variable_text.csv_path.is_none());
    }

    #[test]
    fn names_are_cleaned_and_unique_and_legacy_path_mirrors_the_main_source() {
        let mut vt = VariableText {
            sources: vec![
                DataSource {
                    name: " Lista A! ".into(),
                    path: "/a.csv".into(),
                    on_short: ShortRows::Repeat,
                },
                DataSource {
                    name: "listaa".into(),
                    path: "/b.csv".into(),
                    on_short: ShortRows::Block,
                },
                DataSource {
                    name: "?".into(),
                    path: "/c.csv".into(),
                    on_short: ShortRows::Repeat,
                },
            ],
            csv_path: Some("/viejo.csv".into()),
            ..Default::default()
        };
        vt.normalize();
        let names: Vec<&str> = vt.sources.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["ListaA", "listaa2", "datos3"]);
        assert_eq!(vt.csv_path.as_deref(), Some("/a.csv"));
        // Idempotente.
        let before = vt.clone();
        vt.normalize();
        assert_eq!(vt, before);
    }

    #[test]
    fn sources_roundtrip_through_json() {
        let mut doc = Document::default();
        doc.variable_text.sources = vec![DataSource {
            name: "productos".into(),
            path: "/p.csv".into(),
            on_short: ShortRows::Block,
        }];
        let back = Document::from_json(&doc.to_json().unwrap()).unwrap();
        assert_eq!(back.variable_text.sources, doc.variable_text.sources);
    }

    #[test]
    fn relative_paths_when_close_and_absolute_when_far() {
        let base = &PathBuf::from(abs("/home/ana/proyectos/etiquetas"));
        assert_eq!(
            relativize(&abs("/home/ana/proyectos/etiquetas/datos/a.csv"), base),
            "datos/a.csv"
        );
        assert_eq!(
            relativize(&abs("/home/ana/proyectos/otros/b.csv"), base),
            "../otros/b.csv"
        );
        // Demasiado lejos o sin ancestro común: se queda absoluta.
        assert_eq!(relativize(&abs("/mnt/x/c.csv"), base), abs("/mnt/x/c.csv"));
        assert_eq!(relativize(&abs("/c.csv"), base), abs("/c.csv"));
        // Ya relativa: no se toca.
        assert_eq!(relativize("datos/a.csv", base), "datos/a.csv");
    }

    #[test]
    fn absolutize_undoes_relativize() {
        let base = &PathBuf::from(abs("/home/ana/proyectos/etiquetas"));
        for p in [
            abs("/home/ana/proyectos/etiquetas/datos/a.csv"),
            abs("/home/ana/proyectos/otros/b.csv"),
            abs("/mnt/x/c.csv"),
        ] {
            let rel = relativize(&p, base);
            assert_eq!(absolutize(&rel, base), p, "{rel}");
        }
        assert_eq!(absolutize("", base), "");
    }

    #[test]
    fn save_writes_relative_and_load_restores_absolute() {
        let dir = &PathBuf::from(abs("/home/ana/trabajo"));
        let mut doc = Document::default();
        doc.variable_text.sources = vec![
            DataSource {
                name: "a".into(),
                path: abs("/home/ana/trabajo/datos/a.csv"),
                on_short: ShortRows::Repeat,
            },
            DataSource {
                name: "b".into(),
                path: abs("/mnt/lejos/b.csv"),
                on_short: ShortRows::Repeat,
            },
        ];
        let mut saved = doc.clone();
        saved.relativize_data_paths(dir);
        assert_eq!(saved.variable_text.sources[0].path, "datos/a.csv");
        assert_eq!(saved.variable_text.csv_path.as_deref(), Some("datos/a.csv"));
        assert_eq!(saved.variable_text.sources[1].path, abs("/mnt/lejos/b.csv"));
        // El proyecto se mueve de carpeta: las relativas siguen a la carpeta nueva.
        let mut loaded = Document::from_json(&saved.to_json().unwrap()).unwrap();
        loaded.absolutize_data_paths(&PathBuf::from(abs("/media/usb/trabajo")));
        assert_eq!(
            loaded.variable_text.sources[0].path,
            abs("/media/usb/trabajo/datos/a.csv")
        );
        assert_eq!(
            loaded.variable_text.sources[1].path,
            abs("/mnt/lejos/b.csv")
        );
    }

    #[test]
    fn variable_image_and_code_roundtrip_and_old_objects_have_none() {
        let obj = serde_json::json!({
            "id": "a", "layer": "#000000", "transform": [1,0,0,1,0,0],
            "kind": {"type":"image","source_path":"x.png","width_px":2,"height_px":2,
                     "size_mm":[10.0,10.0],
                     "variable":{"source":"{csv:Logo}","fit":"cover"}},
            "locked": false, "hidden": false, "group": null
        });
        let o: crate::DesignObject = serde_json::from_value(obj).unwrap();
        match &o.kind {
            ObjectKind::Image(i) => {
                let v = i.variable.as_ref().unwrap();
                assert_eq!(v.source, "{csv:Logo}");
                assert_eq!(v.fit, ImageFit::Cover);
            }
            _ => panic!("debía ser imagen"),
        }
        let old = serde_json::json!({
            "id": "a", "layer": "#000000", "transform": [1,0,0,1,0,0],
            "kind": {"type":"image","source_path":"x.png","width_px":2,"height_px":2,"size_mm":[1.0,1.0]},
            "locked": false, "hidden": false, "group": null
        });
        let o: crate::DesignObject = serde_json::from_value(old).unwrap();
        assert!(matches!(&o.kind, ObjectKind::Image(i) if i.variable.is_none()));
        let code = serde_json::json!({"data": "{csv:Id}"});
        let c: VariableCode = serde_json::from_value(code).unwrap();
        assert_eq!(c.kind, "qr");
        assert_eq!(c.data, "{csv:Id}");
    }
}
