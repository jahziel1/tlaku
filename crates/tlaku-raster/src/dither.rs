//! Conversión de gris a potencia: umbral, difusión de error, matrices de Bayer
//! y escala de grises. Los kernels son los publicados en la literatura:
//! Floyd-Steinberg (1976), Jarvis-Judice-Ninke (1976), Stucki (1981), Atkinson
//! (Macintosh, 1984) y Bayer (1973).

use crate::gray::GrayF32;
use crate::{screen, sketch};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BayerSize {
    N2,
    N4,
    N8,
}

impl BayerSize {
    fn n(self) -> usize {
        match self {
            BayerSize::N2 => 2,
            BayerSize::N4 => 4,
            BayerSize::N8 => 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum RasterMode {
    /// Enciende los píxeles con gris menor que el umbral (0..1).
    Threshold(f32),
    FloydSteinberg,
    Jarvis,
    #[default]
    Stucki,
    Atkinson,
    Bayer(BayerSize),
    /// Conserva los niveles: potencia = 1 - gris.
    Grayscale,
    /// Puntos de tamaño variable en celdas de `cell_mm` sobre una rejilla girada `angle_deg`.
    Halftone {
        cell_mm: f64,
        angle_deg: f64,
    },
    /// Líneas de grosor variable con periodo `cell_mm`, giradas `angle_deg`.
    Newsprint {
        cell_mm: f64,
        angle_deg: f64,
    },
    /// Bordes de la imagen.
    Sketch,
}

/// Kernel de difusión: `(dx, dy, peso)` hacia píxeles pendientes y divisor.
struct Kernel {
    taps: &'static [(i32, usize, f32)],
    divisor: f32,
}

const FLOYD_STEINBERG: Kernel = Kernel {
    taps: &[(1, 0, 7.0), (-1, 1, 3.0), (0, 1, 5.0), (1, 1, 1.0)],
    divisor: 16.0,
};

const JARVIS: Kernel = Kernel {
    taps: &[
        (1, 0, 7.0),
        (2, 0, 5.0),
        (-2, 1, 3.0),
        (-1, 1, 5.0),
        (0, 1, 7.0),
        (1, 1, 5.0),
        (2, 1, 3.0),
        (-2, 2, 1.0),
        (-1, 2, 3.0),
        (0, 2, 5.0),
        (1, 2, 3.0),
        (2, 2, 1.0),
    ],
    divisor: 48.0,
};

const STUCKI: Kernel = Kernel {
    taps: &[
        (1, 0, 8.0),
        (2, 0, 4.0),
        (-2, 1, 2.0),
        (-1, 1, 4.0),
        (0, 1, 8.0),
        (1, 1, 4.0),
        (2, 1, 2.0),
        (-2, 2, 1.0),
        (-1, 2, 2.0),
        (0, 2, 4.0),
        (1, 2, 2.0),
        (2, 2, 1.0),
    ],
    divisor: 42.0,
};

/// Solo propaga 6/8 del error.
const ATKINSON: Kernel = Kernel {
    taps: &[
        (1, 0, 1.0),
        (2, 0, 1.0),
        (-1, 1, 1.0),
        (0, 1, 1.0),
        (1, 1, 1.0),
        (0, 2, 1.0),
    ],
    divisor: 8.0,
};

/// Matriz de Bayer `n x n` (valores 0..n²-1) por la recurrencia
/// `M(2n) = [[4M, 4M+2], [4M+3, 4M+1]]`.
fn bayer_matrix(n: usize) -> Vec<u32> {
    let mut m = vec![0u32];
    let mut size = 1;
    while size < n {
        let next = size * 2;
        let mut out = vec![0u32; next * next];
        for y in 0..size {
            for x in 0..size {
                let v = 4 * m[y * size + x];
                out[y * next + x] = v;
                out[y * next + x + size] = v + 2;
                out[(y + size) * next + x] = v + 3;
                out[(y + size) * next + x + size] = v + 1;
            }
        }
        m = out;
        size = next;
    }
    m
}

/// Convierte la imagen (ya a la resolución final) en potencia 0..1. `pixel_mm` es el
/// tamaño físico de un píxel `(x, y)`; solo lo usan los modos con unidades en mm
/// (semitono, newsprint y sketch). Con
/// `serpentine`, los modos de difusión recorren las filas impares de derecha a
/// izquierda (con el kernel espejado), lo que reduce las vetas direccionales.
pub fn to_power(
    img: &GrayF32,
    mode: RasterMode,
    serpentine: bool,
    pixel_mm: (f64, f64),
) -> Vec<f32> {
    match mode {
        RasterMode::Threshold(t) => img
            .data
            .iter()
            .map(|&g| if g < t { 1.0 } else { 0.0 })
            .collect(),
        RasterMode::Grayscale => img.data.iter().map(|&g| 1.0 - g.clamp(0.0, 1.0)).collect(),
        RasterMode::FloydSteinberg => diffuse(img, &FLOYD_STEINBERG, serpentine),
        RasterMode::Jarvis => diffuse(img, &JARVIS, serpentine),
        RasterMode::Stucki => diffuse(img, &STUCKI, serpentine),
        RasterMode::Atkinson => diffuse(img, &ATKINSON, serpentine),
        RasterMode::Bayer(size) => {
            let n = size.n();
            let m = bayer_matrix(n);
            let cells = (n * n) as f32;
            let mut out = Vec::with_capacity(img.data.len());
            for y in 0..img.height {
                for x in 0..img.width {
                    let threshold = (m[(y % n) * n + x % n] as f32 + 0.5) / cells;
                    out.push(if img.get(x, y) < threshold { 1.0 } else { 0.0 });
                }
            }
            out
        }
        RasterMode::Halftone { cell_mm, angle_deg } => {
            screen::halftone(img, cell_mm, angle_deg, pixel_mm)
        }
        RasterMode::Newsprint { cell_mm, angle_deg } => {
            screen::newsprint(img, cell_mm, angle_deg, pixel_mm)
        }
        RasterMode::Sketch => sketch::sketch(img, pixel_mm),
    }
}

fn diffuse(img: &GrayF32, kernel: &Kernel, serpentine: bool) -> Vec<f32> {
    let (w, h) = (img.width, img.height);
    let mut work = img.data.clone();
    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        let reverse = serpentine && y % 2 == 1;
        let dir: i32 = if reverse { -1 } else { 1 };
        for i in 0..w {
            let x = if reverse { w - 1 - i } else { i };
            let v = work[y * w + x];
            let q = if v < 0.5 { 0.0 } else { 1.0 };
            out[y * w + x] = 1.0 - q;
            let err = v - q;
            for &(dx, dy, wt) in kernel.taps {
                let nx = x as i32 + dx * dir;
                let ny = y + dy;
                if nx >= 0 && (nx as usize) < w && ny < h {
                    work[ny * w + nx as usize] += err * wt / kernel.divisor;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Los modos de este módulo no dependen del tamaño del píxel.
    fn to_power(img: &GrayF32, mode: RasterMode, serpentine: bool) -> Vec<f32> {
        super::to_power(img, mode, serpentine, (0.1, 0.1))
    }

    fn flat(v: f32, w: usize, h: usize) -> GrayF32 {
        GrayF32::from_fn(w, h, |_, _| v)
    }

    fn lit(data: &[f32]) -> f64 {
        data.iter().filter(|&&v| v > 0.0).count() as f64 / data.len() as f64
    }

    const ALL: [RasterMode; 8] = [
        RasterMode::Threshold(0.5),
        RasterMode::FloydSteinberg,
        RasterMode::Jarvis,
        RasterMode::Stucki,
        RasterMode::Atkinson,
        RasterMode::Bayer(BayerSize::N2),
        RasterMode::Bayer(BayerSize::N4),
        RasterMode::Bayer(BayerSize::N8),
    ];

    #[test]
    fn kernel_weights_add_up() {
        for k in [&FLOYD_STEINBERG, &JARVIS, &STUCKI] {
            assert_eq!(k.taps.iter().map(|t| t.2).sum::<f32>(), k.divisor);
        }
        assert_eq!(ATKINSON.taps.iter().map(|t| t.2).sum::<f32>(), 6.0);
    }

    #[test]
    fn bayer_matrices_match_published() {
        assert_eq!(bayer_matrix(2), vec![0, 2, 3, 1]);
        assert_eq!(
            bayer_matrix(4),
            vec![0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5]
        );
        let m8 = bayer_matrix(8);
        let mut sorted = m8.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..64).collect::<Vec<_>>());
    }

    #[test]
    fn mid_gray_gives_about_half_lit() {
        // Umbral: 0.4999 todo encendido y 0.5 todo apagado; el 50 % aplica a los demás.
        for mode in &ALL[1..] {
            for serpentine in [false, true] {
                let p = to_power(&flat(0.5, 64, 64), *mode, serpentine);
                let f = lit(&p);
                // Atkinson descarta parte del error, así que se desvía más en los medios.
                let tol = if *mode == RasterMode::Atkinson {
                    0.2
                } else {
                    0.05
                };
                assert!(
                    (f - 0.5).abs() < tol,
                    "{mode:?} serpentina={serpentine}: {f}"
                );
            }
        }
    }

    #[test]
    fn diffusion_preserves_average_darkness() {
        for gray in [0.1f32, 0.25, 0.75, 0.9] {
            for mode in [
                RasterMode::FloydSteinberg,
                RasterMode::Jarvis,
                RasterMode::Stucki,
            ] {
                let p = to_power(&flat(gray, 80, 80), mode, true);
                assert!(
                    (lit(&p) - f64::from(1.0 - gray)).abs() < 0.03,
                    "{mode:?} gris {gray}"
                );
            }
        }
    }

    #[test]
    fn bayer_density_matches_gray() {
        for size in [BayerSize::N2, BayerSize::N4, BayerSize::N8] {
            let p = to_power(&flat(0.25, 64, 64), RasterMode::Bayer(size), false);
            // Con n²=4, 0.25 cae justo entre umbrales; basta con ±1/n².
            assert!((lit(&p) - 0.75).abs() <= 0.25 + 1e-9, "{size:?}");
        }
        let p = to_power(&flat(0.25, 64, 64), RasterMode::Bayer(BayerSize::N8), false);
        assert!((lit(&p) - 0.75).abs() < 0.02);
    }

    #[test]
    fn extremes_are_pure() {
        for mode in ALL {
            assert!(
                to_power(&flat(1.0, 9, 9), mode, true)
                    .iter()
                    .all(|&v| v == 0.0),
                "{mode:?}"
            );
            assert!(
                to_power(&flat(0.0, 9, 9), mode, true)
                    .iter()
                    .all(|&v| v == 1.0),
                "{mode:?}"
            );
        }
    }

    #[test]
    fn outputs_are_binary_except_grayscale() {
        let img = GrayF32::from_fn(30, 20, |x, y| ((x * 31 + y * 17) % 50) as f32 / 49.0);
        for mode in ALL {
            assert!(
                to_power(&img, mode, true)
                    .iter()
                    .all(|&v| v == 0.0 || v == 1.0)
            );
        }
    }

    #[test]
    fn threshold_boundary() {
        let img = GrayF32::new(3, 1, vec![0.29, 0.3, 0.31]).unwrap();
        assert_eq!(
            to_power(&img, RasterMode::Threshold(0.3), false),
            vec![1.0, 0.0, 0.0]
        );
        assert_eq!(
            to_power(&img, RasterMode::Threshold(0.0), false),
            vec![0.0; 3]
        );
        assert_eq!(
            to_power(&img, RasterMode::Threshold(1.0), false),
            vec![1.0; 3]
        );
    }

    #[test]
    fn grayscale_keeps_levels() {
        let img = GrayF32::new(3, 1, vec![0.0, 0.25, 1.0]).unwrap();
        assert_eq!(
            to_power(&img, RasterMode::Grayscale, false),
            vec![1.0, 0.75, 0.0]
        );
    }

    #[test]
    fn serpentine_differs_but_same_density() {
        let img = GrayF32::from_fn(40, 40, |x, _| x as f32 / 39.0);
        let a = to_power(&img, RasterMode::FloydSteinberg, false);
        let b = to_power(&img, RasterMode::FloydSteinberg, true);
        assert_ne!(a, b);
        assert!((lit(&a) - lit(&b)).abs() < 0.02);
    }

    #[test]
    fn tiny_images_do_not_panic() {
        for mode in ALL {
            to_power(&flat(0.4, 1, 1), mode, true);
            to_power(&flat(0.4, 1, 5), mode, true);
            to_power(&flat(0.4, 5, 1), mode, true);
        }
    }
}
