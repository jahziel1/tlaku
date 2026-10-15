//! Procesamiento de imágenes para grabado láser (modo Image).
//!
//! Flujo: cargar (`gray`) -> ajustar (`adjust`) -> remuestrear al tamaño físico
//! (`resample`) -> binarizar o conservar niveles (`dither`, con las tramas de `screen` y
//! los bordes de `sketch`) -> `RasterBitmap` ->
//! barrido (`scan::raster_to_block`). `process` encadena los cuatro primeros pasos.
//!
//! Unidades: mm. La fila 0 de cualquier imagen o bitmap es la de arriba.

use tlaku_i18n::tr;

pub mod adjust;
pub mod bitmap;
pub mod dither;
pub mod downsample;
pub mod gray;
mod load;
pub mod mask;
pub mod preview;
pub mod resample;
pub mod scan;
pub mod screen;
pub mod sketch;
pub mod tonal;
pub mod warp;

pub use adjust::ImageAdjust;
pub use bitmap::RasterBitmap;
pub use dither::{BayerSize, RasterMode};
pub use gray::{GrayF32, Luma};
pub use mask::apply_mask;
pub use preview::{Preview, PreviewView, preview};
pub use resample::Sampling;
pub use scan::{RasterSettings, raster_to_block};
pub use tonal::{DoseMap, rgba_to_png};

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
        return Err(tr!("raster.lib.threshold", value = t));
    }
    if let RasterMode::Halftone { cell_mm, angle_deg }
    | RasterMode::Newsprint { cell_mm, angle_deg } = opts.mode
    {
        if !(cell_mm.is_finite() && cell_mm > 0.0) {
            return Err(tr!("raster.lib.cell_size", value = cell_mm));
        }
        if !angle_deg.is_finite() {
            return Err(tr!("raster.lib.screen_angle", value = angle_deg));
        }
    }
    let (cols, rows) = resample::grid_size(width_mm, height_mm, opts.sampling)?;
    let adjusted = opts.adjust.apply(img);
    // Si la imagen ya está en la rejilla (el caso normal tras `load_for_process`) no se copia.
    let grid = if (adjusted.width, adjusted.height) == (cols, rows) {
        adjusted
    } else {
        let g = resample::resample(&adjusted, cols, rows);
        drop(adjusted);
        g
    };
    let pixel = (width_mm / cols as f64, height_mm / rows as f64);
    let data = dither::to_power(&grid, opts.mode, opts.serpentine, pixel);
    RasterBitmap::new(grid.width, grid.height, pixel.0, pixel.1, data)
}

/// Carga `path` para procesarla en un área de `width_mm` x `height_mm` y devuelve el gris
/// ya reducido a lo que ese trabajo necesita, junto con las opciones a usar con él.
///
/// La imagen se remuestrea a la rejilla de grabado (o a algo mayor si ya era menor) **antes** de
/// los ajustes y del tramado, así que una foto de 100 Mpx para 400 mm a 0,1 mm trabaja con
/// 4 000 x 4 000 píxeles y no con 10 000 x 10 000. Como los radios de realce y de quitar motas
/// se dan en píxeles de la imagen original, se reescalan por el factor de reducción para que
/// actúen sobre el mismo tamaño físico. Una imagen que no hace falta reducir se carga igual
/// que con [`gray::load`] y las opciones no cambian.
pub fn load_for_process(
    path: &std::path::Path,
    luma: Luma,
    width_mm: f64,
    height_mm: f64,
    opts: &ProcessOptions,
) -> Result<(GrayF32, ProcessOptions), String> {
    opts.adjust.validate()?;
    let (cols, rows) = resample::grid_size(width_mm, height_mm, opts.sampling)?;
    let (gray, (src_w, src_h)) = gray::load_scaled(path, luma, cols, rows)?;
    let mut opts = *opts;
    let fx = src_w as f64 / gray.width as f64;
    let fy = src_h as f64 / gray.height as f64;
    let factor = (fx * fy).sqrt();
    if factor > 1.000_001 {
        let a = &mut opts.adjust;
        a.enhance_radius_px = (f64::from(a.enhance_radius_px) / factor) as f32;
        if a.denoise_px > 0 {
            a.denoise_px = (f64::from(a.denoise_px) / factor).ceil().max(1.0) as u32;
        }
    }
    Ok((gray, opts))
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
    fn process_accepts_the_screen_modes() {
        let img = GrayF32::from_fn(50, 50, |x, _| x as f32 / 49.0);
        for mode in [
            RasterMode::Halftone {
                cell_mm: 1.0,
                angle_deg: 45.0,
            },
            RasterMode::Newsprint {
                cell_mm: 1.0,
                angle_deg: 45.0,
            },
            RasterMode::Sketch,
        ] {
            let opts = ProcessOptions {
                mode,
                ..Default::default()
            };
            let b = process(&img, 10.0, 10.0, &opts).unwrap();
            assert_eq!((b.width, b.height), (100, 100));
            assert!(b.data.iter().all(|&v| v == 0.0 || v == 1.0));
        }
    }

    #[test]
    fn process_rejects_bad_screen_options() {
        let img = GrayF32::from_fn(4, 4, |_, _| 0.5);
        for (cell_mm, angle_deg) in [(0.0, 0.0), (-1.0, 0.0), (f64::NAN, 0.0), (1.0, f64::NAN)] {
            for mode in [
                RasterMode::Halftone { cell_mm, angle_deg },
                RasterMode::Newsprint { cell_mm, angle_deg },
            ] {
                let opts = ProcessOptions {
                    mode,
                    ..Default::default()
                };
                assert!(process(&img, 1.0, 1.0, &opts).is_err(), "{mode:?}");
            }
        }
    }

    #[test]
    fn process_big_image_is_fast() {
        // 100 x 100 mm a 0.1 mm = 1 millón de puntos; en debug debe bajar de ~1 s.
        let img = GrayF32::from_fn(400, 400, |x, y| ((x + y) % 400) as f32 / 399.0);
        for mode in [
            RasterMode::Halftone {
                cell_mm: 1.0,
                angle_deg: 22.0,
            },
            RasterMode::Newsprint {
                cell_mm: 1.0,
                angle_deg: 22.0,
            },
            RasterMode::Sketch,
        ] {
            let opts = ProcessOptions {
                mode,
                ..Default::default()
            };
            let t = std::time::Instant::now();
            let b = process(&img, 100.0, 100.0, &opts).unwrap();
            let elapsed = t.elapsed();
            assert_eq!(b.width * b.height, 1_000_000);
            assert!(elapsed.as_secs_f64() < 3.0, "{mode:?} tardó {elapsed:?}");
            eprintln!("{mode:?}: {elapsed:?}");
        }
    }

    #[test]
    fn process_with_enhance_and_denoise_on_a_million_points_is_fast() {
        // 100 x 100 mm a 0,1 mm con una fuente de 1000 x 1000 px: realce y quitar motas
        // activos: en debug el camino sin tramado baja de 1 s.
        let img = GrayF32::from_fn(1000, 1000, |x, y| {
            (((x / 7 + y / 5) % 9) as f32 / 8.0 + ((x * 31 + y * 17) % 13) as f32 / 400.0).min(1.0)
        });
        for mode in [RasterMode::Grayscale, RasterMode::Stucki] {
            let opts = ProcessOptions {
                adjust: ImageAdjust {
                    enhance_radius_px: 1.5,
                    enhance_amount: 1.0,
                    denoise_px: 2,
                    ..Default::default()
                },
                sampling: Sampling::Interval(0.1),
                mode,
                ..Default::default()
            };
            let t = std::time::Instant::now();
            let b = process(&img, 100.0, 100.0, &opts).unwrap();
            let elapsed = t.elapsed();
            assert_eq!(b.width * b.height, 1_000_000);
            eprintln!("{mode:?} con realce y quitar motas: {elapsed:?}");
            // Grayscale es el camino de pass-through; la difusión de error suma su propio coste.
            let limit = if mode == RasterMode::Grayscale {
                1.0
            } else {
                2.0
            };
            assert!(elapsed.as_secs_f64() < limit, "{mode:?} tardó {elapsed:?}");
        }
    }

    #[test]
    fn pass_through_is_grayscale_resampled_to_the_interval() {
        // Sin ajustes ni tramado: la potencia es 1 - gris, remuestreado a la rejilla.
        let img = GrayF32::from_fn(40, 40, |x, _| x as f32 / 39.0);
        let opts = ProcessOptions {
            sampling: Sampling::Interval(0.5),
            mode: RasterMode::Grayscale,
            ..Default::default()
        };
        let b = process(&img, 10.0, 10.0, &opts).unwrap();
        assert_eq!((b.width, b.height), (20, 20));
        let want = resample::resample(&img, 20, 20);
        for y in 0..20 {
            for x in 0..20 {
                assert!((b.get(x, y) - (1.0 - want.get(x, y))).abs() < 1e-6);
            }
        }
        // Valores intermedios: no se binariza.
        assert!(b.data.iter().any(|&v| v > 0.1 && v < 0.9));
    }

    #[test]
    fn denoise_drops_specks_before_dithering() {
        let mut img = GrayF32::from_fn(40, 40, |_, _| 1.0);
        img.data[20 * 40 + 20] = 0.0;
        let base = ProcessOptions {
            sampling: Sampling::Interval(0.1),
            mode: RasterMode::Threshold(0.5),
            ..Default::default()
        };
        assert_eq!(
            process(&img, 4.0, 4.0, &base).unwrap().lit_fraction(),
            1.0 / 1600.0
        );
        let clean = ProcessOptions {
            adjust: ImageAdjust {
                denoise_px: 1,
                ..Default::default()
            },
            ..base
        };
        assert_eq!(process(&img, 4.0, 4.0, &clean).unwrap().lit_fraction(), 0.0);
    }

    #[test]
    fn load_for_process_reduces_to_the_grid_and_rescales_pixel_radii() {
        let dir = std::env::temp_dir().join(format!("tlaku-lfp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("g.png");
        image::GrayImage::from_fn(200, 100, |x, _| image::Luma([(x as u8).wrapping_mul(3)]))
            .save(&path)
            .unwrap();
        let opts = ProcessOptions {
            adjust: ImageAdjust {
                enhance_radius_px: 5.0,
                enhance_amount: 1.0,
                denoise_px: 4,
                ..Default::default()
            },
            sampling: Sampling::Interval(0.5),
            ..Default::default()
        };
        // 10 x 5 mm a 0,5 mm = 20 x 10 puntos: la imagen baja a 10 veces menos por eje.
        let (g, o) = load_for_process(&path, Luma::default(), 10.0, 5.0, &opts).unwrap();
        assert_eq!((g.width, g.height), (20, 10));
        assert!((o.adjust.enhance_radius_px - 0.5).abs() < 1e-6);
        assert_eq!(o.adjust.denoise_px, 1);
        // Lo mismo que cargar entera y remuestrear (sin ajustes de por medio).
        let full = gray::load(&path, Luma::default()).unwrap();
        let want = resample::resample(&full, 20, 10);
        assert!(
            g.data
                .iter()
                .zip(&want.data)
                .all(|(a, b)| (a - b).abs() < 1e-5)
        );
        // Una imagen menor que la rejilla se carga entera y las opciones no cambian.
        let (g, o) = load_for_process(&path, Luma::default(), 100.0, 50.0, &opts).unwrap();
        assert_eq!((g.width, g.height), (200, 100));
        assert_eq!(o, opts);
        // Ajustes inválidos fallan antes de leer nada.
        let bad = ProcessOptions {
            adjust: ImageAdjust {
                gamma: -1.0,
                ..Default::default()
            },
            ..opts
        };
        assert!(
            load_for_process(&dir.join("no-existe.png"), Luma::default(), 1.0, 1.0, &bad).is_err()
        );
        std::fs::remove_dir_all(dir).unwrap();
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
