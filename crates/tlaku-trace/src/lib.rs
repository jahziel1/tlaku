//! Vectorizado de imágenes (Trace Image): convierte un mapa de bits en contornos
//! (`BezPath`) listos para cortar o grabar.
//!
//! Flujo: alfa sobre blanco, luminancia, desenfoque opcional, umbral, limpieza de manchas y
//! vectorizado. El modo `spline` usa vtracer y conserva sus cúbicas; el modo `polygon` sigue
//! el borde de los píxeles y lo simplifica con tolerancia (solo rectas).
//!
//! Salida en mm: la fila 0 de la imagen queda arriba, Y crece hacia arriba y el origen es la
//! esquina inferior izquierda. Cada `BezPath` es una forma con su contorno exterior y sus
//! huecos como subtrayectos cerrados: se rellena con la regla par-impar (una "O" es un
//! `BezPath` con dos subtrayectos).

mod binarize;
mod polygon;
mod spline;

use image::DynamicImage;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tlaku_core::Point;
use tlaku_core::bezpath::{BezPath, PathSeg};

/// Lado máximo de la imagen, en píxeles.
pub const MAX_DIMENSION_PX: u32 = 8192;
/// Área máxima de la imagen, en píxeles (16 Mpx, p. ej. 4000x4000).
pub const MAX_PIXELS: u64 = 16_000_000;

/// Forma de las curvas resultantes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceMode {
    /// Curvas cúbicas suaves (vtracer).
    #[default]
    Spline,
    /// Solo segmentos rectos.
    Polygon,
}

/// Ajustes del trazado.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", default)]
pub struct TraceSettings {
    /// Umbral de brillo 0..1: es tinta lo que tiene luminancia menor (0 nada, 1 casi todo).
    pub threshold: f64,
    /// Traza lo claro en vez de lo oscuro.
    pub invert: bool,
    /// Sigma (px) de un desenfoque previo que alisa bordes ruidosos; 0 lo desactiva.
    pub blur_px: f64,
    /// Lado (px) de la mancha más pequeña que se conserva: se descartan las manchas de tinta
    /// y se rellenan los huecos internos de menos de `speckle_px`² píxeles. 0 o 1 no filtra.
    pub speckle_px: u32,
    /// Ángulo (grados, 0..180) a partir del cual un giro se trata como esquina viva (spline).
    pub corner_threshold_deg: f64,
    /// Suavidad 0..1: más alto da menos nodos y curvas más largas (spline) o una
    /// simplificación más tolerante (polygon).
    pub smoothing: f64,
    pub mode: TraceMode,
}

impl Default for TraceSettings {
    /// Valores pensados para logotipos y dibujos de línea.
    fn default() -> Self {
        Self {
            threshold: 0.5,
            invert: false,
            blur_px: 0.0,
            speckle_px: 4,
            corner_threshold_deg: 60.0,
            smoothing: 0.2,
            mode: TraceMode::Spline,
        }
    }
}

/// Resultado del trazado.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TraceResult {
    /// Una forma por elemento, con sus huecos como subtrayectos (regla par-impar).
    pub paths: Vec<BezPath>,
    pub warnings: Vec<String>,
}

/// Traza un archivo de imagen (PNG, JPEG o BMP).
pub fn trace_path(
    path: impl AsRef<Path>,
    settings: &TraceSettings,
    size_mm: [f64; 2],
) -> Result<TraceResult, String> {
    let path = path.as_ref();
    let bytes = std::fs::read(path)
        .map_err(|e| format!("No se pudo leer la imagen '{}': {e}", path.display()))?;
    trace_bytes(&bytes, settings, size_mm)
}

/// Traza una imagen codificada (PNG, JPEG o BMP).
pub fn trace_bytes(
    bytes: &[u8],
    settings: &TraceSettings,
    size_mm: [f64; 2],
) -> Result<TraceResult, String> {
    let image = image::load_from_memory(bytes)
        .map_err(|e| format!("No se pudo decodificar la imagen: {e}"))?;
    trace(&image, settings, size_mm)
}

fn validate(settings: &TraceSettings, size_mm: [f64; 2]) -> Result<(), String> {
    if !(size_mm[0].is_finite() && size_mm[1].is_finite() && size_mm[0] > 0.0 && size_mm[1] > 0.0) {
        return Err("El tamaño de salida en mm debe ser positivo y finito".into());
    }
    if !(0.0..=1.0).contains(&settings.threshold) {
        return Err("El umbral debe estar entre 0 y 1".into());
    }
    if !(0.0..=50.0).contains(&settings.blur_px) {
        return Err("El desenfoque debe estar entre 0 y 50 px".into());
    }
    if !(0.0..=180.0).contains(&settings.corner_threshold_deg) {
        return Err("El ángulo de esquina debe estar entre 0 y 180 grados".into());
    }
    if !(0.0..=1.0).contains(&settings.smoothing) {
        return Err("La suavidad debe estar entre 0 y 1".into());
    }
    Ok(())
}

/// Vectoriza `image` en contornos de `size_mm` (ancho, alto).
pub fn trace(
    image: &DynamicImage,
    settings: &TraceSettings,
    size_mm: [f64; 2],
) -> Result<TraceResult, String> {
    validate(settings, size_mm)?;
    let (w, h) = (image.width(), image.height());
    if w == 0 || h == 0 {
        return Err("La imagen está vacía".into());
    }
    if w > MAX_DIMENSION_PX || h > MAX_DIMENSION_PX || u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(format!(
            "La imagen de {w}x{h} px es demasiado grande: el máximo es {MAX_DIMENSION_PX} px por \
             lado y {} megapíxeles en total. Redúcela antes de trazar",
            MAX_PIXELS / 1_000_000
        ));
    }

    let mut bitmap =
        binarize::binarize(image, settings.threshold, settings.invert, settings.blur_px);
    let mut warnings = Vec::new();

    let side = settings.speckle_px as usize;
    let removed = bitmap.remove_specks(side.saturating_mul(side));
    if removed > 0 {
        warnings.push(format!(
            "Se descartaron {removed} manchas menores de {} px de lado",
            settings.speckle_px
        ));
    }
    if bitmap.is_blank() {
        warnings.push(
            "La imagen no tiene zonas que trazar con estos ajustes; revisa el umbral o la opción \
             de invertir"
                .into(),
        );
        return Ok(TraceResult {
            paths: Vec::new(),
            warnings,
        });
    }

    let to_mm = Mapper {
        sx: size_mm[0] / f64::from(w),
        sy: size_mm[1] / f64::from(h),
        height_px: f64::from(h),
    };
    let paths = match settings.mode {
        TraceMode::Spline => {
            let params = spline::SplineParams {
                corner_threshold_deg: settings.corner_threshold_deg.round() as i32,
                // Rango útil de vtracer: 3.5 (detallado) a 10 (muy alisado).
                length_threshold: 3.5 + settings.smoothing * 6.5,
            };
            spline::trace_splines(&bitmap, &params, &mut warnings)?
                .iter()
                .map(|shape| cubic_shape_to_path(shape, &to_mm))
                .collect::<Vec<_>>()
        }
        TraceMode::Polygon => {
            // Por debajo de ~0.75 px una diagonal en escalera no se colapsa a una recta.
            let tolerance = 0.75 + settings.smoothing * 1.5;
            polygon::trace_polygons(&bitmap, tolerance)
                .iter()
                .map(|shape| polygon_shape_to_path(shape, &to_mm))
                .collect()
        }
    };
    if paths.is_empty() {
        warnings.push("No se obtuvo ninguna trayectoria".into());
    }
    Ok(TraceResult { paths, warnings })
}

/// Píxeles (Y hacia abajo) a mm (Y hacia arriba, origen abajo a la izquierda).
struct Mapper {
    sx: f64,
    sy: f64,
    height_px: f64,
}

impl Mapper {
    fn point(&self, (x, y): (f64, f64)) -> Point {
        Point::new(x * self.sx, (self.height_px - y) * self.sy)
    }
}

fn cubic_shape_to_path(shape: &spline::CubicShape, m: &Mapper) -> BezPath {
    let mut segs = Vec::new();
    for (start, curves) in shape {
        segs.push(PathSeg::MoveTo {
            to: m.point(*start),
        });
        for [c1, c2, to] in curves {
            segs.push(PathSeg::CubicTo {
                ctrl1: m.point(*c1),
                ctrl2: m.point(*c2),
                to: m.point(*to),
            });
        }
        segs.push(PathSeg::Close);
    }
    BezPath(segs)
}

fn polygon_shape_to_path(shape: &polygon::PixelShape, m: &Mapper) -> BezPath {
    let mut segs = Vec::new();
    for lazo in shape {
        for (i, &p) in lazo.iter().enumerate() {
            let to = m.point(p);
            segs.push(if i == 0 {
                PathSeg::MoveTo { to }
            } else {
                PathSeg::LineTo { to }
            });
        }
        segs.push(PathSeg::Close);
    }
    BezPath(segs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GrayImage, Luma};

    /// Imagen blanca con tinta negra donde `ink(x, y)` es cierto.
    fn img(w: u32, h: u32, ink: impl Fn(u32, u32) -> bool) -> DynamicImage {
        DynamicImage::ImageLuma8(GrayImage::from_fn(w, h, |x, y| {
            Luma([if ink(x, y) { 0 } else { 255 }])
        }))
    }

    fn disc(cx: f64, cy: f64, r: f64) -> impl Fn(u32, u32) -> bool {
        move |x, y| (f64::from(x) + 0.5 - cx).hypot(f64::from(y) + 0.5 - cy) <= r
    }

    fn signed_area(poly: &[Point]) -> f64 {
        let n = poly.len();
        (0..n)
            .map(|i| poly[i].x * poly[(i + 1) % n].y - poly[(i + 1) % n].x * poly[i].y)
            .sum::<f64>()
            / 2.0
    }

    fn flat_loops(p: &BezPath) -> Vec<Vec<Point>> {
        p.flatten(0.01).into_iter().map(|l| l.points).collect()
    }

    fn bbox(p: &BezPath) -> (f64, f64, f64, f64) {
        let pts: Vec<Point> = flat_loops(p).into_iter().flatten().collect();
        let min = |f: fn(&Point) -> f64| pts.iter().map(f).fold(f64::INFINITY, f64::min);
        let max = |f: fn(&Point) -> f64| pts.iter().map(f).fold(f64::NEG_INFINITY, f64::max);
        (min(|p| p.x), min(|p| p.y), max(|p| p.x), max(|p| p.y))
    }

    #[test]
    fn circle_area_matches_pi_r2() {
        // 400x400 px a 100x100 mm: 0.25 mm/px; círculo de 150 px de radio = 37.5 mm.
        let image = img(400, 400, disc(200.0, 200.0, 150.0));
        let res = trace(&image, &TraceSettings::default(), [100.0, 100.0]).unwrap();
        assert_eq!(res.paths.len(), 1);
        let loops = flat_loops(&res.paths[0]);
        assert_eq!(loops.len(), 1);
        let area = signed_area(&loops[0]).abs();
        let expected = std::f64::consts::PI * 37.5 * 37.5;
        assert!(
            (area - expected).abs() / expected < 0.02,
            "área {area} vs {expected}"
        );
        // Usa cúbicas reales.
        assert!(
            res.paths[0]
                .0
                .iter()
                .any(|s| matches!(s, PathSeg::CubicTo { .. }))
        );
    }

    #[test]
    fn ring_has_two_subpaths_even_odd() {
        let outer = disc(200.0, 200.0, 150.0);
        let inner = disc(200.0, 200.0, 80.0);
        let image = img(400, 400, move |x, y| outer(x, y) && !inner(x, y));
        for mode in [TraceMode::Spline, TraceMode::Polygon] {
            let settings = TraceSettings {
                mode,
                ..TraceSettings::default()
            };
            let res = trace(&image, &settings, [100.0, 100.0]).unwrap();
            assert_eq!(res.paths.len(), 1, "{mode:?}");
            let loops = flat_loops(&res.paths[0]);
            assert_eq!(loops.len(), 2, "{mode:?}");
            let mut areas: Vec<f64> = loops.iter().map(|l| signed_area(l).abs()).collect();
            areas.sort_by(f64::total_cmp);
            let ring = areas[1] - areas[0];
            let expected = std::f64::consts::PI * (37.5f64.powi(2) - 20.0f64.powi(2));
            assert!(
                (ring - expected).abs() / expected < 0.03,
                "{mode:?}: anillo {ring} vs {expected}"
            );
        }
    }

    #[test]
    fn speckles_are_removed() {
        // Un cuadrado grande y dos manchas de 2x2 px.
        let image = img(200, 200, |x, y| {
            (50..150).contains(&x) && (50..150).contains(&y)
                || (5..7).contains(&x) && (5..7).contains(&y)
                || (190..192).contains(&x) && (10..12).contains(&y)
        });
        let none = TraceSettings {
            speckle_px: 0,
            ..TraceSettings::default()
        };
        assert_eq!(trace(&image, &none, [200.0, 200.0]).unwrap().paths.len(), 3);
        let res = trace(&image, &TraceSettings::default(), [200.0, 200.0]).unwrap();
        assert_eq!(res.paths.len(), 1);
        assert!(res.warnings.iter().any(|w| w.contains("descartaron 2")));
    }

    #[test]
    fn small_holes_are_filled() {
        let image = img(100, 100, |x, y| {
            (10..90).contains(&x)
                && (10..90).contains(&y)
                && !((50..52).contains(&x) && (50..52).contains(&y))
        });
        let res = trace(&image, &TraceSettings::default(), [100.0, 100.0]).unwrap();
        assert_eq!(flat_loops(&res.paths[0]).len(), 1);
    }

    #[test]
    fn invert_traces_the_light_area() {
        let image = img(100, 100, |x, y| {
            (30..70).contains(&x) && (30..70).contains(&y)
        });
        let normal = trace(&image, &TraceSettings::default(), [100.0, 100.0]).unwrap();
        let (x0, _, x1, _) = bbox(&normal.paths[0]);
        assert!((x1 - x0 - 40.0).abs() < 1.0);
        let inv = TraceSettings {
            invert: true,
            ..TraceSettings::default()
        };
        let res = trace(&image, &inv, [100.0, 100.0]).unwrap();
        // El fondo claro con el cuadrado como hueco.
        assert_eq!(res.paths.len(), 1);
        assert_eq!(flat_loops(&res.paths[0]).len(), 2);
        let (x0, _, x1, _) = bbox(&res.paths[0]);
        assert!((x1 - x0 - 100.0).abs() < 1.0);
    }

    #[test]
    fn polygon_mode_only_has_lines() {
        let image = img(300, 300, disc(150.0, 150.0, 100.0));
        let settings = TraceSettings {
            mode: TraceMode::Polygon,
            ..TraceSettings::default()
        };
        let res = trace(&image, &settings, [30.0, 30.0]).unwrap();
        assert_eq!(res.paths.len(), 1);
        let segs = &res.paths[0].0;
        assert!(segs.iter().all(|s| matches!(
            s,
            PathSeg::MoveTo { .. } | PathSeg::LineTo { .. } | PathSeg::Close
        )));
        // Simplificado: muy lejos de un nodo por píxel de borde.
        assert!(segs.len() < 200, "{} segmentos", segs.len());
        let area = signed_area(&flat_loops(&res.paths[0])[0]).abs();
        let expected = std::f64::consts::PI * 10.0 * 10.0;
        assert!((area - expected).abs() / expected < 0.03);
    }

    #[test]
    fn scale_and_orientation() {
        // Cuadrado arriba a la izquierda: x 10..30, y 10..30 (px) de una imagen 100x50.
        let image = img(100, 50, |x, y| {
            (10..30).contains(&x) && (10..30).contains(&y)
        });
        for mode in [TraceMode::Spline, TraceMode::Polygon] {
            let settings = TraceSettings {
                mode,
                ..TraceSettings::default()
            };
            // 200x100 mm: 2 mm/px.
            let res = trace(&image, &settings, [200.0, 100.0]).unwrap();
            assert_eq!(res.paths.len(), 1);
            let (x0, y0, x1, y1) = bbox(&res.paths[0]);
            let tol = 1.0;
            assert!(
                (x0 - 20.0).abs() < tol && (x1 - 60.0).abs() < tol,
                "{mode:?} x {x0} {x1}"
            );
            // Filas 10..30 de 50, con Y hacia arriba: 100-60 .. 100-20.
            assert!(
                (y0 - 40.0).abs() < tol && (y1 - 80.0).abs() < tol,
                "{mode:?} y {y0} {y1}"
            );
        }
    }

    #[test]
    fn blank_image_gives_zero_paths_with_warning() {
        let image = img(50, 50, |_, _| false);
        let res = trace(&image, &TraceSettings::default(), [10.0, 10.0]).unwrap();
        assert!(res.paths.is_empty());
        assert!(!res.warnings.is_empty());
    }

    #[test]
    fn transparent_pixels_count_as_white() {
        let rgba = image::RgbaImage::from_fn(40, 40, |x, y| {
            if (10..30).contains(&x) && (10..30).contains(&y) {
                image::Rgba([0, 0, 0, 255])
            } else {
                image::Rgba([0, 0, 0, 0])
            }
        });
        let res = trace(
            &DynamicImage::ImageRgba8(rgba),
            &TraceSettings::default(),
            [40.0, 40.0],
        )
        .unwrap();
        assert_eq!(res.paths.len(), 1);
    }

    #[test]
    fn blur_reduces_noise_nodes() {
        // Borde con ruido determinista: el desenfoque deja menos nodos en modo polygon.
        let image = img(200, 200, |x, y| {
            let wobble = y.wrapping_mul(2_654_435_761) >> 28; // 0..15 por fila
            x + wobble / 2 >= 60 && x < 140 && y > 40 && y < 160
        });
        let nodes = |blur: f64| {
            let s = TraceSettings {
                mode: TraceMode::Polygon,
                blur_px: blur,
                ..TraceSettings::default()
            };
            trace(&image, &s, [50.0, 50.0]).unwrap().paths[0].0.len()
        };
        assert!(nodes(4.0) < nodes(0.0));
    }

    #[test]
    fn errors_are_clear() {
        let big = DynamicImage::ImageLuma8(GrayImage::new(MAX_DIMENSION_PX + 1, 10));
        let err = trace(&big, &TraceSettings::default(), [10.0, 10.0]).unwrap_err();
        assert!(err.contains("demasiado grande"));
        let small = img(10, 10, |_, _| true);
        assert!(trace(&small, &TraceSettings::default(), [0.0, 10.0]).is_err());
        let bad = TraceSettings {
            threshold: 2.0,
            ..TraceSettings::default()
        };
        assert!(trace(&small, &bad, [10.0, 10.0]).is_err());
        assert!(trace_bytes(b"no es una imagen", &TraceSettings::default(), [1.0, 1.0]).is_err());
    }

    #[test]
    fn many_shapes_use_per_component_fallback() {
        // 200x200 = 40000 bloques de 2x2 px: supera el límite de vtracer sobre la imagen completa.
        let image = img(800, 800, |x, y| x % 4 < 2 && y % 4 < 2);
        let settings = TraceSettings {
            speckle_px: 0,
            ..TraceSettings::default()
        };
        let res = trace(&image, &settings, [80.0, 80.0]).unwrap();
        assert_eq!(res.paths.len(), 40_000);
        let (x0, y0, _, _) = bbox(&res.paths[0]);
        assert!((0.0..80.0).contains(&x0) && (0.0..80.0).contains(&y0));
    }

    #[test]
    fn trace_bytes_decodes_png() {
        let image = img(60, 60, |x, y| {
            (10..50).contains(&x) && (10..50).contains(&y)
        });
        let mut buf = std::io::Cursor::new(Vec::new());
        image.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        let res = trace_bytes(buf.get_ref(), &TraceSettings::default(), [60.0, 60.0]).unwrap();
        assert_eq!(res.paths.len(), 1);
    }

    /// Rendimiento: `cargo test -p tlaku-trace --release -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn perf_2000x2000() {
        // Logotipo con muchas formas: anillos con un punto central en rejilla.
        let image = img(2000, 2000, |x, y| {
            let (gx, gy) = (f64::from(x % 100) - 50.0, f64::from(y % 100) - 50.0);
            let r = gx.hypot(gy);
            (15.0..40.0).contains(&r) || r < 8.0
        });
        for mode in [TraceMode::Spline, TraceMode::Polygon] {
            let s = TraceSettings {
                mode,
                ..TraceSettings::default()
            };
            let t = std::time::Instant::now();
            let res = trace(&image, &s, [200.0, 200.0]).unwrap();
            println!("{mode:?}: {} formas en {:?}", res.paths.len(), t.elapsed());
        }
        // Caso adverso: ruido de bloques de 3 px (miles de formas pequeñas).
        let noise = img(2000, 2000, |x, y| {
            ((x / 3).wrapping_mul(73_856_093) ^ (y / 3).wrapping_mul(19_349_663)) >> 7 & 1 == 1
        });
        let t = std::time::Instant::now();
        let res = trace(&noise, &TraceSettings::default(), [200.0, 200.0]).unwrap();
        println!("ruido: {} formas en {:?}", res.paths.len(), t.elapsed());
    }
}
