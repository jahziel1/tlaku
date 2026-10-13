//! Descomposición de un polígono simple en piezas convexas: triangulación por orejas y
//! fusión de Hertel-Mehlhorn (se quitan las diagonales que dejan la unión convexa). Ideas
//! estándar de geometría computacional; implementación propia.

use crate::geom::{Pt, Ring, convex_hull, cross, signed_area};

/// Desviación (mm) que se tolera para tratar un polígono como convexo. Los contornos que salen
/// de los desplazamientos traen ruido de redondeo; sin tolerancia un círculo se partiría en
/// decenas de piezas. Cada pieza final se reemplaza por su casco convexo, que la contiene.
const CONVEX_TOL_MM: f64 = 1e-4;

/// ¿Es convexo salvo por desviaciones de `CONVEX_TOL_MM`?
fn is_convex_tol(r: &[Pt]) -> bool {
    let n = r.len();
    n >= 3
        && (0..n).all(|i| {
            let (a, b, c) = (r[i], r[(i + 1) % n], r[(i + 2) % n]);
            let chord = (c[0] - a[0]).hypot(c[1] - a[1]);
            chord == 0.0 || cross(a, c, b) <= CONVEX_TOL_MM * chord
        })
}

fn in_triangle(p: Pt, a: Pt, b: Pt, c: Pt) -> bool {
    // a, b, c antihorario: dentro o sobre el borde si ningún cruce es negativo.
    cross(a, b, p) >= -1e-12 && cross(b, c, p) >= -1e-12 && cross(c, a, p) >= -1e-12
}

/// Triangula un polígono simple antihorario. Devuelve ternas de índices del anillo.
fn triangulate(poly: &[Pt]) -> Vec<[usize; 3]> {
    let n = poly.len();
    let mut idx: Vec<usize> = (0..n).collect();
    let mut tris = Vec::with_capacity(n.saturating_sub(2));
    let mut guard = 0usize;
    while idx.len() > 3 {
        let m = idx.len();
        let mut clipped = false;
        for k in 0..m {
            let (ia, ib, ic) = (idx[(k + m - 1) % m], idx[k], idx[(k + 1) % m]);
            let (a, b, c) = (poly[ia], poly[ib], poly[ic]);
            if cross(a, b, c) <= 1e-12 {
                continue; // esquina cóncava o colineal: no es oreja
            }
            let blocked = idx.iter().any(|&j| {
                j != ia && j != ib && j != ic && {
                    let p = poly[j];
                    // Los puntos que repiten un vértice del triángulo no bloquean.
                    p != a && p != b && p != c && in_triangle(p, a, b, c)
                }
            });
            if blocked {
                continue;
            }
            tris.push([ia, ib, ic]);
            idx.remove(k);
            clipped = true;
            break;
        }
        if !clipped {
            // Polígono degenerado (casi autointersecante): se recorta la esquina más convexa
            // para no quedar en un ciclo infinito.
            guard += 1;
            let m = idx.len();
            let k = (0..m)
                .max_by(|&i, &j| {
                    let f = |k: usize| {
                        cross(
                            poly[idx[(k + m - 1) % m]],
                            poly[idx[k]],
                            poly[idx[(k + 1) % m]],
                        )
                    };
                    f(i).total_cmp(&f(j))
                })
                .unwrap_or(0);
            tris.push([idx[(k + m - 1) % m], idx[k], idx[(k + 1) % m]]);
            idx.remove(k);
            if guard > n {
                break;
            }
        }
    }
    if idx.len() == 3 {
        tris.push([idx[0], idx[1], idx[2]]);
    }
    tris
}

/// Une dos polígonos (listas de índices antihorarias) que comparten la arista `u -> v` en
/// `p` y `v -> u` en `q`. `None` si no comparten arista.
fn merge_pair(p: &[usize], q: &[usize]) -> Option<Vec<usize>> {
    let (np, nq) = (p.len(), q.len());
    for i in 0..np {
        let (u, v) = (p[i], p[(i + 1) % np]);
        for j in 0..nq {
            if q[j] == v && q[(j + 1) % nq] == u {
                // p desde v hasta u (sin repetir) y luego q desde u+1 hasta v-1.
                let mut out = Vec::with_capacity(np + nq - 2);
                for k in 0..np {
                    out.push(p[(i + 1 + k) % np]);
                }
                // `out` termina en u; sigue el resto de q entre u y v.
                for k in 2..nq {
                    out.push(q[(j + k) % nq]);
                }
                return Some(out);
            }
        }
    }
    None
}

/// Piezas convexas (antihorarias) cuya unión es el polígono simple `poly` (antihorario).
pub fn convex_parts(poly: &[Pt]) -> Vec<Ring> {
    if poly.len() < 3 || signed_area(poly) <= 0.0 {
        return Vec::new();
    }
    if is_convex_tol(poly) {
        return vec![convex_hull(poly)];
    }
    let mut parts: Vec<Vec<usize>> = triangulate(poly).into_iter().map(|t| t.to_vec()).collect();
    // Hertel-Mehlhorn: fusiona mientras el resultado siga siendo convexo.
    loop {
        let mut merged = false;
        'outer: for i in 0..parts.len() {
            for j in i + 1..parts.len() {
                if let Some(m) = merge_pair(&parts[i], &parts[j]) {
                    let ring: Ring = m.iter().map(|&k| poly[k]).collect();
                    if is_convex_tol(&ring) {
                        parts[i] = m;
                        parts.swap_remove(j);
                        merged = true;
                        break 'outer;
                    }
                }
            }
        }
        if !merged {
            break;
        }
    }
    parts
        .into_iter()
        .map(|p| convex_hull(&p.iter().map(|&k| poly[k]).collect::<Ring>()))
        .filter(|r| signed_area(r) > 1e-12)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area_sum(parts: &[Ring]) -> f64 {
        parts.iter().map(|p| signed_area(p)).sum()
    }

    #[test]
    fn la_ele_se_parte_en_convexas_con_la_misma_area() {
        let l = vec![
            [0.0, 0.0],
            [20.0, 0.0],
            [20.0, 10.0],
            [10.0, 10.0],
            [10.0, 30.0],
            [0.0, 30.0],
        ];
        let parts = convex_parts(&l);
        assert!(parts.len() >= 2 && parts.len() <= 3, "{}", parts.len());
        assert!(parts.iter().all(|p| crate::geom::is_convex(p)));
        assert!((area_sum(&parts) - signed_area(&l)).abs() < 1e-9);
    }

    #[test]
    fn una_estrella_conserva_el_area() {
        let mut star = Vec::new();
        for i in 0..10 {
            let r = if i % 2 == 0 { 20.0 } else { 8.0 };
            let a = i as f64 * std::f64::consts::PI / 5.0;
            star.push([r * a.cos(), r * a.sin()]);
        }
        let parts = convex_parts(&star);
        assert!(parts.iter().all(|p| crate::geom::is_convex(p)));
        assert!((area_sum(&parts) - signed_area(&star)).abs() < 1e-9);
    }

    #[test]
    fn un_convexo_queda_entero() {
        let sq = vec![[0.0, 0.0], [5.0, 0.0], [5.0, 5.0], [0.0, 5.0]];
        assert_eq!(convex_parts(&sq).len(), 1);
    }
}
