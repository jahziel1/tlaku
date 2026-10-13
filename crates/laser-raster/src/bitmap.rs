//! Resultado del procesado: fracción de potencia por punto.

/// Rejilla de puntos de grabado. La fila 0 es la de arriba de la imagen.
#[derive(Debug, Clone, PartialEq)]
pub struct RasterBitmap {
    pub width: usize,
    pub height: usize,
    /// Tamaño físico de un punto en X (mm).
    pub pixel_size_x_mm: f64,
    /// Tamaño físico de un punto en Y (mm), es decir, la separación entre líneas.
    pub pixel_size_y_mm: f64,
    /// Fracción de potencia 0..1 (0 = apagado), `width * height` valores por filas.
    pub data: Vec<f32>,
}

impl RasterBitmap {
    pub fn new(
        width: usize,
        height: usize,
        pixel_size_x_mm: f64,
        pixel_size_y_mm: f64,
        data: Vec<f32>,
    ) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err("el bitmap no puede tener tamaño cero".into());
        }
        if data.len() != width * height {
            return Err(format!(
                "se esperaban {} puntos y llegaron {}",
                width * height,
                data.len()
            ));
        }
        if !(pixel_size_x_mm > 0.0 && pixel_size_y_mm > 0.0) {
            return Err("el tamaño de punto debe ser positivo".into());
        }
        Ok(Self {
            width,
            height,
            pixel_size_x_mm,
            pixel_size_y_mm,
            data,
        })
    }

    pub fn get(&self, x: usize, y: usize) -> f32 {
        self.data[y * self.width + x]
    }

    pub fn row(&self, y: usize) -> &[f32] {
        &self.data[y * self.width..(y + 1) * self.width]
    }

    pub fn width_mm(&self) -> f64 {
        self.width as f64 * self.pixel_size_x_mm
    }

    pub fn height_mm(&self) -> f64 {
        self.height as f64 * self.pixel_size_y_mm
    }

    /// Proporción de puntos encendidos (potencia mayor que 0).
    pub fn lit_fraction(&self) -> f64 {
        self.data.iter().filter(|&&v| v > 0.0).count() as f64 / self.data.len() as f64
    }

    /// Reduce la potencia a `levels` niveles (2 = binario). Útil en Grayscale para que
    /// píxeles casi iguales se unan en un solo tramo y el G-code sea más corto.
    pub fn quantize(&mut self, levels: u32) {
        if levels < 2 {
            return;
        }
        let n = (levels - 1) as f32;
        for v in &mut self.data {
            *v = (*v * n).round() / n;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_checks() {
        assert!(RasterBitmap::new(2, 2, 0.1, 0.1, vec![0.0; 4]).is_ok());
        assert!(RasterBitmap::new(2, 2, 0.1, 0.1, vec![0.0; 3]).is_err());
        assert!(RasterBitmap::new(0, 2, 0.1, 0.1, vec![]).is_err());
        assert!(RasterBitmap::new(2, 2, 0.0, 0.1, vec![0.0; 4]).is_err());
    }

    #[test]
    fn sizes_and_quantize() {
        let mut b = RasterBitmap::new(2, 1, 0.5, 0.25, vec![0.3, 0.8]).unwrap();
        assert_eq!((b.width_mm(), b.height_mm()), (1.0, 0.25));
        assert_eq!(b.lit_fraction(), 1.0);
        b.quantize(2);
        assert_eq!(b.data, vec![0.0, 1.0]);
        assert_eq!(b.lit_fraction(), 0.5);
    }
}
