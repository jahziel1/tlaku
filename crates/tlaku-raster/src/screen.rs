//! Tramas de semitono (`Halftone`) y de líneas (`Newsprint`) sobre una rejilla girada.
//!
//! Ambas trabajan en mm con el eje Y hacia arriba (la fila 0 de la imagen es la de
//! arriba) y giran la trama `angle_deg` grados en sentido antihorario. La oscuridad de
//! cada celda es la media de `1 - gris` de los píxeles cuyo centro cae en ella, así que
//! la cobertura media del resultado sigue al gris de la imagen.

use crate::gray::GrayF32;

/// La celda efectiva nunca baja de este número de píxeles: con menos no cabe un punto
/// ni una línea con grosor gradual.
const MIN_CELL_PX: f64 = 3.0;

/// Coordenadas `(u, v)` de cada píxel en la rejilla girada, separables por columna y fila.
struct Frame {
    ux: Vec<f64>,
    uy: Vec<f64>,
    vx: Vec<f64>,
    vy: Vec<f64>,
}

impl Frame {
    fn new(w: usize, h: usize, angle_deg: f64, (px, py): (f64, f64)) -> Self {
        let angle = if angle_deg.is_finite() {
            angle_deg
        } else {
            0.0
        };
        let (sin, cos) = angle.to_radians().sin_cos();
        let xs: Vec<f64> = (0..w).map(|x| (x as f64 + 0.5) * px).collect();
        let ys: Vec<f64> = (0..h).map(|y| -(y as f64 + 0.5) * py).collect();
        Self {
            ux: xs.iter().map(|x| x * cos).collect(),
            uy: ys.iter().map(|y| y * sin).collect(),
            vx: xs.iter().map(|x| -x * sin).collect(),
            vy: ys.iter().map(|y| y * cos).collect(),
        }
    }

    #[inline]
    fn uv(&self, x: usize, y: usize) -> (f64, f64) {
        (self.ux[x] + self.uy[y], self.vx[x] + self.vy[y])
    }

    /// Mínimo y máximo de `u` y `v`. Como son lineales, basta con las 4 esquinas.
    fn bounds(&self) -> ((f64, f64), (f64, f64)) {
        let (w, h) = (self.ux.len(), self.uy.len());
        let mut u = (f64::INFINITY, f64::NEG_INFINITY);
        let mut v = u;
        for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
            let (cu, cv) = self.uv(x, y);
            u = (u.0.min(cu), u.1.max(cu));
            v = (v.0.min(cv), v.1.max(cv));
        }
        (u, v)
    }
}

fn effective_cell(cell_mm: f64, (px, py): (f64, f64)) -> f64 {
    let cell = if cell_mm.is_finite() && cell_mm > 0.0 {
        cell_mm
    } else {
        0.0
    };
    cell.max(MIN_CELL_PX * px.max(py))
}

#[inline]
fn darkness(g: f32) -> f32 {
    1.0 - g.clamp(0.0, 1.0)
}

/// Semitono clásico: un disco centrado en cada celda cuya área es la oscuridad media de
/// la celda. Pasada la oscuridad en que el disco toca los lados de la celda, el disco se
/// recorta contra ella (y los huecos blancos quedan en las esquinas), de modo que el área
/// encendida sigue siendo exactamente proporcional hasta el 100 %.
pub fn halftone(img: &GrayF32, cell_mm: f64, angle_deg: f64, pixel_mm: (f64, f64)) -> Vec<f32> {
    let (w, h) = (img.width, img.height);
    let cell = effective_cell(cell_mm, pixel_mm);
    let frame = Frame::new(w, h, angle_deg, pixel_mm);
    let ((umin, _), (vmin, _)) = frame.bounds();
    let (i0, j0) = ((umin / cell).floor() as i64, (vmin / cell).floor() as i64);
    let cell_index = |x: usize, y: usize| {
        let (u, v) = frame.uv(x, y);
        (
            (u / cell).floor() as i64 - i0,
            (v / cell).floor() as i64 - j0,
        )
    };
    let ((_, umax), (_, vmax)) = frame.bounds();
    let ni = ((umax / cell).floor() as i64 - i0 + 1) as usize;
    let nj = ((vmax / cell).floor() as i64 - j0 + 1) as usize;

    let mut sum = vec![0.0f32; ni * nj];
    let mut count = vec![0u32; ni * nj];
    for y in 0..h {
        for x in 0..w {
            let (i, j) = cell_index(x, y);
            let k = j as usize * ni + i as usize;
            sum[k] += darkness(img.data[y * w + x]);
            count[k] += 1;
        }
    }

    let radius = radius_table();
    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let (i, j) = cell_index(x, y);
            let k = j as usize * ni + i as usize;
            let d = sum[k] / count[k] as f32;
            let r = radius[(d.clamp(0.0, 1.0) * 255.0).round() as usize] * cell;
            let (u, v) = frame.uv(x, y);
            let du = u - (i + i0) as f64 * cell - cell / 2.0;
            let dv = v - (j + j0) as f64 * cell - cell / 2.0;
            if du * du + dv * dv < r * r {
                out[y * w + x] = 1.0;
            }
        }
    }
    out
}

/// Área de un círculo de radio `r` recortado a una celda unidad centrada en él.
fn clipped_disc_area(r: f64) -> f64 {
    use std::f64::consts::{FRAC_1_SQRT_2, PI};
    if r <= 0.5 {
        PI * r * r
    } else if r < FRAC_1_SQRT_2 {
        // Se quitan los 4 segmentos que sobresalen de los lados.
        let segment = r * r * (0.5 / r).acos() - 0.5 * (r * r - 0.25).sqrt();
        PI * r * r - 4.0 * segment
    } else {
        1.0
    }
}

/// Radio (en celdas) para cada oscuridad `k / 255`. Se invierte `clipped_disc_area` por
/// bisección. El 100 % usa un radio algo mayor que la semidiagonal para cubrir las esquinas.
fn radius_table() -> Vec<f64> {
    use std::f64::consts::FRAC_1_SQRT_2;
    (0..=255)
        .map(|k| {
            let d = k as f64 / 255.0;
            if d >= 1.0 {
                return FRAC_1_SQRT_2 * 1.05;
            }
            let (mut lo, mut hi) = (0.0, FRAC_1_SQRT_2);
            for _ in 0..40 {
                let mid = (lo + hi) / 2.0;
                if clipped_disc_area(mid) < d {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            (lo + hi) / 2.0
        })
        .collect()
}

/// Líneas paralelas con periodo `cell_mm` (medido perpendicularmente a ellas), cuyo
/// grosor es `periodo * oscuridad local`. La oscuridad se promedia en tramos de medio
/// periodo a lo largo de la línea y se interpola linealmente entre ellos, para que el
/// grosor cambie con suavidad en vez de a saltos.
pub fn newsprint(img: &GrayF32, cell_mm: f64, angle_deg: f64, pixel_mm: (f64, f64)) -> Vec<f32> {
    let (w, h) = (img.width, img.height);
    let period = effective_cell(cell_mm, pixel_mm);
    let seg = period / 2.0;
    let frame = Frame::new(w, h, angle_deg, pixel_mm);
    let ((umin, umax), (vmin, vmax)) = frame.bounds();
    // Un tramo de margen a cada lado para la interpolación.
    let b0 = (umin / seg).floor() as i64 - 1;
    let nb = ((umax / seg).floor() as i64 - b0 + 2) as usize;
    let j0 = (vmin / period).floor() as i64;
    let nj = ((vmax / period).floor() as i64 - j0 + 1) as usize;

    let mut sum = vec![0.0f32; nb * nj];
    let mut count = vec![0u32; nb * nj];
    for y in 0..h {
        for x in 0..w {
            let (u, v) = frame.uv(x, y);
            let b = ((u / seg).floor() as i64 - b0) as usize;
            let j = ((v / period).floor() as i64 - j0) as usize;
            sum[j * nb + b] += darkness(img.data[y * w + x]);
            count[j * nb + b] += 1;
        }
    }
    let mean = |j: usize, b: usize| {
        let k = j * nb + b;
        (count[k] > 0).then(|| sum[k] / count[k] as f32)
    };

    // Tamaño del píxel medido en la perpendicular a las líneas.
    let angle = if angle_deg.is_finite() {
        angle_deg
    } else {
        0.0
    };
    let (sin, cos) = angle.to_radians().sin_cos();
    let pitch = (pixel_mm.0 * sin.abs() + pixel_mm.1 * cos.abs()).max(1e-9);
    // Los píxeles de borde de la línea solo se encienden en una parte proporcional a su
    // cobertura, con una diente de sierra a lo largo de la línea (varios por periodo).
    // Así el grosor medio no se cuantiza al píxel y el borde queda como trazos cortos.
    let dashes = (period / (4.0 * pixel_mm.0.max(pixel_mm.1)))
        .round()
        .max(1.0);
    let dash = |u: f64| 1e-4 + (1.0 - 2e-4) * (u * dashes / period).rem_euclid(1.0);

    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let (u, v) = frame.uv(x, y);
            let j = ((v / period).floor() as i64 - j0) as usize;
            // Posición entre los centros de dos tramos contiguos.
            let t = u / seg - 0.5;
            let left = t.floor();
            let frac = (t - left) as f32;
            let bl = (left as i64 - b0) as usize;
            let d = match (mean(j, bl), mean(j, bl + 1)) {
                (Some(a), Some(b)) => a * (1.0 - frac) + b * frac,
                (Some(a), None) | (None, Some(a)) => a,
                (None, None) => 0.0,
            };
            let dv = (v - (j as i64 + j0) as f64 * period - period / 2.0).abs();
            // Cobertura del píxel por la franja [-media, media]; el píxel mide `pitch`
            // en la perpendicular (aproximado con una caja).
            let half = f64::from(d) * period / 2.0;
            let coverage = ((half - dv) / pitch + 0.5).clamp(0.0, 1.0);
            if coverage > dash(u) {
                out[y * w + x] = 1.0;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PX: (f64, f64) = (0.1, 0.1);

    fn flat(g: f32, w: usize, h: usize) -> GrayF32 {
        GrayF32::from_fn(w, h, |_, _| g)
    }

    fn coverage(data: &[f32]) -> f64 {
        data.iter().filter(|&&v| v > 0.0).count() as f64 / data.len() as f64
    }

    #[test]
    fn clipped_area_is_continuous_and_monotonic() {
        let mut prev = 0.0;
        for k in 0..=1000 {
            let r = k as f64 / 1000.0 * 0.7072;
            let a = clipped_disc_area(r);
            assert!(a >= prev - 1e-12);
            prev = a;
        }
        assert!((clipped_disc_area(0.5) - std::f64::consts::FRAC_PI_4).abs() < 1e-12);
        assert!((clipped_disc_area(std::f64::consts::FRAC_1_SQRT_2 - 1e-9) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn halftone_coverage_follows_gray() {
        for angle in [0.0, 15.0, 45.0, 75.0] {
            for dark in [0.25f32, 0.5, 0.75] {
                let p = halftone(&flat(1.0 - dark, 200, 200), 1.0, angle, PX);
                let c = coverage(&p);
                assert!(
                    (c - f64::from(dark)).abs() < 0.05,
                    "ángulo {angle}, oscuridad {dark}: {c}"
                );
            }
        }
    }

    #[test]
    fn halftone_extremes_are_pure() {
        assert!(
            halftone(&flat(1.0, 60, 60), 1.0, 30.0, PX)
                .iter()
                .all(|&v| v == 0.0)
        );
        assert!(
            halftone(&flat(0.0, 60, 60), 1.0, 30.0, PX)
                .iter()
                .all(|&v| v == 1.0)
        );
    }

    #[test]
    fn halftone_dots_grow_with_darkness() {
        let img = GrayF32::from_fn(300, 100, |x, _| 1.0 - x as f32 / 299.0);
        let p = halftone(&img, 1.0, 45.0, PX);
        let third = |k: usize| {
            let mut n = 0;
            for y in 0..100 {
                for x in k * 100..(k + 1) * 100 {
                    n += (p[y * 300 + x] > 0.0) as usize;
                }
            }
            n
        };
        assert!(third(0) < third(1) && third(1) < third(2));
    }

    /// Fracción de píxeles que coinciden al desplazar el bitmap `(dx, dy)` píxeles.
    fn shift_match(p: &[f32], w: usize, h: usize, dx: i64, dy: i64) -> f64 {
        let (mut same, mut total) = (0usize, 0usize);
        for y in 0..h as i64 {
            for x in 0..w as i64 {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                    continue;
                }
                total += 1;
                same +=
                    (p[(y as usize) * w + x as usize] == p[ny as usize * w + nx as usize]) as usize;
            }
        }
        same as f64 / total as f64
    }

    #[test]
    fn halftone_grid_rotates() {
        let img = flat(0.7, 240, 240);
        // Celda de sqrt(2) mm: a 45 grados el periodo de la trama es 10 px en diagonal.
        let cell = 2.0f64.sqrt();
        let p0 = halftone(&img, cell, 0.0, PX);
        let p45 = halftone(&img, cell, 45.0, PX);
        assert_ne!(p0, p45);
        // A 0 grados el periodo es 14.14 px en X e Y, no 10 en diagonal.
        let diag0 = shift_match(&p0, 240, 240, 10, 10);
        let diag45 = shift_match(&p45, 240, 240, 10, 10);
        let anti45 = shift_match(&p45, 240, 240, 10, -10);
        let axis45 = shift_match(&p45, 240, 240, 14, 0);
        assert!(diag45 > 0.95 && anti45 > 0.95, "{diag45} {anti45}");
        assert!(axis45 < 0.9 && diag0 < 0.9, "{axis45} {diag0}");
        // El sentido del giro es antihorario: 30 y -30 grados no coinciden.
        assert_ne!(
            halftone(&img, cell, 30.0, PX),
            halftone(&img, cell, -30.0, PX)
        );
    }

    #[test]
    fn halftone_period_is_the_cell_size() {
        let p = halftone(&flat(0.7, 200, 200), 1.0, 0.0, PX);
        // 1 mm = 10 px.
        assert!(shift_match(&p, 200, 200, 10, 0) > 0.99);
        assert!(shift_match(&p, 200, 200, 0, 10) > 0.99);
        assert!(shift_match(&p, 200, 200, 5, 0) < 0.9);
    }

    #[test]
    fn newsprint_coverage_follows_gray() {
        for angle in [0.0, 30.0, 45.0, 90.0] {
            for dark in [0.25f32, 0.5, 0.75] {
                let p = newsprint(&flat(1.0 - dark, 200, 200), 1.0, angle, PX);
                let c = coverage(&p);
                assert!(
                    (c - f64::from(dark)).abs() < 0.05,
                    "ángulo {angle}, oscuridad {dark}: {c}"
                );
            }
        }
    }

    #[test]
    fn newsprint_lines_follow_the_angle() {
        // Oscuridad 0.6: la franja mide 6 px exactos y no hay píxeles parciales.
        let img = flat(0.4, 200, 200);
        // A 0 grados las líneas son horizontales: cada fila es constante a lo largo de X.
        let p = newsprint(&img, 1.0, 0.0, PX);
        for y in 0..200 {
            let first = p[y * 200];
            assert!((0..200).all(|x| p[y * 200 + x] == first), "fila {y}");
        }
        // A 90 grados son verticales: cada columna es constante.
        let q = newsprint(&img, 1.0, 90.0, PX);
        for x in 0..200 {
            let first = q[x];
            assert!((0..200).all(|y| q[y * 200 + x] == first), "columna {x}");
        }
        // A 45 grados, el patrón se repite desplazando 1 mm en diagonal (a lo largo de la línea).
        let r = newsprint(&img, 2.0f64.sqrt(), 45.0, PX);
        assert!(shift_match(&r, 200, 200, 10, -10) > 0.97);
        // Medio periodo en la perpendicular invierte las franjas.
        assert!(shift_match(&r, 200, 200, 5, 5) < 0.8);
    }

    #[test]
    fn newsprint_lines_thicken_with_darkness() {
        let img = GrayF32::from_fn(300, 100, |x, _| 1.0 - x as f32 / 299.0);
        let p = newsprint(&img, 1.0, 0.0, PX);
        let third = |k: usize| {
            let mut n = 0;
            for y in 0..100 {
                for x in k * 100..(k + 1) * 100 {
                    n += (p[y * 300 + x] > 0.0) as usize;
                }
            }
            n
        };
        assert!(third(0) < third(1) && third(1) < third(2));
    }

    #[test]
    fn invalid_cell_falls_back_to_minimum() {
        for cell in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let p = halftone(&flat(0.5, 40, 40), cell, 0.0, PX);
            assert_eq!(p.len(), 1600);
        }
        let p = newsprint(&flat(0.5, 1, 1), 1.0, 20.0, PX);
        assert_eq!(p.len(), 1);
    }
}
