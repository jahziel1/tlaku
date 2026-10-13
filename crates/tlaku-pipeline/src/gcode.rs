use crate::job::ComputedJob;
use tlaku_core::toolpath::{Move, PowerMode, Toolpath};
use tlaku_core::{Bounds, Point};
use tlaku_doc::{DeviceProfile, Origin};
use tlaku_grbl::gcode::{GcodeOptions, generate};

/// Pasa de coordenadas del documento (origen abajo a la izquierda) a las de la máquina,
/// cuyo X0 Y0 está en la esquina `origin` y crecen hacia dentro de la cama. Es su propia
/// inversa, así que también pasa de máquina a documento.
pub(crate) fn point_to_machine(p: Point, bed: [f64; 2], origin: Origin) -> Point {
    let (flip_x, flip_y) = match origin {
        Origin::BottomLeft => (false, false),
        Origin::TopLeft => (false, true),
        Origin::BottomRight => (true, false),
        Origin::TopRight => (true, true),
    };
    Point::new(
        if flip_x { bed[0] - p.x } else { p.x },
        if flip_y { bed[1] - p.y } else { p.y },
    )
}

fn to_machine(tp: &mut Toolpath, bed: [f64; 2], origin: Origin) {
    if origin == Origin::BottomLeft {
        return;
    }
    let map = |p: Point| point_to_machine(p, bed, origin);
    for mv in tp.blocks.iter_mut().flat_map(|b| &mut b.moves) {
        match mv {
            Move::Travel { to } | Move::Cut { to, .. } => *to = map(*to),
        }
    }
}

fn script(text: &str) -> Option<String> {
    (!text.trim().is_empty()).then(|| text.to_string())
}

/// G-code GRBL del trabajo para esa máquina: modo de potencia, S máxima, origen de la cama y
/// guiones de inicio y fin.
pub fn gcode(job: &ComputedJob, device: &DeviceProfile) -> String {
    let mut toolpath = job.toolpath.clone();
    let mode = match device.power_mode {
        tlaku_doc::PowerMode::Dynamic => PowerMode::Dynamic,
        tlaku_doc::PowerMode::Constant => PowerMode::Constant,
    };
    for block in &mut toolpath.blocks {
        block.power_mode = mode;
    }
    to_machine(&mut toolpath, device.bed, device.origin);
    generate(
        &toolpath,
        &GcodeOptions {
            s_max: device.s_max,
            return_to_origin: device.return_to_origin,
            start_gcode: script(&device.start_gcode),
            end_gcode: script(&device.end_gcode),
            ..GcodeOptions::default()
        },
    )
}

/// G-code del encuadre del trabajo (rectángulo envolvente con overscan, láser apagado) en
/// coordenadas de la máquina: aplica el mismo cambio de origen que [`gcode`].
pub fn frame_gcode(job: &ComputedJob, device: &DeviceProfile, speed_mm_min: f64) -> Vec<String> {
    let Some(b) = job.bounds() else {
        return Vec::new();
    };
    let a = point_to_machine(b.min, device.bed, device.origin);
    let c = point_to_machine(b.max, device.bed, device.origin);
    let machine = Bounds {
        min: Point::new(a.x.min(c.x), a.y.min(c.y)),
        max: Point::new(a.x.max(c.x), a.y.max(c.y)),
    };
    tlaku_grbl::frame::frame_gcode(machine, speed_mm_min)
}
