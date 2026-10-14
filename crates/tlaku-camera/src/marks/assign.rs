//! Asignación de candidatos a posiciones teóricas, con y sin alineación provisional.

use super::MarkLayout;
use super::candidates::Cand;
use crate::homography::{Homography, estimate_homography};
use crate::imgproc::FloatImage;
use tlaku_core::Point;

// ---------------------------------------------------------------------------
// Asignación
// ---------------------------------------------------------------------------

/// Orientación (signo del área) del triángulo `a, b, c`.
fn orient(a: Point, b: Point, c: Point) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// Escala local de una homografía en `p`: raíz del determinante del jacobiano.
pub(super) fn local_scale(h: &Homography, p: Point) -> f64 {
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
pub(super) type Assignment = Vec<Option<usize>>;

/// Con una alineación provisional: el candidato más cercano a cada posición prevista,
/// dentro de un radio del 7 % de la mayor dimensión de la imagen.
pub(super) fn assign_with_prior(
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
pub(super) fn assign_blind(
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
