use tlaku_core::Bounds;
use tlaku_core::toolpath::{Move, Toolpath, ToolpathBlock};
use tlaku_doc::{DeviceProfile, JobPreview};
use tlaku_plan::Plan;

/// Margen (mm) para no rechazar un diseño que toca el borde por error de redondeo.
const BED_EPS_MM: f64 = 1e-6;

/// Resumen y detalle de un bloque del trabajo (una capa, o una imagen).
#[derive(Debug, Clone)]
pub struct LayerReport {
    /// `#rrggbb` (con ` · 2/3` si es una de las operaciones de una capa con sub-capas), o
    /// `imagen`.
    pub name: String,
    pub color: tlaku_core::Rgb,
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
    /// Puentes colocados (solo capas line con puentes).
    pub tabs: usize,
    /// Z (mm) al empezar el bloque, si la máquina tiene eje Z.
    pub z_mm: Option<f64>,
    /// `(pasada, total)` cuando la capa se divide en un bloque por pasada (Z distinta en cada
    /// una); `None` si el bloque contiene todas sus pasadas.
    pub pass: Option<(u32, u32)>,
    /// El bloque va con el segundo láser (`LayerSettings::laser2`).
    pub laser2: bool,
    /// `(posición, total)` de la operación entre las de su capa, si ésta tiene sub-capas (la
    /// principal es `1/total`). El nombre también lo lleva ("#rrggbb · 2/3").
    pub sub: Option<(u32, u32)>,
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
            .reduce(tlaku_doc::union)
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
    check_area(job, bed, true)
}

/// Como [`check_bounds`] con el perfil de la máquina. Con rotativo la Y no tiene límite de
/// recorrido (la pieza gira), así que solo se verifica la X; que el diseño quepa en la
/// circunferencia se avisa al calcular el trabajo (ver `build`). En una máquina galvo se
/// comprueba el campo de la lente (centrado en la cama).
pub fn check_device_bounds(job: &ComputedJob, device: &DeviceProfile) -> Result<(), String> {
    // El galvo tiene un campo de lente, no una cama: ver `galvo::check_field`.
    if device.kind == tlaku_doc::DeviceKind::Galvo {
        return crate::galvo::check_field(job, device);
    }
    check_area(job, device.bed, !device.rotary.enabled)
}

fn check_area(job: &ComputedJob, bed: [f64; 2], check_y: bool) -> Result<(), String> {
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
            || b.max.x > bed[0] + BED_EPS_MM
            || (check_y && (b.min.y < -BED_EPS_MM || b.max.y > bed[1] + BED_EPS_MM))
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
