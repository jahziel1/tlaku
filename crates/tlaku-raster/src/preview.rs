//! Vista previa de una imagen ya ajustada y tramada, antes de calcular el trabajo.
//!
//! Dos formas de verla:
//!
//! - **Completa** (`region_mm = None`): toda la imagen en a lo sumo `max_px` puntos por lado.
//!   Si la rejilla de grabado es más fina, se usa un intervalo mayor (el resultado es
//!   aproximado y [`Preview::exact`] sale `false`); así se recalcula rápido al mover un control.
//! - **De cerca** (`region_mm = Some`): se procesa con el intervalo real de la capa, igual que el
//!   trabajo, y se devuelve solo el rectángulo pedido. Cada punto de la vista es un punto grabado
//!   (`exact = true`).
//!
//! Ambas reducen la imagen al cargarla ([`crate::load_for_process`]), de modo que una foto enorme
//! cuesta lo mismo que la rejilla pedida y no su tamaño original.

use crate::{Luma, ProcessOptions, RasterBitmap, Sampling, apply_mask, process, resample};
use std::path::Path;
use tlaku_core::{Point, Polyline};
use tlaku_i18n::tr;

/// Menor y mayor `max_px` admitidos.
pub const MIN_PREVIEW_PX: u32 = 32;
pub const MAX_PREVIEW_PX: u32 = 4096;

/// Qué parte de la imagen se quiere ver y con cuántos puntos como máximo por lado.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewView {
    /// Rectángulo `[x0, y0, x1, y1]` en mm locales de la imagen (origen abajo a la izquierda, Y
    /// hacia arriba); `None` = la imagen completa.
    pub region_mm: Option<[f64; 4]>,
    pub max_px: u32,
}

/// Resultado de [`preview`].
#[derive(Debug, Clone, PartialEq)]
pub struct Preview {
    /// Potencia por punto (0 = apagado) de la parte pedida.
    pub bitmap: RasterBitmap,
    /// Rectángulo realmente cubierto `[x0, y0, x1, y1]`, mm locales (Y hacia arriba): la región
    /// pedida ajustada a la rejilla y recortada a la imagen.
    pub covered_mm: [f64; 4],
    /// Intervalo de la rejilla de la vista, mm.
    pub interval_mm: f64,
    /// `true` si la rejilla es la del grabado real (el mismo intervalo que usará el trabajo).
    pub exact: bool,
}

fn interval_of(s: Sampling) -> f64 {
    match s {
        Sampling::Interval(i) => i,
        Sampling::Dpi(d) => resample::interval_from_dpi(d),
    }
}

/// Procesa `path` (de `width_mm` x `height_mm`) con `opts` y devuelve la vista pedida.
/// `mask` son las formas de recorte en mm locales (vacío = completa).
pub fn preview(
    path: &Path,
    luma: Luma,
    width_mm: f64,
    height_mm: f64,
    opts: &ProcessOptions,
    mask: &[Polyline],
    view: &PreviewView,
) -> Result<Preview, String> {
    let max_px = view.max_px.clamp(MIN_PREVIEW_PX, MAX_PREVIEW_PX);
    let real = interval_of(opts.sampling);
    // Se valida la rejilla real (intervalo, tamaño) también para la vista completa.
    resample::grid_size(width_mm, height_mm, opts.sampling)?;
    let mut opts = *opts;
    let interval = match view.region_mm {
        Some(_) => real,
        None => real.max(width_mm.max(height_mm) / f64::from(max_px)),
    };
    opts.sampling = Sampling::Interval(interval);
    let (gray, opts) = crate::load_for_process(path, luma, width_mm, height_mm, &opts)?;
    let mut bitmap = process(&gray, width_mm, height_mm, &opts)?;
    drop(gray);
    apply_mask(&mut bitmap, Point::new(0.0, 0.0), mask);
    let exact = interval <= real * (1.0 + 1e-9);
    let (px, py) = (bitmap.pixel_size_x_mm, bitmap.pixel_size_y_mm);
    let Some([x0, y0, x1, y1]) = view.region_mm else {
        return Ok(Preview {
            covered_mm: [0.0, 0.0, bitmap.width_mm(), bitmap.height_mm()],
            bitmap,
            interval_mm: interval,
            exact,
        });
    };
    if ![x0, y0, x1, y1].iter().all(|v| v.is_finite()) || x1 <= x0 || y1 <= y0 {
        return Err(tr!(
            "raster.preview.bad_region",
            x0 = x0,
            y0 = y0,
            x1 = x1,
            y1 = y1
        ));
    }
    // Columnas y filas (la fila 0 es la de arriba) que cubren la región, dentro del bitmap.
    let col0 = ((x0 / px).floor().max(0.0) as usize).min(bitmap.width - 1);
    let col1 = ((x1 / px).ceil().max(0.0) as usize).clamp(col0 + 1, bitmap.width);
    let row0 = (((height_mm - y1) / py).floor().max(0.0) as usize).min(bitmap.height - 1);
    let row1 = (((height_mm - y0) / py).ceil().max(0.0) as usize).clamp(row0 + 1, bitmap.height);
    let (w, h) = (
        (col1 - col0).min(max_px as usize),
        (row1 - row0).min(max_px as usize),
    );
    let cropped = bitmap.crop(col0, row0, w, h)?;
    let covered_mm = [
        col0 as f64 * px,
        height_mm - (row0 + h) as f64 * py,
        (col0 + w) as f64 * px,
        height_mm - row0 as f64 * py,
    ];
    Ok(Preview {
        bitmap: cropped,
        covered_mm,
        interval_mm: interval,
        exact,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ImageAdjust, RasterMode};

    /// PNG degradado horizontal de `w` x `h` px en una carpeta propia.
    fn gradient(name: &str, w: u32, h: u32) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("tlaku-prev-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("g.png");
        image::GrayImage::from_fn(w, h, |x, _| image::Luma([(x * 255 / (w - 1)) as u8]))
            .save(&path)
            .unwrap();
        path
    }

    fn opts(interval: f64) -> ProcessOptions {
        ProcessOptions {
            sampling: Sampling::Interval(interval),
            mode: RasterMode::Grayscale,
            ..Default::default()
        }
    }

    #[test]
    fn full_view_is_exact_when_the_grid_is_small_enough() {
        let path = gradient("full", 100, 50);
        let view = PreviewView {
            region_mm: None,
            max_px: 1000,
        };
        let p = preview(&path, Luma::default(), 10.0, 5.0, &opts(0.5), &[], &view).unwrap();
        assert_eq!((p.bitmap.width, p.bitmap.height), (20, 10));
        assert!(p.exact);
        assert_eq!(p.covered_mm, [0.0, 0.0, 10.0, 5.0]);
        // Más oscuro (más potencia) a la izquierda: negro = potencia.
        assert!(p.bitmap.get(0, 0) > p.bitmap.get(19, 0));
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn full_view_coarsens_the_grid_to_fit_max_px() {
        let path = gradient("coarse", 400, 200);
        let view = PreviewView {
            region_mm: None,
            max_px: 100,
        };
        // 100 x 50 mm a 0,1 mm serían 1000 x 500: la vista se queda en 100 de ancho.
        let p = preview(&path, Luma::default(), 100.0, 50.0, &opts(0.1), &[], &view).unwrap();
        assert_eq!((p.bitmap.width, p.bitmap.height), (100, 50));
        assert!(!p.exact);
        assert!((p.interval_mm - 1.0).abs() < 1e-9);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn region_view_matches_the_real_grid_and_crops() {
        let path = gradient("region", 200, 100);
        let o = opts(0.5);
        let full = {
            let (g, o) = crate::load_for_process(&path, Luma::default(), 10.0, 5.0, &o).unwrap();
            process(&g, 10.0, 5.0, &o).unwrap()
        };
        // Cuadrante derecho de abajo: x de 5 a 10, y de 0 a 2,5 (columnas 10..20, filas 5..10).
        let view = PreviewView {
            region_mm: Some([5.0, 0.0, 10.0, 2.5]),
            max_px: 1000,
        };
        let p = preview(&path, Luma::default(), 10.0, 5.0, &o, &[], &view).unwrap();
        assert!(p.exact);
        assert_eq!(p.covered_mm, [5.0, 0.0, 10.0, 2.5]);
        assert_eq!((p.bitmap.width, p.bitmap.height), (10, 5));
        assert_eq!(p.bitmap, full.crop(10, 5, 10, 5).unwrap());
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn region_is_clamped_to_the_image_and_to_max_px() {
        let path = gradient("clamp", 100, 100);
        let o = opts(0.1);
        // Región que se sale por la derecha y por arriba: se recorta a la imagen.
        let view = PreviewView {
            region_mm: Some([8.0, 8.0, 50.0, 50.0]),
            max_px: 1000,
        };
        let p = preview(&path, Luma::default(), 10.0, 10.0, &o, &[], &view).unwrap();
        assert_eq!(p.covered_mm, [8.0, 8.0, 10.0, 10.0]);
        assert_eq!((p.bitmap.width, p.bitmap.height), (20, 20));
        // Con max_px menor que la región, se queda en max_px (mínimo admitido: 32).
        let big = PreviewView {
            region_mm: Some([0.0, 0.0, 10.0, 10.0]),
            max_px: 40,
        };
        let p = preview(&path, Luma::default(), 10.0, 10.0, &o, &[], &big).unwrap();
        assert_eq!((p.bitmap.width, p.bitmap.height), (40, 40));
        assert_eq!(p.covered_mm, [0.0, 6.0, 4.0, 10.0]);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn adjustments_and_mask_show_in_the_preview() {
        let path = gradient("adj", 100, 100);
        let view = PreviewView {
            region_mm: None,
            max_px: 1000,
        };
        let plain = preview(&path, Luma::default(), 10.0, 10.0, &opts(0.5), &[], &view).unwrap();
        let bright = ProcessOptions {
            adjust: ImageAdjust {
                brightness: 0.5,
                ..Default::default()
            },
            ..opts(0.5)
        };
        let lighter = preview(&path, Luma::default(), 10.0, 10.0, &bright, &[], &view).unwrap();
        // Más brillo = menos potencia en total.
        let sum = |p: &Preview| p.bitmap.data.iter().sum::<f32>();
        assert!(sum(&lighter) < sum(&plain));
        // Máscara de la mitad izquierda: la derecha queda apagada.
        let half = Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(5.0, 0.0),
                Point::new(5.0, 10.0),
                Point::new(0.0, 10.0),
            ],
            closed: true,
        };
        let masked = preview(
            &path,
            Luma::default(),
            10.0,
            10.0,
            &opts(0.5),
            &[half],
            &view,
        )
        .unwrap();
        assert!(masked.bitmap.get(18, 5) == 0.0 && masked.bitmap.get(1, 5) > 0.0);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn rejects_bad_requests() {
        let path = gradient("bad", 20, 20);
        let bad_region = PreviewView {
            region_mm: Some([5.0, 5.0, 5.0, 8.0]),
            max_px: 100,
        };
        assert!(
            preview(
                &path,
                Luma::default(),
                10.0,
                10.0,
                &opts(0.5),
                &[],
                &bad_region
            )
            .is_err()
        );
        let full = PreviewView {
            region_mm: None,
            max_px: 100,
        };
        assert!(preview(&path, Luma::default(), 0.0, 10.0, &opts(0.5), &[], &full).is_err());
        assert!(preview(&path, Luma::default(), 10.0, 10.0, &opts(0.0), &[], &full).is_err());
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn is_deterministic() {
        let path = gradient("det", 80, 60);
        let o = ProcessOptions {
            mode: RasterMode::FloydSteinberg,
            ..opts(0.25)
        };
        let view = PreviewView {
            region_mm: Some([2.0, 1.0, 8.0, 5.0]),
            max_px: 500,
        };
        let a = preview(&path, Luma::default(), 10.0, 7.5, &o, &[], &view).unwrap();
        let b = preview(&path, Luma::default(), 10.0, 7.5, &o, &[], &view).unwrap();
        assert_eq!(a, b);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
