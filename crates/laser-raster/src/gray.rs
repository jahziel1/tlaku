//! Imagen en escala de grises `f32` y carga desde archivos.

use image::DynamicImage;
use std::path::Path;

/// Imagen de un canal con valores 0..1 (0 = negro, 1 = blanco), fila 0 arriba.
#[derive(Debug, Clone, PartialEq)]
pub struct GrayF32 {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>,
}

impl GrayF32 {
    pub fn new(width: usize, height: usize, data: Vec<f32>) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err("la imagen no puede tener tamaño cero".into());
        }
        if data.len() != width * height {
            return Err(format!(
                "se esperaban {} píxeles y llegaron {}",
                width * height,
                data.len()
            ));
        }
        Ok(Self {
            width,
            height,
            data,
        })
    }

    /// Imagen cuyo píxel `(x, y)` vale `f(x, y)`.
    pub fn from_fn(width: usize, height: usize, f: impl Fn(usize, usize) -> f32) -> Self {
        let data = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .map(|(x, y)| f(x, y))
            .collect();
        Self::new(width, height, data).expect("tamaño válido")
    }

    pub fn get(&self, x: usize, y: usize) -> f32 {
        self.data[y * self.width + x]
    }

    pub fn mean(&self) -> f64 {
        self.data.iter().map(|&v| f64::from(v)).sum::<f64>() / self.data.len() as f64
    }
}

/// Pesos para convertir RGB a luminancia. Se aplican sobre los valores sRGB tal
/// como vienen en el archivo (sin linealizar), igual que la mayoría de editores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Luma {
    /// ITU-R BT.709: 0.2126 R + 0.7152 G + 0.0722 B.
    #[default]
    Rec709,
    /// ITU-R BT.601: 0.299 R + 0.587 G + 0.114 B.
    Rec601,
}

impl Luma {
    fn weights(self) -> [f32; 3] {
        match self {
            Luma::Rec709 => [0.2126, 0.7152, 0.0722],
            Luma::Rec601 => [0.299, 0.587, 0.114],
        }
    }
}

/// Convierte cualquier imagen a gris. El canal alfa se compone sobre blanco
/// (un píxel transparente queda blanco, es decir, sin grabar).
pub fn from_dynamic(img: &DynamicImage, luma: Luma) -> GrayF32 {
    let rgba = img.to_rgba32f();
    let [wr, wg, wb] = luma.weights();
    let data = rgba
        .pixels()
        .map(|p| {
            let [r, g, b, a] = p.0;
            let y = wr * r + wg * g + wb * b;
            (y * a + (1.0 - a)).clamp(0.0, 1.0)
        })
        .collect();
    GrayF32 {
        width: rgba.width() as usize,
        height: rgba.height() as usize,
        data,
    }
}

/// Decodifica PNG, JPEG o BMP desde memoria.
pub fn decode(bytes: &[u8], luma: Luma) -> Result<GrayF32, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("imagen inválida: {e}"))?;
    Ok(from_dynamic(&img, luma))
}

pub fn load(path: &Path, luma: Luma) -> Result<GrayF32, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("no se pudo leer {}: {e}", path.display()))?;
    decode(&bytes, luma)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn rejects_bad_sizes() {
        assert!(GrayF32::new(0, 3, vec![]).is_err());
        assert!(GrayF32::new(2, 2, vec![0.0; 3]).is_err());
    }

    #[test]
    fn alpha_composites_over_white() {
        let mut img = RgbaImage::new(3, 1);
        img.put_pixel(0, 0, Rgba([0, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([0, 0, 0, 0]));
        img.put_pixel(2, 0, Rgba([0, 0, 0, 128]));
        let g = from_dynamic(&DynamicImage::ImageRgba8(img), Luma::Rec709);
        assert_eq!(g.get(0, 0), 0.0);
        assert_eq!(g.get(1, 0), 1.0);
        assert!((g.get(2, 0) - 0.498).abs() < 0.01);
    }

    #[test]
    fn luma_weights_differ() {
        let mut img = RgbaImage::new(1, 1);
        img.put_pixel(0, 0, Rgba([0, 255, 0, 255]));
        let img = DynamicImage::ImageRgba8(img);
        assert!((from_dynamic(&img, Luma::Rec709).get(0, 0) - 0.7152).abs() < 1e-4);
        assert!((from_dynamic(&img, Luma::Rec601).get(0, 0) - 0.587).abs() < 1e-4);
    }

    #[test]
    fn decodes_png_roundtrip() {
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([255, 255, 255, 255]));
        img.put_pixel(1, 0, Rgba([0, 0, 0, 255]));
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(img)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        let g = decode(&bytes, Luma::default()).unwrap();
        assert_eq!((g.width, g.height), (2, 1));
        assert_eq!(g.data, vec![1.0, 0.0]);
        assert!(decode(b"no es una imagen", Luma::default()).is_err());
    }
}
