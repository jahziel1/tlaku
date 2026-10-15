//! Pieza de prueba de encaje: una placa con ranuras de varios anchos y un macho para ver cuál
//! encaja en el material y la máquina reales.
//!
//! Hay dos modos. En **grosor** las ranuras miden `grosor + k·paso` y el macho se mete de canto
//! (sirve para uniones de pestaña en ranura). En **diente** las ranuras miden `diente + k·paso` y
//! el macho, de ese ancho, se mete plano (es el encaje de los dientes de una caja). El valor
//! grabado bajo cada ranura es `k·paso`: la diferencia ranura menos macho, que es lo que se
//! escribe como «holgura de encaje» en el generador de cajas.
//!
//! Se corta con la misma capa (velocidad, potencia y kerf) con que se cortará el trabajo real.

use crate::error::PartsError;
use crate::font;
use crate::part::{Generated, Part};
use crate::rectilinear::{Rect, rect_minus_rects};
use crate::registry::ParamSpec;
use serde::{Deserialize, Serialize};
use tlaku_core::{Point, Polyline};
use tlaku_i18n::tr;

/// Qué se mide con la placa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FitMode {
    /// Ranura contra el canto del material.
    Thickness,
    /// Ranura contra un diente cortado con el mismo láser.
    Finger,
}

/// Parámetros de la pieza de prueba.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FitTestParams {
    pub mode: FitMode,
    pub thickness_mm: f64,
    /// Ancho nominal del diente (solo en modo diente).
    pub finger_mm: f64,
    /// Diferencia de ancho entre una ranura y la siguiente.
    pub step_mm: f64,
    /// Ranuras de cada lado de la nominal (el total es `2·count + 1`).
    pub count: u32,
    /// Largo de cada ranura.
    pub slot_length_mm: f64,
    /// Grabar los valores bajo cada ranura.
    pub labels: bool,
}

impl Default for FitTestParams {
    fn default() -> Self {
        Self {
            mode: FitMode::Thickness,
            thickness_mm: 3.0,
            finger_mm: 10.0,
            step_mm: 0.05,
            count: 6,
            slot_length_mm: 12.0,
            labels: true,
        }
    }
}

/// Campos del formulario de la pieza de prueba.
pub fn schema() -> Vec<ParamSpec> {
    let d = FitTestParams::default();
    vec![
        ParamSpec::choice("mode", "test", &["thickness", "finger"], "thickness"),
        ParamSpec::length("thickness_mm", "test", 0.5, 20.0, 0.1, d.thickness_mm),
        ParamSpec::length("finger_mm", "test", 2.0, 100.0, 0.5, d.finger_mm)
            .when("mode", &["finger"]),
        ParamSpec::length("step_mm", "test", 0.01, 0.5, 0.01, d.step_mm),
        ParamSpec::int("count", "test", 1, 10, d.count as i32),
        ParamSpec::length("slot_length_mm", "test", 4.0, 60.0, 1.0, d.slot_length_mm),
        ParamSpec::boolean("labels", "options", d.labels),
    ]
}

fn range(field: &str, v: f64, lo: f64, hi: f64) -> Result<(), PartsError> {
    if v.is_nan() || !(lo..=hi).contains(&v) {
        return Err(PartsError::field(
            field,
            tr!("parts.fit.range", min = lo, max = hi),
        ));
    }
    Ok(())
}

impl FitTestParams {
    fn validate(&self) -> Result<(), PartsError> {
        range("thickness_mm", self.thickness_mm, 0.5, 20.0)?;
        range("finger_mm", self.finger_mm, 2.0, 100.0)?;
        range("step_mm", self.step_mm, 0.01, 0.5)?;
        range("count", f64::from(self.count), 1.0, 10.0)?;
        range("slot_length_mm", self.slot_length_mm, 4.0, 60.0)?;
        let narrowest = self.nominal() - f64::from(self.count) * self.step_mm;
        if narrowest < 0.3 {
            return Err(PartsError::field("step_mm", tr!("parts.fit.too_narrow")));
        }
        Ok(())
    }

    /// Ancho nominal de las ranuras.
    fn nominal(&self) -> f64 {
        match self.mode {
            FitMode::Thickness => self.thickness_mm,
            FitMode::Finger => self.finger_mm,
        }
    }

    /// Desfases `k·paso` de las ranuras, de menor a mayor.
    pub fn offsets(&self) -> Vec<f64> {
        let n = i64::from(self.count);
        (-n..=n)
            .map(|k| (k as f64 * self.step_mm * 1000.0).round() / 1000.0)
            .collect()
    }
}

/// Texto del desfase: `+0.05`, `-0.10`, `0.00`.
fn offset_label(v: f64, step: f64) -> String {
    let decimals = if (step * 100.0 - (step * 100.0).round()).abs() < 1e-9 {
        2
    } else {
        3
    };
    if v.abs() < 1e-9 {
        format!("{:.*}", decimals, 0.0)
    } else {
        format!("{:+.*}", decimals, v)
    }
}

/// Genera la placa de ranuras y el macho.
pub fn fit_test_parts(p: &FitTestParams) -> Result<Generated, PartsError> {
    p.validate()?;
    let offsets = p.offsets();
    let nominal = p.nominal();
    let widest = nominal + offsets[offsets.len() - 1];
    let label_h = 3.0;
    let texts: Vec<String> = offsets
        .iter()
        .map(|&o| offset_label(o, p.step_mm))
        .collect();
    let label_w = texts
        .iter()
        .map(|t| font::text_width(t, label_h))
        .fold(0.0, f64::max);
    let pitch = (widest + 5.0).max(label_w + 3.0).max(9.0);
    let margin = 5.0;
    let plate_w = 2.0 * margin + pitch * offsets.len() as f64;
    let slot_y0 = 11.0;
    let slot_y1 = slot_y0 + p.slot_length_mm;
    let plate_h = slot_y1 + 4.0 + label_h + 4.0;

    let mut holes = Vec::new();
    let mut engraves = Vec::new();
    for (i, (&o, text)) in offsets.iter().zip(&texts).enumerate() {
        let cx = margin + pitch * (i as f64 + 0.5);
        let w = nominal + o;
        holes.push(Rect::new(cx - w / 2.0, slot_y0, cx + w / 2.0, slot_y1));
        if p.labels {
            engraves.extend(font::text_centered(text, cx, 5.5, label_h));
        }
    }
    let head = match p.mode {
        FitMode::Thickness => format!("T={:.2}", nominal),
        FitMode::Finger => format!("F={:.2}", nominal),
    };
    if p.labels {
        engraves.extend(font::text_strokes(&head, margin, slot_y1 + 4.0, label_h));
    }
    let mut cuts = rect_minus_rects(plate_w, plate_h, &holes);
    // La región ya trae el contorno antihorario y los agujeros horarios.
    cuts.sort_by_key(|a| std::cmp::Reverse(a.points.len()));
    let plate = Part::new("plate", tr!("parts.fit.name.plate"), cuts, engraves);

    // Macho: barra de agarre con el diente encima.
    let (tab_w, tab_len) = match p.mode {
        FitMode::Thickness => (p.slot_length_mm - 1.5, 8.0),
        FitMode::Finger => (p.finger_mm, p.slot_length_mm.min(10.0)),
    };
    let grip_w = tab_w.max(p.slot_length_mm) + 8.0;
    let grip_h = 8.0;
    let (tx0, tx1) = ((grip_w - tab_w) / 2.0, (grip_w + tab_w) / 2.0);
    let outline = Polyline {
        points: vec![
            Point::new(0.0, 0.0),
            Point::new(grip_w, 0.0),
            Point::new(grip_w, grip_h),
            Point::new(tx1, grip_h),
            Point::new(tx1, grip_h + tab_len),
            Point::new(tx0, grip_h + tab_len),
            Point::new(tx0, grip_h),
            Point::new(0.0, grip_h),
        ],
        closed: true,
    };
    let mut tab_marks = Vec::new();
    if p.labels {
        let h = 3.0_f64.min(grip_h - 2.0);
        let w = font::text_width(&head, h);
        if w <= grip_w - 2.0 {
            tab_marks = font::text_centered(&head, grip_w / 2.0, grip_h / 2.0, h);
        }
    }
    let tab = Part::new("tab", tr!("parts.fit.name.tab"), vec![outline], tab_marks);

    let mut warnings = vec![match p.mode {
        FitMode::Thickness => tr!("parts.fit.howto_thickness"),
        FitMode::Finger => tr!("parts.fit.howto_finger"),
    }];
    if plate.width > 400.0 {
        warnings.push(tr!("parts.fit.wide", width = format!("{:.0}", plate.width)));
    }
    Ok(Generated {
        parts: vec![plate, tab],
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{crossings, inside};

    fn slots(g: &Generated) -> Vec<(f64, f64)> {
        // Los agujeros son los contornos horarios; se devuelven (x_centro, ancho) ordenados.
        let mut v: Vec<(f64, f64)> = g.parts[0]
            .cuts
            .iter()
            .filter(|c| crate::rectilinear::signed_area(c) < 0.0)
            .map(|c| {
                let (x0, _, x1, _) = crate::part::bounds_of(std::iter::once(c)).unwrap();
                ((x0 + x1) / 2.0, x1 - x0)
            })
            .collect();
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        v
    }

    #[test]
    fn slot_widths_are_thickness_plus_steps() {
        let p = FitTestParams {
            count: 4,
            ..FitTestParams::default()
        };
        let g = fit_test_parts(&p).unwrap();
        let s = slots(&g);
        assert_eq!(s.len(), 9);
        for (k, (_, w)) in s.iter().enumerate() {
            let want = 3.0 + (k as f64 - 4.0) * 0.05;
            assert!((w - want).abs() < 1e-9, "ranura {k}: {w} vs {want}");
        }
        // Todas caben dentro de la placa y no se tocan.
        let plate = &g.parts[0];
        for pair in s.windows(2) {
            assert!(pair[1].0 - pair[1].1 / 2.0 > pair[0].0 + pair[0].1 / 2.0 + 1.0);
        }
        assert!(s[0].0 - s[0].1 / 2.0 > 2.0);
        assert!(s[8].0 + s[8].1 / 2.0 < plate.width - 2.0);
        assert_eq!(crossings(&plate.cuts), 0);
    }

    #[test]
    fn finger_mode_uses_the_finger_width_and_a_matching_peg() {
        let p = FitTestParams {
            mode: FitMode::Finger,
            finger_mm: 8.0,
            step_mm: 0.1,
            count: 3,
            ..FitTestParams::default()
        };
        let g = fit_test_parts(&p).unwrap();
        let s = slots(&g);
        assert_eq!(s.len(), 7);
        assert!(
            (s[3].1 - 8.0).abs() < 1e-9
                && (s[0].1 - 7.7).abs() < 1e-9
                && (s[6].1 - 8.3).abs() < 1e-9
        );
        let tab = &g.parts[1];
        // El diente del macho mide el ancho nominal: busca el tramo estrecho.
        let widths: Vec<f64> = tab.cuts[0]
            .points
            .windows(2)
            .filter(|w| (w[0].y - w[1].y).abs() < 1e-9 && w[0].y > 7.0)
            .map(|w| (w[0].x - w[1].x).abs())
            .collect();
        assert!(widths.iter().any(|w| (w - 8.0).abs() < 1e-9), "{widths:?}");
    }

    #[test]
    fn thickness_mode_peg_passes_through_the_slot_length() {
        let p = FitTestParams::default();
        let g = fit_test_parts(&p).unwrap();
        let tab = &g.parts[1];
        let tab_top: Vec<f64> = tab.cuts[0]
            .points
            .windows(2)
            .filter(|w| (w[0].y - w[1].y).abs() < 1e-9 && w[0].y > 15.0)
            .map(|w| (w[0].x - w[1].x).abs())
            .collect();
        assert_eq!(tab_top.len(), 1);
        assert!(tab_top[0] < p.slot_length_mm);
    }

    #[test]
    fn every_slot_has_its_value_engraved_below_it() {
        let p = FitTestParams {
            count: 4,
            ..FitTestParams::default()
        };
        let g = fit_test_parts(&p).unwrap();
        let plate = &g.parts[0];
        assert!(!plate.engraves.is_empty());
        let s = slots(&g);
        // El primer rótulo debe quedar bajo la primera ranura y dentro de la placa.
        for (x, _) in &s {
            let near: Vec<&Polyline> = plate
                .engraves
                .iter()
                .filter(|e| e.points.iter().all(|q| (q.x - x).abs() < 6.0 && q.y < 10.0))
                .collect();
            assert!(!near.is_empty(), "sin rótulo bajo la ranura en x={x}");
        }
        let (_, _, _, y1) = crate::part::bounds_of(plate.engraves.iter()).unwrap();
        assert!(y1 < plate.height);
        let off = fit_test_parts(&FitTestParams { labels: false, ..p }).unwrap();
        assert!(off.parts.iter().all(|p| p.engraves.is_empty()));
    }

    #[test]
    fn default_range_covers_a_typical_diode_kerf() {
        // Con dientes, ranura menos diente = holgura + 2·haz: un haz de 0,15 mm pide -0,30 mm.
        let p = FitTestParams {
            mode: FitMode::Finger,
            ..FitTestParams::default()
        };
        let offsets = p.offsets();
        assert!(offsets[0] <= -0.3 + 1e-9 && offsets[offsets.len() - 1] >= 0.3 - 1e-9);
        assert_eq!(offsets.len(), 13);
        let g = fit_test_parts(&p).unwrap();
        assert!(g.parts[0].width < 250.0, "{}", g.parts[0].width);
        assert!(g.warnings.iter().all(|w| !w.contains("{")));
    }

    #[test]
    fn labels_use_signed_two_decimals() {
        assert_eq!(offset_label(0.05, 0.05), "+0.05");
        assert_eq!(offset_label(-0.1, 0.05), "-0.10");
        assert_eq!(offset_label(0.0, 0.05), "0.00");
        assert_eq!(offset_label(0.025, 0.025), "+0.025");
    }

    #[test]
    fn kerf_compensation_shifts_slots_by_twice_the_offset() {
        let g = fit_test_parts(&FitTestParams {
            count: 4,
            ..FitTestParams::default()
        })
        .unwrap();
        let k = 0.07;
        let comp = tlaku_geom::kerf_compensate(&g.parts[0].cuts, k);
        let holes: Vec<f64> = comp
            .iter()
            .filter(|c| {
                crate::rectilinear::signed_area(c) < 0.0 || {
                    let (x0, _, x1, _) = crate::part::bounds_of(std::iter::once(*c)).unwrap();
                    x1 - x0 < 20.0
                }
            })
            .map(|c| {
                let (x0, _, x1, _) = crate::part::bounds_of(std::iter::once(c)).unwrap();
                x1 - x0
            })
            .collect();
        assert_eq!(holes.len(), 9);
        let mut holes = holes;
        holes.sort_by(f64::total_cmp);
        for (i, w) in holes.iter().enumerate() {
            let want = 3.0 + (i as f64 - 4.0) * 0.05 - 2.0 * k;
            assert!((w - want).abs() < 0.003, "{w} vs {want}");
        }
        // Y el macho (contorno exterior) crece 2k.
        let tab = tlaku_geom::kerf_compensate(&g.parts[1].cuts, k);
        let (x0, _, x1, _) = crate::part::bounds_of(tab.iter()).unwrap();
        let (a, _, b, _) = crate::part::bounds_of(g.parts[1].cuts.iter()).unwrap();
        assert!(((x1 - x0) - (b - a) - 2.0 * k).abs() < 0.003);
        assert!(inside(&g.parts[0].cuts, 1.0, 1.0));
    }

    #[test]
    fn invalid_values_name_the_field() {
        let bad = |p: FitTestParams| fit_test_parts(&p).unwrap_err().field;
        assert_eq!(
            bad(FitTestParams {
                step_mm: 0.0,
                ..FitTestParams::default()
            })
            .as_deref(),
            Some("step_mm")
        );
        assert_eq!(
            bad(FitTestParams {
                count: 0,
                ..FitTestParams::default()
            })
            .as_deref(),
            Some("count")
        );
        // 3 mm con 10 pasos de 0,5 mm se queda sin ranura.
        assert_eq!(
            bad(FitTestParams {
                step_mm: 0.5,
                count: 10,
                ..FitTestParams::default()
            })
            .as_deref(),
            Some("step_mm")
        );
        assert_eq!(
            bad(FitTestParams {
                thickness_mm: 50.0,
                ..FitTestParams::default()
            })
            .as_deref(),
            Some("thickness_mm")
        );
    }
}
