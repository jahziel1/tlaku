//! Disposición de las marcas en la cama y sus trazos.

use super::{INSET, MIN_BED_MM};
use tlaku_core::Point;
use tlaku_i18n::tr;

// ---------------------------------------------------------------------------
// Disposición
// ---------------------------------------------------------------------------

/// Posiciones y tamaño de las marcas en la cama.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkLayout {
    pub bed_mm: [f64; 2],
    /// Centros de las marcas (mm). Los 4 primeros son las esquinas en el orden
    /// inferior izquierda, inferior derecha, superior derecha, superior izquierda; con 9
    /// siguen los puntos medios (abajo, derecha, arriba, izquierda) y el centro.
    pub marks: Vec<Point>,
    /// Semilongitud de cada brazo de la cruz (mm).
    pub arm_mm: f64,
    /// Radio del círculo de la marca 0 (mm); rodea la cruz sin tocar sus brazos.
    pub ring_mm: f64,
}

/// Un trazo que se graba (polilínea).
#[derive(Debug, Clone, PartialEq)]
pub struct MarkStroke {
    pub mark: usize,
    pub points: Vec<Point>,
    pub closed: bool,
}

impl MarkLayout {
    /// `count` debe ser 4 o 9.
    pub fn new(bed_mm: [f64; 2], count: usize) -> Result<Self, String> {
        if count != 4 && count != 9 {
            return Err(tr!("camera.layout.bad_count", count = count));
        }
        if !bed_mm.iter().all(|v| v.is_finite() && *v >= MIN_BED_MM) {
            return Err(tr!("camera.layout.bed_small", min = MIN_BED_MM));
        }
        let [w, h] = bed_mm;
        let (x0, x1) = (INSET * w, (1.0 - INSET) * w);
        let (y0, y1) = (INSET * h, (1.0 - INSET) * h);
        let (xm, ym) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let mut marks = vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ];
        if count == 9 {
            marks.extend([
                Point::new(xm, y0),
                Point::new(x1, ym),
                Point::new(xm, y1),
                Point::new(x0, ym),
                Point::new(xm, ym),
            ]);
        }
        let arm_mm = (0.04 * w.min(h)).clamp(4.0, 10.0);
        Ok(Self {
            bed_mm,
            marks,
            arm_mm,
            ring_mm: 1.25 * arm_mm,
        })
    }

    /// Trazos que hay que grabar: dos segmentos por cruz y el círculo de la marca 0.
    pub fn strokes(&self) -> Vec<MarkStroke> {
        let a = self.arm_mm;
        let mut out = Vec::new();
        for (i, c) in self.marks.iter().enumerate() {
            out.push(MarkStroke {
                mark: i,
                points: vec![Point::new(c.x - a, c.y), Point::new(c.x + a, c.y)],
                closed: false,
            });
            out.push(MarkStroke {
                mark: i,
                points: vec![Point::new(c.x, c.y - a), Point::new(c.x, c.y + a)],
                closed: false,
            });
            if i == 0 {
                let n = 48;
                let pts = (0..n)
                    .map(|k| {
                        let t = std::f64::consts::TAU * k as f64 / n as f64;
                        Point::new(c.x + self.ring_mm * t.cos(), c.y + self.ring_mm * t.sin())
                    })
                    .collect();
                out.push(MarkStroke {
                    mark: i,
                    points: pts,
                    closed: true,
                });
            }
        }
        out
    }
}
