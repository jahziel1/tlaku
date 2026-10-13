use crate::nesting::analyze;
use crate::offset::{Join, offset};
use crate::util::clean_closed;
use laser_core::Polyline;

/// Tolerancia de los arcos (mm) de [`kerf_compensate`].
const KERF_TOLERANCE: f64 = 0.005;

/// Compensa el kerf (ancho del corte) desplazando las trayectorias de corte.
///
/// **Convención de signo** (la queja n.º 1 del foro sobre kerf):
/// `kerf_offset_mm` es cuánto se desplaza *cada trayectoria*, normalmente la
/// mitad del ancho del corte.
///
/// - **Positivo**: las piezas físicas salen del tamaño del dibujo. Las
///   trayectorias de profundidad par (contornos exteriores, islas) se agrandan
///   y las de profundidad impar (agujeros) se encogen. Así el haz, que quita
///   material a ambos lados de la trayectoria, deja la pieza (el material que
///   se conserva) justo del tamaño dibujado, y los agujeros también.
/// - **Negativo**: lo contrario (exteriores se encogen, agujeros crecen), para
///   piezas deliberadamente más pequeñas o ajustes de encaje.
/// - **Cero**: no cambia nada.
///
/// La clasificación pieza/agujero sale solo de [`nesting_depth`](crate::nesting_depth)
/// (regla par-impar), nunca del sentido de giro del dibujo ni de la capa.
/// Cada pieza se compensa junto con sus propios agujeros y de forma
/// independiente de las demás: dos piezas solapadas no se funden. Una pieza
/// que desaparece por el desplazamiento se omite.
///
/// Las abiertas y las degeneradas pasan sin cambios al final de la salida; la
/// compensación no aplica a abiertas. Usa uniones redondas (la forma real del
/// haz); para otro estilo, [`kerf_compensate_with`].
pub fn kerf_compensate(paths: &[Polyline], kerf_offset_mm: f64) -> Vec<Polyline> {
    kerf_compensate_with(paths, kerf_offset_mm, Join::Round, KERF_TOLERANCE)
}

/// Como [`kerf_compensate`] con el estilo de esquina y la tolerancia de arcos a elección.
pub fn kerf_compensate_with(
    paths: &[Polyline],
    kerf_offset_mm: f64,
    join: Join,
    tolerance: f64,
) -> Vec<Polyline> {
    let clean = clean_closed(paths);
    let (depth, parent) = analyze(&clean);

    // Cada pieza (profundidad par) con sus agujeros directos.
    let mut groups: Vec<Vec<Polyline>> = Vec::new();
    let mut group_of = vec![usize::MAX; clean.len()];
    for (i, path) in clean.iter().enumerate() {
        if depth[i] % 2 == 0 {
            group_of[i] = groups.len();
            groups.push(vec![path.clone()]);
        }
    }
    for (i, path) in clean.iter().enumerate() {
        if depth[i] % 2 == 1
            && let Some(p) = parent[i]
            && group_of[p] != usize::MAX
        {
            groups[group_of[p]].push(path.clone());
        }
    }

    let mut out: Vec<Polyline> = groups
        .iter()
        .flat_map(|g| offset(g, kerf_offset_mm, join, tolerance))
        .collect();

    // Lo que no se puede compensar pasa tal cual.
    out.extend(
        paths
            .iter()
            .filter(|p| !p.closed || clean_closed(std::slice::from_ref(p)).is_empty())
            .cloned(),
    );
    out
}
