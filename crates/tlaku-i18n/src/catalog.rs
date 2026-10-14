//! Catálogos incluidos al compilar y su lectura.
//!
//! Formato de `locales/<idioma>/<crate>.txt`:
//!
//! ```text
//! # comentario
//! pipeline.import.no_paths = El archivo no contiene trayectorias
//! pipeline.import.long = Un texto largo puede seguir
//!     en las líneas siguientes, con sangría; se unen con un espacio.
//! ```
//!
//! `\n` es un salto de línea y `\\` una barra. `{{` y `}}` son llaves literales. Un archivo nuevo
//! se registra en [`CATALOGS`].

use crate::locale::Locale;
use std::collections::HashMap;
use std::sync::OnceLock;

/// `(crate de origen, español, inglés)`. Un archivo por crate: `pipeline` es `tlaku-pipeline`,
/// `desktop` es `apps/desktop/src-tauri`. Las claves del archivo empiezan con ese nombre.
pub const CATALOGS: &[(&str, &str, &str)] = &[
    (
        "bench",
        include_str!("../locales/es/bench.txt"),
        include_str!("../locales/en/bench.txt"),
    ),
    (
        "camera",
        include_str!("../locales/es/camera.txt"),
        include_str!("../locales/en/camera.txt"),
    ),
    (
        "cli",
        include_str!("../locales/es/cli.txt"),
        include_str!("../locales/en/cli.txt"),
    ),
    (
        "codes",
        include_str!("../locales/es/codes.txt"),
        include_str!("../locales/en/codes.txt"),
    ),
    (
        "core",
        include_str!("../locales/es/core.txt"),
        include_str!("../locales/en/core.txt"),
    ),
    (
        "desktop",
        include_str!("../locales/es/desktop.txt"),
        include_str!("../locales/en/desktop.txt"),
    ),
    (
        "doc",
        include_str!("../locales/es/doc.txt"),
        include_str!("../locales/en/doc.txt"),
    ),
    (
        "dxf",
        include_str!("../locales/es/dxf.txt"),
        include_str!("../locales/en/dxf.txt"),
    ),
    (
        "emu",
        include_str!("../locales/es/emu.txt"),
        include_str!("../locales/en/emu.txt"),
    ),
    (
        "export",
        include_str!("../locales/es/export.txt"),
        include_str!("../locales/en/export.txt"),
    ),
    (
        "fill",
        include_str!("../locales/es/fill.txt"),
        include_str!("../locales/en/fill.txt"),
    ),
    (
        "galvo",
        include_str!("../locales/es/galvo.txt"),
        include_str!("../locales/en/galvo.txt"),
    ),
    (
        "geom",
        include_str!("../locales/es/geom.txt"),
        include_str!("../locales/en/geom.txt"),
    ),
    (
        "grbl",
        include_str!("../locales/es/grbl.txt"),
        include_str!("../locales/en/grbl.txt"),
    ),
    (
        "lbrn",
        include_str!("../locales/es/lbrn.txt"),
        include_str!("../locales/en/lbrn.txt"),
    ),
    (
        "lhymicro",
        include_str!("../locales/es/lhymicro.txt"),
        include_str!("../locales/en/lhymicro.txt"),
    ),
    (
        "nest",
        include_str!("../locales/es/nest.txt"),
        include_str!("../locales/en/nest.txt"),
    ),
    (
        "pipeline",
        include_str!("../locales/es/pipeline.txt"),
        include_str!("../locales/en/pipeline.txt"),
    ),
    (
        "plan",
        include_str!("../locales/es/plan.txt"),
        include_str!("../locales/en/plan.txt"),
    ),
    (
        "raster",
        include_str!("../locales/es/raster.txt"),
        include_str!("../locales/en/raster.txt"),
    ),
    (
        "ruida",
        include_str!("../locales/es/ruida.txt"),
        include_str!("../locales/en/ruida.txt"),
    ),
    (
        "svg",
        include_str!("../locales/es/svg.txt"),
        include_str!("../locales/en/svg.txt"),
    ),
    (
        "text",
        include_str!("../locales/es/text.txt"),
        include_str!("../locales/en/text.txt"),
    ),
    (
        "trace",
        include_str!("../locales/es/trace.txt"),
        include_str!("../locales/en/trace.txt"),
    ),
];

/// Una línea `clave = plantilla` ya leída.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub text: String,
    /// Línea (desde 1) donde empieza.
    pub line: usize,
}

/// Lee un archivo de catálogo. Devuelve las entradas y los problemas de formato encontrados
/// (líneas sin `=`, claves vacías); las pruebas exigen que no haya ninguno.
pub fn parse(src: &str) -> (Vec<Entry>, Vec<String>) {
    let mut entries: Vec<Entry> = Vec::new();
    let mut problems = Vec::new();
    for (i, raw) in src.lines().enumerate() {
        let n = i + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if raw.starts_with([' ', '\t']) {
            match entries.last_mut() {
                Some(last) => {
                    if !last.text.is_empty() {
                        last.text.push(' ');
                    }
                    last.text.push_str(&unescape(trimmed));
                }
                None => problems.push(format!("línea {n}: continuación sin entrada")),
            }
            continue;
        }
        match trimmed.split_once('=') {
            Some((key, text)) if !key.trim().is_empty() => entries.push(Entry {
                key: key.trim().to_string(),
                text: unescape(text.trim()),
                line: n,
            }),
            _ => problems.push(format!("línea {n}: se esperaba `clave = texto`")),
        }
    }
    (entries, problems)
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => out.push('\n'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Fuente de un archivo de catálogo de un crate en un idioma.
pub fn source(l: Locale, krate: &str) -> Option<&'static str> {
    CATALOGS
        .iter()
        .find(|(c, ..)| *c == krate)
        .map(|(_, es, en)| match l {
            Locale::Es => *es,
            Locale::En => *en,
        })
}

fn map(l: Locale) -> &'static HashMap<String, String> {
    static ES: OnceLock<HashMap<String, String>> = OnceLock::new();
    static EN: OnceLock<HashMap<String, String>> = OnceLock::new();
    let cell = match l {
        Locale::Es => &ES,
        Locale::En => &EN,
    };
    cell.get_or_init(|| {
        let mut m = HashMap::new();
        for (krate, ..) in CATALOGS {
            let (entries, _) = parse(source(l, krate).unwrap_or(""));
            for e in entries {
                // Con una clave repetida gana la primera; la prueba de duplicados la delata.
                m.entry(e.key).or_insert(e.text);
            }
        }
        m
    })
}

/// Plantilla de `key` en el idioma `l`, si existe.
pub fn lookup(l: Locale, key: &str) -> Option<&'static str> {
    map(l).get(key).map(String::as_str)
}
