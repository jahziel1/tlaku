//! Marcas grabadas para alinear la cámara con la cama.
//!
//! El usuario graba sobre un cartón 4 o 9 cruces en posiciones conocidas de la cama (la
//! primera, la esquina inferior izquierda, lleva además un círculo concéntrico que fija la
//! orientación). Se fotografía sin mover el material, se corrige la lente y este módulo:
//!
//! 1. Busca cruces en la imagen: puntos donde el tensor de estructura (gradientes
//!    integrados en una ventana pequeña) tiene dos direcciones fuertes, con la condición de
//!    simetría de punto de una cruz (descarta esquinas del cartón, vetas y ruido).
//! 2. Las asigna a sus posiciones teóricas. Con una alineación provisional (la manual de 4
//!    puntos) se usa el candidato más cercano a cada posición prevista; sin ella, una
//!    búsqueda por hipótesis (4 candidatos contra 4 marcas, descartando espejos) puntuada por
//!    cuántas otras marcas explica y por el círculo de la marca de origen. Funciona aunque
//!    falte alguna marca o sobren candidatos.
//! 3. Refina el centro de cada cruz con el método de Förstner (mínimos cuadrados de las
//!    rectas normales al gradiente), con una ventana ligada al tamaño de la cruz en píxeles.
//! 4. Ajusta la homografía con RANSAC y mide la precisión real con validación cruzada
//!    dejando una marca fuera, en mm de cama.
//!
//! Convención: el centro del píxel `(i, j)` es `(i, j)`; `DetectOptions::pixel_offset`
//! permite entregar y recibir coordenadas con el origen en el borde del píxel (0.5).
//! Las marcas se colocan en coordenadas de cama (mm, Y hacia arriba).

use crate::homography::{Homography, ImageToBed, RansacOptions, estimate_homography};
use crate::imgproc::{FloatImage, gaussian_blur};
use image::GrayImage;
use serde::{Deserialize, Serialize};
use tlaku_core::Point;

/// Fracción del ancho (y del alto) de la cama que se deja libre a cada lado: las marcas
/// cubren el 85 % de cada dimensión.
const INSET: f64 = 0.075;
/// Medida mínima de la cama para generar marcas (mm).
const MIN_BED_MM: f64 = 60.0;
/// Un residuo mayor a este valor (mm) se considera dudoso.
pub const RESIDUAL_WARN_MM: f64 = 0.3;

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
            return Err(format!(
                "se pueden generar 4 o 9 marcas (se pidieron {count})"
            ));
        }
        if !bed_mm.iter().all(|v| v.is_finite() && *v >= MIN_BED_MM) {
            return Err(format!(
                "la cama debe medir al menos {MIN_BED_MM} mm por lado para las marcas"
            ));
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

// ---------------------------------------------------------------------------
// Resultado
// ---------------------------------------------------------------------------

/// Una marca de la disposición, con lo que se encontró de ella.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkResult {
    pub index: usize,
    /// Posición teórica en la cama (mm).
    pub bed: [f64; 2],
    /// Centro detectado en la imagen (px); `None` si no se encontró.
    pub image: Option<[f64; 2]>,
    /// Calidad de la detección, de 0 a 1 (simetría de la cruz).
    pub confidence: f64,
    /// Participa en el ajuste final (no fue descartada por incoherente).
    pub used: bool,
    /// Distancia en la cama (mm) entre la marca proyectada con el ajuste y su posición.
    pub residual_mm: Option<f64>,
    /// Error de validación cruzada (mm): se ajusta sin esta marca y se mide aquí.
    pub loo_mm: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarksReport {
    pub marks: Vec<MarkResult>,
    /// Todos los candidatos con forma de cruz que vio el detector (px), para dibujarlos.
    pub candidates: Vec<[f64; 2]>,
    /// Homografía imagen a cama, por filas.
    pub to_bed: [[f64; 3]; 3],
    pub expected: usize,
    pub found: usize,
    pub used: usize,
    /// Error del ajuste (rms, mm) sobre las marcas usadas.
    pub rms_mm: f64,
    pub max_residual_mm: f64,
    /// Validación cruzada (dejar una fuera): rms y máximo en mm. `None` con menos de 5 marcas
    /// usadas: el ajuste es exacto y no hay con qué medir.
    pub loo_rms_mm: Option<f64>,
    pub loo_max_mm: Option<f64>,
    /// Escala de la imagen en el centro de la disposición (px por mm).
    pub scale_px_per_mm: f64,
    pub used_prior: bool,
    pub warnings: Vec<String>,
}

/// Opciones de la detección.
#[derive(Debug, Clone)]
pub struct DetectOptions {
    /// Alineación provisional imagen a cama (p. ej. la manual de 4 puntos), en las mismas
    /// coordenadas de píxel que se entregan. Con ella se busca cerca de lo previsto.
    pub prior_to_bed: Option<Homography>,
    /// Desplazamiento entre el origen de las coordenadas de entrada/salida y el centro del
    /// píxel: 0.0 si `(i, j)` es el centro del píxel, 0.5 si es su esquina.
    pub pixel_offset: f64,
    /// Umbral de RANSAC, en mm de cama.
    pub ransac_threshold_mm: f64,
}

impl Default for DetectOptions {
    fn default() -> Self {
        Self {
            prior_to_bed: None,
            pixel_offset: 0.0,
            ransac_threshold_mm: 1.5,
        }
    }
}

// ---------------------------------------------------------------------------
// Candidatos
// ---------------------------------------------------------------------------

struct Grad {
    gx: Vec<f32>,
    gy: Vec<f32>,
}

impl Grad {
    fn new(b: &FloatImage) -> Self {
        let (w, h) = (b.w, b.h);
        let mut gx = vec![0.0f32; w * h];
        let mut gy = vec![0.0f32; w * h];
        for y in 1..h.saturating_sub(1) {
            for x in 1..w.saturating_sub(1) {
                gx[y * w + x] = 0.5 * (b.get(x + 1, y) - b.get(x - 1, y));
                gy[y * w + x] = 0.5 * (b.get(x, y + 1) - b.get(x, y - 1));
            }
        }
        Self { gx, gy }
    }
}

#[derive(Debug, Clone, Copy)]
struct Cand {
    p: Point,
    /// Simetría de punto, de 0 a 1.
    sym: f64,
    score: f64,
}

/// Correlación entre la imagen y su giro de 180 grados alrededor de `c`, en un disco de
/// radio `r`: vale cerca de 1 en una cruz, un círculo o un punto, y poco en esquinas, puntas
/// de línea y ruido. Como función de `c` tiene su máximo en el centro de la cruz.
fn point_symmetry(b: &FloatImage, c: Point, r: f64) -> f64 {
    let ri = r.ceil() as i64;
    let (mut sa, mut sb, mut saa, mut sbb, mut sab, mut n) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for j in -ri..=ri {
        for i in -ri..=ri {
            // Solo medio plano: cada par se cuenta una vez.
            if j < 0 || (j == 0 && i <= 0) {
                continue;
            }
            let (dx, dy) = (i as f64, j as f64);
            let d = dx.hypot(dy);
            if !(1.5..=r).contains(&d) {
                continue;
            }
            let a = f64::from(b.sample(c.x + dx, c.y + dy));
            let o = f64::from(b.sample(c.x - dx, c.y - dy));
            sa += a;
            sb += o;
            saa += a * a;
            sbb += o * o;
            sab += a * o;
            n += 1.0;
        }
    }
    if n < 6.0 {
        return 0.0;
    }
    let (va, vb) = (saa / n - (sa / n).powi(2), sbb / n - (sb / n).powi(2));
    if va < 1e-6 || vb < 1e-6 {
        return 0.0;
    }
    ((sab / n - sa * sb / (n * n)) / (va * vb).sqrt()).clamp(-1.0, 1.0)
}

/// Centro de simetría cerca de `start`: búsqueda en rejilla de ±2.5 px (paso 0.5) y luego
/// de ±0.5 px (paso 0.1). Devuelve el centro y su simetría.
fn symmetry_center(b: &FloatImage, start: Point, r: f64) -> (Point, f64) {
    let mut best = (start, f64::NEG_INFINITY);
    let scan = |center: Point, half: f64, step: f64, best: &mut (Point, f64)| {
        let n = (half / step).round() as i64;
        for j in -n..=n {
            for i in -n..=n {
                let p = Point::new(center.x + i as f64 * step, center.y + j as f64 * step);
                let s = point_symmetry(b, p, r);
                if s > best.1 {
                    *best = (p, s);
                }
            }
        }
    };
    scan(start, 2.5, 0.5, &mut best);
    let coarse = best.0;
    scan(coarse, 0.5, 0.1, &mut best);
    best
}

/// Candidatos con forma de cruz, ordenados de mejor a peor. `want` es cuántos se conservan.
/// Los picos del tensor de estructura (dos direcciones fuertes de gradiente en una ventana
/// pequeña) dan la posición a unos 2 px; la simetría de punto la afina y descarta lo que no
/// es una cruz (puntas de línea, esquinas del cartón, veta).
fn find_candidates(b: &FloatImage, g: &Grad, want: usize) -> Vec<Cand> {
    let (w, h) = (b.w, b.h);
    if w < 24 || h < 24 {
        return Vec::new();
    }
    let sq = |gv: &[f32], gv2: &[f32]| FloatImage {
        w,
        h,
        data: gv.iter().zip(gv2).map(|(a, c)| a * c).collect(),
    };
    let sxx = gaussian_blur(&sq(&g.gx, &g.gx), 2.0);
    let sxy = gaussian_blur(&sq(&g.gx, &g.gy), 2.0);
    let syy = gaussian_blur(&sq(&g.gy, &g.gy), 2.0);
    let l2: Vec<f32> = (0..w * h)
        .map(|i| {
            let (a, bq, c) = (sxx.data[i], sxy.data[i], syy.data[i]);
            0.5 * (a + c) - (0.25 * (a - c) * (a - c) + bq * bq).sqrt()
        })
        .collect();
    let max = l2.iter().copied().fold(0.0f32, f32::max);
    if max <= 0.0 {
        return Vec::new();
    }
    let mut sorted = l2.clone();
    let mid = sorted.len() / 2;
    let median = *sorted.select_nth_unstable_by(mid, f32::total_cmp).1;
    let thr = (25.0 * median).max(0.003 * max);
    // Máximos locales de 7x7.
    let mut peaks: Vec<(f32, usize, usize)> = Vec::new();
    for y in 4..h - 4 {
        for x in 4..w - 4 {
            let v = l2[y * w + x];
            if v < thr {
                continue;
            }
            let mut is_max = true;
            'win: for dy in -3i64..=3 {
                for dx in -3i64..=3 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let o = l2[(y as i64 + dy) as usize * w + (x as i64 + dx) as usize];
                    // Empates: gana el primero en orden de lectura.
                    if o > v || (o == v && (dy < 0 || (dy == 0 && dx < 0))) {
                        is_max = false;
                        break 'win;
                    }
                }
            }
            if is_max {
                peaks.push((v, x, y));
            }
        }
    }
    peaks.sort_by(|a, c| c.0.total_cmp(&a.0));
    peaks.truncate(120);
    let mut cands: Vec<Cand> = Vec::new();
    for &(v, x, y) in &peaks {
        let (p, sym) = symmetry_center(b, Point::new(x as f64, y as f64), 7.0);
        if sym < 0.5 {
            continue;
        }
        // Dos picos que convergen al mismo centro cuentan una vez (gana el más simétrico).
        if let Some(prev) = cands.iter_mut().find(|c| c.p.dist(p) < 4.0) {
            if sym > prev.sym {
                *prev = Cand {
                    p,
                    sym,
                    score: f64::from(v) * sym * sym,
                };
            }
            continue;
        }
        cands.push(Cand {
            p,
            sym,
            score: f64::from(v) * sym * sym,
        });
    }
    cands.sort_by(|a, c| c.score.total_cmp(&a.score));
    // Fuera lo mucho más débil que lo mejor (ruido de la veta o del cartón).
    if let Some(best) = cands.first().map(|c| c.score) {
        cands.retain(|c| c.score >= 0.02 * best);
    }
    cands.truncate(want);
    cands
}

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
fn refine_cross(
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

// ---------------------------------------------------------------------------
// Asignación
// ---------------------------------------------------------------------------

/// Orientación (signo del área) del triángulo `a, b, c`.
fn orient(a: Point, b: Point, c: Point) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// Escala local de una homografía en `p`: raíz del determinante del jacobiano.
fn local_scale(h: &Homography, p: Point) -> f64 {
    let e = 1.0;
    let (c, px, py) = (
        h.apply(p),
        h.apply(Point::new(p.x + e, p.y)),
        h.apply(Point::new(p.x, p.y + e)),
    );
    let det = (px.x - c.x) * (py.y - c.y) - (px.y - c.y) * (py.x - c.x);
    det.abs().sqrt() / e
}

/// Contraste del círculo de la marca 0: diferencia media de gris entre un anillo a
/// `ring_mm` y otro más afuera (fuera de los brazos), muestreados en las diagonales de la
/// cama. `bed_to_img` lleva mm de cama a píxel. Positivo en valor absoluto = hay círculo.
fn ring_contrast(b: &FloatImage, bed_to_img: &Homography, c: Point, layout: &MarkLayout) -> f64 {
    let (mut sum, mut n) = (0.0, 0.0);
    for k in 0..12 {
        let t = std::f64::consts::TAU * (k as f64 + 0.5) / 12.0;
        let at = |r: f64| {
            let q = bed_to_img.apply(Point::new(c.x + r * t.cos(), c.y + r * t.sin()));
            f64::from(b.sample(q.x, q.y))
        };
        // El círculo es una línea fina: se busca el extremo del perfil radial y se compara con
        // el fondo de afuera.
        let outer = at(layout.ring_mm * 1.35);
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for d in [-0.8, -0.4, 0.0, 0.4, 0.8] {
            let v = at(layout.ring_mm + d);
            lo = lo.min(v);
            hi = hi.max(v);
        }
        sum += (outer - lo).abs().max((hi - outer).abs());
        n += 1.0;
    }
    sum / n
}

/// Resultado de la asignación: para cada marca, el índice del candidato (o `None`).
type Assignment = Vec<Option<usize>>;

/// Con una alineación provisional: el candidato más cercano a cada posición prevista,
/// dentro de un radio del 7 % de la mayor dimensión de la imagen.
fn assign_with_prior(
    layout: &MarkLayout,
    cands: &[Cand],
    bed_to_img: &Homography,
    radius_px: f64,
) -> Assignment {
    let mut out: Assignment = vec![None; layout.marks.len()];
    let mut taken = vec![false; cands.len()];
    // Primero los pares más cercanos, para que un candidato no se lo lleve la marca equivocada.
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for (j, m) in layout.marks.iter().enumerate() {
        let pred = bed_to_img.apply(*m);
        for (i, c) in cands.iter().enumerate() {
            let d = c.p.dist(pred);
            if d <= radius_px {
                pairs.push((d, j, i));
            }
        }
    }
    pairs.sort_by(|a, c| a.0.total_cmp(&c.0));
    for (_, j, i) in pairs {
        if out[j].is_none() && !taken[i] {
            out[j] = Some(i);
            taken[i] = true;
        }
    }
    out
}

/// Cuadrilátero de marcas con el que se generan las hipótesis: sus 4 esquinas, o (con 9
/// marcas) los puntos medios de los bordes, por si falta una esquina.
fn base_quads(m: usize) -> Vec<[usize; 4]> {
    let mut v = vec![[0, 1, 2, 3]];
    if m >= 9 {
        v.push([4, 5, 6, 7]);
        v.push([0, 5, 6, 7]);
        v.push([4, 1, 6, 7]);
        v.push([4, 5, 2, 7]);
        v.push([4, 5, 6, 3]);
    }
    v
}

/// Sin alineación provisional: hipótesis de 4 candidatos contra 4 marcas fijas de la
/// disposición (se recorren todas las ordenaciones de candidatos que conservan la
/// orientación), puntuadas por cuántas marcas más explican.
fn assign_blind(
    layout: &MarkLayout,
    cands: &[Cand],
    b: &FloatImage,
    size: (usize, usize),
) -> Option<Assignment> {
    let (m, n) = (layout.marks.len(), cands.len());
    if n < 4 {
        return None;
    }
    let (iw, ih) = (size.0 as f64, size.1 as f64);
    let best_score = cands.iter().map(|c| c.score).fold(0.0, f64::max).max(1e-12);
    let mut best: Option<(f64, Assignment)> = None;
    for quad in base_quads(m) {
        let lp = quad.map(|j| layout.marks[j]);
        // Orientación esperada de cada trío del cuadrilátero de la cama (la imagen la invierte).
        let trios = [[0, 1, 2], [0, 1, 3], [0, 2, 3], [1, 2, 3]];
        let want: Vec<f64> = trios
            .iter()
            .map(|t| orient(lp[t[0]], lp[t[1]], lp[t[2]]).signum())
            .collect();
        let mut ci = [0usize; 4];
        for a in 0..n {
            ci[0] = a;
            for bq in 0..n {
                if bq == a {
                    continue;
                }
                ci[1] = bq;
                for c in 0..n {
                    if c == a || c == bq {
                        continue;
                    }
                    ci[2] = c;
                    // Con tres puntos ya se descarta lo que no cuadra con el primer trío.
                    let o = orient(cands[a].p, cands[bq].p, cands[c].p);
                    if o * want[0] >= 0.0 || o.abs() < 1.0 {
                        continue;
                    }
                    for d in 0..n {
                        if d == a || d == bq || d == c {
                            continue;
                        }
                        ci[3] = d;
                        let ok = trios.iter().enumerate().skip(1).all(|(k, t)| {
                            let o = orient(cands[ci[t[0]]].p, cands[ci[t[1]]].p, cands[ci[t[2]]].p);
                            o * want[k] < 0.0 && o.abs() >= 1.0
                        });
                        if !ok {
                            continue;
                        }
                        let pairs: Vec<(Point, Point)> =
                            (0..4).map(|q| (lp[q], cands[ci[q]].p)).collect();
                        let Ok(bed_to_img) = estimate_homography(&pairs) else {
                            continue;
                        };
                        if let Some((score, assign)) =
                            score_hypothesis(layout, cands, b, &bed_to_img, (iw, ih), best_score)
                            && best.as_ref().is_none_or(|(s, _)| score > *s)
                        {
                            best = Some((score, assign));
                        }
                    }
                }
            }
        }
    }
    best.map(|(_, a)| a)
}

/// Puntúa una hipótesis cama a imagen: marcas explicadas (cada una con su candidato más
/// cercano dentro de 2 mm), calidad de esos candidatos y círculo de la marca de origen.
fn score_hypothesis(
    layout: &MarkLayout,
    cands: &[Cand],
    b: &FloatImage,
    bed_to_img: &Homography,
    (iw, ih): (f64, f64),
    best_score: f64,
) -> Option<(f64, Assignment)> {
    let (m, n) = (layout.marks.len(), cands.len());
    let scale = local_scale(
        bed_to_img,
        Point::new(layout.bed_mm[0] / 2.0, layout.bed_mm[1] / 2.0),
    );
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    // Toda la disposición debe caber (con holgura) en la imagen.
    let inside = layout.marks.iter().all(|mk| {
        let q = bed_to_img.apply(*mk);
        q.x.is_finite()
            && q.y.is_finite()
            && q.x > -0.25 * iw
            && q.x < 1.25 * iw
            && q.y > -0.25 * ih
            && q.y < 1.25 * ih
    });
    if !inside {
        return None;
    }
    let tol = (2.0 * scale).max(3.0);
    let mut assign: Assignment = vec![None; m];
    let mut used = vec![false; n];
    let mut matched = 0usize;
    let (mut err_sum, mut quality) = (0.0, 0.0);
    for (j, mk) in layout.marks.iter().enumerate() {
        let pred = bed_to_img.apply(*mk);
        let mut bi = None;
        let mut bd = tol;
        for (i, c) in cands.iter().enumerate() {
            if used[i] {
                continue;
            }
            let d = c.p.dist(pred);
            if d < bd {
                bd = d;
                bi = Some(i);
            }
        }
        if let Some(i) = bi {
            used[i] = true;
            assign[j] = Some(i);
            matched += 1;
            err_sum += bd / scale;
            quality += (cands[i].score / best_score).min(1.0);
        }
    }
    if matched < 4 {
        return None;
    }
    // El círculo de la marca 0 resuelve los giros de la disposición.
    let ring_term = if assign[0].is_some() {
        let r0 = ring_contrast(b, bed_to_img, layout.marks[0], layout);
        let mut other: f64 = 0.0;
        for (j, a) in assign.iter().enumerate().skip(1) {
            if a.is_some() {
                other = other.max(ring_contrast(b, bed_to_img, layout.marks[j], layout));
            }
        }
        (r0 - other).clamp(-100.0, 100.0)
    } else {
        -100.0
    };
    let score = matched as f64 * 1000.0 + quality * 50.0 + ring_term * 5.0 - err_sum * 5.0;
    Some((score, assign))
}

// ---------------------------------------------------------------------------
// Detección completa
// ---------------------------------------------------------------------------

fn offset(p: Point, d: f64) -> Point {
    Point::new(p.x + d, p.y + d)
}

/// Detecta las marcas de `layout` en `img` (ya corregida de lente), asigna y ajusta la
/// homografía imagen a cama, y mide la precisión por validación cruzada.
pub fn detect_marks(
    img: &GrayImage,
    layout: &MarkLayout,
    opts: &DetectOptions,
) -> Result<MarksReport, String> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    if w < 64 || h < 64 {
        return Err("la imagen es demasiado pequeña para buscar marcas".into());
    }
    let m = layout.marks.len();
    let off = opts.pixel_offset;
    let raw = FloatImage::from_gray(img);
    let blurred = gaussian_blur(&raw, 1.0);
    let grad = Grad::new(&blurred);
    let cands = find_candidates(&blurred, &grad, 2 * m + 6);
    if cands.len() < 4 {
        return Err(format!(
            "no se encontraron suficientes marcas en la imagen (se vieron {} y hacen falta al menos 4); \
             revisa la iluminación, el enfoque y que las marcas estén dentro del encuadre",
            cands.len()
        ));
    }

    // Asignación.
    let mut used_prior = false;
    let mut assign: Option<Assignment> = None;
    if let Some(prior) = &opts.prior_to_bed
        && let Ok(inv) = prior.inverse()
    {
        // La alineación entra en las coordenadas de salida; el detector trabaja en centros.
        let bed_to_img = Homography::from_matrix(
            &(nalgebra::Matrix3::new(1.0, 0.0, -off, 0.0, 1.0, -off, 0.0, 0.0, 1.0) * inv.matrix()),
        );
        let radius = 0.07 * w.max(h) as f64;
        let a = assign_with_prior(layout, &cands, &bed_to_img, radius);
        if a.iter().flatten().count() >= 4 {
            used_prior = true;
            assign = Some(a);
        }
    }
    let assign = match assign {
        Some(a) => a,
        None => assign_blind(layout, &cands, &blurred, (w, h)).ok_or(
            "no se pudo reconocer la disposición de las marcas (¿son de otro documento, o la \
             imagen está espejada?); con una alineación manual de 4 puntos previa la búsqueda \
             es más fiable",
        )?,
    };
    let mut warnings = Vec::new();

    // Homografía inicial con los candidatos asignados, para fijar el tamaño de la ventana.
    let init: Vec<(usize, Point)> = assign
        .iter()
        .enumerate()
        .filter_map(|(j, a)| a.map(|i| (j, cands[i].p)))
        .collect();
    let init_pairs: Vec<(Point, Point)> = init.iter().map(|&(j, p)| (p, layout.marks[j])).collect();
    let init_h = estimate_homography(&init_pairs)
        .map_err(|e| format!("las marcas encontradas no definen una alineación: {e}"))?;

    // Refinamiento de cada marca: se ajustan las rectas de sus dos brazos y se intersectan.
    // Las direcciones y el tamaño en píxeles salen de la alineación inicial.
    let init_bed_to_img = init_h.inverse()?;
    let mut refined: Vec<Option<(Point, f64)>> = vec![None; m];
    for &(j, p0) in &init {
        let bed = layout.marks[j];
        let c = init_bed_to_img.apply(bed);
        let unit = |dx: f64, dy: f64| {
            let q = init_bed_to_img.apply(Point::new(bed.x + dx, bed.y + dy));
            let (vx, vy) = (q.x - c.x, q.y - c.y);
            let len = vx.hypot(vy);
            ((vx / len, vy / len), len)
        };
        let (u1, s1) = unit(1.0, 0.0);
        let (u2, _) = unit(0.0, 1.0);
        if !(s1.is_finite() && s1 > 0.0 && u1.0.is_finite() && u2.0.is_finite()) {
            continue;
        }
        let arm_px = layout.arm_mm * s1;
        let sym = cands[assign[j].unwrap_or(0)].sym;
        if let Some((p, rms)) = refine_cross(&blurred, p0, u1, u2, arm_px)
            && rms <= 0.8
        {
            refined[j] = Some((p, sym));
        }
    }
    let refined_count = refined.iter().flatten().count();
    if refined_count < 4 {
        return Err(format!(
            "solo se pudo medir con precisión el centro de {refined_count} marcas (hacen falta 4); \
             las líneas grabadas son muy tenues o están desenfocadas: sube un poco la potencia o enfoca"
        ));
    }

    // Ajuste con RANSAC sobre las posiciones refinadas (en coordenadas de salida).
    let present: Vec<usize> = (0..m).filter(|&j| refined[j].is_some()).collect();
    let image_pts: Vec<Point> = present
        .iter()
        .map(|&j| offset(refined[j].map(|r| r.0).unwrap_or(layout.marks[j]), off))
        .collect();
    let bed_pts: Vec<Point> = present.iter().map(|&j| layout.marks[j]).collect();
    let ransac = RansacOptions {
        threshold: opts.ransac_threshold_mm,
        ..RansacOptions::default()
    };
    let fit = ImageToBed::fit(&image_pts, &bed_pts, Some(&ransac))?;
    let to_bed = fit.alignment.to_bed;
    let mut used_flags = vec![false; m];
    for (k, &j) in present.iter().enumerate() {
        used_flags[j] = fit.inliers[k];
    }
    let used_count = used_flags.iter().filter(|&&u| u).count();
    if used_count < 4 {
        return Err(
            "las marcas no son coherentes entre sí; revisa que se hayan grabado completas".into(),
        );
    }

    // Residuos y validación cruzada.
    let mut results: Vec<MarkResult> = (0..m)
        .map(|j| MarkResult {
            index: j,
            bed: [layout.marks[j].x, layout.marks[j].y],
            image: None,
            confidence: 0.0,
            used: used_flags[j],
            residual_mm: None,
            loo_mm: None,
        })
        .collect();
    for (k, &j) in present.iter().enumerate() {
        let r = &mut results[j];
        r.image = Some([image_pts[k].x, image_pts[k].y]);
        r.confidence = refined[j].map_or(0.0, |x| x.1);
        r.residual_mm = Some(to_bed.apply(image_pts[k]).dist(bed_pts[k]));
    }
    let used_idx: Vec<usize> = (0..present.len())
        .filter(|&k| used_flags[present[k]])
        .collect();
    let mut loo_errs = Vec::new();
    if used_idx.len() >= 5 {
        for &k in &used_idx {
            let rest: Vec<(Point, Point)> = used_idx
                .iter()
                .filter(|&&q| q != k)
                .map(|&q| (image_pts[q], bed_pts[q]))
                .collect();
            if let Ok(hh) = estimate_homography(&rest) {
                let e = hh.apply(image_pts[k]).dist(bed_pts[k]);
                results[present[k]].loo_mm = Some(e);
                loo_errs.push(e);
            }
        }
    }
    let residuals: Vec<f64> = used_idx
        .iter()
        .map(|&k| to_bed.apply(image_pts[k]).dist(bed_pts[k]))
        .collect();
    let rms_mm = (residuals.iter().map(|e| e * e).sum::<f64>() / residuals.len() as f64).sqrt();
    let max_residual_mm = residuals.iter().copied().fold(0.0, f64::max);
    let (loo_rms_mm, loo_max_mm) = if loo_errs.is_empty() {
        (None, None)
    } else {
        (
            Some((loo_errs.iter().map(|e| e * e).sum::<f64>() / loo_errs.len() as f64).sqrt()),
            Some(loo_errs.iter().copied().fold(0.0, f64::max)),
        )
    };

    let found = present.len();
    if found < m {
        warnings.push(format!("Se encontraron {found} de {m} marcas."));
    }
    if used_count < found {
        warnings.push(format!(
            "{} marca(s) se descartaron por no concordar con las demás.",
            found - used_count
        ));
    }
    let judge = loo_max_mm.unwrap_or(max_residual_mm);
    if judge > RESIDUAL_WARN_MM {
        warnings.push(format!(
            "Hay un error de {judge:.2} mm en alguna marca (se recomienda menos de {RESIDUAL_WARN_MM} mm)."
        ));
    }

    let center = Point::new(layout.bed_mm[0] / 2.0, layout.bed_mm[1] / 2.0);
    let scale_px_per_mm = local_scale(&fit.alignment.to_image, center);
    Ok(MarksReport {
        marks: results,
        candidates: cands
            .iter()
            .map(|c| {
                let p = offset(c.p, off);
                [p.x, p.y]
            })
            .collect(),
        to_bed: to_bed.m,
        expected: m,
        found,
        used: used_count,
        rms_mm,
        max_residual_mm,
        loo_rms_mm,
        loo_max_mm,
        scale_px_per_mm,
        used_prior,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::LensCalibration;
    use crate::height::{HeightSample, fit_height_model};
    use crate::testutil::{Rng, truth_camera};
    use crate::undistort::UndistortMap;
    use nalgebra::{Matrix3, Rotation3, Vector3};

    const BED: [f64; 2] = [300.0, 200.0];

    /// Pose de la cama frente a la cámara de prueba (X derecha, Y arriba, Z hacia la cámara).
    #[derive(Clone, Copy)]
    struct BedPose {
        r: Matrix3<f64>,
        t: Vector3<f64>,
    }

    fn bed_pose(tilt: (f64, f64, f64), center: (f64, f64, f64)) -> BedPose {
        let d = std::f64::consts::PI / 180.0;
        let rot = Rotation3::from_euler_angles(tilt.0 * d, tilt.1 * d, tilt.2 * d);
        let flip = Matrix3::new(1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, -1.0);
        let r = rot.matrix() * flip;
        // `center` es adónde cae el centro de la cama (a altura 0) en la cámara.
        let c = Vector3::new(BED[0] / 2.0, BED[1] / 2.0, 0.0);
        let t = Vector3::new(center.0, center.1, center.2) - r * c;
        BedPose { r, t }
    }

    fn std_pose() -> BedPose {
        bed_pose((7.0, -5.0, 3.0), (4.0, -3.0, 345.0))
    }

    /// Matriz cama a imagen corregida (centro de píxel) a altura `h`.
    fn truth_h(cam: &LensCalibration, p: &BedPose, h: f64) -> Homography {
        let k = Matrix3::new(cam.fx, 0.0, cam.cx, 0.0, cam.fy, cam.cy, 0.0, 0.0, 1.0);
        let cols = [
            p.r.column(0).into_owned(),
            p.r.column(1).into_owned(),
            p.t + p.r.column(2) * h,
        ];
        Homography::from_matrix(&(k * Matrix3::from_columns(&cols)))
    }

    #[derive(Clone, Copy)]
    struct Look {
        noise: f32,
        blur: f32,
        line_mm: f64,
        texture: f32,
        shade: f32,
        height: f64,
    }

    const LOOK: Look = Look {
        noise: 2.0,
        blur: 0.8,
        line_mm: 0.30,
        texture: 3.0,
        shade: 25.0,
        height: 0.0,
    };

    fn mark_dark(layout: &MarkLayout, x: f64, y: f64, half_w: f64) -> bool {
        let a = layout.arm_mm;
        for (i, c) in layout.marks.iter().enumerate() {
            let (dx, dy) = (x - c.x, y - c.y);
            if dx.abs() > a + 1.0 || dy.abs() > a + 1.0 {
                continue;
            }
            if (dy.abs() <= half_w && dx.abs() <= a) || (dx.abs() <= half_w && dy.abs() <= a) {
                return true;
            }
            if i == 0 && (dx.hypot(dy) - layout.ring_mm).abs() <= half_w {
                return true;
            }
        }
        false
    }

    /// Foto cruda (con distorsión de lente) de las marcas grabadas sobre un cartón.
    fn render(
        cam: &LensCalibration,
        pose: &BedPose,
        layout: &MarkLayout,
        look: Look,
        seed: u64,
    ) -> GrayImage {
        let (w, h) = (cam.width as usize, cam.height as usize);
        let paper = |u: f64, v: f64| -> f32 {
            // Cartón con un degradado de iluminación (viñeteo) suave.
            let (nx, ny) = ((u - 320.0) / 320.0, (v - 240.0) / 240.0);
            215.0 - look.shade * (nx * nx + ny * ny) as f32 * 0.5 + look.shade * 0.3 * nx as f32
        };
        let n = 4;
        let mut data = vec![0.0f32; w * h];
        // Rayo (x, y, 1) con el plano a altura `look.height`.
        let inv = {
            let r = pose.r;
            let t = pose.t + r.column(2) * look.height;
            Matrix3::from_columns(&[r.column(0).into_owned(), r.column(1).into_owned(), t])
                .try_inverse()
                .unwrap()
        };
        for j in 0..h {
            for i in 0..w {
                let mut dark = 0.0;
                for sj in 0..n {
                    for si in 0..n {
                        let u = i as f64 - 0.5 + (si as f64 + 0.5) / n as f64;
                        let v = j as f64 - 0.5 + (sj as f64 + 0.5) / n as f64;
                        let (xd, yd) = ((u - cam.cx) / cam.fx, (v - cam.cy) / cam.fy);
                        let (x, y) = cam.undistort_normalized(xd, yd);
                        let q = inv * Vector3::new(x, y, 1.0);
                        let (bx, by) = (q.x / q.z, q.y / q.z);
                        if mark_dark(layout, bx, by, look.line_mm / 2.0) {
                            dark += 1.0;
                        }
                    }
                }
                let frac = dark / (n * n) as f32;
                let base = paper(i as f64, j as f64);
                data[j * w + i] = base * (1.0 - frac) + 55.0 * frac;
            }
        }
        let blurred = gaussian_blur(&FloatImage { w, h, data }, look.blur);
        let mut rng = Rng::new(seed);
        // Textura de fibras: ruido gaussiano de baja frecuencia más ruido de sensor.
        let mut tex = FloatImage {
            w,
            h,
            data: (0..w * h).map(|_| rng.gauss() as f32).collect(),
        };
        tex = gaussian_blur(&tex, 1.5);
        let pixels: Vec<u8> = blurred
            .data
            .iter()
            .zip(&tex.data)
            .map(|(&v, &tx)| {
                (v + look.texture * 6.0 * tx + look.noise * rng.gauss() as f32)
                    .round()
                    .clamp(0.0, 255.0) as u8
            })
            .collect();
        GrayImage::from_raw(cam.width, cam.height, pixels).unwrap()
    }

    fn corrected(cam: &LensCalibration, raw: &GrayImage) -> GrayImage {
        UndistortMap::new(cam, (cam.width, cam.height))
            .unwrap()
            .undistort(raw)
            .unwrap()
    }

    /// Error máximo y rms (mm) de la alineación estimada frente a la verdad, en una rejilla.
    fn alignment_error(
        cam: &LensCalibration,
        pose: &BedPose,
        h: f64,
        to_bed: &Homography,
    ) -> (f64, f64) {
        let truth = truth_h(cam, pose, h);
        let mut worst: f64 = 0.0;
        let mut sum = 0.0;
        let mut n = 0.0;
        for j in 0..=8 {
            for i in 0..=12 {
                let bed = Point::new(i as f64 * 25.0, j as f64 * 25.0);
                let px = truth.apply(bed);
                let e = to_bed.apply(px).dist(bed);
                worst = worst.max(e);
                sum += e * e;
                n += 1.0;
            }
        }
        (worst, (sum / n).sqrt())
    }

    fn rep_to_bed(r: &MarksReport) -> Homography {
        Homography { m: r.to_bed }
    }

    #[test]
    fn la_disposicion_cubre_el_85_por_ciento_y_tiene_orden_fijo() {
        let l4 = MarkLayout::new(BED, 4).unwrap();
        assert_eq!(l4.marks.len(), 4);
        assert!((l4.marks[0].x - 22.5).abs() < 1e-9 && (l4.marks[2].x - 277.5).abs() < 1e-9);
        assert!((l4.marks[2].x - l4.marks[0].x - 0.85 * 300.0).abs() < 1e-9);
        let l9 = MarkLayout::new(BED, 9).unwrap();
        assert_eq!(l9.marks.len(), 9);
        assert_eq!(l9.marks[..4], l4.marks[..]);
        assert!((l9.marks[8].x - 150.0).abs() < 1e-9 && (l9.marks[8].y - 100.0).abs() < 1e-9);
        // Dos trazos por cruz y un círculo (solo en la marca 0).
        let s = l9.strokes();
        assert_eq!(s.len(), 9 * 2 + 1);
        assert_eq!(s.iter().filter(|x| x.closed).count(), 1);
        assert!(MarkLayout::new(BED, 5).is_err());
        assert!(MarkLayout::new([30.0, 200.0], 4).is_err());
    }

    #[test]
    fn detecta_9_marcas_con_alineacion_provisional_y_mide_la_precision() {
        let cam = truth_camera();
        let pose = std_pose();
        let layout = MarkLayout::new(BED, 9).unwrap();
        let img = corrected(&cam, &render(&cam, &pose, &layout, LOOK, 11));
        // Alineación provisional con error: 8 px de desplazamiento y 2 % de escala.
        let truth = truth_h(&cam, &pose, 0.0);
        let wobble = Homography {
            m: [[1.02, 0.0, 8.0], [0.0, 1.02, -6.0], [0.0, 0.0, 1.0]],
        };
        let prior = truth.then(&wobble).inverse().unwrap();
        let rep = detect_marks(
            &img,
            &layout,
            &DetectOptions {
                prior_to_bed: Some(prior),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(rep.used_prior);
        assert_eq!((rep.found, rep.used), (9, 9));
        let (worst, rms) = alignment_error(&cam, &pose, 0.0, &rep_to_bed(&rep));
        eprintln!(
            "9 marcas: ajuste {:.3} mm, LOO rms {:?} max {:?}, error real max {worst:.3} rms {rms:.3}",
            rep.rms_mm, rep.loo_rms_mm, rep.loo_max_mm
        );
        assert!(worst < 0.35, "error real máximo {worst:.3} mm");
        assert!(rms < 0.2, "error real rms {rms:.3} mm");
        let loo = rep.loo_rms_mm.expect("con 9 marcas hay validación cruzada");
        assert!(loo < 0.35, "LOO rms {loo:.3}");
        // La precisión medida es honesta: no subestima el error real por un factor grande.
        assert!(loo > rms * 0.4, "LOO {loo:.3} vs real {rms:.3}");
    }

    #[test]
    fn detecta_sin_alineacion_previa_y_resuelve_la_orientacion() {
        let cam = truth_camera();
        let layout = MarkLayout::new(BED, 9).unwrap();
        // Normal y girada 180 grados (cámara montada al revés): el círculo fija el origen.
        for (rot_z, dist, seed) in [
            (3.0, 345.0, 21u64),
            (183.0, 345.0, 22u64),
            (95.0, 480.0, 23u64),
        ] {
            let pose = bed_pose((6.0, 4.0, rot_z), (0.0, 0.0, dist));
            let img = corrected(&cam, &render(&cam, &pose, &layout, LOOK, seed));
            let rep = detect_marks(&img, &layout, &DetectOptions::default()).unwrap();
            assert!(!rep.used_prior);
            assert_eq!(rep.used, 9, "rot {rot_z}: {:?}", rep.warnings);
            let (worst, rms) = alignment_error(&cam, &pose, 0.0, &rep_to_bed(&rep));
            eprintln!(
                "sin alineación previa, giro {rot_z}: error real máx {worst:.3} rms {rms:.3} mm; LOO rms {:?}",
                rep.loo_rms_mm
            );
            assert!(worst < 0.4, "rot {rot_z}: error {worst:.3} mm");
        }
    }

    #[test]
    fn tolera_una_marca_ausente() {
        let cam = truth_camera();
        let pose = std_pose();
        let full = MarkLayout::new(BED, 9).unwrap();
        // Se borra la marca 6 (centro superior) de la foto: no se graba.
        let mut partial = full.clone();
        partial.marks[6] = Point::new(-500.0, -500.0);
        let img = corrected(&cam, &render(&cam, &pose, &partial, LOOK, 31));
        let rep = detect_marks(&img, &full, &DetectOptions::default()).unwrap();
        assert_eq!(rep.found, 8, "{:?}", rep.warnings);
        assert!(rep.marks[6].image.is_none());
        let (worst, _) = alignment_error(&cam, &pose, 0.0, &rep_to_bed(&rep));
        assert!(worst < 0.4, "error {worst:.3} mm");
        assert!(rep.warnings.iter().any(|w| w.contains("8 de 9")));
    }

    #[test]
    fn con_4_marcas_funciona_pero_avisa_que_no_mide_la_precision() {
        let cam = truth_camera();
        let pose = std_pose();
        let layout = MarkLayout::new(BED, 4).unwrap();
        let img = corrected(&cam, &render(&cam, &pose, &layout, LOOK, 41));
        let rep = detect_marks(&img, &layout, &DetectOptions::default()).unwrap();
        assert_eq!(rep.used, 4);
        assert!(rep.loo_rms_mm.is_none() && rep.loo_max_mm.is_none());
        let (worst, rms) = alignment_error(&cam, &pose, 0.0, &rep_to_bed(&rep));
        eprintln!("4 marcas: error real máx {worst:.3} rms {rms:.3} mm");
        // Con 4 marcas el centro de la cama queda más lejos de los puntos de ajuste.
        assert!(worst < 0.8, "error {worst:.3} mm");
    }

    #[test]
    fn una_marca_falsa_cerca_de_una_cruz_se_descarta_con_ransac() {
        let cam = truth_camera();
        let pose = std_pose();
        let layout = MarkLayout::new(BED, 9).unwrap();
        let img = corrected(&cam, &render(&cam, &pose, &layout, LOOK, 51));
        let truth = truth_h(&cam, &pose, 0.0);
        // Alineación provisional muy buena; se estropea una cruz dibujando una mancha oscura
        // grande al lado, que mueve su centro varios mm.
        let mut img = img;
        let spot = truth.apply(Point::new(layout.marks[5].x + 4.0, layout.marks[5].y + 3.0));
        for dy in -5i32..=5 {
            for dx in -5i32..=5 {
                if dx * dx + dy * dy <= 25 {
                    let (x, y) = ((spot.x as i32 + dx) as u32, (spot.y as i32 + dy) as u32);
                    img.put_pixel(x, y, image::Luma([40]));
                }
            }
        }
        let rep = detect_marks(
            &img,
            &layout,
            &DetectOptions {
                prior_to_bed: Some(truth.inverse().unwrap()),
                ..Default::default()
            },
        )
        .unwrap();
        // Con la marca 5 estropeada, el ajuste final no debe empeorar.
        let (worst, _) = alignment_error(&cam, &pose, 0.0, &rep_to_bed(&rep));
        assert!(worst < 0.5, "error {worst:.3} mm; usadas {}", rep.used);
    }

    #[test]
    fn sin_marcas_da_un_error_claro() {
        let blank = GrayImage::from_pixel(640, 480, image::Luma([180]));
        let layout = MarkLayout::new(BED, 9).unwrap();
        let e = detect_marks(&blank, &layout, &DetectOptions::default()).unwrap_err();
        assert!(e.contains("marcas"), "{e}");
    }

    #[test]
    fn el_desplazamiento_de_pixel_se_aplica_a_la_salida() {
        let cam = truth_camera();
        let pose = std_pose();
        let layout = MarkLayout::new(BED, 9).unwrap();
        let img = corrected(&cam, &render(&cam, &pose, &layout, LOOK, 61));
        let a = detect_marks(&img, &layout, &DetectOptions::default()).unwrap();
        let b = detect_marks(
            &img,
            &layout,
            &DetectOptions {
                pixel_offset: 0.5,
                ..Default::default()
            },
        )
        .unwrap();
        let (pa, pb) = (a.marks[8].image.unwrap(), b.marks[8].image.unwrap());
        assert!((pb[0] - pa[0] - 0.5).abs() < 1e-9 && (pb[1] - pa[1] - 0.5).abs() < 1e-9);
        // Las dos alineaciones describen lo mismo en sus coordenadas.
        let (wa, _) = alignment_error(&cam, &pose, 0.0, &rep_to_bed(&a));
        assert!(wa < 0.4);
    }

    /// De la foto a dos alturas a la corrección: lo que usará la aplicación.
    #[test]
    fn dos_alturas_de_punta_a_punta_corrigen_el_paralaje() {
        let cam = truth_camera();
        let pose = std_pose();
        let layout = MarkLayout::new(BED, 9).unwrap();
        let mut samples = Vec::new();
        for (k, h) in [0.0, 20.0].into_iter().enumerate() {
            let look = Look { height: h, ..LOOK };
            let img = corrected(&cam, &render(&cam, &pose, &layout, look, 70 + k as u64));
            let rep = detect_marks(&img, &layout, &DetectOptions::default()).unwrap();
            assert_eq!(rep.used, 9, "h {h}: {:?}", rep.warnings);
            let (image_px, bed_mm): (Vec<_>, Vec<_>) = rep
                .marks
                .iter()
                .filter(|m| m.used)
                .map(|m| {
                    let i = m.image.unwrap();
                    (Point::new(i[0], i[1]), Point::new(m.bed[0], m.bed[1]))
                })
                .unzip();
            samples.push(HeightSample {
                height_mm: h,
                image_px,
                bed_mm,
            });
        }
        let fit = fit_height_model(&samples).unwrap();
        // Alturas intermedias (no medidas): 5, 10 y 15 mm.
        let mut worst_corrected: f64 = 0.0;
        let mut worst_plain: f64 = 0.0;
        let at0 = fit.model.image_to_bed_at(0.0).unwrap();
        for h in [0.0, 5.0, 10.0, 15.0, 20.0] {
            let truth = truth_h(&cam, &pose, h);
            let model = fit.model.image_to_bed_at(h).unwrap();
            for j in 0..=8 {
                for i in 0..=12 {
                    let bed = Point::new(i as f64 * 25.0, j as f64 * 25.0);
                    let px = truth.apply(bed);
                    worst_corrected = worst_corrected.max(model.apply(px).dist(bed));
                    worst_plain = worst_plain.max(at0.apply(px).dist(bed));
                }
            }
        }
        eprintln!(
            "altura: error máx corregido {worst_corrected:.3} mm; sin corregir (alineación a 0 mm) {worst_plain:.3} mm"
        );
        assert!(worst_corrected < 0.6, "corregido {worst_corrected:.3} mm");
        assert!(worst_plain > 3.0, "sin corregir {worst_plain:.3} mm");
    }
}
