//! Ajustes de imagen previos al remuestreo: brillo, contraste, gamma,
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
}

impl Default for ImageAdjust {
    fn default() -> Self {
        Self {
            brightness: 0.0,
            contrast: 0.0,
            gamma: 1.0,
            invert: false,
            enhance_radius_px: 0.0,
            enhance_amount: 0.0,
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
        Ok(())
    }

    /// Aplica en orden: contraste y brillo, gamma, enfoque e inversión.
    pub fn apply(&self, img: &GrayF32) -> GrayF32 {
        let factor = if self.contrast >= 0.0 {
            1.0 / (1.0 - self.contrast).max(0.01)
        } else {
            1.0 + self.contrast
        };
        let mut out = img.clone();
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

/// Desenfoque gaussiano separable; los bordes repiten el píxel extremo.
fn gaussian_blur(img: &GrayF32, sigma: f32) -> GrayF32 {
    let radius = (sigma * 3.0).ceil() as i32;
    let mut kernel: Vec<f32> = (-radius..=radius)
        .map(|i| (-((i * i) as f32) / (2.0 * sigma * sigma)).exp())
        .collect();
    let sum: f32 = kernel.iter().sum();
    kernel.iter_mut().for_each(|k| *k /= sum);

    let (w, h) = (img.width, img.height);
    let mut tmp = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            tmp[y * w + x] = kernel
                .iter()
                .enumerate()
                .map(|(k, &wt)| {
                    let sx = (x as i32 + k as i32 - radius).clamp(0, w as i32 - 1) as usize;
                    wt * img.data[y * w + sx]
                })
                .sum();
        }
    }
    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            out[y * w + x] = kernel
                .iter()
                .enumerate()
                .map(|(k, &wt)| {
                    let sy = (y as i32 + k as i32 - radius).clamp(0, h as i32 - 1) as usize;
                    wt * tmp[sy * w + x]
                })
                .sum();
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
        ] {
            assert!(bad.validate().is_err(), "{bad:?}");
        }
    }
}
