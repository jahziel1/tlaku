use crate::catalog;
use crate::locale::{Locale, locale};
use std::collections::BTreeSet;
use std::fmt::Display;
use std::sync::Mutex;

/// Valor ya convertido a texto de un marcador (lo arma [`crate::tr!`]).
pub struct Arg(String);

impl Arg {
    pub fn new(v: &dyn Display) -> Arg {
        Arg(v.to_string())
    }
}

/// Claves que no estaban en el idioma pedido (o en ninguno). Tope para no crecer sin fin.
static FALLBACKS: Mutex<Vec<String>> = Mutex::new(Vec::new());
const MAX_FALLBACKS: usize = 256;

fn note_fallback(entry: String) {
    if let Ok(mut v) = FALLBACKS.lock()
        && v.len() < MAX_FALLBACKS
        && !v.contains(&entry)
    {
        v.push(entry);
    }
}

/// Vacía y devuelve las claves que cayeron en el respaldo: `"en:clave"` si faltaba en inglés y se
/// usó el español, y `"?:clave"` si no existía en ninguno. Lo usan las pruebas.
pub fn take_fallbacks() -> Vec<String> {
    FALLBACKS
        .lock()
        .map(|mut v| std::mem::take(&mut *v))
        .unwrap_or_default()
}

/// Traduce `key` al idioma activo. Es lo que expande [`crate::tr!`].
pub fn translate(key: &str, args: &[(&str, Arg)]) -> String {
    translate_in(locale(), key, args)
}

/// Como [`translate`], con idioma explícito.
pub fn translate_in(l: Locale, key: &str, args: &[(&str, Arg)]) -> String {
    let template = match catalog::lookup(l, key) {
        Some(t) => t,
        None => match catalog::lookup(l.other(), key) {
            Some(t) => {
                note_fallback(format!("{}:{key}", l.code()));
                t
            }
            None => {
                note_fallback(format!("?:{key}"));
                return key.to_string();
            }
        },
    };
    render(template, args)
}

/// Reemplaza `{nombre}` por su valor; `{{` y `}}` son llaves literales. Un marcador sin valor se
/// deja tal cual.
fn render(template: &str, args: &[(&str, Arg)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(i) = rest.find(['{', '}']) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if tail.starts_with("{{") || tail.starts_with("}}") {
            out.push_str(&tail[..1]);
            rest = &tail[2..];
        } else if let Some((name, after)) = marker(tail) {
            match args.iter().find(|(n, _)| *n == name) {
                Some((_, v)) => out.push_str(&v.0),
                None => out.push_str(&tail[..tail.len() - after.len()]),
            }
            rest = after;
        } else {
            out.push_str(&tail[..1]);
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

/// Si `s` empieza con `{nombre}`, devuelve el nombre y lo que sigue.
fn marker(s: &str) -> Option<(&str, &str)> {
    let inner = s.strip_prefix('{')?;
    let end = inner.find('}')?;
    let name = &inner[..end];
    let ok = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    ok.then(|| (name, &inner[end + 1..]))
}

/// Nombres de los marcadores de una plantilla (para las pruebas de coincidencia entre idiomas).
pub fn placeholders(template: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    let mut rest = template;
    while let Some(i) = rest.find(['{', '}']) {
        let tail = &rest[i..];
        if tail.starts_with("{{") || tail.starts_with("}}") {
            rest = &tail[2..];
        } else if let Some((name, after)) = marker(tail) {
            set.insert(name.to_string());
            rest = after;
        } else {
            rest = &tail[1..];
        }
    }
    set
}

/// `true` si `message` empieza como la plantilla de `key` en cualquiera de los dos idiomas (el
/// texto fijo antes del primer marcador). Sirve para reconocer un aviso que produjo otro crate sin
/// depender del idioma. Prefiere un código ([`coded`]) cuando la interfaz también debe reconocerlo.
pub fn msg_is(message: &str, key: &str) -> bool {
    // Un mensaje con código (`[codigo] texto`) se compara por su texto.
    let message = split_code(message).1;
    Locale::ALL.iter().any(|l| {
        catalog::lookup(*l, key).is_some_and(|t| {
            let fixed = &t[..t.find(['{', '}']).unwrap_or(t.len())];
            !fixed.is_empty() && message.starts_with(fixed)
        })
    })
}

/// Como [`msg_is`], pero más estricto: todo el texto fijo de la plantilla de `key` (en cualquiera de
/// los dos idiomas) debe aparecer en `message`, completo y en orden; los marcadores valen cualquier
/// cosa. Si la plantilla empieza con texto fijo, el mensaje empieza con él. Detrás puede venir más
/// texto. Sirve para reconocer un aviso ajeno cuya plantilla empieza con un marcador (`{n} imagen(es)
/// ...`) o comparte el comienzo con otros (`Se omitieron {n} entidades de texto...`).
pub fn msg_matches(message: &str, key: &str) -> bool {
    Locale::ALL.iter().any(|l| {
        catalog::lookup(*l, key).is_some_and(|t| {
            let mut rest = message;
            let mut first = true;
            let mut any_fixed = false;
            for piece in fixed_pieces(t) {
                if piece.is_empty() {
                    first = false;
                    continue;
                }
                any_fixed = true;
                match rest.find(&piece) {
                    Some(0) => rest = &rest[piece.len()..],
                    Some(i) if !first => rest = &rest[i + piece.len()..],
                    _ => return false,
                }
                first = false;
            }
            any_fixed
        })
    })
}

/// Trozos de texto fijo de una plantilla, separados por sus marcadores (`{{`/`}}` ya son llaves).
fn fixed_pieces(template: &str) -> Vec<String> {
    let mut pieces = vec![String::new()];
    let mut rest = template;
    while let Some(i) = rest.find(['{', '}']) {
        pieces.last_mut().unwrap().push_str(&rest[..i]);
        let tail = &rest[i..];
        if tail.starts_with("{{") || tail.starts_with("}}") {
            pieces.last_mut().unwrap().push_str(&tail[..1]);
            rest = &tail[2..];
        } else if let Some((_, after)) = marker(tail) {
            pieces.push(String::new());
            rest = after;
        } else {
            pieces.last_mut().unwrap().push_str(&tail[..1]);
            rest = &tail[1..];
        }
    }
    pieces.last_mut().unwrap().push_str(rest);
    pieces
}

/// Antepone un código estable a un mensaje: `[gcode_zero_power] texto`. La interfaz reconoce el
/// error por el código y quita el prefijo antes de mostrarlo.
pub fn coded(code: &str, message: impl Display) -> String {
    format!("[{code}] {message}")
}

/// Separa el código de un mensaje de [`coded`]: `("gcode_zero_power", "texto")`.
pub fn split_code(message: &str) -> (Option<&str>, &str) {
    if let Some(rest) = message.strip_prefix('[')
        && let Some(end) = rest.find(']')
    {
        let code = &rest[..end];
        let valid = !code.is_empty()
            && code
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if valid {
            return (Some(code), rest[end + 1..].trim_start());
        }
    }
    (None, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(v: &dyn Display) -> Arg {
        Arg::new(v)
    }

    #[test]
    fn replaces_named_markers() {
        let out = render(
            "{capa}: máx {max} mm",
            &[("capa", a(&"C01")), ("max", a(&4.5))],
        );
        assert_eq!(out, "C01: máx 4.5 mm");
    }

    #[test]
    fn keeps_unknown_markers_and_literal_braces() {
        assert_eq!(render("x {y} {{z}}", &[]), "x {y} {z}");
        assert_eq!(render("a { b } c}", &[]), "a { b } c}");
        assert_eq!(render("{n}{n}", &[("n", a(&1))]), "11");
    }

    #[test]
    fn placeholders_ignore_escaped_braces() {
        let set = placeholders("{a} y {{b}} y {c}");
        assert_eq!(set.into_iter().collect::<Vec<_>>(), vec!["a", "c"]);
    }

    #[test]
    fn unicode_text_is_untouched() {
        assert_eq!(
            render("¿Qué {x}? ñandú", &[("x", a(&"é"))]),
            "¿Qué é? ñandú"
        );
    }
}
