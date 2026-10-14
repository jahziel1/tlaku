//! Grabado 3D por cortes (C47, 3D Slice).
//!
//! La imagen en grises es un mapa de profundidad (oscuro = más hondo, blanco puro = superficie).
//! Con `N` cortes, el umbral baja de blanco a negro en pasos iguales: el corte `k` (desde 0) usa
//! el umbral `(N - k) / N` y graba los píxeles **más oscuros** que él. El corte 0 graba todo
//! menos el blanco puro y el último solo lo más oscuro; el negro puro se graba en todos. Es la
//! mecánica descrita en el foro público de LightBurn (ver `docs/spec/02-capas-corte-imagen.md`,
//! sección 3.6); no hay nada copiado. Solo 8 bits (los 16 bits quedan pendientes).
//!
//! Cada corte es una pasada de imagen normal (`Builder::image`) con modo umbral, así que sale
//! como bloque y fila de resumen propios (`imagen · corte k/N`), también en máquinas de pórtico
//! (útil en CO2). Cada corte puede girar el ángulo de barrido `angle_step_deg` y, con eje Z, baja
//! `z_step` por corte (si no hay `z_step` todos van a la misma Z).

use crate::job::LayerReport;
use crate::layers::ResolvedImage;
use crate::zaxis::LayerZ;
use tlaku_core::toolpath::ToolpathBlock;
use tlaku_doc::Slice3d;
use tlaku_i18n::{msg_is, tr};
use tlaku_raster::RasterMode;

/// Cortes mínimos y máximos (8 bits dan como mucho 256 niveles distintos).
pub(crate) const MIN_SLICES: u32 = 2;
pub(crate) const MAX_SLICES: u32 = 256;

/// Aviso que `Builder::image` da cuando una pasada no tiene ningún punto.
pub(crate) fn blank_warning() -> String {
    tr!("pipeline.slice3d.blank")
}

/// Las imágenes de cada corte de `r`: una sola (`r` tal cual) si el 3D no está activo.
pub(crate) fn expand(r: &ResolvedImage, s: &Slice3d) -> Result<Vec<ResolvedImage>, String> {
    if !s.enabled {
        return Ok(vec![r.clone()]);
    }
    if !(MIN_SLICES..=MAX_SLICES).contains(&s.slices) {
        return Err(tr!(
            "pipeline.slice3d.slice_count",
            min = MIN_SLICES,
            max = MAX_SLICES,
            n = s.slices
        ));
    }
    if !s.angle_step_deg.is_finite() {
        return Err(tr!("pipeline.slice3d.bad_angle", angle = s.angle_step_deg));
    }
    let n = s.slices;
    Ok((0..n)
        .map(|k| {
            let mut slice = r.clone();
            slice.mode = RasterMode::Threshold((n - k) as f32 / n as f32);
            slice.raster.angle_deg += s.angle_step_deg * f64::from(k);
            slice
        })
        .collect())
}

/// Z del corte `k` de `total`: baja `z_step` por corte desde `z_offset` (los cortes son bloques
/// distintos, así que no usan el reparto por pasadas de `LayerZ`). Con un solo corte, `z` tal cual.
pub(crate) fn z_for(z: LayerZ, k: usize, total: usize) -> LayerZ {
    if total <= 1 {
        return z;
    }
    LayerZ {
        offset: z.offset + z.step * k as f64,
        step: 0.0,
        ..z
    }
}

/// Nombra los informes del corte `k` de `total` (`imagen · corte 3/16`); no hace nada con un solo
/// corte.
pub(crate) fn name_slice(reports: &mut [LayerReport], k: usize, total: usize) {
    if total > 1 {
        for r in reports {
            r.name = tr!(
                "pipeline.slice3d.slice_name",
                name = r.name,
                k = k + 1,
                total = total
            );
        }
    }
}

/// Reduce los avisos de "imagen en blanco" que dieron los cortes desde `from` a uno solo, con la
/// cuenta (los cortes más profundos suelen quedar vacíos y no son un error).
pub(crate) fn merge_blank_warnings(warnings: &mut Vec<String>, from: usize, total: usize) {
    if total <= 1 {
        return;
    }
    let blank = warnings[from..]
        .iter()
        .filter(|w| msg_is(w, "pipeline.slice3d.blank"))
        .count();
    if blank == 0 {
        return;
    }
    let mut tail = warnings.split_off(from);
    tail.retain(|w| !msg_is(w, "pipeline.slice3d.blank"));
    warnings.extend(tail);
    warnings.push(if blank == total {
        blank_warning()
    } else {
        tr!("pipeline.slice3d.some_blank", blank = blank, total = total)
    });
}

/// Las pausas de la capa van solo antes del primer corte y después del último.
pub(crate) fn fix_pauses(blocks: &mut [ToolpathBlock]) {
    let n = blocks.len();
    for (i, b) in blocks.iter_mut().enumerate() {
        if let Some(g) = &mut b.galvo {
            if i > 0 {
                g.pause_before_ms = 0.0;
            }
            if i + 1 < n {
                g.pause_after_ms = 0.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_doc::{LayerSettings, Overscan};

    fn resolved() -> ResolvedImage {
        let l = LayerSettings {
            angle: 10.0,
            overscan: Overscan::Mm(0.0),
            ..LayerSettings::for_images()
        };
        crate::layers::resolve_image(&l).unwrap()
    }

    fn thresholds(v: &[ResolvedImage]) -> Vec<f32> {
        v.iter()
            .map(|r| match r.mode {
                RasterMode::Threshold(t) => t,
                m => panic!("modo inesperado: {m:?}"),
            })
            .collect()
    }

    #[test]
    fn disabled_gives_the_image_unchanged() {
        let r = resolved();
        let v = expand(&r, &Slice3d::default()).unwrap();
        assert_eq!(v, vec![r]);
    }

    #[test]
    fn thresholds_descend_from_white_to_dark() {
        let s = Slice3d {
            enabled: true,
            slices: 4,
            angle_step_deg: 0.0,
        };
        let v = expand(&resolved(), &s).unwrap();
        assert_eq!(thresholds(&v), vec![1.0, 0.75, 0.5, 0.25]);
        assert!(v.iter().all(|r| r.raster.angle_deg == 10.0));
    }

    #[test]
    fn angle_changes_per_slice() {
        let s = Slice3d {
            enabled: true,
            slices: 3,
            angle_step_deg: 45.0,
        };
        let v = expand(&resolved(), &s).unwrap();
        let angles: Vec<f64> = v.iter().map(|r| r.raster.angle_deg).collect();
        assert_eq!(angles, vec![10.0, 55.0, 100.0]);
    }

    #[test]
    fn invalid_slice_counts_and_angles_are_errors() {
        for slices in [0, 1, 257] {
            let s = Slice3d {
                enabled: true,
                slices,
                angle_step_deg: 0.0,
            };
            let e = expand(&resolved(), &s).unwrap_err();
            assert!(e.contains("cortes"), "{e}");
        }
        let s = Slice3d {
            enabled: true,
            slices: 4,
            angle_step_deg: f64::NAN,
        };
        assert!(expand(&resolved(), &s).is_err());
    }

    #[test]
    fn z_goes_down_per_slice() {
        let z = LayerZ {
            axis: true,
            offset: 1.0,
            step: 0.25,
        };
        assert_eq!(z_for(z, 0, 3).z_at(0), Some(1.0));
        assert_eq!(z_for(z, 4, 8).z_at(0), Some(2.0));
        assert!(!z_for(z, 4, 8).splits(3) && z_for(z, 0, 1).splits(3));
    }

    #[test]
    fn blank_warnings_collapse() {
        let mut w = vec!["otro".to_string()];
        let from = w.len();
        w.extend([blank_warning(), blank_warning()]);
        w.push("x".into());
        merge_blank_warnings(&mut w, from, 4);
        assert_eq!(w.len(), 3);
        assert!(w[2].contains("2 de 4"), "{w:?}");
        assert_eq!(w[0], "otro");
        assert_eq!(w[1], "x");
    }
}
