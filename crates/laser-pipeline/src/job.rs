use laser_core::Bounds;
use laser_core::toolpath::{Move, Toolpath, ToolpathBlock};
use laser_doc::JobPreview;
use laser_plan::Plan;

/// Margen (mm) para no rechazar un diseño que toca el borde por error de redondeo.
const BED_EPS_MM: f64 = 1e-6;

/// Resumen y detalle de un bloque del trabajo (una capa, o una imagen).
#[derive(Debug, Clone)]
pub struct LayerReport {
    /// `#rrggbb`, o `imagen`.
    pub name: String,
    pub color: laser_core::Rgb,
    /// `line`, `fill` o `image`.
    pub mode: &'static str,
    pub count: usize,
    /// Qué se cuenta en `count`: `trayectorias` o `líneas`.
    pub unit: &'static str,
    pub speed_mm_min: f64,
    pub power: String,
    pub passes: u32,
    pub overscan_mm: Option<f64>,
    /// Plan de corte (solo capas line).
    pub plan: Option<Plan>,
    pub seconds: f64,
    /// Caja de todos los movimientos, incluido el overscan.
    pub bounds: Option<Bounds>,
}

/// Resultado de calcular un trabajo: lo necesario para mostrarlo y para generar G-code.
#[derive(Debug, Clone)]
pub struct ComputedJob {
    /// Coordenadas del documento (origen abajo a la izquierda); `gcode` las adapta a la máquina.
    pub toolpath: Toolpath,
    /// Una entrada por bloque del `toolpath`, en el mismo orden.
    pub layers: Vec<LayerReport>,
    pub preview: JobPreview,
}

impl ComputedJob {
    /// Caja envolvente de todos los movimientos, incluido el overscan.
    pub fn bounds(&self) -> Option<Bounds> {
        self.layers
            .iter()
            .filter_map(|l| l.bounds)
            .reduce(laser_doc::union)
    }
}

pub fn block_bounds(block: &ToolpathBlock) -> Option<Bounds> {
    let mut it = block.moves.iter().map(Move::to);
    let first = it.next()?;
    let mut b = Bounds {
        min: first,
        max: first,
    };
    for p in it {
        b.min.x = b.min.x.min(p.x);
        b.min.y = b.min.y.min(p.y);
        b.max.x = b.max.x.max(p.x);
        b.max.y = b.max.y.max(p.y);
    }
    Some(b)
}

/// Verifica que TODOS los movimientos (incluido el overscan) caigan dentro de la cama
/// `ancho x alto`, con el origen en la esquina inferior izquierda.
pub fn check_bounds(job: &ComputedJob, bed: [f64; 2]) -> Result<(), String> {
    let mut problems = Vec::new();
    for (layer, block) in job.layers.iter().zip(&job.toolpath.blocks) {
        let bad_point = block
            .moves
            .iter()
            .map(Move::to)
            .any(|p| !(p.x.is_finite() && p.y.is_finite()));
        if bad_point {
            problems.push(format!(
                "la capa {} tiene coordenadas que no son números",
                layer.name
            ));
            continue;
        }
        let Some(b) = layer.bounds else { continue };
        if b.min.x < -BED_EPS_MM
            || b.min.y < -BED_EPS_MM
            || b.max.x > bed[0] + BED_EPS_MM
            || b.max.y > bed[1] + BED_EPS_MM
        {
            let os = match layer.overscan_mm {
                Some(mm) if mm > 0.0 => format!(" (incluye {mm:.2} mm de overscan)"),
                _ => String::new(),
            };
            problems.push(format!(
                "la capa {} ({}) se sale del área de trabajo de {}x{} mm: ocupa X de {:.2} a {:.2} mm, \
                 Y de {:.2} a {:.2} mm{os}",
                layer.name, layer.mode, bed[0], bed[1], b.min.x, b.max.x, b.min.y, b.max.y
            ));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        problems.push(
            "mueve el diseño dentro del área (o usa una máquina con más área de trabajo)".into(),
        );
        Err(problems.join("\n"))
    }
}
