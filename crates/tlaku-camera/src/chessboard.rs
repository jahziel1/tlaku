//! Detección de tablero de ajedrez (esquinas internas) en imágenes en grises.
//!
//! Algoritmo:
//! 1. Respuesta de punto de silla: `Ixy^2 - Ixx*Iyy` sobre la imagen suavizada
//!    (sigma 2); máximos locales por encima de un umbral relativo.
//! 2. Cada candidato se valida con las dos direcciones de borde (histograma de
//!    orientación del gradiente) y con el patrón de cuatro sectores alternados
//!    (descarta esquinas en L y uniones en T, como los bordes exteriores del tablero).
//! 3. Agrupación en rejilla: desde una semilla se crece por vecinos alineados con
//!    las direcciones de borde, prediciendo cada paso por extrapolación (tolera
//!    perspectiva y distorsión de barril moderadas).
//! 4. Se exige un rectángulo completo de `cols x rows` esquinas; se ordena por filas.
//! 5. Refinamiento subpíxel de Förstner: el punto que minimiza la distancia a las
//!    rectas de borde ponderada por el gradiente.
//!
//! Orden: fila por fila (`cols` esquinas por fila), con ejes `x` hacia las columnas
//! e `y` hacia las filas, orientación directa en la imagen (Y hacia abajo). El
//! origen se fija así: entre las rotaciones posibles se elige la que deja el
//! cuadrado interior contiguo al origen (hacia +x, +y) oscuro; esto elimina la
//! ambigüedad de 180 grados si `cols + rows` es impar (por ejemplo 9x6). Si es par
//! queda la ambigüedad y se prefiere el eje x apuntando más a la derecha; use un
//! tablero con `cols + rows` impar para calibrar.

use crate::imgproc::{FloatImage, gaussian_blur};
use image::GrayImage;
use std::collections::HashMap;
use std::f64::consts::PI;
use tlaku_core::Point;

/// Sigma del suavizado para la respuesta de silla (píxeles).
const SADDLE_SIGMA: f32 = 2.0;
/// Margen al borde de la imagen donde no se buscan esquinas.
const BORDER_MARGIN: usize = 10;
/// Radio de la ventana de orientación (píxeles).
const ORIENT_RADIUS: i32 = 7;
/// Radio de muestreo de los cuatro sectores (píxeles).
const SECTOR_RADIUS: f64 = 4.0;
/// Contraste mínimo entre sectores (niveles de gris).
const MIN_CONTRAST: f32 = 8.0;
/// Tolerancia angular de alineación con las direcciones de borde.
const ALIGN_TOL: f64 = 28.0 * PI / 180.0;
const MAX_CANDIDATES: usize = 1500;
const MAX_SEEDS: usize = 60;

fn not_found(board: [u32; 2]) -> String {
    format!(
        "no se encontró el tablero de {}x{} esquinas; verifica la iluminación y que se vea completo",
        board[0], board[1]
    )
}

#[derive(Debug, Clone)]
struct Cand {
    x: f64,
    y: f64,
    strength: f32,
    /// Dos direcciones de borde (vectores unitarios, sin sentido).
    dirs: [[f64; 2]; 2],
}

/// Busca el tablero de `board = [cols, rows]` esquinas internas y devuelve las
/// `cols * rows` esquinas en píxeles (subpíxel), fila por fila.
pub fn find_chessboard(img: &GrayImage, board: [u32; 2]) -> Result<Vec<Point>, String> {
    let (cols, rows) = (board[0] as usize, board[1] as usize);
    if cols < 2 || rows < 2 || cols.max(rows) < 3 {
        return Err(format!(
            "el tablero debe tener al menos 2x3 esquinas internas (se pidió {cols}x{rows})"
        ));
    }
    let (w, h) = (img.width() as usize, img.height() as usize);
    if w < 3 * BORDER_MARGIN || h < 3 * BORDER_MARGIN {
        return Err(format!("la imagen es demasiado pequeña ({w}x{h})"));
    }
    let base = FloatImage::from_gray(img);
    let l1 = gaussian_blur(&base, 1.0);
    let l2 = gaussian_blur(&base, SADDLE_SIGMA);

    let cands = candidates(&l1, &l2);
    if cands.len() < cols * rows {
        return Err(not_found(board));
    }

    // Semillas por intensidad de respuesta; se crece una rejilla desde cada una.
    let mut order: Vec<usize> = (0..cands.len()).collect();
    order.sort_by(|&a, &b| cands[b].strength.total_cmp(&cands[a].strength));
    let mut too_big = false;
    for &seed in order.iter().take(MAX_SEEDS) {
        let Some(labels) = grow(&cands, seed) else {
            continue;
        };
        if labels.len() < cols * rows {
            continue;
        }
        match pick_rectangle(&labels, cols as i32, rows as i32) {
            Rect::None => {}
            Rect::Ambiguous => too_big = true,
            Rect::One(grid) => {
                let pts = order_and_refine(&l1, &cands, &grid, cols, rows);
                return Ok(pts);
            }
        }
    }
    if too_big {
        return Err(format!(
            "se detectó una cuadrícula mayor que {}x{} esquinas; revisa el tamaño del tablero indicado",
            board[0], board[1]
        ));
    }
    Err(not_found(board))
}

// ---------------------------------------------------------------------------
// Candidatos
// ---------------------------------------------------------------------------

fn candidates(l1: &FloatImage, l2: &FloatImage) -> Vec<Cand> {
    let (w, h) = (l2.w, l2.h);
    let mut resp = vec![0.0f32; w * h];
    let mut max_r = 0.0f32;
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let c = l2.get(x, y);
            let ixx = l2.get(x + 1, y) - 2.0 * c + l2.get(x - 1, y);
            let iyy = l2.get(x, y + 1) - 2.0 * c + l2.get(x, y - 1);
            let ixy = (l2.get(x + 1, y + 1) - l2.get(x - 1, y + 1) - l2.get(x + 1, y - 1)
                + l2.get(x - 1, y - 1))
                * 0.25;
            let r = ixy * ixy - ixx * iyy;
            resp[y * w + x] = r;
            max_r = max_r.max(r);
        }
    }
    let thr = (0.005 * max_r).max(1.0);
    let mut out = Vec::new();
    const NMS: usize = 3;
    for y in BORDER_MARGIN..h - BORDER_MARGIN {
        for x in BORDER_MARGIN..w - BORDER_MARGIN {
            let r = resp[y * w + x];
            if r <= thr {
                continue;
            }
            let mut is_max = true;
            'win: for yy in y - NMS..=y + NMS {
                for xx in x - NMS..=x + NMS {
                    if (xx, yy) == (x, y) {
                        continue;
                    }
                    let o = resp[yy * w + xx];
                    // Desempate por orden de barrido para no duplicar mesetas.
                    let earlier = (yy, xx) < (y, x);
                    if o > r || (o == r && earlier) {
                        is_max = false;
                        break 'win;
                    }
                }
            }
            if !is_max {
                continue;
            }
            let Some(dirs) = edge_directions(l1, x, y) else {
                continue;
            };
            if !cross_pattern(l1, x as f64, y as f64, &dirs) {
                continue;
            }
            out.push(Cand {
                x: x as f64,
                y: y as f64,
                strength: r,
                dirs,
            });
        }
    }
    out.sort_by(|a, b| b.strength.total_cmp(&a.strength));
    out.truncate(MAX_CANDIDATES);
    out
}

/// Diferencia angular con signo entre rectas, en (-pi/2, pi/2].
fn line_diff(a: f64, b: f64) -> f64 {
    (a - b + PI / 2.0).rem_euclid(PI) - PI / 2.0
}

/// Las dos direcciones de borde dominantes alrededor de `(cx, cy)`.
fn edge_directions(l: &FloatImage, cx: usize, cy: usize) -> Option<[[f64; 2]; 2]> {
    const BINS: usize = 36;
    let r = ORIENT_RADIUS;
    let mut samples: Vec<(f64, f64)> = Vec::with_capacity(200);
    let mut max_mag = 0.0f64;
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy > r * r {
                continue;
            }
            let x = (cx as i32 + dx) as usize;
            let y = (cy as i32 + dy) as usize;
            let gx = f64::from(l.get(x + 1, y) - l.get(x - 1, y)) * 0.5;
            let gy = f64::from(l.get(x, y + 1) - l.get(x, y - 1)) * 0.5;
            let mag = gx.hypot(gy);
            max_mag = max_mag.max(mag);
            // El borde es perpendicular al gradiente.
            let ang = (gy.atan2(gx) + PI / 2.0).rem_euclid(PI);
            samples.push((ang, mag));
        }
    }
    if max_mag < 1.5 {
        return None;
    }
    let keep: Vec<(f64, f64)> = samples
        .into_iter()
        .filter(|s| s.1 > 0.25 * max_mag)
        .collect();
    let mut hist = [0.0f64; BINS];
    for &(a, m) in &keep {
        let b = ((a / PI * BINS as f64) as usize).min(BINS - 1);
        hist[b] += m;
    }
    let mut sm = [0.0f64; BINS];
    for i in 0..BINS {
        sm[i] = (hist[(i + BINS - 1) % BINS] + 2.0 * hist[i] + hist[(i + 1) % BINS]) * 0.25;
    }
    let p1 = (0..BINS).max_by(|&a, &b| sm[a].total_cmp(&sm[b]))?;
    let circ = |a: usize, b: usize| {
        let d = a.abs_diff(b);
        d.min(BINS - d)
    };
    let p2 = (0..BINS)
        .filter(|&i| circ(i, p1) >= 4)
        .max_by(|&a, &b| sm[a].total_cmp(&sm[b]))?;
    if sm[p2] < 0.3 * sm[p1] {
        return None;
    }
    let bin_angle = |i: usize| (i as f64 + 0.5) * PI / BINS as f64;
    let refine = |a0: f64| {
        let (mut sw, mut so) = (0.0, 0.0);
        for &(a, m) in &keep {
            let d = line_diff(a, a0);
            if d.abs() < 25.0 * PI / 180.0 {
                sw += m;
                so += m * d;
            }
        }
        if sw > 0.0 { a0 + so / sw } else { a0 }
    };
    let a1 = refine(bin_angle(p1));
    let a2 = refine(bin_angle(p2));
    let sep = line_diff(a1, a2).abs();
    if sep < 30.0 * PI / 180.0 {
        return None;
    }
    let d1 = [a1.cos(), a1.sin()];
    let mut d2 = [a2.cos(), a2.sin()];
    if d1[0] * d2[0] + d1[1] * d2[1] < 0.0 {
        d2 = [-d2[0], -d2[1]];
    }
    Some([d1, d2])
}

/// Comprueba el patrón de cuatro sectores alternados alrededor de una esquina.
fn cross_pattern(l: &FloatImage, x: f64, y: f64, dirs: &[[f64; 2]; 2]) -> bool {
    let [d1, d2] = *dirs;
    let norm = |v: [f64; 2]| {
        let n = v[0].hypot(v[1]).max(1e-12);
        [v[0] / n, v[1] / n]
    };
    let b1 = norm([d1[0] + d2[0], d1[1] + d2[1]]);
    let b2 = norm([d1[0] - d2[0], d1[1] - d2[1]]);
    let at = |b: [f64; 2], s: f64| {
        f64::from(l.sample(x + s * SECTOR_RADIUS * b[0], y + s * SECTOR_RADIUS * b[1]))
    };
    let (a_p, a_m) = (at(b1, 1.0), at(b1, -1.0));
    let (b_p, b_m) = (at(b2, 1.0), at(b2, -1.0));
    let s = (a_p + a_m) * 0.5 - (b_p + b_m) * 0.5;
    s.abs() > f64::from(MIN_CONTRAST)
        && (a_p - a_m).abs() < 0.5 * s.abs()
        && (b_p - b_m).abs() < 0.5 * s.abs()
}

// ---------------------------------------------------------------------------
// Agrupación en rejilla
// ---------------------------------------------------------------------------

/// Ángulo entre el vector `v` y la recta `d`, en [0, pi/2].
fn line_angle(v: (f64, f64), d: [f64; 2]) -> f64 {
    let n = v.0.hypot(v.1);
    if n < 1e-9 {
        return PI / 2.0;
    }
    ((v.0 * d[0] + v.1 * d[1]).abs() / n).min(1.0).acos()
}

fn aligned(c: &Cand, v: (f64, f64)) -> bool {
    line_angle(v, c.dirs[0]) < ALIGN_TOL || line_angle(v, c.dirs[1]) < ALIGN_TOL
}

type Labels = HashMap<(i32, i32), usize>;

struct Node {
    idx: usize,
    u: i32,
    v: i32,
    bu: (f64, f64),
    bv: (f64, f64),
}

/// Crece una rejilla de etiquetas enteras `(u, v)` a partir de una semilla.
fn grow(c: &[Cand], seed: usize) -> Option<Labels> {
    let p = &c[seed];
    let mut best: [Option<(f64, usize)>; 2] = [None, None];
    for (j, q) in c.iter().enumerate() {
        if j == seed {
            continue;
        }
        let v = (q.x - p.x, q.y - p.y);
        let d = v.0.hypot(v.1);
        if d < 4.0 || !aligned(q, v) {
            continue;
        }
        for (k, slot) in best.iter_mut().enumerate() {
            if line_angle(v, p.dirs[k]) < ALIGN_TOL && slot.is_none_or(|b| d < b.0) {
                *slot = Some((d, j));
            }
        }
    }
    let (da, ja) = best[0]?;
    let (db, jb) = best[1]?;
    if da.max(db) > 1.8 * da.min(db) {
        return None;
    }
    let a = (c[ja].x - p.x, c[ja].y - p.y);
    let b = (c[jb].x - p.x, c[jb].y - p.y);
    let det = a.0 * b.1 - a.1 * b.0;
    let (lab_b, bv) = if det > 0.0 {
        ((0, 1), b)
    } else {
        ((0, -1), (-b.0, -b.1))
    };

    let mut labels: Labels = HashMap::new();
    let mut used = vec![false; c.len()];
    let mut queue = std::collections::VecDeque::new();
    for (idx, lab) in [(seed, (0, 0)), (ja, (1, 0)), (jb, lab_b)] {
        labels.insert(lab, idx);
        used[idx] = true;
        queue.push_back(Node {
            idx,
            u: lab.0,
            v: lab.1,
            bu: a,
            bv,
        });
    }
    while let Some(n) = queue.pop_front() {
        let np = (c[n.idx].x, c[n.idx].y);
        for (du, dv) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
            if labels.contains_key(&(n.u + du, n.v + dv)) {
                continue;
            }
            let step = match labels.get(&(n.u - du, n.v - dv)) {
                Some(&pi) => (np.0 - c[pi].x, np.1 - c[pi].y),
                None => (
                    f64::from(du) * n.bu.0 + f64::from(dv) * n.bv.0,
                    f64::from(du) * n.bu.1 + f64::from(dv) * n.bv.1,
                ),
            };
            let slen = step.0.hypot(step.1);
            let pred = (np.0 + step.0, np.1 + step.1);
            let tol = 0.33 * slen;
            let mut found: Option<(f64, usize)> = None;
            for (j, q) in c.iter().enumerate() {
                if used[j] {
                    continue;
                }
                let d = (q.x - pred.0).hypot(q.y - pred.1);
                if d < tol && found.is_none_or(|f| d < f.0) {
                    found = Some((d, j));
                }
            }
            let Some((_, j)) = found else { continue };
            let v = (c[j].x - np.0, c[j].y - np.1);
            if !aligned(&c[n.idx], v) || !aligned(&c[j], v) {
                continue;
            }
            used[j] = true;
            let lab = (n.u + du, n.v + dv);
            labels.insert(lab, j);
            let (mut bu, mut bv) = (n.bu, n.bv);
            if du != 0 {
                bu = (v.0 * f64::from(du), v.1 * f64::from(du));
            } else {
                bv = (v.0 * f64::from(dv), v.1 * f64::from(dv));
            }
            queue.push_back(Node {
                idx: j,
                u: lab.0,
                v: lab.1,
                bu,
                bv,
            });
        }
    }
    Some(labels)
}

enum Rect {
    None,
    /// Hay más de una posición posible: la cuadrícula detectada es mayor.
    Ambiguous,
    /// Rectángulo único: `(nu, nv, origen_u, origen_v)` con las etiquetas.
    One(RectGrid),
}

struct RectGrid {
    nu: i32,
    nv: i32,
    /// Índices de candidato por `(a, b)` con `a` en `0..nu`, `b` en `0..nv`.
    cells: Vec<usize>,
}

fn pick_rectangle(labels: &Labels, cols: i32, rows: i32) -> Rect {
    let (mut umin, mut umax, mut vmin, mut vmax) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
    for &(u, v) in labels.keys() {
        umin = umin.min(u);
        umax = umax.max(u);
        vmin = vmin.min(v);
        vmax = vmax.max(v);
    }
    let mut dims = vec![(cols, rows)];
    if cols != rows {
        dims.push((rows, cols));
    }
    let mut found: Vec<RectGrid> = Vec::new();
    for (nu, nv) in dims {
        for u0 in umin..=umax - nu + 1 {
            for v0 in vmin..=vmax - nv + 1 {
                let mut cells = Vec::with_capacity((nu * nv) as usize);
                let mut ok = true;
                'scan: for b in 0..nv {
                    for a in 0..nu {
                        match labels.get(&(u0 + a, v0 + b)) {
                            Some(&i) => cells.push(i),
                            None => {
                                ok = false;
                                break 'scan;
                            }
                        }
                    }
                }
                if ok {
                    found.push(RectGrid { nu, nv, cells });
                }
            }
        }
    }
    match found.len() {
        0 => Rect::None,
        1 => Rect::One(found.pop().unwrap()),
        _ => Rect::Ambiguous,
    }
}

// ---------------------------------------------------------------------------
// Orden, origen estable y refinamiento
// ---------------------------------------------------------------------------

fn order_and_refine(
    l1: &FloatImage,
    cands: &[Cand],
    grid: &RectGrid,
    cols: usize,
    rows: usize,
) -> Vec<Point> {
    let (nu, nv) = (grid.nu as usize, grid.nv as usize);
    let pos = |a: usize, b: usize| {
        let c = &cands[grid.cells[b * nu + a]];
        (c.x, c.y)
    };
    // Las cuatro rotaciones propias de la rejilla: (x, y) en función de (a, b).
    // Cada una produce una lista fila por fila de `cols x rows`.
    let mut options: Vec<Vec<(f64, f64)>> = Vec::new();
    for rot in 0..4 {
        let (dx, dy) = if rot % 2 == 0 { (nu, nv) } else { (nv, nu) };
        if (dx, dy) != (cols, rows) {
            continue;
        }
        let mut pts = Vec::with_capacity(cols * rows);
        for y in 0..dy {
            for x in 0..dx {
                let (a, b) = match rot {
                    0 => (x, y),
                    1 => (y, nv - 1 - x),
                    2 => (nu - 1 - x, nv - 1 - y),
                    _ => (nu - 1 - y, x),
                };
                pts.push(pos(a, b));
            }
        }
        options.push(pts);
    }

    let square_brightness = |p: &[(f64, f64)], i: usize, j: usize| -> f64 {
        // Centro del cuadrado cuyo vértice (i, j) es el menor: media de sus cuatro esquinas.
        let g = |x: usize, y: usize| p[y * cols + x];
        let cs = [g(i, j), g(i + 1, j), g(i, j + 1), g(i + 1, j + 1)];
        let cx = cs.iter().map(|c| c.0).sum::<f64>() / 4.0;
        let cy = cs.iter().map(|c| c.1).sum::<f64>() / 4.0;
        let ex = ((cs[1].0 - cs[0].0) + (cs[3].0 - cs[2].0)) * 0.12;
        let ey = ((cs[1].1 - cs[0].1) + (cs[3].1 - cs[2].1)) * 0.12;
        let fx = ((cs[2].0 - cs[0].0) + (cs[3].0 - cs[1].0)) * 0.12;
        let fy = ((cs[2].1 - cs[0].1) + (cs[3].1 - cs[1].1)) * 0.12;
        let offs = [(0.0, 0.0), (ex, ey), (-ex, -ey), (fx, fy), (-fx, -fy)];
        offs.iter()
            .map(|o| f64::from(l1.sample(cx + o.0, cy + o.1)))
            .sum::<f64>()
            / 5.0
    };
    let score = |p: &[(f64, f64)]| -> (bool, f64) {
        let own = square_brightness(p, 0, 0);
        let neighbor = if cols >= 3 {
            square_brightness(p, 1, 0)
        } else {
            square_brightness(p, 0, 1)
        };
        let dark = neighbor - own > 5.0;
        (dark, p[1].0 - p[0].0)
    };
    let mut best = 0;
    let mut best_score = score(&options[0]);
    for (i, o) in options.iter().enumerate().skip(1) {
        let s = score(o);
        if (s.0, s.1) > (best_score.0, best_score.1) {
            best = i;
            best_score = s;
        }
    }
    let ordered = &options[best];

    // Refinamiento subpíxel con ventana proporcional al espaciado local.
    let at = |x: usize, y: usize| ordered[y * cols + x];
    let mut out = Vec::with_capacity(cols * rows);
    for y in 0..rows {
        for x in 0..cols {
            let p = at(x, y);
            let mut sp = Vec::new();
            for (nx, ny) in [
                (x + 1, y),
                (x.wrapping_sub(1), y),
                (x, y + 1),
                (x, y.wrapping_sub(1)),
            ] {
                if nx < cols && ny < rows {
                    let q = at(nx, ny);
                    sp.push((q.0 - p.0).hypot(q.1 - p.1));
                }
            }
            let spacing = sp.iter().sum::<f64>() / sp.len().max(1) as f64;
            let radius = (0.3 * spacing).clamp(3.0, 16.0);
            let (rx, ry) = refine_corner(l1, p, radius);
            out.push(Point::new(rx, ry));
        }
    }
    out
}

/// Refinamiento de Förstner: mínimos cuadrados ponderados de la distancia del
/// punto a las rectas de borde definidas por el gradiente en la ventana.
fn refine_corner(l: &FloatImage, start: (f64, f64), radius: f64) -> (f64, f64) {
    let mut cur = start;
    let r = radius.ceil() as i32;
    let sigma2 = (radius * 0.5).powi(2);
    for _ in 0..12 {
        let (mut a11, mut a12, mut a22, mut b1, mut b2) = (0.0, 0.0, 0.0, 0.0, 0.0);
        let (ix, iy) = (cur.0.round() as i32, cur.1.round() as i32);
        for dy in -r..=r {
            for dx in -r..=r {
                let (px, py) = (ix + dx, iy + dy);
                if px < 1 || py < 1 || px as usize >= l.w - 1 || py as usize >= l.h - 1 {
                    continue;
                }
                let d2 = (f64::from(px) - cur.0).powi(2) + (f64::from(py) - cur.1).powi(2);
                if d2 > radius * radius {
                    continue;
                }
                let (x, y) = (px as usize, py as usize);
                let gx = f64::from(l.get(x + 1, y) - l.get(x - 1, y)) * 0.5;
                let gy = f64::from(l.get(x, y + 1) - l.get(x, y - 1)) * 0.5;
                let w = (-d2 / (2.0 * sigma2)).exp();
                let (xx, xy, yy) = (gx * gx * w, gx * gy * w, gy * gy * w);
                a11 += xx;
                a12 += xy;
                a22 += yy;
                b1 += xx * f64::from(px) + xy * f64::from(py);
                b2 += xy * f64::from(px) + yy * f64::from(py);
            }
        }
        let det = a11 * a22 - a12 * a12;
        if det.abs() < 1e-9 {
            return start;
        }
        let nx = (a22 * b1 - a12 * b2) / det;
        let ny = (a11 * b2 - a12 * b1) / det;
        if (nx - start.0).hypot(ny - start.1) > radius {
            return start;
        }
        let moved = (nx - cur.0).hypot(ny - cur.1);
        cur = (nx, ny);
        if moved < 1e-3 {
            break;
        }
    }
    cur
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    const BOARD: [u32; 2] = [9, 6];
    const SQ: f64 = 25.0;

    fn max_err(a: &[Point], b: &[Point]) -> f64 {
        a.iter().zip(b).map(|(p, q)| p.dist(*q)).fold(0.0, f64::max)
    }

    #[test]
    fn detecta_con_rotaciones_y_distorsion_en_orden_estable() {
        let cam = truth_camera();
        // Alabeos de 0, 90, 170 y -100 grados con inclinación: el orden debe ser el
        // del modelo (origen con el cuadrado oscuro hacia +x +y) en todos.
        for (i, deg) in [
            (0.0, 0.0, 0.0),
            (15.0, -20.0, 90.0),
            (-20.0, 25.0, 170.0),
            (25.0, 10.0, -100.0),
        ]
        .into_iter()
        .enumerate()
        {
            let pose = Pose::about_center(BOARD, SQ, deg, (10.0, -10.0, 520.0));
            let truth = board_corners(&cam, &pose, BOARD, SQ);
            let img = render_board(&cam, &pose, BOARD, SQ, 1.5, 10 + i as u64);
            let found = find_chessboard(&img, BOARD).unwrap_or_else(|e| panic!("caso {i}: {e}"));
            assert_eq!(found.len(), 54);
            let err = max_err(&found, &truth);
            assert!(err < 0.35, "caso {i}: error máximo {err:.3} px");
        }
    }

    #[test]
    fn precision_subpixel_media() {
        let cam = truth_camera();
        let pose = Pose::about_center(BOARD, SQ, (20.0, -15.0, 30.0), (0.0, 0.0, 500.0));
        let truth = board_corners(&cam, &pose, BOARD, SQ);
        let img = render_board(&cam, &pose, BOARD, SQ, 1.0, 3);
        let found = find_chessboard(&img, BOARD).unwrap();
        let mean = found
            .iter()
            .zip(&truth)
            .map(|(a, b)| a.dist(*b))
            .sum::<f64>()
            / 54.0;
        assert!(mean < 0.12, "error medio {mean:.3} px");
    }

    #[test]
    fn falla_con_mensaje_claro_sin_tablero() {
        // Ruido puro.
        let mut rng = Rng::new(5);
        let px: Vec<u8> = (0..320 * 240)
            .map(|_| (128.0 + 20.0 * rng.gauss()) as u8)
            .collect();
        let img = GrayImage::from_raw(320, 240, px).unwrap();
        let e = find_chessboard(&img, BOARD).unwrap_err();
        assert_eq!(
            e,
            "no se encontró el tablero de 9x6 esquinas; verifica la iluminación y que se vea completo"
        );
        // Imagen lisa.
        let flat = GrayImage::from_pixel(200, 200, image::Luma([90]));
        assert!(find_chessboard(&flat, BOARD).is_err());
        // Tablero incompleto: se tapa una esquina.
        let cam = truth_camera();
        let pose = Pose::about_center(BOARD, SQ, (10.0, 10.0, 0.0), (0.0, 0.0, 520.0));
        let mut img = render_board(&cam, &pose, BOARD, SQ, 1.0, 1);
        let c = board_corners(&cam, &pose, BOARD, SQ);
        let p = c[0];
        for y in (p.y as u32).saturating_sub(40)..(p.y as u32 + 14) {
            for x in (p.x as u32).saturating_sub(40)..(p.x as u32 + 14) {
                img.put_pixel(x, y, image::Luma([225]));
            }
        }
        assert!(find_chessboard(&img, BOARD).is_err());
    }

    #[test]
    fn tablero_de_otro_tamano_se_reporta() {
        let cam = truth_camera();
        let pose = Pose::about_center([10, 7], SQ, (10.0, 5.0, 0.0), (0.0, 0.0, 560.0));
        let img = render_board(&cam, &pose, [10, 7], SQ, 1.0, 1);
        let e = find_chessboard(&img, BOARD).unwrap_err();
        assert!(e.contains("mayor"), "{e}");
        assert!(find_chessboard(&img, [10, 7]).is_ok());
    }
    #[test]
    fn otros_tamanos_ruido_y_fondo_con_basura() {
        let cam = truth_camera();
        for b in [[6u32, 4u32], [4, 3], [12, 8]] {
            let pose = Pose::about_center(b, 22.0, (15.0, -10.0, 20.0), (0.0, 0.0, 520.0));
            let truth = board_corners(&cam, &pose, b, 22.0);
            let img = render_board(&cam, &pose, b, 22.0, 4.0, 1);
            let f = find_chessboard(&img, b).unwrap_or_else(|e| panic!("{b:?}: {e}"));
            assert!(max_err(&f, &truth) < 0.3, "{b:?}");
        }
        // Manchas oscuras y claras alrededor del tablero.
        let pose = Pose::about_center(BOARD, SQ, (15.0, -10.0, 20.0), (0.0, 0.0, 520.0));
        let truth = board_corners(&cam, &pose, BOARD, SQ);
        let mut img = render_board(&cam, &pose, BOARD, SQ, 1.5, 1);
        for (x0, y0, w, h, v) in [
            (5u32, 5u32, 60u32, 40u32, 20u8),
            (560, 20, 70, 70, 240),
            (10, 420, 200, 40, 10),
        ] {
            for y in y0..(y0 + h).min(480) {
                for x in x0..(x0 + w).min(640) {
                    img.put_pixel(x, y, image::Luma([v]));
                }
            }
        }
        let f = find_chessboard(&img, BOARD).unwrap();
        assert!(max_err(&f, &truth) < 0.3);
    }
}
