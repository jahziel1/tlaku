//! Fuente de trazo sencillo para los rótulos grabados: mayúsculas, cifras y unos pocos signos.
//! Es propia (cuadrícula de 2 x 4 unidades, sin curvas): rotular una pieza no necesita tipografías
//! ni depende del motor de texto.

use tlaku_core::{Point, Polyline};

/// Alto de la cuadrícula de un glifo, en unidades.
const GRID_H: f64 = 4.0;
/// Espacio entre letras, en unidades de la cuadrícula.
const GAP: f64 = 1.0;

/// `(carácter, ancho, trazos)`. Cada punto son dos cifras `xy` y los trazos se separan con `|`.
const GLYPHS: &[(char, u8, &str)] = &[
    (' ', 2, ""),
    ('A', 2, "00 03 14 23 20|02 22"),
    ('B', 2, "00 04 14 23 12 02|12 21 10 00"),
    ('C', 2, "24 04 00 20"),
    ('D', 2, "00 04 14 23 21 10 00"),
    ('E', 2, "24 04 00 20|02 12"),
    ('F', 2, "00 04 24|02 12"),
    ('G', 2, "24 04 00 20 22 12"),
    ('H', 2, "04 00|02 22|24 20"),
    ('I', 2, "04 24|14 10|00 20"),
    ('J', 2, "24 21 10 01"),
    ('K', 2, "04 00|24 02 20"),
    ('L', 2, "04 00 20"),
    ('M', 2, "00 04 12 24 20"),
    ('N', 2, "00 04 20 24"),
    ('O', 2, "00 04 24 20 00"),
    ('P', 2, "00 04 24 22 02"),
    ('Q', 2, "00 04 24 20 00|11 20"),
    ('R', 2, "00 04 24 22 02|12 20"),
    ('S', 2, "24 04 02 22 20 00"),
    ('T', 2, "04 24|14 10"),
    ('U', 2, "04 00 20 24"),
    ('V', 2, "04 10 24"),
    ('W', 2, "04 00 12 20 24"),
    ('X', 2, "04 20|00 24"),
    ('Y', 2, "04 12 24|12 10"),
    ('Z', 2, "04 24 00 20"),
    ('0', 2, "00 04 24 20 00|00 24"),
    ('1', 2, "03 14 10|00 20"),
    ('2', 2, "04 24 22 02 00 20"),
    ('3', 2, "04 24 20 00|02 22"),
    ('4', 2, "04 02 22|24 20"),
    ('5', 2, "24 04 02 22 20 00"),
    ('6', 2, "24 04 00 20 22 02"),
    ('7', 2, "04 24 20"),
    ('8', 2, "00 04 24 20 00|02 22"),
    ('9', 2, "20 24 04 02 22"),
    ('.', 1, "00 01"),
    ('-', 2, "02 22"),
    ('+', 2, "02 22|13 11"),
    ('=', 2, "01 21|03 23"),
    ('/', 2, "00 24"),
    ('x', 2, "01 23|03 21"),
];

/// Letra sin acento (y en mayúscula) con la que se busca el glifo.
fn base(ch: char) -> char {
    match ch {
        'á' | 'à' | 'â' | 'ä' | 'Á' | 'À' | 'Â' | 'Ä' => 'A',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'ó' | 'ò' | 'ô' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Ö' => 'O',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'ñ' | 'Ñ' => 'N',
        'x' => 'x',
        c => c.to_ascii_uppercase(),
    }
}

fn glyph(ch: char) -> Option<(f64, &'static str)> {
    let c = base(ch);
    GLYPHS
        .iter()
        .find(|g| g.0 == c)
        .map(|g| (f64::from(g.1), g.2))
}

/// Ancho en mm que ocupa `text` con letras de `height_mm` de alto.
pub fn text_width(text: &str, height_mm: f64) -> f64 {
    let s = height_mm / GRID_H;
    let units: f64 = text
        .chars()
        .filter_map(glyph)
        .map(|(w, _)| w + GAP)
        .sum::<f64>();
    (units - if units > 0.0 { GAP } else { 0.0 }).max(0.0) * s
}

/// Trazos del texto con la esquina inferior izquierda en `(x, y)`. Los caracteres sin glifo se
/// omiten.
pub fn text_strokes(text: &str, x: f64, y: f64, height_mm: f64) -> Vec<Polyline> {
    let s = height_mm / GRID_H;
    let mut cursor = 0.0;
    let mut out = Vec::new();
    for (w, strokes) in text.chars().filter_map(glyph) {
        for stroke in strokes.split('|').filter(|s| !s.is_empty()) {
            let points: Vec<Point> = stroke
                .split(' ')
                .map(|p| {
                    let b = p.as_bytes();
                    Point::new(
                        x + (cursor + f64::from(b[0] - b'0')) * s,
                        y + f64::from(b[1] - b'0') * s,
                    )
                })
                .collect();
            out.push(Polyline {
                points,
                closed: false,
            });
        }
        cursor += w + GAP;
    }
    out
}

/// Trazos del texto centrados en `(cx, cy)`.
pub fn text_centered(text: &str, cx: f64, cy: f64, height_mm: f64) -> Vec<Polyline> {
    let w = text_width(text, height_mm);
    text_strokes(text, cx - w / 2.0, cy - height_mm / 2.0, height_mm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_glyph_is_well_formed_and_inside_its_cell() {
        for &(ch, w, strokes) in GLYPHS {
            for stroke in strokes.split('|').filter(|s| !s.is_empty()) {
                let pts: Vec<&str> = stroke.split(' ').collect();
                assert!(pts.len() >= 2, "glifo {ch}: trazo de un punto");
                for p in pts {
                    let b = p.as_bytes();
                    assert_eq!(b.len(), 2, "glifo {ch}: punto {p}");
                    assert!(b[0] - b'0' <= w, "glifo {ch}: x fuera");
                    assert!(b[1] - b'0' <= 4, "glifo {ch}: y fuera");
                }
            }
        }
    }

    #[test]
    fn width_and_height_follow_the_scale() {
        let s = text_strokes("A0", 10.0, 20.0, 4.0);
        let xs: Vec<f64> = s
            .iter()
            .flat_map(|p| p.points.iter().map(|q| q.x))
            .collect();
        let ys: Vec<f64> = s
            .iter()
            .flat_map(|p| p.points.iter().map(|q| q.y))
            .collect();
        assert!((xs.iter().cloned().fold(f64::MAX, f64::min) - 10.0).abs() < 1e-12);
        assert!(
            (xs.iter().cloned().fold(f64::MIN, f64::max) - 10.0 - text_width("A0", 4.0)).abs()
                < 1e-12
        );
        assert!((ys.iter().cloned().fold(f64::MAX, f64::min) - 20.0).abs() < 1e-12);
        assert!((ys.iter().cloned().fold(f64::MIN, f64::max) - 24.0).abs() < 1e-12);
    }

    #[test]
    fn accents_and_lowercase_fall_back_to_capitals() {
        assert_eq!(text_width("atrás", 4.0), text_width("ATRAS", 4.0));
    }
}
