//! Codificación de los datos a matrices de módulos (sin geometría).

use crate::{CodeKind, CodeSpec, QrEcc};
use barcoders::error::Error as BarError;
use barcoders::sym::codabar::Codabar;
use barcoders::sym::code39::Code39;
use barcoders::sym::code128::Code128;
use barcoders::sym::ean8::EAN8;
use barcoders::sym::ean13::EAN13;
use barcoders::sym::tf::TF;
use datamatrix::{DataMatrixBuilder, SymbolList};
use qrcode::types::QrError;
use qrcode::{Color, EcLevel, QrCode};
use tlaku_i18n::tr;

/// Código de barras 1D ya codificado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encoded {
    /// 1 = barra, 0 = espacio; sin zona en blanco.
    pub bits: Vec<u8>,
    /// Texto legible que va bajo las barras (con el dígito de control y agrupado como en la
    /// simbología).
    pub text: String,
    pub warnings: Vec<String>,
}

/// Dígito de control EAN/UPC de `payload` (sin el dígito de control): pesos 3, 1, 3, 1… de
/// derecha a izquierda.
pub fn check_digit(payload: &[u8]) -> u8 {
    let sum: u32 = payload
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| u32::from(d) * if i % 2 == 0 { 3 } else { 1 })
        .sum();
    ((10 - sum % 10) % 10) as u8
}

fn digits_of(data: &str, name: &str) -> Result<Vec<u8>, String> {
    data.chars()
        .map(|c| {
            c.to_digit(10)
                .map(|d| d as u8)
                .ok_or_else(|| tr!("codes.encode.digits_only", name = name, ch = c))
        })
        .collect()
}

fn bar_error(kind: &str, e: BarError) -> String {
    match e {
        BarError::Character => tr!("codes.encode.bar_character", kind = kind),
        BarError::Length => tr!("codes.encode.bar_length", kind = kind),
        BarError::Checksum => tr!("codes.encode.bar_checksum", kind = kind),
        BarError::Generate => tr!("codes.encode.bar_generate", kind = kind),
    }
}

/// Lee `data` como `payload_len` dígitos, con el dígito de control opcional; si viene, se
/// comprueba. Devuelve todos los dígitos con el de control al final.
fn with_check(data: &str, name: &str, payload_len: usize) -> Result<Vec<u8>, String> {
    let digits = digits_of(data, name)?;
    if digits.len() != payload_len && digits.len() != payload_len + 1 {
        return Err(tr!(
            "codes.encode.digit_count",
            name = name,
            n = payload_len,
            with_check = payload_len + 1,
            found = digits.len()
        ));
    }
    let expected = check_digit(&digits[..payload_len]);
    if digits.len() == payload_len + 1 && digits[payload_len] != expected {
        return Err(tr!(
            "codes.encode.wrong_check",
            name = name,
            expected = expected
        ));
    }
    let mut all = digits[..payload_len].to_vec();
    all.push(expected);
    Ok(all)
}

fn join(digits: &[u8]) -> String {
    digits.iter().map(|d| char::from(b'0' + d)).collect()
}

/// Codifica un código de barras 1D.
pub fn encode_linear(kind: CodeKind, data: &str) -> Result<Encoded, String> {
    let mut warnings = Vec::new();
    let (bits, text) = match kind {
        CodeKind::Ean13 => {
            let d = with_check(data, "EAN-13", 12)?;
            let ean = EAN13::new(join(&d[..12])).map_err(|e| bar_error("EAN-13", e))?;
            let text = format!("{} {} {}", d[0], join(&d[1..7]), join(&d[7..]));
            (ean.encode(), text)
        }
        CodeKind::UpcA => {
            let d = with_check(data, "UPC-A", 11)?;
            // UPC-A es un EAN-13 con un cero delante.
            let ean =
                EAN13::new(format!("0{}", join(&d[..11]))).map_err(|e| bar_error("UPC-A", e))?;
            let text = format!("{} {} {} {}", d[0], join(&d[1..6]), join(&d[6..11]), d[11]);
            (ean.encode(), text)
        }
        CodeKind::Ean8 => {
            let d = with_check(data, "EAN-8", 7)?;
            let ean = EAN8::new(join(&d[..7])).map_err(|e| bar_error("EAN-8", e))?;
            let text = format!("{} {}", join(&d[..4]), join(&d[4..]));
            (ean.encode(), text)
        }
        CodeKind::Code39 => {
            let upper = data.to_uppercase();
            if upper != data {
                warnings.push(tr!("codes.encode.code39_uppercase"));
            }
            if let Some(bad) = upper
                .chars()
                .find(|c| !(c.is_ascii_digit() || c.is_ascii_uppercase() || "-. $/+%".contains(*c)))
            {
                return Err(tr!("codes.encode.code39_charset", ch = bad));
            }
            let code = Code39::new(&upper).map_err(|e| bar_error("Code 39", e))?;
            (code.encode(), upper)
        }
        CodeKind::Code128 => {
            if let Some(bad) = data.chars().find(|c| !(' '..='~').contains(c)) {
                return Err(tr!("codes.encode.code128_charset", ch = bad));
            }
            if data.chars().count() < 2 {
                // barcoders no codifica un solo carácter; se avisa en vez de rellenar.
                return Err(tr!("codes.encode.code128_short"));
            }
            // Solo dígitos y cantidad par: el juego C los comprime a la mitad de ancho. Si no,
            // el B cubre todo el ASCII imprimible. El primer carácter elige el juego inicial.
            let compact = data.len().is_multiple_of(2) && data.chars().all(|c| c.is_ascii_digit());
            let prefixed = format!("{}{data}", if compact { 'Ć' } else { 'Ɓ' });
            let code = Code128::new(prefixed).map_err(|e| bar_error("Code 128", e))?;
            (code.encode(), data.to_string())
        }
        CodeKind::Itf => {
            let digits = digits_of(data, "ITF")?;
            if digits.is_empty() {
                return Err(tr!("codes.encode.itf_empty"));
            }
            let mut shown = digits.clone();
            if !digits.len().is_multiple_of(2) {
                // ITF codifica pares de dígitos: con cantidad impar se añade un dígito de control.
                let tf = TF::interleaved(data).map_err(|e| bar_error("ITF", e))?;
                warnings.push(tr!("codes.encode.itf_odd"));
                let check = tf_check(&digits);
                shown.push(check);
                (tf.encode(), join(&shown))
            } else {
                let tf = TF::interleaved(data).map_err(|e| bar_error("ITF", e))?;
                (tf.encode(), join(&shown))
            }
        }
        CodeKind::Codabar => {
            let upper = data.to_uppercase();
            let starts = |c: Option<char>| matches!(c, Some('A'..='D'));
            let framed = if starts(upper.chars().next()) && starts(upper.chars().last()) {
                upper.clone()
            } else {
                warnings.push(tr!("codes.encode.codabar_frame"));
                format!("A{upper}A")
            };
            if let Some(bad) = framed
                .chars()
                .find(|c| !(c.is_ascii_digit() || "ABCD-$/:.+".contains(*c)))
            {
                return Err(tr!("codes.encode.codabar_charset", ch = bad));
            }
            let code = Codabar::new(&framed).map_err(|e| bar_error("Codabar", e))?;
            (code.encode(), framed)
        }
        CodeKind::Qr | CodeKind::DataMatrix => {
            return Err(tr!("codes.encode.not_linear"));
        }
    };
    Ok(Encoded {
        bits,
        text,
        warnings,
    })
}

/// Dígito de control módulo 10 que `barcoders` añade en ITF con cantidad impar (pesos 3,1,3…
/// desde la derecha, igual que EAN).
fn tf_check(digits: &[u8]) -> u8 {
    check_digit(digits)
}

/// Matriz de un código 2D: módulos oscuros por filas de arriba abajo y lado en módulos.
pub(crate) fn matrix(
    spec: &CodeSpec,
    warnings: &mut Vec<String>,
) -> Result<(Vec<bool>, usize), String> {
    match spec.kind {
        CodeKind::Qr => {
            let level = match spec.ecc {
                QrEcc::L => EcLevel::L,
                QrEcc::M => EcLevel::M,
                QrEcc::Q => EcLevel::Q,
                QrEcc::H => EcLevel::H,
            };
            let code =
                QrCode::with_error_correction_level(spec.data.as_bytes(), level).map_err(|e| {
                    match e {
                        QrError::DataTooLong => {
                            tr!("codes.encode.qr_too_long")
                        }
                        other => tr!("codes.encode.qr_failed", error = other),
                    }
                })?;
            let bits = code
                .to_colors()
                .into_iter()
                .map(|c| c == Color::Dark)
                .collect();
            Ok((bits, code.width()))
        }
        CodeKind::DataMatrix => {
            let dm = DataMatrixBuilder::new()
                .with_symbol_list(SymbolList::default().enforce_square())
                .encode_str(&spec.data)
                .map_err(|_| tr!("codes.encode.dm_too_long"))?;
            let bitmap = dm.bitmap();
            if bitmap.width() != bitmap.height() {
                warnings.push(tr!("codes.encode.dm_rectangular"));
            }
            let side = bitmap.width();
            Ok((bitmap.bits().to_vec(), side))
        }
        _ => Err(tr!("codes.encode.not_matrix")),
    }
}
