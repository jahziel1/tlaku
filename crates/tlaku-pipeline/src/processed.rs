//! Imagen procesada: lo que el motor grabaría de una imagen con los ajustes de su capa (brillo,
//! contraste, tramado, intervalo y máscara), guardado como PNG ("Guardar imagen procesada", D82).

use std::path::Path;
use tlaku_core::bezpath::BezPath;
use tlaku_core::{Point, Polyline};
use tlaku_doc::LayerSettings;
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
        return Err(format!("tamaño de imagen inválido: {w} x {h} mm"));
    }
    let r = crate::layers::resolve_image(layer)?;
    let gray = tlaku_raster::gray::load(source, Luma::default())
        .map_err(|e| format!("{}: {e}", source.display()))?;
    let opts = ProcessOptions {
        adjust: r.adjust,
        sampling: Sampling::Interval(r.interval_mm),
        mode: r.mode,
        ..Default::default()
    };
    let mut bitmap =
        tlaku_raster::process(&gray, w, h, &opts).map_err(|e| format!("imagen: {e}"))?;
    let shapes: Vec<Polyline> = mask.iter().flat_map(|p| p.flatten(0.01)).collect();
    tlaku_raster::apply_mask(&mut bitmap, Point::new(0.0, 0.0), &shapes);
    let png = bitmap.to_png()?;
    Ok((png, [bitmap.width as u32, bitmap.height as u32]))
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
}
