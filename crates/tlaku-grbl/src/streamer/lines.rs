//! Preparación de líneas de G-code y su validación antes de enviarlas.

use std::fmt;
use tlaku_i18n::tr;

// ---------------------------------------------------------------------------
// Preparación de líneas
// ---------------------------------------------------------------------------

/// Quita comentarios (`;...` y `(...)`) y espacios. Devuelve cadena vacía si no
/// queda nada. Falla con el carácter ofensor si queda uno que GRBL interpretaría
/// como comando en tiempo real (`?`, `!`, `~`, no ASCII) o de control.
pub fn prepare_line(raw: &str) -> Result<String, char> {
    prepare_line_with(raw, true, true)
}

/// Como [`prepare_line`] para cualquier familia: `strip_spaces = false` conserva un espacio
/// entre palabras (Marlin y Smoothie necesitan `M3 S0 I`, no `M3S0I`) y `reject_realtime =
/// false` admite `?`, `!` y `~` (en Marlin no son comandos en tiempo real). Los caracteres no
/// ASCII y de control siempre se rechazan.
pub fn prepare_line_with(
    raw: &str,
    strip_spaces: bool,
    reject_realtime: bool,
) -> Result<String, char> {
    let mut out = String::with_capacity(raw.len());
    let mut in_paren = false;
    for c in raw.chars() {
        match c {
            ';' if !in_paren => break,
            '(' => in_paren = true,
            ')' => in_paren = false,
            _ if in_paren => {}
            _ if c.is_whitespace() => {
                if !strip_spaces && !out.is_empty() && !out.ends_with(' ') {
                    out.push(' ');
                }
            }
            '?' | '!' | '~' if reject_realtime => return Err(c),
            _ if !c.is_ascii() || c.is_control() => return Err(c),
            _ => out.push(c),
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamError {
    /// La línea (con su `\n`) no cabe en el búfer de recepción.
    LineTooLong {
        source_line: usize,
        len: usize,
        max: usize,
    },
    InvalidChar {
        source_line: usize,
        ch: char,
    },
}

impl fmt::Display for StreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LineTooLong {
                source_line,
                len,
                max,
            } => f.write_str(&tr!(
                "grbl.stream.line_too_long",
                line = source_line,
                len = len,
                max = max
            )),
            Self::InvalidChar { source_line, ch } => f.write_str(&tr!(
                "grbl.stream.invalid_char",
                line = source_line,
                ch = format!("{ch:?}")
            )),
        }
    }
}

impl std::error::Error for StreamError {}

/// ¿La línea (ya sin espacios ni comentarios) escribe en la EEPROM de la controladora? Son
/// los ajustes `$x=valor` (incluidos `$Nx=`, `$I=` y `$RST=`) y los G-codes `G10`, `G28.1` y
/// `G30.1`. Durante la escritura GRBL deshabilita la interrupción de recepción serie (hasta
/// unos 20 ms) y puede perder bytes, así que estas líneas no se envían con conteo de
/// caracteres: se mandan cuando no hay nada pendiente y no se envía nada más hasta su `ok`.
pub fn writes_eeprom(text: &str) -> bool {
    let t = text.trim();
    if t.starts_with('$') {
        // `$J=` es un jog, no un ajuste.
        return t.contains('=') && !t.get(..3).is_some_and(|p| p.eq_ignore_ascii_case("$J="));
    }
    let b = t.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i].eq_ignore_ascii_case(&b'G') {
            let start = i + 1;
            let mut end = start;
            while end < b.len() && (b[end].is_ascii_digit() || b[end] == b'.') {
                end += 1;
            }
            if let Ok(v) = t[start..end].parse::<f64>()
                && (v == 10.0 || v == 28.1 || v == 30.1)
            {
                return true;
            }
            i = end.max(i + 1);
        } else {
            i += 1;
        }
    }
    false
}

/// ¿Es un comando de homing (`$H`, `$HX`...)? Puede tardar un minuto o más sin contestar `?`.
pub fn is_homing_line(text: &str) -> bool {
    text.trim()
        .get(..2)
        .is_some_and(|p| p.eq_ignore_ascii_case("$H"))
}
