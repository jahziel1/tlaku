//! Utilidades de imagen en coma flotante: desenfoque gaussiano y muestreo bilineal.

use image::GrayImage;

/// Imagen de un canal en `f32`, escala 0..255.
pub(crate) struct FloatImage {
    pub w: usize,
    pub h: usize,
    pub data: Vec<f32>,
}

impl FloatImage {
    pub fn from_gray(img: &GrayImage) -> Self {
        Self {
            w: img.width() as usize,
            h: img.height() as usize,
            data: img.as_raw().iter().map(|&v| f32::from(v)).collect(),
        }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> f32 {
        self.data[y * self.w + x]
    }

    /// Muestreo bilineal con los bordes replicados.
    pub fn sample(&self, x: f64, y: f64) -> f32 {
        let x = x.clamp(0.0, (self.w - 1) as f64);
        let y = y.clamp(0.0, (self.h - 1) as f64);
        let x0 = (x.floor() as usize).min(self.w.saturating_sub(2));
        let y0 = (y.floor() as usize).min(self.h.saturating_sub(2));
        let fx = (x - x0 as f64) as f32;
        let fy = (y - y0 as f64) as f32;
        let x1 = (x0 + 1).min(self.w - 1);
        let y1 = (y0 + 1).min(self.h - 1);
        let a = self.get(x0, y0) * (1.0 - fx) + self.get(x1, y0) * fx;
        let b = self.get(x0, y1) * (1.0 - fx) + self.get(x1, y1) * fx;
        a * (1.0 - fy) + b * fy
    }
}

/// Desenfoque gaussiano separable con bordes replicados.
pub(crate) fn gaussian_blur(src: &FloatImage, sigma: f32) -> FloatImage {
    let radius = (3.0 * sigma).ceil().max(1.0) as usize;
    let mut kernel: Vec<f32> = (0..=2 * radius)
        .map(|i| {
            let d = i as f32 - radius as f32;
            (-d * d / (2.0 * sigma * sigma)).exp()
        })
        .collect();
    let sum: f32 = kernel.iter().sum();
    kernel.iter_mut().for_each(|k| *k /= sum);

    let (w, h) = (src.w, src.h);
    // Pasada horizontal usando una fila con relleno replicado.
    let mut tmp = vec![0.0f32; w * h];
    let mut padded = vec![0.0f32; w + 2 * radius];
    for y in 0..h {
        let row = &src.data[y * w..(y + 1) * w];
        padded[..radius].fill(row[0]);
        padded[radius..radius + w].copy_from_slice(row);
        padded[radius + w..].fill(row[w - 1]);
        let out = &mut tmp[y * w..(y + 1) * w];
        for (x, o) in out.iter_mut().enumerate() {
            let win = &padded[x..x + 2 * radius + 1];
            *o = win.iter().zip(&kernel).map(|(a, b)| a * b).sum();
        }
    }
    // Pasada vertical acumulando filas completas.
    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        let dst = &mut out[y * w..(y + 1) * w];
        for (k, &kv) in kernel.iter().enumerate() {
            let sy = (y + k).saturating_sub(radius).min(h - 1);
            let srow = &tmp[sy * w..(sy + 1) * w];
            for (d, s) in dst.iter_mut().zip(srow) {
                *d += kv * s;
            }
        }
    }
    FloatImage { w, h, data: out }
}
