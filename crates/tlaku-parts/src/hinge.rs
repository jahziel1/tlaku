//! Bisagras flexibles (living hinge): una zona rectangular con filas de cortes alternados que
//! deja flexionar la madera o el acrílico.
//!
//! Los cortes de cada fila corren a lo largo del eje de la zona (el que dice `orientation`) y
//! entre dos cortes queda un **puente** sin cortar. Las filas pares empiezan centradas en un
//! corte y las impares en un puente (medio paso de desfase), con el patrón simétrico respecto al
//! centro. Los cortes que sobrepasarían el margen se recortan y los restos muy cortos se
//! descartan. Todos los cortes de una fila son la misma curva trasladada, así que ninguno se
//! cruza con otro. La geometría es nominal: son líneas sin ancho (el kerf lo pone el haz).

use crate::error::PartsError;
use crate::part::{Generated, Part};
use crate::registry::ParamSpec;
use serde::{Deserialize, Serialize};
use tlaku_core::{Point, Polyline};
use tlaku_i18n::tr;

/// Forma de cada corte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HingePattern {
    /// Rectas escalonadas: el patrón clásico.
    Straight,
    /// Cortes ondulados (senoide).
    Wave,
    /// Cortes en zigzag (triangular); más flexible a lo largo de los cortes.
    Zigzag,
}

/// Dirección de los cortes dentro de la zona.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    /// Cortes paralelos al eje X (la zona se curva alrededor de X).
    Horizontal,
    /// Cortes paralelos al eje Y.
    Vertical,
}

/// Parámetros de la bisagra.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HingeParams {
    pub width_mm: f64,
    pub height_mm: f64,
    pub pattern: HingePattern,
    pub orientation: Orientation,
    /// Largo de cada corte medido a lo largo de su eje.
    pub cut_length_mm: f64,
    /// Puente sin cortar entre dos cortes de la misma fila.
    pub bridge_mm: f64,
    /// Separación entre filas, medida perpendicular a los cortes.
    pub row_gap_mm: f64,
    /// Franja sin cortes junto al borde de la zona.
    pub margin_mm: f64,
    /// Altura de la onda (solo `wave` y `zigzag`): de pico a eje.
    pub amplitude_mm: f64,
    /// Longitud de onda (solo `wave` y `zigzag`).
    pub wavelength_mm: f64,
    /// Radio de las esquinas del contorno.
    pub corner_radius_mm: f64,
    /// Incluir el contorno rectangular como corte.
    pub outline: bool,
}

impl Default for HingeParams {
    fn default() -> Self {
        Self {
            width_mm: 100.0,
            height_mm: 50.0,
            pattern: HingePattern::Straight,
            orientation: Orientation::Horizontal,
            cut_length_mm: 20.0,
            bridge_mm: 3.0,
            row_gap_mm: 2.0,
            margin_mm: 3.0,
            amplitude_mm: 1.5,
            wavelength_mm: 10.0,
            corner_radius_mm: 0.0,
            outline: true,
        }
    }
}

/// Campos del formulario de la bisagra.
pub fn schema() -> Vec<ParamSpec> {
    let d = HingeParams::default();
    let wavy = ["wave", "zigzag"];
    vec![
        ParamSpec::choice(
            "pattern",
            "pattern",
            &["straight", "wave", "zigzag"],
            "straight",
        ),
        ParamSpec::choice(
            "orientation",
            "pattern",
            &["horizontal", "vertical"],
            "horizontal",
        ),
        ParamSpec::length("width_mm", "size", 5.0, 2000.0, 1.0, d.width_mm),
        ParamSpec::length("height_mm", "size", 5.0, 2000.0, 1.0, d.height_mm),
        ParamSpec::length("cut_length_mm", "cuts", 1.0, 500.0, 0.5, d.cut_length_mm),
        ParamSpec::length("bridge_mm", "cuts", 0.2, 100.0, 0.1, d.bridge_mm),
        ParamSpec::length("row_gap_mm", "cuts", 0.4, 50.0, 0.1, d.row_gap_mm),
        ParamSpec::length("margin_mm", "cuts", 0.0, 200.0, 0.5, d.margin_mm),
        ParamSpec::length("amplitude_mm", "wave", 0.0, 50.0, 0.1, d.amplitude_mm)
            .when("pattern", &wavy),
        ParamSpec::length("wavelength_mm", "wave", 1.0, 500.0, 0.5, d.wavelength_mm)
            .when("pattern", &wavy),
        ParamSpec::length(
            "corner_radius_mm",
            "outline",
            0.0,
            1000.0,
            0.5,
            d.corner_radius_mm,
        ),
        ParamSpec::boolean("outline", "outline", d.outline),
    ]
}

/// Tope de cortes de una bisagra: más solo haría lenta la vista previa y el trabajo.
const MAX_SLITS: usize = 20_000;

/// Un corte más corto que esto (mm) es un resto sin sentido.
const MIN_SEGMENT_MM: f64 = 0.8;
/// Separación mínima entre dos cortes vecinos (mm) para que el puente aguante.
const MIN_WEB_MM: f64 = 0.3;

fn range(field: &str, v: f64, lo: f64, hi: f64) -> Result<(), PartsError> {
    if v.is_nan() || !(lo..=hi).contains(&v) {
        return Err(PartsError::field(
            field,
            tr!("parts.hinge.range", min = lo, max = hi),
        ));
    }
    Ok(())
}

impl HingeParams {
    fn validate(&self) -> Result<(), PartsError> {
        range("width_mm", self.width_mm, 5.0, 2000.0)?;
        range("height_mm", self.height_mm, 5.0, 2000.0)?;
        range("cut_length_mm", self.cut_length_mm, 1.0, 500.0)?;
        range("bridge_mm", self.bridge_mm, 0.2, 100.0)?;
        range("row_gap_mm", self.row_gap_mm, 0.4, 50.0)?;
        range("margin_mm", self.margin_mm, 0.0, 200.0)?;
        range("amplitude_mm", self.amplitude_mm, 0.0, 50.0)?;
        range("wavelength_mm", self.wavelength_mm, 1.0, 500.0)?;
        range("corner_radius_mm", self.corner_radius_mm, 0.0, 1000.0)?;
        if self.corner_radius_mm > self.width_mm.min(self.height_mm) / 2.0 {
            return Err(PartsError::field(
                "corner_radius_mm",
                tr!("parts.hinge.radius_too_big"),
            ));
        }
        // Los cortes más cercanos a una esquina redondeada tocan el contorno si el margen es menor.
        if self.outline
            && self.margin_mm
                < self.corner_radius_mm * (1.0 - std::f64::consts::FRAC_1_SQRT_2) + 0.1
            && self.corner_radius_mm > 0.0
        {
            return Err(PartsError::field(
                "margin_mm",
                tr!("parts.hinge.margin_vs_radius"),
            ));
        }
        Ok(())
    }

    fn wavy(&self) -> bool {
        self.pattern != HingePattern::Straight
    }
}

/// Onda de la forma pedida: `phase` en radianes, devuelve -1..1.
fn wave(pattern: HingePattern, phase: f64) -> f64 {
    match pattern {
        HingePattern::Straight => 0.0,
        HingePattern::Wave => phase.sin(),
        HingePattern::Zigzag => {
            // Triangular con picos en phase = ±pi/2.
            let x = (phase / std::f64::consts::TAU).rem_euclid(1.0);
            if x < 0.25 {
                4.0 * x
            } else if x < 0.75 {
                2.0 - 4.0 * x
            } else {
                4.0 * x - 4.0
            }
        }
    }
}

/// Contorno rectangular `[0, w] x [0, h]` antihorario, con esquinas redondeadas si `r > 0`.
pub fn rounded_outline(w: f64, h: f64, r: f64) -> Polyline {
    if r <= 1e-9 {
        return Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(w, 0.0),
                Point::new(w, h),
                Point::new(0.0, h),
            ],
            closed: true,
        };
    }
    // Pasos del arco para un error de 0,01 mm.
    let tol = 0.01_f64.min(r / 2.0);
    let step = 2.0 * (1.0 - tol / r).clamp(-1.0, 1.0).acos().max(0.02);
    let n = ((std::f64::consts::FRAC_PI_2 / step).ceil() as usize).max(2);
    let mut pts = Vec::new();
    // Centros de las esquinas en orden antihorario desde la inferior derecha.
    let corners = [
        (w - r, r, -std::f64::consts::FRAC_PI_2),
        (w - r, h - r, 0.0),
        (r, h - r, std::f64::consts::FRAC_PI_2),
        (r, r, std::f64::consts::PI),
    ];
    for (cx, cy, a0) in corners {
        for k in 0..=n {
            let a = a0 + std::f64::consts::FRAC_PI_2 * k as f64 / n as f64;
            pts.push(Point::new(cx + r * a.cos(), cy + r * a.sin()));
        }
    }
    Polyline {
        points: pts,
        closed: true,
    }
}

/// Cortes de la zona en el sistema "cortes paralelos a X" (el vertical se transpone después).
fn slits(p: &HingeParams, w: f64, h: f64) -> Vec<Polyline> {
    let amp = if p.wavy() { p.amplitude_mm } else { 0.0 };
    let (x_lo, x_hi) = (p.margin_mm, w - p.margin_mm);
    let (y_lo, y_hi) = (p.margin_mm + amp, h - p.margin_mm - amp);
    if x_hi - x_lo < MIN_SEGMENT_MM || y_hi - y_lo < 0.0 {
        return Vec::new();
    }
    let rows = ((y_hi - y_lo) / p.row_gap_mm).floor() as usize + 1;
    let y_c = (y_lo + y_hi) / 2.0;
    let x_c = w / 2.0;
    let pitch = p.cut_length_mm + p.bridge_mm;
    let k_max = ((x_hi - x_lo) / pitch).ceil() as i64 + 1;
    let omega = std::f64::consts::TAU / p.wavelength_mm;
    let mut out = Vec::new();
    for r in 0..rows {
        let y0 = y_c + (r as f64 - (rows as f64 - 1.0) / 2.0) * p.row_gap_mm;
        // Filas pares: un corte centrado; impares: un puente centrado.
        let shift = if r % 2 == 0 { 0.0 } else { 0.5 };
        for k in -k_max..=k_max {
            let center = x_c + (k as f64 + shift) * pitch;
            let a = (center - p.cut_length_mm / 2.0).max(x_lo);
            let b = (center + p.cut_length_mm / 2.0).min(x_hi);
            if b - a < MIN_SEGMENT_MM {
                continue;
            }
            let y_at = |x: f64| y0 + amp * wave(p.pattern, omega * (x - x_c));
            let mut xs = vec![a];
            if p.wavy() {
                // Muestras: 24 por longitud de onda en la senoide; los picos en el zigzag.
                let step = match p.pattern {
                    HingePattern::Wave => p.wavelength_mm / 24.0,
                    _ => p.wavelength_mm / 4.0,
                };
                let first = ((a - x_c) / step).floor() as i64 + 1;
                let mut i = first;
                while x_c + i as f64 * step < b - 1e-9 {
                    let x = x_c + i as f64 * step;
                    if x > a + 1e-9 {
                        xs.push(x);
                    }
                    i += 1;
                }
            }
            xs.push(b);
            out.push(Polyline {
                points: xs.into_iter().map(|x| Point::new(x, y_at(x))).collect(),
                closed: false,
            });
        }
    }
    out
}

/// Genera la bisagra: una sola pieza con el contorno (opcional) y los cortes.
pub fn hinge_parts(p: &HingeParams) -> Result<Generated, PartsError> {
    p.validate()?;
    let vertical = p.orientation == Orientation::Vertical;
    // En el sistema de trabajo los cortes siempre corren a lo largo de X.
    let (w, h) = if vertical {
        (p.height_mm, p.width_mm)
    } else {
        (p.width_mm, p.height_mm)
    };
    let mut warnings = Vec::new();
    // Los cortes ondulados se acercan entre filas: la separación real es perpendicular.
    if p.wavy() {
        let slope = p.amplitude_mm * std::f64::consts::TAU / p.wavelength_mm;
        let web = p.row_gap_mm / (1.0 + slope * slope).sqrt();
        if web < MIN_WEB_MM {
            return Err(PartsError::field(
                "row_gap_mm",
                tr!("parts.hinge.rows_too_close", web = format!("{web:.2}")),
            ));
        }
    }
    let amp = if p.wavy() { p.amplitude_mm } else { 0.0 };
    let rows = (((h - 2.0 * p.margin_mm - 2.0 * amp) / p.row_gap_mm).max(0.0)).floor() + 1.0;
    let per_row = ((w - 2.0 * p.margin_mm) / (p.cut_length_mm + p.bridge_mm))
        .max(0.0)
        .ceil()
        + 1.0;
    if rows * per_row > MAX_SLITS as f64 {
        return Err(PartsError::general(tr!(
            "parts.hinge.too_many",
            max = MAX_SLITS
        )));
    }
    let mut cuts = slits(p, w, h);
    if cuts.is_empty() {
        return Err(PartsError::general(tr!("parts.hinge.no_room")));
    }
    if p.bridge_mm < 1.0 {
        warnings.push(tr!("parts.hinge.thin_bridge", bridge = p.bridge_mm));
    }
    if p.row_gap_mm < 1.0 {
        warnings.push(tr!("parts.hinge.thin_web", gap = p.row_gap_mm));
    }
    if vertical {
        for c in &mut cuts {
            for pt in &mut c.points {
                std::mem::swap(&mut pt.x, &mut pt.y);
            }
        }
    }
    let mut all = Vec::new();
    if p.outline {
        all.push(rounded_outline(p.width_mm, p.height_mm, p.corner_radius_mm));
    }
    all.extend(cuts);
    Ok(Generated {
        parts: vec![Part::new("hinge", tr!("parts.hinge.name"), all, Vec::new())],
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::crossings;

    fn slit_paths(g: &Generated) -> Vec<&Polyline> {
        g.parts[0].cuts.iter().filter(|c| !c.closed).collect()
    }

    fn all_params() -> Vec<HingeParams> {
        let base = HingeParams::default();
        let mut v = Vec::new();
        for pattern in [
            HingePattern::Straight,
            HingePattern::Wave,
            HingePattern::Zigzag,
        ] {
            for orientation in [Orientation::Horizontal, Orientation::Vertical] {
                v.push(HingeParams {
                    pattern,
                    orientation,
                    ..base.clone()
                });
                v.push(HingeParams {
                    pattern,
                    orientation,
                    width_mm: 63.7,
                    height_mm: 41.3,
                    cut_length_mm: 12.0,
                    bridge_mm: 2.2,
                    row_gap_mm: 1.7,
                    margin_mm: 4.0,
                    corner_radius_mm: 5.0,
                    ..base.clone()
                });
            }
        }
        v
    }

    #[test]
    fn cuts_stay_inside_the_margin() {
        for p in all_params() {
            let g = hinge_parts(&p).unwrap();
            let slits = slit_paths(&g);
            assert!(slits.len() > 10, "{p:?}");
            for s in slits {
                for q in &s.points {
                    let (along, across, a_len, c_len) = match p.orientation {
                        Orientation::Horizontal => (q.x, q.y, p.width_mm, p.height_mm),
                        Orientation::Vertical => (q.y, q.x, p.height_mm, p.width_mm),
                    };
                    assert!(
                        along >= p.margin_mm - 1e-9 && along <= a_len - p.margin_mm + 1e-9,
                        "{p:?}: {q:?}"
                    );
                    assert!(
                        across >= p.margin_mm - 1e-9 && across <= c_len - p.margin_mm + 1e-9,
                        "{p:?}: {q:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn no_cut_crosses_another_or_the_outline() {
        for p in all_params() {
            let g = hinge_parts(&p).unwrap();
            assert_eq!(crossings(&g.parts[0].cuts), 0, "{p:?}");
        }
    }

    #[test]
    fn rows_alternate_and_pattern_is_symmetric() {
        let p = HingeParams {
            outline: false,
            ..HingeParams::default()
        };
        let g = hinge_parts(&p).unwrap();
        let slits = slit_paths(&g);
        // Simetría respecto al centro vertical: el espejo de cada corte existe.
        let w = g.parts[0].width;
        let key = |a: f64, b: f64, y: f64| {
            (
                (a * 1000.0).round() as i64,
                (b * 1000.0).round() as i64,
                (y * 1000.0).round() as i64,
            )
        };
        let mut set = std::collections::HashSet::new();
        for s in &slits {
            let (a, b) = (s.points[0].x, s.points[1].x);
            set.insert(key(a, b, s.points[0].y));
        }
        // La caja puede haberse reducido al quitar el contorno: se compara con el centro de los cortes.
        let min_x = slits.iter().map(|s| s.points[0].x).fold(f64::MAX, f64::min);
        let max_x = slits.iter().map(|s| s.points[1].x).fold(f64::MIN, f64::max);
        let mid = (min_x + max_x) / 2.0;
        for s in &slits {
            let (a, b) = (s.points[0].x, s.points[1].x);
            assert!(
                set.contains(&key(2.0 * mid - b, 2.0 * mid - a, s.points[0].y)),
                "falta el espejo de {a}..{b}"
            );
        }
        assert!(w > 0.0);
        // Dos filas seguidas tienen sus cortes desfasados medio paso.
        let mut rows: Vec<f64> = slits
            .iter()
            .map(|s| (s.points[0].y * 1000.0).round() / 1000.0)
            .collect();
        rows.dedup();
        rows.sort_by(f64::total_cmp);
        rows.dedup();
        assert!(rows.len() >= 3);
        for pair in rows.windows(2) {
            assert!((pair[1] - pair[0] - p.row_gap_mm).abs() < 1e-6);
        }
        let starts = |y: f64| -> Vec<f64> {
            let mut v: Vec<f64> = slits
                .iter()
                .filter(|s| (s.points[0].y - y).abs() < 1e-6)
                .map(|s| s.points[0].x)
                .collect();
            v.sort_by(f64::total_cmp);
            v
        };
        let (r0, r1) = (starts(rows[0]), starts(rows[1]));
        let pitch = p.cut_length_mm + p.bridge_mm;
        // Un corte completo de una fila y uno de la otra están desfasados pitch/2.
        let full0: Vec<f64> = r0
            .iter()
            .copied()
            .filter(|&x| x > p.margin_mm + 1e-6)
            .collect();
        let full1: Vec<f64> = r1
            .iter()
            .copied()
            .filter(|&x| x > p.margin_mm + 1e-6)
            .collect();
        let d = (full1[0] - full0[0]).rem_euclid(pitch);
        assert!((d - pitch / 2.0).abs() < 1e-6, "desfase {d}");
    }

    #[test]
    fn bridges_and_lengths_match_the_request() {
        let p = HingeParams {
            outline: false,
            ..HingeParams::default()
        };
        let g = hinge_parts(&p).unwrap();
        let slits = slit_paths(&g);
        let y0 = slits[0].points[0].y;
        let mut row: Vec<(f64, f64)> = slits
            .iter()
            .filter(|s| (s.points[0].y - y0).abs() < 1e-9)
            .map(|s| (s.points[0].x, s.points[1].x))
            .collect();
        row.sort_by(|a, b| a.0.total_cmp(&b.0));
        for pair in row.windows(2) {
            assert!(
                (pair[1].0 - pair[0].1 - p.bridge_mm).abs() < 1e-9,
                "puente {:?}",
                pair
            );
        }
        // Los cortes interiores miden exactamente el largo pedido.
        for (a, b) in &row[1..row.len() - 1] {
            assert!((b - a - p.cut_length_mm).abs() < 1e-9);
        }
    }

    #[test]
    fn wave_cuts_follow_the_requested_amplitude() {
        let p = HingeParams {
            pattern: HingePattern::Wave,
            amplitude_mm: 2.0,
            wavelength_mm: 8.0,
            cut_length_mm: 30.0,
            ..HingeParams::default()
        };
        let g = hinge_parts(&p).unwrap();
        let slits = slit_paths(&g);
        let long = slits.iter().max_by_key(|s| s.points.len()).unwrap();
        let (lo, hi) = long
            .points
            .iter()
            .fold((f64::MAX, f64::MIN), |(l, h), q| (l.min(q.y), h.max(q.y)));
        assert!((hi - lo) > 3.0 && (hi - lo) <= 4.0 + 1e-9, "{lo}..{hi}");
        // Zigzag: vértices en los picos.
        let z = hinge_parts(&HingeParams {
            pattern: HingePattern::Zigzag,
            ..p.clone()
        })
        .unwrap();
        let zl = slit_paths(&z)
            .into_iter()
            .max_by_key(|s| s.points.len())
            .unwrap()
            .clone();
        let (lo, hi) = zl
            .points
            .iter()
            .fold((f64::MAX, f64::MIN), |(l, h), q| (l.min(q.y), h.max(q.y)));
        assert!((hi - lo - 4.0).abs() < 1e-9, "{lo}..{hi}");
    }

    #[test]
    fn outline_is_the_requested_rectangle() {
        let g = hinge_parts(&HingeParams::default()).unwrap();
        let part = &g.parts[0];
        assert!((part.width - 100.0).abs() < 1e-9 && (part.height - 50.0).abs() < 1e-9);
        let rounded = hinge_parts(&HingeParams {
            corner_radius_mm: 8.0,
            ..HingeParams::default()
        })
        .unwrap();
        assert!((rounded.parts[0].width - 100.0).abs() < 1e-9);
        let outline = &rounded.parts[0].cuts[0];
        assert!(outline.points.len() > 20);
        // Ningún punto del contorno redondeado cae a menos de r del centro de la esquina en el exterior.
        assert!(
            outline
                .points
                .iter()
                .all(|q| q.x >= -1e-9 && q.x <= 100.0 + 1e-9)
        );
        let none = hinge_parts(&HingeParams {
            outline: false,
            ..HingeParams::default()
        })
        .unwrap();
        assert!(none.parts[0].cuts.iter().all(|c| !c.closed));
    }

    #[test]
    fn invalid_values_name_the_field() {
        let bad = |p: HingeParams| hinge_parts(&p).unwrap_err();
        assert_eq!(
            bad(HingeParams {
                width_mm: 1.0,
                ..HingeParams::default()
            })
            .field
            .as_deref(),
            Some("width_mm")
        );
        assert_eq!(
            bad(HingeParams {
                bridge_mm: 0.0,
                ..HingeParams::default()
            })
            .field
            .as_deref(),
            Some("bridge_mm")
        );
        assert_eq!(
            bad(HingeParams {
                corner_radius_mm: 40.0,
                ..HingeParams::default()
            })
            .field
            .as_deref(),
            Some("corner_radius_mm")
        );
        assert_eq!(
            bad(HingeParams {
                margin_mm: 0.5,
                corner_radius_mm: 10.0,
                ..HingeParams::default()
            })
            .field
            .as_deref(),
            Some("margin_mm")
        );
        let tight = bad(HingeParams {
            pattern: HingePattern::Zigzag,
            amplitude_mm: 5.0,
            wavelength_mm: 4.0,
            row_gap_mm: 0.5,
            ..HingeParams::default()
        });
        assert_eq!(tight.field.as_deref(), Some("row_gap_mm"));
    }

    #[test]
    fn region_without_room_is_an_error() {
        let e = hinge_parts(&HingeParams {
            margin_mm: 26.0,
            ..HingeParams::default()
        });
        assert!(e.is_err(), "{e:?}");
    }

    #[test]
    fn absurd_sizes_are_rejected_instead_of_hanging() {
        let e = hinge_parts(&HingeParams {
            width_mm: 2000.0,
            height_mm: 2000.0,
            cut_length_mm: 1.0,
            bridge_mm: 0.2,
            row_gap_mm: 0.4,
            ..HingeParams::default()
        });
        assert!(e.is_err());
        let b = crate::boxes::box_parts(&crate::boxes::BoxParams {
            width_mm: 5000.0,
            finger_mm: 1.5,
            ..Default::default()
        });
        assert_eq!(b.unwrap_err().field.as_deref(), Some("finger_mm"));
    }

    #[test]
    fn thin_bridges_warn() {
        let g = hinge_parts(&HingeParams {
            bridge_mm: 0.6,
            ..HingeParams::default()
        })
        .unwrap();
        assert!(!g.warnings.is_empty());
    }
}
