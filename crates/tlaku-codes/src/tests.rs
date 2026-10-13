use super::*;
use std::path::PathBuf;
use tlaku_core::bezpath::PathSeg;

fn font_lib() -> FontLibrary {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tlaku-text/tests/fixtures/BebasNeue-Regular.ttf");
    let mut lib = FontLibrary::new();
    lib.load_font_file(&path)
        .expect("la fuente de prueba debe cargar");
    lib
}

fn spec(kind: CodeKind, data: &str) -> CodeSpec {
    CodeSpec {
        kind,
        data: data.into(),
        font_family: "Bebas Neue".into(),
        ..CodeSpec::default()
    }
}

fn gen_ok(s: &CodeSpec) -> CodeOutput {
    generate(&font_lib(), s).unwrap_or_else(|e| panic!("{e}"))
}

/// Contornos aplanados de todas las formas (cada subtrayecto es uno).
fn rings(out: &CodeOutput) -> Vec<Vec<(f64, f64)>> {
    out.paths
        .iter()
        .flat_map(|p| p.flatten(0.001))
        .map(|l| l.points.iter().map(|p| (p.x, p.y)).collect())
        .collect()
}

/// ¿El punto cae dentro de la región? Regla par-impar sobre todos los contornos.
fn inside(rings: &[Vec<(f64, f64)>], x: f64, y: f64) -> bool {
    let mut hits = 0;
    for r in rings {
        let n = r.len();
        for i in 0..n {
            let (a, b) = (r[i], r[(i + 1) % n]);
            if (a.1 > y) != (b.1 > y) {
                let t = (y - a.1) / (b.1 - a.1);
                if x < a.0 + t * (b.0 - a.0) {
                    hits += 1;
                }
            }
        }
    }
    hits % 2 == 1
}

fn all_closed(out: &CodeOutput) -> bool {
    out.paths.iter().all(|p| {
        let mut open = false;
        for s in &p.0 {
            match s {
                PathSeg::MoveTo { .. } => {
                    if open {
                        return false;
                    }
                    open = true;
                }
                PathSeg::Close => open = false,
                _ => {}
            }
        }
        !open
    })
}

/// Raster en escala de grises: `px` píxeles por mm con `quiet` mm blancos alrededor.
fn raster(out: &CodeOutput, px: f64, quiet: f64, invert_colors: bool) -> (usize, usize, Vec<u8>) {
    let rs = rings(out);
    let w = ((out.width_mm + 2.0 * quiet) * px).ceil() as usize;
    let h = ((out.height_mm + 2.0 * quiet) * px).ceil() as usize;
    let mut img = vec![255u8; w * h];
    for j in 0..h {
        for i in 0..w {
            let x = (i as f64 + 0.5) / px - quiet;
            // La imagen tiene Y hacia abajo.
            let y = out.height_mm + quiet - (j as f64 + 0.5) / px;
            let dark = inside(&rs, x, y) != invert_colors;
            if dark {
                img[j * w + i] = 0;
            }
        }
    }
    (w, h, img)
}

fn decode_qr(out: &CodeOutput) -> String {
    let (w, h, img) = raster(out, 5.0, out.width_mm * 0.15, false);
    let mut prepared = rqrr::PreparedImage::prepare_from_greyscale(w, h, |x, y| img[y * w + x]);
    let grids = prepared.detect_grids();
    assert_eq!(grids.len(), 1, "no se detectó el QR");
    grids[0].decode().expect("el QR debe decodificarse").1
}

#[test]
fn qr_decodes_back_to_the_same_text() {
    for (data, ecc) in [
        ("hola", QrEcc::L),
        ("https://tlaku.example/ñandú?x=1&y=2", QrEcc::M),
        ("Texto largo ".repeat(10).trim(), QrEcc::Q),
        ("12345678901234567890", QrEcc::H),
    ] {
        let mut s = spec(CodeKind::Qr, data);
        s.ecc = ecc;
        s.size_mm = 40.0;
        let out = gen_ok(&s);
        assert!(all_closed(&out));
        assert_eq!(decode_qr(&out), data, "{ecc:?}");
    }
}

#[test]
fn qr_size_and_origin_are_exact() {
    let mut s = spec(CodeKind::Qr, "tamaño");
    s.size_mm = 25.0;
    let out = gen_ok(&s);
    assert!((out.width_mm - 25.0).abs() < 1e-9 && (out.height_mm - 25.0).abs() < 1e-9);
    let rs = rings(&out);
    let min_x = rs
        .iter()
        .flatten()
        .map(|p| p.0)
        .fold(f64::INFINITY, f64::min);
    let min_y = rs
        .iter()
        .flatten()
        .map(|p| p.1)
        .fold(f64::INFINITY, f64::min);
    let max_x = rs
        .iter()
        .flatten()
        .map(|p| p.0)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(min_x.abs() < 1e-9 && min_y.abs() < 1e-9 && (max_x - 25.0).abs() < 1e-6);
    assert_eq!(out.modules, 21);
}

#[test]
fn qr_modules_are_welded_into_few_shapes() {
    let out = gen_ok(&spec(CodeKind::Qr, "unir"));
    // Un QR tiene 21x21 módulos; unidos quedan muchas menos formas que módulos oscuros.
    let dark: usize = 21 * 21 / 2;
    assert!(out.paths.len() < dark / 2, "{} formas", out.paths.len());
    // El patrón de posición tiene un anillo: exterior + agujero en el mismo BezPath.
    assert!(out.paths.iter().any(|p| {
        p.0.iter()
            .filter(|s| matches!(s, PathSeg::MoveTo { .. }))
            .count()
            >= 2
    }));
}

#[test]
fn inverted_qr_is_the_background_with_modules_as_holes() {
    let mut s = spec(CodeKind::Qr, "inverso");
    s.size_mm = 30.0;
    s.margin_mm = 3.0;
    let normal = gen_ok(&s);
    s.invert = true;
    let inv = gen_ok(&s);
    assert!((inv.width_mm - 36.0).abs() < 1e-9 && (inv.height_mm - 36.0).abs() < 1e-9);
    let (rn, ri) = (rings(&normal), rings(&inv));
    // En todo punto del área de módulos, lo grabado en una es lo no grabado en la otra.
    let m = 30.0 / normal.modules as f64;
    for r in 0..normal.modules {
        for c in 0..normal.modules {
            let (x, y) = ((c as f64 + 0.5) * m, (r as f64 + 0.5) * m);
            assert_ne!(
                inside(&rn, x, y),
                inside(&ri, x + 3.0, y + 3.0),
                "módulo ({c},{r})"
            );
        }
    }
    // El margen queda grabado (fondo).
    assert!(inside(&ri, 1.0, 1.0) && inside(&ri, 35.0, 18.0));
    // El fondo invertido decodifica con colores invertidos otra vez.
    let (w, h, img) = raster(&inv, 5.0, 4.0, true);
    let mut prepared = rqrr::PreparedImage::prepare_from_greyscale(w, h, |x, y| img[y * w + x]);
    let grids = prepared.detect_grids();
    assert_eq!(grids.len(), 1);
    assert_eq!(grids[0].decode().unwrap().1, "inverso");
}

#[test]
fn qr_errors_are_in_spanish() {
    let mut s = spec(CodeKind::Qr, &"x".repeat(5000));
    s.ecc = QrEcc::H;
    let e = generate(&font_lib(), &s).unwrap_err();
    assert!(e.contains("demasiado largo"), "{e}");
    assert!(generate(&font_lib(), &spec(CodeKind::Qr, "")).is_err());
    let mut bad = spec(CodeKind::Qr, "a");
    bad.size_mm = 0.0;
    assert!(generate(&font_lib(), &bad).is_err());
    bad.size_mm = 10.0;
    bad.margin_mm = -1.0;
    assert!(generate(&font_lib(), &bad).is_err());
}

#[test]
fn tiny_modules_warn() {
    let mut s = spec(CodeKind::Qr, "pequeño");
    s.size_mm = 3.0;
    let out = gen_ok(&s);
    assert!(out.warnings.iter().any(|w| w.contains("módulo")));
    s.size_mm = 40.0;
    assert!(gen_ok(&s).warnings.is_empty());
}

#[test]
fn data_matrix_decodes_back() {
    for data in ["Tlaku", "1234567890", "Data Matrix: ñandú 2026"] {
        let mut s = spec(CodeKind::DataMatrix, data);
        s.size_mm = 20.0;
        let out = gen_ok(&s);
        assert!(all_closed(&out));
        let side = out.modules;
        let rs = rings(&out);
        let m = 20.0 / side as f64;
        let pixels: Vec<bool> = (0..side)
            .flat_map(|r| (0..side).map(move |c| (r, c)))
            .map(|(r, c)| inside(&rs, (c as f64 + 0.5) * m, 20.0 - (r as f64 + 0.5) * m))
            .collect();
        let decoded = datamatrix::DataMatrix::decode(&pixels, side).expect("debe decodificar");
        let text = datamatrix::data::latin1_to_utf8(&decoded).unwrap_or_default();
        assert_eq!(text, data);
    }
}

/// Lee los módulos de un código 1D muestreando el centro de cada uno a media altura.
fn read_bars(out: &CodeOutput, size_mm: f64, bar_h: f64, n: usize) -> Vec<u8> {
    let rs = rings(out);
    let y = out.height_mm - bar_h / 2.0;
    let m = size_mm / n as f64;
    (0..n)
        .map(|i| u8::from(inside(&rs, (i as f64 + 0.5) * m, y)))
        .collect()
}

fn bits_str(bits: &[u8]) -> String {
    bits.iter().map(|b| char::from(b'0' + b)).collect()
}

// Tablas del estándar EAN/UPC (GS1), escritas aparte de `barcoders` para decodificar sin usarla.
const L_CODES: [&str; 10] = [
    "0001101", "0011001", "0010011", "0111101", "0100011", "0110001", "0101111", "0111011",
    "0110111", "0001011",
];
const PARITY: [&str; 10] = [
    "LLLLLL", "LLGLGG", "LLGGLG", "LLGGGL", "LGLLGG", "LGGLLG", "LGGGLL", "LGLGLG", "LGLGGL",
    "LGGLGL",
];

fn complement(code: &str) -> String {
    code.chars()
        .map(|c| if c == '0' { '1' } else { '0' })
        .collect()
}

fn g_code(d: usize) -> String {
    complement(L_CODES[d]).chars().rev().collect()
}

fn find(table: impl Fn(usize) -> String, chunk: &str) -> Option<usize> {
    (0..10).find(|&d| table(d) == chunk)
}

/// Decodifica EAN-13 (95 módulos) a sus 13 dígitos.
fn decode_ean13(bits: &str) -> String {
    assert_eq!(bits.len(), 95);
    assert_eq!(&bits[..3], "101");
    assert_eq!(&bits[45..50], "01010");
    assert_eq!(&bits[92..], "101");
    let mut digits = Vec::new();
    let mut parity = String::new();
    for k in 0..6 {
        let chunk = &bits[3 + 7 * k..10 + 7 * k];
        if let Some(d) = find(|d| L_CODES[d].to_string(), chunk) {
            parity.push('L');
            digits.push(d);
        } else {
            parity.push('G');
            digits.push(find(g_code, chunk).expect("módulo desconocido"));
        }
    }
    let first = PARITY
        .iter()
        .position(|p| *p == parity)
        .expect("paridad desconocida");
    for k in 0..6 {
        let chunk = &bits[50 + 7 * k..57 + 7 * k];
        digits.push(find(|d| complement(L_CODES[d]), chunk).expect("módulo R desconocido"));
    }
    std::iter::once(first)
        .chain(digits)
        .map(|d| char::from(b'0' + d as u8))
        .collect()
}

fn decode_ean8(bits: &str) -> String {
    assert_eq!(bits.len(), 67);
    assert_eq!(&bits[..3], "101");
    assert_eq!(&bits[31..36], "01010");
    assert_eq!(&bits[64..], "101");
    let mut out = String::new();
    for k in 0..4 {
        let d = find(|d| L_CODES[d].to_string(), &bits[3 + 7 * k..10 + 7 * k]).unwrap();
        out.push(char::from(b'0' + d as u8));
    }
    for k in 0..4 {
        let d = find(|d| complement(L_CODES[d]), &bits[36 + 7 * k..43 + 7 * k]).unwrap();
        out.push(char::from(b'0' + d as u8));
    }
    out
}

#[test]
fn ean13_encodes_the_spec_example() {
    // 5901234123457 es el ejemplo clásico de la especificación (dígito de control 7).
    let enc = encode_linear(CodeKind::Ean13, "590123412345").unwrap();
    assert_eq!(enc.text, "5 901234 123457");
    assert_eq!(decode_ean13(&bits_str(&enc.bits)), "5901234123457");
    // Con el dígito de control puesto por el usuario sale igual.
    assert_eq!(
        encode_linear(CodeKind::Ean13, "5901234123457").unwrap(),
        enc
    );
}

#[test]
fn ean_check_digit_rules() {
    assert_eq!(check_digit(&[5, 9, 0, 1, 2, 3, 4, 1, 2, 3, 4, 5]), 7);
    assert_eq!(check_digit(&[9, 6, 3, 8, 5, 0, 7]), 4);
    let e = encode_linear(CodeKind::Ean13, "5901234123450").unwrap_err();
    assert!(e.contains("dígito de control") && e.contains('7'), "{e}");
    assert!(
        encode_linear(CodeKind::Ean13, "12345")
            .unwrap_err()
            .contains("12 dígitos")
    );
    assert!(
        encode_linear(CodeKind::Ean13, "59012341234a")
            .unwrap_err()
            .contains("dígitos")
    );
}

#[test]
fn ean8_and_upca_use_the_ean_tables() {
    let ean8 = encode_linear(CodeKind::Ean8, "9638507").unwrap();
    assert_eq!(decode_ean8(&bits_str(&ean8.bits)), "96385074");
    assert_eq!(ean8.text, "9638 5074");
    // UPC-A de ejemplo: 036000291452 (control 2).
    let upc = encode_linear(CodeKind::UpcA, "03600029145").unwrap();
    assert_eq!(upc.text, "0 36000 29145 2");
    assert_eq!(decode_ean13(&bits_str(&upc.bits)), "0036000291452");
    assert_eq!(encode_linear(CodeKind::UpcA, "036000291452").unwrap(), upc);
    assert!(encode_linear(CodeKind::UpcA, "036000291453").is_err());
}

#[test]
fn code128_structure_and_charsets() {
    let b = encode_linear(CodeKind::Code128, "Hi").unwrap();
    // inicio B + 2 símbolos + control (11 módulos cada uno) + parada de 13.
    assert_eq!(b.bits.len(), 11 * 4 + 13);
    assert!(bits_str(&b.bits).starts_with("11010010000"));
    assert!(bits_str(&b.bits).ends_with("1100011101011"));
    // Solo dígitos y par: el juego C usa un símbolo por cada dos dígitos.
    let c = encode_linear(CodeKind::Code128, "123456").unwrap();
    assert_eq!(c.bits.len(), 11 * 5 + 13);
    assert!(bits_str(&c.bits).starts_with("11010011100"));
    assert!(encode_linear(CodeKind::Code128, "ñ").is_err());
    assert!(encode_linear(CodeKind::Code128, "x").is_err());
}

#[test]
fn code39_itf_and_codabar_rules() {
    let c = encode_linear(CodeKind::Code39, "tlaku-26").unwrap();
    assert_eq!(c.text, "TLAKU-26");
    assert!(c.warnings.iter().any(|w| w.contains("mayúsculas")));
    // '*' de inicio y de fin: 12 módulos + separador cada carácter.
    assert_eq!(c.bits.len(), 13 * (8 + 2) - 1);
    assert!(bits_str(&c.bits).starts_with("1001011011010"));
    assert!(
        encode_linear(CodeKind::Code39, "a_b")
            .unwrap_err()
            .contains('_')
    );

    let odd = encode_linear(CodeKind::Itf, "123").unwrap();
    assert!(odd.warnings.iter().any(|w| w.contains("par")));
    assert_eq!(odd.text.len(), 4);
    let even = encode_linear(CodeKind::Itf, "1234").unwrap();
    assert!(even.warnings.is_empty());
    assert!(bits_str(&even.bits).starts_with("1010") && bits_str(&even.bits).ends_with("1101"));
    assert!(encode_linear(CodeKind::Itf, "12x4").is_err());

    let cb = encode_linear(CodeKind::Codabar, "1234").unwrap();
    assert_eq!(cb.text, "A1234A");
    assert!(cb.warnings.iter().any(|w| w.contains("A")));
    assert!(
        encode_linear(CodeKind::Codabar, "B123C")
            .unwrap()
            .warnings
            .is_empty()
    );
}

#[test]
fn bars_geometry_matches_the_encoded_modules() {
    for (kind, data) in [
        (CodeKind::Ean13, "590123412345"),
        (CodeKind::Code128, "Tlaku 2026"),
        (CodeKind::Code39, "ABC-123"),
        (CodeKind::Itf, "12345678"),
        (CodeKind::Ean8, "9638507"),
        (CodeKind::UpcA, "03600029145"),
        (CodeKind::Codabar, "A1234B"),
    ] {
        let mut s = spec(kind, data);
        s.size_mm = 60.0;
        s.height_mm = 12.0;
        let enc = encode_linear(kind, data).unwrap();
        let out = gen_ok(&s);
        assert!(all_closed(&out), "{kind:?}");
        assert!((out.width_mm - 60.0).abs() < 1e-6 && (out.height_mm - 12.0).abs() < 1e-6);
        assert_eq!(out.modules, enc.bits.len());
        assert_eq!(
            read_bars(&out, 60.0, 12.0, enc.bits.len()),
            enc.bits,
            "{kind:?}"
        );
        // Cada tramo de barras contiguas es una sola forma, sin costuras.
        let runs = enc
            .bits
            .windows(2)
            .filter(|w| w[0] == 0 && w[1] == 1)
            .count()
            + usize::from(enc.bits[0] == 1);
        assert_eq!(out.paths.len(), runs, "{kind:?}");
    }
}

#[test]
fn readable_text_goes_below_the_bars_and_fits() {
    let mut s = spec(CodeKind::Ean13, "590123412345");
    s.size_mm = 40.0;
    s.height_mm = 15.0;
    let plain = gen_ok(&s);
    s.human_readable = true;
    let out = gen_ok(&s);
    assert!(out.height_mm > plain.height_mm + 2.0, "{}", out.height_mm);
    assert!(out.width_mm <= 40.0 + 1e-6);
    assert!(all_closed(&out));
    // Las barras conservan su forma: arriba, a la misma altura.
    let rs = rings(&out);
    let bar_bottom = out.height_mm - 15.0;
    let n = 95;
    let enc = encode_linear(CodeKind::Ean13, "590123412345").unwrap();
    let m = 40.0 / n as f64;
    for (i, bit) in enc.bits.iter().enumerate() {
        assert_eq!(
            inside(&rs, (i as f64 + 0.5) * m, bar_bottom + 7.5),
            *bit == 1
        );
    }
    // Hay algo dibujado por debajo de las barras (el texto) y nada pegado a ellas.
    let below: Vec<f64> = rs
        .iter()
        .flatten()
        .map(|p| p.1)
        .filter(|y| *y < bar_bottom - 1e-9)
        .collect();
    assert!(!below.is_empty());
    let top_of_text = below.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    assert!(top_of_text < bar_bottom - 0.1);
    assert!(out.paths.len() > plain.paths.len());
}

#[test]
fn long_readable_text_shrinks_to_the_bars_width() {
    let mut s = spec(CodeKind::Code128, "UNA ETIQUETA MUY LARGA 0123456789");
    s.size_mm = 30.0;
    s.height_mm = 10.0;
    s.human_readable = true;
    let out = gen_ok(&s);
    assert!(out.width_mm <= 30.0 + 1e-6, "{}", out.width_mm);
}

#[test]
fn readable_text_needs_an_outline_font() {
    let mut s = spec(CodeKind::Code39, "ABC");
    s.human_readable = true;
    s.font_family = String::new();
    assert!(generate(&font_lib(), &s).unwrap_err().contains("fuente"));
    s.font_family = "Hershey Roman Simplex".into();
    assert!(generate(&font_lib(), &s).unwrap_err().contains("una línea"));
    s.font_family = "No Existe".into();
    assert!(generate(&font_lib(), &s).is_err());
}

#[test]
fn inverted_barcode_is_the_background_with_bars_as_holes() {
    let mut s = spec(CodeKind::Code128, "Tlaku");
    s.size_mm = 50.0;
    s.height_mm = 10.0;
    s.margin_mm = 2.5;
    s.invert = true;
    let out = gen_ok(&s);
    assert!((out.width_mm - 55.0).abs() < 1e-9 && (out.height_mm - 15.0).abs() < 1e-9);
    let enc = encode_linear(CodeKind::Code128, "Tlaku").unwrap();
    let rs = rings(&out);
    let m = 50.0 / enc.bits.len() as f64;
    for (i, bit) in enc.bits.iter().enumerate() {
        assert_eq!(
            inside(&rs, 2.5 + (i as f64 + 0.5) * m, 7.5),
            *bit == 0,
            "módulo {i}"
        );
    }
    assert!(inside(&rs, 1.0, 1.0));
}

#[test]
fn inverted_barcode_with_text_cuts_the_text_out_of_the_background() {
    let mut s = spec(CodeKind::Ean8, "9638507");
    s.size_mm = 30.0;
    s.height_mm = 10.0;
    s.human_readable = true;
    s.invert = true;
    let inv = gen_ok(&s);
    s.invert = false;
    let normal = gen_ok(&s);
    // Misma caja interior: el fondo es mayor por el margen (2 mm por lado).
    assert!((inv.width_mm - normal.width_mm - 4.0).abs() < 1e-6);
    assert!((inv.height_mm - normal.height_mm - 4.0).abs() < 1e-6);
    let (rn, ri) = (rings(&normal), rings(&inv));
    // Muestra en una rejilla: dentro de la caja interior, grabado en uno = no grabado en el otro.
    let mut checked = 0;
    for i in 0..120 {
        for j in 0..60 {
            let (x, y) = (
                (i as f64 + 0.5) * normal.width_mm / 120.0,
                (j as f64 + 0.5) * normal.height_mm / 60.0,
            );
            assert_ne!(inside(&rn, x, y), inside(&ri, x + 2.0, y + 2.0));
            checked += 1;
        }
    }
    assert_eq!(checked, 7200);
}

#[test]
fn matrix_geometry_matches_every_module() {
    let mut s = spec(CodeKind::Qr, "módulos");
    s.size_mm = 33.0;
    let out = gen_ok(&s);
    let (bits, side) = crate::encode::matrix(&s, &mut Vec::new()).unwrap();
    let rs = rings(&out);
    let m = 33.0 / side as f64;
    for r in 0..side {
        for c in 0..side {
            let got = inside(&rs, (c as f64 + 0.5) * m, 33.0 - (r as f64 + 0.5) * m);
            assert_eq!(got, bits[r * side + c], "({c},{r})");
        }
    }
}

#[test]
fn spec_round_trips_through_json_with_defaults() {
    let json = serde_json::json!({ "kind": "ean13", "data": "590123412345", "size_mm": 40.0 });
    let s: CodeSpec = serde_json::from_value(json).unwrap();
    assert_eq!(s.kind, CodeKind::Ean13);
    assert_eq!(s.ecc, QrEcc::M);
    assert!(!s.invert);
    let back = serde_json::to_value(&s).unwrap();
    assert_eq!(back["kind"], "ean13");
}
