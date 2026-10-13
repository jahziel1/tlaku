//! Booleanas sobre anillos y suma de Minkowski, la base de los polígonos de no-ajuste (NFP)
//! y de ajuste interior (IFP).
//!
//! - NFP(A, B): posiciones del origen de B para las que B solapa a A. Es la suma de Minkowski
//!   `A ⊕ (−B)`; para piezas no convexas se calcula como la unión de las sumas de sus partes
//!   convexas (la descomposición está en [`crate::convex`]).
//! - IFP(R, B): posiciones del origen de B para las que B queda completamente dentro de la
//!   región R. Para una región cualquiera se obtiene restando de un marco la suma de Minkowski
//!   de su complemento con `−B`. Esa suma se calcula sin descomponer el complemento, con el
//!   hecho de que para un conjunto convexo K y cualquier k0 de K, `O ⊕ K = (O + k0) ∪ ⋃ (e ⊕ K)`
//!   con `e` cada arista del borde de O.

use crate::geom::{Pt, Ring, add, bounds, convex_hull, make_ccw, make_cw, signed_area, sub};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::overlay::FloatOverlay;

/// Área mínima (mm²) de un anillo resultante de una booleana; por debajo es ruido de redondeo.
const MIN_RING_AREA: f64 = 1e-9;

fn flatten(shapes: Vec<Vec<Vec<[f64; 2]>>>) -> Vec<Ring> {
    let mut out = Vec::new();
    for shape in shapes {
        for (i, c) in shape.into_iter().enumerate() {
            if c.len() < 3 || signed_area(&c).abs() < MIN_RING_AREA {
                continue;
            }
            out.push(if i == 0 { make_ccw(c) } else { make_cw(c) });
        }
    }
    out
}

/// Unión de anillos orientados (exteriores antihorarios, huecos horarios) con regla no-cero.
pub fn union_rings(rings: &[Ring]) -> Vec<Ring> {
    if rings.is_empty() {
        return Vec::new();
    }
    flatten(
        FloatOverlay::<[f64; 2]>::with_subj(&rings.to_vec())
            .overlay(OverlayRule::Subject, FillRule::NonZero),
    )
}

/// `subject` menos `clip` (ambos con anillos orientados).
pub fn difference(subject: &[Ring], clip: &[Ring]) -> Vec<Ring> {
    if subject.is_empty() {
        return Vec::new();
    }
    if clip.is_empty() {
        return subject.to_vec();
    }
    flatten(
        FloatOverlay::<[f64; 2]>::with_subj_and_clip(&subject.to_vec(), &clip.to_vec())
            .overlay(OverlayRule::Difference, FillRule::NonZero),
    )
}

/// Reflexión por el origen (conserva el sentido antihorario).
pub fn negate(r: &[Pt]) -> Ring {
    r.iter().map(|p| [-p[0], -p[1]]).collect()
}

/// Suma de Minkowski de dos polígonos convexos antihorarios.
pub fn minkowski_convex(p: &[Pt], q: &[Pt]) -> Ring {
    fn start(r: &[Pt]) -> usize {
        (0..r.len())
            .min_by(|&i, &j| {
                r[i][1]
                    .total_cmp(&r[j][1])
                    .then(r[i][0].total_cmp(&r[j][0]))
            })
            .unwrap_or(0)
    }
    let (n, m) = (p.len(), q.len());
    let (si, sj) = (start(p), start(q));
    let pp = |i: usize| p[(si + i) % n];
    let qq = |j: usize| q[(sj + j) % m];
    let (mut i, mut j) = (0usize, 0usize);
    let mut out = Vec::with_capacity(n + m);
    while i < n || j < m {
        out.push(add(pp(i), qq(j)));
        let c = {
            let ep = sub(pp(i + 1), pp(i));
            let eq = sub(qq(j + 1), qq(j));
            ep[0] * eq[1] - ep[1] * eq[0]
        };
        if c >= 0.0 && i < n {
            i += 1;
        }
        if c <= 0.0 && j < m {
            j += 1;
        }
    }
    out
}

/// NFP de A con B: unión de las sumas de las partes convexas de A con las de `−B`.
pub fn nfp(a_parts: &[Ring], b_neg_parts: &[Ring]) -> Vec<Ring> {
    let mut sums = Vec::with_capacity(a_parts.len() * b_neg_parts.len());
    for a in a_parts {
        for b in b_neg_parts {
            sums.push(minkowski_convex(a, b));
        }
    }
    union_rings(&sums)
}

/// Suma de Minkowski de la arista `a -> b` con el polígono convexo `k`.
fn sweep(a: Pt, b: Pt, k: &[Pt]) -> Ring {
    let d = sub(b, a);
    let mut pts = Vec::with_capacity(k.len() * 2);
    for &p in k {
        pts.push(add(p, a));
        pts.push(add(add(p, a), d));
    }
    convex_hull(&pts)
}

fn rect_ring(lo: Pt, hi: Pt) -> Ring {
    vec![lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]]
}

/// IFP de una región cualquiera `free` (exteriores antihorarios, huecos horarios) para una
/// pieza cuyas partes convexas reflejadas son `b_neg_parts` y cuya caja mide `w x h` con su
/// esquina inferior izquierda en el origen. El resultado es un conjunto de anillos orientados.
pub fn ifp_general(free: &[Ring], b_neg_parts: &[Ring], w: f64, h: f64) -> Vec<Ring> {
    let Some((lo, hi)) = bounds(free) else {
        return Vec::new();
    };
    let big_lo = [lo[0] - w - 1.0, lo[1] - h - 1.0];
    let big_hi = [hi[0] + w + 1.0, hi[1] + h + 1.0];
    let big = rect_ring(big_lo, big_hi);
    // Aristas del borde del complemento: marco y contornos de la región.
    let mut edges: Vec<(Pt, Pt)> = Vec::new();
    for ring in std::iter::once(&big).chain(free.iter()) {
        let n = ring.len();
        for i in 0..n {
            edges.push((ring[i], ring[(i + 1) % n]));
        }
    }
    let mut forbidden: Vec<Ring> = Vec::new();
    for k in b_neg_parts {
        let k0 = k[0];
        // O + k0: el marco con la región como hueco (orientación invertida).
        forbidden.push(big.iter().map(|&p| add(p, k0)).collect());
        for r in free {
            let mut moved: Ring = r.iter().map(|&p| add(p, k0)).collect();
            moved.reverse();
            forbidden.push(moved);
        }
        for &(a, b) in &edges {
            forbidden.push(sweep(a, b, k));
        }
    }
    let union = union_rings(&forbidden);
    let t_lo = big_lo;
    let t_hi = [big_hi[0] - w, big_hi[1] - h];
    if t_hi[0] <= t_lo[0] || t_hi[1] <= t_lo[1] {
        return Vec::new();
    }
    difference(&[rect_ring(t_lo, t_hi)], &union)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: f64, h: f64) -> Ring {
        vec![[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]]
    }

    fn total_area(rings: &[Ring]) -> f64 {
        rings.iter().map(|r| signed_area(r)).sum()
    }

    #[test]
    fn minkowski_de_dos_rectangulos() {
        let s = minkowski_convex(&rect(4.0, 2.0), &rect(1.0, 3.0));
        assert!((signed_area(&s) - 5.0 * 5.0).abs() < 1e-9);
    }

    #[test]
    fn nfp_de_dos_cuadrados_es_un_cuadrado_doble() {
        let a = vec![rect(50.0, 50.0)];
        let b = vec![negate(&rect(50.0, 50.0))];
        let n = nfp(&a, &b);
        assert_eq!(n.len(), 1);
        assert!((total_area(&n) - 100.0 * 100.0).abs() < 1e-6);
        let (lo, hi) = bounds(&n).unwrap();
        assert!((lo[0] + 50.0).abs() < 1e-6 && (hi[0] - 50.0).abs() < 1e-6);
    }

    #[test]
    fn ifp_general_de_un_rectangulo_coincide_con_la_forma_cerrada() {
        let region = vec![rect(100.0, 60.0)];
        let ifp = ifp_general(&region, &[negate(&rect(30.0, 20.0))], 30.0, 20.0);
        // Posiciones válidas del origen: [0, 70] x [0, 40].
        assert!(
            (total_area(&ifp) - 70.0 * 40.0).abs() < 1e-3,
            "{}",
            total_area(&ifp)
        );
        let (lo, hi) = bounds(&ifp).unwrap();
        assert!(lo[0].abs() < 1e-3 && lo[1].abs() < 1e-3);
        assert!((hi[0] - 70.0).abs() < 1e-3 && (hi[1] - 40.0).abs() < 1e-3);
    }

    #[test]
    fn ifp_general_de_una_region_en_ele() {
        let l = vec![
            [0.0, 0.0],
            [100.0, 0.0],
            [100.0, 20.0],
            [20.0, 20.0],
            [20.0, 100.0],
            [0.0, 100.0],
        ];
        // Un cuadrado de 20x20 cabe en cualquier punto de la L: origen en [0,80]x[0,0] o [0,0]x[0,80].
        let ifp = ifp_general(&[l], &[negate(&rect(20.0, 20.0))], 20.0, 20.0);
        // Dos bandas degeneradas de ancho cero: con área casi nula (el ajuste exacto).
        assert!(total_area(&ifp) < 1e-3, "{}", total_area(&ifp));
        // Un cuadrado de 10x10 sí deja bandas con área.
        let ifp2 = ifp_general(
            &[vec![
                [0.0, 0.0],
                [100.0, 0.0],
                [100.0, 20.0],
                [20.0, 20.0],
                [20.0, 100.0],
                [0.0, 100.0],
            ]],
            &[negate(&rect(10.0, 10.0))],
            10.0,
            10.0,
        );
        // Bandas: 90x10 (horizontal) + 10x80 (vertical, sin el cuadrado común) = 900 + 800.
        assert!(
            (total_area(&ifp2) - 1700.0).abs() < 1e-3,
            "{}",
            total_area(&ifp2)
        );
    }
}
