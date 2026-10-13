//! Modo Sketch: solo los bordes de la imagen, como un dibujo a línea.
//!
//! Es un detector de bordes tipo Canny simplificado, con parámetros fijos:
//!
//! 1. Suavizado gaussiano separable con sigma = `BLUR_MM` (0.2 mm) pasado a píxeles y
//!    acotado a `[MIN_SIGMA_PX, MAX_SIGMA_PX]` (0.8 a 4 px). Quita el ruido del
//!    remuestreo y de las fotos.
//! 2. Gradiente de Sobel con bordes replicados (sin bordes falsos en el marco); la
//!    magnitud se expresa en niveles de gris por píxel.
//! 3. Referencia de contraste: `max(magnitud máxima, REF_FLOOR * borde blanco-negro)`.
//!    Con el suelo, una imagen casi plana no se normaliza hacia arriba y no produce ruido.
//! 4. Adelgazamiento: se conserva solo el máximo local a lo largo de la dirección del
//!    gradiente (cuantizada a 4 direcciones), de modo que los trazos miden ~1 píxel.
//! 5. Histéresis: se parte de los píxeles por encima de `HIGH` (25 % de la referencia) y
//!    se extienden por vecindad de 8 hasta los que superan `LOW` (10 %). Elimina el
//!    punteado débil que no toca ningún borde claro.
//!
//! La salida es 1.0 en los bordes y 0.0 en el resto (potencia binaria).

use crate::gray::GrayF32;

const BLUR_MM: f64 = 0.2;
const MIN_SIGMA_PX: f64 = 0.8;
const MAX_SIGMA_PX: f64 = 4.0;
const REF_FLOOR: f32 = 0.5;
const HIGH: f32 = 0.25;
const LOW: f32 = 0.10;

pub fn sketch(img: &GrayF32, (px, py): (f64, f64)) -> Vec<f32> {
    let (w, h) = (img.width, img.height);
    let sigma = (BLUR_MM / ((px + py) / 2.0)).clamp(MIN_SIGMA_PX, MAX_SIGMA_PX);
    let blurred = blur(&img.data, w, h, sigma);
    let (mag, dir) = sobel(&blurred, w, h);

    // Un escalón blanco-negro, tras el suavizado, tiene pendiente máxima ~1/(sigma*sqrt(2*pi))
    // niveles por píxel (como mucho 0.5 sin suavizado, medida con Sobel/8).
    let full_edge = (1.0 / (sigma * (2.0 * std::f64::consts::PI).sqrt())).min(0.5) as f32;
    let max_mag = mag.iter().copied().fold(0.0f32, f32::max);
    let reference = max_mag.max(REF_FLOOR * full_edge);
    let (high, low) = (HIGH * reference, LOW * reference);

    // Adelgazamiento: candidatos = máximos locales por encima de `low`.
    let mut state = vec![0u8; w * h]; // 0 = nada, 1 = débil, 2 = fuerte
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let m = mag[i];
            if m < low {
                continue;
            }
            let (dx, dy) = match dir[i] {
                0 => (1i64, 0i64),
                1 => (1, 1),
                2 => (0, 1),
                _ => (-1, 1),
            };
            let at = |sx: i64, sy: i64| {
                let (nx, ny) = (x as i64 + sx, y as i64 + sy);
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                    0.0
                } else {
                    mag[ny as usize * w + nx as usize]
                }
            };
            // `>=` de un lado y `>` del otro para quedarse con uno solo si hay empate.
            if m >= at(-dx, -dy) && m > at(dx, dy) {
                state[i] = if m >= high { 2 } else { 1 };
            }
        }
    }

    // Histéresis: los fuertes arrastran a los débiles conectados (vecindad de 8).
    let mut out = vec![0.0f32; w * h];
    let mut stack: Vec<usize> = (0..w * h).filter(|&i| state[i] == 2).collect();
    for &i in &stack {
        out[i] = 1.0;
    }
    while let Some(i) = stack.pop() {
        let (x, y) = ((i % w) as i64, (i / w) as i64);
        for ny in (y - 1).max(0)..=(y + 1).min(h as i64 - 1) {
            for nx in (x - 1).max(0)..=(x + 1).min(w as i64 - 1) {
                let n = ny as usize * w + nx as usize;
                if state[n] == 1 && out[n] == 0.0 {
                    out[n] = 1.0;
                    stack.push(n);
                }
            }
        }
    }
    out
}

/// Gaussiana separable con bordes replicados.
fn blur(data: &[f32], w: usize, h: usize, sigma: f64) -> Vec<f32> {
    let radius = (3.0 * sigma).ceil() as i64;
    let mut kernel: Vec<f32> = (-radius..=radius)
        .map(|i| (-(i * i) as f64 / (2.0 * sigma * sigma)).exp() as f32)
        .collect();
    let total: f32 = kernel.iter().sum();
    kernel.iter_mut().for_each(|k| *k /= total);

    let mut tmp = vec![0.0f32; w * h];
    for y in 0..h {
        let row = &data[y * w..(y + 1) * w];
        for x in 0..w {
            let mut acc = 0.0;
            for (k, &wt) in kernel.iter().enumerate() {
                let sx = (x as i64 + k as i64 - radius).clamp(0, w as i64 - 1) as usize;
                acc += row[sx] * wt;
            }
            tmp[y * w + x] = acc;
        }
    }
    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        for (k, &wt) in kernel.iter().enumerate() {
            let sy = (y as i64 + k as i64 - radius).clamp(0, h as i64 - 1) as usize;
            let src = &tmp[sy * w..(sy + 1) * w];
            let dst = &mut out[y * w..(y + 1) * w];
            for (d, s) in dst.iter_mut().zip(src) {
                *d += s * wt;
            }
        }
    }
    out
}

/// Magnitud de Sobel (en niveles por píxel, es decir, dividida entre 8) y dirección del
/// gradiente cuantizada: 0 = horizontal, 1 = diagonal descendente (x e y crecen),
/// 2 = vertical, 3 = diagonal ascendente.
fn sobel(data: &[f32], w: usize, h: usize) -> (Vec<f32>, Vec<u8>) {
    let mut mag = vec![0.0f32; w * h];
    let mut dir = vec![0u8; w * h];
    for y in 0..h {
        let (ya, yb) = (y.saturating_sub(1), (y + 1).min(h - 1));
        for x in 0..w {
            let (xa, xb) = (x.saturating_sub(1), (x + 1).min(w - 1));
            let p = |xx: usize, yy: usize| data[yy * w + xx];
            let gx =
                (p(xb, ya) + 2.0 * p(xb, y) + p(xb, yb)) - (p(xa, ya) + 2.0 * p(xa, y) + p(xa, yb));
            let gy =
                (p(xa, yb) + 2.0 * p(x, yb) + p(xb, yb)) - (p(xa, ya) + 2.0 * p(x, ya) + p(xb, ya));
            let i = y * w + x;
            mag[i] = gx.hypot(gy) / 8.0;
            // Ángulo en octantes de 45 grados (sin signo de sentido).
            let a = gy.atan2(gx).to_degrees().rem_euclid(180.0);
            dir[i] = match a {
                a if !(22.5..157.5).contains(&a) => 0,
                a if a < 67.5 => 1,
                a if a < 112.5 => 2,
                _ => 3,
            };
        }
    }
    (mag, dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PX: (f64, f64) = (0.1, 0.1);

    /// Cuadrado negro de 100x100 px (10 mm) sobre fondo blanco de 200x200 px.
    fn square() -> GrayF32 {
        GrayF32::from_fn(200, 200, |x, y| {
            if (50..150).contains(&x) && (50..150).contains(&y) {
                0.0
            } else {
                1.0
            }
        })
    }

    #[test]
    fn square_outline_only() {
        let p = sketch(&square(), PX);
        let lit: Vec<(usize, usize)> = (0..200)
            .flat_map(|y| (0..200).map(move |x| (x, y)))
            .filter(|&(x, y)| p[y * 200 + x] > 0.0)
            .collect();
        assert!(!lit.is_empty());
        // Todo lo encendido queda pegado al contorno (a menos de 3 px).
        for &(x, y) in &lit {
            let near_x = [50i64, 150].iter().any(|e| (x as i64 - e).abs() <= 3);
            let near_y = [50i64, 150].iter().any(|e| (y as i64 - e).abs() <= 3);
            let in_x = (47..=153).contains(&x);
            let in_y = (47..=153).contains(&y);
            assert!(
                (near_x && in_y) || (near_y && in_x),
                "píxel fuera del contorno: ({x}, {y})"
            );
        }
        // Interior, exterior y marco de la imagen vacíos.
        assert_eq!(p[100 * 200 + 100], 0.0);
        assert_eq!(p[10 * 200 + 10], 0.0);
        // Cada lado del cuadrado queda cubierto casi entero y es delgado.
        let covered = |f: &dyn Fn(usize) -> (usize, usize)| {
            (55..145)
                .filter(|&k| {
                    let (x, y) = f(k);
                    (x.saturating_sub(2)..=x + 2)
                        .any(|xx| (y.saturating_sub(2)..=y + 2).any(|yy| p[yy * 200 + xx] > 0.0))
                })
                .count()
        };
        assert!(covered(&|k| (k, 50)) >= 88);
        assert!(covered(&|k| (k, 150)) >= 88);
        assert!(covered(&|k| (50, k)) >= 88);
        assert!(covered(&|k| (150, k)) >= 88);
        // Trazos finos: pocos píxeles por unidad de perímetro (4 lados x 100 px).
        assert!(lit.len() < 400 * 2, "{} píxeles", lit.len());
    }

    #[test]
    fn flat_and_faint_regions_stay_empty() {
        for g in [0.0f32, 0.5, 1.0] {
            let img = GrayF32::from_fn(80, 80, |_, _| g);
            assert!(sketch(&img, PX).iter().all(|&v| v == 0.0));
        }
        // Ruido de +-2 % no produce bordes.
        let noisy = GrayF32::from_fn(80, 80, |x, y| {
            0.5 + 0.02 * (((x * 7919 + y * 104729) % 13) as f32 / 12.0 - 0.5)
        });
        assert!(sketch(&noisy, PX).iter().all(|&v| v == 0.0));
    }

    #[test]
    fn output_is_binary_and_tiny_images_do_not_panic() {
        let img = GrayF32::from_fn(30, 20, |x, y| ((x * 31 + y * 17) % 50) as f32 / 49.0);
        assert!(sketch(&img, PX).iter().all(|&v| v == 0.0 || v == 1.0));
        for (w, h) in [(1, 1), (1, 5), (5, 1), (2, 2)] {
            let img = GrayF32::from_fn(w, h, |x, _| (x % 2) as f32);
            assert_eq!(sketch(&img, (1.0, 1.0)).len(), w * h);
        }
    }

    #[test]
    fn diagonal_edge_is_thin() {
        let img = GrayF32::from_fn(120, 120, |x, y| if x > y { 1.0 } else { 0.0 });
        let p = sketch(&img, PX);
        let lit = p.iter().filter(|&&v| v > 0.0).count();
        assert!((100..400).contains(&lit), "{lit}");
    }
}
