//! Acomodo de las piezas de un generador: filas de izquierda a derecha que saltan de renglón al
//! llegar al ancho máximo (el de la cama). No hace nesting: respeta el orden del generador para
//! que el resultado sea predecible; el nesting de la app puede reacomodar después.

use crate::part::Part;

/// Esquina inferior izquierda de una pieza dentro del conjunto.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub x: f64,
    pub y: f64,
}

/// Posiciones de las piezas y tamaño del conjunto.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub places: Vec<Placement>,
    pub width: f64,
    pub height: f64,
}

/// Reparte `parts` en filas de a lo más `max_width` mm (una pieza más ancha va sola en su fila),
/// con `gap` mm entre piezas. La primera fila queda abajo; la esquina inferior izquierda del
/// conjunto es (0, 0).
pub fn pack(parts: &[Part], max_width: f64, gap: f64) -> Layout {
    let mut places = Vec::with_capacity(parts.len());
    let (mut x, mut y, mut row_h, mut width) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    for p in parts {
        if x > 0.0 && x + p.width > max_width + 1e-9 {
            y += row_h + gap;
            x = 0.0;
            row_h = 0.0;
        }
        places.push(Placement { x, y });
        x += p.width + gap;
        row_h = row_h.max(p.height);
        width = width.max(x - gap);
    }
    Layout {
        places,
        width,
        height: y + row_h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(w: f64, h: f64) -> Part {
        Part {
            key: "p".into(),
            name: "p".into(),
            cuts: Vec::new(),
            engraves: Vec::new(),
            width: w,
            height: h,
        }
    }

    #[test]
    fn single_row_when_everything_fits() {
        let parts = [part(100.0, 60.0), part(80.0, 60.0)];
        let l = pack(&parts, 400.0, 5.0);
        assert_eq!(l.places[1], Placement { x: 105.0, y: 0.0 });
        assert_eq!((l.width, l.height), (185.0, 60.0));
    }

    #[test]
    fn wraps_and_never_overlaps() {
        let parts: Vec<Part> = (0..9)
            .map(|i| part(40.0 + 7.0 * i as f64, 20.0 + 3.0 * i as f64))
            .collect();
        let gap = 4.0;
        let l = pack(&parts, 150.0, gap);
        assert!(l.width <= 150.0 + 1e-9);
        for i in 0..parts.len() {
            let (a, pa) = (&parts[i], l.places[i]);
            assert!(
                pa.x >= 0.0
                    && pa.x + a.width <= l.width + 1e-9
                    && pa.y + a.height <= l.height + 1e-9
            );
            for (j, b) in parts.iter().enumerate().skip(i + 1) {
                let pb = l.places[j];
                let apart = pa.x + a.width + gap <= pb.x + 1e-9
                    || pb.x + b.width + gap <= pa.x + 1e-9
                    || pa.y + a.height + gap <= pb.y + 1e-9
                    || pb.y + b.height + gap <= pa.y + 1e-9;
                assert!(apart, "las piezas {i} y {j} se tocan");
            }
        }
    }

    #[test]
    fn a_part_wider_than_the_bed_goes_alone() {
        let parts = [part(30.0, 10.0), part(500.0, 10.0), part(30.0, 10.0)];
        let l = pack(&parts, 200.0, 2.0);
        assert_eq!(l.places[1].x, 0.0);
        assert!(l.places[1].y > 0.0);
        assert_eq!(l.width, 500.0);
    }
}
