//! Geometría 2D: transformaciones afines y aplanado de curvas con error acotado.

use std::f64::consts::{FRAC_PI_2, TAU};

/// Tope de segmentos por curva, para no agotar memoria con tolerancias absurdas.
const MAX_SEGMENTS: usize = 20_000;

pub type P2 = (f64, f64);

/// Transformación afín: (x, y) -> (a x + c y + e, b x + d y + f).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Affine {
    pub const IDENTITY: Affine = Affine {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    pub fn translate(x: f64, y: f64) -> Self {
        Affine {
            e: x,
            f: y,
            ..Self::IDENTITY
        }
    }

    pub fn scale(sx: f64, sy: f64) -> Self {
        Affine {
            a: sx,
            d: sy,
            ..Self::IDENTITY
        }
    }

    pub fn rotate(radians: f64) -> Self {
        let (s, c) = radians.sin_cos();
        Affine {
            a: c,
            b: s,
            c: -s,
            d: c,
            e: 0.0,
            f: 0.0,
        }
    }

    /// Composición: primero se aplica `inner` y después `self`.
    pub fn mul(&self, inner: &Affine) -> Affine {
        Affine {
            a: self.a * inner.a + self.c * inner.b,
            b: self.b * inner.a + self.d * inner.b,
            c: self.a * inner.c + self.c * inner.d,
            d: self.b * inner.c + self.d * inner.d,
            e: self.a * inner.e + self.c * inner.f + self.e,
            f: self.b * inner.e + self.d * inner.f + self.f,
        }
    }

    pub fn apply(&self, (x, y): P2) -> P2 {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    /// Mayor factor de estiramiento (valor singular máximo de la parte lineal).
    /// Un error `t` antes de transformar es a lo sumo `t * max_scale()` después.
    pub fn max_scale(&self) -> f64 {
        let s = self.a * self.a + self.b * self.b + self.c * self.c + self.d * self.d;
        let det = self.a * self.d - self.b * self.c;
        let disc = (s * s - 4.0 * det * det).max(0.0).sqrt();
        ((s + disc) / 2.0).sqrt()
    }
}

/// Sistema de coordenadas del objeto (OCS) proyectado a XY, según el
/// algoritmo del eje arbitrario. Con normal +Z es la identidad; con -Z
/// invierte X (objetos dibujados en espejo).
pub fn ocs(nx: f64, ny: f64, nz: f64) -> Affine {
    let len = (nx * nx + ny * ny + nz * nz).sqrt();
    if !len.is_finite() || len < 1e-12 {
        return Affine::IDENTITY;
    }
    let n = [nx / len, ny / len, nz / len];
    let reference = if n[0].abs() < 1.0 / 64.0 && n[1].abs() < 1.0 / 64.0 {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    };
    let ax = normalize3(cross(reference, n));
    let ay = cross(n, ax);
    Affine {
        a: ax[0],
        b: ax[1],
        c: ay[0],
        d: ay[1],
        e: 0.0,
        f: 0.0,
    }
}

pub fn cross(u: [f64; 3], v: [f64; 3]) -> [f64; 3] {
    [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ]
}

fn normalize3(v: [f64; 3]) -> [f64; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-12 {
        return [1.0, 0.0, 0.0];
    }
    [v[0] / len, v[1] / len, v[2] / len]
}

/// Paso angular máximo para que la flecha de la cuerda no pase de `tol`:
/// `r (1 - cos(d/2)) <= tol`. Se limita a 90 grados.
fn arc_step(radius: f64, tol: f64) -> f64 {
    if radius <= tol || radius <= 0.0 {
        return FRAC_PI_2;
    }
    (2.0 * (1.0 - tol / radius).acos()).min(FRAC_PI_2)
}

/// Número de segmentos para barrer `sweep` radianes de un arco de radio `radius`.
pub fn arc_segments(radius: f64, sweep: f64, tol: f64) -> usize {
    let n = (sweep.abs() / arc_step(radius, tol)).ceil();
    (n as usize).clamp(1, MAX_SEGMENTS)
}

/// Puntos de un arco (incluye inicio y fin). Todos están sobre la curva.
pub fn arc_points(center: P2, radius: f64, start: f64, sweep: f64, tol: f64, out: &mut Vec<P2>) {
    let n = arc_segments(radius, sweep, tol);
    for i in 0..=n {
        let a = start + sweep * i as f64 / n as f64;
        out.push((center.0 + radius * a.cos(), center.1 + radius * a.sin()));
    }
}

/// Círculo completo sin repetir el primer punto; empieza en el ángulo 0.
pub fn circle_points(center: P2, radius: f64, tol: f64) -> Vec<P2> {
    let n = arc_segments(radius, TAU, tol).max(3);
    (0..n)
        .map(|i| {
            let a = TAU * i as f64 / n as f64;
            (center.0 + radius * a.cos(), center.1 + radius * a.sin())
        })
        .collect()
}

/// Agrega los puntos del tramo `p0 -> p1` con `bulge` (tan de un cuarto del
/// ángulo incluido; positivo = antihorario). No incluye `p0`, sí `p1`.
pub fn bulge_points(p0: P2, p1: P2, bulge: f64, tol: f64, out: &mut Vec<P2>) {
    let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
    let chord = dx.hypot(dy);
    if bulge.abs() < 1e-12 || chord < 1e-12 || !bulge.is_finite() {
        out.push(p1);
        return;
    }
    let half = chord / 2.0;
    let radius = half * (1.0 + bulge * bulge) / (2.0 * bulge.abs());
    // Distancia con signo del punto medio de la cuerda al centro, hacia la izquierda.
    let h = half * (1.0 - bulge * bulge) / (2.0 * bulge);
    let mid = ((p0.0 + p1.0) / 2.0, (p0.1 + p1.1) / 2.0);
    let center = (mid.0 - dy / chord * h, mid.1 + dx / chord * h);
    let sweep = 4.0 * bulge.atan();
    let start = (p0.1 - center.1).atan2(p0.0 - center.0);

    let n = arc_segments(radius, sweep, tol);
    for i in 1..n {
        let a = start + sweep * i as f64 / n as f64;
        out.push((center.0 + radius * a.cos(), center.1 + radius * a.sin()));
    }
    out.push(p1);
}

/// Elipse (completa o parcial) de centro `c`, semieje mayor `major` y semieje
/// menor `minor` (ambos vectores), entre los parámetros `start` y `start + sweep`.
/// Devuelve los puntos; si es completa no repite el primero.
pub fn ellipse_points(c: P2, major: P2, minor: P2, start: f64, sweep: f64, tol: f64) -> Vec<P2> {
    let a_max = major.0.hypot(major.1).max(minor.0.hypot(minor.1));
    // |P''(t)| <= a_max, así que la flecha de una cuerda es <= a_max dt^2 / 8.
    let step = if a_max > 0.0 {
        (8.0 * tol / a_max).sqrt().min(FRAC_PI_2)
    } else {
        FRAC_PI_2
    };
    let full = (sweep - TAU).abs() < 1e-9;
    let n = ((sweep / step).ceil() as usize).clamp(if full { 3 } else { 1 }, MAX_SEGMENTS);
    let count = if full { n } else { n + 1 };
    (0..count)
        .map(|i| {
            let t = start + sweep * i as f64 / n as f64;
            let (s, co) = t.sin_cos();
            (
                c.0 + co * major.0 + s * minor.0,
                c.1 + co * major.1 + s * minor.1,
            )
        })
        .collect()
}

/// B-spline racional de grado `degree` evaluado con de Boor.
pub struct Nurbs<'a> {
    pub degree: usize,
    pub knots: &'a [f64],
    pub points: &'a [P2],
    pub weights: &'a [f64],
}

impl Nurbs<'_> {
    /// Evalúa en `u` usando el polinomio del tramo de nudos `span`
    /// (`knots[span] <= u <= knots[span + 1]`, con `degree <= span < n`).
    fn eval(&self, span: usize, u: f64) -> P2 {
        let p = self.degree;
        let mut d = [[0.0f64; 3]; 16];
        for (j, slot) in d.iter_mut().enumerate().take(p + 1) {
            let i = j + span - p;
            let w = self.weights[i];
            *slot = [self.points[i].0 * w, self.points[i].1 * w, w];
        }
        for r in 1..=p {
            for j in (r..=p).rev() {
                let left = self.knots[j + span - p];
                let denom = self.knots[j + 1 + span - r] - left;
                let alpha = if denom.abs() < 1e-14 {
                    0.0
                } else {
                    (u - left) / denom
                };
                let prev = d[j - 1];
                for (cur, prev) in d[j].iter_mut().zip(prev) {
                    *cur = (1.0 - alpha) * prev + alpha * *cur;
                }
            }
        }
        let r = d[p];
        if r[2].abs() < 1e-14 {
            (r[0], r[1])
        } else {
            (r[0] / r[2], r[1] / r[2])
        }
    }

    /// Aplana toda la curva (de `knots[degree]` a `knots[n]`) con error
    /// máximo `tol`, comprobado por muestreo en 1/4, 1/2 y 3/4 de cada
    /// subtramo. Todos los puntos están sobre la curva.
    pub fn flatten(&self, tol: f64) -> Vec<P2> {
        let n = self.points.len();
        let mut out = Vec::new();
        for span in self.degree..n {
            let (u0, u1) = (self.knots[span], self.knots[span + 1]);
            if u1 - u0 < 1e-12 {
                continue;
            }
            let p0 = self.eval(span, u0);
            if out.is_empty() {
                out.push(p0);
            }
            let p1 = self.eval(span, u1);
            self.subdivide(span, (u0, p0), (u1, p1), tol, 0, &mut out);
        }
        out
    }

    fn subdivide(
        &self,
        span: usize,
        (u0, p0): (f64, P2),
        (u1, p1): (f64, P2),
        tol: f64,
        depth: u32,
        out: &mut Vec<P2>,
    ) {
        let mid = 0.5 * (u0 + u1);
        let pm = self.eval(span, mid);
        let flat = depth >= 14 || out.len() >= MAX_SEGMENTS || {
            let quarters = [0.25, 0.75].map(|f| self.eval(span, u0 + (u1 - u0) * f));
            dist_to_segment(pm, p0, p1) <= tol
                && quarters.iter().all(|&q| dist_to_segment(q, p0, p1) <= tol)
        };
        if flat {
            out.push(p1);
        } else {
            self.subdivide(span, (u0, p0), (mid, pm), tol, depth + 1, out);
            self.subdivide(span, (mid, pm), (u1, p1), tol, depth + 1, out);
        }
    }
}

/// Vector de nudos con extremos repetidos (`degree + 1` veces) y resto uniforme.
pub fn clamped_uniform_knots(n_points: usize, degree: usize) -> Vec<f64> {
    let inner = n_points - degree; // número de tramos
    let mut knots = vec![0.0; degree + 1];
    for i in 1..inner {
        knots.push(i as f64);
    }
    knots.extend(std::iter::repeat_n(inner as f64, degree + 1));
    knots
}

pub fn dist_to_segment(p: P2, a: P2, b: P2) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 < 1e-24 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    };
    (p.0 - (a.0 + t * dx)).hypot(p.1 - (a.1 + t * dy))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn close(a: P2, b: P2) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
    }

    #[test]
    fn affine_composition_order() {
        let m = Affine::translate(10.0, 0.0).mul(&Affine::rotate(PI / 2.0));
        assert!(close(m.apply((1.0, 0.0)), (10.0, 1.0)));
    }

    #[test]
    fn max_scale_of_known_matrices() {
        assert!((Affine::scale(2.0, 3.0).max_scale() - 3.0).abs() < 1e-12);
        assert!((Affine::rotate(1.0).max_scale() - 1.0).abs() < 1e-12);
        assert!((Affine::scale(-4.0, 1.0).max_scale() - 4.0).abs() < 1e-12);
        assert_eq!(Affine::scale(0.0, 0.0).max_scale(), 0.0);
    }

    #[test]
    fn ocs_flips_x_for_negative_normal() {
        let m = ocs(0.0, 0.0, -1.0);
        assert!(close(m.apply((3.0, 2.0)), (-3.0, 2.0)));
        assert_eq!(ocs(0.0, 0.0, 1.0), Affine::IDENTITY);
    }

    #[test]
    fn arc_segment_count_respects_tolerance() {
        for (r, tol) in [(10.0, 0.01), (0.5, 0.05), (1000.0, 0.001), (3.0, 5.0)] {
            let n = arc_segments(r, TAU, tol);
            let step = TAU / n as f64;
            let sagitta = r * (1.0 - (step / 2.0).cos());
            assert!(sagitta <= tol + 1e-12 || tol >= r, "r={r} tol={tol} n={n}");
        }
    }

    #[test]
    fn clamped_knots_shape() {
        assert_eq!(
            clamped_uniform_knots(4, 2),
            vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0]
        );
    }
}
