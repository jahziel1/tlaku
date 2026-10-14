//! Refinamiento del centro de cada cruz (método de Förstner).

use crate::imgproc::FloatImage;
use tlaku_core::Point;

// ---------------------------------------------------------------------------
// Refinamiento subpíxel por ajuste de rectas
// ---------------------------------------------------------------------------

/// Posición subpíxel del centro de una línea fina (oscura o clara) en el perfil que cruza
/// `q` en la dirección `n` (unitaria), esperada a `expect` px de `q`. El fondo se estima con
/// las dos orillas del perfil (interpolación lineal, así que un degradado de luz no sesga) y
/// el centro es el centroide del exceso al cuadrado. Devuelve el desplazamiento de la línea
/// respecto de `q` a lo largo de `n`, o `None` si no hay una línea clara.
fn line_offset(b: &FloatImage, q: Point, n: (f64, f64), expect: f64) -> Option<f64> {
    const STEP: f64 = 0.5;
    const HALF: f64 = 6.0;
    let count = (2.0 * HALF / STEP) as usize + 1;
    let samples: Vec<(f64, f64)> = (0..count)
        .map(|k| {
            let rel = -HALF + k as f64 * STEP;
            let s = expect + rel;
            (rel, f64::from(b.sample(q.x + s * n.0, q.y + s * n.1)))
        })
        .collect();
    let flank = |lo: f64, hi: f64| {
        let v: Vec<f64> = samples
            .iter()
            .filter(|(r, _)| (lo..=hi).contains(r))
            .map(|(_, p)| *p)
            .collect();
        v.iter().sum::<f64>() / v.len() as f64
    };
    let (left, right) = (flank(-HALF, -4.5), flank(4.5, HALF));
    let base = |rel: f64| left + (right - left) * (rel + 5.25) / 10.5;
    let dev: Vec<(f64, f64)> = samples.iter().map(|&(r, p)| (r, p - base(r))).collect();
    let inner: f64 = dev
        .iter()
        .filter(|(r, _)| r.abs() <= 3.0)
        .map(|(_, d)| d)
        .sum();
    // Línea oscura: el perfil baja (desviación negativa); clara: sube.
    let sign = if inner <= 0.0 { -1.0 } else { 1.0 };
    let flank_noise = {
        let v: Vec<f64> = dev
            .iter()
            .filter(|(r, _)| r.abs() >= 4.5)
            .map(|(_, d)| *d)
            .collect();
        (v.iter().map(|d| d * d).sum::<f64>() / v.len() as f64).sqrt()
    };
    let depth = dev
        .iter()
        .filter(|(r, _)| r.abs() <= 3.0)
        .map(|(_, d)| sign * d)
        .fold(0.0, f64::max);
    if depth < (4.0 * flank_noise).max(2.0) {
        return None;
    }
    // Solo la parte alta del perfil pesa (por encima de la mitad de la profundidad menos ruido).
    let cut = 0.25 * depth;
    let (mut sw, mut swr) = (0.0, 0.0);
    for (r, d) in &dev {
        if r.abs() > 3.5 {
            continue;
        }
        let e = (sign * d - cut).max(0.0);
        sw += e * e;
        swr += e * e * r;
    }
    if sw <= 0.0 {
        return None;
    }
    Some(expect + swr / sw)
}

/// Una recta `p + t·d` (`d` unitaria).
#[derive(Debug, Clone, Copy)]
struct Line {
    p: Point,
    d: (f64, f64),
}

/// Mide un brazo de la cruz: perfiles perpendiculares a lo largo del brazo (sin la zona del
/// cruce, donde la otra línea estorba) y recta de mínimos cuadrados con descarte de
/// atípicos. Devuelve la recta, el rms de los residuos (px) y el número de perfiles usados.
fn measure_arm(b: &FloatImage, c: Point, u: (f64, f64), t_max: f64) -> Option<(Line, f64, usize)> {
    const T_MIN: f64 = 3.5;
    let n = (-u.1, u.0);
    // Recta actual en la forma o(t) = a + k·t (desviación perpendicular al brazo nominal).
    let (mut a, mut k) = (0.0, 0.0);
    let mut stats = (0.0, 0);
    for pass in 0..3 {
        let mut pts: Vec<(f64, f64)> = Vec::new();
        let mut t = T_MIN;
        while t <= t_max {
            for sgn in [-1.0, 1.0] {
                let tt = sgn * t;
                let q = Point::new(c.x + tt * u.0, c.y + tt * u.1);
                if let Some(o) = line_offset(b, q, n, a + k * tt) {
                    pts.push((tt, o));
                }
            }
            t += 0.5;
        }
        // Recta robusta: ajuste, descarte de lo que se aleja y reajuste.
        let mut keep: Vec<bool> = vec![true; pts.len()];
        let mut fit = (0.0, 0.0, 0.0);
        for _ in 0..3 {
            let sel: Vec<&(f64, f64)> = pts
                .iter()
                .zip(&keep)
                .filter(|(_, k)| **k)
                .map(|(p, _)| p)
                .collect();
            let m = sel.len() as f64;
            if sel.len() < 6 {
                return None;
            }
            let (st, so) = (
                sel.iter().map(|p| p.0).sum::<f64>() / m,
                sel.iter().map(|p| p.1).sum::<f64>() / m,
            );
            let sxx: f64 = sel.iter().map(|p| (p.0 - st).powi(2)).sum();
            if sxx < 1e-9 {
                return None;
            }
            let slope = sel.iter().map(|p| (p.0 - st) * (p.1 - so)).sum::<f64>() / sxx;
            let icpt = so - slope * st;
            let rms = (sel
                .iter()
                .map(|p| (p.1 - icpt - slope * p.0).powi(2))
                .sum::<f64>()
                / m)
                .sqrt();
            fit = (icpt, slope, rms);
            let lim = (2.5 * rms).max(0.35);
            let mut changed = false;
            for (i, p) in pts.iter().enumerate() {
                let ok = (p.1 - icpt - slope * p.0).abs() <= lim;
                if keep[i] != ok {
                    keep[i] = ok;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let used: Vec<&(f64, f64)> = pts
            .iter()
            .zip(&keep)
            .filter(|(_, k)| **k)
            .map(|(p, _)| p)
            .collect();
        let (neg, pos) = (
            used.iter().filter(|p| p.0 < 0.0).count(),
            used.iter().filter(|p| p.0 > 0.0).count(),
        );
        if neg < 3 || pos < 3 {
            return None;
        }
        a = fit.0;
        k = fit.1;
        stats = (fit.2, used.len());
        let _ = pass;
    }
    // Recta: pasa por c + a·n con dirección u + k·n.
    let d = (u.0 + k * n.0, u.1 + k * n.1);
    let len = d.0.hypot(d.1);
    Some((
        Line {
            p: Point::new(c.x + a * n.0, c.y + a * n.1),
            d: (d.0 / len, d.1 / len),
        },
        stats.0,
        stats.1,
    ))
}

/// Intersección de dos rectas; `None` si son casi paralelas.
fn intersect(l1: &Line, l2: &Line) -> Option<Point> {
    let cross = l1.d.0 * l2.d.1 - l1.d.1 * l2.d.0;
    if cross.abs() < 0.2 {
        return None;
    }
    let (dx, dy) = (l2.p.x - l1.p.x, l2.p.y - l1.p.y);
    let s = (dx * l2.d.1 - dy * l2.d.0) / cross;
    Some(Point::new(l1.p.x + s * l1.d.0, l1.p.y + s * l1.d.1))
}

/// Centro subpíxel de una cruz: intersección de las rectas de sus dos brazos. `u1`, `u2` son
/// las direcciones de los brazos en la imagen (unitarias, de la alineación provisional) y
/// `arm_px` la semilongitud del brazo. Se repite recentrando los perfiles. Devuelve el centro
/// y el rms de los residuos de las rectas (px).
pub(super) fn refine_cross(
    b: &FloatImage,
    start: Point,
    u1: (f64, f64),
    u2: (f64, f64),
    arm_px: f64,
) -> Option<(Point, f64)> {
    let t_max = (0.85 * arm_px).max(6.0);
    let mut c = start;
    let mut rms = 0.0;
    for _ in 0..3 {
        let (l1, r1, _) = measure_arm(b, c, u1, t_max)?;
        let (l2, r2, _) = measure_arm(b, c, u2, t_max)?;
        let x = intersect(&l1, &l2)?;
        rms = r1.max(r2);
        let moved = x.dist(c);
        c = x;
        if moved < 0.02 {
            break;
        }
        if c.dist(start) > 4.0 {
            return None;
        }
    }
    Some((c, rms))
}
