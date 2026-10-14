use crate::job::{ComputedJob, LayerReport};
use crate::rotary;
use tlaku_core::toolpath::{PowerMode, Toolpath};
use tlaku_core::{Bounds, Point};
use tlaku_doc::{DeviceProfile, Origin};
use tlaku_grbl::frame::frame_gcode_scaled;
use tlaku_grbl::gcode::{GcodeOptions, generate_mapped};

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

fn script(text: &str) -> Option<String> {
    (!text.trim().is_empty()).then(|| text.to_string())
}

/// Suma `min + max` de la Y de todos los movimientos (con `map` aplicado): con ella se refleja la
/// Y dentro de la caja del trabajo (`suma - y`), de modo que la caja queda igual y el diseño sale
/// espejado en el eje del rotativo. `None` si no hay ninguna Y finita.
fn mirror_sum(tp: &Toolpath, map: impl Fn(Point) -> Point) -> Option<f64> {
    let (lo, hi) = tp
        .blocks
        .iter()
        .flat_map(|b| &b.moves)
        .map(|m| map(m.to()).y)
        .filter(|y| y.is_finite())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), y| {
            (lo.min(y), hi.max(y))
        });
    (lo <= hi).then_some(lo + hi)
}

/// G-code del trabajo para esa máquina: dialecto del perfil, modo de potencia, S máxima,
/// origen de la cama, espejo y escala del rotativo, sentido de Z y guiones de inicio y fin.
pub fn gcode(job: &ComputedJob, device: &DeviceProfile) -> String {
    let toolpath = &job.toolpath;
    let mode = match device.power_mode {
        tlaku_doc::PowerMode::Dynamic => PowerMode::Dynamic,
        tlaku_doc::PowerMode::Constant => PowerMode::Constant,
    };
    // Las coordenadas se pasan a las de la máquina al escribir cada punto, sin copiar el
    // trabajo (con una imagen grande son cientos de MB).
    let to_machine = |p: Point| point_to_machine(p, device.bed, device.origin);
    let mirror = if device.rotary.enabled && device.rotary.mirror {
        mirror_sum(toolpath, to_machine)
    } else {
        None
    };
    let map = |p: Point| {
        let p = to_machine(p);
        match mirror {
            Some(sum) => Point::new(p.x, sum - p.y),
            None => p,
        }
    };
    generate_mapped(
        toolpath,
        &GcodeOptions {
            dialect: device.dialect.clone(),
            // Si el rotativo es inválido `compute` ya falló; aquí se cae a Y sin escalar.
            y_scale: rotary::y_scale(device).unwrap_or(1.0),
            s_max: device.s_max,
            return_to_origin: device.return_to_origin,
            start_gcode: script(&device.start_gcode),
            end_gcode: script(&device.end_gcode),
            rapid_feed: Some(device.rapid_rate),
            z_invert: device.z_invert,
            merge_tolerance_mm: tlaku_grbl::gcode::DEFAULT_MERGE_TOLERANCE_MM,
            // Sin segundo láser en el perfil todas las capas salen con el láser 1 (ver
            // `dialect_warnings`).
            block_laser2: if device.laser2 {
                job.layers.iter().map(|l| l.laser2).collect()
            } else {
                Vec::new()
            },
        },
        Some(mode),
        &map,
    )
}

/// Carga que el trabajo pone al enlace serie de `baud` baudios: líneas por segundo en su tramo más
/// denso (ventana de 1 s, sobre el trabajo ya fusionado como lo escribe [`gcode`]) y largo medio
/// de línea del `gcode` generado. Pásala a `tlaku_grbl::preflight::preflight_link_with_load`.
pub fn link_load(
    job: &ComputedJob,
    device: &DeviceProfile,
    gcode: &str,
    baud: u32,
) -> tlaku_grbl::preflight::LinkLoad {
    // Mismos límites que el estimado de `compute`; con rotativo, en mm de superficie.
    let k = rotary::y_scale(device).unwrap_or(1.0);
    let profile = tlaku_sim::MachineProfile {
        max_rate_mm_min: [device.max_rate[0], device.max_rate[1] / k],
        accel_mm_s2: [device.accel[0], device.accel[1] / k],
        rapid_rate_mm_min: device.rapid_rate,
        ..tlaku_sim::MachineProfile::default()
    };
    let compact = tlaku_grbl::gcode::compact_toolpath(
        &job.toolpath,
        tlaku_grbl::gcode::DEFAULT_MERGE_TOLERANCE_MM,
    );
    let rate = tlaku_sim::line_rate(Point::new(0.0, 0.0), &compact, &profile, 1.0);
    tlaku_grbl::preflight::LinkLoad::new(baud, rate.peak_per_s, gcode.lines().count(), gcode.len())
}

/// Avisos que dependen de lo que el dialecto de la máquina no puede hacer.
pub(crate) fn dialect_warnings(
    toolpath: &Toolpath,
    layers: &[LayerReport],
    device: &DeviceProfile,
) -> Vec<String> {
    // El galvo no usa G-code: su dialecto no cuenta.
    if device.kind == tlaku_doc::DeviceKind::Galvo {
        return Vec::new();
    }
    let d = &device.dialect;
    let mut out = Vec::new();
    if d.air_on.is_none() && toolpath.blocks.iter().any(|b| b.air_assist) {
        out.push(format!(
            "la máquina '{}' no tiene comando de aire en su dialecto de G-code: la asistencia de \
             aire de las capas se ignora",
            device.name
        ));
    }
    if layers.iter().any(|l| l.laser2) {
        if !device.laser2 {
            out.push(format!(
                "la máquina '{}' no tiene segundo láser: las capas que piden el láser 2 salen con \
                 el láser 1",
                device.name
            ));
        } else if d.laser2_select.is_none() {
            out.push(format!(
                "el dialecto de la máquina '{}' no tiene comando para seleccionar el láser 2: las \
                 capas del láser 2 salen sin cambiar de láser (configúralo en el dialecto de G-code)",
                device.name
            ));
        }
    }
    if device.power_mode == tlaku_doc::PowerMode::Dynamic && d.dynamic_on.is_none() {
        out.push(format!(
            "la máquina '{}' no tiene potencia dinámica en su dialecto: se usa potencia constante{}",
            device.name,
            if d.constant_on.is_empty() {
                String::new()
            } else {
                format!(" ({})", d.constant_on)
            }
        ));
    }
    if layers.iter().any(|l| l.mode == "image") {
        if !d.s_inline {
            out.push(format!(
                "la máquina '{}' pone la potencia en una línea aparte: cada cambio de S frena el \
                 cabezal y el grabado de imágenes sale con manchas; usa un dialecto con S en línea",
                device.name
            ));
        } else if device.kind == tlaku_doc::DeviceKind::GrblM3 {
            out.push(format!(
                "la máquina '{}' es GRBL sin modo láser: cada cambio de S pausa el movimiento y \
                 el grabado de imágenes sale con manchas; actualiza a GRBL 1.1f o posterior",
                device.name
            ));
        }
    }
    if d.s_decimals == 0 && device.s_max < 100.0 {
        out.push(format!(
            "la máquina '{}' escribe S sin decimales pero su S máxima es {}: la potencia solo \
             tiene {} nivel(es); sube la S máxima o los decimales de S",
            device.name,
            device.s_max,
            device.s_max.max(0.0).round() as u64 + 1
        ));
    }
    out
}

/// G-code del encuadre del trabajo (rectángulo envolvente con overscan, láser apagado) en
/// coordenadas de la máquina: aplica el mismo cambio de origen y la misma escala de Y del
/// rotativo que [`gcode`].
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
    frame_gcode_scaled(
        machine,
        speed_mm_min,
        rotary::y_scale(device).unwrap_or(1.0),
    )
}
