//! Sobrante de una hoja: la hoja menos las piezas colocadas (con un margen de seguridad).
//! Sirve para guardar el retazo y reutilizarlo como contenedor de otro trabajo.

use crate::geom::{Ring, signed_area};
use crate::ops::{difference, union_rings};
use crate::prep::region_rings;
use crate::region::{offset, point_in_ring};
use crate::{NestContainer, SheetsInput, SheetsOutcome};
use tlaku_core::{Point, Polyline};

/// Esquinas más agudas que esto (razón punta/distancia) se recortan en los desplazamientos.
const MITER_LIMIT: f64 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeftoverOptions {
    /// Distancia (mm) que el retazo guarda de las piezas colocadas (suele ser la separación).
    pub clearance: f64,
    /// Ancho mínimo (mm): lo más angosto que esto no es aprovechable y se descarta (tiras y astillas).
    pub min_width: f64,
    /// Área mínima (mm²) de un retazo.
    pub min_area: f64,
}

impl Default for LeftoverOptions {
    fn default() -> Self {
        Self {
            clearance: 0.0,
            min_width: 10.0,
            min_area: 400.0,
        }
    }
}

fn to_polyline(r: &Ring) -> Polyline {
    Polyline {
        points: r.iter().map(|p| Point::new(p[0], p[1])).collect(),
        closed: true,
    }
}

/// Retazos que quedan de `container` tras quitar `placed` (los contornos de cada pieza ya
/// colocada, en coordenadas de la hoja). Cada elemento es un retazo conexo: su contorno exterior
/// seguido de sus huecos, todo cerrado. El orden va de mayor a menor área. Es conservador:
/// el retazo nunca invade el margen alrededor de las piezas ni sale de la hoja.
pub fn leftover(
    container: &NestContainer,
    placed: &[Vec<Polyline>],
    opts: &LeftoverOptions,
) -> Vec<Vec<Polyline>> {
    let sheet: Vec<Ring> = match container {
        NestContainer::Rect { width, height } => {
            if !(width.is_finite() && height.is_finite() && *width > 0.0 && *height > 0.0) {
                return Vec::new();
            }
            vec![vec![
                [0.0, 0.0],
                [*width, 0.0],
                [*width, *height],
                [0.0, *height],
            ]]
        }
        NestContainer::Region(paths) => region_rings(paths),
    };
    if sheet.is_empty() {
        return Vec::new();
    }
    // Las piezas se toman macizas (sin sus huecos): el hueco de una pieza es su propio desecho.
    let mut solids: Vec<Ring> = Vec::new();
    for region in placed {
        for r in region_rings(region) {
            if signed_area(&r) <= 0.0 {
                continue;
            }
            if opts.clearance > 1e-9 {
                solids.extend(
                    offset(std::slice::from_ref(&r), opts.clearance, MITER_LIMIT)
                        .into_iter()
                        .filter(|g| signed_area(g) > 0.0),
                );
            } else {
                solids.push(r);
            }
        }
    }
    let mut rest = difference(&sheet, &union_rings(&solids));
    let r = opts.min_width / 2.0;
    if r > 1e-9 && !rest.is_empty() {
        // Apertura morfológica: lo que no deja pasar un disco de ancho `min_width` se descarta.
        let eroded = offset(&rest, -r, MITER_LIMIT);
        let opened = offset(&eroded, r, MITER_LIMIT);
        let removed = difference(&rest, &opened);
        rest = difference(&rest, &removed);
    }

    let outers: Vec<&Ring> = rest.iter().filter(|g| signed_area(g) > 0.0).collect();
    let mut groups: Vec<(f64, Vec<Ring>)> = outers
        .iter()
        .map(|o| (signed_area(o), vec![(*o).clone()]))
        .collect();
    for h in rest.iter().filter(|g| signed_area(g) < 0.0) {
        // El hueco pertenece al exterior más pequeño que lo contiene.
        let owner = outers
            .iter()
            .enumerate()
            .filter(|(_, o)| point_in_ring(h[0], o))
            .min_by(|a, b| signed_area(a.1).total_cmp(&signed_area(b.1)))
            .map(|(i, _)| i);
        if let Some(i) = owner {
            groups[i].1.push(h.clone());
            groups[i].0 += signed_area(h);
        }
    }
    groups.retain(|(area, _)| *area >= opts.min_area);
    groups.sort_by(|a, b| b.0.total_cmp(&a.0));
    groups
        .into_iter()
        .map(|(_, rings)| rings.iter().map(to_polyline).collect())
        .collect()
}

/// Sobrante de cada hoja usada de un resultado de [`crate::nest_sheets`], en el mismo orden que
/// `out.sheets` y en las coordenadas de cada hoja.
pub fn sheet_leftovers(
    input: &SheetsInput,
    out: &SheetsOutcome,
    opts: &LeftoverOptions,
) -> Vec<Vec<Vec<Polyline>>> {
    out.sheets
        .iter()
        .map(|sheet| {
            let container = input
                .sheets
                .get(sheet.index)
                .or(input.refill.as_ref())
                .expect("la hoja usada sale de la lista o de la hoja nueva");
            let placed: Vec<Vec<Polyline>> = sheet
                .placements
                .iter()
                .map(|pl| {
                    let t = pl.transform;
                    input.pieces[pl.piece]
                        .region
                        .iter()
                        .map(|p| Polyline {
                            points: p
                                .points
                                .iter()
                                .map(|q| {
                                    Point::new(
                                        t[0] * q.x + t[2] * q.y + t[4],
                                        t[1] * q.x + t[3] * q.y + t[5],
                                    )
                                })
                                .collect(),
                            closed: p.closed,
                        })
                        .collect()
                })
                .collect();
            leftover(container, &placed, opts)
        })
        .collect()
}
