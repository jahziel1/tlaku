//! Homografía plana: DLT normalizada (Hartley y Zisserman), RANSAC y alineación
//! imagen (ya corregida) a milímetros de la cama.
//!
//! La homografía solo es válida para el plano en que se tomaron las
//! correspondencias (la altura del material al grabar las marcas). Para otro
//! espesor hay que guardar una alineación por altura.

use nalgebra::{DMatrix, Matrix3};
use serde::{Deserialize, Serialize};
use tlaku_core::Point;

/// Matriz 3x3 de transformación proyectiva, por filas: `[x', y', w'] = M [x, y, 1]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Homography {
    pub m: [[f64; 3]; 3],
}

impl Homography {
    pub fn identity() -> Self {
        Self {
            m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    pub(crate) fn from_matrix(h: &Matrix3<f64>) -> Self {
        let mut m = [[0.0; 3]; 3];
        for (r, row) in m.iter_mut().enumerate() {
            for (c, v) in row.iter_mut().enumerate() {
                *v = h[(r, c)];
            }
        }
        Self { m }
    }

    pub(crate) fn matrix(&self) -> Matrix3<f64> {
        Matrix3::from_fn(|r, c| self.m[r][c])
    }

    /// Aplica la transformación. Si el punto cae en el infinito devuelve
    /// coordenadas no finitas.
    pub fn apply(&self, p: Point) -> Point {
        let m = &self.m;
        let w = m[2][0] * p.x + m[2][1] * p.y + m[2][2];
        Point::new(
            (m[0][0] * p.x + m[0][1] * p.y + m[0][2]) / w,
            (m[1][0] * p.x + m[1][1] * p.y + m[1][2]) / w,
        )
    }

    pub fn apply_all(&self, pts: &[Point]) -> Vec<Point> {
        pts.iter().map(|&p| self.apply(p)).collect()
    }

    pub fn inverse(&self) -> Result<Homography, String> {
        let inv = self
            .matrix()
            .try_inverse()
            .ok_or_else(|| "la homografía es singular y no se puede invertir".to_string())?;
        Ok(Self::from_matrix(&normalize_scale(inv)))
    }

    /// Composición: primero `self`, después `next`.
    pub fn then(&self, next: &Homography) -> Homography {
        Self::from_matrix(&normalize_scale(next.matrix() * self.matrix()))
    }

    /// Error cuadrático medio (raíz) de la transferencia sobre los pares dados,
    /// en las unidades del destino.
    pub fn rms_error(&self, pairs: &[(Point, Point)]) -> f64 {
        if pairs.is_empty() {
            return 0.0;
        }
        let s: f64 = pairs
            .iter()
            .map(|&(a, b)| {
                let q = self.apply(a);
                (q.x - b.x).powi(2) + (q.y - b.y).powi(2)
            })
            .sum();
        (s / pairs.len() as f64).sqrt()
    }
}

fn normalize_scale(m: Matrix3<f64>) -> Matrix3<f64> {
    if m[(2, 2)].abs() > 1e-12 {
        m / m[(2, 2)]
    } else {
        let n = m.norm();
        if n > 0.0 { m / n } else { m }
    }
}

/// Normalización de Hartley: centroide al origen y distancia media `sqrt(2)`.
fn normalization(pts: &[Point]) -> Result<Matrix3<f64>, String> {
    let n = pts.len() as f64;
    let cx = pts.iter().map(|p| p.x).sum::<f64>() / n;
    let cy = pts.iter().map(|p| p.y).sum::<f64>() / n;
    let mean = pts.iter().map(|p| (p.x - cx).hypot(p.y - cy)).sum::<f64>() / n;
    if !mean.is_finite() || mean < 1e-12 {
        return Err("los puntos son degenerados (todos coinciden)".into());
    }
    let s = std::f64::consts::SQRT_2 / mean;
    Ok(Matrix3::new(
        s,
        0.0,
        -s * cx,
        0.0,
        s,
        -s * cy,
        0.0,
        0.0,
        1.0,
    ))
}

/// Estima la homografía que lleva el primer punto de cada par al segundo con la
/// DLT normalizada. Con 4 pares la solución es exacta; con más, mínimos cuadrados
/// algebraicos.
pub fn estimate_homography(pairs: &[(Point, Point)]) -> Result<Homography, String> {
    if pairs.len() < 4 {
        return Err(format!(
            "se necesitan al menos 4 pares de puntos para la homografía (hay {})",
            pairs.len()
        ));
    }
    if pairs
        .iter()
        .any(|(a, b)| !(a.x.is_finite() && a.y.is_finite() && b.x.is_finite() && b.y.is_finite()))
    {
        return Err("hay puntos con coordenadas no finitas".into());
    }
    let src: Vec<Point> = pairs.iter().map(|p| p.0).collect();
    let dst: Vec<Point> = pairs.iter().map(|p| p.1).collect();
    let ts = normalization(&src)?;
    let td = normalization(&dst)?;

    // Se rellena con filas nulas hasta 9 para que la SVD devuelva el espacio nulo completo.
    let rows = (2 * pairs.len()).max(9);
    let mut a = DMatrix::<f64>::zeros(rows, 9);
    for (i, (p, q)) in pairs.iter().enumerate() {
        let (x, y) = (ts[(0, 0)] * p.x + ts[(0, 2)], ts[(1, 1)] * p.y + ts[(1, 2)]);
        let (u, v) = (td[(0, 0)] * q.x + td[(0, 2)], td[(1, 1)] * q.y + td[(1, 2)]);
        let r0 = [-x, -y, -1.0, 0.0, 0.0, 0.0, u * x, u * y, u];
        let r1 = [0.0, 0.0, 0.0, -x, -y, -1.0, v * x, v * y, v];
        for c in 0..9 {
            a[(2 * i, c)] = r0[c];
            a[(2 * i + 1, c)] = r1[c];
        }
    }
    let svd = a.svd(false, true);
    let v_t = svd
        .v_t
        .ok_or("falló la descomposición SVD de la homografía")?;
    let sv = &svd.singular_values;
    let mut order: Vec<usize> = (0..sv.len()).collect();
    order.sort_by(|&i, &j| sv[j].total_cmp(&sv[i]));
    let smallest = order[order.len() - 1];
    let second = order[order.len() - 2];
    if sv[second] <= 1e-9 * sv[order[0]].max(1e-300) {
        return Err(
            "los puntos son degenerados (colineales o repetidos); usa marcas bien repartidas"
                .into(),
        );
    }
    let hn = Matrix3::from_fn(|r, c| v_t[(smallest, 3 * r + c)]);
    let td_inv = td
        .try_inverse()
        .ok_or("normalización del destino no invertible")?;
    let h = td_inv * hn * ts;
    if !h.iter().all(|v| v.is_finite()) {
        return Err("la homografía resultante no es finita".into());
    }
    Ok(Homography::from_matrix(&normalize_scale(h)))
}

/// Opciones de RANSAC. `threshold` está en las unidades del destino (mm o px).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RansacOptions {
    pub threshold: f64,
    pub max_iterations: u32,
    pub confidence: f64,
    pub seed: u64,
}

impl Default for RansacOptions {
    fn default() -> Self {
        Self {
            threshold: 3.0,
            max_iterations: 2000,
            confidence: 0.99,
            seed: 0x0074_6c61_6b75,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RansacResult {
    pub homography: Homography,
    /// Marca por par: `true` si es un acierto del modelo final.
    pub inliers: Vec<bool>,
    pub inlier_count: usize,
}

struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

fn transfer_error(h: &Homography, pair: &(Point, Point)) -> f64 {
    let q = h.apply(pair.0);
    let e = (q.x - pair.1.x).hypot(q.y - pair.1.y);
    if e.is_finite() { e } else { f64::INFINITY }
}

fn collinear3(a: Point, b: Point, c: Point) -> bool {
    let area = ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).abs();
    let scale = (b.x - a.x).hypot(b.y - a.y) * (c.x - a.x).hypot(c.y - a.y);
    area <= 1e-6 * scale.max(1e-12)
}

fn degenerate_sample(s: &[(Point, Point)]) -> bool {
    for i in 0..4 {
        for j in (i + 1)..4 {
            for k in (j + 1)..4 {
                if collinear3(s[i].0, s[j].0, s[k].0) || collinear3(s[i].1, s[j].1, s[k].1) {
                    return true;
                }
            }
        }
    }
    false
}

/// Homografía robusta: RANSAC con muestras mínimas de 4 pares (determinista por
/// `seed`) y reajuste final por mínimos cuadrados sobre los aciertos.
pub fn estimate_homography_ransac(
    pairs: &[(Point, Point)],
    opts: &RansacOptions,
) -> Result<RansacResult, String> {
    let n = pairs.len();
    if n < 4 {
        return Err(format!(
            "se necesitan al menos 4 pares de puntos para la homografía (hay {n})"
        ));
    }
    let mut rng = XorShift(opts.seed | 1);
    let mut best: Option<(usize, f64, Homography)> = None;
    let mut needed = opts.max_iterations;
    let mut iter = 0;
    while iter < needed.min(opts.max_iterations) {
        iter += 1;
        let mut idx = [0usize; 4];
        let mut k = 0;
        while k < 4 {
            let c = (rng.next() % n as u64) as usize;
            if !idx[..k].contains(&c) {
                idx[k] = c;
                k += 1;
            }
        }
        let sample: Vec<(Point, Point)> = idx.iter().map(|&i| pairs[i]).collect();
        if degenerate_sample(&sample) {
            continue;
        }
        let Ok(h) = estimate_homography(&sample) else {
            continue;
        };
        let mut count = 0;
        let mut sum = 0.0;
        for p in pairs {
            let e = transfer_error(&h, p);
            if e <= opts.threshold {
                count += 1;
                sum += e;
            }
        }
        let better = match &best {
            None => true,
            Some((c, s, _)) => count > *c || (count == *c && sum < *s),
        };
        if better {
            best = Some((count, sum, h));
            let w = count as f64 / n as f64;
            let denom = (1.0 - w.powi(4)).ln();
            if w >= 1.0 {
                needed = 0;
            } else if denom < 0.0 {
                let it = ((1.0 - opts.confidence).ln() / denom).ceil();
                needed = if it.is_finite() {
                    it as u32
                } else {
                    opts.max_iterations
                };
            }
        }
    }
    let (count, _, mut h) = best.ok_or("RANSAC no encontró ninguna muestra válida")?;
    if count < 4 {
        return Err(format!(
            "no hay consenso: solo {count} de {n} pares concuerdan; revisa las marcas o sube el umbral"
        ));
    }
    // Reajuste sobre los aciertos, dos rondas.
    let mut inliers: Vec<bool> = pairs
        .iter()
        .map(|p| transfer_error(&h, p) <= opts.threshold)
        .collect();
    for _ in 0..2 {
        let sel: Vec<(Point, Point)> = pairs
            .iter()
            .zip(&inliers)
            .filter(|(_, ok)| **ok)
            .map(|(p, _)| *p)
            .collect();
        if sel.len() < 4 {
            break;
        }
        if let Ok(nh) = estimate_homography(&sel) {
            h = nh;
        }
        inliers = pairs
            .iter()
            .map(|p| transfer_error(&h, p) <= opts.threshold)
            .collect();
    }
    let inlier_count = inliers.iter().filter(|&&b| b).count();
    Ok(RansacResult {
        homography: h,
        inliers,
        inlier_count,
    })
}

/// Alineación entre la imagen de la cámara (ya sin distorsión) y la cama.
/// Es serializable para guardarla junto al perfil de la máquina y no perderla al
/// reiniciar. Es válida solo para la altura de material en que se grabaron las marcas.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ImageToBed {
    /// Píxel de la imagen corregida a milímetros de la cama.
    pub to_bed: Homography,
    /// Milímetros de la cama a píxel de la imagen corregida.
    pub to_image: Homography,
}

#[derive(Debug, Clone)]
pub struct AlignmentFit {
    pub alignment: ImageToBed,
    /// Error de transferencia (rms) en mm sobre los pares usados.
    pub rms_mm: f64,
    /// Qué pares se usaron (todos `true` si no se pidió modo robusto).
    pub inliers: Vec<bool>,
}

impl ImageToBed {
    pub fn from_homography(to_bed: Homography) -> Result<Self, String> {
        Ok(Self {
            to_image: to_bed.inverse()?,
            to_bed,
        })
    }

    /// Ajusta la alineación con marcas vistas en la imagen corregida (`image_px`) y su
    /// posición conocida en la cama (`bed_mm`). Con `robust` se descartan atípicos
    /// (el umbral va en mm).
    pub fn fit(
        image_px: &[Point],
        bed_mm: &[Point],
        robust: Option<&RansacOptions>,
    ) -> Result<AlignmentFit, String> {
        if image_px.len() != bed_mm.len() {
            return Err(format!(
                "hay {} puntos de imagen y {} de cama; deben coincidir",
                image_px.len(),
                bed_mm.len()
            ));
        }
        let pairs: Vec<(Point, Point)> = image_px
            .iter()
            .copied()
            .zip(bed_mm.iter().copied())
            .collect();
        let (h, inliers) = match robust {
            Some(o) => {
                let r = estimate_homography_ransac(&pairs, o)?;
                (r.homography, r.inliers)
            }
            None => (estimate_homography(&pairs)?, vec![true; pairs.len()]),
        };
        let used: Vec<(Point, Point)> = pairs
            .iter()
            .zip(&inliers)
            .filter(|(_, ok)| **ok)
            .map(|(p, _)| *p)
            .collect();
        Ok(AlignmentFit {
            rms_mm: h.rms_error(&used),
            alignment: Self::from_homography(h)?,
            inliers,
        })
    }

    pub fn pixel_to_bed(&self, p: Point) -> Point {
        self.to_bed.apply(p)
    }

    pub fn bed_to_pixel(&self, p: Point) -> Point {
        self.to_image.apply(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(seed: &mut u64) -> f64 {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((*seed >> 11) as f64) / ((1u64 << 53) as f64)
    }

    fn truth() -> Homography {
        Homography {
            m: [[0.8, 0.15, 30.0], [-0.1, 0.9, 12.0], [1.0e-4, -2.0e-4, 1.0]],
        }
    }

    fn grid_pairs(h: &Homography, n: usize, seed: u64) -> Vec<(Point, Point)> {
        let mut s = seed;
        (0..n)
            .map(|_| {
                let p = Point::new(lcg(&mut s) * 600.0, lcg(&mut s) * 400.0);
                (p, h.apply(p))
            })
            .collect()
    }

    #[test]
    fn exacta_con_cuatro_puntos() {
        let h = truth();
        let pairs = grid_pairs(&h, 4, 1);
        let est = estimate_homography(&pairs).unwrap();
        assert!(est.rms_error(&pairs) < 1e-8);
        let probe = Point::new(300.0, 200.0);
        let (a, b) = (est.apply(probe), h.apply(probe));
        assert!((a.x - b.x).hypot(a.y - b.y) < 1e-7);
    }

    #[test]
    fn exacta_con_muchos_puntos_e_inversa() {
        let h = truth();
        let pairs = grid_pairs(&h, 40, 2);
        let est = estimate_homography(&pairs).unwrap();
        assert!(est.rms_error(&pairs) < 1e-8);
        let inv = est.inverse().unwrap();
        for (a, b) in &pairs {
            let r = inv.apply(*b);
            assert!(r.dist(*a) < 1e-6);
        }
        let id = est.then(&inv);
        for i in 0..3 {
            for j in 0..3 {
                let e = if i == j { 1.0 } else { 0.0 };
                assert!((id.m[i][j] - e).abs() < 1e-8);
            }
        }
    }

    #[test]
    fn con_ruido_el_error_es_pequeno() {
        let h = truth();
        let mut s = 9;
        let pairs: Vec<_> = grid_pairs(&h, 60, 3)
            .into_iter()
            .map(|(a, b)| {
                (
                    a,
                    Point::new(
                        b.x + (lcg(&mut s) - 0.5) * 0.6,
                        b.y + (lcg(&mut s) - 0.5) * 0.6,
                    ),
                )
            })
            .collect();
        let est = estimate_homography(&pairs).unwrap();
        assert!(est.rms_error(&pairs) < 0.3);
    }

    #[test]
    fn menos_de_cuatro_o_colineales_fallan() {
        let h = truth();
        assert!(estimate_homography(&grid_pairs(&h, 3, 4)).is_err());
        let line: Vec<_> = (0..6)
            .map(|i| {
                let p = Point::new(i as f64 * 10.0, i as f64 * 5.0);
                (p, h.apply(p))
            })
            .collect();
        let e = estimate_homography(&line).unwrap_err();
        assert!(e.contains("degenerados"), "{e}");
    }

    #[test]
    fn ransac_descarta_atipicos() {
        let h = truth();
        let mut s = 77;
        let mut pairs: Vec<_> = grid_pairs(&h, 50, 5)
            .into_iter()
            .map(|(a, b)| {
                (
                    a,
                    Point::new(
                        b.x + (lcg(&mut s) - 0.5) * 0.4,
                        b.y + (lcg(&mut s) - 0.5) * 0.4,
                    ),
                )
            })
            .collect();
        // 12 atípicos groseros.
        for p in pairs.iter_mut().take(12) {
            p.1 = Point::new(
                p.1.x + 40.0 + lcg(&mut s) * 50.0,
                p.1.y - 35.0 - lcg(&mut s) * 40.0,
            );
        }
        let plain = estimate_homography(&pairs).unwrap();
        let r = estimate_homography_ransac(
            &pairs,
            &RansacOptions {
                threshold: 1.0,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            r.inliers[..12].iter().all(|&b| !b),
            "los atípicos deben descartarse"
        );
        assert!(r.inlier_count >= 36);
        let clean: Vec<_> = pairs[12..].to_vec();
        assert!(r.homography.rms_error(&clean) < 0.3);
        assert!(plain.rms_error(&clean) > r.homography.rms_error(&clean) * 3.0);
        // Determinista.
        let r2 = estimate_homography_ransac(
            &pairs,
            &RansacOptions {
                threshold: 1.0,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.inliers, r2.inliers);
    }

    #[test]
    fn alineacion_imagen_a_cama() {
        // Cama de 300x200 mm vista a 2 px/mm con una leve perspectiva.
        let bed_to_img = Homography {
            m: [[2.0, 0.05, 20.0], [0.02, -2.0, 430.0], [1e-5, 2e-5, 1.0]],
        };
        let bed: Vec<Point> = [
            (10.0, 10.0),
            (290.0, 12.0),
            (288.0, 190.0),
            (12.0, 188.0),
            (150.0, 100.0),
        ]
        .iter()
        .map(|&(x, y)| Point::new(x, y))
        .collect();
        let img: Vec<Point> = bed.iter().map(|&p| bed_to_img.apply(p)).collect();
        let fit = ImageToBed::fit(&img, &bed, None).unwrap();
        assert!(fit.rms_mm < 1e-6);
        let p = fit.alignment.pixel_to_bed(img[4]);
        assert!(p.dist(bed[4]) < 1e-6);
        assert!(fit.alignment.bed_to_pixel(bed[0]).dist(img[0]) < 1e-5);
        assert!(ImageToBed::fit(&img, &bed[..3], None).is_err());
    }
}
