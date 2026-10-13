//! Modo polígono: sigue el borde de los píxeles (aristas entre tinta y fondo) y lo simplifica
//! con Ramer-Douglas-Peucker. Implementación propia, sin dependencias de trazado.

use crate::binarize::Bitmap;

/// Contornos de una forma en píxeles: el exterior y sus huecos (cada uno es un lazo cerrado).
pub(crate) type PixelShape = Vec<Vec<(f64, f64)>>;

// Direcciones en pantalla (Y hacia abajo), en sentido horario: E, S, O, N.
const DX: [i64; 4] = [1, 0, -1, 0];
const DY: [i64; 4] = [0, 1, 0, -1];

/// Traza todas las formas del mapa. `tolerance` (px) es el error máximo de la simplificación.
pub(crate) fn trace_polygons(bm: &Bitmap, tolerance: f64) -> Vec<PixelShape> {
    let (w, h) = (bm.w, bm.h);
    let vw = w + 1;
    let ink_at = |x: i64, y: i64| -> bool {
        x >= 0 && y >= 0 && x < w as i64 && y < h as i64 && bm.ink[y as usize * w + x as usize]
    };

    // Aristas dirigidas con la tinta a la derecha: máscara de salidas por vértice.
    let mut mask = vec![0u8; vw * (h + 1)];
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            if !ink_at(x, y) {
                continue;
            }
            let v = |vx: i64, vy: i64| vy as usize * vw + vx as usize;
            if !ink_at(x, y - 1) {
                mask[v(x, y)] |= 1 << 0; // E por arriba
            }
            if !ink_at(x + 1, y) {
                mask[v(x + 1, y)] |= 1 << 1; // S por la derecha
            }
            if !ink_at(x, y + 1) {
                mask[v(x + 1, y + 1)] |= 1 << 2; // O por abajo
            }
            if !ink_at(x - 1, y) {
                mask[v(x, y + 1)] |= 1 << 3; // N por la izquierda
            }
        }
    }

    let (labels, count) = bm.label_components();
    let mut shapes: Vec<PixelShape> = vec![Vec::new(); count];

    for sv in 0..mask.len() {
        while mask[sv] != 0 {
            let start_dir = mask[sv].trailing_zeros() as usize;
            let (sx, sy) = ((sv % vw) as i64, (sv / vw) as i64);
            // Píxel de tinta a la derecha de la arista inicial (da la etiqueta de la forma).
            let (px, py) = match start_dir {
                0 | 3 => (sx, sy),
                1 => (sx - 1, sy),
                _ => (sx, sy - 1),
            };
            let label = labels[py as usize * w + px as usize] as usize;

            let mut pts: Vec<(i64, i64)> = vec![(sx, sy)];
            let (mut cx, mut cy, mut dir) = (sx, sy, start_dir);
            let limit = mask.len() * 2;
            for _ in 0..limit {
                mask[cy as usize * vw + cx as usize] &= !(1 << dir);
                cx += DX[dir];
                cy += DY[dir];
                let m = mask[cy as usize * vw + cx as usize]
                    | if (cx, cy) == (sx, sy) {
                        1 << start_dir
                    } else {
                        0
                    };
                // Preferencia: giro a la derecha, recto, giro a la izquierda (tinta conectada en cruz).
                let next = [(dir + 1) % 4, dir, (dir + 3) % 4, (dir + 2) % 4]
                    .into_iter()
                    .find(|d| m & (1 << d) != 0);
                let Some(next) = next else { break };
                if (cx, cy) == (sx, sy) && next == start_dir {
                    break;
                }
                if next != dir {
                    pts.push((cx, cy));
                }
                dir = next;
            }
            let pts: Vec<(f64, f64)> = pts.into_iter().map(|(x, y)| (x as f64, y as f64)).collect();
            let simplified = simplify_closed(&pts, tolerance);
            if simplified.len() >= 3 {
                shapes[label].push(simplified);
            }
        }
    }
    shapes.retain(|s| !s.is_empty());
    shapes
}

fn dist_to_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.0 - (a.0 + t * dx)).hypot(p.1 - (a.1 + t * dy))
}

/// RDP sobre una cadena abierta; marca en `keep` los puntos que se conservan.
fn rdp(pts: &[(f64, f64)], lo: usize, hi: usize, tol: f64, keep: &mut [bool]) {
    let mut stack = vec![(lo, hi)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let (mut far, mut far_d) = (a, 0.0);
        for (i, &p) in pts.iter().enumerate().take(b).skip(a + 1) {
            let d = dist_to_segment(p, pts[a], pts[b]);
            if d > far_d {
                (far, far_d) = (i, d);
            }
        }
        if far_d > tol {
            keep[far] = true;
            stack.push((a, far));
            stack.push((far, b));
        }
    }
}

/// Simplifica un lazo cerrado: parte del punto 0 y del más lejano a él y aplica RDP a cada mitad.
fn simplify_closed(pts: &[(f64, f64)], tol: f64) -> Vec<(f64, f64)> {
    let n = pts.len();
    if n <= 4 || tol <= 0.0 {
        return pts.to_vec();
    }
    let far = (1..n)
        .max_by(|&i, &j| {
            let d = |k: usize| (pts[k].0 - pts[0].0).hypot(pts[k].1 - pts[0].1);
            d(i).total_cmp(&d(j))
        })
        .unwrap_or(1);
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[far] = true;
    rdp(pts, 0, far, tol, &mut keep);
    // Segunda mitad: far..n y vuelta al 0 (se duplica el punto 0 al final).
    let mut second: Vec<(f64, f64)> = pts[far..].to_vec();
    second.push(pts[0]);
    let mut keep2 = vec![false; second.len()];
    keep2[0] = true;
    keep2[second.len() - 1] = true;
    rdp(&second, 0, second.len() - 1, tol, &mut keep2);

    let mut out: Vec<(f64, f64)> = (0..far).filter(|&i| keep[i]).map(|i| pts[i]).collect();
    out.extend(
        (0..second.len() - 1)
            .filter(|&i| keep2[i])
            .map(|i| second[i]),
    );
    drop_collinear(out)
}

/// Quita los vértices alineados con sus vecinos (p. ej. el punto de arranque del lazo).
fn drop_collinear(pts: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    let n = pts.len();
    if n < 4 {
        return pts;
    }
    let keep: Vec<bool> = (0..n)
        .map(|i| dist_to_segment(pts[i], pts[(i + n - 1) % n], pts[(i + 1) % n]) > 1e-9)
        .collect();
    pts.into_iter()
        .zip(keep)
        .filter_map(|(p, k)| k.then_some(p))
        .collect()
}
