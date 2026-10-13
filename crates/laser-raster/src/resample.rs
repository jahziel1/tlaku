//! Remuestreo a la rejilla física de barrido.

use crate::gray::GrayF32;

/// Límite de píxeles de la rejilla de salida, para fallar con un mensaje claro en
/// lugar de agotar la memoria con un intervalo de línea absurdamente pequeño.
const MAX_PIXELS: usize = 100_000_000;

/// Cómo se decide la resolución de la rejilla.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sampling {
    /// Separación entre líneas de barrido en mm.
    Interval(f64),
    /// Puntos por pulgada (intervalo = 25.4 / dpi).
    Dpi(f64),
    /// Sin remuestreo: un píxel de la imagen = un punto.
    PassThrough,
}

pub fn interval_from_dpi(dpi: f64) -> f64 {
    25.4 / dpi
}

pub fn dpi_from_interval(interval_mm: f64) -> f64 {
    25.4 / interval_mm
}

/// Imagen ya a la resolución de grabado, con el tamaño físico de cada píxel.
#[derive(Debug, Clone, PartialEq)]
pub struct Resampled {
    pub image: GrayF32,
    pub pixel_size_x_mm: f64,
    pub pixel_size_y_mm: f64,
}

/// Lleva `img` a un área de `width_mm` x `height_mm`. Con intervalo o DPI, las filas
/// y columnas son `round(tamaño / intervalo)` (mínimo 1), de modo que el tamaño físico
/// total se respeta exactamente y el píxel puede diferir ligeramente del intervalo.
/// Reduce con promedio por área y amplía con interpolación bilineal.
pub fn resample_to_mm(
    img: &GrayF32,
    width_mm: f64,
    height_mm: f64,
    sampling: Sampling,
) -> Result<Resampled, String> {
    if !(width_mm.is_finite() && width_mm > 0.0 && height_mm.is_finite() && height_mm > 0.0) {
        return Err(format!("tamaño inválido: {width_mm} x {height_mm} mm"));
    }
    let (w, h) = match sampling {
        Sampling::PassThrough => (img.width, img.height),
        Sampling::Interval(_) | Sampling::Dpi(_) => {
            let interval = match sampling {
                Sampling::Interval(i) => i,
                Sampling::Dpi(d) => interval_from_dpi(d),
                Sampling::PassThrough => unreachable!(),
            };
            if !(interval.is_finite() && interval > 0.0) {
                return Err(format!("intervalo de línea inválido: {interval} mm"));
            }
            let cols = (width_mm / interval).round().max(1.0);
            let rows = (height_mm / interval).round().max(1.0);
            if cols * rows > MAX_PIXELS as f64 {
                return Err(format!(
                    "la rejilla de {cols} x {rows} puntos es demasiado grande; aumenta el intervalo"
                ));
            }
            (cols as usize, rows as usize)
        }
    };
    let image = if sampling == Sampling::PassThrough {
        img.clone()
    } else {
        resample(img, w, h)
    };
    Ok(Resampled {
        image,
        pixel_size_x_mm: width_mm / w as f64,
        pixel_size_y_mm: height_mm / h as f64,
    })
}

/// Cambia el tamaño en píxeles. Cada eje se trata por separado: promedio por área si
/// se reduce, bilineal (alineada a centros de píxel) si se amplía.
pub fn resample(img: &GrayF32, new_w: usize, new_h: usize) -> GrayF32 {
    assert!(new_w > 0 && new_h > 0, "tamaño destino cero");
    let wx = axis_weights(img.width, new_w);
    let wy = axis_weights(img.height, new_h);

    let mut tmp = vec![0.0f32; new_w * img.height];
    for y in 0..img.height {
        let row = &img.data[y * img.width..(y + 1) * img.width];
        for (x, taps) in wx.iter().enumerate() {
            tmp[y * new_w + x] = taps.iter().map(|&(k, w)| row[k] * w).sum();
        }
    }
    let mut data = vec![0.0f32; new_w * new_h];
    for (y, taps) in wy.iter().enumerate() {
        for x in 0..new_w {
            data[y * new_w + x] = taps.iter().map(|&(k, w)| tmp[k * new_w + x] * w).sum();
        }
    }
    GrayF32 {
        width: new_w,
        height: new_h,
        data,
    }
}

/// Para cada posición destino, los píxeles origen y sus pesos (suman 1).
fn axis_weights(src: usize, dst: usize) -> Vec<Vec<(usize, f32)>> {
    let scale = src as f64 / dst as f64;
    (0..dst)
        .map(|i| {
            if dst < src {
                let (a, b) = (i as f64 * scale, (i + 1) as f64 * scale);
                let first = a.floor() as usize;
                let last = (b.ceil() as usize).min(src);
                (first..last)
                    .filter_map(|k| {
                        let overlap = b.min((k + 1) as f64) - a.max(k as f64);
                        (overlap > 1e-12).then_some((k, (overlap / scale) as f32))
                    })
                    .collect()
            } else {
                let p = ((i as f64 + 0.5) * scale - 0.5).clamp(0.0, (src - 1) as f64);
                let k0 = p.floor() as usize;
                let frac = (p - k0 as f64) as f32;
                if frac == 0.0 || k0 + 1 >= src {
                    vec![(k0, 1.0)]
                } else {
                    vec![(k0, 1.0 - frac), (k0 + 1, frac)]
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_when_same_size() {
        let img = GrayF32::from_fn(7, 5, |x, y| ((x * 3 + y * 5) % 11) as f32 / 10.0);
        assert_eq!(resample(&img, 7, 5), img);
    }

    #[test]
    fn downscale_preserves_mean_and_averages() {
        let img = GrayF32::from_fn(6, 6, |x, y| ((x * 7 + y * 13) % 10) as f32 / 9.0);
        let small = resample(&img, 4, 3);
        assert!((small.mean() - img.mean()).abs() < 1e-5);
        let two = resample(&GrayF32::new(4, 1, vec![0.0, 1.0, 0.0, 1.0]).unwrap(), 2, 1);
        assert_eq!(two.data, vec![0.5, 0.5]);
    }

    #[test]
    fn downscale_non_integer_ratio_weights_by_area() {
        // 3 píxeles -> 2: cada destino cubre 1.5 píxeles origen.
        let out = resample(&GrayF32::new(3, 1, vec![0.0, 0.6, 1.0]).unwrap(), 2, 1);
        assert!((out.data[0] - 0.2).abs() < 1e-6); // (0*1 + 0.6*0.5)/1.5
        assert!((out.data[1] - (0.6 * 0.5 + 1.0) / 1.5).abs() < 1e-6);
    }

    #[test]
    fn upscale_is_bilinear_and_bounded() {
        let out = resample(&GrayF32::new(2, 1, vec![0.0, 1.0]).unwrap(), 4, 1);
        let expected = [0.0, 0.25, 0.75, 1.0];
        for (a, b) in out.data.iter().zip(expected) {
            assert!((a - b).abs() < 1e-6, "{:?}", out.data);
        }
    }

    #[test]
    fn mixed_axes() {
        let img = GrayF32::from_fn(4, 2, |x, _| x as f32 / 3.0);
        let out = resample(&img, 2, 6);
        assert_eq!((out.width, out.height), (2, 6));
        assert!(out.data.iter().all(|v| (0.0..=1.0).contains(v)));
    }

    #[test]
    fn grid_from_interval_and_dpi() {
        let img = GrayF32::from_fn(100, 50, |_, _| 0.5);
        let r = resample_to_mm(&img, 20.0, 10.0, Sampling::Interval(0.1)).unwrap();
        assert_eq!((r.image.width, r.image.height), (200, 100));
        assert!((r.pixel_size_x_mm - 0.1).abs() < 1e-12);
        let r = resample_to_mm(&img, 25.4, 12.7, Sampling::Dpi(10.0)).unwrap();
        assert_eq!((r.image.width, r.image.height), (10, 5));
        assert!((r.pixel_size_y_mm - 2.54).abs() < 1e-12);
    }

    #[test]
    fn grid_rounds_and_keeps_physical_size() {
        let img = GrayF32::from_fn(10, 10, |_, _| 0.5);
        let r = resample_to_mm(&img, 10.0, 10.0, Sampling::Interval(0.3)).unwrap();
        assert_eq!(r.image.width, 33);
        assert!((r.pixel_size_x_mm * 33.0 - 10.0).abs() < 1e-9);
        let tiny = resample_to_mm(&img, 0.01, 0.01, Sampling::Interval(1.0)).unwrap();
        assert_eq!((tiny.image.width, tiny.image.height), (1, 1));
    }

    #[test]
    fn pass_through_keeps_pixels() {
        let img = GrayF32::from_fn(30, 20, |x, y| (x + y) as f32 / 50.0);
        let r = resample_to_mm(&img, 15.0, 5.0, Sampling::PassThrough).unwrap();
        assert_eq!(r.image, img);
        assert!((r.pixel_size_x_mm - 0.5).abs() < 1e-12);
        assert!((r.pixel_size_y_mm - 0.25).abs() < 1e-12);
    }

    #[test]
    fn invalid_inputs() {
        let img = GrayF32::from_fn(2, 2, |_, _| 0.5);
        assert!(resample_to_mm(&img, 0.0, 1.0, Sampling::PassThrough).is_err());
        assert!(resample_to_mm(&img, 1.0, 1.0, Sampling::Interval(0.0)).is_err());
        assert!(resample_to_mm(&img, 1.0, 1.0, Sampling::Dpi(-5.0)).is_err());
        assert!(resample_to_mm(&img, 1e6, 1e6, Sampling::Interval(0.1)).is_err());
    }

    #[test]
    fn dpi_conversions() {
        assert!((interval_from_dpi(254.0) - 0.1).abs() < 1e-12);
        assert!((dpi_from_interval(0.1) - 254.0).abs() < 1e-9);
    }
}
