//! Corrección por altura del material.
//!
//! Con la imagen ya sin distorsión, un plano horizontal a altura `h` sobre la cama se
//! proyecta con `x ~ K [r1 r2 (t + h·r3)] [X Y 1]ᵀ`. Por tanto la homografía cama a
//! imagen de ese plano es `H(h) = [H₁ H₂ (H₃ + h·G)]`: las dos primeras columnas no
//! dependen de la altura y la tercera es lineal en `h`. Con marcas medidas a dos (o más)
//! alturas, `H₁, H₂, H₃` y `G` se estiman juntas con una DLT conjunta (sin conocer `K`).
//! Con una sola altura se puede estimar `G` desde la matriz de la cámara (respaldo menos
//! fiable, ver [`HeightModel::from_intrinsics`]).
//!
//! Convención: todas las matrices de este módulo son cama (mm) a imagen (px); la inversa
//! se calcula al evaluar. La escala de las matrices es común a todas las alturas, por eso
//! no se normaliza (normalizar cada una por separado rompería la linealidad).

use crate::homography::Homography;
use nalgebra::{DMatrix, Matrix3, Vector3};
use serde::{Deserialize, Serialize};
use tlaku_core::Point;

/// Diferencia mínima entre la altura más baja y la más alta para estimar el modelo (mm).
pub const MIN_HEIGHT_SPAN_MM: f64 = 2.0;

/// Marcas vistas a una altura: píxeles de la imagen corregida y su posición en la cama.
#[derive(Debug, Clone, PartialEq)]
pub struct HeightSample {
    /// Altura del plano de las marcas sobre la cama, en mm.
    pub height_mm: f64,
    pub image_px: Vec<Point>,
    pub bed_mm: Vec<Point>,
}

/// De dónde sale el modelo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeightSource {
    /// Alineación medida a dos o más alturas (exacta en teoría, no necesita `K`).
    Measured,
    /// Una sola alineación y la matriz de la cámara de la calibración (aproximada).
    Modelled,
}

/// `H(h) = [H₁ H₂ (H₃ + (h − ref)·G)]`, de la cama a la imagen.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HeightModel {
    /// Altura (mm) a la que `bed_to_image` es exacta.
    pub ref_height_mm: f64,
    /// Cama a imagen en `ref_height_mm`, por filas (sin normalizar la escala).
    pub bed_to_image: [[f64; 3]; 3],
    /// Cuánto cambia la tercera columna por cada mm de altura.
    pub gradient: [f64; 3],
    pub source: HeightSource,
    /// Rango de alturas en que se midió (con `Modelled`, solo `ref_height_mm`).
    pub min_height_mm: f64,
    pub max_height_mm: f64,
}

/// Resultado del ajuste conjunto.
#[derive(Debug, Clone)]
pub struct HeightFit {
    pub model: HeightModel,
    /// Error de transferencia (rms, mm de cama) de cada muestra con el modelo conjunto.
    pub sample_rms_mm: Vec<f64>,
}

impl HeightModel {
    /// Matriz cama a imagen del plano a altura `h`.
    pub fn bed_to_image_at(&self, h: f64) -> Homography {
        let mut m = self.bed_to_image;
        let dh = h - self.ref_height_mm;
        for (r, row) in m.iter_mut().enumerate() {
            row[2] += dh * self.gradient[r];
        }
        Homography { m }
    }

    /// Imagen a cama del plano a altura `h`, normalizada para guardarla o dibujar.
    pub fn image_to_bed_at(&self, h: f64) -> Result<Homography, String> {
        self.bed_to_image_at(h).inverse()
    }

    pub fn pixel_to_bed(&self, p: Point, h: f64) -> Result<Point, String> {
        Ok(self.image_to_bed_at(h)?.apply(p))
    }

    pub fn bed_to_pixel(&self, p: Point, h: f64) -> Point {
        self.bed_to_image_at(h).apply(p)
    }

    /// Cuántos mm queda `h` fuera del rango medido (0 si está dentro).
    pub fn extrapolation_mm(&self, h: f64) -> f64 {
        if h < self.min_height_mm {
            self.min_height_mm - h
        } else if h > self.max_height_mm {
            h - self.max_height_mm
        } else {
            0.0
        }
    }

    /// Modelo de respaldo con una sola alineación: la tercera columna cambia con la altura
    /// según la pose de la cámara, que se obtiene de `bed_to_image` y de la matriz de la
    /// imagen corregida `k = [fx, fy, cx, cy]`. Depende de que la focal sea buena y de que la
    /// cama sea un plano; con ruido en la alineación, la dirección vertical sale aproximada.
    pub fn from_intrinsics(
        bed_to_image: &Homography,
        ref_height_mm: f64,
        k: [f64; 4],
    ) -> Result<Self, String> {
        let [fx, fy, cx, cy] = k;
        if !(fx > 0.0 && fy > 0.0 && k.iter().all(|v| v.is_finite())) {
            return Err("la matriz de la cámara no es válida".into());
        }
        let kmat = Matrix3::new(fx, 0.0, cx, 0.0, fy, cy, 0.0, 0.0, 1.0);
        let kinv = kmat
            .try_inverse()
            .ok_or("la matriz de la cámara es singular")?;
        let p = bed_to_image.matrix();
        let a1: Vector3<f64> = kinv * p.column(0);
        let a2: Vector3<f64> = kinv * p.column(1);
        let a3: Vector3<f64> = kinv * p.column(2);
        // El signo global de una homografía es arbitrario: se elige el que deja la cama
        // delante de la cámara (z > 0).
        let sign = if a3.z < 0.0 { -1.0 } else { 1.0 };
        let (a1, a2) = (a1 * sign, a2 * sign);
        let mu = (a1.norm() + a2.norm()) / 2.0;
        if !mu.is_finite() || mu < 1e-12 {
            return Err("la alineación es degenerada para estimar la altura".into());
        }
        let r1 = a1.normalize();
        // Gram-Schmidt: r2 ortogonal a r1 (la alineación con ruido no es exactamente ortonormal).
        let r2 = (a2 - r1 * r1.dot(&a2))
            .try_normalize(1e-12)
            .ok_or("la alineación es degenerada")?;
        let mut r3 = r1.cross(&r2);
        // Subir el material lo acerca a la cámara (z de la cámara disminuye).
        if r3.z > 0.0 {
            r3 = -r3;
        }
        let g = kmat * r3 * (mu * sign);
        Ok(Self {
            ref_height_mm,
            bed_to_image: bed_to_image.m,
            gradient: [g.x, g.y, g.z],
            source: HeightSource::Modelled,
            min_height_mm: ref_height_mm,
            max_height_mm: ref_height_mm,
        })
    }
}

/// Transformación de similitud 2D `x' = s (x - c)`.
struct Similarity {
    c: Point,
    s: f64,
}

fn similarity(pts: &[Point]) -> Result<Similarity, String> {
    let n = pts.len() as f64;
    let cx = pts.iter().map(|p| p.x).sum::<f64>() / n;
    let cy = pts.iter().map(|p| p.y).sum::<f64>() / n;
    let mean = pts.iter().map(|p| (p.x - cx).hypot(p.y - cy)).sum::<f64>() / n;
    if !mean.is_finite() || mean < 1e-12 {
        return Err("los puntos de las marcas son degenerados".into());
    }
    Ok(Similarity {
        c: Point::new(cx, cy),
        s: std::f64::consts::SQRT_2 / mean,
    })
}

impl Similarity {
    fn apply(&self, p: Point) -> (f64, f64) {
        (self.s * (p.x - self.c.x), self.s * (p.y - self.c.y))
    }
    fn matrix(&self) -> Matrix3<f64> {
        Matrix3::new(
            self.s,
            0.0,
            -self.s * self.c.x,
            0.0,
            self.s,
            -self.s * self.c.y,
            0.0,
            0.0,
            1.0,
        )
    }
}

/// Ajuste conjunto de `[H₁ H₂ H₃ G]` con marcas a varias alturas (DLT normalizada).
///
/// Las incógnitas son las tres primeras columnas de la matriz a la altura más baja y la
/// derivada `G`; cada correspondencia aporta dos ecuaciones lineales. Se normalizan los
/// puntos de la imagen y de la cama con similitudes (la de la cama conserva la estructura
/// `[c1 c2 c3 + h·g]`: solo cambia `c3`) y la altura se escala a `[0, 1]`.
pub fn fit_height_model(samples: &[HeightSample]) -> Result<HeightFit, String> {
    if samples.len() < 2 {
        return Err("hacen falta marcas a dos alturas distintas para el modelo de altura".into());
    }
    for s in samples {
        if s.image_px.len() != s.bed_mm.len() {
            return Err("cada altura necesita tantos puntos de imagen como de cama".into());
        }
        if s.image_px.len() < 4 {
            return Err(format!(
                "la altura de {:.1} mm tiene menos de 4 marcas",
                s.height_mm
            ));
        }
        let finite = s
            .image_px
            .iter()
            .chain(&s.bed_mm)
            .all(|p| p.x.is_finite() && p.y.is_finite());
        if !finite || !s.height_mm.is_finite() {
            return Err("hay valores no finitos en las marcas".into());
        }
    }
    let h_min = samples
        .iter()
        .map(|s| s.height_mm)
        .fold(f64::INFINITY, f64::min);
    let h_max = samples
        .iter()
        .map(|s| s.height_mm)
        .fold(f64::NEG_INFINITY, f64::max);
    let span = h_max - h_min;
    if span < MIN_HEIGHT_SPAN_MM {
        return Err(format!(
            "las alturas deben diferir al menos {MIN_HEIGHT_SPAN_MM} mm (difieren {span:.1} mm)"
        ));
    }

    let all_img: Vec<Point> = samples
        .iter()
        .flat_map(|s| s.image_px.iter().copied())
        .collect();
    let all_bed: Vec<Point> = samples
        .iter()
        .flat_map(|s| s.bed_mm.iter().copied())
        .collect();
    let ti = similarity(&all_img)?;
    let tb = similarity(&all_bed)?;

    let n: usize = all_img.len();
    let rows = (2 * n).max(12);
    let mut a = DMatrix::<f64>::zeros(rows, 12);
    let mut row = 0;
    for s in samples {
        let hh = (s.height_mm - h_min) / span;
        for (pi, pb) in s.image_px.iter().zip(&s.bed_mm) {
            let (u, v) = ti.apply(*pi);
            let (x, y) = tb.apply(*pb);
            // p = c1·x + c2·y + c3 + hh·g, con cada columna de 3 componentes.
            let basis = [x, y, 1.0, hh];
            for (m, &b) in basis.iter().enumerate() {
                a[(row, 3 * m)] = -b;
                a[(row, 3 * m + 2)] = u * b;
                a[(row + 1, 3 * m + 1)] = -b;
                a[(row + 1, 3 * m + 2)] = v * b;
            }
            row += 2;
        }
    }
    let svd = a.svd(false, true);
    let v_t = svd
        .v_t
        .ok_or("falló la descomposición SVD del modelo de altura")?;
    let sv = &svd.singular_values;
    let mut order: Vec<usize> = (0..sv.len()).collect();
    order.sort_by(|&i, &j| sv[j].total_cmp(&sv[i]));
    let smallest = order[order.len() - 1];
    let second = order[order.len() - 2];
    if sv[second] <= 1e-9 * sv[order[0]].max(1e-300) {
        return Err(
            "las marcas son degeneradas para el modelo de altura (colineales o repetidas)".into(),
        );
    }
    let theta: Vec<f64> = (0..12).map(|i| v_t[(smallest, i)]).collect();
    let col = |m: usize| Vector3::new(theta[3 * m], theta[3 * m + 1], theta[3 * m + 2]);
    let n0 = Matrix3::from_columns(&[col(0), col(1), col(2)]);
    let gn = col(3);
    let ti_inv = ti
        .matrix()
        .try_inverse()
        .ok_or("normalización de la imagen no invertible")?;
    let m0 = ti_inv * n0 * tb.matrix();
    let g = ti_inv * gn / span;
    if !m0.iter().chain(g.iter()).all(|v| v.is_finite()) {
        return Err("el modelo de altura no es finito".into());
    }
    // Escala: tercera componente de la tercera columna a +1 si es posible (solo estética;
    // el signo global es arbitrario y así el modelo tiene w > 0 cerca de la cama).
    let k = if m0[(2, 2)].abs() > 1e-12 {
        1.0 / m0[(2, 2)]
    } else {
        1.0
    };
    let m0 = m0 * k;
    let g = g * k;
    let model = HeightModel {
        ref_height_mm: h_min,
        bed_to_image: Homography::from_matrix(&m0).m,
        gradient: [g.x, g.y, g.z],
        source: HeightSource::Measured,
        min_height_mm: h_min,
        max_height_mm: h_max,
    };
    let sample_rms_mm = samples
        .iter()
        .map(|s| {
            let inv = model.image_to_bed_at(s.height_mm);
            match inv {
                Ok(h) => {
                    let sum: f64 = s
                        .image_px
                        .iter()
                        .zip(&s.bed_mm)
                        .map(|(p, b)| {
                            let q = h.apply(*p);
                            (q.x - b.x).powi(2) + (q.y - b.y).powi(2)
                        })
                        .sum();
                    (sum / s.image_px.len() as f64).sqrt()
                }
                Err(_) => f64::INFINITY,
            }
        })
        .collect();
    Ok(HeightFit {
        model,
        sample_rms_mm,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cámara de prueba mirando hacia abajo, inclinada, con la cama (Y arriba) vista sin espejo.
    /// Devuelve la matriz cama a imagen a altura `h` (`K [r1 r2 t + h r3]`).
    pub(crate) fn truth(h: f64, k: [f64; 4]) -> Matrix3<f64> {
        let d = std::f64::consts::PI / 180.0;
        let rot = nalgebra::Rotation3::from_euler_angles(12.0 * d, -8.0 * d, 5.0 * d);
        // Cama: X a la derecha, Y arriba (en pantalla), Z hacia la cámara.
        let flip = Matrix3::new(1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, -1.0);
        let r = rot.matrix() * flip;
        let t = Vector3::new(-140.0, 95.0, 380.0);
        let kmat = Matrix3::new(k[0], 0.0, k[2], 0.0, k[1], k[3], 0.0, 0.0, 1.0);
        let cols = [
            r.column(0).into_owned(),
            r.column(1).into_owned(),
            t + r.column(2) * h,
        ];
        kmat * Matrix3::from_columns(&cols)
    }

    const K: [f64; 4] = [620.0, 625.0, 322.0, 241.0];

    fn bed_grid() -> Vec<Point> {
        let mut v = Vec::new();
        for j in 0..3 {
            for i in 0..3 {
                v.push(Point::new(25.0 + 125.0 * i as f64, 20.0 + 80.0 * j as f64));
            }
        }
        v
    }

    fn project(h: f64, p: Point) -> Point {
        Homography::from_matrix(&truth(h, K)).apply(p)
    }

    fn sample(h: f64, noise: f64, seed: u64) -> HeightSample {
        let mut s = seed | 1;
        let mut rnd = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            ((s >> 11) as f64) / ((1u64 << 53) as f64) - 0.5
        };
        let bed = bed_grid();
        let image = bed
            .iter()
            .map(|&p| {
                let q = project(h, p);
                Point::new(q.x + rnd() * noise, q.y + rnd() * noise)
            })
            .collect();
        HeightSample {
            height_mm: h,
            image_px: image,
            bed_mm: bed,
        }
    }

    /// Error máximo (mm) del modelo frente a la verdad en una rejilla de la cama.
    fn max_error(model: &HeightModel, h: f64) -> f64 {
        let truth_inv = Homography::from_matrix(&truth(h, K)).inverse().unwrap();
        let mut worst: f64 = 0.0;
        for j in 0..=8 {
            for i in 0..=12 {
                let bed = Point::new(i as f64 * 25.0, j as f64 * 25.0);
                let px = project(h, bed);
                let est = model.pixel_to_bed(px, h).unwrap();
                worst = worst.max(est.dist(bed));
                worst = worst.max(truth_inv.apply(px).dist(bed));
            }
        }
        worst
    }

    #[test]
    fn el_modelo_exacto_recupera_las_alturas_intermedias() {
        let fit = fit_height_model(&[sample(0.0, 0.0, 1), sample(20.0, 0.0, 2)]).unwrap();
        for h in [0.0, 3.0, 10.0, 20.0, 27.0, -2.0] {
            assert!(max_error(&fit.model, h) < 1e-6, "h {h}");
        }
        assert!(fit.sample_rms_mm.iter().all(|&e| e < 1e-6));
        assert_eq!(fit.model.source, HeightSource::Measured);
        assert_eq!(fit.model.extrapolation_mm(10.0), 0.0);
        assert!((fit.model.extrapolation_mm(27.0) - 7.0).abs() < 1e-12);
    }

    #[test]
    fn con_ruido_de_medio_pixel_el_error_es_submilimetrico() {
        // Ruido uniforme de ±0.25 px (0.15 px rms) en cada marca.
        let fit = fit_height_model(&[sample(0.0, 0.5, 3), sample(20.0, 0.5, 4)]).unwrap();
        let in_range = [0.0, 5.0, 10.0, 15.0, 20.0]
            .iter()
            .map(|&h| max_error(&fit.model, h))
            .fold(0.0, f64::max);
        assert!(in_range < 0.5, "error dentro del rango {in_range:.3} mm");
        let extrap = max_error(&fit.model, 25.0);
        assert!(extrap < 1.0, "error extrapolando 5 mm: {extrap:.3} mm");
    }

    #[test]
    fn sin_correccion_el_error_por_altura_es_de_varios_mm() {
        // Referencia: con la alineación de la cama usada a 20 mm el error sería grande.
        let at0 = Homography::from_matrix(&truth(0.0, K)).inverse().unwrap();
        let bed = Point::new(290.0, 190.0);
        let px = project(20.0, bed);
        let naive = at0.apply(px).dist(bed);
        assert!(naive > 3.0, "sin corregir: {naive:.2} mm");
    }

    #[test]
    fn tres_alturas_y_alturas_desordenadas() {
        let fit = fit_height_model(&[
            sample(20.0, 0.0, 7),
            sample(0.0, 0.0, 8),
            sample(8.0, 0.0, 9),
        ])
        .unwrap();
        assert!((fit.model.ref_height_mm - 0.0).abs() < 1e-12);
        assert!((fit.model.max_height_mm - 20.0).abs() < 1e-12);
        assert!(max_error(&fit.model, 14.0) < 1e-6);
    }

    #[test]
    fn el_respaldo_con_la_matriz_de_la_camara_es_exacto_con_k_buena() {
        let h0 = Homography::from_matrix(&truth(0.0, K));
        let model = HeightModel::from_intrinsics(&h0, 0.0, K).unwrap();
        assert_eq!(model.source, HeightSource::Modelled);
        for h in [5.0, 10.0, 20.0] {
            assert!(
                max_error(&model, h) < 1e-6,
                "h {h}: {}",
                max_error(&model, h)
            );
        }
    }

    #[test]
    fn el_respaldo_con_k_imprecisa_mejora_mucho_sin_corregir() {
        // Focal 8 % mayor y punto principal desplazado: sigue acercando bastante.
        let h0 = Homography::from_matrix(&truth(0.0, K));
        let bad = [K[0] * 1.08, K[1] * 1.08, K[2] + 6.0, K[3] - 5.0];
        let model = HeightModel::from_intrinsics(&h0, 0.0, bad).unwrap();
        let naive = {
            let bed = Point::new(290.0, 190.0);
            h0.inverse().unwrap().apply(project(20.0, bed)).dist(bed)
        };
        let corrected = max_error(&model, 20.0);
        assert!(corrected < naive, "{corrected:.2} vs {naive:.2}");
    }

    #[test]
    fn valida_las_entradas() {
        assert!(fit_height_model(&[sample(0.0, 0.0, 1)]).is_err());
        let e = fit_height_model(&[sample(0.0, 0.0, 1), sample(1.0, 0.0, 2)]).unwrap_err();
        assert!(e.contains("difieren"), "{e}");
        let mut few = sample(10.0, 0.0, 3);
        few.image_px.truncate(3);
        few.bed_mm.truncate(3);
        assert!(fit_height_model(&[sample(0.0, 0.0, 1), few]).is_err());
        let mut nan = sample(10.0, 0.0, 3);
        nan.image_px[0] = Point::new(f64::NAN, 0.0);
        assert!(fit_height_model(&[sample(0.0, 0.0, 1), nan]).is_err());
    }
}
