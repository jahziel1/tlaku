//! Imagen procesada: lo que el motor grabaría de una imagen con los ajustes de su capa (brillo,
//! contraste, tramado, intervalo y máscara), guardado como PNG ("Guardar imagen procesada", D82).

use std::path::Path;
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Point, Polyline};
use tlaku_doc::LayerSettings;
use tlaku_i18n::tr;
use tlaku_raster::{Luma, ProcessOptions, Sampling};

/// PNG en escala de grises de la imagen `source` de `size_mm` (ancho, alto) tratada con los
/// ajustes de imagen de `layer`; negro = potencia máxima. `mask` son las curvas de recorte en
/// coordenadas locales de la imagen (mm, Y hacia arriba); vacío = imagen completa. Devuelve el
/// PNG y su tamaño en píxeles. Falla con un mensaje en español si los ajustes son inválidos.
pub fn processed_png(
    source: &Path,
    size_mm: [f64; 2],
    layer: &LayerSettings,
    mask: &[BezPath],
) -> Result<(Vec<u8>, [u32; 2]), String> {
    let [w, h] = size_mm;
    if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
        return Err(tr!("pipeline.processed.bad_size", w = w, h = h));
    }
    let r = crate::layers::resolve_image(layer)?;
    let opts = ProcessOptions {
        adjust: r.adjust,
        sampling: Sampling::Interval(r.interval_mm),
        mode: r.mode,
        ..Default::default()
    };
    let (gray, opts) = tlaku_raster::load_for_process(source, Luma::default(), w, h, &opts)
        .map_err(|e| format!("{}: {e}", source.display()))?;
    let mut bitmap = tlaku_raster::process(&gray, w, h, &opts)
        .map_err(|e| tr!("pipeline.common.image_error", error = e))?;
    let shapes: Vec<Polyline> = mask.iter().flat_map(|p| p.flatten(0.01)).collect();
    tlaku_raster::apply_mask(&mut bitmap, Point::new(0.0, 0.0), &shapes);
    let png = bitmap.to_png()?;
    Ok((png, [bitmap.width as u32, bitmap.height as u32]))
}

/// Vista previa de una imagen: el PNG de lo que se grabaría (ver [`tlaku_raster::preview`]) y
/// qué parte de la imagen cubre.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessedPreview {
    pub png: Vec<u8>,
    /// Tamaño del PNG en píxeles.
    pub size_px: [u32; 2],
    /// Rectángulo cubierto `[x0, y0, x1, y1]`, mm locales de la imagen (Y hacia arriba).
    pub covered_mm: [f64; 4],
    /// Intervalo de la rejilla de la vista, mm.
    pub interval_mm: f64,
    /// `true` si es la rejilla real del trabajo; `false` si se usó una más gruesa para ir rápido.
    pub exact: bool,
}

/// Igual que [`processed_png`] pero para la vista previa de la capa: toda la imagen en a lo sumo
/// `max_px` puntos por lado, o (con `region_mm`) un rectángulo a la resolución real del trabajo.
/// Los ajustes son los de `layer`, de modo que se puede probar un ajuste sin calcular el trabajo.
pub fn processed_preview(
    source: &Path,
    size_mm: [f64; 2],
    layer: &LayerSettings,
    mask: &[BezPath],
    region_mm: Option<[f64; 4]>,
    max_px: u32,
) -> Result<ProcessedPreview, String> {
    let [w, h] = size_mm;
    if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
        return Err(tr!("pipeline.processed.bad_size", w = w, h = h));
    }
    let r = crate::layers::resolve_image(layer)?;
    let opts = ProcessOptions {
        adjust: r.adjust,
        sampling: Sampling::Interval(r.interval_mm),
        mode: r.mode,
        ..Default::default()
    };
    let shapes: Vec<Polyline> = mask.iter().flat_map(|p| p.flatten(0.01)).collect();
    let view = tlaku_raster::PreviewView { region_mm, max_px };
    let p = tlaku_raster::preview(source, Luma::default(), w, h, &opts, &shapes, &view)
        .map_err(|e| format!("{}: {e}", source.display()))?;
    let png = p.bitmap.to_png()?;
    Ok(ProcessedPreview {
        png,
        size_px: [p.bitmap.width as u32, p.bitmap.height as u32],
        covered_mm: p.covered_mm,
        interval_mm: p.interval_mm,
        exact: p.exact,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GrayImage, Luma as L};

    #[test]
    fn processes_with_layer_settings_and_mask() {
        let dir = std::env::temp_dir().join(format!("tlaku-processed-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("origen.png");
        GrayImage::from_pixel(20, 20, L([0])).save(&src).unwrap();
        let mut layer = LayerSettings::for_images();
        layer.interval = 1.0;
        // Imagen negra de 10x10 mm a 1 mm/punto: 10x10 puntos todos encendidos.
        let (png, size) = processed_png(&src, [10.0, 10.0], &layer, &[]).unwrap();
        assert_eq!(size, [10, 10]);
        let img = image::load_from_memory(&png).unwrap().to_luma8();
        assert!(img.pixels().all(|p| p.0[0] < 128));
        // Máscara de la mitad izquierda: la derecha queda en blanco.
        let p = |x: f64, y: f64| Point::new(x, y);
        use tlaku_core::bezpath::PathSeg;
        let half = BezPath(vec![
            PathSeg::MoveTo { to: p(0.0, 0.0) },
            PathSeg::LineTo { to: p(5.0, 0.0) },
            PathSeg::LineTo { to: p(5.0, 10.0) },
            PathSeg::LineTo { to: p(0.0, 10.0) },
            PathSeg::Close,
        ]);
        let (png, _) = processed_png(&src, [10.0, 10.0], &layer, &[half]).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_luma8();
        assert!(img.get_pixel(1, 5).0[0] < 128);
        assert_eq!(img.get_pixel(8, 5).0[0], 255);
        // Tamaño inválido.
        assert!(processed_png(&src, [0.0, 10.0], &layer, &[]).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn preview_matches_the_processed_png_and_follows_the_layer_adjustments() {
        let dir = std::env::temp_dir().join(format!("tlaku-preview-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("origen.png");
        GrayImage::from_fn(40, 40, |x, _| L([(x * 6) as u8]))
            .save(&src)
            .unwrap();
        let mut layer = LayerSettings::for_images();
        layer.interval = 0.5;
        // La vista completa con margen de sobra coincide con la imagen procesada.
        let full = processed_preview(&src, [10.0, 10.0], &layer, &[], None, 1000).unwrap();
        let (png, size) = processed_png(&src, [10.0, 10.0], &layer, &[]).unwrap();
        assert!(full.exact);
        assert_eq!(full.size_px, size);
        assert_eq!(full.png, png);
        assert_eq!(full.covered_mm, [0.0, 0.0, 10.0, 10.0]);
        // Un ajuste de la capa cambia la vista (más brillo = menos potencia = más blanco).
        layer.image.brightness = 0.6;
        let lighter = processed_preview(&src, [10.0, 10.0], &layer, &[], None, 1000).unwrap();
        assert_ne!(lighter.png, full.png);
        let mean = |png: &[u8]| {
            let img = image::load_from_memory(png).unwrap().to_luma8();
            img.pixels().map(|p| f64::from(p.0[0])).sum::<f64>()
                / f64::from(img.width() * img.height())
        };
        assert!(mean(&lighter.png) > mean(&full.png));
        // De cerca: la mitad derecha a la rejilla real.
        let zoom = processed_preview(
            &src,
            [10.0, 10.0],
            &layer,
            &[],
            Some([5.0, 0.0, 10.0, 10.0]),
            500,
        )
        .unwrap();
        assert_eq!(zoom.size_px, [10, 20]);
        assert_eq!(zoom.covered_mm, [5.0, 0.0, 10.0, 10.0]);
        // Ajustes inválidos y tamaño inválido.
        layer.image.brightness = 3.0;
        assert!(processed_preview(&src, [10.0, 10.0], &layer, &[], None, 100).is_err());
        layer.image.brightness = 0.0;
        assert!(processed_preview(&src, [0.0, 10.0], &layer, &[], None, 100).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
