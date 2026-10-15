//! Geometría del perfil de evolvente: un engranaje (exterior o el hueco de una corona) y la
//! cremallera, como contornos cerrados de puntos.
//!
//! Matemática pública del engranaje de evolvente; implementación propia. Convenciones:
//!
//! - `r = m·z/2` es el radio primitivo, `rb = r·cos α` el radio base y `inv φ = tan φ − φ` la
//!   función evolvente. El flanco es la evolvente del círculo base: a un radio `R ≥ rb` el
//!   semiángulo del diente (centrado en el ángulo 0) es
//!   `h(R) = s/(2r) + inv α − inv φ(R)` con `cos φ(R) = rb/R` y `s` el espesor del diente medido
//!   sobre el arco del círculo primitivo. Por debajo del círculo base el flanco sigue radial.
//! - Un diente va de `-h` (flanco derecho) a `+h` (flanco izquierdo), sentido antihorario.
//! - El flanco se muestrea en el parámetro de rodadura `t = √(R²−rb²)/rb` (en `t`, la evolvente es
//!   `rb·(cos t + t sin t, sin t − t cos t)` girada) con un paso tal que la flecha de la cuerda
//!   no pase de `tolerance` (la curvatura de la evolvente es `rb·t`).
//! - Pie con filete: un arco tangente al círculo de pie y al flanco, de radio a lo más
//!   `fillet_max`, que además no baja de la holgura que la punta del engranaje conjugado deja
//!   libre (`rf + clearance`), para no estorbarle.

use std::f64::consts::{PI, TAU};
use tlaku_core::Point;

/// Por qué no se pudo construir el perfil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileError {
    /// Los dientes acaban en punta (semiángulo en la cabeza ≤ 0): pocos dientes o ángulo grande.
    PointedTip,
    /// El diente es tan ancho que se toca con el vecino por debajo del círculo de pie.
    Overlap,
    /// Dimensiones imposibles (radios que no cuadran).
    Invalid,
}

/// Perfil de un engranaje de `teeth` dientes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Profile {
    pub teeth: u32,
    pub module: f64,
    /// Ángulo de presión en radianes.
    pub alpha: f64,
    /// Radio exterior (de cabeza) del diente.
    pub tip_radius: f64,
    /// Radio del fondo entre dientes.
    pub root_radius: f64,
    /// Espesor del diente sobre el arco del círculo primitivo.
    pub thickness: f64,
    /// Radio de filete máximo en el pie (0 = pie simple).
    pub fillet_max: f64,
    /// Holgura de cabeza: lo que la punta del conjugado deja libre sobre el fondo.
    pub clearance: f64,
    /// Flecha máxima de la aproximación (mm).
    pub tolerance: f64,
}

/// `inv φ = tan φ − φ`.
pub fn inv(phi: f64) -> f64 {
    phi.tan() - phi
}

fn rot(p: Point, a: f64) -> Point {
    let (s, c) = a.sin_cos();
    Point::new(p.x * c - p.y * s, p.x * s + p.y * c)
}

fn polar(r: f64, a: f64) -> Point {
    Point::new(r * a.cos(), r * a.sin())
}

/// Puntos de un arco de radio `r` centrado en `c` de `a0` a `a1` (ambos incluidos), con una
/// flecha de cuerda de a lo más `tol`.
pub fn arc(c: Point, r: f64, a0: f64, a1: f64, tol: f64) -> Vec<Point> {
    let step = if tol >= r {
        PI
    } else {
        (2.0 * (1.0 - tol / r).acos()).clamp(1e-3, PI)
    };
    let n = (((a1 - a0).abs() / step).ceil() as usize).max(1);
    (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f64 / n as f64;
            Point::new(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect()
}

/// Círculo de radio `r` centrado en `c` como polígono cerrado (sin repetir el primer punto).
pub fn circle(c: Point, r: f64, tol: f64) -> Vec<Point> {
    let mut pts = arc(c, r, 0.0, TAU, tol);
    pts.pop();
    pts
}

impl Profile {
    pub fn pitch_radius(&self) -> f64 {
        self.module * f64::from(self.teeth) / 2.0
    }

    pub fn base_radius(&self) -> f64 {
        self.pitch_radius() * self.alpha.cos()
    }

    /// Semiángulo del diente al radio `radius`.
    pub fn half_angle(&self, radius: f64) -> f64 {
        let rb = self.base_radius();
        let gamma = self.thickness / (2.0 * self.pitch_radius());
        let big_r = radius.max(rb);
        let phi = (rb / big_r).clamp(-1.0, 1.0).acos();
        gamma + inv(self.alpha) - inv(phi)
    }

    /// Punto del flanco derecho al radio `radius` (por debajo del círculo base, sobre el radial).
    pub fn flank_point(&self, radius: f64) -> Point {
        polar(radius, -self.half_angle(radius))
    }

    /// Punto del flanco derecho para el parámetro de rodadura `t ≥ 0`.
    fn flank_at_t(&self, t: f64) -> Point {
        let rb = self.base_radius();
        let gamma = self.thickness / (2.0 * self.pitch_radius());
        let angle = -(gamma + inv(self.alpha) - (t - t.atan()));
        polar(rb * (1.0 + t * t).sqrt(), angle)
    }

    fn t_of(&self, radius: f64) -> f64 {
        let rb = self.base_radius();
        ((radius * radius - rb * rb).max(0.0)).sqrt() / rb
    }

    /// Flanco derecho desde el radio `start` hasta la cabeza, muestreado a la tolerancia.
    fn flank(&self, start: f64) -> Vec<Point> {
        let rb = self.base_radius();
        let t_end = self.t_of(self.tip_radius);
        let t_start = if start > rb { self.t_of(start) } else { 0.0 };
        let mut pts = Vec::new();
        if start < rb {
            pts.push(self.flank_point(start));
        }
        let span = t_end - t_start;
        // flecha ≈ rb·t·dt²/8 (en el extremo de mayor curvatura) ≤ tolerancia
        let dt = (8.0 * self.tolerance / (rb * t_end.max(1e-6))).sqrt();
        let n = ((span / dt).ceil() as usize).max(3);
        for i in 0..=n {
            let t = t_start + span * i as f64 / n as f64;
            pts.push(self.flank_at_t(t));
        }
        // El último punto es exactamente la cabeza.
        if let Some(last) = pts.last_mut() {
            *last = self.flank_point(self.tip_radius);
        }
        pts
    }

    /// Centro del filete de radio `rho` tangente al flanco derecho y al círculo de pie, y el
    /// radio `R*` en que toca el flanco. `None` si no existe.
    fn fillet_center(&self, rho: f64) -> Option<(Point, f64)> {
        let rf = self.root_radius;
        let normal = |radius: f64| {
            let e = 1e-6 * radius;
            let a = self.flank_point(radius - e);
            let b = self.flank_point(radius + e);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let l = dx.hypot(dy);
            // El hueco queda a la derecha del flanco recorrido hacia afuera.
            (dy / l, -dx / l)
        };
        let center = |radius: f64| {
            let p = self.flank_point(radius);
            let n = normal(radius);
            Point::new(p.x + rho * n.0, p.y + rho * n.1)
        };
        let g = |radius: f64| {
            let c = center(radius);
            c.x.hypot(c.y) - (rf + rho)
        };
        let (mut lo, mut hi) = (rf, self.tip_radius);
        if g(lo) >= 0.0 || g(hi) <= 0.0 {
            return None;
        }
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if g(mid) < 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let r_star = 0.5 * (lo + hi);
        Some((center(r_star), r_star))
    }

    /// El filete más grande (hasta `fillet_max`) que cabe: tangente, sin subir de `rf + clearance`
    /// y sin tocar el filete del diente vecino. Devuelve sus puntos del círculo de pie al flanco.
    fn fillet(&self) -> Option<(Vec<Point>, f64)> {
        if self.fillet_max <= 1e-6 || self.clearance <= 1e-6 {
            return None;
        }
        let limit = self.root_radius + self.clearance * 0.95;
        let half_pitch = PI / f64::from(self.teeth);
        let feasible = |rho: f64| -> Option<(Point, f64)> {
            let (c, r_star) = self.fillet_center(rho)?;
            let ok = r_star <= limit && (-c.y.atan2(c.x)).abs() < half_pitch * 0.999;
            ok.then_some((c, r_star))
        };
        let mut best = None;
        if let Some(found) = feasible(self.fillet_max) {
            best = Some((self.fillet_max, found));
        } else {
            let (mut lo, mut hi) = (0.0, self.fillet_max);
            for _ in 0..30 {
                let mid = 0.5 * (lo + hi);
                if let Some(found) = feasible(mid) {
                    best = Some((mid, found));
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
        }
        let (rho, (c, r_star)) = best?;
        if rho < 0.02 * self.module {
            return None;
        }
        let t_root = {
            let k = self.root_radius / c.x.hypot(c.y);
            Point::new(c.x * k, c.y * k)
        };
        let t_flank = self.flank_point(r_star);
        let a0 = (t_root.y - c.y).atan2(t_root.x - c.x);
        let mut a1 = (t_flank.y - c.y).atan2(t_flank.x - c.x);
        // Por el camino corto.
        while a1 - a0 > PI {
            a1 -= TAU;
        }
        while a1 - a0 < -PI {
            a1 += TAU;
        }
        let mut pts = arc(c, rho, a0, a1, self.tolerance);
        if let Some(first) = pts.first_mut() {
            *first = t_root;
        }
        if let Some(last) = pts.last_mut() {
            *last = t_flank;
        }
        Some((pts, r_star))
    }

    /// Un diente centrado en el ángulo 0, de la raíz derecha a la raíz izquierda.
    fn tooth(&self) -> Result<Vec<Point>, ProfileError> {
        let (rb, ra, rf) = (self.base_radius(), self.tip_radius, self.root_radius);
        if !(rb > 0.0 && ra > rf && ra > rb && rf > 0.0 && self.thickness > 0.0) {
            return Err(ProfileError::Invalid);
        }
        let h_tip = self.half_angle(ra);
        if h_tip <= 1e-4 {
            return Err(ProfileError::PointedTip);
        }
        let mut right = Vec::new();
        match self.fillet() {
            Some((arc_pts, r_star)) => {
                right.extend(arc_pts);
                // El primer punto del flanco ya es el final del filete.
                right.extend(self.flank(r_star).into_iter().skip(1));
            }
            None => right.extend(self.flank(rf)),
        }
        let mut tooth = right.clone();
        // Arco de la cabeza (sin los extremos, que ya son los del flanco).
        let tip = arc(Point::new(0.0, 0.0), ra, -h_tip, h_tip, self.tolerance);
        if tip.len() > 2 {
            tooth.extend(tip[1..tip.len() - 1].iter().copied());
        }
        tooth.extend(right.iter().rev().map(|p| Point::new(p.x, -p.y)));
        Ok(tooth)
    }

    /// Contorno cerrado (antihorario, sin repetir el primer punto) con el diente 0 centrado en
    /// el ángulo `rotation` (radianes).
    pub fn outline(&self, rotation: f64) -> Result<Vec<Point>, ProfileError> {
        let tooth = self.tooth()?;
        let pitch = TAU / f64::from(self.teeth);
        let first = tooth[0];
        let last = tooth[tooth.len() - 1];
        // Ángulos (positivos) de la raíz izquierda del diente y de la raíz derecha del siguiente.
        let a_start = last.y.atan2(last.x);
        let a_end = pitch + first.y.atan2(first.x);
        let span = a_end - a_start;
        if span < -1e-9 {
            return Err(ProfileError::Overlap);
        }
        let root = if span > 1e-7 {
            let pts = arc(
                Point::new(0.0, 0.0),
                self.root_radius,
                a_start,
                a_end,
                self.tolerance,
            );
            pts[1..pts.len() - 1].to_vec()
        } else {
            Vec::new()
        };
        let mut out = Vec::with_capacity((tooth.len() + root.len()) * self.teeth as usize);
        for k in 0..self.teeth {
            let a = rotation + pitch * f64::from(k);
            out.extend(tooth.iter().map(|p| rot(*p, a)));
            out.extend(root.iter().map(|p| rot(*p, a)));
        }
        out.dedup_by(|a, b| a.dist(*b) < 1e-9);
        if out.len() > 1 && out[0].dist(out[out.len() - 1]) < 1e-9 {
            out.pop();
        }
        Ok(out)
    }
}

/// Cremallera: contorno cerrado (antihorario) de `teeth` dientes de paso `π·m`, con la línea
/// primitiva en `y = 0`, los dientes hacia `+y` y el diente `i` centrado en `x = i·π·m`. El
/// cuerpo baja `base` bajo el fondo. Error si los dientes se tocan en el fondo o se afilan.
pub fn rack_outline(
    teeth: u32,
    module: f64,
    alpha: f64,
    clearance: f64,
    thickness: f64,
    base: f64,
) -> Result<Vec<Point>, ProfileError> {
    let pitch = PI * module;
    let (add, ded) = (module, module + clearance);
    let tan = alpha.tan();
    let half_tip = thickness / 2.0 - add * tan;
    let half_root = thickness / 2.0 + ded * tan;
    if half_tip <= 1e-4 {
        return Err(ProfileError::PointedTip);
    }
    if half_root > pitch / 2.0 + 1e-9 {
        return Err(ProfileError::Overlap);
    }
    let (x0, x1) = (-pitch / 2.0, (f64::from(teeth) - 0.5) * pitch);
    let mut pts = vec![
        Point::new(x0, -ded - base),
        Point::new(x1, -ded - base),
        Point::new(x1, -ded),
    ];
    for i in (0..teeth).rev() {
        let xc = f64::from(i) * pitch;
        pts.push(Point::new(xc + half_root, -ded));
        pts.push(Point::new(xc + half_tip, add));
        pts.push(Point::new(xc - half_tip, add));
        pts.push(Point::new(xc - half_root, -ded));
    }
    pts.push(Point::new(x0, -ded));
    pts.dedup_by(|a, b| a.dist(*b) < 1e-9);
    Ok(pts)
}
