use crate::job::ComputedJob;
use laser_core::Point;
use laser_core::toolpath::{Move, PowerMode, Toolpath};
use laser_doc::{DeviceProfile, Origin};
use laser_grbl::gcode::{GcodeOptions, generate};

/// Pasa de coordenadas del documento (origen abajo a la izquierda) a las de la máquina,
/// cuyo X0 Y0 está en la esquina `origin` y crecen hacia dentro de la cama.
fn to_machine(tp: &mut Toolpath, bed: [f64; 2], origin: Origin) {
    let (flip_x, flip_y) = match origin {
        Origin::BottomLeft => return,
        Origin::TopLeft => (false, true),
        Origin::BottomRight => (true, false),
        Origin::TopRight => (true, true),
    };
    let map = |p: Point| {
        Point::new(
            if flip_x { bed[0] - p.x } else { p.x },
            if flip_y { bed[1] - p.y } else { p.y },
        )
    };
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
        laser_doc::PowerMode::Dynamic => PowerMode::Dynamic,
        laser_doc::PowerMode::Constant => PowerMode::Constant,
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
