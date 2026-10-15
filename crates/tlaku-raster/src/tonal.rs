//! Mapa de energía para la vista previa «por tonos»: acumula, sobre una rejilla de píxeles de
//! tamaño fijo, la energía que el láser deposita (potencia en fracción x segundos) y la devuelve
//! como imagen. No sabe nada de capas ni de materiales: la dosis la convierte en color quien lo
//! pide (`to_rgba`), y quien llena el mapa (`tlaku-pipeline::tonal`) decide qué energía lleva
//! cada trazo.
//!
//! Coordenadas en mm con Y hacia arriba, como el resto del motor; la fila 0 de la imagen de
//! salida es la de arriba. Cada segmento reparte su energía entre los píxeles que cruza,
//! proporcional al largo dentro de cada uno (recorrido exacto de la rejilla), así que la suma se
//! conserva y el costo depende de los píxeles cruzados, no del largo en mm.

use tlaku_i18n::tr;

/// Tope de píxeles de un mapa (16 M: ~64 MB de energía en `f32`).
pub const MAX_CELLS: usize = 16_000_000;

/// Rejilla de energía.
#[derive(Debug, Clone)]
pub struct DoseMap {
    /// Esquina inferior izquierda de la región (mm).
    x0: f64,
    y0: f64,
    /// Lado de un píxel (mm).
    px: f64,
    width: usize,
    height: usize,
    /// Energía por píxel, fila 0 = la de abajo (Y menor).
    energy: Vec<f32>,
}

impl DoseMap {
    /// Mapa que cubre `region_mm` (`[x0, y0, x1, y1]`) con píxeles de `px_mm`; el último píxel de
    /// cada fila o columna puede pasarse un poco del borde. Falla si la región o el píxel no son
    /// válidos o si el mapa pasa de [`MAX_CELLS`].
    pub fn new(region_mm: [f64; 4], px_mm: f64) -> Result<Self, String> {
        let [x0, y0, x1, y1] = region_mm;
        let ok = [x0, y0, x1, y1, px_mm].iter().all(|v| v.is_finite())
            && x1 > x0
            && y1 > y0
            && px_mm > 0.0;
        if !ok {
            return Err(tr!("raster.tonal.bad_region"));
        }
        let w = ((x1 - x0) / px_mm).ceil().max(1.0);
        let h = ((y1 - y0) / px_mm).ceil().max(1.0);
        if w * h > MAX_CELLS as f64 {
            return Err(tr!("raster.tonal.too_big", w = w, h = h));
        }
        let (width, height) = (w as usize, h as usize);
        Ok(Self {
            x0,
            y0,
            px: px_mm,
            width,
            height,
            energy: vec![0.0; width * height],
        })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn px_mm(&self) -> f64 {
        self.px
    }

    /// Región que cubre exactamente la rejilla `[x0, y0, x1, y1]` (mm).
    pub fn covered_mm(&self) -> [f64; 4] {
        [
            self.x0,
            self.y0,
            self.x0 + self.width as f64 * self.px,
            self.y0 + self.height as f64 * self.px,
        ]
    }

    /// Energía total acumulada (suma de todos los píxeles).
    pub fn total_energy(&self) -> f64 {
        self.energy.iter().map(|&e| f64::from(e)).sum()
    }

    /// Dosis (energía / mm2) del píxel de columna `ix` y fila `iy` (0 = abajo).
    pub fn dose_at(&self, ix: usize, iy: usize) -> f64 {
        f64::from(self.energy[iy * self.width + ix]) / (self.px * self.px)
    }

    /// Dosis máxima de la rejilla.
    pub fn max_dose(&self) -> f64 {
        self.energy.iter().fold(0.0f32, |m, &e| m.max(e)) as f64 / (self.px * self.px)
    }

    /// Deposita `energy` (potencia en fracción x segundos) en el punto `(x, y)` mm.
    pub fn add_point(&mut self, x: f64, y: f64, energy: f64) {
        if !(energy.is_finite() && energy > 0.0 && x.is_finite() && y.is_finite()) {
            return;
        }
        let u = (x - self.x0) / self.px;
        let v = (y - self.y0) / self.px;
        if u < 0.0 || v < 0.0 || u >= self.width as f64 || v >= self.height as f64 {
            return;
        }
        self.energy[v as usize * self.width + u as usize] += energy as f32;
    }

    /// Deposita `energy_per_mm` (fracción de potencia / velocidad en mm/s) a lo largo del
    /// segmento de `a` a `b` (mm), repartido por el largo dentro de cada píxel. Lo que queda
    /// fuera de la región se descarta.
    pub fn add_segment(&mut self, a: (f64, f64), b: (f64, f64), energy_per_mm: f64) {
        if !(energy_per_mm.is_finite() && energy_per_mm > 0.0) {
            return;
        }
        let (w, h) = (self.width as f64, self.height as f64);
        let (u0, v0) = ((a.0 - self.x0) / self.px, (a.1 - self.y0) / self.px);
        let (u1, v1) = ((b.0 - self.x0) / self.px, (b.1 - self.y0) / self.px);
        if !(u0.is_finite() && v0.is_finite() && u1.is_finite() && v1.is_finite()) {
            return;
        }
        let (du, dv) = (u1 - u0, v1 - v0);
        // Recorte a la rejilla (Liang-Barsky) en el parámetro t de 0 a 1.
        let (mut t0, mut t1) = (0.0f64, 1.0f64);
        for (p, q) in [(-du, u0), (du, w - u0), (-dv, v0), (dv, h - v0)] {
            if p == 0.0 {
                if q < 0.0 {
                    return;
                }
            } else {
                let r = q / p;
                if p < 0.0 {
                    t0 = t0.max(r);
                } else {
                    t1 = t1.min(r);
                }
            }
            if t0 >= t1 {
                return;
            }
        }
        let len_px = (du * du + dv * dv).sqrt() * (t1 - t0);
        if len_px <= 0.0 {
            return;
        }
        // Energía por unidad de parámetro s (0..1 sobre el trozo recortado).
        let per_s = energy_per_mm * len_px * self.px;
        let (su, sv) = (u0 + du * t0, v0 + dv * t0);
        let (cu, cv) = ((du * (t1 - t0)), (dv * (t1 - t0)));
        let mut ix = (su.floor().max(0.0) as usize).min(self.width - 1);
        let mut iy = (sv.floor().max(0.0) as usize).min(self.height - 1);
        let step_x: isize = if cu > 0.0 { 1 } else { -1 };
        let step_y: isize = if cv > 0.0 { 1 } else { -1 };
        let boundary = |i: usize, step: isize| if step > 0 { (i + 1) as f64 } else { i as f64 };
        let mut s_max_x = if cu == 0.0 {
            f64::INFINITY
        } else {
            (boundary(ix, step_x) - su) / cu
        };
        let mut s_max_y = if cv == 0.0 {
            f64::INFINITY
        } else {
            (boundary(iy, step_y) - sv) / cv
        };
        let ds_x = if cu == 0.0 {
            f64::INFINITY
        } else {
            (1.0 / cu).abs()
        };
        let ds_y = if cv == 0.0 {
            f64::INFINITY
        } else {
            (1.0 / cv).abs()
        };
        let mut s = 0.0f64;
        // Cada vuelta avanza a la siguiente frontera; el tope protege de errores de redondeo.
        for _ in 0..(self.width + self.height + 4) {
            let next = s_max_x.min(s_max_y).min(1.0);
            let ds = (next - s).max(0.0);
            self.energy[iy * self.width + ix] += (ds * per_s) as f32;
            if next >= 1.0 {
                break;
            }
            s = next;
            if s_max_x <= s_max_y {
                let nx = ix as isize + step_x;
                if nx < 0 || nx >= self.width as isize {
                    break;
                }
                ix = nx as usize;
                s_max_x += ds_x;
            } else {
                let ny = iy as isize + step_y;
                if ny < 0 || ny >= self.height as isize {
                    break;
                }
                iy = ny as usize;
                s_max_y += ds_y;
            }
        }
    }

    /// Reparte la energía en un ancho de `width_mm` (el ancho del punto del láser) con un
    /// promedio de caja separable; no hace nada si el punto cabe en un píxel. La energía se
    /// conserva salvo lo que sale por el borde de la región.
    pub fn spread(&mut self, width_mm: f64) {
        if !(width_mm.is_finite() && width_mm > 0.0) {
            return;
        }
        let k = (width_mm / self.px).round();
        if k < 2.0 {
            return;
        }
        let radius = (k as usize) / 2;
        let (w, h) = (self.width, self.height);
        // Horizontal.
        let mut line = vec![0.0f64; w.max(h)];
        let mut prefix = vec![0.0f64; w.max(h) + 1];
        for row in 0..h {
            let slice = &mut self.energy[row * w..(row + 1) * w];
            box_filter(
                slice.iter().map(|&e| f64::from(e)),
                w,
                radius,
                &mut prefix,
                &mut line,
            );
            for (dst, &src) in slice.iter_mut().zip(&line[..w]) {
                *dst = src as f32;
            }
        }
        // Vertical.
        for col in 0..w {
            box_filter(
                (0..h).map(|r| f64::from(self.energy[r * w + col])),
                h,
                radius,
                &mut prefix,
                &mut line,
            );
            for (r, &src) in line[..h].iter().enumerate() {
                self.energy[r * w + col] = src as f32;
            }
        }
    }

    /// Imagen RGBA (4 bytes por píxel, fila 0 = arriba): `color` recibe la dosis de cada píxel.
    pub fn to_rgba(&self, color: impl Fn(f64) -> [u8; 4]) -> Vec<u8> {
        let inv_area = 1.0 / (self.px * self.px);
        let mut out = Vec::with_capacity(self.width * self.height * 4);
        for iy in (0..self.height).rev() {
            for &e in &self.energy[iy * self.width..(iy + 1) * self.width] {
                out.extend_from_slice(&color(f64::from(e) * inv_area));
            }
        }
        out
    }
}

/// Promedio de caja de ancho `2 * radius + 1` con ceros fuera de los bordes: `out[i]` es la suma
/// de `src[i - radius..=i + radius]` dividida por el ancho de la ventana.
fn box_filter(
    src: impl Iterator<Item = f64>,
    n: usize,
    radius: usize,
    prefix: &mut [f64],
    out: &mut [f64],
) {
    prefix[0] = 0.0;
    for (i, v) in src.take(n).enumerate() {
        prefix[i + 1] = prefix[i] + v;
    }
    let inv = 1.0 / (2 * radius + 1) as f64;
    for (i, o) in out.iter_mut().take(n).enumerate() {
        let lo = i.saturating_sub(radius);
        let hi = (i + radius + 1).min(n);
        *o = (prefix[hi] - prefix[lo]) * inv;
    }
}

/// PNG de una imagen RGBA de `width` x `height` (compresión rápida: la vista previa se pide
/// muchas veces).
pub fn rgba_to_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    use image::ImageEncoder;
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    let mut out = Vec::new();
    PngEncoder::new_with_quality(&mut out, CompressionType::Fast, FilterType::Sub)
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|e| tr!("raster.bitmap.png_encode", error = e))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(w_mm: f64, h_mm: f64, px: f64) -> DoseMap {
        DoseMap::new([0.0, 0.0, w_mm, h_mm], px).unwrap()
    }

    #[test]
    fn segment_energy_is_conserved_and_split_by_length() {
        let mut m = map(10.0, 10.0, 1.0);
        // 4 mm horizontales desde la mitad de un píxel: 0,5 + 1 + 1 + 1 + 0,5 = 4 mm.
        m.add_segment((0.5, 2.5), (4.5, 2.5), 2.0);
        assert!((m.total_energy() - 8.0).abs() < 1e-4);
        assert!((m.dose_at(0, 2) - 1.0).abs() < 1e-5); // 0,5 mm x 2
        assert!((m.dose_at(2, 2) - 2.0).abs() < 1e-5);
        assert!((m.dose_at(4, 2) - 1.0).abs() < 1e-5);
        assert_eq!(m.dose_at(2, 3), 0.0);
    }

    #[test]
    fn diagonal_and_reversed_segments_conserve_energy() {
        let mut m = map(10.0, 10.0, 0.5);
        m.add_segment((1.0, 1.0), (8.0, 6.0), 1.0);
        let len = (49.0f64 + 25.0).sqrt();
        assert!((m.total_energy() - len).abs() < 1e-3);
        let mut r = map(10.0, 10.0, 0.5);
        r.add_segment((8.0, 6.0), (1.0, 1.0), 1.0);
        assert!((r.total_energy() - len).abs() < 1e-3);
        let mut v = map(10.0, 10.0, 0.5);
        v.add_segment((3.2, 9.0), (3.2, 1.0), 1.0); // vertical por el interior de un píxel
        assert!((v.total_energy() - 8.0).abs() < 1e-3);
    }

    #[test]
    fn segments_are_clipped_to_the_region() {
        let mut m = map(10.0, 10.0, 1.0);
        m.add_segment((-5.0, 5.5), (15.0, 5.5), 1.0);
        assert!((m.total_energy() - 10.0).abs() < 1e-4);
        m.add_segment((-5.0, -5.0), (-1.0, -1.0), 1.0);
        m.add_segment((20.0, 0.0), (30.0, 10.0), 1.0);
        assert!((m.total_energy() - 10.0).abs() < 1e-4);
        // Una arista exacta de la región tampoco rompe nada.
        m.add_segment((0.0, 0.0), (10.0, 10.0), 1.0);
        assert!((m.total_energy() - 10.0 - 200.0f64.sqrt()).abs() < 1e-3);
    }

    #[test]
    fn passes_accumulate_and_dwell_goes_to_one_pixel() {
        let mut m = map(4.0, 4.0, 1.0);
        for _ in 0..3 {
            m.add_segment((0.0, 1.5), (4.0, 1.5), 0.5);
        }
        assert!((m.dose_at(1, 1) - 1.5).abs() < 1e-5);
        m.add_point(2.5, 2.5, 0.25);
        assert!((m.dose_at(2, 2) - 0.25).abs() < 1e-6);
        m.add_point(-1.0, 2.0, 5.0);
        m.add_point(2.0, f64::NAN, 5.0);
        assert!((m.total_energy() - 6.25).abs() < 1e-4);
    }

    #[test]
    fn spread_keeps_energy_and_blurs() {
        let mut m = map(20.0, 20.0, 0.5);
        m.add_point(10.25, 10.25, 1.0);
        let before = m.max_dose();
        m.spread(2.0); // 4 píxeles -> ventana de 5
        assert!((m.total_energy() - 1.0).abs() < 1e-5);
        assert!(m.max_dose() < before / 10.0);
        let mut small = map(20.0, 20.0, 0.5);
        small.add_point(10.25, 10.25, 1.0);
        small.spread(0.3); // cabe en un píxel: no cambia
        assert_eq!(small.max_dose(), before);
    }

    #[test]
    fn rgba_has_row_zero_on_top() {
        let mut m = map(2.0, 2.0, 1.0);
        m.add_point(0.5, 1.5, 1.0); // arriba a la izquierda
        let img = m.to_rgba(|d| {
            if d > 0.0 {
                [255, 0, 0, 255]
            } else {
                [0, 0, 0, 255]
            }
        });
        assert_eq!(&img[0..4], &[255, 0, 0, 255]);
        assert_eq!(&img[4..8], &[0, 0, 0, 255]);
        let png = rgba_to_png(2, 2, &img).unwrap();
        assert_eq!(&png[1..4], b"PNG");
    }

    #[test]
    fn rejects_bad_regions_and_huge_maps() {
        assert!(DoseMap::new([0.0, 0.0, 0.0, 5.0], 1.0).is_err());
        assert!(DoseMap::new([0.0, 0.0, 5.0, 5.0], 0.0).is_err());
        assert!(DoseMap::new([0.0, 0.0, f64::NAN, 5.0], 1.0).is_err());
        assert!(DoseMap::new([0.0, 0.0, 1.0e5, 1.0e5], 1.0).is_err());
    }
}
