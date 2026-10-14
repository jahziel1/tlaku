//! Pruebas del catálogo: completitud, marcadores, duplicados y uso en el código del workspace.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use tlaku_i18n::catalog::{self, CATALOGS, Entry};
use tlaku_i18n::{Locale, placeholders};

fn parsed(l: Locale, krate: &str) -> Vec<Entry> {
    let (entries, problems) = catalog::parse(catalog::source(l, krate).unwrap());
    assert!(
        problems.is_empty(),
        "locales/{}/{krate}.txt: {problems:?}",
        l.code()
    );
    entries
}

/// Todas las entradas: `(crate, clave) -> plantilla`, por idioma.
fn all(l: Locale) -> BTreeMap<String, (String, String)> {
    let mut m = BTreeMap::new();
    for (krate, ..) in CATALOGS {
        for e in parsed(l, krate) {
            m.insert(e.key, (krate.to_string(), e.text));
        }
    }
    m
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn keys_are_well_formed_and_belong_to_their_crate() {
    for l in Locale::ALL {
        for (krate, ..) in CATALOGS {
            for e in parsed(l, krate) {
                let parts: Vec<&str> = e.key.split('.').collect();
                let ok = parts.len() >= 3
                    && parts.iter().all(|p| {
                        !p.is_empty()
                            && p.chars()
                                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                    });
                assert!(
                    ok,
                    "{}: clave mal formada «{}» (usa crate.módulo.qué)",
                    krate, e.key
                );
                assert_eq!(
                    parts[0],
                    *krate,
                    "{}/{}: la clave «{}» debe empezar con «{krate}.»",
                    l.code(),
                    krate,
                    e.key
                );
                assert!(!e.text.trim().is_empty(), "{}: texto vacío", e.key);
            }
        }
    }
}

#[test]
fn no_duplicate_keys() {
    for l in Locale::ALL {
        let mut seen = BTreeSet::new();
        for (krate, ..) in CATALOGS {
            for e in parsed(l, krate) {
                assert!(
                    seen.insert(e.key.clone()),
                    "clave repetida en {}: {} (línea {} de {krate}.txt)",
                    l.code(),
                    e.key,
                    e.line
                );
            }
        }
    }
}

#[test]
fn every_key_exists_in_both_languages() {
    let es = all(Locale::Es);
    let en = all(Locale::En);
    let missing_en: Vec<_> = es.keys().filter(|k| !en.contains_key(*k)).collect();
    let missing_es: Vec<_> = en.keys().filter(|k| !es.contains_key(*k)).collect();
    assert!(missing_en.is_empty(), "faltan en inglés: {missing_en:?}");
    assert!(missing_es.is_empty(), "faltan en español: {missing_es:?}");
}

#[test]
fn placeholders_match_between_languages() {
    let es = all(Locale::Es);
    let en = all(Locale::En);
    for (key, (_, text_es)) in &es {
        if let Some((_, text_en)) = en.get(key) {
            assert_eq!(
                placeholders(text_es),
                placeholders(text_en),
                "{key}: los marcadores {{..}} difieren entre es y en"
            );
        }
    }
}

/// Claves cuyo inglés puede ser idéntico al español (nombres propios, códigos). Una por línea en
/// `locales/identical_ok.txt`.
fn identical_ok() -> BTreeSet<String> {
    include_str!("../locales/identical_ok.txt")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

#[test]
fn english_differs_from_spanish() {
    let es = all(Locale::Es);
    let en = all(Locale::En);
    let ok = identical_ok();
    let known: BTreeSet<_> = es.keys().cloned().collect();
    for k in &ok {
        assert!(
            known.contains(k),
            "identical_ok.txt: la clave «{k}» no existe"
        );
    }
    let same: Vec<_> = es
        .iter()
        .filter(|(k, (_, t))| en.get(*k).is_some_and(|(_, e)| e == t) && !ok.contains(*k))
        .map(|(k, _)| k)
        .collect();
    assert!(
        same.is_empty(),
        "el inglés es igual al español (tradúcelo o agrégalo a locales/identical_ok.txt): {same:?}"
    );
}

/// Archivos `.rs` del workspace donde se usa `tr!` (sin `target/`, `node_modules/` ni este crate).
fn rust_sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in rd.flatten() {
            let p = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if p.is_dir() {
                if matches!(
                    name.as_str(),
                    "target" | "node_modules" | ".git" | "tlaku-i18n"
                ) {
                    continue;
                }
                walk(&p, out);
            } else if name.ends_with(".rs") {
                out.push(p);
            }
        }
    }
    let root = workspace_root();
    let mut out = Vec::new();
    walk(&root.join("crates"), &mut out);
    walk(&root.join("apps/desktop/src-tauri/src"), &mut out);
    out
}

/// Claves de cada `tr!("clave"...)` de un texto fuente.
fn used_keys(src: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let bytes = src.as_bytes();
    let mut from = 0;
    while let Some(i) = src[from..].find("tr!(") {
        let at = from + i;
        from = at + 4;
        // `str!(`, `my_tr!(`: no es nuestra macro.
        if at > 0 {
            let prev = bytes[at - 1];
            if prev.is_ascii_alphanumeric() || prev == b'_' {
                continue;
            }
        }
        let rest = src[from..].trim_start();
        if let Some(q) = rest.strip_prefix('"')
            && let Some(end) = q.find('"')
        {
            keys.push(q[..end].to_string());
        }
    }
    keys
}

#[test]
fn every_key_used_in_code_exists_in_the_catalog() {
    let es = all(Locale::Es);
    let en = all(Locale::En);
    let mut missing = Vec::new();
    let mut count = 0;
    for file in rust_sources() {
        let Ok(src) = std::fs::read_to_string(&file) else {
            continue;
        };
        for key in used_keys(&src) {
            count += 1;
            if !es.contains_key(&key) || !en.contains_key(&key) {
                missing.push(format!("{} -> {key}", file.display()));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "claves de tr! que no están en el catálogo:\n{}",
        missing.join("\n")
    );
    // Si el escaneo dejara de encontrar usos, esta prueba pasaría en falso.
    if !es.is_empty() {
        assert!(
            count > 0,
            "el escaneo no encontró ningún tr!(\"...\") en el workspace"
        );
    }
}

#[test]
fn every_catalog_key_is_referenced_in_code() {
    let es = all(Locale::Es);
    let mut corpus = String::new();
    for file in rust_sources() {
        if let Ok(src) = std::fs::read_to_string(&file) {
            corpus.push_str(&src);
        }
    }
    let unused: Vec<_> = es
        .keys()
        .filter(|k| !corpus.contains(&format!("\"{k}\"")))
        .collect();
    assert!(
        unused.is_empty(),
        "claves del catálogo que ningún código usa: {unused:?}"
    );
}

#[test]
fn scanner_reads_multiline_and_ignores_lookalikes() {
    let src = "let a = tr!(\"a.b.c\");\nlet b = tr!(\n    \"d.e.f\",\n    x = 1\n);\nlet c = str!(\"no\");\nlet d = my_tr!(\"no\");";
    assert_eq!(used_keys(src), vec!["a.b.c", "d.e.f"]);
}
