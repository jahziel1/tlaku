//! Máscara de recorte: apaga los puntos de un bitmap que quedan fuera de unas formas.

use crate::bitmap::RasterBitmap;
use tlaku_core::{Point, Polyline};

/// Pone a 0 los puntos del bitmap cuyo centro queda fuera de `shapes`.
///
/// `origin` es la esquina inferior izquierda del bitmap en el mismo sistema que las formas
/// (mm, Y hacia arriba). Se usa la regla par-impar sobre todas las formas juntas: una forma
/// dentro de otra deja un hueco. Los trayectos abiertos se cierran por la cuerda. Sin formas
/// no se toca nada (la máscara vacía significa la imagen completa).
pub fn apply_mask(bitmap: &mut RasterBitmap, origin: Point, shapes: &[Polyline]) {
    let shapes: Vec<&Polyline> = shapes.iter().filter(|s| s.points.len() >= 3).collect();
    if shapes.is_empty() {
        return;
    }
    let (w, h) = (bitmap.width, bitmap.height);
    let mut xs: Vec<f64> = Vec::new();
    for row in 0..h {
        // El centro de la fila: la fila 0 es la de arriba.
        let y =
            origin.y + (h - 1 - row) as f64 * bitmap.pixel_size_y_mm + bitmap.pixel_size_y_mm / 2.0;
        xs.clear();
        for s in &shapes {
            let n = s.points.len();
            for i in 0..n {
                let (a, b) = (s.points[i], s.points[(i + 1) % n]);
                // Semiabierto en Y para no contar dos veces un vértice.
                if (a.y <= y) != (b.y <= y) {
                    xs.push(a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x));
                }
            }
        }
        xs.sort_by(f64::total_cmp);
        let line = &mut bitmap.data[row * w..(row + 1) * w];
        for (col, v) in line.iter_mut().enumerate() {
            let x = origin.x + (col as f64 + 0.5) * bitmap.pixel_size_x_mm;
            // Dentro si el número de cruces a la izquierda es impar.
            let crossed = xs.partition_point(|&c| c <= x);
            if crossed % 2 == 0 {
                *v = 0.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Polyline {
        Polyline {
            points: vec![
                Point::new(x0, y0),
                Point::new(x1, y0),
                Point::new(x1, y1),
                Point::new(x0, y1),
            ],
            closed: true,
        }
    }

    /// Bitmap de 10x10 puntos de 1 mm, todo encendido, con origen en (5, 5).
    fn full() -> RasterBitmap {
        RasterBitmap::new(10, 10, 1.0, 1.0, vec![1.0; 100]).unwrap()
    }

    #[test]
    fn sin_formas_no_toca_nada() {
        let mut b = full();
        apply_mask(&mut b, Point::new(5.0, 5.0), &[]);
        assert_eq!(b.lit_fraction(), 1.0);
    }

    #[test]
    fn deja_solo_lo_de_dentro() {
        let mut b = full();
        // 4x4 mm dentro del bitmap (que ocupa x e y de 5 a 15).
        apply_mask(
            &mut b,
            Point::new(5.0, 5.0),
            &[square(7.0, 8.0, 11.0, 12.0)],
        );
        assert_eq!(b.data.iter().filter(|&&v| v > 0.0).count(), 16);
        // Fila 0 = arriba (y 14..15): apagada; la fila de y 11..12 (fila 3) tiene x 7..11 encendido.
        assert_eq!(b.get(0, 0), 0.0);
        assert_eq!(b.get(2, 3), 1.0);
        assert_eq!(b.get(1, 3), 0.0);
        assert_eq!(b.get(6, 3), 0.0);
    }

    #[test]
    fn una_forma_dentro_de_otra_deja_hueco() {
        let mut b = full();
        apply_mask(
            &mut b,
            Point::new(5.0, 5.0),
            &[square(5.0, 5.0, 15.0, 15.0), square(8.0, 8.0, 12.0, 12.0)],
        );
        assert_eq!(b.data.iter().filter(|&&v| v > 0.0).count(), 100 - 16);
    }

    #[test]
    fn una_forma_fuera_apaga_todo() {
        let mut b = full();
        apply_mask(
            &mut b,
            Point::new(5.0, 5.0),
            &[square(100.0, 100.0, 110.0, 110.0)],
        );
        assert_eq!(b.lit_fraction(), 0.0);
    }
}
