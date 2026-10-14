//! Superficies de contacto simuladas para la sonda de `G38.x`.
//!
//! La controladora emulada no tiene una pieza física: la prueba le pone una
//! [`ProbeSurface`] (un plano, un objeto redondo o una caja, siempre en coordenadas de máquina)
//! y el emulador decide en qué punto de la trayectoria de `G38.2`/`G38.3` (hacia la pieza) o
//! `G38.4`/`G38.5` (alejándose) cambia el estado del pin de la sonda. Todas las formas son
//! convexas, así que el trozo de una recta que cae dentro es un solo intervalo.
//!
//! Fuente del comportamiento (documentación pública de GRBL 1.1, wiki de gnea/grbl, "G-code
//! Commands" y "Alarm Codes"): `G38.2` y `G38.3` avanzan hacia la pieza hasta que el pin cambia
//! (`G38.2` da `ALARM:5` si no hay contacto en todo el recorrido; `G38.3` no); `G38.4` y `G38.5`
//! lo mismo pero hasta que el pin se suelta; si el pin ya está en el estado de llegada al
//! empezar, `ALARM:4`. Al terminar se imprime `[PRB:x,y,z:1]` (o `:0` sin contacto) con la
//! posición de máquina. Está escrito desde esa descripción, no desde el código de GRBL.

/// Tolerancia (mm) con la que un punto sobre la superficie cuenta como en contacto.
const EPS: f64 = 1e-6;

/// Lo que toca la sonda. Coordenadas de máquina (MPos) en mm.
#[derive(Debug, Clone, PartialEq)]
pub enum ProbeSurface {
    /// Placa o pieza plana: hay contacto con `Z <= z`.
    Plane { z: f64 },
    /// Objeto redondo sobre la cama (un cilindro vertical): contacto con el punto `(X, Y)`
    /// dentro del círculo y `Z <= top`.
    Disc {
        center: [f64; 2],
        radius: f64,
        top: f64,
    },
    /// Caja alineada con los ejes: contacto con el punto entre `min` y `max`.
    Block { min: [f64; 3], max: [f64; 3] },
}

/// Intervalo `[lo, hi]` de la recta `v0 + (v1 - v0) t` donde se cumple `v <= limit` (si `upper`)
/// o `v >= limit`, recortando el intervalo previo. `None` si queda vacío.
fn clip(range: (f64, f64), v0: f64, v1: f64, limit: f64, upper: bool) -> Option<(f64, f64)> {
    let (lo, hi) = range;
    // `s * v` crece cuando nos acercamos a la violación: queremos `s * (v - limit) <= 0`.
    let s = if upper { 1.0 } else { -1.0 };
    let a = s * (v0 - limit);
    let b = s * (v1 - v0);
    let (lo, hi) = if b.abs() < 1e-12 {
        if a <= EPS { (lo, hi) } else { return None }
    } else {
        let t = -a / b;
        if b > 0.0 {
            (lo, hi.min(t))
        } else {
            (lo.max(t), hi)
        }
    };
    (lo <= hi + 1e-12).then_some((lo, hi))
}

impl ProbeSurface {
    /// ¿El pin de la sonda está activo con la cabeza en `p`?
    pub fn contains(&self, p: [f64; 3]) -> bool {
        self.span(p, p).is_some()
    }

    /// Tramo `[t0, t1]` (con `0 <= t0 <= t1 <= 1`) de la recta `from -> to` que cae dentro de
    /// la superficie, o `None` si no la toca.
    pub fn span(&self, from: [f64; 3], to: [f64; 3]) -> Option<(f64, f64)> {
        let mut r = (0.0, 1.0);
        match self {
            Self::Plane { z } => {
                r = clip(r, from[2], to[2], *z, true)?;
            }
            Self::Disc {
                center,
                radius,
                top,
            } => {
                r = clip(r, from[2], to[2], *top, true)?;
                let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
                let (fx, fy) = (from[0] - center[0], from[1] - center[1]);
                let a = dx * dx + dy * dy;
                let c = fx * fx + fy * fy - radius * radius;
                if a < 1e-18 {
                    if c > EPS * radius.max(1.0) {
                        return None;
                    }
                } else {
                    let b = 2.0 * (fx * dx + fy * dy);
                    let disc = b * b - 4.0 * a * c;
                    if disc < 0.0 {
                        return None;
                    }
                    let q = disc.sqrt();
                    let (t0, t1) = ((-b - q) / (2.0 * a), (-b + q) / (2.0 * a));
                    r = (r.0.max(t0), r.1.min(t1));
                    if r.0 > r.1 + 1e-12 {
                        return None;
                    }
                }
            }
            Self::Block { min, max } => {
                for i in 0..3 {
                    r = clip(r, from[i], to[i], max[i], true)?;
                    r = clip(r, from[i], to[i], min[i], false)?;
                }
            }
        }
        Some((r.0.max(0.0), r.1.min(1.0)))
    }

    /// Dónde cambia el pin al ir de `from` a `to`. Hacia la pieza (`away == false`) es el primer
    /// punto con contacto; alejándose, el último punto con contacto antes de soltarse. `None` si
    /// el pin no cambia en todo el recorrido (o ya estaba en el estado de llegada).
    pub fn transition(&self, from: [f64; 3], to: [f64; 3], away: bool) -> Option<[f64; 3]> {
        let at = |t: f64| {
            [
                from[0] + (to[0] - from[0]) * t,
                from[1] + (to[1] - from[1]) * t,
                from[2] + (to[2] - from[2]) * t,
            ]
        };
        let (t0, t1) = self.span(from, to)?;
        if away {
            // Debe empezar dentro y salir antes del final.
            (t0 <= 1e-12 && t1 < 1.0 - 1e-12).then(|| at(t1))
        } else {
            (t0 > 1e-12).then(|| at(t0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: [f64; 3], b: [f64; 3]) {
        for i in 0..3 {
            assert!((a[i] - b[i]).abs() < 1e-9, "{a:?} != {b:?}");
        }
    }

    #[test]
    fn plane_touches_when_descending() {
        let s = ProbeSurface::Plane { z: 5.0 };
        near(
            s.transition([0.0, 0.0, 20.0], [0.0, 0.0, -10.0], false)
                .unwrap(),
            [0.0, 0.0, 5.0],
        );
        // No llega hasta el plano.
        assert!(
            s.transition([0.0, 0.0, 20.0], [0.0, 0.0, 8.0], false)
                .is_none()
        );
        // Ya en contacto: no hay entrada.
        assert!(
            s.transition([0.0, 0.0, 4.0], [0.0, 0.0, -3.0], false)
                .is_none()
        );
        assert!(s.contains([1.0, 1.0, 5.0]) && !s.contains([1.0, 1.0, 5.01]));
    }

    #[test]
    fn disc_is_hit_on_its_edge_from_any_side() {
        let s = ProbeSurface::Disc {
            center: [50.0, 40.0],
            radius: 10.0,
            top: 3.0,
        };
        near(
            s.transition([0.0, 40.0, 1.0], [50.0, 40.0, 1.0], false)
                .unwrap(),
            [40.0, 40.0, 1.0],
        );
        near(
            s.transition([50.0, 100.0, 1.0], [50.0, 40.0, 1.0], false)
                .unwrap(),
            [50.0, 50.0, 1.0],
        );
        // Por encima del objeto o pasando de largo, no toca.
        assert!(
            s.transition([0.0, 40.0, 5.0], [100.0, 40.0, 5.0], false)
                .is_none()
        );
        assert!(
            s.transition([0.0, 60.0, 1.0], [100.0, 60.0, 1.0], false)
                .is_none()
        );
        // Alejándose desde dentro: sale por el otro lado.
        near(
            s.transition([50.0, 40.0, 1.0], [80.0, 40.0, 1.0], true)
                .unwrap(),
            [60.0, 40.0, 1.0],
        );
    }

    #[test]
    fn block_clips_every_axis() {
        let s = ProbeSurface::Block {
            min: [10.0, 10.0, 0.0],
            max: [20.0, 20.0, 5.0],
        };
        near(
            s.transition([0.0, 15.0, 2.0], [30.0, 15.0, 2.0], false)
                .unwrap(),
            [10.0, 15.0, 2.0],
        );
        assert!(
            s.transition([0.0, 25.0, 2.0], [30.0, 25.0, 2.0], false)
                .is_none()
        );
    }
}
