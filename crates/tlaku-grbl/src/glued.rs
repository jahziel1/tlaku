//! Respuestas pegadas en una sola línea (solo familia GRBL).
//!
//! En una Creality Falcon2 Pro real (GRBL 1.1f CV50-Pro) llegó, 1 vez en ~600 líneas,
//! `<Run|MPos:0.287,0.112,0.000|Bf:28,65427|FS:10530,0|Ov:100,100,10ok`: un reporte de estado
//! cortado (faltan los últimos caracteres y el salto de línea) pegado al `ok` de una línea del
//! trabajo. Leída como una sola línea, el `ok` se perdía y el streamer quedaba esperando una
//! respuesta que nunca llegaría. [`split_glued`] separa esos casos para que cada pieza se trate
//! como lo que es.
//!
//! Solo se separa cuando la forma es inequívoca (nunca por simple sufijo `ok`):
//!
//! - `…ok` / `…error:N` detrás de un reporte completo (`<…>`) o de un mensaje (`[…]`);
//! - `…ok` / `…error:N` detrás de un reporte cortado (empieza con `<` y no tiene `>`): el
//!   fragmento se devuelve aparte y **no** es un reporte válido (`StatusReport::parse` lo
//!   rechaza), así que un campo incompleto como `Ov:100,100,10` nunca llega a ser un override;
//! - `okok`, `ok<Idle|…>`, `ok[MSG:…]`, `error:3ok`.
//!
//! Líneas como `okay`, `[OK] ILmp Link connect ok` o `ok T:20` quedan intactas.

/// Si `line` es varias respuestas pegadas, las piezas en orden (el reporte cortado, si lo hay,
/// va como pieza propia). `None` si la línea es normal: el camino habitual no reserva memoria.
pub fn split_glued(line: &str) -> Option<Vec<&str>> {
    // Camino rápido: sin `ok`/`error:` pegado a otra cosa no hay nada que separar.
    if line == "ok" || !(line.contains("ok") || line.contains("error:")) {
        return None;
    }
    let mut head = Vec::new();
    let mut rest = line;
    // Confirmaciones al inicio: `ok<Idle|...>`, `okok`, `error:3ok`...
    while rest.len() > ack_len_at_start(rest) {
        let n = ack_len_at_start(rest);
        if n == 0 || !starts_new_piece(&rest[n..]) {
            break;
        }
        head.push(&rest[..n]);
        rest = &rest[n..];
    }
    // Confirmaciones al final, de dentro hacia fuera: `<...>ok`, `<...truncado` + `ok`, `okok`.
    let mut tail = Vec::new();
    while let Some(n) = ack_len_at_end(rest) {
        let prefix = &rest[..rest.len() - n];
        if !can_precede_ack(prefix) {
            break;
        }
        tail.push(&rest[rest.len() - n..]);
        rest = prefix;
    }
    if head.is_empty() && tail.is_empty() {
        return None;
    }
    tail.reverse();
    let mut pieces = head;
    if !rest.is_empty() {
        pieces.push(rest);
    }
    pieces.extend(tail);
    Some(pieces)
}

/// Longitud de un `ok` o `error:N` al inicio de `s` (0 si no hay).
fn ack_len_at_start(s: &str) -> usize {
    if s.starts_with("ok") {
        return 2;
    }
    match s.strip_prefix("error:") {
        Some(digits) => {
            let d = digits.bytes().take_while(u8::is_ascii_digit).count();
            if d == 0 { 0 } else { 6 + d }
        }
        None => 0,
    }
}

/// Longitud de un `ok` o `error:N` al final de `s`.
fn ack_len_at_end(s: &str) -> Option<usize> {
    if s.ends_with("ok") {
        return Some(2);
    }
    let digits = s.bytes().rev().take_while(u8::is_ascii_digit).count();
    if digits == 0 || !s[..s.len() - digits].ends_with("error:") {
        return None;
    }
    Some(6 + digits)
}

/// ¿Lo que sigue a una confirmación al inicio empieza otra pieza? Un reporte, un mensaje u otra
/// confirmación; con eso `okay` y compañía no se tocan.
fn starts_new_piece(s: &str) -> bool {
    s.starts_with('<') || s.starts_with('[') || ack_len_at_start(s) > 0
}

/// ¿Puede `prefix` ir inmediatamente delante de una confirmación pegada? Primero se pelan las
/// confirmaciones que ya lleva al final (`okok`, `[MSG:x]ok` + `ok`).
fn can_precede_ack(prefix: &str) -> bool {
    if prefix.is_empty() {
        return false;
    }
    let mut base = prefix;
    while let Some(n) = ack_len_at_end(base) {
        base = &base[..base.len() - n];
    }
    // Solo confirmaciones (`okok`).
    base.is_empty()
        // Reporte completo, o reporte cortado (nunca cierra con `>`).
        || (base.starts_with('<') && (base.ends_with('>') || !base.contains('>')))
        // Mensaje completo (`[MSG:...]`, `[GC:...]`).
        || (base.starts_with('[') && base.ends_with(']'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_real_falcon2_line_is_split_into_fragment_and_ok() {
        let line = "<Run|MPos:0.287,0.112,0.000|Bf:28,65427|FS:10530,0|Ov:100,100,10ok";
        assert_eq!(
            split_glued(line),
            Some(vec![
                "<Run|MPos:0.287,0.112,0.000|Bf:28,65427|FS:10530,0|Ov:100,100,10",
                "ok"
            ])
        );
    }

    #[test]
    fn truncated_report_with_error_and_complete_report_with_ok() {
        assert_eq!(
            split_glued("<Run|MPos:0.287,0.1error:20"),
            Some(vec!["<Run|MPos:0.287,0.1", "error:20"])
        );
        assert_eq!(
            split_glued("<Idle|MPos:0.000,0.000,0.000|FS:0,0>ok"),
            Some(vec!["<Idle|MPos:0.000,0.000,0.000|FS:0,0>", "ok"])
        );
    }

    #[test]
    fn ok_glued_at_the_start_or_doubled() {
        assert_eq!(
            split_glued("ok<Idle|MPos:0,0,0|FS:0,0>"),
            Some(vec!["ok", "<Idle|MPos:0,0,0|FS:0,0>"])
        );
        assert_eq!(split_glued("okok"), Some(vec!["ok", "ok"]));
        assert_eq!(split_glued("okokok"), Some(vec!["ok", "ok", "ok"]));
        assert_eq!(split_glued("error:3ok"), Some(vec!["error:3", "ok"]));
        assert_eq!(
            split_glued("ok<Run|MPos:1,2,3|Bf:28,65427|FS:10530,0|Ov:100,100,10ok"),
            Some(vec![
                "ok",
                "<Run|MPos:1,2,3|Bf:28,65427|FS:10530,0|Ov:100,100,10",
                "ok"
            ])
        );
    }

    #[test]
    fn messages_followed_by_ok() {
        assert_eq!(
            split_glued("[MSG:Caution: Unlocked]ok"),
            Some(vec!["[MSG:Caution: Unlocked]", "ok"])
        );
        assert_eq!(
            split_glued("ok[MSG:Reset to continue]"),
            Some(vec!["ok", "[MSG:Reset to continue]"])
        );
    }

    #[test]
    fn normal_lines_are_left_alone() {
        for line in [
            "ok",
            "okay",
            "ok T:20.0 /0.0",
            "error:3",
            "error:",
            "<Idle|MPos:0,0,0|FS:0,0>",
            "<Run|MPos:0.287,0.112",
            "[OK] ILmp Link connect ok",
            "[OK] gas_init OK",
            "[MSG:Using cpu_map…esp32-s3]",
            "Grbl 1.1f ['$' for help]",
            "Probe ok",
            "token",
            "ALARM:3",
            "$100=250.000",
        ] {
            assert_eq!(split_glued(line), None, "{line}");
        }
    }
}
