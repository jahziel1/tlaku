//! Preproceso: imagen a mapa binario (tinta / fondo), suavizado y limpieza de manchas.

use image::DynamicImage;

/// Mapa binario: `true` = tinta (lo que se traza).
pub(crate) struct Bitmap {
    pub w: usize,
    pub h: usize,
    pub ink: Vec<bool>,
}

/// Luminancia 0..255 de cada píxel, con el alfa compuesto sobre blanco.
fn luminance(image: &DynamicImage) -> (usize, usize, Vec<f32>) {
    let rgba = image.to_rgba8();
    let (w, h) = (rgba.width() as usize, rgba.height() as usize);
    let lum = rgba
        .pixels()
        .map(|p| {
            let [r, g, b, a] = p.0;
            let l = 0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b);
            let a = f32::from(a) / 255.0;
            l * a + 255.0 * (1.0 - a)
        })
        .collect();
    (w, h, lum)
}

/// Desenfoque gaussiano separable (bordes replicados), en sitio.
fn gaussian_blur(data: &mut [f32], w: usize, h: usize, sigma: f32) {
    let radius = (sigma * 3.0).ceil().max(1.0) as usize;
    let mut kernel: Vec<f32> = (0..=2 * radius)
        .map(|i| {
            let d = i as f32 - radius as f32;
            (-d * d / (2.0 * sigma * sigma)).exp()
        })
        .collect();
    let sum: f32 = kernel.iter().sum();
    kernel.iter_mut().for_each(|k| *k /= sum);

    let mut tmp = vec![0.0f32; data.len()];
    // Horizontal: data -> tmp.
    for y in 0..h {
        let row = &data[y * w..(y + 1) * w];
        for x in 0..w {
            let mut acc = 0.0;
            for (i, k) in kernel.iter().enumerate() {
                let sx = (x + i).saturating_sub(radius).min(w - 1);
                acc += k * row[sx];
            }
            tmp[y * w + x] = acc;
        }
    }
    // Vertical: tmp -> data.
    for y in 0..h {
        for x in 0..w {
            let mut acc = 0.0;
            for (i, k) in kernel.iter().enumerate() {
                let sy = (y + i).saturating_sub(radius).min(h - 1);
                acc += k * tmp[sy * w + x];
            }
            data[y * w + x] = acc;
        }
    }
}

/// Binariza: es tinta lo oscuro (`lum < threshold * 255`), o lo claro si `invert`.
pub(crate) fn binarize(image: &DynamicImage, threshold: f64, invert: bool, blur_px: f64) -> Bitmap {
    let (w, h, mut lum) = luminance(image);
    if blur_px > 0.0 {
        gaussian_blur(&mut lum, w, h, blur_px as f32);
    }
    let limit = (threshold * 255.0) as f32;
    let ink = lum.iter().map(|&l| (l < limit) != invert).collect();
    Bitmap { w, h, ink }
}

impl Bitmap {
    /// Elimina las manchas de tinta con menos de `min_area` píxeles (conectividad 4) y rellena
    /// los huecos interiores de menos de `min_area` (conectividad 8, sin tocar el borde).
    /// Devuelve cuántas regiones se descartaron.
    pub fn remove_specks(&mut self, min_area: usize) -> usize {
        if min_area <= 1 {
            return 0;
        }
        self.flip_small(true, min_area) + self.flip_small(false, min_area)
    }

    fn flip_small(&mut self, ink_value: bool, min_area: usize) -> usize {
        let (w, h) = (self.w, self.h);
        let mut visited = vec![false; w * h];
        let mut stack = Vec::new();
        let mut comp = Vec::new();
        let mut flipped = 0;
        for start in 0..w * h {
            if visited[start] || self.ink[start] != ink_value {
                continue;
            }
            comp.clear();
            stack.push(start);
            visited[start] = true;
            let mut touches_border = false;
            while let Some(i) = stack.pop() {
                comp.push(i);
                let (x, y) = (i % w, i / w);
                if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
                    touches_border = true;
                }
                // Tinta: vecinos en cruz; fondo: también diagonales.
                for dy in -1i64..=1 {
                    for dx in -1i64..=1 {
                        if (dx == 0 && dy == 0) || (ink_value && dx != 0 && dy != 0) {
                            continue;
                        }
                        let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                        if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                            continue;
                        }
                        let n = ny as usize * w + nx as usize;
                        if !visited[n] && self.ink[n] == ink_value {
                            visited[n] = true;
                            stack.push(n);
                        }
                    }
                }
            }
            // Un fondo que toca el borde es el fondo real, no un hueco.
            if comp.len() < min_area && (ink_value || !touches_border) {
                for &i in &comp {
                    self.ink[i] = !ink_value;
                }
                flipped += 1;
            }
        }
        flipped
    }

    /// Etiqueta componentes de tinta (conectividad 4) y devuelve (etiquetas, cantidad).
    pub fn label_components(&self) -> (Vec<u32>, usize) {
        let bm = self;
        let (w, h) = (bm.w, bm.h);
        let mut labels = vec![u32::MAX; w * h];
        let mut stack = Vec::new();
        let mut count = 0u32;
        for start in 0..w * h {
            if !bm.ink[start] || labels[start] != u32::MAX {
                continue;
            }
            labels[start] = count;
            stack.push(start);
            while let Some(i) = stack.pop() {
                let (x, y) = (i % w, i / w);
                let mut visit = |n: usize| {
                    if bm.ink[n] && labels[n] == u32::MAX {
                        labels[n] = count;
                        stack.push(n);
                    }
                };
                if x > 0 {
                    visit(i - 1);
                }
                if x + 1 < w {
                    visit(i + 1);
                }
                if y > 0 {
                    visit(i - w);
                }
                if y + 1 < h {
                    visit(i + w);
                }
            }
            count += 1;
        }
        (labels, count as usize)
    }

    pub fn is_blank(&self) -> bool {
        !self.ink.iter().any(|&b| b)
    }
}
