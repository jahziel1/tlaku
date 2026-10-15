//! Puntos de la cama que visita el cabezal.

use serde::{Deserialize, Serialize};
use tlaku_i18n::tr;

/// Lado mínimo y máximo de la rejilla (puntos por lado).
pub const MIN_GRID: usize = 3;
pub const MAX_GRID: usize = 6;
/// Medida mínima del área segura (mm): con menos, los puntos quedan demasiado juntos.
const MIN_SPAN_MM: f64 = 40.0;

/// Cómo se reparten los puntos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanOptions {
    /// Ancho y alto de la cama (mm).
    pub bed: [f64; 2],
    /// Puntos por lado de la rejilla (3 = 3x3).
    pub grid: usize,
    /// Margen libre a cada lado de la cama (mm): el cabezal no se acerca a los bordes.
    pub margin_mm: f64,
    /// Zona de la cama que ve la cámara `[x0, y0, x1, y1]` (mm); `None` = toda la cama.
    #[serde(default)]
    pub region: Option<[f64; 4]>,
}

impl Default for PlanOptions {
    fn default() -> Self {
        Self {
            bed: [400.0, 400.0],
            grid: 3,
            margin_mm: 20.0,
            region: None,
        }
    }
}

/// Puntos a visitar y el punto de la foto de referencia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeadAlignPlan {
    /// Puntos de la rejilla (mm de documento), en orden de recorrido (serpentina).
    pub points: Vec<[f64; 2]>,
    /// Posición del cabezal en la foto de referencia (otra posición, libre de la rejilla, en la
    /// misma fila que el primer punto).
    pub reference: [f64; 2],
    /// Área segura usada `[x0, y0, x1, y1]` (mm).
    pub area: [f64; 4],
}

/// Rejilla `grid x grid` repartida de lado a lado del área segura (la cama menos el margen,
/// recortada a la zona de la cámara), recorrida en serpentina. La referencia está en la primera
/// fila, a mitad de camino entre sus dos primeros puntos: otro lugar, dentro del área segura y
/// lejos de los de la rejilla.
pub fn plan_grid(opts: &PlanOptions) -> Result<HeadAlignPlan, String> {
    let n = opts.grid;
    if !(MIN_GRID..=MAX_GRID).contains(&n) {
        return Err(tr!(
            "camera.head_align.plan.grid_range",
            min = MIN_GRID,
            max = MAX_GRID
        ));
    }
    let finite = opts.bed.iter().all(|v| v.is_finite() && *v > 0.0)
        && opts.margin_mm.is_finite()
        && opts.margin_mm >= 0.0;
    if !finite {
        return Err(tr!("camera.head_align.plan.bad_bed"));
    }
    let m = opts.margin_mm;
    let mut area = [m, m, opts.bed[0] - m, opts.bed[1] - m];
    if let Some(r) = opts.region {
        if r.iter().any(|v| !v.is_finite()) {
            return Err(tr!("camera.head_align.plan.bad_region"));
        }
        let (rx0, rx1) = (r[0].min(r[2]), r[0].max(r[2]));
        let (ry0, ry1) = (r[1].min(r[3]), r[1].max(r[3]));
        area = [
            area[0].max(rx0),
            area[1].max(ry0),
            area[2].min(rx1),
            area[3].min(ry1),
        ];
    }
    if area[2] - area[0] < MIN_SPAN_MM || area[3] - area[1] < MIN_SPAN_MM {
        return Err(tr!(
            "camera.head_align.plan.too_small",
            min = MIN_SPAN_MM as u32
        ));
    }
    Ok(grid_over(area, n))
}

/// `n x n` puntos de lado a lado de `area`, en serpentina, con la referencia en la primera celda.
fn grid_over(area: [f64; 4], n: usize) -> HeadAlignPlan {
    let step = |lo: f64, hi: f64, i: usize| lo + (hi - lo) * i as f64 / (n - 1) as f64;
    let round = |v: f64| (v * 100.0).round() / 100.0;
    let mut points = Vec::with_capacity(n * n);
    for j in 0..n {
        let y = round(step(area[1], area[3], j));
        for k in 0..n {
            let i = if j % 2 == 0 { k } else { n - 1 - k };
            points.push([round(step(area[0], area[2], i)), y]);
        }
    }
    // En la primera fila (misma Y que sus puntos): el pórtico queda igual en esas fotos y la
    // diferencia de imágenes lo trata como fondo.
    let reference = [
        round(area[0] + (area[2] - area[0]) / (2 * (n - 1)) as f64),
        round(area[1]),
    ];
    HeadAlignPlan {
        points,
        reference,
        area,
    }
}

/// Nueva rejilla alrededor de donde ya se vio el cabezal (`seen`, mm), para cámaras que cubren
/// solo parte de la cama: la caja de lo visto crece `grow` (fracción de su tamaño) a cada lado y
/// se recorta al área segura. Una caja casi sin tamaño (un solo punto visto) se abre `probe_mm`
/// a cada lado. `None` si no hay nada visto o la caja ya ocupa toda el área segura.
pub fn plan_refine(
    area: [f64; 4],
    seen: &[[f64; 2]],
    grid: usize,
    grow: f64,
    probe_mm: f64,
) -> Option<HeadAlignPlan> {
    if seen.is_empty() || !(MIN_GRID..=MAX_GRID).contains(&grid) {
        return None;
    }
    let lo = |i: usize| seen.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min);
    let hi = |i: usize| seen.iter().map(|p| p[i]).fold(f64::NEG_INFINITY, f64::max);
    let mut r = [lo(0), lo(1), hi(0), hi(1)];
    for k in 0..2 {
        let span = r[k + 2] - r[k];
        let pad = if span < MIN_SPAN_MM / 2.0 {
            probe_mm
        } else {
            span * grow
        };
        r[k] -= pad;
        r[k + 2] += pad;
    }
    let clipped = [
        r[0].max(area[0]),
        r[1].max(area[1]),
        r[2].min(area[2]),
        r[3].min(area[3]),
    ];
    if clipped[2] - clipped[0] < MIN_SPAN_MM || clipped[3] - clipped[1] < MIN_SPAN_MM {
        return None;
    }
    let full = (area[2] - area[0]) * (area[3] - area[1]);
    let size = (clipped[2] - clipped[0]) * (clipped[3] - clipped[1]);
    (size < 0.98 * full).then(|| grid_over(clipped, grid))
}
