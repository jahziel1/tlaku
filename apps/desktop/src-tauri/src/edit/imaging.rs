//! Comandos del área de imágenes: la vista previa de una imagen con los ajustes de su capa
//! (C51), sobre `tlaku_pipeline::processed_preview`.
//!
//! `preview_image` devuelve el PNG (en base64) de lo que se grabaría de la imagen, sin calcular el
//! trabajo. Sin `region_mm` es la imagen entera en a lo sumo `max_px` puntos por lado; con
//! `region_mm` es un rectángulo a la resolución real del trabajo (para verla a tamaño real o con
//! zoom).

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::Serialize;
use std::path::Path;
use tlaku_doc::{ImageObject, LayerSettings};
use tlaku_i18n::tr;

/// Vista previa lista para mostrar.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImagePreview {
    /// PNG en escala de grises codificado en base64 (sin prefijo `data:`).
    pub png_base64: String,
    pub width_px: u32,
    pub height_px: u32,
    /// Rectángulo cubierto `[x0, y0, x1, y1]`, mm locales de la imagen (Y hacia arriba).
    pub covered_mm: [f64; 4],
    /// Separación entre puntos de la vista, mm.
    pub interval_mm: f64,
    /// `true` si es la rejilla real del trabajo; `false` si se usó una más gruesa para ir rápido.
    pub exact: bool,
}

pub fn run_preview(
    image: &ImageObject,
    layer: &LayerSettings,
    region_mm: Option<[f64; 4]>,
    max_px: u32,
) -> Result<ImagePreview, String> {
    if image.source_path.trim().is_empty() {
        return Err(tr!("desktop.edit.imaging.no_path"));
    }
    let p = tlaku_pipeline::processed_preview(
        Path::new(&image.source_path),
        image.size_mm,
        layer,
        &image.mask,
        region_mm,
        max_px,
    )?;
    Ok(ImagePreview {
        png_base64: STANDARD.encode(&p.png),
        width_px: p.size_px[0],
        height_px: p.size_px[1],
        covered_mm: p.covered_mm,
        interval_mm: p.interval_mm,
        exact: p.exact,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn preview_image(
    image: ImageObject,
    layer: LayerSettings,
    region_mm: Option<[f64; 4]>,
    max_px: u32,
) -> Result<ImagePreview, String> {
    tauri::async_runtime::spawn_blocking(move || run_preview(&image, &layer, region_mm, max_px))
        .await
        .map_err(crate::edit::task_interrupted)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GrayImage, Luma};

    fn fixture(name: &str) -> (std::path::PathBuf, ImageObject, LayerSettings) {
        let dir = std::env::temp_dir().join(format!("tlaku-imaging-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("origen.png");
        GrayImage::from_fn(40, 40, |x, _| Luma([(x * 6) as u8]))
            .save(&src)
            .unwrap();
        let image = ImageObject {
            source_path: src.to_string_lossy().into_owned(),
            width_px: 40,
            height_px: 40,
            size_mm: [10.0, 10.0],
            mask: Vec::new(),
            variable: None,
        };
        let mut layer = LayerSettings::for_images();
        layer.interval = 0.5;
        (dir, image, layer)
    }

    #[test]
    fn previews_the_whole_image_and_a_zoomed_region() {
        let (dir, image, layer) = fixture("ok");
        let full = run_preview(&image, &layer, None, 1000).unwrap();
        assert_eq!((full.width_px, full.height_px), (20, 20));
        assert!(full.exact);
        let png = STANDARD.decode(&full.png_base64).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        let zoom = run_preview(&image, &layer, Some([5.0, 0.0, 10.0, 5.0]), 500).unwrap();
        assert_eq!((zoom.width_px, zoom.height_px), (10, 10));
        assert_eq!(zoom.covered_mm, [5.0, 0.0, 10.0, 5.0]);
        // Con max_px bajo la vista completa se hace más gruesa y lo dice.
        let coarse = run_preview(&image, &layer, None, 32).unwrap();
        assert!(coarse.exact || coarse.width_px <= 32);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_an_empty_path_and_bad_settings() {
        let (dir, mut image, mut layer) = fixture("err");
        layer.image.gamma = -1.0;
        assert!(run_preview(&image, &layer, None, 100).is_err());
        layer.image.gamma = 1.0;
        image.source_path = " ".into();
        assert!(run_preview(&image, &layer, None, 100).is_err());
        image.source_path = dir.join("no-existe.png").to_string_lossy().into_owned();
        assert!(run_preview(&image, &layer, None, 100).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
