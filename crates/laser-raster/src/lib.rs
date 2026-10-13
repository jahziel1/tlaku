//! Procesamiento de imágenes para grabado láser (modo Image).
//!
//! Flujo: cargar (`gray`) -> ajustar (`adjust`) -> remuestrear al tamaño físico
//! (`resample`) -> binarizar o conservar niveles (`dither`) -> `RasterBitmap` ->
//! barrido (`scan::raster_to_block`). `process` encadena los cuatro primeros pasos.
//!
//! Unidades: mm. La fila 0 de cualquier imagen o bitmap es la de arriba.

pub mod adjust;
pub mod bitmap;
pub mod dither;
pub mod gray;
pub mod resample;
pub mod scan;

pub use adjust::ImageAdjust;
pub use bitmap::RasterBitmap;
pub use dither::{BayerSize, RasterMode};
pub use gray::{GrayF32, Luma};
pub use resample::Sampling;
pub use scan::{RasterSettings, raster_to_block};

/// Opciones del procesado de una imagen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProcessOptions {
    pub adjust: ImageAdjust,
    pub sampling: Sampling,
    pub mode: RasterMode,
    /// Recorrido en serpentina para los modos de difusión de error.
    pub serpentine: bool,
}

impl Default for ProcessOptions {
    fn default() -> Self {
        Self {
            adjust: ImageAdjust::default(),
            sampling: Sampling::Interval(0.1),
            mode: RasterMode::default(),
            serpentine: true,
        }
    }
}

/// Ajusta, remuestrea a `width_mm` x `height_mm` y convierte a potencia.
pub fn process(
    img: &GrayF32,
    width_mm: f64,
    height_mm: f64,
    opts: &ProcessOptions,
) -> Result<RasterBitmap, String> {
    opts.adjust.validate()?;
    if let RasterMode::Threshold(t) = opts.mode
        && !(0.0..=1.0).contains(&t)
    {
        return Err(format!("umbral fuera de 0-1: {t}"));
    }
    let adjusted = opts.adjust.apply(img);
    let grid = resample::resample_to_mm(&adjusted, width_mm, height_mm, opts.sampling)?;
    let data = dither::to_power(&grid.image, opts.mode, opts.serpentine);
    RasterBitmap::new(
        grid.image.width,
        grid.image.height,
        grid.pixel_size_x_mm,
        grid.pixel_size_y_mm,
        data,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_end_to_end() {
        let img = GrayF32::from_fn(50, 50, |x, _| x as f32 / 49.0);
        let opts = ProcessOptions {
            sampling: Sampling::Interval(0.5),
            mode: RasterMode::FloydSteinberg,
            ..Default::default()
        };
        let b = process(&img, 10.0, 5.0, &opts).unwrap();
        assert_eq!((b.width, b.height), (20, 10));
        assert!((b.lit_fraction() - 0.5).abs() < 0.05);
        // Más oscuro a la izquierda.
        let left: f32 = (0..10).map(|y| b.get(0, y) + b.get(1, y)).sum();
        let right: f32 = (0..10).map(|y| b.get(18, y) + b.get(19, y)).sum();
        assert!(left > right);
    }

    #[test]
    fn process_rejects_bad_options() {
        let img = GrayF32::from_fn(4, 4, |_, _| 0.5);
        let bad_thr = ProcessOptions {
            mode: RasterMode::Threshold(2.0),
            ..Default::default()
        };
        assert!(process(&img, 1.0, 1.0, &bad_thr).is_err());
        let bad_adj = ProcessOptions {
            adjust: ImageAdjust {
                gamma: -1.0,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(process(&img, 1.0, 1.0, &bad_adj).is_err());
    }
}
