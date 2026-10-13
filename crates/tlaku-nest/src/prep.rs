//! Preparación de piezas y contenedor: contornos limpios, simplificado conservador,
//! separación (se infla cada pieza la mitad de la separación) y descomposición convexa.
//!
//! Conservador significa que el contorno que usa el motor **contiene** a la pieza real: así,
//! si dos contornos del motor no se solapan, las piezas reales no se tocan y distan al menos
//! la separación pedida. El simplificado (Douglas-Peucker con tolerancia `eps`) se compensa
//! inflando `eps` de más; en los huecos y en el contenedor el desplazamiento va hacia adentro.

use crate::convex::convex_parts;
use crate::geom::{Pt, Ring, bounds, clean_ring, make_ccw, signed_area, simplify_ring};
use crate::region::{offset, weld};
use tlaku_core::Polyline;

/// Tolerancia numérica (mm): las piezas pueden invadir la separación en este margen, lo que
/// permite que ajustes exactos (rectángulos que llenan la cama) sigan siendo posibles.
pub const TOL_MM: f64 = 1e-3;
/// Tolerancia inicial del simplificado conservador (mm).
const SIMPLIFY_MM: f64 = 0.05;
/// Vértices máximos del contorno de una pieza para el cálculo de NFP.
const MAX_VERTS: usize = 40;
/// Vértices máximos del contorno del contenedor (solo entra en el cálculo del IFP).
const MAX_CONTAINER_VERTS: usize = 160;
/// Esquinas más agudas que esto (razón punta/distancia) se recortan en los offsets.
const MITER_LIMIT: f64 = 20.0;

pub struct PieceType {
    /// Área real (sin huecos) en mm².
    pub true_area: f64,
    /// Área del contorno inflado (para ordenar).
    pub inflated_area: f64,
    /// Punto de referencia en el documento (centro de la caja original).
    pub c0: Pt,
    /// Partes convexas del contorno inflado, con el origen en `c0`.
    pub parts: Vec<Ring>,
    /// Huecos aprovechables (antihorarios, ya reducidos por la separación), origen en `c0`.
    pub holes: Vec<Ring>,
}

fn to_ring(p: &Polyline) -> Ring {
    p.points.iter().map(|q| [q.x, q.y]).collect()
}

/// Región (exteriores antihorarios, huecos horarios) de un conjunto de contornos cerrados.
pub fn region_rings(paths: &[Polyline]) -> Vec<Ring> {
    let closed: Vec<Ring> = paths
        .iter()
        .filter(|p| {
            p.points.len() >= 3 && p.points.iter().all(|q| q.x.is_finite() && q.y.is_finite())
        })
        .map(to_ring)
        .collect();
    weld(&closed)
}

/// Simplifica todos los anillos con la misma tolerancia, subiéndola hasta que ningún anillo
/// pase de `MAX_VERTS` vértices. Devuelve los anillos y la tolerancia efectivamente usada
/// (0 si no se cambió nada).
fn simplify_all(rings: &[Ring], max_verts: usize) -> (Vec<Ring>, f64) {
    // Si el contorno ya es liviano solo se quitan los puntos redundantes (sin perder nada).
    let exact: Vec<Ring> = rings.iter().map(|r| clean_ring(r, 1e-9)).collect();
    if exact.iter().all(|r| r.len() <= max_verts && r.len() >= 3) {
        return (exact, 0.0);
    }
    let mut eps = SIMPLIFY_MM;
    loop {
        let out: Vec<Ring> = rings
            .iter()
            .map(|r| {
                let s = clean_ring(&simplify_ring(r, eps), 1e-9);
                if s.len() >= 3 && signed_area(&s).abs() > 1e-9 {
                    s
                } else {
                    r.clone()
                }
            })
            .collect();
        let max_v = out.iter().map(Vec::len).max().unwrap_or(0);
        if max_v <= max_verts || eps > 5.0 {
            let changed = out.iter().zip(rings).any(|(a, b)| a.len() != b.len());
            return (out, if changed { eps } else { 0.0 });
        }
        eps *= 1.6;
    }
}

fn offset_rings(rings: &[Ring], d: f64) -> Vec<Ring> {
    offset(rings, d, MITER_LIMIT)
}

/// Prepara una pieza. `None` si no tiene área.
pub fn prepare_piece(region: &[Polyline], spacing: f64, use_holes: bool) -> Option<PieceType> {
    let rings = region_rings(region);
    let true_area: f64 = rings.iter().map(|r| signed_area(r)).sum();
    if rings.is_empty() || true_area <= 1e-9 {
        return None;
    }
    let (lo, hi) = bounds(&rings)?;
    let c0 = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    let (simple, eps) = simplify_all(&rings, MAX_VERTS);
    let d = spacing / 2.0 + eps;
    let inflated = if d > 1e-9 {
        offset_rings(&simple, d)
    } else {
        simple
    };

    let shift = |r: &Ring| -> Ring { r.iter().map(|p| [p[0] - c0[0], p[1] - c0[1]]).collect() };
    let mut parts = Vec::new();
    let mut inflated_area = 0.0;
    let mut holes = Vec::new();
    for r in &inflated {
        let a = signed_area(r);
        if a > 1e-9 {
            inflated_area += a;
            parts.extend(convex_parts(&clean_ring(&shift(r), 1e-9)));
        } else if a < -1e-9 && use_holes {
            // El hueco libre es la zona interior; se agranda `TOL_MM` por la tolerancia.
            let free = make_ccw(shift(r));
            if let Some(g) = offset_rings(std::slice::from_ref(&free), TOL_MM)
                .into_iter()
                .find(|g| signed_area(g) > 0.0)
            {
                holes.push(g);
            } else {
                holes.push(free);
            }
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(PieceType {
        true_area,
        inflated_area,
        c0,
        parts,
        holes,
    })
}

/// Pieza vacía para las entradas con cero copias (nunca se coloca).
pub fn empty_piece() -> PieceType {
    PieceType {
        true_area: 0.0,
        inflated_area: 0.0,
        c0: [0.0; 2],
        parts: Vec::new(),
        holes: Vec::new(),
    }
}

pub enum ContainerData {
    /// Rectángulo ya desplazado (zona permitida para el contorno inflado) y su caja en `[lo, hi]`.
    Rect { lo: Pt, hi: Pt },
    /// Región libre ya desplazada: anillos orientados.
    Region(Vec<Ring>),
}

pub struct Container {
    pub data: ContainerData,
    /// Área neta real del contenedor (mm²).
    pub net_area: f64,
    /// Esquina inferior izquierda real (origen de las medidas de compactación).
    pub origin: Pt,
}

/// Prepara el contenedor. `margin` es la distancia mínima al borde de la pieza real.
pub fn prepare_container(
    rect: Option<(f64, f64)>,
    region: &[Polyline],
    spacing: f64,
    margin: f64,
) -> Option<Container> {
    // La pieza inflada puede llegar a `margin - spacing/2` del borde (negativo = lo rebasa).
    let e = margin - spacing / 2.0;
    if let Some((w, h)) = rect {
        if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
            return None;
        }
        return Some(Container {
            data: ContainerData::Rect {
                lo: [e - TOL_MM, e - TOL_MM],
                hi: [w - e + TOL_MM, h - e + TOL_MM],
            },
            net_area: w * h,
            origin: [0.0, 0.0],
        });
    }
    let rings = region_rings(region);
    let net_area: f64 = rings.iter().map(|r| signed_area(r)).sum();
    if rings.is_empty() || net_area <= 1e-9 {
        return None;
    }
    let (lo, _) = bounds(&rings)?;
    let (simple, eps) = simplify_all(&rings, MAX_CONTAINER_VERTS);
    // Hacia adentro `e + eps` y, a la vez, la tolerancia hacia afuera.
    let d = -(e + eps) + TOL_MM;
    let free = if d.abs() > 1e-9 {
        offset_rings(&simple, d)
    } else {
        simple
    };
    Some(Container {
        data: ContainerData::Region(free),
        net_area,
        origin: lo,
    })
}
