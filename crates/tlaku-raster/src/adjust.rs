//! Ajustes de imagen previos al remuestreo: quitar motas, brillo, contraste, gamma,
//! enfoque (máscara de enfoque) e inversión.

use crate::gray::GrayF32;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageAdjust {
    /// Desplazamiento aditivo, -1..1 (0 = sin cambio).
    pub brightness: f32,
    /// -1..1 (0 = sin cambio). Escala los valores alrededor del gris medio; 1 deja
    /// la imagen casi binaria y -1 la aplana a gris.
    pub contrast: f32,
    /// Exponente `v^gamma`: 1 = sin cambio, mayor oscurece los medios tonos y menor
    /// los aclara.
    pub gamma: f32,
    /// Negativo (luz <-> sombra).
    pub invert: bool,
    /// Radio de la máscara de enfoque (sigma gaussiano) en píxeles de la imagen
    /// original; 0 la desactiva.
    pub enhance_radius_px: f32,
    /// Intensidad de la máscara de enfoque; 0 la desactiva.
    pub enhance_amount: f32,
    /// Tamaño máximo, en píxeles de la imagen original, de las motas que se quitan; 0 lo
    /// desactiva. Se aplica con un filtro de mediana antes de todo lo demás (ver
    /// [`median_radius_for_speck`]).
    pub denoise_px: u32,
}

/// Mayor `denoise_px` admitido (ventana de mediana de hasta 25 x 25).
pub const MAX_DENOISE_PX: u32 = 16;

impl Default for ImageAdjust {
    fn default() -> Self {
        Self {
            brightness: 0.0,
            contrast: 0.0,
            gamma: 1.0,
            invert: false,
            enhance_radius_px: 0.0,
            enhance_amount: 0.0,
            denoise_px: 0,
        }
    }
}

impl ImageAdjust {
    pub fn validate(&self) -> Result<(), String> {
        if !(-1.0..=1.0).contains(&self.brightness) {
            return Err(format!("brillo fuera de -1..1: {}", self.brightness));
        }
        if !(-1.0..=1.0).contains(&self.contrast) {
            return Err(format!("contraste fuera de -1..1: {}", self.contrast));
        }
        if !(self.gamma.is_finite() && self.gamma > 0.0) {
            return Err(format!("gamma debe ser mayor que 0: {}", self.gamma));
        }
        if !(self.enhance_radius_px.is_finite() && self.enhance_radius_px >= 0.0) {
            return Err(format!(
                "radio de enfoque inválido: {}",
                self.enhance_radius_px
            ));
        }
        if !(self.enhance_amount.is_finite() && self.enhance_amount >= 0.0) {
            return Err(format!(
                "cantidad de enfoque inválida: {}",
                self.enhance_amount
            ));
        }
        if self.denoise_px > MAX_DENOISE_PX {
            return Err(format!(
                "quitar motas admite hasta {MAX_DENOISE_PX} px: {}",
                self.denoise_px
            ));
        }
        Ok(())
    }

    /// Aplica en orden: quitar motas, contraste y brillo, gamma, enfoque e inversión.
    pub fn apply(&self, img: &GrayF32) -> GrayF32 {
        let factor = if self.contrast >= 0.0 {
            1.0 / (1.0 - self.contrast).max(0.01)
        } else {
            1.0 + self.contrast
        };
        let mut out = if self.denoise_px > 0 {
            median_filter(img, median_radius_for_speck(self.denoise_px))
        } else {
            img.clone()
        };
        for v in &mut out.data {
            let mut x = ((*v - 0.5) * factor + 0.5 + self.brightness).clamp(0.0, 1.0);
            if self.gamma != 1.0 {
                x = x.powf(self.gamma);
            }
            *v = x;
        }
        if self.enhance_radius_px > 0.0 && self.enhance_amount > 0.0 {
            let blur = gaussian_blur(&out, self.enhance_radius_px);
            for (v, b) in out.data.iter_mut().zip(&blur.data) {
                *v = (*v + self.enhance_amount * (*v - b)).clamp(0.0, 1.0);
            }
        }
        if self.invert {
            for v in &mut out.data {
                *v = 1.0 - *v;
            }
        }
        out
    }
}

/// Radio de la ventana de mediana que borra una mota cuadrada de `size` x `size` píxeles.
///
/// Una mediana de ventana `(2r+1)^2` elimina cualquier mota de área menor que la mitad de la
/// ventana, y el cuadrado de lado `size` es la peor forma con su ancho: hace falta
/// `size^2 < (2r+1)^2 / 2`, es decir `2r+1 > size * sqrt(2)`.
pub fn median_radius_for_speck(size: u32) -> usize {
    let side = f64::from(size) * std::f64::consts::SQRT_2;
    (((side - 1.0) / 2.0).ceil().max(1.0)) as usize
}

/// Filtro de mediana cuadrado de radio `radius` (ventana `(2r+1)^2`), con los bordes
/// repitiendo el píxel extremo. Cuantiza a 256 niveles y usa un histograma deslizante
/// (Huang): el coste por píxel crece con el radio, no con el área de la ventana.
pub fn median_filter(img: &GrayF32, radius: usize) -> GrayF32 {
    let (w, h) = (img.width, img.height);
    let q: Vec<u8> = img
        .data
        .iter()
        .map(|&v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8)
        .collect();
    let side = 2 * radius + 1;
    let half = (side * side / 2) as u32;
    let clamp = |i: i64, n: usize| i.clamp(0, n as i64 - 1) as usize;
    // Columnas y filas de la ventana ya con el recorte de bordes aplicado.
    let rows_for = |y: usize| -> Vec<usize> {
        (0..side)
            .map(|k| clamp(y as i64 + k as i64 - radius as i64, h))
            .collect()
    };
    let mut out = vec![0.0f32; w * h];
    let mut hist = [0u32; 256];
    for y in 0..h {
        let rows = rows_for(y);
        hist.fill(0);
        for dx in 0..side {
            let sx = clamp(dx as i64 - radius as i64, w);
            for &sy in &rows {
                hist[q[sy * w + sx] as usize] += 1;
            }
        }
        // Mediana inicial: menor nivel m tal que (cuenta de niveles <= m) > half.
        let (mut m, mut below) = (0usize, 0u32);
        for x in 0..w {
            if x > 0 {
                let gone = clamp(x as i64 - 1 - radius as i64, w);
                let come = clamp(x as i64 + radius as i64, w);
                for &sy in &rows {
                    let a = q[sy * w + gone] as usize;
                    hist[a] -= 1;
                    if a < m {
                        below -= 1;
                    }
                    let b = q[sy * w + come] as usize;
                    hist[b] += 1;
                    if b < m {
                        below += 1;
                    }
                }
            }
            while below > half {
                m -= 1;
                below -= hist[m];
            }
            while below + hist[m] <= half {
                below += hist[m];
                m += 1;
            }
            out[y * w + x] = m as f32 / 255.0;
        }
    }
    GrayF32 {
        width: w,
        height: h,
        data: out,
    }
}

/// Desenfoque gaussiano separable; los bordes repiten el píxel extremo.
///
/// Horizontal con la fila copiada a un búfer con los bordes ya repetidos (sin recortar índices
/// por tap) y vertical acumulando filas completas, que es lo más amable con la caché.
fn gaussian_blur(img: &GrayF32, sigma: f32) -> GrayF32 {
    let radius = (sigma * 3.0).ceil() as usize;
    let mut kernel: Vec<f32> = (0..=2 * radius)
        .map(|i| {
            let d = i as f32 - radius as f32;
            (-(d * d) / (2.0 * sigma * sigma)).exp()
        })
        .collect();
    let sum: f32 = kernel.iter().sum();
    kernel.iter_mut().for_each(|k| *k /= sum);

    let (w, h) = (img.width, img.height);
    let mut tmp = vec![0.0f32; w * h];
    let mut pad = vec![0.0f32; w + 2 * radius];
    for y in 0..h {
        let row = &img.data[y * w..(y + 1) * w];
        pad[..radius].fill(row[0]);
        pad[radius..radius + w].copy_from_slice(row);
        pad[radius + w..].fill(row[w - 1]);
        let out = &mut tmp[y * w..(y + 1) * w];
        out.fill(0.0);
        for (k, &wt) in kernel.iter().enumerate() {
            for (o, &p) in out.iter_mut().zip(&pad[k..k + w]) {
                *o += wt * p;
            }
        }
    }
    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        let dst = &mut out[y * w..(y + 1) * w];
        for (k, &wt) in kernel.iter().enumerate() {
            let sy = (y + k).saturating_sub(radius).min(h - 1);
            for (o, &t) in dst.iter_mut().zip(&tmp[sy * w..(sy + 1) * w]) {
                *o += wt * t;
            }
        }
    }
    GrayF32 {
        width: w,
        height: h,
        data: out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(v: f32) -> GrayF32 {
        GrayF32::from_fn(4, 4, |_, _| v)
    }

    #[test]
    fn default_is_identity() {
        let img = GrayF32::from_fn(5, 3, |x, y| (x + y) as f32 / 8.0);
        assert_eq!(ImageAdjust::default().apply(&img), img);
    }

    #[test]
    fn brightness_and_clamp() {
        let a = ImageAdjust {
            brightness: 0.25,
            ..Default::default()
        };
        assert!((a.apply(&flat(0.5)).get(0, 0) - 0.75).abs() < 1e-6);
        assert_eq!(a.apply(&flat(0.9)).get(0, 0), 1.0);
    }

    #[test]
    fn contrast_pivots_on_mid_gray() {
        let hi = ImageAdjust {
            contrast: 0.5,
            ..Default::default()
        };
        assert!((hi.apply(&flat(0.5)).get(0, 0) - 0.5).abs() < 1e-6);
        assert!((hi.apply(&flat(0.6)).get(0, 0) - 0.7).abs() < 1e-5);
        let lo = ImageAdjust {
            contrast: -1.0,
            ..Default::default()
        };
        assert!((lo.apply(&flat(0.9)).get(0, 0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn gamma_darkens_above_one() {
        let a = ImageAdjust {
            gamma: 2.0,
            ..Default::default()
        };
        assert!((a.apply(&flat(0.5)).get(0, 0) - 0.25).abs() < 1e-6);
        let b = ImageAdjust {
            gamma: 0.5,
            ..Default::default()
        };
        assert!(b.apply(&flat(0.25)).get(0, 0) > 0.25);
    }

    #[test]
    fn invert() {
        let a = ImageAdjust {
            invert: true,
            ..Default::default()
        };
        assert!((a.apply(&flat(0.2)).get(0, 0) - 0.8).abs() < 1e-6);
    }

    #[test]
    fn unsharp_increases_edge_contrast_and_keeps_flat() {
        let a = ImageAdjust {
            enhance_radius_px: 1.0,
            enhance_amount: 1.0,
            ..Default::default()
        };
        assert!((a.apply(&flat(0.4)).get(2, 2) - 0.4).abs() < 1e-5);
        let edge = GrayF32::from_fn(10, 3, |x, _| if x < 5 { 0.3 } else { 0.7 });
        let out = a.apply(&edge);
        assert!(out.get(4, 1) < 0.3);
        assert!(out.get(5, 1) > 0.7);
    }

    /// Imagen blanca con un bloque negro de `n` x `n` px en (10, 10).
    fn speck(n: usize) -> GrayF32 {
        GrayF32::from_fn(30, 30, |x, y| {
            if (10..10 + n).contains(&x) && (10..10 + n).contains(&y) {
                0.0
            } else {
                1.0
            }
        })
    }

    #[test]
    fn denoise_removes_specks_up_to_the_size_and_keeps_bigger_blocks() {
        for size in 1..=4u32 {
            let a = ImageAdjust {
                denoise_px: size,
                ..Default::default()
            };
            let out = a.apply(&speck(size as usize));
            assert!(out.data.iter().all(|&v| v == 1.0), "mota de {size} px");
        }
        // Un bloque bastante mayor que el tamaño se conserva (el centro sigue negro).
        let a = ImageAdjust {
            denoise_px: 2,
            ..Default::default()
        };
        let out = a.apply(&speck(8));
        assert_eq!(out.get(13, 13), 0.0);
        assert_eq!(out.get(0, 0), 1.0);
    }

    #[test]
    fn denoise_keeps_straight_edges_and_flat_areas() {
        let edge = GrayF32::from_fn(20, 8, |x, _| if x < 10 { 0.2 } else { 0.8 });
        let a = ImageAdjust {
            denoise_px: 2,
            ..Default::default()
        };
        let out = a.apply(&edge);
        for y in 0..8 {
            for x in 0..20 {
                assert!((out.get(x, y) - edge.get(x, y)).abs() < 0.005, "({x},{y})");
            }
        }
    }

    #[test]
    fn median_matches_a_naive_implementation() {
        let img = GrayF32::from_fn(13, 9, |x, y| ((x * 7 + y * 13 + x * y) % 17) as f32 / 16.0);
        for radius in [1usize, 2, 3] {
            let fast = median_filter(&img, radius);
            for y in 0..9i64 {
                for x in 0..13i64 {
                    let mut win = Vec::new();
                    for dy in -(radius as i64)..=radius as i64 {
                        for dx in -(radius as i64)..=radius as i64 {
                            let sx = (x + dx).clamp(0, 12) as usize;
                            let sy = (y + dy).clamp(0, 8) as usize;
                            win.push((img.get(sx, sy) * 255.0 + 0.5) as u8);
                        }
                    }
                    win.sort_unstable();
                    let want = f32::from(win[win.len() / 2]) / 255.0;
                    assert_eq!(
                        fast.get(x as usize, y as usize),
                        want,
                        "r={radius} ({x},{y})"
                    );
                }
            }
        }
    }

    #[test]
    fn speck_radius_grows_with_size() {
        assert_eq!(median_radius_for_speck(1), 1);
        assert_eq!(median_radius_for_speck(2), 1);
        assert_eq!(median_radius_for_speck(3), 2);
        assert_eq!(median_radius_for_speck(4), 3);
    }

    #[test]
    fn validation() {
        assert!(ImageAdjust::default().validate().is_ok());
        for bad in [
            ImageAdjust {
                brightness: 2.0,
                ..Default::default()
            },
            ImageAdjust {
                contrast: -2.0,
                ..Default::default()
            },
            ImageAdjust {
                gamma: 0.0,
                ..Default::default()
            },
            ImageAdjust {
                gamma: f32::NAN,
                ..Default::default()
            },
            ImageAdjust {
                enhance_radius_px: -1.0,
                ..Default::default()
            },
            ImageAdjust {
                enhance_amount: -1.0,
                ..Default::default()
            },
            ImageAdjust {
                denoise_px: MAX_DENOISE_PX + 1,
                ..Default::default()
            },
        ] {
            assert!(bad.validate().is_err(), "{bad:?}");
        }
    }
}
