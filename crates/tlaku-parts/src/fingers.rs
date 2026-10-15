//! Reparto de los dientes de una arista.
//!
//! Una arista de largo `len` se divide en `n` celdas iguales, con `n` impar y al menos 3. Las
//! celdas pares (0, 2, ..., n-1) son de una pieza y las impares de la otra, así que las dos
//! piezas tienen la misma pieza en ambos extremos de la arista (la que "cierra" la esquina) y los
//! dientes quedan simétricos respecto al centro. El ancho de celda sale de repartir el largo
//! completo, no del ancho pedido: el ancho pedido solo elige cuántas celdas.

/// Celdas mínimas por arista: con una sola, las dos piezas no se entrelazan.
pub const MIN_CELLS: usize = 3;

/// Reparto de una arista que empieza en `start` y mide `len`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cells {
    pub start: f64,
    pub len: f64,
    pub n: usize,
}

impl Cells {
    /// Reparte `len` en celdas lo más cercanas posible a `target`, con número impar de celdas.
    pub fn plan(start: f64, len: f64, target: f64) -> Self {
        let ideal = (len / target).max(1.0);
        // Vecinos impares de lo ideal: el que deja la celda más cercana al ancho pedido.
        let below = {
            let f = ideal.floor() as usize;
            if f % 2 == 1 { f } else { f.saturating_sub(1) }
        }
        .max(MIN_CELLS);
        let above = below + 2;
        let err = |n: usize| (len / n as f64 - target).abs();
        let n = if err(above) < err(below) && ideal > below as f64 {
            above
        } else {
            below
        };
        Self { start, len, n }
    }

    /// Ancho de cada celda.
    pub fn width(&self) -> f64 {
        self.len / self.n as f64
    }

    /// Posición del límite `k` (de 0 a `n`).
    pub fn at(&self, k: usize) -> f64 {
        self.start + self.len * k as f64 / self.n as f64
    }

    /// Intervalo `[a, b]` de la celda `k`.
    pub fn cell(&self, k: usize) -> (f64, f64) {
        (self.at(k), self.at(k + 1))
    }

    /// Cuántas celdas tiene la pieza que posee las pares.
    pub fn even_count(&self) -> usize {
        self.n.div_ceil(2)
    }

    /// Cuántas tiene la que posee las impares.
    pub fn odd_count(&self) -> usize {
        self.n / 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_is_odd_and_at_least_three() {
        for len in [10.0, 20.0, 31.0, 59.9, 100.0, 194.0, 777.7] {
            for target in [3.0, 5.0, 8.0, 10.0, 25.0] {
                let c = Cells::plan(0.0, len, target);
                assert_eq!(c.n % 2, 1, "{len}/{target}");
                assert!(c.n >= MIN_CELLS);
                assert!((c.at(c.n) - len).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn width_stays_close_to_the_request() {
        let c = Cells::plan(0.0, 100.0, 10.0);
        // 100/10 = 10 -> 9 u 11 celdas; 11 da 9,09 y 9 da 11,1.
        assert_eq!(c.n, 11);
        assert!((c.width() - 100.0 / 11.0).abs() < 1e-12);
        let c = Cells::plan(0.0, 94.0, 10.0);
        assert_eq!(c.n, 9);
    }

    #[test]
    fn cells_are_symmetric_about_the_center() {
        let c = Cells::plan(3.0, 74.0, 7.0);
        let mid = 3.0 + 37.0;
        for k in 0..=c.n {
            assert!((c.at(k) + c.at(c.n - k) - 2.0 * mid).abs() < 1e-9);
        }
    }

    #[test]
    fn owners_get_complementary_counts() {
        let c = Cells::plan(0.0, 100.0, 10.0);
        assert_eq!(c.even_count(), c.odd_count() + 1);
        assert_eq!(c.even_count() + c.odd_count(), c.n);
    }
}
