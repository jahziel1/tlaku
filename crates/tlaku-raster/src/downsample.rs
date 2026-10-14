//! Reducción por promedio de área que recibe la imagen por trozos (filas, franjas o teselas).
//!
//! Sirve para no tener la imagen completa en memoria: el cargador decodifica un trozo, lo
//! pasa a gris y lo acumula aquí; la memoria que cuesta es la de la rejilla de destino. Cada
//! eje se reduce igual que [`crate::resample::resample`] cuando el destino es menor que el
//! origen, y se deja tal cual (peso 1) cuando no lo es: **nunca amplía**.

use crate::gray::GrayF32;
use tlaku_i18n::tr;

/// Pesos de un píxel origen: cae en la celda destino `d0` con `w0` y, si cruza el borde, en
/// `d0 + 1` con `w1` (cero si no lo cruza).
#[derive(Debug, Clone, Copy)]
struct Tap {
    d0: u32,
    w0: f32,
    w1: f32,
}

fn axis_taps(src: usize, dst: usize) -> Vec<Tap> {
    debug_assert!(dst >= 1 && dst <= src);
    let scale = src as f64 / dst as f64;
    (0..src)
        .map(|k| {
            let (a, b) = (k as f64, (k + 1) as f64);
            let i0 = ((a / scale).floor() as usize).min(dst - 1);
            let edge = (i0 + 1) as f64 * scale;
            if i0 == dst - 1 || b <= edge + 1e-9 {
                Tap {
                    d0: i0 as u32,
                    w0: (1.0 / scale) as f32,
                    w1: 0.0,
                }
            } else {
                Tap {
                    d0: i0 as u32,
                    w0: ((edge - a) / scale) as f32,
                    w1: ((b - edge) / scale) as f32,
                }
            }
        })
        .collect()
}

/// Acumulador de la imagen reducida.
pub struct Downsampler {
    src_w: usize,
    src_h: usize,
    dst_w: usize,
    dst_h: usize,
    tx: Vec<Tap>,
    ty: Vec<Tap>,
    acc: Vec<f32>,
    /// Fila destino en construcción (con una celda de más para el borde).
    scratch: Vec<f32>,
}

impl Downsampler {
    /// Reduce de `src_w` x `src_h` a lo sumo `max_w` x `max_h` (un eje que ya cabe no cambia).
    pub fn new(src_w: usize, src_h: usize, max_w: usize, max_h: usize) -> Result<Self, String> {
        if src_w == 0 || src_h == 0 {
            return Err(tr!("raster.downsample.zero_size"));
        }
        let dst_w = src_w.min(max_w.max(1));
        let dst_h = src_h.min(max_h.max(1));
        let cells = dst_w
            .checked_mul(dst_h)
            .filter(|&c| c <= isize::MAX as usize / 8)
            .ok_or_else(|| tr!("raster.downsample.too_big"))?;
        Ok(Self {
            src_w,
            src_h,
            dst_w,
            dst_h,
            tx: axis_taps(src_w, dst_w),
            ty: axis_taps(src_h, dst_h),
            acc: vec![0.0; cells],
            scratch: vec![0.0; dst_w + 1],
        })
    }

    /// Tamaño del resultado.
    pub fn size(&self) -> (usize, usize) {
        (self.dst_w, self.dst_h)
    }

    /// Suma el tramo de la fila `y` que empieza en la columna `x0`.
    pub fn add(&mut self, x0: usize, y: usize, gray: &[f32]) {
        if gray.is_empty() || y >= self.src_h || x0 >= self.src_w {
            return;
        }
        let gray = &gray[..gray.len().min(self.src_w - x0)];
        let taps = &self.tx[x0..x0 + gray.len()];
        let lo = taps[0].d0 as usize;
        let hi = (taps[taps.len() - 1].d0 as usize + 1).min(self.dst_w);
        self.scratch[lo..=hi].fill(0.0);
        for (&v, t) in gray.iter().zip(taps) {
            self.scratch[t.d0 as usize] += v * t.w0;
            self.scratch[t.d0 as usize + 1] += v * t.w1;
        }
        let ty = self.ty[y];
        let rows = [(ty.d0 as usize, ty.w0), (ty.d0 as usize + 1, ty.w1)];
        for (dy, wy) in rows {
            if wy == 0.0 || dy >= self.dst_h {
                continue;
            }
            let row = &mut self.acc[dy * self.dst_w..(dy + 1) * self.dst_w];
            for (r, s) in row[lo..hi.min(self.dst_w - 1) + 1]
                .iter_mut()
                .zip(&self.scratch[lo..])
            {
                *r += s * wy;
            }
        }
    }

    pub fn finish(self) -> GrayF32 {
        GrayF32 {
            width: self.dst_w,
            height: self.dst_h,
            data: self.acc,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resample::resample;

    fn noise(w: usize, h: usize) -> GrayF32 {
        GrayF32::from_fn(w, h, |x, y| ((x * 31 + y * 17 + x * y) % 23) as f32 / 22.0)
    }

    fn run(img: &GrayF32, dw: usize, dh: usize, chunk: usize) -> GrayF32 {
        let mut ds = Downsampler::new(img.width, img.height, dw, dh).unwrap();
        for y in 0..img.height {
            let row = &img.data[y * img.width..(y + 1) * img.width];
            // En trozos de `chunk` columnas, como llegan las teselas.
            for (i, part) in row.chunks(chunk).enumerate() {
                ds.add(i * chunk, y, part);
            }
        }
        ds.finish()
    }

    #[test]
    fn matches_resample_when_reducing() {
        for (sw, sh, dw, dh) in [(40, 30, 10, 7), (37, 41, 13, 29), (17, 9, 17, 3)] {
            let img = noise(sw, sh);
            let want = resample(&img, dw, dh);
            for chunk in [1000, 8, 3] {
                let got = run(&img, dw, dh, chunk);
                assert_eq!((got.width, got.height), (dw, dh));
                for (a, b) in got.data.iter().zip(&want.data) {
                    assert!((a - b).abs() < 1e-5, "{sw}x{sh}->{dw}x{dh}: {a} {b}");
                }
            }
        }
    }

    #[test]
    fn never_enlarges() {
        let img = noise(10, 6);
        let got = run(&img, 50, 3, 4);
        assert_eq!((got.width, got.height), (10, 3));
        let same = run(&img, 10, 6, 10);
        assert_eq!(same, img);
    }

    #[test]
    fn rejects_empty() {
        assert!(Downsampler::new(0, 4, 2, 2).is_err());
    }
}
