//! Prueba de materiales: una cuadrícula de celdas donde cada una es una capa con su propia
//! combinación de parámetros, con rótulos de trazo sencillo.
//!
//! El documento resultante es un `Document` v2 normal: un rectángulo por celda, cada uno en una
//! capa de color único, y (si se pide) los rótulos en una capa line aparte. Las filas van de
//! arriba hacia abajo y las columnas de izquierda a derecha; `origin` es la esquina inferior
//! izquierda de todo el conjunto (rótulos incluidos).

use crate::layers::MAX_PASSES;
use std::collections::HashSet;
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Point, Polyline, Rgb};
use tlaku_doc::affine::IDENTITY;
use tlaku_doc::{
    DOC_VERSION, DesignObject, Document, LayerMode, LayerSettings, ObjectKind, TestAxis, TestParam,
    VectorObject,
};

/// Máximo de celdas por eje.
const MAX_COUNT: u32 = 30;
/// Altura de los rótulos, en mm.
const LABEL_HEIGHT: f64 = 3.0;
/// Espacio entre un rótulo y lo que rotula, en mm.
const LABEL_GAP: f64 = 2.0;
/// Color de la capa de rótulos: negro, que ninguna celda usa.
const LABEL_COLOR: Rgb = Rgb(0, 0, 0);

// ---------------------------------------------------------------------------
// Fuente de trazo sencillo
// ---------------------------------------------------------------------------

/// Letra de la fuente propia: trazos abiertos sobre una cuadrícula de `w` x 4 unidades (Y hacia
/// arriba). Solo contiene lo necesario para los rótulos de la prueba.
struct Glyph {
    ch: char,
    w: f64,
    strokes: &'static [&'static [(f64, f64)]],
}

const GLYPHS: &[Glyph] = &[
    Glyph {
        ch: ' ',
        w: 1.0,
        strokes: &[],
    },
    Glyph {
        ch: '0',
        w: 2.0,
        strokes: &[&[(0.0, 0.0), (2.0, 0.0), (2.0, 4.0), (0.0, 4.0), (0.0, 0.0)]],
    },
    Glyph {
        ch: '1',
        w: 2.0,
        strokes: &[&[(0.0, 3.0), (1.0, 4.0), (1.0, 0.0)]],
    },
    Glyph {
        ch: '2',
        w: 2.0,
        strokes: &[&[
            (0.0, 4.0),
            (2.0, 4.0),
            (2.0, 2.0),
            (0.0, 2.0),
            (0.0, 0.0),
            (2.0, 0.0),
        ]],
    },
    Glyph {
        ch: '3',
        w: 2.0,
        strokes: &[
            &[(0.0, 4.0), (2.0, 4.0), (2.0, 0.0), (0.0, 0.0)],
            &[(0.0, 2.0), (2.0, 2.0)],
        ],
    },
    Glyph {
        ch: '4',
        w: 2.0,
        strokes: &[
            &[(0.0, 4.0), (0.0, 2.0), (2.0, 2.0)],
            &[(2.0, 4.0), (2.0, 0.0)],
        ],
    },
    Glyph {
        ch: '5',
        w: 2.0,
        strokes: &[&[
            (2.0, 4.0),
            (0.0, 4.0),
            (0.0, 2.0),
            (2.0, 2.0),
            (2.0, 0.0),
            (0.0, 0.0),
        ]],
    },
    Glyph {
        ch: '6',
        w: 2.0,
        strokes: &[&[
            (2.0, 4.0),
            (0.0, 4.0),
            (0.0, 0.0),
            (2.0, 0.0),
            (2.0, 2.0),
            (0.0, 2.0),
        ]],
    },
    Glyph {
        ch: '7',
        w: 2.0,
        strokes: &[&[(0.0, 4.0), (2.0, 4.0), (2.0, 0.0)]],
    },
    Glyph {
        ch: '8',
        w: 2.0,
        strokes: &[
            &[(0.0, 0.0), (2.0, 0.0), (2.0, 4.0), (0.0, 4.0), (0.0, 0.0)],
            &[(0.0, 2.0), (2.0, 2.0)],
        ],
    },
    Glyph {
        ch: '9',
        w: 2.0,
        strokes: &[&[
            (2.0, 2.0),
            (0.0, 2.0),
            (0.0, 4.0),
            (2.0, 4.0),
            (2.0, 0.0),
            (0.0, 0.0),
        ]],
    },
    Glyph {
        ch: '.',
        w: 0.6,
        strokes: &[&[(0.0, 0.0), (0.6, 0.0), (0.6, 0.6), (0.0, 0.6), (0.0, 0.0)]],
    },
    Glyph {
        ch: '-',
        w: 2.0,
        strokes: &[&[(0.0, 2.0), (2.0, 2.0)]],
    },
    Glyph {
        ch: '/',
        w: 2.0,
        strokes: &[&[(0.0, 0.0), (2.0, 4.0)]],
    },
    Glyph {
        ch: '%',
        w: 2.0,
        strokes: &[
            &[(0.0, 0.0), (2.0, 4.0)],
            &[(0.0, 3.0), (0.8, 3.0), (0.8, 4.0), (0.0, 4.0), (0.0, 3.0)],
            &[(1.2, 0.0), (2.0, 0.0), (2.0, 1.0), (1.2, 1.0), (1.2, 0.0)],
        ],
    },
    Glyph {
        ch: 'A',
        w: 2.0,
        strokes: &[
            &[(0.0, 0.0), (0.0, 3.0), (1.0, 4.0), (2.0, 3.0), (2.0, 0.0)],
            &[(0.0, 2.0), (2.0, 2.0)],
        ],
    },
    Glyph {
        ch: 'I',
        w: 2.0,
        strokes: &[
            &[(0.0, 4.0), (2.0, 4.0)],
            &[(1.0, 4.0), (1.0, 0.0)],
            &[(0.0, 0.0), (2.0, 0.0)],
        ],
    },
    Glyph {
        ch: 'M',
        w: 2.0,
        strokes: &[&[(0.0, 0.0), (0.0, 4.0), (1.0, 2.0), (2.0, 4.0), (2.0, 0.0)]],
    },
    Glyph {
        ch: 'N',
        w: 2.0,
        strokes: &[&[(0.0, 0.0), (0.0, 4.0), (2.0, 0.0), (2.0, 4.0)]],
    },
    Glyph {
        ch: 'P',
        w: 2.0,
        strokes: &[&[(0.0, 0.0), (0.0, 4.0), (2.0, 4.0), (2.0, 2.0), (0.0, 2.0)]],
    },
    Glyph {
        ch: 'S',
        w: 2.0,
        strokes: &[&[
            (2.0, 4.0),
            (0.0, 4.0),
            (0.0, 2.0),
            (2.0, 2.0),
            (2.0, 0.0),
            (0.0, 0.0),
        ]],
    },
    Glyph {
        ch: 'T',
        w: 2.0,
        strokes: &[&[(0.0, 4.0), (2.0, 4.0)], &[(1.0, 4.0), (1.0, 0.0)]],
    },
    Glyph {
        ch: 'V',
        w: 2.0,
        strokes: &[&[(0.0, 4.0), (1.0, 0.0), (2.0, 4.0)]],
    },
];

/// Separación entre letras, en unidades de la cuadrícula.
const LETTER_SPACING: f64 = 1.0;

fn glyph(ch: char) -> Option<&'static Glyph> {
    GLYPHS.iter().find(|g| g.ch == ch)
}

/// Ancho en mm del texto a la altura `height`. Los caracteres que la fuente no tiene se omiten.
fn text_width(text: &str, height: f64) -> f64 {
    let unit = height / 4.0;
    let mut units = 0.0;
    for g in text.chars().filter_map(glyph) {
        if units > 0.0 {
            units += LETTER_SPACING;
        }
        units += g.w;
    }
    units * unit
}

/// Trazos del texto con la esquina inferior izquierda en `(x, y)`; un `BezPath` por trazo.
fn text_paths(text: &str, x: f64, y: f64, height: f64) -> Vec<BezPath> {
    let unit = height / 4.0;
    let mut cursor = 0.0;
    let mut out = Vec::new();
    for g in text.chars().filter_map(glyph) {
        for stroke in g.strokes {
            let poly = Polyline {
                points: stroke
                    .iter()
                    .map(|&(px, py)| Point::new(x + (cursor + px) * unit, y + py * unit))
                    .collect(),
                closed: false,
            };
            out.push(BezPath::from(&poly));
        }
        cursor += g.w + LETTER_SPACING;
    }
    out
}

// ---------------------------------------------------------------------------
// Valores y nombres
// ---------------------------------------------------------------------------

/// Número con hasta `decimals` decimales y sin ceros sobrantes.
fn trim_number(v: f64, decimals: usize) -> String {
    let s = format!("{v:.decimals$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

fn format_value(param: TestParam, v: f64) -> String {
    match param {
        TestParam::Speed => trim_number(v, 0),
        TestParam::Power => trim_number(v, 1),
        TestParam::Passes => trim_number(v, 0),
        TestParam::Interval => trim_number(v, 3),
    }
}

/// Fragmento del nombre de capa: "P 40%", "V 1200", "Pas 2", "Int 0.1".
fn short_name(param: TestParam, v: f64) -> String {
    let n = format_value(param, v);
    match param {
        TestParam::Speed => format!("V {n}"),
        TestParam::Power => format!("P {n}%"),
        TestParam::Passes => format!("Pas {n}"),
        TestParam::Interval => format!("Int {n}"),
    }
}

/// Encabezado del eje en la fuente de trazo: nombre del parámetro y su unidad.
fn axis_header(param: TestParam) -> &'static str {
    match param {
        TestParam::Speed => "V MM/MIN",
        TestParam::Power => "P %",
        TestParam::Passes => "PAS",
        TestParam::Interval => "INT MM",
    }
}

fn param_label(param: TestParam) -> &'static str {
    match param {
        TestParam::Speed => "la velocidad",
        TestParam::Power => "la potencia",
        TestParam::Passes => "las pasadas",
        TestParam::Interval => "el intervalo",
    }
}

/// Valores del eje: interpolación lineal de `from` a `to`, inclusive. Con un solo valor, `from`.
/// Las pasadas se redondean a un entero de al menos 1.
fn axis_values(axis: &TestAxis) -> Vec<f64> {
    (0..axis.count)
        .map(|i| {
            let v = if axis.count == 1 {
                axis.from
            } else {
                axis.from + (axis.to - axis.from) * f64::from(i) / f64::from(axis.count - 1)
            };
            if axis.param == TestParam::Passes {
                v.round().max(1.0)
            } else {
                v
            }
        })
        .collect()
}

fn apply_param(layer: &mut LayerSettings, param: TestParam, v: f64) {
    match param {
        TestParam::Speed => layer.speed = v,
        TestParam::Power => layer.power = v,
        TestParam::Passes => layer.passes = v as u32,
        TestParam::Interval => layer.interval = v,
    }
}

// ---------------------------------------------------------------------------
// Validación
// ---------------------------------------------------------------------------

fn check_axis(axis: &TestAxis, which: &str, mode: LayerMode) -> Result<(), String> {
    if axis.count < 1 || axis.count > MAX_COUNT {
        return Err(format!(
            "Las {which} deben ser entre 1 y {MAX_COUNT} (se pidieron {}).",
            axis.count
        ));
    }
    if !axis.from.is_finite() || !axis.to.is_finite() {
        return Err(format!(
            "El rango de las {which} contiene valores no válidos."
        ));
    }
    if axis.param == TestParam::Interval && mode == LayerMode::Line {
        return Err(format!(
            "El intervalo no se usa en modo line; elige otro parámetro para las {which} o cambia a relleno."
        ));
    }
    let what = param_label(axis.param);
    for v in [axis.from, axis.to] {
        match axis.param {
            TestParam::Power if !(0.0..=100.0).contains(&v) => {
                return Err(format!(
                    "La potencia de las {which} debe estar entre 0 y 100 % (se pidió {v})."
                ));
            }
            TestParam::Speed | TestParam::Interval if v <= 0.0 => {
                return Err(format!(
                    "Para {what} de las {which} se necesita un valor mayor que 0 (se pidió {v})."
                ));
            }
            TestParam::Passes if v.round() > f64::from(MAX_PASSES) => {
                return Err(format!(
                    "Las pasadas de las {which} no pueden pasar de {MAX_PASSES} (se pidió {v})."
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate(p: &tlaku_doc::MaterialTestParams) -> Result<(), String> {
    match p.mode {
        LayerMode::Line | LayerMode::Fill => {}
        LayerMode::Image => {
            return Err("La prueba de materiales solo admite los modos line y fill.".into());
        }
    }
    if p.rows.param == p.cols.param {
        return Err("Las filas y las columnas deben variar parámetros distintos.".into());
    }
    check_axis(&p.rows, "filas", p.mode)?;
    check_axis(&p.cols, "columnas", p.mode)?;
    if !(p.cell[0].is_finite() && p.cell[1].is_finite() && p.cell[0] > 0.0 && p.cell[1] > 0.0) {
        return Err("El tamaño de la celda debe ser mayor que 0 mm.".into());
    }
    if !p.gap.is_finite() || p.gap < 0.0 {
        return Err("La separación entre celdas no puede ser negativa.".into());
    }
    if !p.origin.iter().all(|v| v.is_finite()) {
        return Err("El origen de la cuadrícula no es válido.".into());
    }
    if p.labels {
        if !(p.label_speed.is_finite() && p.label_speed > 0.0) {
            return Err("La velocidad de los rótulos debe ser mayor que 0.".into());
        }
        if !(0.0..=100.0).contains(&p.label_power) {
            return Err("La potencia de los rótulos debe estar entre 0 y 100 %.".into());
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Generador
// ---------------------------------------------------------------------------

fn rect_path(x: f64, y: f64, w: f64, h: f64) -> BezPath {
    BezPath::from(&Polyline {
        points: vec![
            Point::new(x, y),
            Point::new(x + w, y),
            Point::new(x + w, y + h),
            Point::new(x, y + h),
        ],
        closed: true,
    })
}

fn vector_object(id: String, layer: Rgb, paths: Vec<BezPath>) -> DesignObject {
    DesignObject {
        id,
        layer,
        transform: IDENTITY,
        kind: ObjectKind::Vector(VectorObject { paths }),
        locked: false,
        hidden: false,
        group: None,
    }
}

/// Color `n` de una secuencia de tonos bien separados (razón áurea) con saturación y brillo
/// alternados. Nunca es negro, así que no choca con el de los rótulos.
fn hsv_color(n: u32) -> Rgb {
    let hue = (f64::from(n) * 0.618_033_988_749_895).fract() * 6.0;
    let s = [0.95, 0.7, 0.85][(n % 3) as usize];
    let v = [1.0, 0.8, 0.6, 0.9][(n % 4) as usize];
    let c = v * s;
    let x = c * (1.0 - (hue % 2.0 - 1.0).abs());
    let (r, g, b) = match hue as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    let byte = |f: f64| ((f + m) * 255.0).round() as u8;
    Rgb(byte(r), byte(g), byte(b))
}

/// Colores únicos para `count` celdas, distintos del de los rótulos.
fn unique_colors(count: usize) -> Vec<Rgb> {
    let mut used: HashSet<(u8, u8, u8)> = HashSet::new();
    used.insert((LABEL_COLOR.0, LABEL_COLOR.1, LABEL_COLOR.2));
    let mut out = Vec::with_capacity(count);
    let mut n = 0;
    while out.len() < count {
        let c = hsv_color(n);
        n += 1;
        if used.insert((c.0, c.1, c.2)) {
            out.push(c);
        }
    }
    out
}

/// Documento con la cuadrícula de prueba lista para enviarse.
pub fn material_test(params: &tlaku_doc::MaterialTestParams) -> Result<Document, String> {
    validate(params)?;
    let rows = axis_values(&params.rows);
    let cols = axis_values(&params.cols);
    let [cw, ch] = params.cell;
    let gap = params.gap;

    // Textos de rótulos (se miden antes para reservar el margen izquierdo).
    let col_texts: Vec<String> = cols
        .iter()
        .map(|&v| format_value(params.cols.param, v))
        .collect();
    let row_texts: Vec<String> = rows
        .iter()
        .map(|&v| format_value(params.rows.param, v))
        .collect();
    let row_header = axis_header(params.rows.param);
    let col_header = axis_header(params.cols.param);

    let mut left_margin = 0.0;
    if params.labels {
        let widest = |texts: &[String]| {
            texts
                .iter()
                .map(|s| text_width(s, LABEL_HEIGHT))
                .fold(0.0, f64::max)
        };
        if widest(&col_texts) > cw + gap - 0.5 {
            return Err(format!(
                "Con rótulos, la celda debe medir al menos {:.1} mm de ancho (o aumenta la separación).",
                widest(&col_texts) - gap + 0.5
            ));
        }
        if ch + gap < LABEL_HEIGHT + 0.5 {
            return Err(format!(
                "Con rótulos, la celda debe medir al menos {:.1} mm de alto (o aumenta la separación).",
                LABEL_HEIGHT + 0.5 - gap
            ));
        }
        left_margin = widest(&row_texts).max(text_width(row_header, LABEL_HEIGHT)) + LABEL_GAP;
    }

    let grid_x = params.origin[0] + left_margin;
    let grid_y = params.origin[1];
    let grid_h = f64::from(params.rows.count) * ch + f64::from(params.rows.count - 1) * gap;

    let colors = unique_colors(rows.len() * cols.len());
    let mut objects = Vec::new();
    let mut layers = Vec::new();

    for (i, &rv) in rows.iter().enumerate() {
        for (j, &cv) in cols.iter().enumerate() {
            let color = colors[i * cols.len() + j];
            let x = grid_x + j as f64 * (cw + gap);
            // La fila 0 es la de arriba.
            let y = grid_y + (rows.len() - 1 - i) as f64 * (ch + gap);
            let mut layer = LayerSettings {
                color,
                name: format!(
                    "{} · {}",
                    short_name(params.rows.param, rv),
                    short_name(params.cols.param, cv)
                ),
                mode: params.mode,
                output: true,
                visible: true,
                order: None,
                ..params.base.clone()
            };
            apply_param(&mut layer, params.rows.param, rv);
            apply_param(&mut layer, params.cols.param, cv);
            layers.push(layer);
            objects.push(vector_object(
                format!("cell-{}-{}", i + 1, j + 1),
                color,
                vec![rect_path(x, y, cw, ch)],
            ));
        }
    }

    if params.labels {
        let mut label_layer = LayerSettings::for_color(LABEL_COLOR);
        label_layer.name = "Rótulos".into();
        label_layer.mode = LayerMode::Line;
        label_layer.speed = params.label_speed;
        label_layer.power = params.label_power;
        layers.push(label_layer);

        let header_y = grid_y + grid_h + LABEL_GAP + LABEL_HEIGHT + LABEL_GAP;
        let mut label = |id: String, text: &str, x: f64, y: f64| {
            let paths = text_paths(text, x, y, LABEL_HEIGHT);
            if !paths.is_empty() {
                objects.push(vector_object(id, LABEL_COLOR, paths));
            }
        };
        // Encabezados: parámetro de las filas arriba a la izquierda y el de las columnas sobre
        // la primera columna.
        label(
            "label-rows-header".into(),
            row_header,
            params.origin[0],
            header_y,
        );
        label("label-cols-header".into(), col_header, grid_x, header_y);
        // Valores de las columnas, centrados sobre cada una y justo encima de la cuadrícula.
        let values_y = grid_y + grid_h + LABEL_GAP;
        for (j, text) in col_texts.iter().enumerate() {
            let center = grid_x + j as f64 * (cw + gap) + cw / 2.0;
            label(
                format!("label-col-{}", j + 1),
                text,
                center - text_width(text, LABEL_HEIGHT) / 2.0,
                values_y,
            );
        }
        // Valores de las filas, alineados a la derecha, centrados en la altura de cada fila.
        for (i, text) in row_texts.iter().enumerate() {
            let y = grid_y + (rows.len() - 1 - i) as f64 * (ch + gap) + ch / 2.0;
            label(
                format!("label-row-{}", i + 1),
                text,
                grid_x - LABEL_GAP - text_width(text, LABEL_HEIGHT),
                y - LABEL_HEIGHT / 2.0,
            );
        }
    }

    Ok(Document {
        version: DOC_VERSION,
        objects,
        layers,
        device_id: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_doc::{DeviceProfile, JobOptions, MaterialTestParams};

    fn axis(param: TestParam, count: u32, from: f64, to: f64) -> TestAxis {
        TestAxis {
            param,
            count,
            from,
            to,
        }
    }

    fn params() -> MaterialTestParams {
        MaterialTestParams {
            mode: LayerMode::Line,
            rows: axis(TestParam::Power, 3, 20.0, 60.0),
            cols: axis(TestParam::Speed, 4, 600.0, 1800.0),
            cell: [10.0, 10.0],
            gap: 3.0,
            base: LayerSettings::default(),
            labels: false,
            label_speed: 1500.0,
            label_power: 15.0,
            origin: [10.0, 10.0],
        }
    }

    fn bbox(obj: &DesignObject) -> tlaku_core::Bounds {
        obj.bounds().expect("el objeto dibuja algo")
    }

    #[test]
    fn una_capa_y_un_objeto_por_celda() {
        let doc = material_test(&params()).unwrap();
        assert_eq!(doc.version, 2);
        assert_eq!(doc.objects.len(), 12);
        assert_eq!(doc.layers.len(), 12);
        let colors: HashSet<_> = doc.layers.iter().map(|l| l.color).collect();
        assert_eq!(colors.len(), 12, "los colores deben ser únicos");
        for o in &doc.objects {
            assert!(doc.layer(o.layer).is_some());
        }
    }

    #[test]
    fn interpola_linealmente_e_incluye_los_extremos() {
        let doc = material_test(&params()).unwrap();
        let powers: Vec<f64> = (0..3).map(|i| doc.layers[i * 4].power).collect();
        assert_eq!(powers, [20.0, 40.0, 60.0]);
        let speeds: Vec<f64> = doc.layers[..4].iter().map(|l| l.speed).collect();
        assert_eq!(speeds, [600.0, 1000.0, 1400.0, 1800.0]);
        assert_eq!(doc.layers[0].name, "P 20% · V 600");
        assert_eq!(doc.layers[5].name, "P 40% · V 1000");
    }

    #[test]
    fn la_base_se_conserva_y_con_un_valor_se_usa_from() {
        let mut p = params();
        p.base.air = true;
        p.base.kerf = 0.1;
        p.rows = axis(TestParam::Power, 1, 35.0, 90.0);
        let doc = material_test(&p).unwrap();
        assert_eq!(doc.layers.len(), 4);
        assert!(
            doc.layers
                .iter()
                .all(|l| l.air && l.kerf == 0.1 && l.power == 35.0)
        );
    }

    #[test]
    fn las_pasadas_se_redondean_a_entero_de_al_menos_uno() {
        let mut p = params();
        p.cols = axis(TestParam::Passes, 4, 0.2, 3.0);
        let doc = material_test(&p).unwrap();
        let passes: Vec<u32> = doc.layers[..4].iter().map(|l| l.passes).collect();
        // 0.2, 1.07, 1.93, 3.0 -> 1, 1, 2, 3
        assert_eq!(passes, [1, 1, 2, 3]);
    }

    #[test]
    fn las_celdas_van_en_cuadricula_con_la_fila_cero_arriba() {
        let doc = material_test(&params()).unwrap();
        let first = bbox(&doc.objects[0]);
        assert_eq!((first.min.x, first.max.x), (10.0, 20.0));
        // Fila 0 de 3: la más alta.
        assert_eq!((first.min.y, first.max.y), (36.0, 46.0));
        let last = bbox(&doc.objects[11]);
        assert_eq!((last.min.x, last.max.x), (49.0, 59.0));
        assert_eq!((last.min.y, last.max.y), (10.0, 20.0));
    }

    #[test]
    fn el_modo_fill_se_aplica_a_todas_las_celdas() {
        let mut p = params();
        p.mode = LayerMode::Fill;
        p.base.interval = 0.2;
        p.cols = axis(TestParam::Interval, 2, 0.1, 0.3);
        let doc = material_test(&p).unwrap();
        assert!(doc.layers.iter().all(|l| l.mode == LayerMode::Fill));
        assert_eq!(doc.layers[0].interval, 0.1);
        assert_eq!(doc.layers[1].interval, 0.3);
        assert!(doc.layers[0].name.contains("Int 0.1"));
    }

    #[test]
    fn los_rotulos_van_en_su_propia_capa_line() {
        let mut p = params();
        p.labels = true;
        let doc = material_test(&p).unwrap();
        assert_eq!(doc.layers.len(), 13);
        let label_layer = doc.layers.last().unwrap();
        assert_eq!(label_layer.color, LABEL_COLOR);
        assert_eq!(label_layer.mode, LayerMode::Line);
        assert_eq!(label_layer.speed, 1500.0);
        assert_eq!(label_layer.power, 15.0);
        // Ninguna celda usa el color de los rótulos.
        assert!(doc.layers[..12].iter().all(|l| l.color != LABEL_COLOR));
        let labels: Vec<_> = doc
            .objects
            .iter()
            .filter(|o| o.layer == LABEL_COLOR)
            .collect();
        // 4 columnas + 3 filas + 2 encabezados.
        assert_eq!(labels.len(), 9);
        let cells_top = doc.objects[..12]
            .iter()
            .map(|o| bbox(o).max.y)
            .fold(f64::MIN, f64::max);
        let cells_left = doc.objects[..12]
            .iter()
            .map(|o| bbox(o).min.x)
            .fold(f64::MAX, f64::min);
        for o in labels {
            let b = bbox(o);
            assert!(b.max.x - b.min.x > 0.0);
            assert!(
                b.min.x >= p.origin[0] - 1e-9,
                "{} se sale por la izquierda",
                o.id
            );
            assert!(b.min.y >= p.origin[1] - 1e-9);
            let over_grid = o.id.starts_with("label-col") || o.id.contains("header");
            let beside_grid = o.id.starts_with("label-row-");
            if over_grid {
                assert!(
                    b.min.y >= cells_top,
                    "{} debe ir arriba de la cuadrícula",
                    o.id
                );
            }
            if beside_grid {
                assert!(b.max.x <= cells_left, "{} debe ir a la izquierda", o.id);
            }
            // Alto de unos 3 mm como máximo.
            assert!(b.max.y - b.min.y <= LABEL_HEIGHT + 1e-9);
        }
    }

    #[test]
    fn los_rotulos_no_estan_sin_labels() {
        let doc = material_test(&params()).unwrap();
        assert!(doc.objects.iter().all(|o| o.layer != LABEL_COLOR));
    }

    #[test]
    fn la_fuente_cubre_todo_lo_que_se_rotula() {
        for param in [
            TestParam::Speed,
            TestParam::Power,
            TestParam::Passes,
            TestParam::Interval,
        ] {
            for ch in axis_header(param).chars().chain("0123456789.".chars()) {
                assert!(glyph(ch).is_some(), "falta el glifo '{ch}'");
            }
        }
        // Todo glifo tiene trazos con al menos dos puntos y dentro de su caja.
        for g in GLYPHS {
            for s in g.strokes {
                assert!(s.len() >= 2, "trazo degenerado en '{}'", g.ch);
                assert!(
                    s.iter()
                        .all(|&(x, y)| (0.0..=g.w).contains(&x) && (0.0..=4.0).contains(&y))
                );
            }
        }
        // 3 mm de alto: "12" mide 2+1+2 = 5 unidades de 0.75 mm.
        assert!((text_width("12", 3.0) - 3.75).abs() < 1e-9);
    }

    #[test]
    fn rechaza_entradas_invalidas() {
        let err = |f: &dyn Fn(&mut MaterialTestParams)| {
            let mut p = params();
            f(&mut p);
            material_test(&p).unwrap_err()
        };
        assert!(err(&|p| p.rows.count = 0).contains("filas"));
        assert!(err(&|p| p.cols.count = 31).contains("columnas"));
        assert!(err(&|p| p.cell = [0.0, 5.0]).contains("celda"));
        assert!(err(&|p| p.cell = [5.0, f64::NAN]).contains("celda"));
        assert!(err(&|p| p.gap = -1.0).contains("separación"));
        assert!(err(&|p| p.rows.to = 120.0).contains("0 y 100"));
        assert!(err(&|p| p.rows.from = -5.0).contains("0 y 100"));
        assert!(err(&|p| p.cols.from = 0.0).contains("mayor que 0"));
        assert!(err(&|p| p.cols.to = f64::INFINITY).contains("no válidos"));
        assert!(err(&|p| p.cols.param = TestParam::Power).contains("distintos"));
        assert!(err(&|p| p.cols = axis(TestParam::Interval, 3, 0.1, 0.3)).contains("line"));
        assert!(err(&|p| p.mode = LayerMode::Image).contains("line y fill"));
        assert!(err(&|p| p.cols = axis(TestParam::Passes, 3, 1.0, 5000.0)).contains("pasadas"));
        assert!(
            err(&|p| {
                p.labels = true;
                p.label_power = 150.0;
            })
            .contains("rótulos")
        );
        assert!(
            err(&|p| {
                p.labels = true;
                p.cell = [4.0, 10.0];
                p.gap = 0.0;
            })
            .contains("ancho")
        );
    }

    #[test]
    fn el_limite_de_30_por_eje_es_valido() {
        let mut p = params();
        p.rows.count = 30;
        p.cols.count = 30;
        let doc = material_test(&p).unwrap();
        assert_eq!(doc.objects.len(), 900);
        let colors: HashSet<_> = doc.layers.iter().map(|l| l.color).collect();
        assert_eq!(colors.len(), 900);
    }

    #[test]
    fn el_pipeline_acepta_el_documento() {
        let mut p = params();
        p.labels = true;
        let doc = material_test(&p).unwrap();
        let job = crate::compute(&doc, &DeviceProfile::default(), &JobOptions::default())
            .expect("el documento de prueba se calcula");
        assert!(!job.toolpath.blocks.is_empty());
        assert!(job.preview.total_s > 0.0);

        // También en modo fill y sin rótulos.
        p.mode = LayerMode::Fill;
        p.labels = false;
        p.base.interval = 0.5;
        let doc = material_test(&p).unwrap();
        crate::compute(&doc, &DeviceProfile::default(), &JobOptions::default()).unwrap();
    }

    #[test]
    fn el_documento_sobrevive_a_json() {
        let mut p = params();
        p.labels = true;
        let doc = material_test(&p).unwrap();
        let back = Document::from_json(&doc.to_json().unwrap()).unwrap();
        assert_eq!(back, doc);
    }
}
