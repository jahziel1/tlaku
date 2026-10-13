//! Geometría plana básica sobre anillos (`Vec<[f64; 2]>`): área, caja, simplificado, casco
//! convexo y desplazamientos. Sin dependencias: todo lo demás del crate se apoya aquí.

pub type Pt = [f64; 2];
pub type Ring = Vec<Pt>;

pub fn sub(a: Pt, b: Pt) -> Pt {
    [a[0] - b[0], a[1] - b[1]]
}

pub fn add(a: Pt, b: Pt) -> Pt {
    [a[0] + b[0], a[1] + b[1]]
}

/// Producto cruz de `(a - o)` y `(b - o)`: positivo si `o, a, b` giran en sentido antihorario.
pub fn cross(o: Pt, a: Pt, b: Pt) -> f64 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

/// Área con signo (positiva si el anillo va en sentido antihorario).
pub fn signed_area(r: &[Pt]) -> f64 {
    if r.len() < 3 {
        return 0.0;
    }
    let o = r[0];
    r.windows(2)
        .map(|w| (w[0][0] - o[0]) * (w[1][1] - o[1]) - (w[1][0] - o[0]) * (w[0][1] - o[1]))
        .sum::<f64>()
        / 2.0
}

/// Caja envolvente `(mínimo, máximo)`; `None` si no hay puntos.
pub fn bounds<'a>(rings: impl IntoIterator<Item = &'a Ring>) -> Option<(Pt, Pt)> {
    let mut b: Option<(Pt, Pt)> = None;
    for p in rings.into_iter().flatten() {
        match &mut b {
            None => b = Some((*p, *p)),
            Some((lo, hi)) => {
                lo[0] = lo[0].min(p[0]);
                lo[1] = lo[1].min(p[1]);
                hi[0] = hi[0].max(p[0]);
                hi[1] = hi[1].max(p[1]);
            }
        }
    }
    b
}

/// Deja el anillo antihorario (invierte si hace falta).
pub fn make_ccw(mut r: Ring) -> Ring {
    if signed_area(&r) < 0.0 {
        r.reverse();
    }
    r
}

/// Deja el anillo horario.
pub fn make_cw(mut r: Ring) -> Ring {
    if signed_area(&r) > 0.0 {
        r.reverse();
    }
    r
}

/// Quita puntos repetidos y los colineales (que no cambian la forma). Conserva el sentido.
pub fn clean_ring(r: &[Pt], eps: f64) -> Ring {
    let mut out: Ring = Vec::with_capacity(r.len());
    for &p in r {
        if out
            .last()
            .is_none_or(|q| (q[0] - p[0]).hypot(q[1] - p[1]) > eps)
        {
            out.push(p);
        }
    }
    while out.len() > 1 && {
        let (a, b) = (out[0], out[out.len() - 1]);
        (a[0] - b[0]).hypot(a[1] - b[1]) <= eps
    } {
        out.pop();
    }
    // Colineales: se repite hasta que no quede ninguno.
    loop {
        let n = out.len();
        if n < 3 {
            return out;
        }
        let mut changed = false;
        let mut keep = Vec::with_capacity(n);
        for i in 0..n {
            let (a, b, c) = (out[(i + n - 1) % n], out[i], out[(i + 1) % n]);
            let len = (c[0] - a[0]).hypot(c[1] - a[1]);
            let dist = if len > 0.0 {
                cross(a, c, b).abs() / len
            } else {
                0.0
            };
            if dist <= eps * 0.1 {
                changed = true;
            } else {
                keep.push(b);
            }
        }
        out = keep;
        if !changed {
            return out;
        }
    }
}

fn dist_to_chord(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    if len2 == 0.0 {
        return (p[0] - a[0]).hypot(p[1] - a[1]);
    }
    cross(a, b, p).abs() / len2.sqrt()
}

/// Douglas-Peucker sobre una cadena abierta: marca los puntos que se conservan.
fn dp_chain(pts: &[Pt], eps: f64, keep: &mut [bool]) {
    let n = pts.len();
    let mut stack = vec![(0usize, n - 1)];
    while let Some((i, j)) = stack.pop() {
        if j <= i + 1 {
            continue;
        }
        let (mut far, mut far_d) = (i, 0.0);
        for k in i + 1..j {
            let d = dist_to_chord(pts[k], pts[i], pts[j]);
            if d > far_d {
                far_d = d;
                far = k;
            }
        }
        if far_d > eps {
            keep[far] = true;
            stack.push((i, far));
            stack.push((far, j));
        }
    }
}

/// Douglas-Peucker sobre un anillo cerrado: cada punto original queda a `eps` o menos del
/// anillo simplificado.
pub fn simplify_ring(r: &[Pt], eps: f64) -> Ring {
    let n = r.len();
    if n <= 3 || eps <= 0.0 {
        return r.to_vec();
    }
    // Dos anclas lejanas: el punto más a la izquierda y el más lejano a él.
    let a = (0..n)
        .min_by(|&i, &j| {
            r[i][0]
                .total_cmp(&r[j][0])
                .then(r[i][1].total_cmp(&r[j][1]))
        })
        .unwrap_or(0);
    let b = (0..n)
        .max_by(|&i, &j| {
            let di = (r[i][0] - r[a][0]).hypot(r[i][1] - r[a][1]);
            let dj = (r[j][0] - r[a][0]).hypot(r[j][1] - r[a][1]);
            di.total_cmp(&dj)
        })
        .unwrap_or(0);
    if a == b {
        return r.to_vec();
    }
    let (lo, hi) = (a.min(b), a.max(b));
    let chain1: Vec<Pt> = r[lo..=hi].to_vec();
    let chain2: Vec<Pt> = r[hi..].iter().chain(r[..=lo].iter()).copied().collect();
    let mut out = Vec::new();
    for chain in [&chain1, &chain2] {
        let mut keep = vec![false; chain.len()];
        keep[0] = true;
        keep[chain.len() - 1] = true;
        dp_chain(chain, eps, &mut keep);
        // El último punto de cada cadena es el primero de la siguiente: no se repite.
        for (p, k) in chain.iter().zip(&keep).take(chain.len() - 1) {
            if *k {
                out.push(*p);
            }
        }
    }
    if out.len() < 3 { r.to_vec() } else { out }
}

/// Casco convexo antihorario (cadena monótona de Andrew).
pub fn convex_hull(points: &[Pt]) -> Ring {
    let mut p: Vec<Pt> = points.to_vec();
    p.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    p.dedup();
    if p.len() < 3 {
        return p;
    }
    let mut h: Ring = Vec::with_capacity(p.len() * 2);
    for &q in &p {
        while h.len() >= 2 && cross(h[h.len() - 2], h[h.len() - 1], q) <= 0.0 {
            h.pop();
        }
        h.push(q);
    }
    let lower = h.len() + 1;
    for &q in p.iter().rev().skip(1) {
        while h.len() >= lower && cross(h[h.len() - 2], h[h.len() - 1], q) <= 0.0 {
            h.pop();
        }
        h.push(q);
    }
    h.pop();
    h
}

/// ¿El anillo (antihorario) es convexo? Tolera colineales.
#[cfg(test)]
pub fn is_convex(r: &[Pt]) -> bool {
    let n = r.len();
    n >= 3 && (0..n).all(|i| cross(r[i], r[(i + 1) % n], r[(i + 2) % n]) >= -1e-12)
}

/// Distancia mínima de un punto a un segmento.
#[cfg(test)]
pub fn dist_point_segment(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    if len2 == 0.0 {
        return (p[0] - a[0]).hypot(p[1] - a[1]);
    }
    let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0);
    (p[0] - (a[0] + t * dx)).hypot(p[1] - (a[1] + t * dy))
}
