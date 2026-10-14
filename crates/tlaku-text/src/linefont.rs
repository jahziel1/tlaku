//! Fuentes de un solo trazo (Hershey incluidas y SHX de AutoCAD cargadas por el usuario).
//!
//! Un glifo es una lista de trazos abiertos en unidades de la fuente, con la línea base en
//! `y = 0` y Y hacia arriba. El texto resultante son trayectorias **abiertas**: en modo línea se
//! graban como trazo, no como contorno.
//!
//! Hershey: los datos `.jhf` están en `data/hershey/` con su aviso (`NOTICE.txt`, y el
//! reconocimiento en `THIRD_PARTY.md`). Formato (descrito en el propio paquete): por glifo,
//! columnas 0-4 número, 5-7 cantidad de vértices (incluida la pareja izquierda/derecha), 8 y 9
//! posiciones izquierda y derecha, y después parejas de caracteres con las coordenadas relativas
//! a la letra `R`; la pareja ` R` levanta la pluma.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tlaku_i18n::tr;

/// Altura de la mayúscula en unidades de las fuentes Hershey (de y = -12 a y = 9).
const HERSHEY_CAP_UNITS: f64 = 21.0;
/// Fila (hacia abajo) de la línea base en los datos Hershey.
const HERSHEY_BASELINE_ROW: i32 = 9;
/// Glifos imprimibles ASCII por archivo (32..=126); el glifo 96 del archivo es un recuadro.
const HERSHEY_GLYPHS: usize = 95;

/// Un trazo: puntos unidos por rectas, sin cerrar.
pub type Stroke = Vec<(f64, f64)>;

#[derive(Debug, Clone, PartialEq)]
pub struct LineGlyph {
    /// Avance horizontal en unidades de la fuente.
    pub advance: f64,
    pub strokes: Vec<Stroke>,
}

/// De dónde sale la fuente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineFontSource {
    /// Incluida en Tlaku.
    Hershey,
    /// Archivo `.shx` de AutoCAD cargado por el usuario.
    Shx,
}

#[derive(Debug, Clone)]
pub struct LineFont {
    pub name: String,
    pub source: LineFontSource,
    /// Altura de la mayúscula en unidades de la fuente.
    pub cap_units: f64,
    /// Altura de la minúscula sin ascendentes, en unidades de la fuente.
    pub x_height_units: f64,
    glyphs: HashMap<char, LineGlyph>,
}

impl LineFont {
    pub fn new(
        name: impl Into<String>,
        source: LineFontSource,
        cap_units: f64,
        glyphs: HashMap<char, LineGlyph>,
    ) -> Self {
        let x_height_units = glyphs
            .get(&'x')
            .and_then(top_of)
            .filter(|h| *h > 0.0)
            .unwrap_or(cap_units * 2.0 / 3.0);
        Self {
            name: name.into(),
            source,
            cap_units,
            x_height_units,
            glyphs,
        }
    }

    /// Glifo del carácter; los acentuados y `¡ ¿ °` se componen con los básicos.
    pub fn glyph(&self, ch: char) -> Option<LineGlyph> {
        if let Some(g) = self.glyphs.get(&ch) {
            return Some(g.clone());
        }
        self.compose(ch)
    }

    pub fn has_glyph(&self, ch: char) -> bool {
        self.glyphs.contains_key(&ch) || self.compose(ch).is_some()
    }

    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }

    /// Letra acentuada = letra base + marca dibujada encima (o debajo, la cedilla).
    fn compose(&self, ch: char) -> Option<LineGlyph> {
        if ch == '¡' || ch == '¿' {
            let base = self.glyphs.get(if ch == '¡' { &'!' } else { &'?' })?;
            // Giro de 180° alrededor del centro de la mayúscula (el glifo queda "boca abajo").
            let (cx, cy) = (base.advance / 2.0, self.cap_units / 2.0);
            return Some(LineGlyph {
                advance: base.advance,
                strokes: base
                    .strokes
                    .iter()
                    .map(|s| {
                        s.iter()
                            .map(|&(x, y)| (2.0 * cx - x, 2.0 * cy - y))
                            .collect()
                    })
                    .collect(),
            });
        }
        if ch == '°' {
            let adv = self.glyphs.get(&'0')?.advance;
            let r = self.cap_units * 0.1;
            let mut g = LineGlyph {
                advance: adv,
                strokes: Vec::new(),
            };
            g.strokes.push(circle(adv / 2.0, self.cap_units - r, r));
            return Some(g);
        }
        let (base_ch, mark) = decompose(ch)?;
        let base = self.glyphs.get(&base_ch)?;
        let mut strokes = base.strokes.clone();
        if matches!(base_ch, 'i' | 'j') {
            // El punto de la i/j se sustituye por la marca: se quitan los trazos por encima de
            // la altura de la minúscula.
            strokes.retain(|s| min_y(s) <= self.x_height_units + 0.5);
        }
        let top = strokes
            .iter()
            .flat_map(|s| s.iter().map(|p| p.1))
            .fold(f64::NEG_INFINITY, f64::max);
        let top = if top.is_finite() { top } else { self.cap_units };
        let cx = base.advance / 2.0;
        strokes.extend(mark_strokes(mark, cx, top + 2.0));
        Some(LineGlyph {
            advance: base.advance,
            strokes,
        })
    }
}

fn top_of(g: &LineGlyph) -> Option<f64> {
    g.strokes
        .iter()
        .flat_map(|s| s.iter().map(|p| p.1))
        .fold(None, |m: Option<f64>, y| Some(m.map_or(y, |m| m.max(y))))
}

fn min_y(s: &Stroke) -> f64 {
    s.iter().map(|p| p.1).fold(f64::INFINITY, f64::min)
}

fn circle(cx: f64, cy: f64, r: f64) -> Stroke {
    (0..=8)
        .map(|i| {
            let a = std::f64::consts::TAU * f64::from(i) / 8.0;
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

#[derive(Clone, Copy)]
enum Mark {
    Acute,
    Grave,
    Circumflex,
    Diaeresis,
    Tilde,
    Ring,
    Cedilla,
}

fn decompose(ch: char) -> Option<(char, Mark)> {
    use Mark::*;
    Some(match ch {
        'á' => ('a', Acute),
        'é' => ('e', Acute),
        'í' => ('i', Acute),
        'ó' => ('o', Acute),
        'ú' => ('u', Acute),
        'ý' => ('y', Acute),
        'Á' => ('A', Acute),
        'É' => ('E', Acute),
        'Í' => ('I', Acute),
        'Ó' => ('O', Acute),
        'Ú' => ('U', Acute),
        'Ý' => ('Y', Acute),
        'à' => ('a', Grave),
        'è' => ('e', Grave),
        'ì' => ('i', Grave),
        'ò' => ('o', Grave),
        'ù' => ('u', Grave),
        'À' => ('A', Grave),
        'È' => ('E', Grave),
        'Ì' => ('I', Grave),
        'Ò' => ('O', Grave),
        'Ù' => ('U', Grave),
        'â' => ('a', Circumflex),
        'ê' => ('e', Circumflex),
        'î' => ('i', Circumflex),
        'ô' => ('o', Circumflex),
        'û' => ('u', Circumflex),
        'Â' => ('A', Circumflex),
        'Ê' => ('E', Circumflex),
        'Î' => ('I', Circumflex),
        'Ô' => ('O', Circumflex),
        'Û' => ('U', Circumflex),
        'ä' => ('a', Diaeresis),
        'ë' => ('e', Diaeresis),
        'ï' => ('i', Diaeresis),
        'ö' => ('o', Diaeresis),
        'ü' => ('u', Diaeresis),
        'ÿ' => ('y', Diaeresis),
        'Ä' => ('A', Diaeresis),
        'Ë' => ('E', Diaeresis),
        'Ï' => ('I', Diaeresis),
        'Ö' => ('O', Diaeresis),
        'Ü' => ('U', Diaeresis),
        'ã' => ('a', Tilde),
        'õ' => ('o', Tilde),
        'ñ' => ('n', Tilde),
        'Ã' => ('A', Tilde),
        'Õ' => ('O', Tilde),
        'Ñ' => ('N', Tilde),
        'å' => ('a', Ring),
        'Å' => ('A', Ring),
        'ç' => ('c', Cedilla),
        'Ç' => ('C', Cedilla),
        _ => return None,
    })
}

/// Trazos de la marca centrada en `cx`, con su base a la altura `y`.
fn mark_strokes(mark: Mark, cx: f64, y: f64) -> Vec<Stroke> {
    match mark {
        Mark::Acute => vec![vec![(cx - 1.0, y), (cx + 2.0, y + 4.0)]],
        Mark::Grave => vec![vec![(cx + 1.0, y), (cx - 2.0, y + 4.0)]],
        Mark::Circumflex => vec![vec![(cx - 2.5, y), (cx, y + 3.5), (cx + 2.5, y)]],
        // Cada punto es un trazo corto, vertical.
        Mark::Diaeresis => vec![
            vec![(cx - 2.2, y + 1.0), (cx - 2.2, y + 2.0)],
            vec![(cx + 2.2, y + 1.0), (cx + 2.2, y + 2.0)],
        ],
        Mark::Tilde => vec![vec![
            (cx - 3.0, y + 1.0),
            (cx - 1.5, y + 2.6),
            (cx, y + 1.6),
            (cx + 1.5, y + 0.6),
            (cx + 3.0, y + 2.2),
        ]],
        Mark::Ring => vec![circle(cx, y + 2.0, 1.8)],
        // La cedilla cuelga de la línea base: no depende de la altura de la letra.
        Mark::Cedilla => vec![vec![
            (cx - 0.5, 0.0),
            (cx, -2.0),
            (cx + 2.0, -3.0),
            (cx + 1.5, -5.0),
            (cx - 0.5, -5.5),
        ]],
    }
}

/// Lee un archivo `.jhf`. Solo se usan los primeros 95 glifos (ASCII 32..=126).
pub fn parse_jhf(name: &str, text: &str) -> Result<LineFont, String> {
    let mut glyphs = HashMap::new();
    let mut index = 0usize;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if index >= HERSHEY_GLYPHS {
            break;
        }
        let ch = char::from(32 + index as u8);
        index += 1;
        let bad = || tr!("text.linefont.bad_glyph", index = index, name = name);
        if !line.is_ascii() || line.len() < 10 {
            return Err(bad());
        }
        let b = line.as_bytes();
        let count: usize = line[5..8].trim().parse().map_err(|_| bad())?;
        let (left, right) = (i32::from(b[8]) - 82, i32::from(b[9]) - 82);
        let body = &b[10..];
        if count == 0 || body.len() != 2 * (count - 1) {
            return Err(bad());
        }
        let mut strokes: Vec<Stroke> = Vec::new();
        let mut current: Stroke = Vec::new();
        for pair in body.chunks_exact(2) {
            if pair == b" R" {
                if current.len() > 1 {
                    strokes.push(std::mem::take(&mut current));
                }
                current.clear();
                continue;
            }
            let x = i32::from(pair[0]) - 82 - left;
            let y = HERSHEY_BASELINE_ROW - (i32::from(pair[1]) - 82);
            current.push((f64::from(x), f64::from(y)));
        }
        if current.len() > 1 {
            strokes.push(current);
        }
        glyphs.insert(
            ch,
            LineGlyph {
                advance: f64::from(right - left),
                strokes,
            },
        );
    }
    if glyphs.len() != HERSHEY_GLYPHS {
        return Err(tr!(
            "text.linefont.glyph_count",
            name = name,
            found = glyphs.len(),
            expected = HERSHEY_GLYPHS
        ));
    }
    Ok(LineFont::new(
        name,
        LineFontSource::Hershey,
        HERSHEY_CAP_UNITS,
        glyphs,
    ))
}

macro_rules! hershey {
    ($name:literal, $file:literal) => {
        (
            $name,
            include_str!(concat!("../data/hershey/", $file, ".jhf")),
        )
    };
}

/// Fuentes Hershey incluidas: nombre visible y datos.
const HERSHEY_FILES: &[(&str, &str)] = &[
    hershey!("Hershey Roman Simplex", "rowmans"),
    hershey!("Hershey Roman Duplex", "rowmand"),
    hershey!("Hershey Roman Triplex", "rowmant"),
    hershey!("Hershey Futura Light", "futural"),
    hershey!("Hershey Futura Medium", "futuram"),
    hershey!("Hershey Times Roman", "timesr"),
    hershey!("Hershey Times Roman Bold", "timesrb"),
    hershey!("Hershey Times Italic", "timesi"),
    hershey!("Hershey Times Italic Bold", "timesib"),
    hershey!("Hershey Script Simplex", "scripts"),
    hershey!("Hershey Script Complex", "scriptc"),
    hershey!("Hershey Cursive", "cursive"),
    hershey!("Hershey Gothic English", "gothiceng"),
    hershey!("Hershey Gothic German", "gothicger"),
    hershey!("Hershey Gothic Italian", "gothicita"),
];

/// Fuentes Hershey incluidas, ya leídas (se leen una sola vez).
pub fn builtin_line_fonts() -> &'static [Arc<LineFont>] {
    static FONTS: OnceLock<Vec<Arc<LineFont>>> = OnceLock::new();
    FONTS.get_or_init(|| {
        HERSHEY_FILES
            .iter()
            .map(|(name, data)| {
                Arc::new(parse_jhf(name, data).expect("los datos Hershey incluidos son válidos"))
            })
            .collect()
    })
}

/// Fuente Hershey incluida por nombre, sin distinguir mayúsculas.
pub fn builtin_line_font(name: &str) -> Option<Arc<LineFont>> {
    let wanted = name.trim().to_lowercase();
    builtin_line_fonts()
        .iter()
        .find(|f| f.name.to_lowercase() == wanted)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_font_loads_with_all_ascii() {
        let fonts = builtin_line_fonts();
        assert_eq!(fonts.len(), HERSHEY_FILES.len());
        for f in fonts {
            assert_eq!(f.glyph_count(), 95, "{}", f.name);
            for c in ' '..='~' {
                assert!(f.has_glyph(c), "{} sin '{c}'", f.name);
            }
            assert!(f.cap_units == 21.0 && f.x_height_units > 5.0, "{}", f.name);
        }
    }

    #[test]
    fn simplex_h_has_known_geometry() {
        let f = builtin_line_font("hershey roman simplex").unwrap();
        let h = f.glyph('H').unwrap();
        // Dos verticales y el travesaño; 21 unidades de alto sobre la línea base.
        assert_eq!(h.strokes.len(), 3);
        assert_eq!(top_of(&h), Some(21.0));
        let lowest = h
            .strokes
            .iter()
            .flat_map(|s| s.iter().map(|p| p.1))
            .fold(f64::INFINITY, f64::min);
        assert_eq!(lowest, 0.0);
    }

    #[test]
    fn space_has_advance_and_no_strokes() {
        let f = builtin_line_font("Hershey Roman Simplex").unwrap();
        let s = f.glyph(' ').unwrap();
        assert!(s.strokes.is_empty() && s.advance > 0.0);
    }

    #[test]
    fn accents_add_a_mark_above_the_letter() {
        let f = builtin_line_font("Hershey Roman Simplex").unwrap();
        let n = f.glyph('n').unwrap();
        let ntilde = f.glyph('ñ').unwrap();
        assert_eq!(ntilde.advance, n.advance);
        assert_eq!(ntilde.strokes.len(), n.strokes.len() + 1);
        assert!(top_of(&ntilde).unwrap() > f.cap_units * 0.7);
        // La i pierde su punto: í = tallo + acento (2 trazos), i = tallo + punto.
        let i = f.glyph('i').unwrap();
        let iacute = f.glyph('í').unwrap();
        assert_eq!(iacute.strokes.len(), i.strokes.len());
        // ç cuelga bajo la línea base.
        let c = f.glyph('ç').unwrap();
        assert!(c.strokes.iter().flatten().any(|p| p.1 < -1.0));
    }

    #[test]
    fn inverted_punctuation_is_a_half_turn() {
        let f = builtin_line_font("Hershey Roman Simplex").unwrap();
        let q = f.glyph('?').unwrap();
        let iq = f.glyph('¿').unwrap();
        assert_eq!(q.strokes.len(), iq.strokes.len());
        assert_ne!(q.strokes, iq.strokes);
        // El punto de "?" está abajo y el de "¿" arriba del trazo curvo (medido por su centro).
        let dot_y = |g: &LineGlyph| {
            let s = g.strokes.iter().min_by_key(|s| s.len()).unwrap();
            s.iter().map(|p| p.1).sum::<f64>() / s.len() as f64
        };
        assert!(dot_y(&q) < 5.0 && dot_y(&iq) > 16.0);
    }

    #[test]
    fn rejects_corrupt_data() {
        assert!(parse_jhf("x", "  699  1JZ\n  714  9MWRFRT").is_err());
        assert!(parse_jhf("x", "").is_err());
    }
}
